//! Rust-owned package linker tails and Core package loading.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use crate::package_archive::{OwnedPackage, PackageParts};
use crate::package_flow::{self, LoadObjectsOps};
use hp2_abi::engine::generated::{FLazyLoader, FGenerationInfo, FName, FObjectExport, FObjectImport, TArrayRaw, UClass, ULinkerLoad, UObject};
use std::alloc::Layout;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::ptr;

/// Rust ownership for the canonical linker prefix's raw arrays and source bytes.
/// The vectors below are the sole storage behind `ULinkerLoad`'s actual arrays.
pub(crate) struct LoaderTail {
    bytes: Rc<Vec<u8>>,
    names: Vec<FName>,
    imports: Vec<FObjectImport>,
    exports: Vec<FObjectExport>,
    import_loading: Vec<u8>,
    export_loading: Vec<u8>,
    generations: Vec<FGenerationInfo>,
    filename: Vec<u32>,
    path: Option<PathBuf>,
    loading: bool,
    verified: bool,
    archive_position: usize,
    lazy_loaders: Vec<*mut c_void>,
    lazy_entries: Vec<LazyArrayEntry>,
}
#[derive(Clone, Copy)]
struct LazyArrayEntry {
    loader: *mut c_void,
    array: *mut TArrayRaw,
    saved_archive: *mut *mut c_void,
    saved_position: *mut i32,
    row: usize,
    element_width: usize,
    body: crate::lazy_array::LazyArrayBody,
}

/// Extended Rust-owned linker allocation; the measured native prefix remains first.
#[repr(C)]
pub(crate) struct OwnedLinkerLoad {
    pub(crate) raw: ULinkerLoad,
    tail: MaybeUninit<LoaderTail>,
    initialized: bool,
    tail_release_safe: bool,
}


/// Returns the allocation layout required for every Rust-owned LinkerLoad.
pub(crate) fn allocation_layout() -> Layout {
    Layout::new::<OwnedLinkerLoad>()
}

fn local_name(names: &[FName], index: i32) -> RuntimeResult<i32> {
    let row = usize::try_from(index).map_err(|_| RuntimeError::new("Package name index out of range"))?;
    names.get(row).map(|name| name.index)
        .ok_or_else(|| RuntimeError::new("Package name index out of range"))
}

fn make_tail(access: CoreAccess<'_>, parts: PackageParts) -> RuntimeResult<LoaderTail> {
    let mut names = Vec::with_capacity(parts.names.len());
    for entry in &parts.names {
        let index = access.intern(&entry.text, crate::name_owner::FindName::Add)?;
        let canonical = access.name_at(index);
        if canonical.is_null() { return Err(RuntimeError::new("Canonical package name was not published")); }
        unsafe { (*canonical).flags |= entry.flags; }
        names.push(FName { index });
    }
    let mut imports = Vec::with_capacity(parts.imports.len());
    for entry in parts.imports {
        // Generated import records contain integers and nullable pointers.
        let mut import: FObjectImport = unsafe { std::mem::zeroed() };
        import.class_package = local_name(&names, entry.class_package)?.to_ne_bytes();
        import.class_name = local_name(&names, entry.class_name)?.to_ne_bytes();
        import.package_index = entry.package_index;
        import.object_name = local_name(&names, entry.object_name)?.to_ne_bytes();
        import.source_index = -1;
        imports.push(import);
    }
    let mut exports = Vec::with_capacity(parts.exports.len());
    for entry in parts.exports {
        // Zero initialization also initializes the generated record's padding.
        let mut export: FObjectExport = unsafe { std::mem::zeroed() };
        export.class_index = entry.class_index;
        export.super_index = entry.super_index;
        export.package_index = entry.package_index;
        export.object_name = local_name(&names, entry.object_name)?.to_ne_bytes();
        export.object_flags = entry.object_flags;
        export.serial_size = entry.serial_size;
        export.serial_offset = entry.serial_offset;
        export._i_hash_next = -1;
        exports.push(export);
    }
    if names.len() != parts.summary.name_count as usize
        || imports.len() != parts.summary.import_count as usize
        || exports.len() != parts.summary.export_count as usize
    {
        return Err(RuntimeError::new("Linker table count mismatch"));
    }
    let generations = parts.summary.generations.iter().map(|generation| FGenerationInfo {
        export_count: generation.export_count,
        name_count: generation.name_count,
    }).collect();
    let mut filename: Vec<u32> = parts.path.as_ref()
        .map(|path| path.to_string_lossy().chars().map(u32::from).collect())
        .unwrap_or_default();
    filename.push(0);
    let import_loading = vec![0; imports.len()];
    let export_loading = vec![0; exports.len()];
    Ok(LoaderTail {
        bytes: parts.bytes, names, imports, exports, import_loading, export_loading,
        generations, filename, path: parts.path, loading: false, verified: false,
        archive_position: 0, lazy_loaders: Vec::new(), lazy_entries: Vec::new(),
    })
}

fn array_header<T>(data: *const T, length: usize) -> RuntimeResult<TArrayRaw> {
    let count = i32::try_from(length).map_err(|_| RuntimeError::new("Linker table exceeds array limit"))?;
    Ok(TArrayRaw { data: data.cast_mut().cast(), num: count, max: count })
}

/// Installs canonical raw tables into the Rust tail and native linker prefix.
///
/// # Safety
/// `linker` points to a live, correctly sized `OwnedLinkerLoad` allocation with
/// a zeroed, uninitialized tail. It must be initialized exactly once.
pub(crate) unsafe fn initialize_tail(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    parts: PackageParts,
) -> RuntimeResult<()> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad allocation")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    let summary = &parts.summary;
    let file_version = ((u32::from(summary.licensee_version) << 16) | u32::from(summary.version)) as i32;
    let summary_tag = summary.tag as i32;
    let package_flags = summary.package_flags;
    let name_count = summary.name_count as i32;
    let name_offset = summary.name_offset as i32;
    let export_count = summary.export_count as i32;
    let export_offset = summary.export_offset as i32;
    let import_count = summary.import_count as i32;
    let import_offset = summary.import_offset as i32;
    let guid = summary.guid;
    let tail = make_tail(access, parts)?;
    let name_map = array_header(tail.names.as_ptr(), tail.names.len())?;
    let import_map = array_header(tail.imports.as_ptr(), tail.imports.len())?;
    let export_map = array_header(tail.exports.as_ptr(), tail.exports.len())?;
    let generations = array_header(tail.generations.as_ptr(), tail.generations.len())?;
    let filename = array_header(tail.filename.as_ptr(), tail.filename.len())?;
    unsafe {
        ptr::addr_of_mut!((*owner).tail).write(MaybeUninit::new(tail));
        ptr::addr_of_mut!((*owner).initialized).write(true);
        ptr::addr_of_mut!((*owner).tail_release_safe).write(false);
        (*owner).raw.lazy_loaders = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        (*owner).raw.base.name_map = name_map;
        (*owner).raw.base.import_map = import_map;
        (*owner).raw.base.export_map = export_map;
        (*owner).raw.base.filename = filename;
        (*owner).raw.base.summary.tag = summary_tag;
        (*owner).raw.base.summary.file_version = file_version;
        (*owner).raw.base.summary.package_flags = package_flags;
        (*owner).raw.base.summary.name_count = name_count;
        (*owner).raw.base.summary.name_offset = name_offset;
        (*owner).raw.base.summary.export_count = export_count;
        (*owner).raw.base.summary.export_offset = export_offset;
        (*owner).raw.base.summary.import_count = import_count;
        (*owner).raw.base.summary.import_offset = import_offset;
        (*owner).raw.base.summary.guid = guid;
        (*owner).raw.base.summary.generations = generations;
    }
    let _ = access;
    Ok(())
}

/// Explicitly releases linker-owned vectors and shared package bytes before allocation free.
///
/// # Safety
/// The pointer is a live owned LinkerLoad allocation in its destruction phase;
/// no subsequent loader hook may inspect its tail.
pub(crate) unsafe fn destroy_tail(linker: *mut UObject) -> RuntimeResult<()> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad allocation")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if unsafe { (*owner).initialized && !(*owner).tail_release_safe } {
        return Err(RuntimeError::new("Linker tail cannot be released before export detachment"));
    }
    if unsafe { (*owner).initialized } {
        unsafe {
            ptr::drop_in_place((*owner).tail.as_mut_ptr());
            ptr::addr_of_mut!((*owner).initialized).write(false);
            (*owner).raw.base.name_map = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            (*owner).raw.base.import_map = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            (*owner).raw.base.export_map = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            (*owner).raw.base.filename = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            (*owner).raw.base.summary.generations = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        }
    }
    Ok(())
}


pub(crate) struct LoaderState {
    pub(crate) linkers: Vec<*mut UObject>,
    pub(crate) begin_load_count: u32,
    loaded_objects: Vec<*mut UObject>,
    current_loaded: *mut UObject,
    pending_imports: bool,
}

struct BeginLoadScope(*mut LoaderState);

impl BeginLoadScope {
    unsafe fn begin(state: *mut LoaderState) -> Self {
        unsafe { (*state).begin_load_count = (*state).begin_load_count.wrapping_add(1); }
        Self(state)
    }

    fn complete(self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        let state = self.0;
        drop(self);
        if unsafe { (*state).begin_load_count } == 0 {
            finish_loading(access)?;
        }
        Ok(())
    }
}

impl Drop for BeginLoadScope {
    fn drop(&mut self) {
        unsafe { (*self.0).begin_load_count = (*self.0).begin_load_count.wrapping_sub(1); }
    }
}

impl LoaderState {
    pub(crate) fn new() -> Self {
        Self {
            linkers: Vec::new(), begin_load_count: 0,
            loaded_objects: Vec::new(), current_loaded: ptr::null_mut(), pending_imports: false,
        }
    }
}

pub(crate) fn linker_file_size(linker: *mut UObject) -> RuntimeResult<i32> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad allocation")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if !unsafe { (*owner).initialized } {
        return Err(RuntimeError::new("LinkerLoad tail is not initialized"));
    }
    let size = unsafe { (*owner).tail.assume_init_ref().bytes.len() };
    i32::try_from(size).map_err(|_| RuntimeError::new("Package file is too large"))
}

pub(crate) fn linker_local_name(linker: *mut UObject, index: i32) -> RuntimeResult<i32> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad allocation")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if !unsafe { (*owner).initialized } {
        return Err(RuntimeError::new("LinkerLoad tail is not initialized"));
    }
    let names = unsafe { &(*owner).tail.assume_init_ref().names };
    let row = usize::try_from(index).map_err(|_| RuntimeError::new("Package name index out of range"))?;
    names.get(row).map(|name| name.index)
        .ok_or_else(|| RuntimeError::new("Package name index out of range"))
}

/// Completes newly resolved exports only at the outermost load boundary.
pub(crate) fn begin_load(access: CoreAccess<'_>) {
    unsafe {
        let state = access.loader_state();
        (*state).begin_load_count = (*state).begin_load_count.wrapping_add(1);
    }
}

pub(crate) fn end_load(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let state = access.loader_state();
    let count = unsafe { (*state).begin_load_count };
    if count == 0 { return Err(RuntimeError::new("EndLoad without matching BeginLoad")); }
    unsafe { (*state).begin_load_count = count - 1; }
    if count == 1 { finish_loading(access)?; }
    Ok(())
}

///
/// # Safety
/// The callback and its objects belong to this engine-thread capability.
pub(crate) unsafe fn with_load_scope<T>(
    access: CoreAccess<'_>, body: impl FnOnce() -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    let scope = unsafe { BeginLoadScope::begin(access.loader_state()) };
    let value = body()?;
    scope.complete(access)?;
    Ok(value)
}

struct DirectLoadObjects<'a>(CoreAccess<'a>);

impl LoadObjectsOps for DirectLoadObjects<'_> {
    type Error = RuntimeError;
    fn loaded_count(&self) -> RuntimeResult<usize> {
        Ok(unsafe { (*self.0.loader_state()).loaded_objects.len() })
    }
    fn loaded_at(&self, index: usize) -> RuntimeResult<usize> {
        unsafe { (&(*self.0.loader_state()).loaded_objects).get(index).copied() }
            .map(|object| object as usize)
            .ok_or_else(|| RuntimeError::new("Loaded object index is out of range"))
    }
    fn current(&self) -> usize {
        unsafe { (*self.0.loader_state()).current_loaded as usize }
    }
    fn set_current(&self, object: usize) {
        unsafe { (*self.0.loader_state()).current_loaded = object as *mut UObject; }
    }
    fn sort_loaded_from(&self, index: usize) -> RuntimeResult<()> {
        let state = self.0.loader_state();
        let count = unsafe { (*state).loaded_objects.len() } - index;
        // Sorting only reads canonical linker keys; no hook can move this buffer.
        let objects = unsafe { (*state).loaded_objects.as_mut_ptr().add(index) };
        crate::sort::quicksort(
            count as isize,
            |valid| if valid { Ok(()) } else { Err(RuntimeError::new("Invalid loaded object sort range")) },
            |a, b| package_flow::compare_loaded_with(
                self,
                unsafe { *objects.add(a as usize) } as usize,
                unsafe { *objects.add(b as usize) } as usize,
            ),
            |a, b| unsafe { ptr::swap(objects.add(a as usize), objects.add(b as usize)); },
        )
    }
    fn needs_load(&self, object: usize) -> RuntimeResult<bool> {
        Ok(unsafe { (*(object as *mut UObject)).object_flags }
            & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load != 0)
    }
    fn preload_object(&self, object: usize) -> RuntimeResult<()> {
        unsafe { preload(self.0, object as *mut UObject) }
    }
    fn postload_object(&self, object: usize) -> RuntimeResult<()> {
        unsafe { self.0.conditional_post_load(object as *mut UObject) }
    }
    fn check_postload_count(&self, original: usize) -> RuntimeResult<()> {
        if self.loaded_count()? != original {
            return Err(RuntimeError::new("PostLoad added objects to the completed preload collection"));
        }
        Ok(())
    }
    fn release_loaded(&self) -> RuntimeResult<()> {
        unsafe { (*self.0.loader_state()).loaded_objects.clear(); }
        Ok(())
    }
    fn linker(&self, object: usize) -> RuntimeResult<usize> {
        Ok(unsafe { (*(object as *mut UObject))._linker as usize })
    }
    fn object_index(&self, object: usize) -> RuntimeResult<i32> {
        Ok(unsafe { (*(object as *mut UObject)).index })
    }
    fn linker_index(&self, object: usize) -> RuntimeResult<i32> {
        Ok(unsafe { (*(object as *mut UObject))._linker_index })
    }
    fn serial_offset(&self, linker: usize, row: i32) -> RuntimeResult<i32> {
        Ok(unsafe { (*export_slot(linker as *mut UObject, row as usize)?).serial_offset })
    }
}

