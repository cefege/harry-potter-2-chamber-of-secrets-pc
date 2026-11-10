//! Direct Core package saving with an owned LinkerSave record and file transaction.
//!
//! The record keeps the measured native prefix first and places Rust-only save
//! state after it; neither the prefix nor the tail is ever passed to C++.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_records::LinkerSaveRecord;
use crate::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use crate::package_archive::{
    GenerationInfo, OwnedPackage, PackageArchiveWriter, PackageExport, PackageImport,
    PackageName, PackageSummary,
};
use crate::package_flow::package_save::{self, PackageSaveOps, SaveContext};
use crate::runtime_archive::RuntimeArchive;
use hp2_abi::engine::generated::UObject;
use std::alloc::Layout;
use std::cell::RefCell;
use crate::fast_hash::{FastMap as HashMap, FastSet as HashSet};
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
#[derive(Debug, Default)]
struct ConformState {
    names: Vec<i32>,
    name_texts: Vec<Vec<u32>>,
    imports: Vec<PackageImport>,
    records: Vec<PackageExport>,
    guid: [u32; 4],
    generations: Vec<GenerationInfo>,
}

impl ConformState {
    fn from_package(access: CoreAccess<'_>, package: &OwnedPackage) -> RuntimeResult<Self> {
        let names = package.names().iter()
            .map(|entry| access.intern(&entry.text, crate::name_owner::FindName::Add))
            .collect::<RuntimeResult<Vec<_>>>()?;
        Ok(Self {
            names,
            name_texts: package.names().iter().map(|entry| entry.text.clone()).collect(),
            imports: package.imports().to_vec(),
            records: package.exports().to_vec(),
            guid: package.summary().guid,
            generations: package.summary().generations.clone(),
        })
    }
}

impl CoreAccess<'_> {
    /// Saves a canonical package graph to a finalized package79 file.
    ///
    /// # Safety
    /// `outer`, `base`, and every traversed object must belong to this Rust Core
    /// owner and remain live during traversal. Native C++ object graphs are not
    /// supported. Serialization callbacks may reenter Core but must not destroy
    /// the selected graph or the temporary LinkerSave object.
    ///
    /// # Errors
    /// Returns the first traversal, archive, or cleanup error.
    ///
    /// # Return value
    /// Returns `false` if the finalized temporary file cannot be committed to `filename`.
    pub unsafe fn save_package(
        self,
        outer: *mut UObject,
        base: *mut UObject,
        top_level_flags: u32,
        filename: &Path,
        conform: Option<&OwnedPackage>,
    ) -> RuntimeResult<bool> {
        let access = self;
        if outer.is_null() {
            return Err(save_error("Package save requires an outer"));
        }
        if filename.as_os_str().is_empty() {
            return Err(save_error("Package save requires a filename"));
        }
        let save_class = access.native_class("LinkerSave")?;
        let name = access.intern_str("LinkerSave", crate::name_owner::FindName::Add)?;
        // SAFETY: LinkerSave is a registered Rust class and the caller guarantees
        // the package roots remain live through this synchronous save operation.
        let linker = unsafe {
            access.construct_object(save_class, outer, name, RF_TRANSIENT, ptr::null_mut())?
        };
        let linker_index = unsafe { (*linker).index };
        let result = (|| {
            let has_conform = conform.is_some();
            let mut conform_state = conform.map(|package| ConformState::from_package(access, package)).transpose()?;
            let final_path = filename.to_path_buf();
            let temporary_path = final_path.parent()
                .map(|parent| parent.join("Save.tmp"))
                .unwrap_or_else(|| PathBuf::from("Save.tmp"));
            let mut summary = PackageSummary {
                tag: hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.file_tag as u32,
                version: hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.version as u16,
                licensee_version: hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.licensee as u16,
                package_flags: 0,
                name_count: 0,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
                guid: conform_state.as_ref().map_or([0; 4], |state| state.guid),
                generations: {
                    let mut generations = conform_state.as_mut().map_or_else(Vec::new, |state| std::mem::take(&mut state.generations));
                    generations.push(GenerationInfo { export_count: 0, name_count: 0 });
                    generations
                },
            };
            if conform_state.is_none() {
                fill_guid(&mut summary.guid)?;
            }
            if crate::object_model::is_a(outer.cast(), access.native_class("Package")?.cast()) {
                let package = outer.cast::<crate::core_records::PackageRecord>();
                unsafe { (*package).package_flags |= PKG_ALLOW_DOWNLOAD; }
                summary.package_flags = unsafe { (*package).package_flags };
            }
            let archive = RuntimeArchive::create(&temporary_path).map_err(|error| {
                RuntimeError::new("Failed to create package archive")
                    .with_context(&error.to_string())
                    .with_source_path(&temporary_path)
            })?;
            let conform_state = conform_state.unwrap_or_default();
            let tail = SaveTail {
                archive: Some(archive),
                temporary_path,
                final_path,
                summary,
                names: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                object_indices: HashMap::default(),
                name_indices: HashMap::default(),
                name_usage: HashMap::default(),
                conform_names: conform_state.names,
                original_names: None,
                conform_name_texts: conform_state.name_texts,
                conform_imports: conform_state.imports,
                conform_records: conform_state.records,
                original_exports: Vec::new(),
                original_export_used: Vec::new(),
                conform_name_map: HashMap::default(),
                has_conform,
                committed: false,
            };
            // SAFETY: `linker` is the canonical published LinkerSave allocation,
            // whose class constructor initialized this `RefCell` exactly once.
            unsafe { (*linker.cast::<OwnedLinkerSave>()).tail.replace(Some(tail)); }
            let mut ops = DirectSaveOps { access, linker, outer, base, top_level_flags, has_conform };
            let saved = package_save::save_package(&mut ops);
            let cleanup = ops.clear_save_tags();
            saved?;
            cleanup?;
            ops.close_and_preserve()
        })();
        let destroy = unsafe { access.destroy_slot(linker_index) };
        let free = unsafe { access.free_slot(linker_index) };
        match (result, destroy, free) {
            (Ok(paths), Ok(()), Ok(())) => {
                match fs::rename(&paths.temporary, &paths.final_path) {
                    Ok(()) => Ok(true),
                    Err(_) => {
                        let _ = fs::remove_file(&paths.temporary);
                        Ok(false)
                    }
                }
            }
            (Err(error), _, _) => Err(error),
            (Ok(paths), Err(error), _) | (Ok(paths), Ok(()), Err(error)) => {
                let _ = fs::remove_file(paths.temporary);
                Err(error)
            }
        }
    }
}
const RF_TRANSIENT: u32 = 0x0000_4000;
const RF_TAG_EXP: u32 = 0x0000_0010;
const RF_TAG_IMP: u32 = 0x0000_0008;
const RF_PUBLIC: u32 = 0x0000_0004;
const PKG_ALLOW_DOWNLOAD: u32 = 0x0000_0001;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TagTraversal { None, Exports, Imports }

