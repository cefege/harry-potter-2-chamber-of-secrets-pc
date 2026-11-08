//! Live package-map construction, name serialization, indexing, and queries.
//!
//! C++ retains typed containers and object layouts. Rust owns ordered traversal,
//! admission, generation negotiation, offsets, and serialization decisions.

use crate::object_collections::ObjectCollection;
use std::ffi::c_void;

/// Typed-container and field-access leaves for the live C++ package map.
#[repr(C)]
pub struct Hp2PackageMapServices {
    pub object_linker: unsafe extern "C" fn(object: *mut c_void) -> *mut c_void,
    pub object_linker_index: unsafe extern "C" fn(object: *mut c_void) -> i32,
    pub find_linker:
        unsafe extern "C" fn(map: *mut c_void, linker: *mut c_void, found: *mut *const i32) -> i32,
    pub package_len: unsafe extern "C" fn(packages: *mut c_void) -> i32,
    pub package_at:
        unsafe extern "C" fn(packages: *mut c_void, index: i32, package: *mut *mut c_void) -> i32,
    pub package_parent: unsafe extern "C" fn(package: *mut c_void) -> *mut c_void,
    pub package_object_count: unsafe extern "C" fn(package: *mut c_void) -> i32,
    pub package_object_base: unsafe extern "C" fn(package: *mut c_void) -> i32,
    pub export_object:
        unsafe extern "C" fn(package: *mut c_void, index: i32, object: *mut *mut c_void) -> i32,
}

/// Loading leaves are separate because `CreateExport` requires UPackageMap's
/// friend access. Each callback catches C++ exceptions and returns their status.
#[repr(C)]
pub struct Hp2PackageMapLoadServices {
    pub begin_load: unsafe extern "C" fn() -> i32,
    pub create_export:
        unsafe extern "C" fn(package: *mut c_void, index: i32, object: *mut *mut c_void) -> i32,
    pub end_load: unsafe extern "C" fn() -> i32,
}

