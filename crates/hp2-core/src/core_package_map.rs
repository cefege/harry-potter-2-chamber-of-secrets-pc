//! Canonical package-map assembly over Core-owned package linkers.

use crate::array_storage::Hp2ArraySlots;
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_records::PackageMapRecord;
use hp2_abi::engine::generated::{
    FGenerationInfo, FObjectImport, FPackageInfo, TArrayRaw, UPackageMap, UObject,
};
use std::{ffi::c_void, ptr};


fn package_map(map: *mut UPackageMap) -> RuntimeResult<*mut PackageMapRecord> {
    if map.is_null() { return Err(RuntimeError::new("Null package map")); }
    Ok(map.cast())
}

unsafe fn list_record(map: *mut PackageMapRecord, index: i32) -> RuntimeResult<*mut FPackageInfo> {
    let list = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
    if list.num < 0 || list.max < list.num || (list.num != 0 && list.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap List storage"));
    }
    if index < 0 || index >= list.num { return Err(RuntimeError::new("PackageMap List index out of range")); }
    Ok(unsafe { list.data.cast::<FPackageInfo>().add(index as usize) })
}

unsafe fn append_package(map: *mut PackageMapRecord, linker: *mut UObject) -> RuntimeResult<i32> {
    let owner = linker.cast::<crate::core_loader::OwnedLinkerLoad>();
    let raw = unsafe { &(*owner).raw };
    let filename = raw.base.filename;
    if filename.num < 0 || (filename.num > 0 && filename.data.is_null()) {
        return Err(RuntimeError::new("Invalid LinkerLoad filename"));
    }
    let root = raw.base.linker_root.cast::<UObject>();
    if root.is_null() { return Err(RuntimeError::new("LinkerLoad has no package root")); }
    let file_size = crate::core_loader::linker_file_size(linker)?;
    let old = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
    if old.num < 0 || old.max < old.num || (old.num != 0 && old.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap List storage"));
    }
    let allow_download = raw.base.summary.package_flags & 0x0000_0001 != 0 && filename.num > 0;
    let url_bytes = if allow_download {
        (filename.num as usize).checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| RuntimeError::new("Package URL is too large"))?
    } else { 0 };
    let url = if url_bytes == 0 { ptr::null_mut() } else { unsafe { libc::malloc(url_bytes) }.cast::<u32>() };
    if url_bytes != 0 && url.is_null() { return Err(RuntimeError::new("Out of memory")); }
    if url_bytes != 0 { unsafe { ptr::copy_nonoverlapping(filename.data.cast::<u32>(), url, filename.num as usize); } }
    let slots = crate::array_storage::Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*map).list.data) },
        num: unsafe { ptr::addr_of_mut!((*map).list.num) },
        max: unsafe { ptr::addr_of_mut!((*map).list.max) },
    };
    let index = match unsafe {
        crate::array_storage::add_zeroed(
            slots,
            1,
            std::mem::size_of::<FPackageInfo>() as i32,
            ptr::null(),
            crate::array_storage::system_allocator(),
        )
    } {
        Ok(index) => index,
        Err(_) => {
            unsafe {
                ptr::addr_of_mut!((*map).list.num).write_unaligned(old.num);
                ptr::addr_of_mut!((*map).list.max).write_unaligned(old.max);
                libc::free(url.cast());
            }
            return Err(RuntimeError::new("Out of memory"));
        }
    };
    let list = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
    let row = unsafe { list.data.cast::<FPackageInfo>().add(index as usize) };
    unsafe {
        ptr::addr_of_mut!((*row).url).write_unaligned(TArrayRaw {
            data: url.cast::<c_void>(),
            num: if allow_download { filename.num } else { 0 },
            max: if allow_download { filename.num } else { 0 },
        });
        ptr::addr_of_mut!((*row).linker).write_unaligned(linker.cast());
        ptr::addr_of_mut!((*row).parent).write_unaligned(root.cast());
        ptr::addr_of_mut!((*row).guid).write_unaligned(raw.base.summary.guid);
        ptr::addr_of_mut!((*row).file_size).write_unaligned(file_size);
        ptr::addr_of_mut!((*row).package_flags).write_unaligned(raw.base.summary.package_flags);
    }
    Ok(index)
}