#[derive(Clone, Debug)]
struct SaveExport {
    object: *mut UObject,
    record: PackageExport,
}

#[derive(Clone, Debug)]
struct SaveImport {
    object: *mut UObject,
    record: PackageImport,
}

impl OwnedLinkerSave {
    fn with_tail<R>(&self, f: impl FnOnce(&mut SaveTail) -> RuntimeResult<R>) -> RuntimeResult<R> {
        let mut tail = self.tail.borrow_mut();
        let tail = tail.as_mut().ok_or_else(|| save_error("LinkerSave state is not initialized"))?;
        f(tail)
    }

    fn tail(&self) -> RuntimeResult<std::cell::Ref<'_, SaveTail>> {
        std::cell::Ref::filter_map(self.tail.borrow(), Option::as_ref)
            .map_err(|_| save_error("LinkerSave state is not initialized"))
    }
}

struct SaveMapping<'a> {
    access: CoreAccess<'a>,
    linker: *mut OwnedLinkerSave,
    mode: TagTraversal,
    name_context: u32,
}

impl SaveMapping<'_> {
    fn traverse(&self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() { return Ok(()); }
        match self.mode {
            TagTraversal::Exports => unsafe { tag_export(self.access, self.linker, object, self) },
            TagTraversal::Imports => unsafe { tag_import(self.access, self.linker, object, self) },
            TagTraversal::None => Err(save_error("Reference visited outside a package tagging pass")),
        }
    }
}

impl ArchiveMapping for SaveMapping<'_> {
    fn load_object(&self, _index: i32) -> RuntimeResult<*mut UObject> {
        Err(save_error("Save mapping cannot load objects"))
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { return Ok(0); }
        let flags = unsafe { (*object).object_flags };
        let index = unsafe { (*self.linker).tail()?.object_indices.get(&(object as usize)).copied() };
        match index {
            Some(index) => Ok(index),
            // Native ULinkerSave indexes a zero-initialized ObjectIndices slot.
            // Transient non-public objects are omitted from the import map, so
            // serializing their reference reads the untouched null index.
            None if flags & RF_TRANSIENT != 0 && flags & RF_PUBLIC == 0 => Ok(0),
            None => Err(save_error("Object is not mapped for package save").with_utf32_context(
                unsafe { crate::core_lifetime::object_full_name(self.access, object)? },
            )),
        }
    }
    fn load_name(&self, _local_index: i32) -> RuntimeResult<i32> {
        Err(save_error("Save mapping cannot load names"))
    }
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32> {
        unsafe { (*self.linker).tail()?.name_indices.get(&global_index).copied() }
            .ok_or_else(|| save_error("Name is not mapped for package save"))
    }
    fn visit_object(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        if object_slot.is_null() { return Err(save_error("Null package object reference")); }
        self.traverse(unsafe { ptr::read_unaligned(object_slot) })
    }
    fn visit_name(&self, name_index: i32) -> RuntimeResult<()> {
        if self.mode == TagTraversal::Imports { tag_name(self.access, self.linker, name_index, self.name_context)?; }
        Ok(())
    }
}

unsafe fn tag_export(access: CoreAccess<'_>, linker: *mut OwnedLinkerSave, object: *mut UObject, map: &SaveMapping<'_>) -> RuntimeResult<()> {
    if object.is_null() { return Ok(()); }
    let context = DirectSaveOps::context_for(access, linker)?;
    if unsafe { (*object).object_flags } & (RF_TRANSIENT | RF_TAG_EXP) != 0 { return Ok(()); }
    if !object_is_in(access, object, context.outer)? { return Ok(()); }
    unsafe { (*object).object_flags |= RF_TAG_EXP; }
    apply_load_context(access, object)?;
    let archive = ObjectArchive::tag_exports(save_archive_flags(), map);
    let class = unsafe { (*object).class.cast::<UObject>() };
    let outer = unsafe { (*object).outer.cast::<UObject>() };
    map.traverse(class)?;
    map.traverse(outer)?;
    unsafe { access.serialize_object(object, &archive) }
}

unsafe fn tag_import(access: CoreAccess<'_>, linker: *mut OwnedLinkerSave, object: *mut UObject, map: &SaveMapping<'_>) -> RuntimeResult<()> {
    if object.is_null() { return Ok(()); }
    let flags = unsafe { (*object).object_flags };
    if flags & RF_TRANSIENT != 0 && flags & RF_PUBLIC == 0 { return Ok(()); }
    let owner = unsafe { &*linker };
    owner.with_tail(|tail| {
        let count = tail.object_indices.get(&(object as usize)).copied().unwrap_or(0);
        tail.object_indices.insert(object as usize, count.wrapping_add(1));
        Ok(())
    })?;
    if flags & RF_TAG_EXP != 0 || flags & RF_TAG_IMP != 0 { return Ok(()); }
    unsafe { (*object).object_flags |= RF_TAG_IMP; }
    apply_load_context(access, object)?;
    let outer = unsafe { (*object).outer.cast::<UObject>() };
    if !outer.is_null() { map.traverse(outer)?; }
    Ok(())
}