fn finish_loading(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let ops = DirectLoadObjects(access);
    package_flow::preload_loaded_with(&ops)?;
    package_flow::postload_loaded_with(&ops)?;
    access.flush_localized()?;
    let state = access.loader_state();
    if unsafe { (*state).pending_imports } {
        // EndLoad drops non-native import caches; the export table owns their identity.
        let count = unsafe { (*state).linkers.len() };
        for index in 0..count {
            let linker = unsafe { (&(*state).linkers)[index] };
            if linker.is_null() { continue; }
            let imports = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.import_map };
            for row in 0..imports.num.max(0) as usize {
                let slot = unsafe { ptr::addr_of_mut!((*imports.data.cast::<FObjectImport>().add(row)).x_object) };
                let object = unsafe { slot.read_unaligned().cast::<UObject>() };
                if !object.is_null()
                    && unsafe { (*object).object_flags } & crate::object_lifecycle::RF_NATIVE == 0
                {
                    unsafe { slot.write_unaligned(ptr::null_mut()); }
                }
            }
        }
        unsafe { (*state).pending_imports = false; }
    }
    Ok(())
}

/// Opens and parses a package into an owned package value without translating its format.
pub(crate) fn open_package(path: &Path) -> RuntimeResult<PackageParts> {
    OwnedPackage::open(path).map(OwnedPackage::into_parts)
}

pub(crate) fn package_path(access: CoreAccess<'_>, path: &Path) -> RuntimeResult<Option<PathBuf>> {
    let input = crate::native_path::path_to_units(path)?;
    let Some(path) = crate::package_file_owner::find_package_path(access, &input)? else {
        return Ok(None);
    };
    // One physical package must not acquire two linkers through System/../System
    // search prefixes or an explicitly supplied filename.
    std::fs::canonicalize(&path).map(Some).map_err(|error| {
        RuntimeError::new("Package filename could not be resolved")
            .with_context(&format!("{}: {error}", path.display()))
    })
}

/// Native and file-backed packages share the source top-level package namespace.
pub(crate) fn find_package_root(access: CoreAccess<'_>, name: i32) -> RuntimeResult<*mut UObject> {
    let class = access.native_class("Package")?;
    Ok(unsafe { access.find_object(class, ptr::null_mut(), name, true) })
}

/// Loads package metadata into canonical Core-owned linker records.
///
/// # Safety
/// Core native classes are initialized and the supplied capability is the engine thread.
pub(crate) unsafe fn load_package(
    access: CoreAccess<'_>,
    path: &Path,
    flags: u32,
) -> RuntimeResult<*mut UObject> {
    unsafe {
        with_load_scope(access, || {
            let linker = get_package_linker_body(access, path, flags, ptr::null_mut(), None)?;
            if flags & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.verify == 0 {
                let tail = tail_mut_ptr(linker)?;
                if !(*tail).loading {
                    (*tail).loading = true;
                    let count = (*linker.cast::<OwnedLinkerLoad>()).raw.base.export_map.num.max(0);
                    for row in 0..count {
                        if let Err(error) = create_export(access, linker, row as usize) {
                            (*tail_mut_ptr(linker)?).loading = false;
                            return Err(discard_failed_linker(access, linker,
                                error.with_context(&format!("{}: export {row}", path.display()))));
                        }
                    }
                    (*tail_mut_ptr(linker)?).loading = false;
                }
            }
            Ok(linker)
        })
    }
}

/// Acquires package metadata without constructing unrelated exports.
///
/// # Safety
/// Core classes are initialized, and `parent` is null or a live package on the engine thread.
pub(crate) unsafe fn get_package_linker(
    access: CoreAccess<'_>,
    path: &Path,
    flags: u32,
    parent: *mut UObject,
    expected_guid: Option<&[u32; 4]>,
) -> RuntimeResult<*mut UObject> {
    unsafe { with_load_scope(access, || get_package_linker_body(access, path, flags, parent, expected_guid)) }
}

unsafe fn get_package_linker_body(
    access: CoreAccess<'_>, path: &Path, flags: u32, parent: *mut UObject,
    expected_guid: Option<&[u32; 4]>,
) -> RuntimeResult<*mut UObject> {
    if !parent.is_null() {
        if let Some(linker) = linker_for_root(access, parent) {
            let guid = unsafe { ptr::addr_of!((*linker.cast::<OwnedLinkerLoad>()).raw.base.summary.guid).read_unaligned() };
            if expected_guid.is_some_and(|expected| guid != *expected) {
                return Err(RuntimeError::new("Package GUID does not match the expected linker GUID")
                    .with_context(&path.display().to_string()));
            }
            return Ok(linker);
        }
    }
    let path = if let Some(guid) = expected_guid {
        let name = crate::native_path::path_to_units(path)?;
        let mut resolved = [0u32; 256];
        if !crate::package_file_owner::find_package_file(access, &name, Some(guid), &mut resolved)? {
            return Err(RuntimeError::new("Package file with expected GUID was not found")
                .with_context(&path.display().to_string()));
        }
        crate::native_path::path_from_units(&resolved)?
    } else {
        package_path(access, path)?.ok_or_else(|| {
            RuntimeError::new("Package file was not found").with_context(&path.display().to_string())
        })?
    };
    let package_name = path.file_name().and_then(|value| value.to_str())
        .and_then(|value| value.split('.').next())
        .ok_or_else(|| RuntimeError::new("Filename has no package name"))?;
    let package_name_index = access.intern_str(package_name, crate::name_owner::FindName::Add)?;
    let package_class = access.native_class("Package")?;
    let existing = if parent.is_null() {
        find_package_root(access, package_name_index)?
    } else {
        parent
    };
    let root = if existing.is_null() {
        unsafe {
            access.construct_object(
                package_class, ptr::null_mut(), package_name_index,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.public_flag, ptr::null_mut(),
            )?
        }
    } else {
        existing
    };
    if root.is_null() { return Err(RuntimeError::new("Package root is not initialized")); }
    if let Some(cached) = linker_for_root(access, root) {
        let tail = unsafe { &*tail_ptr(cached)? };
        if tail.loading || unsafe { (*cached.cast::<OwnedLinkerLoad>()).raw.base.success != 0 } {
            let guid = unsafe { ptr::addr_of!((*cached.cast::<OwnedLinkerLoad>()).raw.base.summary.guid).read_unaligned() };
            if expected_guid.is_some_and(|expected| guid != *expected) {
                return Err(RuntimeError::new("Package GUID does not match the expected linker GUID")
                    .with_context(&path.display().to_string()));
            }
            return Ok(cached);
        }
        return Err(RuntimeError::new("Package load previously failed")
            .with_context(&path.display().to_string()));
    }
    let parts = open_package(&path)
        .map_err(|error| error.with_context("ULinkerLoad::ULinkerLoad"))?;
    if expected_guid.is_some_and(|guid| parts.summary.guid != *guid) {
        return Err(RuntimeError::new("Package GUID does not match the expected linker GUID")
            .with_context(&path.display().to_string()));
    }
    let minimum = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.min_version as u16;
    if parts.summary.version < minimum {
        return Err(RuntimeError::new("Package version is too old")
            .with_context(&format!("{}: {} < {minimum}", path.display(), parts.summary.version)));
    }
    let package_flags = parts.summary.package_flags;
    let linker_class = access.native_class("LinkerLoad")?;
    let linker_name = access.intern_str(&format!("Linker_{package_name}"), crate::name_owner::FindName::Add)?;
    let linker = unsafe { access.construct_object(linker_class, root, linker_name, flags, ptr::null_mut())? };
    if let Err(error) = unsafe { initialize_tail(access, linker, parts) } {
        return Err(unsafe { discard_failed_linker(access, linker, error) });
    }
    let owner = linker.cast::<OwnedLinkerLoad>();
    let mode = access.process_mode();
    let constants = &hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let context_flags = crate::linker_runtime::load_context_flags(
        mode.is_editor, mode.is_client, mode.is_server,
        constants.load_edit, constants.load_client, constants.load_server,
    );
    unsafe {
        (*owner).raw.load_flags = flags;
        (*owner).raw.base.linker_root = root.cast();
        (*owner).raw.base._context_flags = context_flags;
        (*root.cast::<crate::core_records::PackageRecord>()).package_flags = package_flags;
    }
    unsafe { initialize_archive_state(linker, context_flags); }
    if let Err(error) = build_export_hash(linker) {
        let error = error.with_context(&path.display().to_string());
        return Err(unsafe { discard_failed_linker(access, linker, error) });
    }
    let loading_tail = unsafe { tail_mut_ptr(linker) };
    match loading_tail {
        Ok(tail) => unsafe { (*tail).loading = true; },
        Err(error) => return Err(unsafe { discard_failed_linker(access, linker, error) }),
    }
    unsafe { (&mut *access.loader_state()).linkers.push(linker); }
    if flags & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.no_verify == 0 {
        if let Err(error) = verify_linker_imports(access, linker) {
            if let Ok(tail) = unsafe { tail_mut_ptr(linker) } {
                unsafe { (*tail).loading = false; }
            }
            let error = error.with_context(&path.display().to_string());
            return Err(unsafe { discard_failed_linker(access, linker, error) });
        }
    }
    let tail = match unsafe { tail_mut_ptr(linker) } {
        Ok(tail) => tail,
        Err(error) => return Err(unsafe { discard_failed_linker(access, linker, error) }),
    };
    unsafe {
        (*tail).loading = false;
        (*owner).raw.base.success = 1;
    }
    Ok(linker)
}

unsafe fn discard_failed_linker(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    error: RuntimeError,
) -> RuntimeError {
    let index = unsafe { ptr::read_unaligned(ptr::addr_of!((*linker).index)) };
    if index < 0 || access.object_at(index) != linker {
        return error.with_context("failed linker cleanup: canonical object slot is unavailable");
    }
    let destroy = unsafe { access.destroy_slot(index) };
    let slot_is_linker = access.object_at(index) == linker;
    let teardown_ready = slot_is_linker
        && unsafe { (*access.storage()).teardown_bucket(index).is_some() };
    let still_registered = unsafe {
        (&(*access.loader_state()).linkers).contains(&linker)
    };
    let tail_release_safe = if slot_is_linker {
        let owner = linker.cast::<OwnedLinkerLoad>();
        unsafe { !(*owner).initialized || (*owner).tail_release_safe }
    } else {
        false
    };
    let free = if teardown_ready && !still_registered && tail_release_safe {
        Some(unsafe { access.free_slot(index) })
    } else {
        None
    };
    let mut error = error;
    if let Err(cleanup) = destroy {
        error = error.with_context(&format!("linker destruction failed: {cleanup}"));
    }
    if let Some(Err(cleanup)) = free {
        error = error.with_context(&format!("linker storage release failed: {cleanup}"));
    } else if still_registered {
        error = error.with_context("linker storage retained because it remains registered");
    } else if !tail_release_safe {
        error = error.with_context("linker storage retained because export detachment was not verified");
    } else if !teardown_ready {
        error = error.with_context("linker storage release was not safe after teardown");
    }
    error
}
unsafe fn initialize_archive_state(linker: *mut UObject, context_flags: u32) {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let archive = unsafe {
        (ptr::addr_of_mut!((*owner).raw).cast::<u8>()
            .add(hp2_abi::engine::generated::ULinkerLoad::FARCHIVE_OFFSET))
            .cast::<hp2_abi::engine::generated::FArchive>()
    };
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let file_version = unsafe { (*owner).raw.base.summary.file_version as u32 };
    unsafe {
        (*archive).ar_ver = file_version as u16 as i32;
        (*archive).ar_licensee_ver = (file_version >> 16) as u16 as i32;
        (*archive).ar_is_loading = 1;
        (*archive).ar_is_saving = 0;
        (*archive).ar_is_trans = 0;
        (*archive).ar_is_persistent = 1;
        (*archive).ar_for_edit = if context_flags & constants.load_edit != 0 { 1 } else { 0 };
        (*archive).ar_for_client = if context_flags & constants.load_client != 0 { 1 } else { 0 };
        (*archive).ar_for_server = if context_flags & constants.load_server != 0 { 1 } else { 0 };
        (*archive).ar_is_error = 0;
    }
}
fn import_row_from_index(index: i32, diagnostic: &str) -> RuntimeResult<usize> {
    let row = index.checked_neg().and_then(|absolute| absolute.checked_sub(1))
        .ok_or_else(|| RuntimeError::new(diagnostic))?;
    usize::try_from(row).map_err(|_| RuntimeError::new(diagnostic))
}