unsafe fn package_name(linker: *mut UObject) -> RuntimeResult<i32> {
    let root = unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.linker_root.cast::<UObject>() };
    if root.is_null() { return Err(RuntimeError::new("LinkerLoad has no package root")); }
    Ok(i32::from_ne_bytes(unsafe { ptr::addr_of!((*root).name).read_unaligned() }))
}

unsafe fn append_linker(access: CoreAccess<'_>, map: *mut PackageMapRecord, linker: *mut UObject) -> RuntimeResult<i32> {
    if linker.is_null() { return Err(RuntimeError::new("Null LinkerLoad")); }
    let mut package_class = None;
    crate::package_map::add_linker_with(
        linker.cast(),
        |linker| Ok(unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.summary.package_flags }),
        |linker| Ok(unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.linker_root }),
        || Ok(unsafe { ptr::addr_of!((*map).list.num).read_unaligned() }),
        |index| {
            let info = unsafe { list_record(map, index)? };
            Ok(unsafe { ptr::addr_of!((*info).parent).read_unaligned() })
        },
        |linker| unsafe { append_package(map, linker.cast()).map(|_| ()) },
        |linker| {
            let imports = unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.import_map };
            if imports.num < 0 || (imports.num > 0 && imports.data.is_null()) {
                return Err(RuntimeError::new("Invalid LinkerLoad import map"));
            }
            Ok(imports.num)
        },
        |linker, index| {
            let package_class = match package_class {
                Some(name) => name,
                None => {
                    let name = access.find_name(&[80, 97, 99, 107, 97, 103, 101])?;
                    package_class = Some(name);
                    name
                }
            };
            let imports = unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.import_map };
            if imports.num < 0 || index < 0 || index >= imports.num || imports.data.is_null() {
                return Err(RuntimeError::new("Invalid LinkerLoad import map"));
            }
            let import = unsafe { imports.data.cast::<FObjectImport>().add(index as usize).read_unaligned() };
            let is_package = i32::from_ne_bytes(import.class_name) == package_class && import.package_index == 0;
            Ok((is_package, i32::from_ne_bytes(import.object_name)))
        },
        || i32::try_from(unsafe { (&(*access.loader_state()).linkers).len() })
            .map_err(|_| RuntimeError::new("Too many Core linkers")),
        |index| {
            let index = usize::try_from(index).map_err(|_| RuntimeError::new("Invalid loader index"))?;
            let loader = unsafe { (&(*access.loader_state()).linkers).get(index).copied() }
                .ok_or_else(|| RuntimeError::new("Loader index out of range"))?;
            Ok(loader.cast())
        },
        |loader| unsafe { package_name(loader.cast()) },
        |loader| unsafe { append_linker(access, map, loader.cast()).map(|_| ()) },
    )
}

struct CoreComputeOps<'a> {
    access: CoreAccess<'a>,
    map: *mut PackageMapRecord,
}