fn check_status(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/// Maps an object's local export index through the live linker map.
///
/// # Safety
/// Containers, objects, returned field pointers, and callbacks must remain valid
/// for their use. Only `object` may be null. Callbacks preserve C++ leaf semantics.
pub unsafe fn object_to_index(
    object: *mut c_void,
    linker_map: *mut c_void,
    packages: *mut c_void,
    services: &Hp2PackageMapServices,
) -> Result<i32, i32> {
    unsafe {
        if !object.is_null()
            && !(services.object_linker)(object).is_null()
            && (services.object_linker_index)(object) != -1
        {
            let mut found = std::ptr::null();
            check_status((services.find_linker)(
                linker_map,
                (services.object_linker)(object),
                &mut found,
            ))?;
            if !found.is_null() {
                let mut package = std::ptr::null_mut();
                check_status((services.package_at)(packages, *found, &mut package))?;
                if (services.object_linker_index)(object) < (services.package_object_count)(package)
                {
                    return Ok((services.package_object_base)(package)
                        .wrapping_add((services.object_linker_index)(object)));
                }
            }
        }
        Ok(-1)
    }
}

/// Searches parents in list order, including null-parent matches.
///
/// # Safety
/// The live package container and callback-returned packages must remain valid.
/// `outer` is compared by identity and may be null.
pub unsafe fn supports_package(
    packages: *mut c_void,
    outer: *mut c_void,
    services: &Hp2PackageMapServices,
) -> Result<i32, i32> {
    unsafe {
        let mut index = 0;
        while index < (services.package_len)(packages) {
            let mut package = std::ptr::null_mut();
            check_status((services.package_at)(packages, index, &mut package))?;
            if (services.package_parent)(package) == outer {
                return Ok(1);
            }
            index += 1;
        }
        Ok(0)
    }
}

/// Resolves a package-map index using typed package operations shared by Core and Engine.
///
/// # Safety
/// Every returned package, export and callback pointer remains live for the operation.
pub unsafe fn index_to_object_with<P: Copy, E>(
    mut index: i32,
    load: bool,
    mut package_len: impl FnMut() -> Result<i32, E>,
    mut package_at: impl FnMut(i32) -> Result<P, E>,
    mut package_object_count: impl FnMut(P) -> Result<i32, E>,
    mut export_object: impl FnMut(P, i32) -> Result<*mut c_void, E>,
    mut begin_load: impl FnMut() -> Result<(), E>,
    mut create_export: impl FnMut(P, i32) -> Result<*mut c_void, E>,
    mut end_load: impl FnMut() -> Result<(), E>,
) -> Result<*mut c_void, E> {
    if index < 0 { return Ok(std::ptr::null_mut()); }
    let mut package_index = 0;
    while package_index < package_len()? {
        let package = package_at(package_index)?;
        let count = package_object_count(package)?;
        if index < count {
            let object = export_object(package, index)?;
            if !object.is_null() || !load { return Ok(object); }
            begin_load()?;
            let created = create_export(package, index)?;
            end_load()?;
            return Ok(created);
        }
        index = index.wrapping_sub(count);
        package_index += 1;
    }
    Ok(std::ptr::null_mut())
}

/// Resolves a concatenated export index, loading only an absent selected export.
/// A failed load leaf stops immediately, just as a thrown C++ exception did;
/// `EndLoad` is not added as cleanup after a failed `CreateExport`.
///
/// # Safety
/// The live package container, selected package/linker, and callbacks must remain
/// valid. Selected packages must have the same valid linker/export slot required
/// by the original C++ implementation, even when `load` is zero.
pub unsafe fn index_to_object(
    packages: *mut c_void,
    index: i32,
    load: i32,
    services: &Hp2PackageMapServices,
    loading: &Hp2PackageMapLoadServices,
) -> Result<*mut c_void, i32> {
    unsafe {
        index_to_object_with(
            index,
            load != 0,
            || Ok((services.package_len)(packages)),
            |i| {
                let mut package = std::ptr::null_mut();
                check_status((services.package_at)(packages, i, &mut package))?;
                Ok(package)
            },
            |package| Ok((services.package_object_count)(package)),
            |package, i| {
                let mut object = std::ptr::null_mut();
                check_status((services.export_object)(package, i, &mut object))?;
                Ok(object)
            },
            || check_status((loading.begin_load)()),
            |package, i| {
                let mut object = std::ptr::null_mut();
                check_status((loading.create_export)(package, i, &mut object))?;
                Ok(object)
            },
            || check_status((loading.end_load)()),
        )
    }
}

/// Borrowed field addresses, not a copy of the live package record.
#[repr(C)]
pub struct Hp2PackageInfoView {
    pub linker: *mut *mut c_void,
    pub parent: *mut c_void,
    pub object_base: *mut i32,
    pub object_count: *mut i32,
    pub name_base: *mut i32,
    pub name_count: *mut i32,
    pub local_generation: *mut i32,
    pub remote_generation: *mut i32,
}

#[repr(C)]
pub struct Hp2PackageMapBuildView {
    pub owner: *mut c_void,
    pub packages: *mut c_void,
    pub linker_map: *mut c_void,
    pub names: *mut c_void,
    pub max_objects: *mut u32,
    pub max_names: *mut u32,
    pub loaders: *mut ObjectCollection,
}

#[repr(C)]
pub struct Hp2PackageImportView {
    pub is_package: i32,
    pub package_index: i32,
    pub object_name: i32,
}

/// Construction and archive leaves, separate from the existing query ABI.
/// Fallible leaves return status and never unwind across the ABI.
#[repr(C)]
pub struct Hp2PackageMapBuildServices {
    pub package_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub package_at: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2PackageInfoView) -> i32,
    pub linker_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub linker_root: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub append_package: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub import_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub import_at: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2PackageImportView) -> i32,
    pub root_name: unsafe extern "C" fn(*mut c_void) -> i32,
    pub add_linker: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub compute: unsafe extern "C" fn(*mut c_void) -> i32,
    pub check_linker: unsafe extern "C" fn(*mut c_void) -> i32,
    pub max_global_names: unsafe extern "C" fn() -> i32,
    pub names_empty: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub names_add: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub names_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub name_slot: unsafe extern "C" fn(*mut c_void, i32, *mut *mut i32) -> i32,
    pub linkers_empty: unsafe extern "C" fn(*mut c_void) -> i32,
    pub export_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub name_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub generation_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub generation_exports: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub generation_names: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub linker_name: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub set_linker: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub name_index: unsafe extern "C" fn(*mut c_void) -> i32,
    pub serialize_int: unsafe extern "C" fn(*mut c_void, *mut u32, u32) -> i32,
    pub is_loading: unsafe extern "C" fn(*mut c_void) -> i32,
    pub is_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub name_none: unsafe extern "C" fn(*mut c_void),
    pub select_name: unsafe extern "C" fn(*mut c_void, i32, *mut c_void) -> i32,
}