fn export_index_from_row(index: i32) -> RuntimeResult<i32> {
    if index < 0 { return Err(RuntimeError::new("Invalid source export index")); }
    index.checked_add(1).ok_or_else(|| RuntimeError::new("Source export index out of range"))
}
fn export_class_key(linker: *mut UObject, export: FObjectExport) -> RuntimeResult<(i32, i32)> {

    let owner = linker.cast::<OwnedLinkerLoad>();
    let class_index = export.class_index;
    if class_index == 0 {
        let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        return Ok((constants.class_name, constants.core));
    }
    if class_index > 0 {
        let row = usize::try_from(class_index - 1).map_err(|_| RuntimeError::new("Export class index out of range"))?;
        let raw = unsafe { &(*owner).raw.base.export_map };
        if row >= raw.num.max(0) as usize || raw.data.is_null() {
            return Err(RuntimeError::new("Export class index out of range"));
        }
        let class = unsafe { ptr::read_unaligned(raw.data.cast::<FObjectExport>().add(row)) };
        let package = unsafe { (*owner).raw.base.linker_root.cast::<UObject>() };
        let package_name = unsafe { i32::from_ne_bytes((*package).name) };
        return Ok((i32::from_ne_bytes(class.object_name), package_name));
    }
    let row = import_row_from_index(class_index, "Import class index out of range")?;
    let raw = unsafe { &(*owner).raw.base.import_map };
    if row >= raw.num.max(0) as usize || raw.data.is_null() {
        return Err(RuntimeError::new("Import class index out of range"));
    }
    let class = unsafe { ptr::read_unaligned(raw.data.cast::<FObjectImport>().add(row)) };
    let package = if class.package_index == 0 {
        i32::from_ne_bytes(class.class_package)
    } else if class.package_index < 0 {
        let package_row = import_row_from_index(class.package_index, "Import package index out of range")?;
        if package_row >= raw.num.max(0) as usize { return Err(RuntimeError::new("Import package index out of range")); }
        i32::from_ne_bytes(unsafe { ptr::read_unaligned(raw.data.cast::<FObjectImport>().add(package_row)) }.object_name)
    } else {
        let package_row = usize::try_from(class.package_index - 1).map_err(|_| RuntimeError::new("Export package index out of range"))?;
        let exports = unsafe { &(*owner).raw.base.export_map };
        if package_row >= exports.num.max(0) as usize { return Err(RuntimeError::new("Export package index out of range")); }
        i32::from_ne_bytes(unsafe { ptr::read_unaligned(exports.data.cast::<FObjectExport>().add(package_row)) }.object_name)
    };
    Ok((i32::from_ne_bytes(class.object_name), package))
}

fn build_export_hash(linker: *mut UObject) -> RuntimeResult<()> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    unsafe { (*owner).raw.export_hash.fill(-1); }
    let exports = unsafe { (*owner).raw.base.export_map };
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    for row in 0..exports.num.max(0) as usize {
        let export_ptr = unsafe { exports.data.cast::<FObjectExport>().add(row) };
        let export = unsafe { ptr::read_unaligned(export_ptr) };
        let (class_name, class_package) = export_class_key(linker, export)?;
        let hash = crate::linker_runtime::hash_names(
            i32::from_ne_bytes(export.object_name), class_name, class_package,
            constants.unreal_i, constants.unreal_share,
        ) as usize & 255;
        let index = i32::try_from(row).map_err(|_| RuntimeError::new("Export index exceeds linker hash range"))?;
        unsafe {
            ptr::write_unaligned(ptr::addr_of_mut!((*export_ptr)._i_hash_next), (*owner).raw.export_hash[hash]);
            (*owner).raw.export_hash[hash] = index;
        }
    }
    Ok(())
}

unsafe fn tail_ptr(linker: *mut UObject) -> RuntimeResult<*const LoaderTail> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if !unsafe { (*owner).initialized } { return Err(RuntimeError::new("LinkerLoad tail is not initialized")); }
    Ok(unsafe { (*owner).tail.as_ptr() })
}
unsafe fn tail_mut_ptr(linker: *mut UObject) -> RuntimeResult<*mut LoaderTail> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if !unsafe { (*owner).initialized } { return Err(RuntimeError::new("LinkerLoad tail is not initialized")); }
    Ok(unsafe { (*owner).tail.as_mut_ptr() })
}

/// Resolves a serialized object index through the canonical linker arrays.
///
/// # Safety
/// `linker` is a live initialized owned linker; recursive resolution is engine-thread-only.
pub(crate) unsafe fn resolve_index(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    index: i32,
) -> RuntimeResult<*mut UObject> {
    if index == 0 { return Ok(ptr::null_mut()); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    if index > 0 {
        let row = usize::try_from(index - 1).map_err(|_| RuntimeError::new("Export index out of range"))?;
        let map = unsafe { (*owner).raw.base.export_map };
        if row >= map.num.max(0) as usize || (map.num > 0 && map.data.is_null()) {
            return Err(RuntimeError::new("Export index out of range"));
        }
        return unsafe { create_export(access, linker, row) };
    }
    let row = import_row_from_index(index, "Import index out of range")?;
    let map = unsafe { (*owner).raw.base.import_map };
    if row >= map.num.max(0) as usize || (map.num > 0 && map.data.is_null()) {
        return Err(RuntimeError::new("Import index out of range"));
    }
    let import = unsafe { ptr::read_unaligned(map.data.cast::<FObjectImport>().add(row)) };
    if !import.x_object.is_null() {
        let object = import.x_object.cast::<UObject>();
        if is_core_class_import(&import) {
            // Bind a matching package export onto the registered class before
            // preloading its reflected metadata. Keep the import cache canonical.
            let mut should_preload = !unsafe { (*object)._linker }.is_null();
            if !should_preload {
                if let Some(bound) = unsafe { resolve_class_import_export(access, linker, row)? } {
                    if bound.is_null() {
                        let slot = import_slot(linker, row)?;
                        unsafe {
                            ptr::write_unaligned(
                                ptr::addr_of_mut!((*slot).x_object),
                                ptr::null_mut(),
                            );
                        }
                        return Ok(ptr::null_mut());
                    }
                    if bound != object {
                        return Err(RuntimeError::new("Class import export replaced its canonical object"));
                    }
                    if unsafe { (*object)._linker }.is_null() {
                        return Err(RuntimeError::new("Class import export did not attach linker identity"));
                    }
                    should_preload = true;
                }
            }
            if should_preload {
                unsafe { preload(access, object)?; }
            }
        }
        return Ok(object);
    }
    {
        let tail = unsafe { &mut *tail_mut_ptr(linker)? };
        if tail.import_loading.get(row).copied().ok_or_else(|| RuntimeError::new("Import state index out of range"))? != 0 {
            return Err(RuntimeError::new("Cyclic package import resolution").with_context(&format!("import {row}")));
        }
        tail.import_loading[row] = 1;
    }
    let resolved = unsafe {
        (|| -> RuntimeResult<*mut UObject> {
            match verify_import(access, linker, row) {
                Err(error) => Err(error),
                Ok(true) => Ok(ptr::null_mut()),
                Ok(false) => {
                    let current_map = (*owner).raw.base.import_map;
                    let current = ptr::read_unaligned(current_map.data.cast::<FObjectImport>().add(row));
                    if current.source_index != -1 {
                        if current.source_linker.is_null() {
                            Err(RuntimeError::new("Resolved import has no source linker"))
                        } else {
                            resolve_index(
                                access,
                                current.source_linker.cast(),
                                export_index_from_row(current.source_index)?,
                            )
                        }
                    } else {
                        match resolve_import_body(access, linker, &current) {
                            Ok(object) if object.is_null() => {
                                match try_shareware_import(access, linker, row)? {
                                    Some(object) => Ok(object),
                                    None => Ok(ptr::null_mut()),
                                }
                            }
                            Ok(object) => Ok(object),
                            Err(error) if is_missing_import(&error) => {
                                match try_shareware_import(access, linker, row)? {
                                    Some(object) => Ok(object),
                                    None => Err(error),
                                }
                            }
                            Err(error) => Err(error),
                        }
                    }
                }
            }
        })()
    };
    unsafe { (&mut (*tail_mut_ptr(linker)?).import_loading)[row] = 0; }
    match resolved {
        Ok(object) => {
            let map = unsafe { (*owner).raw.base.import_map };
            let slot = unsafe { map.data.cast::<FObjectImport>().add(row) };
            unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*slot).x_object), object.cast()); }
            if !object.is_null() {
                unsafe { (*access.loader_state()).pending_imports = true; }
            }
            // Publish the import before loading class fields so reentrant self-imports
            // observe the canonical class and the class preloading guard.
            if !object.is_null() && is_core_class_import(&import) {
                unsafe { preload(access, object)?; }
            }
            Ok(object)
        }
        Err(error) if unsafe { (*owner).raw.load_flags }
            & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.forgiving != 0
            && is_missing_import(&error) =>
        {
            mark_broken_import(owner);
            let map = unsafe { (*owner).raw.base.import_map };
            let slot = unsafe { map.data.cast::<FObjectImport>().add(row) };
            unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*slot).x_object), ptr::null_mut()); }
            Ok(ptr::null_mut())
        }
        Err(error) => Err(error.with_context(&format!("import {row}"))),
    }
}

fn mark_broken_import(owner: *mut OwnedLinkerLoad) {
    let root = unsafe { (*owner).raw.base.linker_root.cast::<crate::core_records::PackageRecord>() };
    if !root.is_null() {
        unsafe { (*root).package_flags |= hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.broken_links; }
    }
}

fn is_missing_import(error: &RuntimeError) -> bool {
    [ "Unresolved package import", "Unresolved import class", "Unresolved package import outer" ]
        .iter()
        .any(|text| error.diagnostic().iter().copied().eq(text.chars().map(u32::from)))
}
fn import_slot(linker: *mut UObject, row: usize) -> RuntimeResult<*mut FObjectImport> {
    let map = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.import_map };
    if row >= map.num.max(0) as usize || map.data.is_null() {
        return Err(RuntimeError::new("Import index out of range"));
    }
    Ok(unsafe { map.data.cast::<FObjectImport>().add(row) })
}

fn verify_linker_imports(access: CoreAccess<'_>, linker: *mut UObject) -> RuntimeResult<()> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    if unsafe { (*tail_ptr(linker)?).verified } { return Ok(()); }
    let root = unsafe { (*owner).raw.base.linker_root.cast::<UObject>() };
    if root.is_null() { return Err(RuntimeError::new("Linker has no package root")); }
    let package_class = access.native_class("Package")?;
    if unsafe { crate::object_model::is_a(root.cast(), package_class.cast()) } {
        let package = root.cast::<crate::core_records::PackageRecord>();
        unsafe {
            (*package).package_flags &= !hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.broken_links;
        }
    }
    let count = unsafe { (*owner).raw.base.import_map.num.max(0) };
    for row in 0..count {
        verify_import(access, linker, row as usize)?;
    }
    unsafe { (*tail_mut_ptr(linker)?).verified = true; }
    Ok(())
}

fn verify_import(access: CoreAccess<'_>, linker: *mut UObject, selected_import_row: usize) -> RuntimeResult<bool> {
    verify_import_at_depth(access, linker, selected_import_row, 0)
}