fn tag_name(access: CoreAccess<'_>, linker: *mut OwnedLinkerSave, name: i32, context: u32) -> RuntimeResult<()> {
    let owner = unsafe { &*linker };
    owner.with_tail(|tail| {
        let slot = access.name_at(name);
        if slot.is_null() { return Err(save_error("Invalid name index in package import")); }
        unsafe { (*slot).flags |= RF_TAG_EXP | context; }
        let uses = tail.name_usage.get(&name).copied().unwrap_or(0);
        tail.name_usage.insert(name, uses.wrapping_add(1));
        Ok(())
    })
}

fn save_archive_flags() -> ArchiveFlags {
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(constants.version as u16, constants.licensee as u16, true)
}

fn apply_load_context(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let flags = unsafe { (*object).object_flags };
    let mut load = 0;
    for (exclude, include) in [
        (0x0010_0000, constants.load_client),
        (0x0020_0000, constants.load_server),
        (0x0040_0000, constants.load_edit),
    ] {
        if flags & exclude == 0 { load |= include; }
    }
    unsafe { (*object).object_flags |= load; }
    Ok(())
}

fn object_is_in(access: CoreAccess<'_>, object: *mut UObject, outer: *mut UObject) -> RuntimeResult<bool> {
    if outer.is_null() { return Ok(true); }
    let limit = access.object_count().max(0) as usize;
    let mut current = if object.is_null() { ptr::null_mut() } else { unsafe { (*object).outer.cast::<UObject>() } };
    for _ in 0..limit {
        if current.is_null() { return Ok(false); }
        if current == outer { return Ok(true); }
        current = unsafe { (*current).outer.cast() };
    }
    Err(save_error("Invalid object outer chain"))
}

struct SavePaths {
    temporary: PathBuf,
    final_path: PathBuf,
}

impl Drop for SaveTail {
    fn drop(&mut self) {
        if let Some(mut archive) = self.archive.take() {
            let _ = archive.close();
        }
        if !self.committed {
            let _ = fs::remove_file(&self.temporary_path);
        }
    }
}

struct SaveTail {
    archive: Option<RuntimeArchive>,
    temporary_path: PathBuf,
    final_path: PathBuf,
    summary: PackageSummary,
    names: Vec<i32>,
    imports: Vec<SaveImport>,
    exports: Vec<SaveExport>,
    object_indices: HashMap<usize, i32>,
    name_usage: HashMap<i32, i32>,
    name_indices: HashMap<i32, i32>,
    conform_names: Vec<i32>,
    original_names: Option<Vec<i32>>,
    conform_name_texts: Vec<Vec<u32>>,
    conform_imports: Vec<PackageImport>,
    conform_records: Vec<PackageExport>,
    original_exports: Vec<SaveExport>,
    original_export_used: Vec<bool>,
    conform_name_map: HashMap<Vec<u32>, usize>,
    has_conform: bool,
    committed: bool,
}

struct DirectSaveOps<'a> {
    access: CoreAccess<'a>,
    linker: *mut UObject,
    outer: *mut UObject,
    base: *mut UObject,
    top_level_flags: u32,
    has_conform: bool,
}

impl<'a> DirectSaveOps<'a> {
    fn owner(&self) -> &OwnedLinkerSave {
        // SAFETY: the canonical LinkerSave allocation outlives this invocation.
        unsafe { &*self.linker.cast::<OwnedLinkerSave>() }
    }

    fn context_for(_access: CoreAccess<'_>, linker: *mut OwnedLinkerSave) -> RuntimeResult<SaveContext> {
        let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        let tail = unsafe { (*linker).tail()? };
        Ok(SaveContext {
            outer: unsafe { (*linker.cast::<UObject>()).outer.cast() },
            base: ptr::null_mut(),
            top_level_flags: 0,
            has_conform: tail.has_conform,
            check: constants.checks != 0,
            slow: constants.slow_checks != 0,
            tag_export: RF_TAG_EXP,
            tag_import: RF_TAG_IMP,
            export_name_flags: RF_TAG_EXP | RF_TAG_IMP | constants.load_mask,
            public_flag: constants.public_flag,
        })
    }

    fn with_tail<R>(&self, f: impl FnOnce(&mut SaveTail) -> RuntimeResult<R>) -> RuntimeResult<R> {
        self.owner().with_tail(f)
    }


    fn local_name(&self, name: i32) -> RuntimeResult<i32> {
        self.owner().tail()?.name_indices.get(&name).copied()
            .ok_or_else(|| save_error("Name missing from package map"))
    }

    fn package_index(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() || object == self.outer { return Ok(0); }
        self.owner().tail()?.object_indices.get(&(object as usize)).copied()
            .ok_or_else(|| save_error("Object missing from package map"))
    }

    fn class_index(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { return Ok(0); }
        self.package_index(object)
    }

    fn object_name_index(object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { return Err(save_error("Null object in package table")); }
        Ok(i32::from_ne_bytes(unsafe { (*object).name }))
    }

    fn close_and_preserve(&mut self) -> RuntimeResult<SavePaths> {
        self.with_tail(|tail| {
            let mut archive = tail.archive.take().ok_or_else(|| save_error("Package file already closed"))?;
            let flush = archive_status(archive.flush(), "Package flush");
            let close = archive_status(archive.close(), "Package close");
            flush?;
            close?;
            tail.committed = true;
            Ok(SavePaths { temporary: std::mem::take(&mut tail.temporary_path), final_path: std::mem::take(&mut tail.final_path) })
        })
    }

    fn write_summary(&self) -> RuntimeResult<()> {
        let mut writer = PackageArchiveWriter::new();
        crate::package_archive::write_package_summary(&mut writer, &self.owner().tail()?.summary)?;
        self.with_tail(|tail| write_archive_bytes(tail, writer.bytes()))
    }

    fn lookup_conform_path(
        access: CoreAccess<'_>,
        tail: &SaveTail,
        root: &[u32],
        export_index: usize,
    ) -> RuntimeResult<Vec<u32>> {
        let record = *tail.conform_records.get(export_index)
            .ok_or_else(|| save_error("Conform export index is out of range"))?;
        let mut path = Self::conform_parent_path(tail, root, record.package_index)?;
        if !path.is_empty() { path.push(u32::from('.')); }
        path.extend(Self::conform_name(tail, record.object_name)?);
        let class_name = Self::conform_class_name(access, tail, record.class_index)?;
        let mut full_name = class_name;
        full_name.push(u32::from(' '));
        full_name.extend(path);
        Ok(full_name)
    }