impl crate::package_map::ComputePackageMapOps for CoreComputeOps<'_> {
    type Error = RuntimeError;

    fn package_len(&mut self) -> RuntimeResult<i32> {
        let list = unsafe { ptr::addr_of!((*self.map).list).read_unaligned() };
        if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
            return Err(RuntimeError::new("Invalid PackageMap List storage"));
        }
        Ok(list.num)
    }

    fn check_linker(&mut self, index: i32) -> RuntimeResult<()> {
        let info = unsafe { list_record(self.map, index)? };
        if unsafe { ptr::addr_of!((*info).linker).read_unaligned() }.is_null() {
            return Err(RuntimeError::new("Null package linker"));
        }
        Ok(())
    }

    fn max_global_names(&mut self) -> RuntimeResult<i32> {
        Ok(self.access.name_count())
    }

    fn reset_names(&mut self, count: i32) -> RuntimeResult<()> {
        let slot = unsafe { ptr::addr_of_mut!((*self.map).name_indices) };
        let old = unsafe { slot.read_unaligned() };
        if old.num < 0 || (old.num > 0 && old.data.is_null()) {
            return Err(RuntimeError::new("Invalid PackageMap name indices"));
        }
        let bytes = usize::try_from(count).ok()
            .and_then(|count| count.checked_mul(std::mem::size_of::<i32>()))
            .ok_or_else(|| RuntimeError::new("PackageMap name index table is too large"))?;
        let names = if bytes == 0 {
            unsafe { libc::free(old.data); }
            ptr::null_mut()
        } else {
            let names = unsafe { libc::realloc(old.data, bytes) }.cast::<i32>();
            if names.is_null() { return Err(RuntimeError::new("Out of memory")); }
            names
        };
        unsafe { slot.write_unaligned(TArrayRaw { data: names.cast(), num: count, max: count }); }
        Ok(())
    }

    fn names_len(&mut self) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*self.map).name_indices.num).read_unaligned() })
    }

    fn name_slot(&mut self, index: i32) -> RuntimeResult<*mut i32> {
        let names = unsafe { ptr::addr_of!((*self.map).name_indices).read_unaligned() };
        if index < 0 || index >= names.num || names.data.is_null() {
            return Err(RuntimeError::new("Global name index is out of range"));
        }
        Ok(unsafe { names.data.cast::<i32>().add(index as usize) })
    }

    fn empty_linkers(&mut self) -> RuntimeResult<()> {
        unsafe { crate::core_array::empty_package_linker_map(self.map) }
    }

    fn max_indices(&mut self) -> RuntimeResult<(u32, u32)> {
        Ok(unsafe {
            (
                ptr::addr_of!((*self.map).max_object_index).read_unaligned(),
                ptr::addr_of!((*self.map).max_name_index).read_unaligned(),
            )
        })
    }

    fn set_max_indices(&mut self, objects: u32, names: u32) -> RuntimeResult<()> {
        unsafe {
            ptr::addr_of_mut!((*self.map).max_object_index).write_unaligned(objects);
            ptr::addr_of_mut!((*self.map).max_name_index).write_unaligned(names);
        }
        Ok(())
    }

    fn package_info(&mut self, index: i32) -> RuntimeResult<crate::package_map::ComputePackageInfo> {
        let info = unsafe { list_record(self.map, index)? };
        Ok(crate::package_map::ComputePackageInfo {
            linker: unsafe { ptr::addr_of!((*info).linker).read_unaligned() },
            object_base: unsafe { ptr::addr_of_mut!((*info).object_base) },
            object_count: unsafe { ptr::addr_of_mut!((*info).object_count) },
            name_base: unsafe { ptr::addr_of_mut!((*info).name_base) },
            name_count: unsafe { ptr::addr_of_mut!((*info).name_count) },
            local_generation: unsafe { ptr::addr_of_mut!((*info).local_generation) },
            remote_generation: unsafe { ptr::addr_of_mut!((*info).remote_generation) },
        })
    }

    fn export_count(&mut self, linker: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.export_map.num })
    }

    fn name_count(&mut self, linker: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.name_map.num })
    }

    fn generation_count(&mut self, linker: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.summary.generations.num })
    }

    fn generation_exports(&mut self, linker: *mut c_void, index: i32) -> RuntimeResult<i32> {
        let generations = unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.summary.generations };
        if index < 0 || index >= generations.num || generations.data.is_null() {
            return Err(RuntimeError::new("Invalid remote package generation"));
        }
        Ok(unsafe { ptr::addr_of!((*generations.data.cast::<FGenerationInfo>().add(index as usize)).export_count).read_unaligned() })
    }

    fn generation_names(&mut self, linker: *mut c_void, index: i32) -> RuntimeResult<i32> {
        let generations = unsafe { (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.summary.generations };
        if index < 0 || index >= generations.num || generations.data.is_null() {
            return Err(RuntimeError::new("Invalid remote package generation"));
        }
        Ok(unsafe { ptr::addr_of!((*generations.data.cast::<FGenerationInfo>().add(index as usize)).name_count).read_unaligned() })
    }

    fn linker_name(&mut self, linker: *mut c_void, index: i32) -> RuntimeResult<i32> {
        crate::core_loader::linker_local_name(linker.cast(), index)
    }

    fn set_linker(&mut self, linker: *mut c_void, index: i32) -> RuntimeResult<()> {
        unsafe { crate::core_array::append_package_linker(self.map, linker.cast(), index) }
    }
}