unsafe fn build_package(
    packages: *mut c_void,
    index: i32,
    s: &Hp2PackageMapBuildServices,
) -> Result<Hp2PackageInfoView, i32> {
    unsafe {
        let mut info = std::mem::MaybeUninit::uninit();
        check_status((s.package_at)(packages, index, info.as_mut_ptr()))?;
        Ok(info.assume_init())
    }
}

unsafe fn build_import(
    linker: *mut c_void,
    index: i32,
    s: &Hp2PackageMapBuildServices,
) -> Result<Hp2PackageImportView, i32> {
    unsafe {
        let mut import = std::mem::MaybeUninit::uninit();
        check_status((s.import_at)(linker, index, import.as_mut_ptr()))?;
        Ok(import.assume_init())
    }
}

unsafe fn name_slot(
    names: *mut c_void,
    index: i32,
    s: &Hp2PackageMapBuildServices,
) -> Result<*mut i32, i32> {
    unsafe {
        let mut slot = std::ptr::null_mut();
        check_status((s.name_slot)(names, index, &mut slot))?;
        Ok(slot)
    }
}

/// Shared admission and recursive import policy for native and Core adapters.
pub(crate) fn add_linker_with<E>(
    linker: *mut c_void,
    mut linker_flags: impl FnMut(*mut c_void) -> Result<u32, E>,
    mut linker_root: impl FnMut(*mut c_void) -> Result<*mut c_void, E>,
    mut package_len: impl FnMut() -> Result<i32, E>,
    mut package_parent: impl FnMut(i32) -> Result<*mut c_void, E>,
    mut append_package: impl FnMut(*mut c_void) -> Result<(), E>,
    mut import_len: impl FnMut(*mut c_void) -> Result<i32, E>,
    mut package_import: impl FnMut(*mut c_void, i32) -> Result<(bool, i32), E>,
    mut loader_len: impl FnMut() -> Result<i32, E>,
    mut loader_at: impl FnMut(i32) -> Result<*mut c_void, E>,
    mut loader_root_name: impl FnMut(*mut c_void) -> Result<i32, E>,
    mut add_linker: impl FnMut(*mut c_void) -> Result<(), E>,
) -> Result<i32, E> {
    if linker_flags(linker)? & 0x0004 != 0 { return Ok(-1); }
    let root = linker_root(linker)?;
    let mut index = 0;
    while index < package_len()? {
        if package_parent(index)? == root { return Ok(index); }
        index += 1;
    }
    let index = package_len()?;
    append_package(linker)?;
    let mut import_index = 0;
    while import_index < import_len(linker)? {
        if package_import(linker, import_index)?.0 {
            let mut loader_index = 0;
            while loader_index < loader_len()? {
                let mut loader = loader_at(loader_index)?;
                let import_name = package_import(linker, import_index)?.1;
                if loader_root_name(loader)? == import_name {
                    loader = loader_at(loader_index)?;
                    add_linker(loader)?;
                }
                loader_index += 1;
            }
        }
        import_index += 1;
    }
    Ok(index)
}