    fn conform_class_name(
        access: CoreAccess<'_>,
        tail: &SaveTail,
        index: i32,
    ) -> RuntimeResult<Vec<u32>> {
        if index == 0 {
            let name = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.class_name;
            return access.name_text(name);
        }
        let name_index = if index > 0 {
            tail.conform_records.get((index - 1) as usize)
                .ok_or_else(|| save_error("Conform class export index is out of range"))?.object_name
        } else {
            let import_index = usize::try_from(index.unsigned_abs() - 1)
                .map_err(|_| save_error("Invalid conform class import index"))?;
            tail.conform_imports.get(import_index)
                .ok_or_else(|| save_error("Conform class import index is out of range"))?.object_name
        };
        Self::conform_name(tail, name_index)
    }
    fn conform_name(tail: &SaveTail, index: i32) -> RuntimeResult<Vec<u32>> {
        tail.conform_name_texts.get(usize::try_from(index).map_err(|_| save_error("Invalid conform name index"))?)
            .cloned().ok_or_else(|| save_error("Conform name index is out of range"))
    }

    fn conform_parent_path(tail: &SaveTail, root: &[u32], index: i32) -> RuntimeResult<Vec<u32>> {
        let mut current = index;
        let mut visited = HashSet::default();
        let mut names = Vec::new();
        let limit = tail.conform_records.len().saturating_add(tail.conform_imports.len());
        for _ in 0..limit {
            if current == 0 { break; }
            let (parent, name) = if current > 0 {
                let row_index = usize::try_from(current - 1).map_err(|_| save_error("Invalid conform export index"))?;
                if !visited.insert((true, row_index)) { return Err(save_error("Cyclic conform export path")); }
                let row = tail.conform_records.get(row_index)
                    .ok_or_else(|| save_error("Conform export index is out of range"))?;
                (row.package_index, row.object_name)
            } else {
                let row_index = usize::try_from(current.unsigned_abs() - 1)
                    .map_err(|_| save_error("Invalid conform import index"))?;
                if !visited.insert((false, row_index)) { return Err(save_error("Cyclic conform import path")); }
                let row = tail.conform_imports.get(row_index)
                    .ok_or_else(|| save_error("Conform import index is out of range"))?;
                (row.package_index, row.object_name)
            };
            names.push(Self::conform_name(tail, name)?);
            current = parent;
        }
        if current != 0 { return Err(save_error("Invalid conform package path")); }
        let mut path = root.to_vec();
        for name in names.iter().rev() {
            if !path.is_empty() { path.push(u32::from('.')); }
            path.extend(name.iter().copied());
        }
        Ok(path)
    }
}

/// Complete Rust-owned linker-save allocation; its first field preserves the native identity prefix.
#[repr(C)]
pub(crate) struct OwnedLinkerSave {
    pub(crate) raw: LinkerSaveRecord,
    tail: RefCell<Option<SaveTail>>,
}

pub(crate) fn allocation_layout() -> Layout { Layout::new::<OwnedLinkerSave>() }

/// Initializes the Rust tail after the canonical LinkerSave object is allocated.
///
/// # Safety
/// `object` is a live canonical LinkerSave allocation with an empty Rust tail.
pub(crate) unsafe fn initialize(object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null LinkerSave allocation")); }
    let owner = object.cast::<OwnedLinkerSave>();
    unsafe {
        ptr::addr_of_mut!((*owner).tail).write(RefCell::new(None));
    }
    Ok(())
}

/// Releases an initialized LinkerSave tail before the canonical UObject teardown.
///
/// # Safety
/// `object` is a live canonical LinkerSave whose tail was initialized exactly once.
pub(crate) unsafe fn destroy(object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null LinkerSave allocation")); }
    let owner = object.cast::<OwnedLinkerSave>();
    let tail = unsafe { ptr::addr_of!((*owner).tail).read() };
    let previous = tail.replace(None);
    drop(previous);
    Ok(())
}

impl<'a> PackageSaveOps for DirectSaveOps<'a> {
    type Error = RuntimeError;

    fn context(&self) -> SaveContext {
        let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        SaveContext {
            outer: self.outer,
            base: self.base,
            top_level_flags: self.top_level_flags,
            has_conform: self.has_conform,
            check: constants.checks != 0,
            slow: constants.slow_checks != 0,
            tag_export: RF_TAG_EXP,
            tag_import: RF_TAG_IMP,
            export_name_flags: RF_TAG_EXP | RF_TAG_IMP | constants.load_mask,
            public_flag: constants.public_flag,
        }
    }

    fn clear_save_tags(&mut self) -> RuntimeResult<()> {
        let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        let mask = RF_TAG_EXP | RF_TAG_IMP | constants.load_client | constants.load_server | constants.load_edit;
        for index in 0..self.object_count()? {
            let object = self.object_at(index)?;
            if !object.is_null() { unsafe { (*object).object_flags &= !mask; } }
        }
        for index in 0..self.global_name_count()? {
            let entry = self.access.name_at(index as i32);
            if !entry.is_null() { unsafe { (*entry).flags &= !mask; } }
        }
        Ok(())
    }

    fn tag_export(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        let map = SaveMapping { access: self.access, linker: self.linker.cast(), mode: TagTraversal::Exports, name_context: 0 };
        unsafe { tag_export(self.access, self.linker.cast(), object, &map) }
    }

    fn tag_import(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        let name_context = unsafe { (*object).object_flags } & (constants.load_client | constants.load_server | constants.load_edit);
        let map = SaveMapping { access: self.access, linker: self.linker.cast(), mode: TagTraversal::Imports, name_context };
        let archive = ObjectArchive::tag_imports(save_archive_flags(), &map);
        // SAFETY: the shared phase selects a live export and the mapping only tags its references.
        unsafe { self.access.serialize_object(object, &archive)?; }
        map.traverse(unsafe { (*object).class.cast() })
    }