unsafe fn compute(access: CoreAccess<'_>, map: *mut PackageMapRecord) -> RuntimeResult<()> {
    let mut ops = CoreComputeOps { access, map };
    unsafe { crate::package_map::compute_with(&mut ops) }
}

unsafe fn copy_linkers(access: CoreAccess<'_>, destination: *mut PackageMapRecord, source: *mut UPackageMap) -> RuntimeResult<()> {
    if source.is_null() { return Err(RuntimeError::new("Null source package map")); }
    let source = source.cast::<PackageMapRecord>();
    crate::package_map::copy_linkers_with(
        || {
            let list = unsafe { ptr::addr_of!((*source).list).read_unaligned() };
            if list.num < 0 || (list.num > 0 && list.data.is_null()) {
                return Err(RuntimeError::new("Invalid source PackageMap List"));
            }
            Ok(list.num)
        },
        |index| {
            let list = unsafe { ptr::addr_of!((*source).list).read_unaligned() };
            if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
                return Err(RuntimeError::new("Invalid source PackageMap List"));
            }
            if index < 0 || index >= list.num { return Ok(None); }
            let row = unsafe { list.data.cast::<FPackageInfo>().add(index as usize) };
            let linker = unsafe { ptr::addr_of!((*row).linker).read_unaligned().cast::<UObject>() };
            Ok((!linker.is_null()).then_some(linker.cast()))
        },
        |linker| unsafe { append_linker(access, destination, linker.cast()).map(|_| ()) },
        || unsafe { compute(access, destination) },
    )
}

impl CoreAccess<'_> {
    /// Adds a Core-owned linker and recursively admits its package imports.
    ///
    /// # Safety
    /// `map` is a live writable package map; `linker` is a live linker owned by this Core.
    pub unsafe fn package_map_add_linker(self, map: *mut UPackageMap, linker: *mut UObject) -> RuntimeResult<i32> {
        let map = package_map(map)?;
        unsafe { append_linker(self, map, linker) }
    }

    /// Rebuilds package ranges, global name indices, and the live linker map.
    ///
    /// # Safety
    /// `map` is a live writable package map whose entries refer to live Core-owned linkers.
    pub unsafe fn package_map_compute(self, map: *mut UPackageMap) -> RuntimeResult<()> {
        let map = package_map(map)?;
        unsafe { compute(self, map) }
    }

    /// Copies non-null source linkers into the destination and recomputes its mappings.
    ///
    /// # Safety
    /// Both maps are live; source entries and destination entries refer only to live Core-owned linkers.
    pub unsafe fn package_map_copy_linkers(self, destination: *mut UPackageMap, source: *mut UPackageMap) -> RuntimeResult<()> {
        let destination = package_map(destination)?;
        unsafe { copy_linkers(self, destination, source) }
    }

    /// Begins a Core package load transaction.
    pub fn begin_load(self) -> RuntimeResult<()> {
        crate::core_loader::begin_load(self);
        Ok(())
    }

    /// Ends a Core package load transaction and finishes at its outermost boundary.
    pub fn end_load(self) -> RuntimeResult<()> {
        crate::core_loader::end_load(self)
    }
}
/// Resolves one wire package-map index using the Core-owned linker/export table.
///
/// # Safety
/// `map` is live and every selected linker/export table belongs to this Core owner.
pub unsafe fn package_map_index_to_object(
    access: CoreAccess<'_>,
    map: *mut UPackageMap,
    index: i32,
    load: bool,
) -> RuntimeResult<*mut UObject> {
    let map = package_map(map)?;
    let list = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
    if list.num < 0 || list.max < list.num || (list.num != 0 && list.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap List storage"));
    }
    let resolve = unsafe {
        crate::package_map::index_to_object_with(
            index,
            load,
            || Ok(list.num),
            |package_index| {
                if package_index < 0 || package_index >= list.num {
                    return Err(RuntimeError::new("PackageMap List index out of range"));
                }
                Ok(list.data.cast::<FPackageInfo>().add(package_index as usize))
            },
            |package| {
                let count = (*package).object_count;
                if count < 0 {
                    Err(RuntimeError::new("Invalid PackageMap object count"))
                } else {
                    Ok(count)
                }
            },
            |package, local_index| {
                let linker = (*package).linker.cast::<UObject>();
                if linker.is_null() {
                    return Err(RuntimeError::new("PackageMap package has no linker"));
                }
                let exports = (*linker.cast::<crate::core_loader::OwnedLinkerLoad>()).raw.base.export_map;
                if exports.num < 0 || (exports.num > 0 && exports.data.is_null()) || local_index >= exports.num {
                    return Err(RuntimeError::new("PackageMap export index is out of range"));
                }
                let export = exports.data.cast::<hp2_abi::engine::generated::FObjectExport>()
                    .add(local_index as usize);
                Ok((*export)._object.cast::<c_void>())
            },
            || access.begin_load(),
            |package, local_index| {
                let linker = (*package).linker.cast::<UObject>();
                if linker.is_null() {
                    return Err(RuntimeError::new("PackageMap package has no linker"));
                }
                crate::core_loader::create_export(access, linker, local_index as usize)
                    .map(|object| object.cast())
            },
            || access.end_load(),
        )
    }?;
    Ok(resolve.cast())
}