/// Admits before recursion, so cyclic imports and duplicate roots terminate.
/// Virtual recursion is retained through `add_linker`; the default C++ method
/// reenters this algorithm, while overrides retain their legacy dispatch.
///
/// # Safety
/// Views and callbacks must describe live C++ storage. Linkers and loader roots
/// must satisfy the original nonnull preconditions. Collection lengths are
/// reread after callbacks; no package address survives recursive admission.
pub unsafe fn add_linker(
    map: &Hp2PackageMapBuildView,
    linker: *mut c_void,
    s: &Hp2PackageMapBuildServices,
) -> Result<i32, i32> {
    unsafe {
        add_linker_with(
            linker,
            |linker| Ok((s.linker_flags)(linker)),
            |linker| Ok((s.linker_root)(linker)),
            || Ok((s.package_len)(map.packages)),
            |index| Ok(build_package(map.packages, index, s)?.parent),
            |linker| check_status((s.append_package)(map.packages, linker)),
            |linker| Ok((s.import_len)(linker)),
            |linker, index| {
                let import = build_import(linker, index, s)?;
                Ok((import.is_package != 0 && import.package_index == 0, import.object_name))
            },
            || Ok((*map.loaders).len()),
            |index| Ok((*map.loaders).at(index)),
            |loader| Ok((s.root_name)(loader)),
            |loader| {
                let mut ignored_index = -1;
                check_status((s.add_linker)(map.owner, loader, &mut ignored_index))
            },
        )
    }
}

/// Builds concatenated ranges and the first occurrence of each global name.
///
/// # Safety
/// Field addresses remain valid through the same non-reentrant leaves as the
/// original FPackageInfo reference. Generation indices and global name indices
/// retain their legacy valid-index preconditions (negative generations are not
/// repaired). Count arithmetic preserves the original 32-bit representation.
#[derive(Clone, Copy)]
pub(crate) struct ComputePackageInfo {
    pub linker: *mut c_void,
    pub object_base: *mut i32,
    pub object_count: *mut i32,
    pub name_base: *mut i32,
    pub name_count: *mut i32,
    pub local_generation: *mut i32,
    pub remote_generation: *mut i32,
}