fn verify_import_at_depth(
    access: CoreAccess<'_>, linker: *mut UObject, selected_import_row: usize, depth: usize,
) -> RuntimeResult<bool> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let import = unsafe { ptr::read_unaligned(import_slot(linker, selected_import_row)?) };
    if depth >= unsafe { (*owner).raw.base.import_map.num.max(0) as usize } {
        return Err(RuntimeError::new("Cyclic package import resolution")
            .with_context(&format!("import {selected_import_row}")));
    }
    if import.source_index < -1 { return Err(RuntimeError::new("Invalid resolved import index")); }
    if import.source_index >= 0 {
        if import.source_linker.is_null() {
            return Err(RuntimeError::new("Resolved import has no source linker"));
        }
        return Ok(false);
    }
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let parent = import.package_index;
    let (source, parent_source_index) = if parent == 0 {
        if i32::from_ne_bytes(import.class_name) != constants.package
            || i32::from_ne_bytes(import.class_package) != constants.core
        {
            return Err(RuntimeError::new("Invalid top-level package import")
                .with_context(&native_name_for(&access.name_text(i32::from_ne_bytes(import.object_name))?)));
        }
        let name = access.name_text(i32::from_ne_bytes(import.object_name))?;
        let linker_flags = unsafe { (*owner).raw.load_flags } & constants.propagate;
        let package_flags = constants.throw_flag | linker_flags;
        let (_, source) = package_root_and_linker_flags(access, &name, package_flags)?;
        (source, -1)
    } else {
        if parent > 0 { return Err(RuntimeError::new("Invalid positive import outer")); }
        let source_import_row = import_row_from_index(parent, "Import outer index out of range")?;
        verify_import_at_depth(access, linker, source_import_row, depth + 1)?;
        let source_import_map = unsafe { (*owner).raw.base.import_map };
        if source_import_row >= source_import_map.num.max(0) as usize
            || source_import_map.data.is_null()
        {
            return Err(RuntimeError::new("Import outer index out of range"));
        }
        let source_import = unsafe {
            ptr::read_unaligned(source_import_map.data.cast::<FObjectImport>().add(source_import_row))
        };
        if source_import.source_index < -1 {
            return Err(RuntimeError::new("Invalid resolved import outer index"));
        }
        if source_import.source_linker.is_null() {
            return Err(RuntimeError::new("Resolved import outer has no source linker"));
        }
        (source_import.source_linker.cast(), source_import.source_index)
    };
    if source.is_null() { return Err(RuntimeError::new("Import source linker is null")); }
    unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*import_slot(linker, selected_import_row)?).source_linker), source.cast()); }
    let name = i32::from_ne_bytes(import.object_name);
    let class_name = i32::from_ne_bytes(import.class_name);
    let object_class = class_name;
    let class_package = i32::from_ne_bytes(import.class_package);
    let hash = crate::linker_runtime::hash_names(
        name, class_name, class_package, constants.unreal_i, constants.unreal_share,
    ) as usize & 255;
    let source_owner = source.cast::<OwnedLinkerLoad>();
    let export_count = unsafe { (*source_owner).raw.base.export_map.num.max(0) as usize };
    let export_data = unsafe { (*source_owner).raw.base.export_map.data };
    if export_count != 0 && export_data.is_null() {
        return Err(RuntimeError::new("Export table has no backing storage"));
    }
    let mut export_row = unsafe { (*source_owner).raw.export_hash[hash] };
    let mut visited = 0usize;
    while export_row != -1 {
        if visited >= export_count { return Err(RuntimeError::new("Cyclic export hash chain")); }
        visited += 1;
        let export_table_row = usize::try_from(export_row).map_err(|_| RuntimeError::new("Invalid export hash index"))?;
        if export_table_row >= export_count { return Err(RuntimeError::new("Export hash index out of range")); }
        let export_ptr = unsafe { export_data.cast::<FObjectExport>().add(export_table_row) };
        let export = unsafe { ptr::read_unaligned(export_ptr) };
        let (candidate_class, candidate_package) = export_class_key(source, export)?;
        let class_hack = class_package == constants.unreal_i && candidate_package == constants.unreal_share;
        if i32::from_ne_bytes(export.object_name) == name
            && candidate_class == class_name
            && (candidate_package == class_package || class_hack)
            && (parent >= 0 || parent_matches(export.package_index, parent_source_index))
        {
            if export.object_flags & constants.public_flag == 0 {
                if unsafe { (*owner).raw.load_flags } & constants.forgiving != 0 {
                    let root = unsafe { (*owner).raw.base.linker_root.cast::<crate::core_records::PackageRecord>() };
                    if !root.is_null() { unsafe { (*root).package_flags |= constants.broken_links; } }
                    return Ok(true);
                }
                return Err(RuntimeError::new("Private package import")
                    .with_context(&native_name_for(&access.name_text(name)?)));
            }
            unsafe {
                ptr::write_unaligned(
                    ptr::addr_of_mut!((*import_slot(linker, selected_import_row)?).source_index),
                    export_row,
                );
            }
            break;
        }
        export_row = unsafe { ptr::read_unaligned(ptr::addr_of!((*export_ptr)._i_hash_next)) };
    }
    let mesh_name = access.intern_str("Mesh", crate::name_owner::FindName::Find)?;
    if object_class == mesh_name {
        let lod_mesh = access.intern_str("LodMesh", crate::name_owner::FindName::Find)?;
        unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*import_slot(linker, selected_import_row)?).class_name), lod_mesh.to_ne_bytes()); }
        // The legacy retry runs after the initial hash lookup, including a hit.
        let retry_hash = crate::linker_runtime::hash_names(
            name, lod_mesh, class_package, constants.unreal_i, constants.unreal_share,
        ) as usize & 255;
        let mut retry = unsafe { (*source_owner).raw.export_hash[retry_hash] };
        let mut visited = 0usize;
        while retry != -1 {
            if visited >= export_count { return Err(RuntimeError::new("Cyclic export hash chain")); }
            visited += 1;
            let retry_row = usize::try_from(retry).map_err(|_| RuntimeError::new("Invalid export hash index"))?;
            if retry_row >= export_count { return Err(RuntimeError::new("Export hash index out of range")); }
            let export_ptr = unsafe { export_data.cast::<FObjectExport>().add(retry_row) };
            let export = unsafe { ptr::read_unaligned(export_ptr) };
            let (candidate_class, candidate_package) = export_class_key(source, export)?;
            if i32::from_ne_bytes(export.object_name) == name
                && candidate_class == lod_mesh
                && (candidate_package == class_package || (class_package == constants.unreal_i && candidate_package == constants.unreal_share))
                && (parent >= 0 || parent_matches(export.package_index, parent_source_index))
            {
                if export.object_flags & constants.public_flag == 0 {
                    if unsafe { (*owner).raw.load_flags } & constants.forgiving != 0 {
                        let root = unsafe { (*owner).raw.base.linker_root.cast::<crate::core_records::PackageRecord>() };
                        if !root.is_null() { unsafe { (*root).package_flags |= constants.broken_links; } }
                        return Ok(true);
                    }
                    return Err(RuntimeError::new("Private package import")
                        .with_context(&native_name_for(&access.name_text(name)?)));
                }
                unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*import_slot(linker, selected_import_row)?).source_index), retry); }
                break;
            }
            retry = unsafe { ptr::read_unaligned(ptr::addr_of!((*export_ptr)._i_hash_next)) };
        }
    }
    Ok(false)
}

fn parent_matches(source_package: i32, parent_source: i32) -> bool {
    source_package == 0 || (parent_source != -1 && parent_source.wrapping_add(1) == source_package)
}

unsafe fn try_shareware_import(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
) -> RuntimeResult<Option<*mut UObject>> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let map = unsafe { (*owner).raw.base.import_map };
    if row >= map.num.max(0) as usize || map.data.is_null() {
        return Err(RuntimeError::new("Import index out of range"));
    }
    let import_ptr = unsafe { map.data.cast::<FObjectImport>().add(row) };
    let import = unsafe { ptr::read_unaligned(import_ptr) };
    if import.package_index >= 0 || import.source_linker.is_null() { return Ok(None); }
    let parent_row = import_row_from_index(import.package_index, "Import outer index out of range")?;
    if parent_row >= map.num.max(0) as usize { return Err(RuntimeError::new("Import outer index out of range")); }
    let parent = unsafe { ptr::read_unaligned(map.data.cast::<FObjectImport>().add(parent_row)) };
    if parent.package_index != 0 { return Ok(None); }
    let source = import.source_linker.cast::<UObject>();
    let root = unsafe { (*source.cast::<OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
    if root.is_null() { return Err(RuntimeError::new("Import source linker has no package root")); }
    let root_name = access.name_text(i32::from_ne_bytes(unsafe { (*root).name }))?;
    if !root_name.iter().copied().eq("UnrealI".chars().map(u32::from)) { return Ok(None); }

    let core = access.intern_str("Core", crate::name_owner::FindName::Add)?;
    let package = access.intern_str("Package", crate::name_owner::FindName::Add)?;
    let share = access.intern_str("UnrealShare", crate::name_owner::FindName::Add)?;
    let tail = unsafe { tail_mut_ptr(linker)? };
    let imports = unsafe { &mut (*tail).imports };
    let new_row = imports.len();
    let new_count = i32::try_from(new_row.checked_add(1)
        .ok_or_else(|| RuntimeError::new("Import index exceeds linker index range"))?)
        .map_err(|_| RuntimeError::new("Import index exceeds linker index range"))?;
    let mut import: FObjectImport = unsafe { std::mem::zeroed() };
    import.class_package = core.to_ne_bytes();
    import.class_name = package.to_ne_bytes();
    import.object_name = share.to_ne_bytes();
    import.source_index = -1;
    imports.push(import);
    unsafe { (*tail).import_loading.push(0); }
    let max = i32::try_from(imports.capacity())
        .map_err(|_| RuntimeError::new("Linker table exceeds array limit"))?;
    let header = TArrayRaw {
        data: imports.as_mut_ptr().cast(),
        num: new_count,
        max,
    };
    unsafe {
        (*owner).raw.base.import_map = header;
        let updated_map = (*owner).raw.base.import_map;
        ptr::write_unaligned(
            ptr::addr_of_mut!((*updated_map.data.cast::<FObjectImport>().add(row)).package_index),
            new_count.checked_neg().ok_or_else(|| RuntimeError::new("Import index exceeds linker index range"))?,
        );
    }
    let share_package = unsafe { resolve_index(access, linker, -new_count)? };
    if share_package.is_null() { return Ok(Some(ptr::null_mut())); }
    match verify_import(access, linker, row)? {
        true => Ok(Some(ptr::null_mut())),
        false => {
            let updated = unsafe { ptr::read_unaligned((*owner).raw.base.import_map.data.cast::<FObjectImport>().add(row)) };
            if updated.source_index != -1 && !updated.source_linker.is_null() {
                unsafe { resolve_index(access, updated.source_linker.cast(), export_index_from_row(updated.source_index)?) }.map(Some)
            } else {
                unsafe { resolve_import_body(access, linker, &updated) }.map(Some)
            }
        }
    }
}

struct FindAnyContext {
    class: *mut UObject,
    name: i32,
    found: *mut UObject,
}

unsafe extern "C" fn match_any_import_object(
    object: *mut std::ffi::c_void,
    context: *mut std::ffi::c_void,
    matches: *mut i32,
) -> i32 {
    let object = object.cast::<UObject>();
    let context = context.cast::<FindAnyContext>();
    unsafe {
        *matches = (i32::from_ne_bytes((*object).name) == (*context).name
            && crate::object_model::is_a(object.cast(), (*context).class.cast()))
            as i32;
        if *matches != 0 { (*context).found = object; }
    }
    0
}

fn find_any_import_object(
    access: CoreAccess<'_>,
    class: *mut UObject,
    name: i32,
) -> RuntimeResult<*mut UObject> {
    let mut context = FindAnyContext { class, name, found: ptr::null_mut() };
    let mut index = -1;
    unsafe {
        crate::object_storage::next_index(
            access.storage(),
            &mut index,
            (&mut context as *mut FindAnyContext).cast(),
            match_any_import_object,
        )
    }.map_err(|_| RuntimeError::new("Canonical object scan failed"))?;
    Ok(context.found)
}

fn package_for_import(linker: *mut UObject, import: &FObjectImport) -> RuntimeResult<*mut UObject> {
    let mut parent = import.package_index;
    if parent >= 0 { return Ok(ptr::null_mut()); }
    loop {
        let row = import_row_from_index(parent, "Import package index out of range")?;
        let ancestor = unsafe { ptr::read_unaligned(import_slot(linker, row)?) };
        if ancestor.package_index == 0 {
            let mut package = ancestor.x_object.cast::<UObject>();
            // Metadata verification can own the linker root before resolving its package import.
            if package.is_null() && !ancestor.source_linker.is_null() {
                package = unsafe {
                    (*ancestor.source_linker.cast::<OwnedLinkerLoad>())
                        .raw.base.linker_root.cast::<UObject>()
                };
            }
            return if package.is_null() {
                Err(RuntimeError::new("Unresolved package import"))
            } else {
                Ok(package)
            };
        }
        if ancestor.package_index > 0 {
            return Err(RuntimeError::new("Invalid positive import outer"));
        }
        parent = ancestor.package_index;
    }
}

fn is_core_class_import(import: &FObjectImport) -> bool {
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    i32::from_ne_bytes(import.class_package) == constants.core
        && i32::from_ne_bytes(import.class_name) == constants.class_name
}

unsafe fn resolve_class_import_export(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
) -> RuntimeResult<Option<*mut UObject>> {
    let import = unsafe { ptr::read_unaligned(import_slot(linker, row)?) };
    if !is_core_class_import(&import) {
        return Ok(None);
    }
    if import.source_index == -1 && import.source_linker.is_null() {
        match import.package_index {
            0 => return Ok(None),
            parent if parent > 0 => {
                return Err(RuntimeError::new("Invalid positive import outer"));
            }
            parent => {
                let parent_row = import_row_from_index(parent, "Import outer index out of range")?;
                let parent = unsafe { ptr::read_unaligned(import_slot(linker, parent_row)?) };
                if parent.source_linker.is_null() {
                    return Ok(None);
                }
            }
        }
    }
    if verify_import(access, linker, row)? {
        return Ok(Some(ptr::null_mut()));
    }
    let verified = unsafe { ptr::read_unaligned(import_slot(linker, row)?) };
    if verified.source_index == -1 {
        return Ok(None);
    }
    let export_row = usize::try_from(verified.source_index)
        .map_err(|_| RuntimeError::new("Invalid resolved import index"))?;
    unsafe { create_export(access, verified.source_linker.cast(), export_row) }.map(Some)
}

unsafe fn resolve_import_body(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    import: &FObjectImport,
) -> RuntimeResult<*mut UObject> {
    let class_package = access.name_text(i32::from_ne_bytes(import.class_package))?;
    let class_name = access.name_text(i32::from_ne_bytes(import.class_name))?;
    let object_name = access.name_text(i32::from_ne_bytes(import.object_name))?;
    let package_core = class_package.iter().copied().eq("Core".chars().map(u32::from));
    let class_is_package = class_name.iter().copied().eq("Package".chars().map(u32::from));
    let display_name = native_name_for(&object_name);
    if package_core && class_is_package && import.package_index == 0 {
        if !import.source_linker.is_null() {
            let root = unsafe { (*import.source_linker.cast::<OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
            if !root.is_null() { return Ok(root); }
        }
        let (root, _) = package_root_and_linker(access, &object_name)?;
        return Ok(root);
    }
    let outer = if import.package_index == 0 {
        ptr::null_mut()
    } else {
        unsafe { resolve_index(access, linker, import.package_index)? }
    };
    if outer.is_null() { return Err(RuntimeError::new("Unresolved package import outer").with_context(&display_name)); }
    let class = if package_core {
        access.native_class(&native_name_for(&class_name))?
    } else {
        let (package_root, source) = package_root_and_linker(access, &class_package)?;
        let class_name_index = access.intern(&class_name, crate::name_owner::FindName::Add)?;
        let class_meta = access.native_class("Class")?;
        let mut class = unsafe { access.find_object(class_meta, package_root, class_name_index, true) };
        if class.is_null() {
            let class_meta_name = access.intern_str("Class", crate::name_owner::FindName::Find)?;
            let core_name = access.intern_str("Core", crate::name_owner::FindName::Find)?;
            let package_name = i32::from_ne_bytes(unsafe { (*package_root).name });
            let row = find_export_row(source, class_meta_name, core_name, class_name_index, package_name)?
                .ok_or_else(|| RuntimeError::new("Unresolved import class")
                    .with_context(&format!("{}.{}", native_name_for(&class_package), native_name_for(&class_name))))?;
            class = unsafe { create_class_export(access, source, row)? };
        }
        class.cast::<hp2_abi::engine::generated::UClass>()
    };
    let name_index = i32::from_ne_bytes(import.object_name);
    let object = unsafe { access.find_object(class, outer, name_index, true) };
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let required = constants.public_flag | constants.native | constants.transient;
    if !object.is_null() && unsafe { (*object).object_flags & required == required } {
        return Ok(object);
    }
    let safe_replace = unsafe { (*class.cast::<hp2_abi::engine::generated::UClass>()).class_flags }
        & constants.safe_replace != 0;
    let package = package_for_import(linker, import)?;
    if !package.is_null() {
        let object = unsafe { access.find_object(class, package, name_index, true) };
        if !object.is_null() && unsafe { (*object).object_flags & required == required } {
            return Ok(object);
        }
    }
    if safe_replace { return Ok(ptr::null_mut()); }
    let object = find_any_import_object(access, class.cast(), name_index)?;
    if !object.is_null() { return Ok(object); }
    Err(RuntimeError::new("Unresolved package import").with_context(&display_name))
}
pub(crate) fn native_name_for(text: &[u32]) -> String {
    text.iter().filter_map(|unit| char::from_u32(*unit)).collect()
}
pub(crate) fn linker_for_root(access: CoreAccess<'_>, root: *mut UObject) -> Option<*mut UObject> {
    let count = unsafe { (*access.loader_state()).linkers.len() };
    for index in 0..count {
        let linker = unsafe { (&(*access.loader_state()).linkers)[index] };
        if linker.is_null() { continue; }
        let owner = linker.cast::<OwnedLinkerLoad>();
        if unsafe { (*owner).initialized && (*owner).raw.base.linker_root.cast::<UObject>() == root } {
            if unsafe { (*owner).raw.base.success != 0 || (*(*owner).tail.as_ptr()).loading } {
                return Some(linker);
            }
        }
    }
    None
}

fn package_root_and_linker(access: CoreAccess<'_>, name: &[u32]) -> RuntimeResult<(*mut UObject, *mut UObject)> {
    package_root_and_linker_flags(
        access,
        name,
        hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.no_verify,
    )
}

pub(crate) fn package_root_and_linker_flags(
    access: CoreAccess<'_>,
    name: &[u32],
    flags: u32,
) -> RuntimeResult<(*mut UObject, *mut UObject)> {
    let package_name = native_name_for(name);
    let package_name_index = access.intern(name, crate::name_owner::FindName::Add)?;
    let root = find_package_root(access, package_name_index)?;
    if !root.is_null() {
        if let Some(linker) = linker_for_root(access, root) { return Ok((root, linker)); }
    }
    let linker = unsafe { get_package_linker(access, Path::new(&package_name), flags, ptr::null_mut(), None) }
        .map_err(|error| error.with_context("UObject::GetPackageLinker"))?;
    let root = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
    if root.is_null() { return Err(RuntimeError::new("Package linker has no root").with_context(&package_name)); }
    Ok((root, linker))
}


pub(crate) fn export_outer_name(linker: *mut UObject, index: i32) -> RuntimeResult<i32> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    if index == 0 {
        let root = unsafe { (*owner).raw.base.linker_root.cast::<UObject>() };
        if root.is_null() { return Err(RuntimeError::new("Linker has no package root")); }
        return Ok(i32::from_ne_bytes(unsafe { (*root).name }));
    }
    if index < 0 {
        let row = import_row_from_index(index, "Import outer index out of range")?;
        let map = unsafe { &(*owner).raw.base.import_map };
        if row >= map.num.max(0) as usize || map.data.is_null() {
            return Err(RuntimeError::new("Import outer index out of range"));
        }
        return Ok(i32::from_ne_bytes(
            unsafe { ptr::read_unaligned(map.data.cast::<FObjectImport>().add(row)) }.object_name,
        ));
    }
    let row = usize::try_from(index - 1).map_err(|_| RuntimeError::new("Export outer index out of range"))?;
    let map = unsafe { &(*owner).raw.base.export_map };
    if row >= map.num.max(0) as usize || map.data.is_null() {
        return Err(RuntimeError::new("Export outer index out of range"));
    }
    Ok(i32::from_ne_bytes(unsafe { ptr::read_unaligned(map.data.cast::<FObjectExport>().add(row)) }.object_name))
}

fn find_export_row(
    linker: *mut UObject, class_name: i32, class_package: i32, object_name: i32, outer_name: i32,
) -> RuntimeResult<Option<usize>> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let map = unsafe { (*owner).raw.base.export_map };
    if map.num > 0 && map.data.is_null() {
        return Err(RuntimeError::new("Export table has no backing storage"));
    }
    for row in 0..map.num.max(0) as usize {
        let export = unsafe { ptr::read_unaligned(map.data.cast::<FObjectExport>().add(row)) };
        if i32::from_ne_bytes(export.object_name) != object_name { continue; }
        let (candidate_class, candidate_package) = export_class_key(linker, export)?;
        if candidate_class != class_name || candidate_package != class_package { continue; }
        if export_outer_name(linker, export.package_index)? == outer_name { return Ok(Some(row)); }
    }
    Ok(None)
}

/// Creates and preloads one class export.
///
/// # Safety
/// `linker` is a live initialized owned linker and `row` is a valid export index.
unsafe fn create_class_export(access: CoreAccess<'_>, linker: *mut UObject, row: usize) -> RuntimeResult<*mut UObject> {
    let class = unsafe { create_export(access, linker, row)? };
    unsafe { preload(access, class)?; }
    Ok(class)
}

/// Creates one canonical export object, resolving its class layout before construction.
///
/// # Safety
/// `linker` is a live initialized owned linker and `row` is a valid export index.
pub(crate) unsafe fn create_export(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
) -> RuntimeResult<*mut UObject> {
    let export_ptr = unsafe { export_slot(linker, row)? };
    let export = unsafe { ptr::read_unaligned(export_ptr) };
    let existing = export._object.cast::<UObject>();
    if !existing.is_null() { return Ok(existing); }
    let context_flags = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base._context_flags };
    if !crate::linker_runtime::export_matches_context(export.object_flags, context_flags) {
        return Ok(ptr::null_mut());
    }
    if unsafe { export_loading_state(linker, row)? } != 0 {
        return Err(RuntimeError::new("Cyclic export construction").with_context(&format!("export {row}")));
    }
    unsafe { set_export_loading_state(linker, row, 1)?; }
    let result = unsafe { create_export_body(access, linker, row, export) };
    let reset = unsafe { set_export_loading_state(linker, row, 0) };
    match (result, reset) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(object), Ok(())) => Ok(object),
    }
}