    fn object_count(&mut self) -> RuntimeResult<usize> { Ok(self.access.object_count().max(0) as usize) }
    fn object_at(&mut self, index: usize) -> RuntimeResult<*mut UObject> {
        let index = i32::try_from(index).map_err(|_| save_error("Object index overflow"))?;
        Ok(self.access.object_at(index))
    }
    fn name_flags(&mut self, name: i32) -> RuntimeResult<u32> {
        let entry = self.access.name_at(name);
        if entry.is_null() { return Err(save_error("Invalid package name index")); }
        Ok(unsafe { (*entry).flags })
    }
    fn object_flags(&mut self, object: *mut UObject) -> RuntimeResult<u32> {
        if object.is_null() { return Err(save_error("Null object in save traversal")); }
        Ok(unsafe { (*object).object_flags })
    }
    fn object_outer(&mut self, object: *mut UObject) -> RuntimeResult<*mut UObject> {
        if object.is_null() { return Err(save_error("Null object in save traversal")); }
        Ok(unsafe { (*object).outer.cast() })
    }
    fn object_class(&mut self, object: *mut UObject) -> RuntimeResult<*mut UObject> {
        if object.is_null() { return Err(save_error("Null object in save traversal")); }
        Ok(unsafe { (*object).class.cast() })
    }
    fn object_name(&mut self, object: *mut UObject) -> RuntimeResult<i32> { Self::object_name_index(object) }
    fn set_object_name_flags(&mut self, name: i32, flags: u32) -> RuntimeResult<()> {
        let entry = self.access.name_at(name);
        if entry.is_null() { return Err(save_error("Invalid package name index")); }
        unsafe { (*entry).flags |= flags; }
        Ok(())
    }
    fn is_in_package(&mut self, object: *mut UObject, outer: *mut UObject) -> RuntimeResult<bool> {
        object_is_in(self.access, object, outer)
    }
    fn report_private_import(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        let text = object_path_key(self.access, object)?;
        let tail = self.owner().tail()?;
        let filename = &tail.final_path;
        let mut message: Vec<u32> = format!("Can't save {}: Graph is linked to external private object ", filename.display()).chars().map(u32::from).collect();
        message.extend(text);
        Err(RuntimeError::from_utf32(message).with_source_path(filename))
    }
    fn report_saving_object(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        let text = object_path_key(self.access, object)?;
        let mut message: Vec<u32> = "Saving: ".chars().map(u32::from).collect();
        message.extend(text);
        let category = self.access.intern_str("Log", crate::name_owner::FindName::Add)?;
        self.access.emit_log(category, &message)
    }