pub(crate) trait ComputePackageMapOps {
    type Error;
    fn package_len(&mut self) -> Result<i32, Self::Error>;
    fn check_linker(&mut self, index: i32) -> Result<(), Self::Error>;
    fn max_global_names(&mut self) -> Result<i32, Self::Error>;
    fn reset_names(&mut self, count: i32) -> Result<(), Self::Error>;
    fn names_len(&mut self) -> Result<i32, Self::Error>;
    fn name_slot(&mut self, index: i32) -> Result<*mut i32, Self::Error>;
    fn empty_linkers(&mut self) -> Result<(), Self::Error>;
    fn max_indices(&mut self) -> Result<(u32, u32), Self::Error>;
    fn set_max_indices(&mut self, objects: u32, names: u32) -> Result<(), Self::Error>;
    fn package_info(&mut self, index: i32) -> Result<ComputePackageInfo, Self::Error>;
    fn export_count(&mut self, linker: *mut c_void) -> Result<i32, Self::Error>;
    fn name_count(&mut self, linker: *mut c_void) -> Result<i32, Self::Error>;
    fn generation_count(&mut self, linker: *mut c_void) -> Result<i32, Self::Error>;
    fn generation_exports(&mut self, linker: *mut c_void, index: i32) -> Result<i32, Self::Error>;
    fn generation_names(&mut self, linker: *mut c_void, index: i32) -> Result<i32, Self::Error>;
    fn linker_name(&mut self, linker: *mut c_void, index: i32) -> Result<i32, Self::Error>;
    fn set_linker(&mut self, linker: *mut c_void, index: i32) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn compute_with<O: ComputePackageMapOps>(ops: &mut O) -> Result<(), O::Error> {
    let mut index = 0;
    while index < ops.package_len()? {
        ops.check_linker(index)?;
        index += 1;
    }
    let max_global_names = ops.max_global_names()?;
    ops.reset_names(max_global_names)?;
    index = 0;
    while index < ops.names_len()? {
        unsafe { ops.name_slot(index)?.write(-1); }
        index += 1;
    }
    ops.empty_linkers()?;
    ops.set_max_indices(0, 0)?;
    index = 0;
    while index < ops.package_len()? {
        let info = ops.package_info(index)?;
        let (mut max_objects, mut max_names) = ops.max_indices()?;
        let object_base = max_objects as i32;
        let name_base = max_names as i32;
        unsafe {
            info.object_base.write_unaligned(object_base);
            info.name_base.write_unaligned(name_base);
            info.object_count.write_unaligned(ops.export_count(info.linker)?);
            info.name_count.write_unaligned(ops.name_count(info.linker)?);
            info.local_generation.write_unaligned(ops.generation_count(info.linker)?);
        }
        let local_generation = unsafe { info.local_generation.read_unaligned() };
        let mut remote_generation = unsafe { info.remote_generation.read_unaligned() };
        if remote_generation == 0 {
            remote_generation = local_generation;
            unsafe { info.remote_generation.write_unaligned(remote_generation); }
        }
        if remote_generation < local_generation {
            let generation_index = remote_generation.wrapping_sub(1);
            let count = ops.generation_exports(info.linker, generation_index)?;
            unsafe { info.object_count.write_unaligned(info.object_count.read_unaligned().min(count)); }
            let count = ops.generation_names(info.linker, generation_index)?;
            unsafe { info.name_count.write_unaligned(info.name_count.read_unaligned().min(count)); }
        }
        let object_count = unsafe { info.object_count.read_unaligned() };
        let name_count = unsafe { info.name_count.read_unaligned() };
        max_objects = max_objects.wrapping_add(object_count as u32);
        max_names = max_names.wrapping_add(name_count as u32);
        ops.set_max_indices(max_objects, max_names)?;
        let mut local = 0;
        while local < ops.name_count(info.linker)?.min(name_count) {
            let mut global = ops.linker_name(info.linker, local)?;
            let slot = ops.name_slot(global)?;
            if unsafe { slot.read_unaligned() } == -1 {
                global = ops.linker_name(info.linker, local)?;
                let slot = ops.name_slot(global)?;
                unsafe { slot.write_unaligned(name_base.wrapping_add(local)); }
            }
            local += 1;
        }
        ops.set_linker(info.linker, index)?;
        index += 1;
    }
    Ok(())
}

struct NativeComputeOps<'a> {
    map: &'a Hp2PackageMapBuildView,
    services: &'a Hp2PackageMapBuildServices,
}