/// # Safety
/// `linker` is a live initialized owned linker and `row` is a valid export index.
unsafe fn export_slot(linker: *mut UObject, row: usize) -> RuntimeResult<*mut FObjectExport> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad")); }
    let map = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.export_map };
    if row >= map.num.max(0) as usize || map.data.is_null() {
        return Err(RuntimeError::new("Export index out of range"));
    }
    Ok(unsafe { map.data.cast::<FObjectExport>().add(row) })
}

/// # Safety
/// `linker` is a live initialized owned linker and `row` is a valid export index.
unsafe fn export_loading_state(linker: *mut UObject, row: usize) -> RuntimeResult<u8> {
    let tail = unsafe { &*tail_ptr(linker)? };
    tail.export_loading.get(row).copied()
        .ok_or_else(|| RuntimeError::new("Export state index out of range"))
}

/// # Safety
/// `linker` is a live initialized owned linker and `row` is a valid export index.
unsafe fn set_export_loading_state(linker: *mut UObject, row: usize, state: u8) -> RuntimeResult<()> {
    let tail = unsafe { &mut *tail_mut_ptr(linker)? };
    let slot = tail.export_loading.get_mut(row)
        .ok_or_else(|| RuntimeError::new("Export state index out of range"))?;
    *slot = state;
    Ok(())
}

/// Resolves class and outer objects, then publishes one export instance.
///
/// # Safety
/// `linker` and all resolved exports remain live canonical objects on the engine thread.
unsafe fn create_export_body(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
    export: FObjectExport,
) -> RuntimeResult<*mut UObject> {
    let linker_index = i32::try_from(row).map_err(|_| RuntimeError::new("Export index exceeds linker index range"))?;
    let name_index = i32::from_ne_bytes(export.object_name);
    let owner = linker.cast::<OwnedLinkerLoad>();
    let root = unsafe { (*owner).raw.base.linker_root.cast::<UObject>() };
    let class = if export.class_index == 0 { access.native_class("Class")?.cast::<UObject>() }
        else { unsafe { resolve_index(access, linker, export.class_index)? } };
    unsafe { preload(access, class)?; }
    let export_ptr = unsafe { export_slot(linker, row)? };
    let existing = unsafe { ptr::read_unaligned(ptr::addr_of!((*export_ptr)._object)) }.cast::<UObject>();
    if !existing.is_null() { return Ok(existing); }
    let outer = if export.package_index == 0 { root }
        else { unsafe { resolve_index(access, linker, export.package_index)? } };
    let load = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let flags = (export.object_flags & load.load_mask) | load.need_load | load.need_post_load;
    let published = unsafe { access.find_object(class.cast(), outer, name_index, true) };
    // Source CreateExport reuses a by-name hit only for class exports whose
    // found object is a registered native class (stock: the export's class
    // is `Class` and the hit carries CLASS_Native). Every other unbound hit
    // is stale state from a prior load of this package — ResetLoaders
    // unlinks exports when it destroys a linker, so a stale actor reads
    // `_linker == null` exactly like an unbound native class — and must be
    // re-created, not reused.
    let published_reusable = !published.is_null() && {
        let published_linker = unsafe { (*published)._linker }.cast::<UObject>();
        if published_linker.is_null() {
            class == access.native_class("Class")?.cast::<UObject>()
                && unsafe { (*published).object_flags } & crate::object_lifecycle::RF_NATIVE != 0
        } else {
            registered_linker(access, published_linker).is_ok()
        }
    };
    let object = if published_reusable {
        unsafe { (*published).object_flags |= flags; }
        published
    } else {
        unsafe { access.construct_object(class.cast(), outer, name_index, flags, ptr::null_mut())? }
    };
    unsafe {
        (*object)._linker = linker.cast();
        (*object)._linker_index = linker_index;
        let export_ptr = export_slot(linker, row)?;
        ptr::write_unaligned(ptr::addr_of_mut!((*export_ptr)._object), object.cast());
    }
    unsafe { (*access.loader_state()).loaded_objects.push(object); }
    if class == access.native_class("Class")?.cast::<UObject>() && export.super_index != 0 {
        let super_class = unsafe { resolve_index(access, linker, export.super_index)? };
        if super_class.is_null() { return Err(RuntimeError::new("Class export has a null superclass")); }
        unsafe { (*object.cast::<hp2_abi::engine::generated::UField>()).super_field = super_class.cast(); }
        unsafe { access.inherit_class_metadata(object.cast(), super_class.cast())?; }
    }
    Ok(object)
}


struct LinkerMapping<'a> { access: CoreAccess<'a>, linker: *mut UObject }
impl ArchiveMapping for LinkerMapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        unsafe { resolve_index(self.access, self.linker, index) }
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        let owner = self.linker.cast::<OwnedLinkerLoad>();
        let raw = unsafe { &(*owner).raw };
        for row in 0..raw.base.export_map.num.max(0) as usize {
            let export = unsafe { ptr::read_unaligned(raw.base.export_map.data.cast::<FObjectExport>().add(row)) };
            if export._object.cast::<UObject>() == object { return Ok(i32::try_from(row + 1).map_err(|_| RuntimeError::new("Export index exceeds package index range"))?); }
        }
        for row in 0..raw.base.import_map.num.max(0) as usize {
            let import = unsafe { ptr::read_unaligned(raw.base.import_map.data.cast::<FObjectImport>().add(row)) };
            if import.x_object.cast::<UObject>() == object { return Ok(-i32::try_from(row + 1).map_err(|_| RuntimeError::new("Import index exceeds package index range"))?); }
        }
        Err(RuntimeError::new("Object is not mapped by linker"))
    }
    fn load_name(&self, local_index: i32) -> RuntimeResult<i32> {
        let owner = self.linker.cast::<OwnedLinkerLoad>();
        let raw = unsafe { &(*owner).raw.base.name_map };
        let row = usize::try_from(local_index).map_err(|_| RuntimeError::new("Package name index out of range"))?;
        if row >= raw.num.max(0) as usize { return Err(RuntimeError::new("Package name index out of range")); }
        Ok(unsafe { ptr::read_unaligned(raw.data.cast::<FName>().add(row)) }.index)
    }
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32> {
        let owner = self.linker.cast::<OwnedLinkerLoad>();
        let raw = unsafe { &(*owner).raw.base.name_map };
        for row in 0..raw.num.max(0) as usize {
            if unsafe { ptr::read_unaligned(raw.data.cast::<FName>().add(row)) }.index == global_index {
                return i32::try_from(row).map_err(|_| RuntimeError::new("Package name index exceeds archive range"));
            }
        }
        Err(RuntimeError::new("Name is not mapped by linker"))
    }
    fn visit_object(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        if object_slot.is_null() { return Err(RuntimeError::new("Null linker object reference slot")); }
        let object = unsafe { ptr::read_unaligned(object_slot) };
        if object.is_null() { return Ok(()); }
        let _ = self.save_object(object)?;
        Ok(())
    }
    fn visit_name(&self, name_index: i32) -> RuntimeResult<()> { self.save_name(name_index).map(|_| ()) }
    fn linker_identity(&self) -> Option<*mut UObject> { Some(self.linker) }
}

#[derive(Clone, Copy)]
pub(crate) struct LazyArrayBinding {
    pub(crate) linker: *mut UObject,
    pub(crate) row: usize,
    pub(crate) element_width: usize,
    pub(crate) body: crate::lazy_array::LazyArrayBody,
    pub(crate) version: u16,
}

fn linker_archive_pointer(linker: *mut UObject) -> *mut c_void {
    unsafe {
        linker.cast::<u8>()
            .add(ULinkerLoad::FARCHIVE_OFFSET)
            .cast()
    }
}

fn registered_linker(access: CoreAccess<'_>, linker: *mut UObject) -> RuntimeResult<()> {
    if linker.is_null() {
        return Err(RuntimeError::new("Null lazy-array linker"));
    }
    let state = unsafe { &*access.loader_state() };
    if !state.linkers.contains(&linker) {
        return Err(RuntimeError::new("Lazy-array linker is not owned by this Core"));
    }
    Ok(())
}