/// Maps an object through a live Core package map, returning `INDEX_NONE` when absent.
///
/// # Safety
/// `map` and any linked object/linker records must remain live for the operation.
pub unsafe fn package_map_object_to_index(
    map: *mut UPackageMap,
    object: *mut UObject,
) -> RuntimeResult<i32> {
    let map = package_map(map)?;
    unsafe { crate::core_array::object_to_index(map, object) }
}

/// Returns the package map's current exclusive object-index bound.
///
/// # Safety
/// `map` must be a live canonical package map.
pub unsafe fn package_map_max_object_index(map: *mut UPackageMap) -> RuntimeResult<u32> {
    let map = package_map(map)?;
    Ok(unsafe { (*map).max_object_index })
}

impl CoreAccess<'_> {
    /// Resolves a wire package-map index through Core-owned package/export records.
    ///
    /// # Safety
    /// `map` and every selected linker/export table must remain live.
    pub unsafe fn package_map_index_to_object(
        self,
        map: *mut UPackageMap,
        index: i32,
        load: bool,
    ) -> RuntimeResult<*mut UObject> {
        unsafe { package_map_index_to_object(self, map, index, load) }
    }

    /// Maps a canonical object to its package-map index, or `INDEX_NONE`.
    ///
    /// # Safety
    /// `map` and `object` must remain live.
    pub unsafe fn package_map_object_to_index(
        self,
        map: *mut UPackageMap,
        object: *mut UObject,
    ) -> RuntimeResult<i32> {
        unsafe { package_map_object_to_index(map, object) }
    }

    /// Returns the package map's current exclusive object-index bound.
    ///
    /// # Safety
    /// `map` must be a live canonical package map.
    pub unsafe fn package_map_max_object_index(self, map: *mut UPackageMap) -> RuntimeResult<u32> {
        unsafe { package_map_max_object_index(map) }
    }
}
/// Gets or constructs a class replication cache using Core-owned class fields and map storage.
///
/// # Safety
/// `map` and `class` must remain live during recursive cache construction.
pub unsafe fn package_map_get_class_net_cache(
    access: CoreAccess<'_>,
    map: *mut UPackageMap,
    class: *mut hp2_abi::engine::generated::UClass,
) -> RuntimeResult<*mut hp2_abi::engine::generated::FClassNetCache> {
    if class.is_null() {
        return Err(RuntimeError::new("Null class for replication cache"));
    }
    let record = package_map(map)?;
    unsafe { class_net_cache(access, record, class) }
}