impl ComputePackageMapOps for NativeComputeOps<'_> {
    type Error = i32;

    fn package_len(&mut self) -> Result<i32, i32> {
        Ok(unsafe { (self.services.package_len)(self.map.packages) })
    }

    fn check_linker(&mut self, index: i32) -> Result<(), i32> {
        let info = unsafe { build_package(self.map.packages, index, self.services)? };
        check_status(unsafe { (self.services.check_linker)(*info.linker) })
    }

    fn max_global_names(&mut self) -> Result<i32, i32> {
        Ok(unsafe { (self.services.max_global_names)() })
    }

    fn reset_names(&mut self, count: i32) -> Result<(), i32> {
        check_status(unsafe { (self.services.names_empty)(self.map.names, count) })?;
        check_status(unsafe { (self.services.names_add)(self.map.names, count) })
    }

    fn names_len(&mut self) -> Result<i32, i32> {
        Ok(unsafe { (self.services.names_len)(self.map.names) })
    }

    fn name_slot(&mut self, index: i32) -> Result<*mut i32, i32> {
        unsafe { name_slot(self.map.names, index, self.services) }
    }

    fn empty_linkers(&mut self) -> Result<(), i32> {
        check_status(unsafe { (self.services.linkers_empty)(self.map.linker_map) })
    }

    fn max_indices(&mut self) -> Result<(u32, u32), i32> {
        Ok((unsafe { *self.map.max_objects }, unsafe { *self.map.max_names }))
    }

    fn set_max_indices(&mut self, objects: u32, names: u32) -> Result<(), i32> {
        unsafe {
            *self.map.max_objects = objects;
            *self.map.max_names = names;
        }
        Ok(())
    }

    fn package_info(&mut self, index: i32) -> Result<ComputePackageInfo, i32> {
        let info = unsafe { build_package(self.map.packages, index, self.services)? };
        Ok(ComputePackageInfo {
            linker: unsafe { *info.linker },
            object_base: info.object_base,
            object_count: info.object_count,
            name_base: info.name_base,
            name_count: info.name_count,
            local_generation: info.local_generation,
            remote_generation: info.remote_generation,
        })
    }

    fn export_count(&mut self, linker: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (self.services.export_len)(linker) })
    }

    fn name_count(&mut self, linker: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (self.services.name_len)(linker) })
    }

    fn generation_count(&mut self, linker: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (self.services.generation_len)(linker) })
    }

    fn generation_exports(&mut self, linker: *mut c_void, index: i32) -> Result<i32, i32> {
        let mut count = 0;
        check_status(unsafe { (self.services.generation_exports)(linker, index, &mut count) })?;
        Ok(count)
    }

    fn generation_names(&mut self, linker: *mut c_void, index: i32) -> Result<i32, i32> {
        let mut count = 0;
        check_status(unsafe { (self.services.generation_names)(linker, index, &mut count) })?;
        Ok(count)
    }

    fn linker_name(&mut self, linker: *mut c_void, index: i32) -> Result<i32, i32> {
        let mut name = 0;
        check_status(unsafe { (self.services.linker_name)(linker, index, &mut name) })?;
        Ok(name)
    }

    fn set_linker(&mut self, linker: *mut c_void, index: i32) -> Result<(), i32> {
        check_status(unsafe { (self.services.set_linker)(self.map.linker_map, linker, index) })
    }
}

/// Builds concatenated ranges and the first occurrence of each global name.
///
/// # Safety
/// Field addresses remain valid through the same non-reentrant leaves as the
/// original FPackageInfo reference. Generation indices and global name indices
/// retain their legacy valid-index preconditions (negative generations are not
/// repaired). Count arithmetic preserves the original 32-bit representation.
pub unsafe fn compute(
    map: &Hp2PackageMapBuildView,
    s: &Hp2PackageMapBuildServices,
) -> Result<(), i32> {
    let mut ops = NativeComputeOps { map, services: s };
    unsafe { compute_with(&mut ops) }
}

/// Shared copy traversal for callback-backed and Core-owned package maps.
pub(crate) fn copy_linkers_with<E>(
    mut source_len: impl FnMut() -> Result<i32, E>,
    mut source_linker: impl FnMut(i32) -> Result<Option<*mut c_void>, E>,
    mut add_linker: impl FnMut(*mut c_void) -> Result<(), E>,
    mut compute: impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let mut index = 0;
    while index < source_len()? {
        if let Some(linker) = source_linker(index)? {
            add_linker(linker)?;
        }
        index += 1;
    }
    compute()
}