fn linker_for_archive(access: CoreAccess<'_>, archive: *mut c_void) -> RuntimeResult<*mut UObject> {
    if archive.is_null() {
        return Err(RuntimeError::new("Lazy array has no saved archive"));
    }
    let state = unsafe { &*access.loader_state() };
    for &linker in &state.linkers {
        if !linker.is_null() && linker_archive_pointer(linker) == archive {
            return Ok(linker);
        }
    }
    Err(RuntimeError::new("Saved lazy archive is not owned by this Core"))
}

fn export_payload(
    linker: *mut UObject,
    row: usize,
) -> RuntimeResult<(Rc<Vec<u8>>, usize, usize, u16, u16)> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let tail = unsafe { &*tail_ptr(linker)? };
    let export = tail.exports.get(row).ok_or_else(|| RuntimeError::new("Lazy export index out of range"))?;
    if export.serial_size < 0 || export.serial_offset < 0 {
        return Err(RuntimeError::new("Invalid lazy export serial range"));
    }
    let start = usize::try_from(export.serial_offset).map_err(|_| RuntimeError::new("Invalid lazy export offset"))?;
    let size = usize::try_from(export.serial_size).map_err(|_| RuntimeError::new("Invalid lazy export size"))?;
    let end = start.checked_add(size).ok_or_else(|| RuntimeError::new("Lazy export range overflow"))?;
    if end > tail.bytes.len() {
        return Err(RuntimeError::new("Lazy export serial range outside package"));
    }
    let version = unsafe { (*owner).raw.base.summary.file_version as u16 };
    let licensee = (unsafe { (*owner).raw.base.summary.file_version as u32 } >> 16) as u16;
    Ok((tail.bytes.clone(), start, end, version, licensee))
}

unsafe fn owner_export_identity(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<(*mut UObject, usize)> {
    if object.is_null() {
        return Err(RuntimeError::new("Cannot attach a lazy array on a null object"));
    }
    let linker = unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker)) }.cast::<UObject>();
    let index = unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker_index)) };
    if linker.is_null() || index < 0 {
        return Err(RuntimeError::new("Lazy-array owner has no canonical linker export"));
    }
    registered_linker(access, linker)?;
    let row = usize::try_from(index).map_err(|_| RuntimeError::new("Invalid lazy export index"))?;
    let tail = unsafe { &*tail_ptr(linker)? };
    let export = tail.exports.get(row).ok_or_else(|| RuntimeError::new("Lazy owner export index out of range"))?;
    if export._object.cast::<UObject>() != object {
        return Err(RuntimeError::new("Lazy-array owner does not match its linker export"));
    }
    Ok((linker, row))
}

fn validate_lazy_archive(
    linker: *mut UObject,
    row: usize,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<(usize, i32)> {
    if !archive.is_loading() {
        return Err(RuntimeError::new("Lazy attachment requires a loading archive"));
    }
    if archive.is_trans() {
        return Err(RuntimeError::new("Transaction archives do not own lazy linker attachments"));
    }
    if archive.linker_identity() != Some(linker) {
        return Err(RuntimeError::new("Lazy attachment archive uses a different linker mapping"));
    }
    let (bytes, start, end, _, _) = export_payload(linker, row)?;
    let (archive_bytes, range) = archive.shared_source_range()
        .ok_or_else(|| RuntimeError::new("Lazy attachment requires shared linker-owned source bytes"))?;
    if !Rc::ptr_eq(&bytes, &archive_bytes) || range.start != start || range.end != end {
        return Err(RuntimeError::new("Lazy attachment source is not the owner's bounded export"));
    }
    let relative = archive.tell()?;
    let absolute = start.checked_add(relative)
        .filter(|position| *position <= end)
        .ok_or_else(|| RuntimeError::new("Lazy attachment cursor is outside its export"))?;
    let saved = i32::try_from(absolute).map_err(|_| RuntimeError::new("Lazy saved position exceeds INT"))?;
    if saved <= 0 {
        return Err(RuntimeError::new("Lazy saved position is not a positive package offset"));
    }
    Ok((absolute, saved))
}

fn publish_lazy_list(linker: *mut UObject, tail: &mut LoaderTail) -> RuntimeResult<()> {
    let count = i32::try_from(tail.lazy_loaders.len())
        .map_err(|_| RuntimeError::new("Lazy-loader count exceeds TArray range"))?;
    let capacity = i32::try_from(tail.lazy_loaders.capacity()).unwrap_or(i32::MAX);
    let data = if capacity == 0 { ptr::null_mut() } else { tail.lazy_loaders.as_mut_ptr().cast() };
    unsafe {
        (*linker.cast::<OwnedLinkerLoad>()).raw.lazy_loaders = TArrayRaw { data, num: count, max: capacity };
    }
    Ok(())
}

/// # Safety
/// All object, archive, and descriptor pointers are live for this synchronous attach.
pub(crate) unsafe fn attach_lazy_array(
    access: CoreAccess<'_>,
    object: *mut UObject,
    slots: crate::lazy_array::LazyArraySlots,
    archive: &ObjectArchive<'_>,
    element_width: usize,
    body: crate::lazy_array::LazyArrayBody,
) -> RuntimeResult<()> {
    if element_width == 0 {
        return Err(RuntimeError::new("Lazy-array element width is zero"));
    }
    let (linker, row) = unsafe { owner_export_identity(access, object)? };
    let (position, saved_position) = validate_lazy_archive(linker, row, archive)?;
    let saved_archive = unsafe { ptr::read_unaligned(slots.saved_archive) };
    let current_position = unsafe { ptr::read_unaligned(slots.saved_position) };
    if !saved_archive.is_null() || current_position != 0 {
        return Err(RuntimeError::new("Lazy array is already attached or has stale saved state"));
    }
    let owner = linker.cast::<OwnedLinkerLoad>();
    let tail = unsafe { &mut *tail_mut_ptr(linker)? };
    if tail.lazy_loaders.contains(&slots.loader) {
        return Err(RuntimeError::new("Lazy loader is already present in its linker list"));
    }
    if tail.lazy_loaders.len() >= i32::MAX as usize || tail.lazy_entries.len() >= i32::MAX as usize {
        return Err(RuntimeError::new("Lazy-loader count exceeds TArray range"));
    }
    tail.lazy_entries.try_reserve(1).map_err(|_| RuntimeError::new("Out of memory growing lazy-array metadata"))?;
    tail.lazy_loaders.try_reserve(1).map_err(|_| RuntimeError::new("Out of memory growing lazy-loader list"))?;
    tail.lazy_loaders.push(slots.loader);
    tail.lazy_entries.push(LazyArrayEntry {
        loader: slots.loader, array: slots.array, saved_archive: slots.saved_archive,
        saved_position: slots.saved_position, row, element_width, body,
    });
    if let Err(error) = publish_lazy_list(linker, tail) {
        tail.lazy_loaders.pop();
        tail.lazy_entries.pop();
        return Err(error);
    }
    tail.archive_position = position;
    let real_archive = unsafe {
        ptr::addr_of_mut!((*owner).raw).cast::<u8>()
            .add(ULinkerLoad::FARCHIVE_OFFSET)
            .cast::<c_void>()
    };
    unsafe {
        ptr::write_unaligned(slots.saved_archive, real_archive);
        ptr::write_unaligned(slots.saved_position, saved_position);
    }
    Ok(())
}

/// Resolves the loader-owned metadata for one still-attached array.
///
/// # Safety
/// The descriptor slots point into a live canonical lazy-array record.
pub(crate) unsafe fn lazy_array_binding(
    access: CoreAccess<'_>,
    slots: crate::lazy_array::LazyArraySlots,
) -> RuntimeResult<Option<LazyArrayBinding>> {
    let saved_archive = unsafe { ptr::read_unaligned(slots.saved_archive) };
    let saved_position = unsafe { ptr::read_unaligned(slots.saved_position) };
    if saved_archive.is_null() {
        if saved_position != 0 {
            return Err(RuntimeError::new("Lazy array has a position without a saved archive"));
        }
        let state = unsafe { &*access.loader_state() };
        for &linker in &state.linkers {
            if linker.is_null() { continue; }
            let tail = unsafe { &*tail_ptr(linker)? };
            if tail.lazy_loaders.contains(&slots.loader) {
                return Err(RuntimeError::new("Lazy loader is listed without a saved archive"));
            }
        }
        return Ok(None);
    }
    let linker = linker_for_archive(access, saved_archive)?;
    let tail = unsafe { &*tail_ptr(linker)? };
    let entry = tail.lazy_entries.iter().find(|entry| entry.loader == slots.loader)
        .ok_or_else(|| RuntimeError::new("Saved lazy loader is absent from its linker list"))?;
    if entry.array != slots.array || entry.saved_archive != slots.saved_archive
        || entry.saved_position != slots.saved_position
    {
        return Err(RuntimeError::new("Lazy-array descriptor differs from its linker registration"));
    }
    let (_, _, _, version, _) = export_payload(linker, entry.row)?;
    Ok(Some(LazyArrayBinding {
        linker, row: entry.row, element_width: entry.element_width, body: entry.body,
        version,
    }))
}

/// Removes exactly one registered loader and clears its original saved slots.
///
/// # Safety
/// The descriptor slots point into a live canonical lazy-array record.
pub(crate) unsafe fn detach_lazy_array(
    access: CoreAccess<'_>,
    slots: crate::lazy_array::LazyArraySlots,
) -> RuntimeResult<()> {
    let saved_archive = unsafe { ptr::read_unaligned(slots.saved_archive) };
    let saved_position = unsafe { ptr::read_unaligned(slots.saved_position) };
    if saved_archive.is_null() {
        if saved_position != 0 {
            return Err(RuntimeError::new("Lazy array has a position without a saved archive"));
        }
        let state = unsafe { &*access.loader_state() };
        for &linker in &state.linkers {
            if linker.is_null() { continue; }
            if unsafe { (&*tail_ptr(linker)?).lazy_loaders.contains(&slots.loader) } {
                return Err(RuntimeError::new("Lazy loader is listed without a saved archive"));
            }
        }
        return Ok(());
    }
    let linker = linker_for_archive(access, saved_archive)?;
    let tail = unsafe { &mut *tail_mut_ptr(linker)? };
    let mut found = None;
    for (index, loader) in tail.lazy_loaders.iter().enumerate() {
        if *loader == slots.loader {
            if found.is_some() {
                return Err(RuntimeError::new("Lazy-loader list removal was not unique"));
            }
            found = Some(index);
        }
    }
    let index = found.ok_or_else(|| RuntimeError::new("Lazy-loader list removal was not unique"))?;
    let entry = tail.lazy_entries.get(index)
        .ok_or_else(|| RuntimeError::new("Lazy-loader metadata list is inconsistent"))?;
    if entry.array != slots.array || entry.saved_archive != slots.saved_archive
        || entry.saved_position != slots.saved_position
    {
        return Err(RuntimeError::new("Lazy-array descriptor differs from its linker registration"));
    }
    tail.lazy_loaders.remove(index);
    tail.lazy_entries.remove(index);
    if tail.lazy_loaders.is_empty() {
        tail.lazy_loaders = Vec::new();
        tail.lazy_entries = Vec::new();
    }
    publish_lazy_list(linker, tail)?;
    unsafe {
        ptr::write_unaligned(slots.saved_archive, ptr::null_mut());
        ptr::write_unaligned(slots.saved_position, 0);
    }
    let _ = saved_position;
    Ok(())
}

unsafe fn clear_lazy_loader_fields(loader: *mut c_void) -> RuntimeResult<()> {
    if loader.is_null() { return Err(RuntimeError::new("Null lazy-loader identity during final detach")); }
    let loader = loader.cast::<FLazyLoader>();
    unsafe {
        ptr::write_unaligned(ptr::addr_of_mut!((*loader).saved_ar), ptr::null_mut());
        ptr::write_unaligned(ptr::addr_of_mut!((*loader).saved_pos), 0);
    }
    Ok(())
}


/// Returns the shared linker file cursor, which may be in another export.
pub(crate) unsafe fn lazy_archive_position(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
) -> RuntimeResult<usize> {
    registered_linker(access, linker)?;
    let tail = unsafe { &*tail_ptr(linker)? };
    if row >= tail.exports.len() {
        return Err(RuntimeError::new("Lazy source export index out of range"));
    }
    if tail.archive_position > tail.bytes.len() {
        return Err(RuntimeError::new("Lazy linker cursor outside package"));
    }
    Ok(tail.archive_position)
}

/// Sets the shared file cursor. Deferred body reads remain export-bounded in
/// `serialize_lazy_array_source`; restoring another export's cursor is valid.
pub(crate) unsafe fn set_lazy_archive_position(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
    position: usize,
) -> RuntimeResult<()> {
    registered_linker(access, linker)?;
    let tail = unsafe { &mut *tail_mut_ptr(linker)? };
    if row >= tail.exports.len() {
        return Err(RuntimeError::new("Lazy source export index out of range"));
    }
    if position > tail.bytes.len() {
        return Err(RuntimeError::new("Lazy seek outside package"));
    }
    tail.archive_position = position;
    Ok(())
}

/// Serializes one deferred body directly from its shared bounded linker export.
///
/// # Safety
/// The linker and array remain live through the typed body call; the callback
/// must not retain the temporary archive or source borrow.
pub(crate) unsafe fn serialize_lazy_array_source(
    access: CoreAccess<'_>,
    linker: *mut UObject,
    row: usize,
    position: usize,
    array: *mut c_void,
    body: crate::lazy_array::LazyArrayBody,
) -> RuntimeResult<()> {
    registered_linker(access, linker)?;
    let (bytes, start, end, version, licensee) = export_payload(linker, row)?;
    if position < start || position > end {
        return Err(RuntimeError::new("Lazy saved position is outside its export"));
    }
    let payload = crate::package_archive::ExportArchive::from_shared_range(bytes, start..end)?;
    let mapping = LinkerMapping { access, linker };
    let archive = ObjectArchive::loading(payload, ArchiveFlags::new(version, licensee, true), &mapping);
    archive.seek(position - start)?;
    let result = unsafe { body(access, array, &archive) };
    let cursor = archive.tell().and_then(|relative| {
        start.checked_add(relative).ok_or_else(|| RuntimeError::new("Lazy archive position overflow"))
    });
    if let Ok(cursor) = cursor {
        if registered_linker(access, linker).is_ok() {
            unsafe { (&mut *tail_mut_ptr(linker)?).archive_position = cursor; }
        }
    }
    match result {
        Err(error) => Err(error),
        Ok(()) => {
            let cursor = cursor?;
            unsafe { set_lazy_archive_position(access, linker, row, cursor)?; }
            Ok(())
        }
    }
}

/// Resolves one object export without retaining a table borrow through construction callbacks.
///
/// # Safety
/// `object` is a live canonical object with valid linker and export identity.
pub(crate) unsafe fn preload(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Cannot preload a null object")); }
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let mut flags = unsafe { (*object).object_flags };
    if flags & constants.need_load == 0 || flags & constants.preloading != 0 { return Ok(()); }
    let structure = access.native_class("Struct")?;
    if unsafe { crate::object_model::is_a(object.cast(), structure.cast()) } {
        let superclass = unsafe { (*object.cast::<hp2_abi::engine::generated::UField>()).super_field.cast::<UObject>() };
        if !superclass.is_null() { unsafe { preload(access, superclass)?; } }
    }
    flags = unsafe { (*object).object_flags };
    if flags & constants.need_load == 0 || flags & constants.preloading != 0 { return Ok(()); }
    let linker = unsafe { (*object)._linker.cast::<UObject>() };
    if linker.is_null() { return Err(RuntimeError::new("Object requiring load has no linker")); }
    let object_name = native_name_for(&access.name_text(i32::from_ne_bytes(unsafe { (*object).name }))?);
    let path = unsafe { (&*tail_ptr(linker)?).path.clone() };
    let context = format!("{}: {object_name}", path.as_deref().map(Path::display).map(|v| v.to_string()).unwrap_or_default());
    unsafe { (*object).object_flags = (flags & !constants.need_load) | constants.preloading; }
    let result = unsafe { preload_body(access, linker, object) }
        .map_err(|error| error.with_context(&context));
    if result.is_ok() { unsafe { (*object).object_flags &= !constants.preloading; } }
    result
}

unsafe fn preload_body(access: CoreAccess<'_>, linker: *mut UObject, object: *mut UObject) -> RuntimeResult<()> {
    let row = unsafe { (*object)._linker_index };
    if row < 0 { return Err(RuntimeError::new("Invalid export index during preload")); }
    let owner = linker.cast::<OwnedLinkerLoad>();
    let (bytes, serial_size, serial_offset, version, licensee) = unsafe {
        let tail = unsafe { &*tail_ptr(linker)? };
        let row = usize::try_from(row).map_err(|_| RuntimeError::new("Export index out of range during preload"))?;
        let export = *tail.exports.get(row).ok_or_else(|| RuntimeError::new("Export index out of range during preload"))?;
        (tail.bytes.clone(), export.serial_size, export.serial_offset,
            (*owner).raw.base.summary.file_version as u16,
            ((*owner).raw.base.summary.file_version as u32 >> 16) as u16)
    };
    if serial_size < 0 || serial_offset < 0 { return Err(RuntimeError::new("Invalid export serial range")); }
    let export = crate::package_archive::PackageExport {
        class_index: 0, super_index: 0, package_index: 0, object_name: 0,
        object_flags: 0, serial_size, serial_offset, offset: 0, end_offset: 0,
    };
    // The cursor validates both the seek and the end bound against source bytes.
    let payload = crate::package_archive::shared_export_archive(bytes, &export)?;
    let mapping = LinkerMapping { access, linker };
    let archive = ObjectArchive::loading(payload, ArchiveFlags::new(version, licensee, true), &mapping);
    unsafe { access.serialize_object(object, &archive)?; }
    let consumed = archive.tell()?;
    if consumed != serial_size as usize {
        return Err(RuntimeError::new("Export serial size mismatch").with_context(&format!("{consumed}/{}", serial_size)));
    }
    let row = usize::try_from(row).map_err(|_| RuntimeError::new("Export index out of range during preload"))?;
    let absolute_position = usize::try_from(serial_offset)
        .map_err(|_| RuntimeError::new("Invalid export serial offset"))?
        .checked_add(consumed)
        .ok_or_else(|| RuntimeError::new("Lazy linker cursor overflow"))?;
    unsafe { set_lazy_archive_position(access, linker, row, absolute_position)?; }
    Ok(())
}

/// Serializes the native prefix fields without duplicating their object/name maps.
///
/// # Safety
/// `linker` is a live canonical LinkerLoad object and the archive mapping is valid.
pub(crate) unsafe fn loader_serialize(
    access: CoreAccess<'_>, linker: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let (name_map, import_map, export_map) = unsafe {
        ((*owner).raw.base.name_map, (*owner).raw.base.import_map, (*owner).raw.base.export_map)
    };
    for (map, size) in [
        (import_map, std::mem::size_of::<FObjectImport>()),
        (export_map, std::mem::size_of::<FObjectExport>()),
    ] {
        let count = usize::try_from(map.num.max(0)).map_err(|_| RuntimeError::new("Negative linker table count"))?;
        let bytes = count.checked_mul(size).ok_or_else(|| RuntimeError::new("Linker table byte count overflow"))?;
        archive.count_bytes(bytes, bytes)?;
    }
    for row in 0..name_map.num.max(0) as usize {
        let slot = unsafe { ptr::addr_of_mut!((*name_map.data.cast::<FName>().add(row)).index) };
        unsafe { archive.serialize_name_slot(slot)?; }
    }
    unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*owner).raw.base.linker_root).cast())?; }
    for row in 0..export_map.num.max(0) as usize {
        let export = unsafe { export_map.data.cast::<FObjectExport>().add(row) };
        let name = unsafe { ptr::addr_of_mut!((*export).object_name).cast::<i32>() };
        unsafe { archive.serialize_name_slot(name)?; }
    }
    for row in 0..import_map.num.max(0) as usize {
        let import = unsafe { import_map.data.cast::<FObjectImport>().add(row) };
        unsafe {
            archive.serialize_object_slot(ptr::addr_of_mut!((*import).source_linker).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*import).class_package).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*import).class_name).cast())?;
        }
    }
    let _ = access;
    Ok(())
}