#[repr(C)]
struct ClassCachePair {
    hash_next: i32,
    _padding: i32,
    key: *mut c_void,
    value: *mut hp2_abi::engine::generated::FClassNetCache,
}

unsafe fn class_net_cache(
    access: CoreAccess<'_>,
    map: *mut PackageMapRecord,
    class: *mut hp2_abi::engine::generated::UClass,
) -> RuntimeResult<*mut hp2_abi::engine::generated::FClassNetCache> {
    use hp2_abi::engine::generated::{FClassNetCache, FFieldNetCache, UField, UProperty, UClass};
    let property_class = access.native_class("Core.Property")?;
    let cache_map = unsafe { ptr::addr_of_mut!((*map).class_field_indices) };
    let map_pairs = unsafe { ptr::addr_of!((*cache_map).pairs).read_unaligned() };
    if map_pairs.num < 0 || map_pairs.max < map_pairs.num || (map_pairs.num > 0 && map_pairs.data.is_null()) {
        return Err(RuntimeError::new("Invalid class cache map pairs"));
    }
    let pairs = map_pairs.data.cast::<ClassCachePair>();
    for i in 0..map_pairs.num {
        let pair = unsafe { pairs.add(i as usize) };
        if unsafe { ptr::addr_of!((*pair).key).read_unaligned() } == class.cast() {
            return Ok(unsafe { ptr::addr_of!((*pair).value).read_unaligned() });
        }
    }
    let object_index = unsafe { crate::core_array::object_to_index(map, class.cast())? };
    if object_index == -1 {
        return Ok(ptr::null_mut());
    }

    let cache = unsafe { libc::calloc(1, std::mem::size_of::<FClassNetCache>()).cast::<FClassNetCache>() };
    if cache.is_null() {
        return Err(RuntimeError::new("Class replication cache allocation failed"));
    }
    unsafe { (*cache).class = class.cast(); }
    let field_map = unsafe { cache_field_map(cache) };
    if let Err(error) = unsafe { crate::map_storage::initialize_owned_pod(field_map.cast()) } {
        unsafe { libc::free(cache.cast()); }
        return Err(error);
    }
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*cache_map).pairs.data).cast() },
        num: unsafe { ptr::addr_of_mut!((*cache_map).pairs.num) },
        max: unsafe { ptr::addr_of_mut!((*cache_map).pairs.max) },
    };
    let pair_index = unsafe {
        crate::array_storage::add(
            slots,
            1,
            std::mem::size_of::<ClassCachePair>() as i32,
            ptr::null(),
            crate::array_storage::system_allocator(),
        )
    }.map_err(|_| RuntimeError::new("Class replication cache map allocation failed"))?;
    let pair = unsafe { (*cache_map).pairs.data.cast::<ClassCachePair>().add(pair_index as usize) };
    unsafe {
        pair.write_unaligned(ClassCachePair {
            hash_next: -1,
            _padding: 0,
            key: class.cast(),
            value: cache,
        });
        rehash_class_cache_map(cache_map)?;
    }

    let superclass = unsafe { (*class).super_field.cast::<UClass>() };
    if !superclass.is_null() {
        let inherited = unsafe { class_net_cache(access, map, superclass)? };
        unsafe {
            (*cache).super_ = inherited.cast();
            (*cache).rep_condition_count = (*inherited).rep_condition_count;
            (*cache).fields_base = (*inherited).fields_base.wrapping_add((*inherited).fields.num);
            for index in 0..(*inherited).rep_properties.num {
                let property = (*inherited).rep_properties.data.cast::<*mut c_void>().add(index as usize).read_unaligned();
                append_cache_ptr(ptr::addr_of_mut!((*cache).rep_properties), property)?;
            }
        }
    }

    let net_fields = unsafe { ptr::addr_of!((*class).net_fields).read_unaligned() };
    if net_fields.num < 0 || net_fields.max < net_fields.num || (net_fields.num > 0 && net_fields.data.is_null()) {
        return Err(RuntimeError::new("Invalid class NetFields array"));
    }
    let property_class = property_class;
    let mut condition_count = unsafe { (*cache).rep_condition_count };
    for i in 0..net_fields.num {
        let field = unsafe { net_fields.data.cast::<*mut UField>().add(i as usize).read_unaligned() };
        let index = unsafe { crate::core_array::object_to_index(map, field.cast())? };
        if index == -1 { continue; }
        let mut condition = -1;
        let is_property = unsafe { is_property_field(field, property_class) };
        if is_property {
            let owner = unsafe { (*field.cast::<UProperty>()).rep_owner };
            let owner_index = if owner == field.cast() {
                -1
            } else {
                unsafe { crate::core_array::object_to_index(map, owner.cast())? }
            };
            if owner == field.cast() || owner_index == -1 {
                condition = condition_count;
                condition_count = condition_count.wrapping_add(1);
            }
        }
        let field_index = unsafe { (*cache).fields_base.wrapping_add((*cache).fields.num) };
        let field_slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*cache).fields.data).cast() },
            num: unsafe { ptr::addr_of_mut!((*cache).fields.num) },
            max: unsafe { ptr::addr_of_mut!((*cache).fields.max) },
        };
        let added = unsafe {
            crate::array_storage::add(
                field_slots,
                1,
                std::mem::size_of::<FFieldNetCache>() as i32,
                ptr::null(),
                crate::array_storage::system_allocator(),
            )
        }.map_err(|_| RuntimeError::new("Class replication fields allocation failed"))?;
        unsafe {
            (*cache).fields.data.cast::<FFieldNetCache>().add(added as usize).write_unaligned(
                FFieldNetCache { field: field.cast(), field_net_index: field_index, condition_index: condition },
            );
        }
    }
    unsafe { insert_cache_field_map(cache)?; }
    // Fill inherited owner conditions and the replicated-property list after all fields are indexed.
    for i in 0..unsafe { (*cache).fields.num } {
        let entry = unsafe { (*cache).fields.data.cast::<FFieldNetCache>().add(i as usize) };
        let field = unsafe { (*entry).field.cast::<UField>() };
        if !unsafe { is_property_field(field, property_class) } { continue; }
        let owner = unsafe { (*field.cast::<UProperty>()).rep_owner };
        if unsafe { (*entry).condition_index } == -1 {
            unsafe { (*entry).condition_index = find_owner_condition(cache, owner)?; }
        }
        let owner_class = unsafe { (*owner.cast::<UObject>()).outer.cast::<UClass>() };
        if owner_class.is_null() || unsafe { (*owner_class).class_flags & 0x800 != 0 } {
            continue;
        }
        unsafe { append_cache_ptr(ptr::addr_of_mut!((*cache).rep_properties), entry.cast())?; }
    }
    unsafe { (*cache).rep_condition_count = condition_count; }
    Ok(cache)
}