    fn global_name_count(&mut self) -> RuntimeResult<usize> { Ok(self.access.name_count().max(0) as usize) }
    fn global_name_flags(&mut self, index: usize) -> RuntimeResult<u32> {
        let entry = self.access.name_at(index as i32);
        Ok(if entry.is_null() { 0 } else { unsafe { (*entry).flags } })
    }
    fn name_count(&mut self) -> RuntimeResult<usize> { Ok(self.owner().tail()?.names.len()) }
    fn name_at(&mut self, index: usize) -> RuntimeResult<i32> {
        self.owner().tail()?.names.get(index).copied()
            .ok_or_else(|| save_error("Package name index out of range"))
    }
    fn set_name_count(&mut self, count: usize) -> RuntimeResult<()> {
        let count = u32::try_from(count).map_err(|_| save_error("Package name table too large"))?;
        self.with_tail(|tail| { tail.summary.name_count = count; Ok(()) })
    }
    fn set_import_count(&mut self, count: usize) -> RuntimeResult<()> {
        let count = u32::try_from(count).map_err(|_| save_error("Package import table too large"))?;
        self.with_tail(|tail| { tail.summary.import_count = count; Ok(()) })
    }
    fn save_name(&mut self, name: i32) -> RuntimeResult<()> {
        let text = self.access.name_text(name)?;
        let flags = self.name_flags(name)?;
        let local = self.owner().tail()?.names.iter().position(|candidate| *candidate == name)
            .ok_or_else(|| save_error("Name missing from package name table"))?;
        let local = i32::try_from(local).map_err(|_| save_error("Package name table too large"))?;
        self.with_tail(|tail| {
            let version = tail.summary.version;
            append_wire(tail, |writer| writer.write_name(&PackageName {
                text, flags, offset: 0, end_offset: 0,
            }, version))
        })?;
        self.with_tail(|tail| { tail.name_indices.insert(name, local); Ok(()) })
    }
    fn set_name_usage(&mut self, name: i32, usage: i32) -> RuntimeResult<()> {
        self.with_tail(|tail| { tail.name_usage.insert(name, usage); Ok(()) })
    }
    fn name_usage(&mut self, name: i32) -> RuntimeResult<i32> {
        Ok(self.owner().tail()?.name_usage.get(&name).copied().unwrap_or(0))
    }
    fn find_name(&mut self, name: i32) -> RuntimeResult<Option<usize>> {
        Ok(self.owner().tail()?.names.iter().position(|candidate| *candidate == name))
    }
    fn append_name_slot(&mut self) -> RuntimeResult<usize> {
        self.with_tail(|tail| {
            let index = tail.names.len();
            tail.names.push(0);
            Ok(index)
        })
    }
    fn swap_names(&mut self, left: usize, right: usize) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            if left >= tail.names.len() || right >= tail.names.len() { return Err(save_error("Name swap index out of range")); }
            tail.names.swap(left, right);
            Ok(())
        })
    }
    fn set_name_at(&mut self, index: usize, name: i32) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            let slot = tail.names.get_mut(index).ok_or_else(|| save_error("Name index out of range"))?;
            *slot = name;
            Ok(())
        })
    }
    fn sort_names_from(&mut self, index: usize) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            if index > tail.names.len() { return Err(save_error("Name sort index out of range")); }
            let usages = &tail.name_usage;
            tail.names[index..].sort_unstable_by(|left, right| {
                package_save::compare_qsort_mapped_values(
                    usages.get(left).copied().unwrap_or(0),
                    usages.get(right).copied().unwrap_or(0),
                ).cmp(&0)
            });
            Ok(())
        })
    }
    fn conform_name_count(&mut self) -> RuntimeResult<usize> {
        self.with_tail(|tail| {
            if tail.original_names.is_none() { tail.original_names = Some(tail.names.clone()); }
            Ok(tail.conform_names.len())
        })
    }
    fn conform_name_at(&mut self, index: usize) -> RuntimeResult<i32> {
        self.owner().tail()?.conform_names.get(index).copied()
            .ok_or_else(|| save_error("Conform name index out of range"))
    }
    fn original_name_count(&mut self) -> RuntimeResult<usize> {
        self.owner().tail()?.original_names.as_ref().map(Vec::len)
            .ok_or_else(|| save_error("Original name snapshot is missing"))
    }
    fn original_name_at(&mut self, index: usize) -> RuntimeResult<i32> {
        self.owner().tail()?.original_names.as_ref()
            .and_then(|names| names.get(index)).copied()
            .ok_or_else(|| save_error("Original name index out of range"))
    }
    fn add_name(&mut self, name: i32) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            if !tail.names.contains(&name) { tail.names.push(name); }
            Ok(())
        })
    }
    fn add_import(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() { return Err(save_error("Null import object")); }
        self.with_tail(|tail| {
            if !tail.imports.iter().any(|entry| entry.object == object) {
                tail.imports.push(SaveImport {
                    object,
                    record: PackageImport {
                        class_package: 0, class_name: 0, package_index: 0,
                        object_name: 0, offset: 0, end_offset: 0,
                    },
                });
            }
            Ok(())
        })
    }
    fn add_export(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() { return Err(save_error("Null export object")); }
        self.with_tail(|tail| {
            if !tail.exports.iter().any(|entry| entry.object == object) {
                tail.exports.push(SaveExport {
                    object,
                    record: PackageExport {
                        class_index: 0, super_index: 0, package_index: 0, object_name: 0,
                        object_flags: 0, serial_size: 0, serial_offset: 0, offset: 0, end_offset: 0,
                    },
                });
            }
            Ok(())
        })
    }
    fn sort_imports(&mut self) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            let usage = &tail.object_indices;
            tail.imports.sort_unstable_by(|left, right| {
                package_save::compare_qsort_mapped_values(
                    usage.get(&(left.object as usize)).copied().unwrap_or(0),
                    usage.get(&(right.object as usize)).copied().unwrap_or(0),
                ).cmp(&0)
            });
            Ok(())
        })
    }
    fn sort_exports(&mut self, conform_start: usize) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            if conform_start > tail.exports.len() { return Err(save_error("Conform export index out of range")); }
            let usage = &tail.object_indices;
            tail.exports[conform_start..].sort_unstable_by(|left, right| {
                package_save::compare_qsort_mapped_values(
                    usage.get(&(left.object as usize)).copied().unwrap_or(0),
                    usage.get(&(right.object as usize)).copied().unwrap_or(0),
                ).cmp(&0)
            });
            Ok(())
        })
    }
    fn export_count(&mut self) -> RuntimeResult<usize> { Ok(self.owner().tail()?.exports.len()) }
    fn export_at(&mut self, index: usize) -> RuntimeResult<*mut UObject> {
        self.owner().tail()?.exports.get(index).map(|entry| entry.object).ok_or_else(|| save_error("Export index out of range"))
    }
    fn import_count(&mut self) -> RuntimeResult<usize> { Ok(self.owner().tail()?.imports.len()) }
    fn import_at(&mut self, index: usize) -> RuntimeResult<*mut UObject> {
        self.owner().tail()?.imports.get(index).map(|entry| entry.object).ok_or_else(|| save_error("Import index out of range"))
    }
    fn set_object_usage(&mut self, object: *mut UObject, usage: i32) -> RuntimeResult<()> {
        if object.is_null() { return Err(save_error("Null object map identity")); }
        self.with_tail(|tail| { tail.object_indices.insert(object as usize, usage); Ok(()) })
    }
    fn object_usage(&mut self, object: *mut UObject) -> RuntimeResult<i32> {
        self.owner().tail()?.object_indices.get(&(object as usize)).copied().ok_or_else(|| save_error("Object missing from package map"))
    }

    fn original_export_count(&mut self) -> RuntimeResult<usize> {
        self.with_tail(|tail| {
            tail.original_exports = std::mem::take(&mut tail.exports);
            tail.original_export_used = vec![false; tail.original_exports.len()];
            Ok(tail.original_exports.len())
        })
    }
    fn map_original_export_name(&mut self, index: usize) -> RuntimeResult<()> {
        let object = self.owner().tail()?.original_exports.get(index)
            .map(|entry| entry.object).ok_or_else(|| save_error("Current export index out of range"))?;
        let key = object_path_key(self.access, object)?;
        self.with_tail(|tail| { tail.conform_name_map.insert(key, index); Ok(()) })
    }
    fn conform_export_count(&mut self) -> RuntimeResult<usize> {
        Ok(self.owner().tail()?.conform_records.len())
    }
    fn find_conform_export(&mut self, index: usize) -> RuntimeResult<Option<usize>> {
        let root = object_path_only(self.access, self.outer)?;
        let key = {
            let tail = self.owner().tail()?;
            Self::lookup_conform_path(self.access, &tail, &root, index)?
        };
        Ok(self.owner().tail()?.conform_name_map.get(&key).copied())
    }
    fn conform_export_used(&mut self, index: usize) -> RuntimeResult<bool> {
        self.owner().tail()?.original_export_used.get(index).copied()
            .ok_or_else(|| save_error("Current export index out of range"))
    }
    fn set_conform_export_used(&mut self, index: usize) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            let used = tail.original_export_used.get_mut(index)
                .ok_or_else(|| save_error("Current export index out of range"))?;
            *used = true;
            Ok(())
        })
    }
    fn copy_original_export(&mut self, index: usize) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            let entry = tail.original_exports.get(index).cloned()
                .ok_or_else(|| save_error("Current export index out of range"))?;
            tail.exports.push(entry);
            Ok(())
        })
    }
    fn add_empty_export(&mut self) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            tail.exports.push(SaveExport {
                object: ptr::null_mut(),
                record: PackageExport {
                    class_index: 0, super_index: 0, package_index: 0, object_name: 0,
                    object_flags: 0, serial_size: 0, serial_offset: 0, offset: 0, end_offset: 0,
                },
            });
            Ok(())
        })
    }
    fn check_last_export_matches_original(&mut self, index: usize) -> RuntimeResult<bool> {
        let object = self.owner().tail()?.exports.last()
            .map(|entry| entry.object).unwrap_or(ptr::null_mut());
        if object.is_null() { return Ok(false); }
        let original = self.owner().tail()?.original_exports.get(index)
            .map(|entry| entry.object).ok_or_else(|| save_error("Current export index out of range"))?;
        Ok(object == original)
    }
    fn original_export_present(&mut self, index: usize) -> RuntimeResult<bool> {
        let object = self.owner().tail()?.original_exports.get(index)
            .map(|entry| entry.object).ok_or_else(|| save_error("Current export index out of range"))?;
        Ok(self.owner().tail()?.exports.iter().any(|entry| entry.object == object))
    }

    fn initialize_summary(&mut self) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            tail.summary.name_count = 0;
            tail.summary.name_offset = 0;
            tail.summary.import_count = 0;
            tail.summary.import_offset = 0;
            tail.summary.export_count = 0;
            tail.summary.export_offset = 0;
            if tail.summary.generations.is_empty() {
                tail.summary.generations.push(GenerationInfo { export_count: 0, name_count: 0 });
            }
            Ok(())
        })?;
        self.write_summary()
    }
    fn serialize_summary(&mut self) -> RuntimeResult<()> { self.write_summary() }
    fn tell(&mut self) -> RuntimeResult<u64> {
        self.with_tail(|tail| {
            let archive = tail.archive.as_mut().ok_or_else(|| save_error("Package file is closed"))?;
            let position = archive.tell().map_err(|status| save_error(&format!("Package tell failed ({status})")))?;
            u64::try_from(position).map_err(|_| save_error("Invalid package file position"))
        })
    }
    fn seek(&mut self, offset: u64) -> RuntimeResult<()> {
        let offset = i32::try_from(offset).map_err(|_| save_error("Package file offset overflow"))?;
        self.with_tail(|tail| {
            let archive = tail.archive.as_mut().ok_or_else(|| save_error("Package file is closed"))?;
            archive_status(archive.seek(offset), "Package seek")
        })
    }
    fn set_name_offset(&mut self, offset: u64) -> RuntimeResult<()> {
        let value = u32::try_from(offset).map_err(|_| save_error("Package name offset overflow"))?;
        self.with_tail(|tail| { tail.summary.name_offset = value; Ok(()) })
    }
    fn set_import_offset(&mut self, offset: u64) -> RuntimeResult<()> {
        let value = u32::try_from(offset).map_err(|_| save_error("Package import offset overflow"))?;
        self.with_tail(|tail| { tail.summary.import_offset = value; Ok(()) })
    }
    fn set_export_offset(&mut self, offset: u64) -> RuntimeResult<()> {
        let value = u32::try_from(offset).map_err(|_| save_error("Package export offset overflow"))?;
        self.with_tail(|tail| { tail.summary.export_offset = value; Ok(()) })
    }
    fn closing_status(&mut self) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", crate::name_owner::FindName::Add)?;
        let message: Vec<u32> = "Closing".chars().map(u32::from).collect();
        self.access.emit_log(category, &message)
    }
    fn update_generation_counts(&mut self) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            tail.summary.name_count = u32::try_from(tail.names.len()).map_err(|_| save_error("Package name table too large"))?;
            tail.summary.import_count = u32::try_from(tail.imports.len()).map_err(|_| save_error("Package import table too large"))?;
            tail.summary.export_count = u32::try_from(tail.exports.len()).map_err(|_| save_error("Package export table too large"))?;
            let generation = tail.summary.generations.last_mut()
                .ok_or_else(|| save_error("Package summary has no generation"))?;
            generation.export_count = i32::try_from(tail.exports.len()).map_err(|_| save_error("Package export table too large"))?;
            generation.name_count = i32::try_from(tail.names.len()).map_err(|_| save_error("Package name table too large"))?;
            Ok(())
        })
    }
    fn write_import(&mut self, index: usize) -> RuntimeResult<()> {
        let entry = self.owner().tail()?.imports.get(index).cloned().ok_or_else(|| save_error("Import index out of range"))?;
        let class = unsafe { (*entry.object).class.cast::<UObject>() };
        let class_outer = if class.is_null() { ptr::null_mut() } else { unsafe { (*class).outer.cast::<UObject>() } };
        let record = PackageImport {
            class_package: if class_outer.is_null() { 0 } else { self.local_name(Self::object_name_index(class_outer)?)? },
            class_name: if class.is_null() { 0 } else { self.local_name(Self::object_name_index(class)?)? },
            package_index: self.package_index(unsafe { (*entry.object).outer.cast() })?,
            object_name: self.local_name(Self::object_name_index(entry.object)?)?,
            offset: 0,
            end_offset: 0,
        };
        let mut writer = PackageArchiveWriter::new();
        writer.write_import(&record)?;
        self.with_tail(|tail| {
            let offset = u32::try_from(tail.archive.as_mut().ok_or_else(|| save_error("Package file is closed"))?
                .tell().map_err(|status| save_error(&format!("Package tell failed ({status})")))?)
                .map_err(|_| save_error("Package table offset overflow"))?;
            let mut record = record;
            record.offset = offset;
            record.end_offset = offset;
            tail.imports[index].record = record;
            write_archive_bytes(tail, writer.bytes())
        })
    }
    fn write_export_map(&mut self, index: usize) -> RuntimeResult<()> {
        let record = self.owner().tail()?.exports.get(index).map(|entry| entry.record).ok_or_else(|| save_error("Export index out of range"))?;
        self.with_tail(|tail| {
            append_wire(tail, |writer| writer.write_export(&record))
        })
    }
    fn prepare_export(&mut self, index: usize, object: *mut UObject) -> RuntimeResult<()> {
        let class = unsafe { (*object).class.cast::<UObject>() };
        let super_field = if unsafe { crate::object_model::is_a(object.cast(), self.access.native_class("Struct")?.cast()) } {
            // SAFETY: reflected structs share the canonical UField prefix.
            unsafe { (*object.cast::<hp2_abi::engine::generated::UField>()).super_field.cast::<UObject>() }
        } else { ptr::null_mut() };
        let package_index = self.package_index(unsafe { (*object).outer.cast() })?;
        let class_index = if unsafe { crate::object_model::is_a(object.cast(), self.access.native_class("Class")?.cast()) } {
            0
        } else {
            self.class_index(class)?
        };
        let super_index = self.class_index(super_field)?;
        let object_name = self.local_name(Self::object_name_index(object)?)?;
        let flags = unsafe { (*object).object_flags }
            & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.load_mask;
        let offset = i32::try_from(self.tell()?).map_err(|_| save_error("Package export offset overflow"))?;
        self.with_tail(|tail| {
            let entry = tail.exports.get_mut(index).ok_or_else(|| save_error("Export index out of range"))?;
            entry.record = PackageExport {
                class_index, super_index, package_index, object_name, object_flags: flags,
                serial_size: 0, serial_offset: offset, offset: 0, end_offset: 0,
            };
            Ok(())
        })
    }
    fn serialize_object(&mut self, object: *mut UObject) -> RuntimeResult<()> {
        let mapping = SaveMapping { access: self.access, linker: self.linker.cast(), mode: TagTraversal::None, name_context: 0 };
        let origin = usize::try_from(self.tell()?).map_err(|_| save_error("Invalid export file origin"))?;
        let archive = ObjectArchive::saving_at(save_archive_flags(), &mapping, origin);
        // SAFETY: the object is canonical, live, and serialization is synchronous.
        unsafe { self.access.serialize_object(object, &archive)?; }
        let bytes = archive.take_bytes()?;
        self.with_tail(|tail| write_archive_bytes(tail, &bytes))
    }
    fn serial_offset(&mut self, index: usize) -> RuntimeResult<u64> {
        let offset = self.owner().tail()?.exports.get(index).ok_or_else(|| save_error("Export index out of range"))?.record.serial_offset;
        u64::try_from(offset).map_err(|_| save_error("Invalid export serial offset"))
    }
    fn set_export_size(&mut self, index: usize, size: i32) -> RuntimeResult<()> {
        if size < 0 { return Err(save_error("Negative export serial size")); }
        self.with_tail(|tail| {
            let entry = tail.exports.get_mut(index).ok_or_else(|| save_error("Export index out of range"))?;
            entry.record.serial_size = size;
            if size == 0 { entry.record.serial_offset = 0; }
            Ok(())
        })
    }
    fn finish_summary(&mut self) -> RuntimeResult<()> {
        self.with_tail(|tail| {
            let archive = tail.archive.as_mut().ok_or_else(|| save_error("Package file is closed"))?;
            archive_status(archive.flush(), "Package flush")
        })
    }
    fn check(&mut self, condition: bool) -> RuntimeResult<()> {
        if condition { Ok(()) } else { Err(save_error("Package save check failed")) }
    }
}
fn fill_guid(guid: &mut [u32; 4]) -> RuntimeResult<()> {
    use std::io::Read;
    let mut source = fs::File::open("/dev/urandom")
        .map_err(|error| RuntimeError::new("Failed to generate package GUID").with_context(&error.to_string()))?;
    let mut bytes = [0; 16];
    source.read_exact(&mut bytes)
        .map_err(|error| RuntimeError::new("Failed to generate package GUID").with_context(&error.to_string()))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    for (word, bytes) in guid.iter_mut().zip(bytes.chunks_exact(4)) {
        *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    Ok(())
}

fn save_error(message: &str) -> RuntimeError { RuntimeError::new(message) }

fn archive_status(result: Result<(), i32>, operation: &str) -> RuntimeResult<()> {
    result.map_err(|status| RuntimeError::new(&format!("{operation} failed ({status})")))
}

fn write_bytes(archive: &mut RuntimeArchive, bytes: &[u8]) -> RuntimeResult<()> {
    let length = i32::try_from(bytes.len()).map_err(|_| save_error("Package write exceeds archive limit"))?;
    if length == 0 { return Ok(()); }
    // SAFETY: the source slice remains live for the synchronous archive write.
    let status = unsafe { archive.serialize(bytes.as_ptr().cast_mut().cast::<c_void>(), length) };
    archive_status(if status == 0 { Ok(()) } else { Err(status) }, "Package write")
}

fn write_archive_bytes(tail: &mut SaveTail, bytes: &[u8]) -> RuntimeResult<()> {
    let archive = tail.archive.as_mut().ok_or_else(|| save_error("LinkerSave file is closed"))?;
    write_bytes(archive, bytes)
}

fn append_wire(tail: &mut SaveTail, write: impl FnOnce(&mut PackageArchiveWriter) -> RuntimeResult<()>) -> RuntimeResult<()> {
    let mut writer = PackageArchiveWriter::new();
    write(&mut writer)?;
    write_archive_bytes(tail, writer.bytes())
}


fn object_path_only(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    if object.is_null() { return Err(save_error("Null conform object")); }
    let limit = access.object_count().max(0) as usize;
    let mut chain = Vec::new();
    let mut current = object;
    while !current.is_null() {
        if chain.len() >= limit { return Err(save_error("Invalid object outer chain")); }
        chain.push(access.name_text(unsafe { i32::from_ne_bytes((*current).name) })?);
        current = unsafe { (*current).outer.cast() };
    }
    let mut key = Vec::new();
    for (index, part) in chain.iter().rev().enumerate() {
        if index != 0 { key.push(u32::from('.')); }
        key.extend(part.iter().copied());
    }
    Ok(key)
}

fn object_path_key(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    if object.is_null() { return Err(save_error("Null conform object")); }
    let class = unsafe { (*object).class.cast::<UObject>() };
    if class.is_null() { return Err(save_error("Conform object has no class")); }
    let class_name = access.name_text(unsafe { i32::from_ne_bytes((*class).name) })?;
    let mut full_name = class_name;
    full_name.push(u32::from(' '));
    full_name.extend(object_path_only(access, object)?);
    Ok(full_name)
}