/// Visits canonical import/export references without holding table borrows over callbacks.
///
/// # Safety
/// `linker` is live, and the reference archive was created for the same owning Core access.
pub(crate) unsafe fn loader_visit_references(
    access: CoreAccess<'_>, linker: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    let owner = linker.cast::<OwnedLinkerLoad>();
    let (names, imports, exports) = unsafe {
        ((*owner).raw.base.name_map, (*owner).raw.base.import_map, (*owner).raw.base.export_map)
    };
    unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*owner).raw.base.linker_root).cast())?; }
    for row in 0..names.num.max(0) as usize {
        let slot = unsafe { ptr::addr_of_mut!((*names.data.cast::<FName>().add(row)).index) };
        unsafe { archive.serialize_name_slot(slot)?; }
    }
    for row in 0..exports.num.max(0) as usize {
        let export = unsafe { exports.data.cast::<FObjectExport>().add(row) };
        unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*export).object_name).cast())?; }
    }
    for row in 0..imports.num.max(0) as usize {
        let import = unsafe { imports.data.cast::<FObjectImport>().add(row) };
        unsafe {
            archive.serialize_object_slot(ptr::addr_of_mut!((*import).source_linker).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*import).class_package).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*import).class_name).cast())?;
        }
    }
    let _ = access;
    Ok(())
}
/// Tears down direct Rust-owned LinkerLoad state before releasing its owned tail.
///
/// # Safety
/// `linker` is a live owned LinkerLoad allocation already in its teardown phase.
pub(crate) unsafe fn loader_destroy(access: CoreAccess<'_>, linker: *mut UObject) -> RuntimeResult<()> {
    if linker.is_null() {
        return Err(RuntimeError::new("Null LinkerLoad allocation"));
    }
    let mut ops = CoreDestroyLoadOps { access };
    let policy = crate::linker_runtime::destroy_load_direct_with(&mut ops, linker as usize);
    let owner = linker.cast::<OwnedLinkerLoad>();
    let (still_registered, tail_release_safe) = unsafe {
        (
            (&(*access.loader_state()).linkers).contains(&linker),
            !(*owner).initialized || (*owner).tail_release_safe,
        )
    };
    if still_registered || !tail_release_safe {
        let message = if still_registered {
            "Linker tail retained because the linker remains registered"
        } else {
            "Linker tail retained because export detachment was not verified"
        };
        return match policy {
            Err(error) => Err(error.with_context(message)),
            Ok(()) => Err(RuntimeError::new(message)),
        };
    }
    let tail = unsafe { destroy_tail(linker) };
    policy.and(tail)
}

struct CoreDestroyLoadOps<'a> {
    access: CoreAccess<'a>,
}

impl package_flow::ResetLoadersOps for CoreDestroyLoadOps<'_> {
    type Error = RuntimeError;
    fn loader_count(&self) -> RuntimeResult<i32> {
        i32::try_from(unsafe { (*self.access.loader_state()).linkers.len() })
            .map_err(|_| RuntimeError::new("Core loader count exceeds INT"))
    }
    fn loader_at(&self, index: i32) -> RuntimeResult<usize> {
        unsafe { &(*self.access.loader_state()).linkers }.get(index as usize)
            .copied().map(|linker| linker as usize)
            .ok_or_else(|| RuntimeError::new("Core loader index is out of range"))
    }
    fn loader_root(&self, linker: usize) -> RuntimeResult<usize> {
        Ok(unsafe { (*(linker as *mut OwnedLinkerLoad)).raw.base.linker_root } as usize)
    }
    fn export_count(&self, linker: usize) -> RuntimeResult<usize> {
        let mut ops = CoreDestroyLoadOps { access: self.access };
        let count = crate::linker_runtime::DestroyLoadOps::export_count(&mut ops, linker)?;
        usize::try_from(count).map_err(|_| RuntimeError::new("Negative linker export count"))
    }
    fn export_object(&self, linker: usize, index: usize) -> RuntimeResult<usize> {
        let mut ops = CoreDestroyLoadOps { access: self.access };
        crate::linker_runtime::DestroyLoadOps::export_object(&mut ops, linker, index as i32)
    }
    fn object_class_flags(&self, object: usize) -> RuntimeResult<usize> {
        let class = unsafe { (*(object as *mut UObject)).class.cast::<UClass>() };
        Ok(unsafe { (*class).class_flags } as usize)
    }
    fn detach_export(&self, linker: usize, index: usize) -> RuntimeResult<()> {
        let mut ops = CoreDestroyLoadOps { access: self.access };
        crate::linker_runtime::DestroyLoadOps::detach_export(&mut ops, linker, index as i32)
    }
    fn detach_lazy(&self, linker: usize) -> RuntimeResult<()> {
        let mut ops = CoreDestroyLoadOps { access: self.access };
        crate::linker_runtime::DestroyLoadOps::detach_all_lazy(&mut ops, linker)
    }
    fn delete_loader(&self, linker: usize) -> RuntimeResult<()> {
        unsafe { self.access.delete_object(linker as *mut UObject) }
    }
}