unsafe fn append_cache_ptr(array: *mut TArrayRaw, value: *mut c_void) -> RuntimeResult<()> {
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data).cast() },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    };
    let index = unsafe { crate::array_storage::add(slots, 1, 8, ptr::null(), crate::array_storage::system_allocator()) }
        .map_err(|_| RuntimeError::new("Replication property array allocation failed"))?;
    unsafe { (*array).data.cast::<*mut c_void>().add(index as usize).write_unaligned(value); }
    Ok(())
}

unsafe fn cache_field_map(
    cache: *mut hp2_abi::engine::generated::FClassNetCache,
) -> *mut crate::core_records::MapStorageRaw {
    let offset = std::mem::offset_of!(hp2_abi::engine::generated::FClassNetCache, fields)
        + std::mem::size_of::<TArrayRaw>();
    unsafe { cache.cast::<u8>().add(offset).cast() }
}

unsafe fn rehash_class_cache_map(map: *mut crate::core_records::MapStorageRaw) -> RuntimeResult<()> {
    let count = unsafe { (*map).hash_count };
    let hash = unsafe { (*map).hash };
    let pairs = unsafe { (*map).pairs };
    if count <= 0 || !(count as u32).is_power_of_two() || hash.is_null() || pairs.num < 0 || (pairs.num > 0 && pairs.data.is_null()) {
        return Err(RuntimeError::new("Invalid class cache map hash"));
    }
    for bucket in 0..count {
        unsafe { hash.add(bucket as usize).write_unaligned(-1); }
    }
    for index in 0..pairs.num {
        let pair = unsafe { pairs.data.cast::<ClassCachePair>().add(index as usize) };
        let key = unsafe { (*pair).key };
        if key.is_null() { return Err(RuntimeError::new("Null class cache key")); }
        let bucket = (unsafe { (*key.cast::<UObject>()).index } & (count - 1)) as usize;
        unsafe {
            (*pair).hash_next = hash.add(bucket).read_unaligned();
            hash.add(bucket).write_unaligned(index);
        }
    }
    Ok(())
}