/// # Safety
/// Source/destination collections remain live. Virtual admission may mutate
/// either list; source length and linker are reread on each legacy access.
pub unsafe fn copy_linkers(
    map: &Hp2PackageMapBuildView,
    other_packages: *mut c_void,
    s: &Hp2PackageMapBuildServices,
) -> Result<(), i32> {
    unsafe {
        copy_linkers_with(
            || Ok((s.package_len)(other_packages)),
            |index| {
                if (*build_package(other_packages, index, s)?.linker).is_null() {
                    Ok(None)
                } else {
                    Ok(Some((*build_package(other_packages, index, s)?.linker)))
                }
            },
            |linker| {
                let mut ignored_index = -1;
                check_status((s.add_linker)(map.owner, linker, &mut ignored_index))
            },
            || check_status((s.compute)(map.owner)),
        )
    }
}

/// # Safety
/// Archive/name pointers are live typed C++ objects. Selected name-map indices
/// and the initial global name index retain the original valid-index contract.
pub unsafe fn serialize_name(
    map: &Hp2PackageMapBuildView,
    archive: *mut c_void,
    name: *mut c_void,
    s: &Hp2PackageMapBuildServices,
) -> Result<i32, i32> {
    unsafe {
        let mut index = if (s.name_index)(name) < (s.names_len)(map.names) {
            *name_slot(map.names, (s.name_index)(name), s)? as u32
        } else {
            *map.max_names
        };
        check_status((s.serialize_int)(
            archive,
            &mut index,
            (*map.max_names).wrapping_add(1),
        ))?;
        if (s.is_loading)(archive) != 0 {
            (s.name_none)(name);
            if index < *map.max_names && (s.is_error)(archive) == 0 {
                let mut i = 0;
                while i < (s.package_len)(map.packages) {
                    let info = build_package(map.packages, i, s)?;
                    if index < *info.name_count as u32 {
                        check_status((s.select_name)(*info.linker, index as i32, name))?;
                        break;
                    }
                    index = index.wrapping_sub(*info.name_count as u32);
                    i += 1;
                }
            }
            Ok(1)
        } else {
            Ok(i32::from(index != *map.max_names))
        }
    }
}

/// Borrowed scalar slots in a C++-owned class cache; array elements are never
/// retained across allocation or shrinking.
#[repr(C)]
pub struct Hp2ClassNetCacheView {
    pub super_cache: *mut *mut c_void,
    pub condition_count: *mut i32,
    pub fields_base: *mut i32,
}

#[repr(C)]
pub struct Hp2ClassNetFieldView {
    pub field: *mut c_void,
    pub condition_index: *mut i32,
}

/// Class-cache storage leaves, independent of the index/construction ABIs.
#[repr(C)]
pub struct Hp2ClassNetCacheServices {
    pub find_cache: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub object_to_index: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub publish_cache: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub cache_view: unsafe extern "C" fn(*mut c_void, *mut Hp2ClassNetCacheView),
    pub super_class: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub get_class_cache: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub copy_rep_properties: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub net_fields_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub net_field_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub fields_empty: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub fields_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub field_property: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub rep_owner: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub append_field: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
    pub fields_shrink: unsafe extern "C" fn(*mut c_void) -> i32,
    pub field_at: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2ClassNetFieldView) -> i32,
    pub set_field_map: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub owner_condition: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub native_replication: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub add_rep_property: unsafe extern "C" fn(*mut c_void, i32) -> i32,
}

unsafe fn class_cache_view(
    cache: *mut c_void,
    s: &Hp2ClassNetCacheServices,
) -> Hp2ClassNetCacheView {
    unsafe {
        let mut view = std::mem::MaybeUninit::uninit();
        (s.cache_view)(cache, view.as_mut_ptr());
        view.assume_init()
    }
}