impl crate::linker_runtime::DestroyLoadOps for CoreDestroyLoadOps<'_> {
    type Error = RuntimeError;

    fn unloading_log(&mut self, linker: usize) -> RuntimeResult<()> {
        let linker = linker as *mut UObject;
        let owner = linker.cast::<OwnedLinkerLoad>();
        let root = unsafe {
            let root = (*owner).raw.base.linker_root.cast::<UObject>();
            if root.is_null() { (*linker).outer.cast::<UObject>() } else { root }
        };
        if root.is_null() {
            return Err(RuntimeError::new("LinkerLoad has no package root"));
        }
        let root_name = self.access.name_text(i32::from_ne_bytes(unsafe { (*root).name }))?;
        let class = unsafe { (*root).class.cast::<UObject>() };
        if class.is_null() {
            return Err(RuntimeError::new("LinkerLoad package root has no class"));
        }
        let class_name = self.access.name_text(i32::from_ne_bytes(unsafe { (*class).name }))?;
        let mut message = "Unloading: ".chars().map(u32::from).collect::<Vec<_>>();
        message.extend(class_name);
        message.push(u32::from(' '));
        message.extend(root_name);
        let category = self.access.intern_str("Log", crate::name_owner::FindName::Add)?;
        self.access.emit_log(category, &message)
    }

    fn detach_all_lazy(&mut self, linker: usize) -> RuntimeResult<()> {
        let linker = linker as *mut UObject;
        let mut first_error = registered_linker(self.access, linker).err();
        let mut allow_load = first_error.is_none();
        let mut index = 0;
        loop {
            // Keep the native list published until bulk detachment completes.
            // Copy each descriptor before callbacks, which may change the list.
            let (entry, loader, header_consistent) = unsafe {
                let owner = linker.cast::<OwnedLinkerLoad>();
                let tail = &*tail_ptr(linker)?;
                let header = (*owner).raw.lazy_loaders;
                let count = i32::try_from(tail.lazy_loaders.len()).unwrap_or(i32::MAX);
                let capacity = i32::try_from(tail.lazy_loaders.capacity()).unwrap_or(i32::MAX);
                let data = if capacity == 0 { ptr::null_mut() } else { tail.lazy_loaders.as_ptr() as *mut c_void };
                (
                    tail.lazy_entries.get(index).copied(),
                    tail.lazy_loaders.get(index).copied(),
                    tail.lazy_loaders.len() == tail.lazy_entries.len()
                        && header.num == count && header.max == capacity && header.data.cast::<c_void>() == data,
                )
            };
            if !header_consistent {
                if first_error.is_none() {
                    first_error = Some(RuntimeError::new("Linker lazy-loader header differs from its owned list"));
                }
                allow_load = false;
            }
            let Some(entry) = entry else {
                let Some(loader) = loader else { break; };
                if first_error.is_none() {
                    first_error = Some(RuntimeError::new("Lazy-loader list has an unregistered identity"));
                }
                if let Err(error) = unsafe { clear_lazy_loader_fields(loader) } {
                    if first_error.is_none() { first_error = Some(error); }
                }
                index += 1;
                continue;
            };
            let valid_saved_slots = !entry.saved_archive.is_null() && !entry.saved_position.is_null();
            let saved_archive = if valid_saved_slots {
                unsafe { ptr::read_unaligned(entry.saved_archive) }
            } else {
                ptr::null_mut()
            };
            if loader != Some(entry.loader) || !valid_saved_slots || saved_archive != linker_archive_pointer(linker) {
                if first_error.is_none() {
                    first_error = Some(RuntimeError::new("Lazy-loader registration has inconsistent identity or saved slots"));
                }
                allow_load = false;
            }
            if allow_load && unsafe { ptr::read_unaligned(entry.saved_position) } > 0 {
                let slots = crate::lazy_array::LazyArraySlots {
                    array: entry.array, loader: entry.loader,
                    saved_archive: entry.saved_archive, saved_position: entry.saved_position,
                };
                if let Err(error) = unsafe { crate::lazy_array::lazy_array_load(self.access, slots) } {
                    if first_error.is_none() { first_error = Some(error); }
                    allow_load = false;
                }
            }
            // A callback may already have detached this entry. Do not skip the
            // replacement at this index, or touch an entry after its removal.
            let still_current = unsafe {
                (&*tail_ptr(linker)?).lazy_entries.get(index)
                    .is_some_and(|current| current.loader == entry.loader)
            };
            if !still_current { continue; }
            if valid_saved_slots {
                unsafe {
                    ptr::write_unaligned(entry.saved_archive, ptr::null_mut());
                    ptr::write_unaligned(entry.saved_position, 0);
                }
            } else if let Err(error) = unsafe { clear_lazy_loader_fields(entry.loader) } {
                if first_error.is_none() { first_error = Some(error); }
            }
            if let Some(loader) = loader.filter(|loader| *loader != entry.loader) {
                if let Err(error) = unsafe { clear_lazy_loader_fields(loader) } {
                    if first_error.is_none() { first_error = Some(error); }
                }
            }
            index += 1;
        }
        unsafe {
            // Completed prefixes may already have been destroyed reentrantly.
            // Releasing pointer lists must not dereference their entries again.
            (*linker.cast::<OwnedLinkerLoad>()).raw.lazy_loaders =
                TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            let tail = &mut *tail_mut_ptr(linker)?;
            tail.lazy_loaders = Vec::new();
            tail.lazy_entries = Vec::new();
        }
        match first_error { Some(error) => Err(error), None => Ok(()) }
    }

    fn export_count(&mut self, linker: usize) -> RuntimeResult<i32> {
        Ok(unsafe { (*(linker as *mut OwnedLinkerLoad)).raw.base.export_map.num })
    }

    fn export_object(&mut self, linker: usize, index: i32) -> RuntimeResult<usize> {
        let owner = linker as *mut OwnedLinkerLoad;
        let map = unsafe { (*owner).raw.base.export_map };
        let row = usize::try_from(index)
            .map_err(|_| RuntimeError::new("Linker export index is out of range"))?;
        if row >= map.num.max(0) as usize || map.data.is_null() {
            return Err(RuntimeError::new("Linker export index is out of range"));
        }
        let export = unsafe { map.data.cast::<FObjectExport>().add(row) };
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*export)._object)) } as usize)
    }

    fn detach_export(&mut self, linker: usize, index: i32) -> RuntimeResult<()> {
        let linker = linker as *mut UObject;
        let object = self.export_object(linker as usize, index)? as *mut UObject;
        if object.is_null() {
            return Ok(());
        }
        let object_linker = unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker)) };
        let object_index = unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker_index)) };
        if object_linker.cast::<UObject>() != linker || object_index != index {
            return Err(RuntimeError::new("Linker export has an inconsistent object identity"));
        }
        unsafe { self.access.unlink_object(object)?; }
        Ok(())
    }

    fn finish_export_detachment(&mut self, linker: usize) -> RuntimeResult<()> {
        let linker = linker as *mut UObject;
        for index in 0..self.access.object_count() {
            let object = self.access.object_at(index);
            if object.is_null() || object == linker {
                continue;
            }
            let object_linker = unsafe {
                ptr::read_unaligned(ptr::addr_of!((*object)._linker)).cast::<UObject>()
            };
            if object_linker == linker {
                return Err(RuntimeError::new("Linker export remains attached")
                    .with_context(&format!("object slot {index}")));
            }
        }

        let owner = linker.cast::<OwnedLinkerLoad>();
        let (lazy, loader, alt_loader) = unsafe {
            ((*owner).raw.lazy_loaders, (*owner).raw.loader, (*owner).raw.alt_loader)
        };
        if lazy.num != 0 || !lazy.data.is_null() {
            return Err(RuntimeError::new("Linker lazy loaders remain attached"));
        }
        if !loader.is_null() || !alt_loader.is_null() {
            return Err(RuntimeError::new("Linker archive handles remain attached"));
        }
        let exports = unsafe { (*owner).raw.base.export_map };
        if exports.num < 0 {
            return Err(RuntimeError::new("Negative linker export count"));
        }
        if exports.num > 0 && exports.data.is_null() {
            return Err(RuntimeError::new("Linker export table has no backing storage"));
        }
        for row in 0..exports.num as usize {
            let export = unsafe { exports.data.cast::<FObjectExport>().add(row) };
            let object = unsafe { ptr::read_unaligned(ptr::addr_of!((*export)._object)) };
            if !object.is_null() {
                return Err(RuntimeError::new("Linker export remains attached")
                    .with_context(&format!("export {row}")));
            }
        }
        unsafe { (*owner).tail_release_safe = true; }
        Ok(())
    }

    fn unregister(&mut self, linker: usize) -> RuntimeResult<()> {
        let linkers = unsafe { &mut (*self.access.loader_state()).linkers };
        linkers.retain(|entry| *entry != linker as *mut UObject);
        Ok(())
    }

    fn archive(
        &mut self,
        linker: usize,
        field: crate::linker_runtime::DestroyLoadArchive,
    ) -> RuntimeResult<usize> {
        let owner = linker as *mut OwnedLinkerLoad;
        let archive = unsafe {
            match field {
                crate::linker_runtime::DestroyLoadArchive::Loader => (*owner).raw.loader,
                crate::linker_runtime::DestroyLoadArchive::AltLoader => (*owner).raw.alt_loader,
            }
        };
        if archive.is_null() {
            Ok(0)
        } else {
            Err(RuntimeError::new("LinkerLoad contains an unowned archive handle"))
        }
    }

    fn close_archive(&mut self, _archive: usize) -> RuntimeResult<()> {
        // There is no standalone archive allocation on the Rust-owned load path.
        Ok(())
    }

    fn clear_archive(
        &mut self,
        linker: usize,
        field: crate::linker_runtime::DestroyLoadArchive,
    ) -> RuntimeResult<()> {
        let owner = linker as *mut OwnedLinkerLoad;
        unsafe {
            match field {
                crate::linker_runtime::DestroyLoadArchive::Loader => (*owner).raw.loader = ptr::null_mut(),
                crate::linker_runtime::DestroyLoadArchive::AltLoader => (*owner).raw.alt_loader = ptr::null_mut(),
            }
        }
        Ok(())
    }

    fn super_destroy(&mut self, _linker: usize) -> RuntimeResult<()> {
        // The owner lifecycle dispatcher runs Rust UObject destruction immediately after this policy.
        Ok(())
    }
}

struct CoreSetLinkerOps;

impl crate::linker_runtime::SetLinkerOps for CoreSetLinkerOps {
    type Error = RuntimeError;

    fn current_linker(&mut self, object: usize) -> RuntimeResult<usize> {
        // SAFETY: this adapter is only used with live canonical Core objects.
        let object = object as *mut UObject;
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker)) } as usize)
    }

    fn current_index(&mut self, object: usize) -> RuntimeResult<i32> {
        // SAFETY: this adapter is only used with live canonical Core objects.
        let object = object as *mut UObject;
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*object)._linker_index)) })
    }

    fn export_object(&mut self, linker: usize, index: i32) -> RuntimeResult<usize> {
        let linker = linker as *mut UObject;
        let row = usize::try_from(index)
            .map_err(|_| RuntimeError::new("Object has an invalid linker export index"))?;
        // SAFETY: `linker` is the object's previously stored canonical linker.
        let map = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.export_map };
        if row >= map.num.max(0) as usize || map.data.is_null() {
            return Err(RuntimeError::new("Object linker export index is out of range"));
        }
        // SAFETY: the checked map row is live for the duration of this engine-thread operation.
        let export = unsafe { map.data.cast::<FObjectExport>().add(row) };
        Ok(unsafe { ptr::read_unaligned(ptr::addr_of!((*export)._object)) } as usize)
    }

    fn set_export_object(
        &mut self,
        linker: usize,
        index: i32,
        object: usize,
    ) -> RuntimeResult<()> {
        let linker = linker as *mut UObject;
        let row = usize::try_from(index)
            .map_err(|_| RuntimeError::new("Object has an invalid linker export index"))?;
        // SAFETY: `linker` is the object's previously stored canonical linker.
        let map = unsafe { (*linker.cast::<OwnedLinkerLoad>()).raw.base.export_map };
        if row >= map.num.max(0) as usize || map.data.is_null() {
            return Err(RuntimeError::new("Object linker export index is out of range"));
        }
        // SAFETY: the checked map row is live for the duration of this engine-thread operation.
        let export = unsafe { map.data.cast::<FObjectExport>().add(row) };
        // SAFETY: the export row is canonical and `object` is the already-validated owner.
        unsafe {
            ptr::write_unaligned(
                ptr::addr_of_mut!((*export)._object),
                (object as *mut UObject).cast(),
            );
        }
        Ok(())
    }

    fn set_object_linker(&mut self, object: usize, linker: usize) -> RuntimeResult<()> {
        let object = object as *mut UObject;
        // SAFETY: the generic SetLinker kernel receives live canonical object/linker handles.
        unsafe {
            ptr::write_unaligned(
                ptr::addr_of_mut!((*object)._linker),
                (linker as *mut UObject).cast(),
            );
        }
        Ok(())
    }

    fn set_object_index(&mut self, object: usize, index: i32) -> RuntimeResult<()> {
        let object = object as *mut UObject;
        // SAFETY: the generic SetLinker kernel receives a live canonical object handle.
        unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*object)._linker_index), index) };
        Ok(())
    }
    fn invalid_export_owner(
        &mut self,
        _object: usize,
        _linker: usize,
        _index: i32,
    ) -> RuntimeError {
        RuntimeError::new("Object linker export does not own the object")
    }
}

impl CoreAccess<'_> {
    /// Loads every context-eligible export in a package, retaining its cached linker.
    ///
    /// `verify` suppresses the explicit LoadAllObjects phase, as in native
    /// LoadPackage. Import verification and selected-object materialization are
    /// separate operations.
    ///
    /// # Safety
    /// Bootstrap is complete, and every type actually constructed has a complete
    /// compatible Rust declaration. This capability is used on the engine thread.
    pub unsafe fn load_package(self, path: &Path, flags: u32) -> RuntimeResult<*mut UObject> {
        unsafe { load_package(self, path, flags) }
    }

    /// Acquires a linker without constructing unrelated exports, checking an optional GUID.
    ///
    /// # Safety
    /// Bootstrap is complete; `parent` is null or a live package and every constructed
    /// class has a complete compatible Rust record declaration.
    pub unsafe fn get_package_linker(
        self,
        path: &Path,
        flags: u32,
        parent: *mut UObject,
        expected_guid: Option<&[u32; 4]>,
    ) -> RuntimeResult<*mut UObject> {
        unsafe { get_package_linker(self, path, flags, parent, expected_guid) }
    }
    /// Verifies every import of a canonical linker in source order.
    ///
    /// # Safety
    /// `linker` is a live, registered Core-owned `ULinkerLoad`.
    pub unsafe fn verify_linker(self, linker: *mut UObject) -> RuntimeResult<()> {
        if linker.is_null() { return Err(RuntimeError::new("Cannot verify a null linker")); }
        let owner = linker.cast::<OwnedLinkerLoad>();
        if unsafe { !(*owner).initialized || (*owner).raw.base.success == 0 } {
            return Err(RuntimeError::new("Cannot verify an uninitialized linker"));
        }
        verify_linker_imports(self, linker)
    }


    /// `UObject::ResetLoaders(outer, DynamicOnly, ForceLazyLoad)` over canonical linkers.
    ///
    /// # Safety
    /// `outer` is null or a live canonical package, and all package-owned objects remain
    /// valid on the engine thread for the duration of the synchronous reset.
    pub unsafe fn reset_loaders(
        self,
        outer: *mut UObject,
        dynamic_only: bool,
        force_lazy_load: bool,
    ) -> RuntimeResult<()> {
        package_flow::reset_loaders_with(
            &CoreDestroyLoadOps { access: self }, outer as usize, dynamic_only, force_lazy_load,
            hp2_abi::engine::generated::CLASS_RUNTIME_STATIC as usize,
        )
    }

    /// Preloads one linked object, preserving recursive-load and error ordering.
    ///
    /// # Safety
    /// `object` is a live canonical object with valid linker and export identity.
    pub unsafe fn preload(self, object: *mut UObject) -> RuntimeResult<()> {
        unsafe { preload(self, object) }
    }
    /// Detaches an object from its canonical export and clears its linker identity.
    ///
    /// # Safety
    /// `object` must be a live Rust-owned object on this engine thread.
    pub(crate) unsafe fn unlink_object(self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Cannot unlink a null object"));
        }
        let _ = self;
        let mut ops = CoreSetLinkerOps;
        crate::linker_runtime::set_linker_with(&mut ops, object as usize, 0, -1)
    }
}