unsafe fn insert_cache_field_map(cache: *mut hp2_abi::engine::generated::FClassNetCache) -> RuntimeResult<()> {
    let map = unsafe { cache_field_map(cache) };
    let count = unsafe { (*cache).fields.num };
    for index in 0..count {
        let field = unsafe { (*cache).fields.data.cast::<hp2_abi::engine::generated::FFieldNetCache>().add(index as usize) };
        let storage = unsafe { ptr::addr_of_mut!((*map).pairs) };
        let slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*storage).data).cast() },
            num: unsafe { ptr::addr_of_mut!((*storage).num) },
            max: unsafe { ptr::addr_of_mut!((*storage).max) },
        };
        let added = unsafe { crate::array_storage::add(slots, 1, 24, ptr::null(), crate::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Class field map allocation failed"))?;
        let pair = unsafe { (*storage).data.cast::<ClassCachePair>().add(added as usize) };
        unsafe {
            pair.write_unaligned(ClassCachePair {
                hash_next: -1,
                _padding: 0,
                key: (*field).field,
                value: field.cast(),
            });
        }
    }
    unsafe { rehash_class_cache_map(map) }
}

unsafe fn is_property_field(
    field: *mut hp2_abi::engine::generated::UField,
    property_class: *mut hp2_abi::engine::generated::UClass,
) -> bool {
    if field.is_null() || property_class.is_null() { return false; }
    let mut class = unsafe { (*field.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
    while !class.is_null() {
        if class == property_class { return true; }
        class = unsafe { (*class).super_field.cast() };
    }
    false
}

unsafe fn find_owner_condition(
    cache: *mut hp2_abi::engine::generated::FClassNetCache,
    owner: *mut c_void,
) -> RuntimeResult<i32> {
    let mut current = cache;
    while !current.is_null() {
        let fields = unsafe { (*current).fields };
        for index in 0..fields.num {
            let entry = unsafe { fields.data.cast::<hp2_abi::engine::generated::FFieldNetCache>().add(index as usize) };
            if unsafe { (*entry).field } == owner {
                return Ok(unsafe { (*entry).condition_index });
            }
        }
        current = unsafe { (*current).super_.cast() };
    }
    Err(RuntimeError::new("Replication owner has no class-cache field"))
}

impl CoreAccess<'_> {
    /// Returns a canonical Core-owned class replication cache.
    ///
    /// # Safety
    /// `map` and `class` must remain live during recursive cache construction.
    pub unsafe fn package_map_get_class_net_cache(
        self,
        map: *mut UPackageMap,
        class: *mut hp2_abi::engine::generated::UClass,
    ) -> RuntimeResult<*mut hp2_abi::engine::generated::FClassNetCache> {
        unsafe { package_map_get_class_net_cache(self, map, class) }
    }
}