/// Builds in published C++ storage, retaining partially initialized caches on
/// failure. Recursive cache loading and admission use the original virtual calls.
///
/// # Safety
/// Callbacks must expose live class/cache storage and preserve legacy nonnull
/// superclass-cache and replication-owner lookup preconditions. Cache scalar
/// slots remain stable across callbacks; field slots remain stable after shrink.
pub unsafe fn get_class_net_cache(
    map: *mut c_void,
    class: *mut c_void,
    s: &Hp2ClassNetCacheServices,
) -> Result<*mut c_void, i32> {
    const INDEX_NONE: i32 = -1;
    unsafe {
        let mut cache = std::ptr::null_mut();
        check_status((s.find_cache)(map, class, &mut cache))?;
        if !cache.is_null() {
            return Ok(cache);
        }
        let mut index = INDEX_NONE;
        check_status((s.object_to_index)(map, class, &mut index))?;
        if index == INDEX_NONE {
            return Ok(cache);
        }
        check_status((s.publish_cache)(map, class, &mut cache))?;
        let view = class_cache_view(cache, s);
        *view.super_cache = std::ptr::null_mut();
        *view.condition_count = 0;
        *view.fields_base = 0;
        let mut super_class = std::ptr::null_mut();
        check_status((s.super_class)(class, &mut super_class))?;
        if !super_class.is_null() {
            // The legacy expression calls GetSuperClass again after its test.
            check_status((s.super_class)(class, &mut super_class))?;
            let mut super_cache = std::ptr::null_mut();
            check_status((s.get_class_cache)(map, super_class, &mut super_cache))?;
            *view.super_cache = super_cache;
            check_status((s.copy_rep_properties)(cache, super_cache))?;
            let inherited = class_cache_view(super_cache, s);
            *view.condition_count = *inherited.condition_count;
            *view.fields_base = (*inherited.fields_base).wrapping_add((s.fields_len)(super_cache));
        }
        check_status((s.fields_empty)(cache, (s.net_fields_len)(class)))?;
        let mut i = 0;
        while i < (s.net_fields_len)(class) {
            let mut field = std::ptr::null_mut();
            check_status((s.net_field_at)(class, i, &mut field))?;
            check_status((s.object_to_index)(map, field, &mut index))?;
            if index != INDEX_NONE {
                let mut condition = INDEX_NONE;
                let field_index = (*view.fields_base).wrapping_add((s.fields_len)(cache));
                let mut property = std::ptr::null_mut();
                check_status((s.field_property)(field, &mut property))?;
                if !property.is_null() {
                    let owner = (s.rep_owner)(property);
                    let own_condition = if owner == property {
                        true
                    } else {
                        check_status((s.object_to_index)(map, owner, &mut index))?;
                        index == INDEX_NONE
                    };
                    if own_condition {
                        condition = *view.condition_count;
                        *view.condition_count = condition.wrapping_add(1);
                    }
                }
                check_status((s.append_field)(cache, field, field_index, condition))?;
            }
            i += 1;
        }
        check_status((s.fields_shrink)(cache))?;
        let mut i = 0;
        while i < (s.fields_len)(cache) {
            check_status((s.set_field_map)(cache, i))?;
            i += 1;
        }
        let mut i = 0;
        while i < (s.fields_len)(cache) {
            let mut field = std::mem::MaybeUninit::<Hp2ClassNetFieldView>::uninit();
            check_status((s.field_at)(cache, i, field.as_mut_ptr()))?;
            let field = field.assume_init();
            let mut property = std::ptr::null_mut();
            check_status((s.field_property)(field.field, &mut property))?;
            if !property.is_null() {
                if *field.condition_index == INDEX_NONE {
                    let mut condition = INDEX_NONE;
                    check_status((s.owner_condition)(
                        cache,
                        (s.rep_owner)(property),
                        &mut condition,
                    ))?;
                    *field.condition_index = condition;
                }
                let mut native = 0;
                check_status((s.native_replication)(property, &mut native))?;
                if native == 0 {
                    check_status((s.add_rep_property)(cache, i))?;
                }
            }
            i += 1;
        }
        Ok(cache)
    }
}
