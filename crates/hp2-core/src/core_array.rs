//! Direct Rust lifetime and archive operations for Core-owned TArray fields.

use crate::core_access::{RuntimeError, RuntimeResult};
use crate::object_archive::ObjectArchive;
use hp2_abi::engine::generated::{FName, TArrayRaw};
use std::{ffi::c_void, ptr};

#[derive(Clone, Copy)]
pub(crate) struct CoreArrayElementOps {
    pub construct: unsafe fn(*mut u8) -> RuntimeResult<()>,
    pub destroy: unsafe fn(*mut u8) -> RuntimeResult<()>,
    pub copy: unsafe fn(*mut u8, *const u8) -> RuntimeResult<()>,
}

unsafe fn raw(array: *mut TArrayRaw) -> TArrayRaw {
    unsafe { ptr::read_unaligned(array) }
}

unsafe fn set_raw(array: *mut TArrayRaw, value: TArrayRaw) {
    unsafe { ptr::write_unaligned(array, value) }
}

unsafe fn set_owned_count(array: *mut TArrayRaw, count: i32) {
    let mut value = unsafe { raw(array) };
    value.num = count;
    value.max = count;
    unsafe { set_raw(array, value) }
}

unsafe fn publish_count(array: *mut TArrayRaw, count: i32) {
    let mut value = unsafe { raw(array) };
    value.num = count;
    unsafe { set_raw(array, value) }
}

fn bytes_for(count: i32, width: usize) -> RuntimeResult<usize> {
    if count < 0 { return Err(RuntimeError::new("Invalid TArray count")); }
    (count as usize).checked_mul(width).ok_or_else(|| RuntimeError::new("TArray allocation overflow"))
}

/// # Safety
/// `array` is a live TArray; all live elements are initialized and satisfy `ops`.
pub(crate) unsafe fn construct_array(array: *mut TArrayRaw, count: i32, width: usize, ops: CoreArrayElementOps) -> RuntimeResult<()> {
    if count < 0 { return Err(RuntimeError::new("Invalid TArray count")); }
    let bytes = bytes_for(count, width)?;
    let data = if bytes == 0 { ptr::null_mut() } else {
        let p = unsafe { libc::calloc(1, bytes) };
        if p.is_null() { return Err(RuntimeError::new("Out of memory")); }
        p
    };
    unsafe { set_raw(array, TArrayRaw { data: data.cast(), num: 0, max: count }); }
    for i in 0..count {
        let slot = unsafe { data.cast::<u8>().add(i as usize * width) };
        if let Err(error) = unsafe { (ops.construct)(slot) } {
            for initialized in (0..i).rev() {
                let prior = unsafe { data.cast::<u8>().add(initialized as usize * width) };
                let _ = unsafe { (ops.destroy)(prior) };
            }
            unsafe { libc::free(data); set_raw(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
            return Err(error);
        }
        unsafe { set_owned_count(array, i + 1); }
    }
    Ok(())
}

/// # Safety
/// `array` is a live TArray; its initialized elements satisfy `ops`.
pub(crate) unsafe fn destroy_array(array: *mut TArrayRaw, width: usize, ops: CoreArrayElementOps) -> RuntimeResult<()> {
    unsafe { destroy_array_with(array, width, Some(ops.destroy)) }
}

/// Releases Core-allocated TArray storage, optionally destroying each element.
///
/// # Safety
/// `array` is a live TArray and every live element is initialized. When present,
/// `destroy` is valid for each element at the supplied width.
pub(crate) unsafe fn destroy_array_with(
    array: *mut TArrayRaw,
    width: usize,
    destroy: Option<unsafe fn(*mut u8) -> RuntimeResult<()>>,
) -> RuntimeResult<()> {
    let value = unsafe { raw(array) };
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid TArray storage"));
    }
    let mut first = None;
    if let Some(destroy) = destroy {
        for i in (0..value.num as usize).rev() {
            let slot = unsafe { value.data.cast::<u8>().add(i * width) };
            if let Err(error) = unsafe { destroy(slot) } {
                if first.is_none() { first = Some(error); }
            }
        }
    }
    unsafe { libc::free(value.data); set_raw(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
    match first { Some(error) => Err(error), None => Ok(()) }
}

/// # Safety
/// `array` is a live TArray with initialized elements compatible with `ops`; source contains `count` valid elements.
pub(crate) unsafe fn copy_array(array: *mut TArrayRaw, source: *const u8, count: i32, width: usize, ops: CoreArrayElementOps) -> RuntimeResult<()> {
    if count < 0 || (count > 0 && source.is_null()) { return Err(RuntimeError::new("Invalid TArray copy source")); }
    unsafe { destroy_array(array, width, ops)?; }
    unsafe { construct_array(array, count, width, ops)?; }
    let destination = unsafe { raw(array) }.data.cast::<u8>();
    for i in 0..count as usize {
        let to = unsafe { destination.add(i * width) };
        let from = unsafe { source.add(i * width) };
        unsafe { (ops.copy)(to, from)?; }
    }
    Ok(())
}

/// # Safety
/// `array` is a live TArray with initialized elements compatible with `ops`.
pub(crate) unsafe fn empty_array(array: *mut TArrayRaw, slack: i32, width: usize, ops: CoreArrayElementOps) -> RuntimeResult<()> {
    if slack < 0 { return Err(RuntimeError::new("Invalid TArray slack")); }
    let old = unsafe { raw(array) };
    if old.num < 0 || (old.num > 0 && old.data.is_null()) { return Err(RuntimeError::new("Invalid TArray storage")); }
    let mut first = None;
    for i in (0..old.num as usize).rev() {
        if let Err(error) = unsafe { (ops.destroy)(old.data.cast::<u8>().add(i * width)) } { if first.is_none() { first = Some(error); } }
    }
    let capacity = bytes_for(slack, width)?;
    let data = if capacity == 0 { ptr::null_mut() } else {
        let p = unsafe { libc::realloc(old.data, capacity) };
        if p.is_null() { return Err(RuntimeError::new("Out of memory")); }
        p
    };
    unsafe { set_raw(array, TArrayRaw { data: data.cast(), num: 0, max: slack }); }
    match first { Some(error) => Err(error), None => Ok(()) }
}
/// Appends one raw element with the publication semantics of `FArray::Add`.
///
/// # Safety
/// `array` is a live TArray. The caller must initialize the new element before
/// any fallible operation, callback, or destruction can observe its live count.
pub(crate) unsafe fn append_array_uninitialized(
    array: *mut TArrayRaw,
    width: usize,
) -> RuntimeResult<i32> {
    if array.is_null() || width == 0 {
        return Err(RuntimeError::new("Invalid TArray append"));
    }
    let value = unsafe { raw(array) };
    if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid TArray storage"));
    }
    let index = value.num;
    let new_count = index.checked_add(1).ok_or_else(|| RuntimeError::new("TArray count overflow"))?;
    if new_count > value.max {
        let capacity = value.max.max(4).checked_mul(2).ok_or_else(|| RuntimeError::new("TArray capacity overflow"))?;
        let allocation_size = bytes_for(capacity, width)?;
        let allocation = unsafe { libc::realloc(value.data, allocation_size) };
        if allocation.is_null() { return Err(RuntimeError::new("Out of memory")); }
        let mut updated = value;
        updated.data = allocation.cast();
        updated.max = capacity;
        unsafe { set_raw(array, updated); }
    }
    unsafe { publish_count(array, new_count); }
    Ok(index)
}
/// Empties an initialized array through its element destructor.
///
/// # Safety
/// `array` is a live TArray and every live element is initialized for `destroy`.
pub(crate) unsafe fn empty_array_with(
    array: *mut TArrayRaw,
    width: usize,
    mut destroy: impl FnMut(*mut u8) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    if array.is_null() { return Err(RuntimeError::new("Null TArray")); }
    let value = unsafe { raw(array) };
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid TArray storage"));
    }
    let mut first = None;
    for index in (0..value.num as usize).rev() {
        let element = unsafe { value.data.cast::<u8>().add(index * width) };
        if let Err(error) = destroy(element) {
            if first.is_none() { first = Some(error); }
        }
    }
    if !value.data.is_null() { unsafe { libc::free(value.data); } }
    unsafe { set_raw(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
    match first { Some(error) => Err(error), None => Ok(()) }
}

/// Appends UTF-8 text as one directly allocated UTF-32 FString array element.
/// # Safety
/// `array` is a live `TArray<FString>` whose current strings are valid libc allocations.
pub(crate) unsafe fn append_string_utf8(array: *mut TArrayRaw, text: &str) -> RuntimeResult<()> {
    let old = unsafe { raw(array) };
    if old.num < 0 || old.max < old.num || (old.num > 0 && old.data.is_null()) {
        return Err(RuntimeError::new("Invalid FString array storage"));
    }
    let next = old.num.checked_add(1).ok_or_else(|| RuntimeError::new("FString array count overflow"))?;
    let char_count = i32::try_from(text.chars().count()).map_err(|_| RuntimeError::new("FString is too long"))?;
    let char_bytes = bytes_for(char_count, std::mem::size_of::<u32>())?;
    let chars = if char_bytes == 0 { ptr::null_mut() } else {
        let p = unsafe { libc::malloc(char_bytes) }.cast::<u32>();
        if p.is_null() { return Err(RuntimeError::new("Out of memory")); }
        for (index, scalar) in text.chars().enumerate() { unsafe { p.add(index).write(scalar as u32); } }
        p
    };
    let reserve = unsafe {
        crate::array_storage::reserve_owned(
            ptr::addr_of_mut!((*array).data), ptr::addr_of_mut!((*array).num),
            ptr::addr_of_mut!((*array).max), next, std::mem::size_of::<TArrayRaw>() as i32,
        )
    };
    if reserve.is_err() {
        unsafe { libc::free(chars.cast::<c_void>()); }
        return Err(RuntimeError::new("Unable to grow FString array"));
    }
    let grown = unsafe { ptr::addr_of!((*array).data).read_unaligned().cast::<TArrayRaw>() };
    let item = unsafe { grown.add(old.num as usize) };
    unsafe {
        ptr::write_unaligned(item, TArrayRaw { data: chars.cast(), num: char_count, max: char_count });
        ptr::addr_of_mut!((*array).num).write_unaligned(next);
    }
    Ok(())
}

/// Serializes one Core-owned FString with the allocator used by `CoreAccess`.
///
/// # Safety
/// `array` is a live FString record with valid counted, terminated storage.
pub(crate) unsafe fn serialize_owned_fstring(
    access: crate::core_access::CoreAccess<'_>,
    archive: &ObjectArchive<'_>,
    array: *mut TArrayRaw,
) -> RuntimeResult<()> {
    let value = unsafe { raw(array) };
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid FString storage"));
    }
    if archive.is_loading() {
        let mut text = Vec::new();
        archive.serialize_fstring(&mut text)?;
        text.push(0);
        unsafe { access.assign_fstring(array, &text)?; }
    } else if archive.is_saving() {
        if value.num == 0 {
            let mut count = 0;
            archive.serialize_compact_index(&mut count)?;
            return Ok(());
        }
        let text = if value.num <= 1 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize - 1) }
        };
        archive.write_fstring(text)?;
    }
    Ok(())
}

/// Releases a Core-owned FString through its paired array allocator.
///
/// # Safety
/// `array` is a live FString with storage allocated by Core's system allocator.
pub(crate) unsafe fn destroy_owned_fstring(array: *mut TArrayRaw) -> RuntimeResult<()> {
    let slots = unsafe { array_slots(array) };
    unsafe { crate::array_storage::release(slots, crate::array_storage::system_allocator()) }
        .map_err(|_| RuntimeError::new("Unable to release FString storage"))
}

/// Serializes `TArray<FString>` using Core-owned outer and inner allocations.
///
/// # Safety
/// `array` is a live TArray whose elements are initialized Core-owned FStrings.
pub(crate) unsafe fn serialize_owned_fstring_array(
    access: crate::core_access::CoreAccess<'_>,
    archive: &ObjectArchive<'_>,
    array: *mut TArrayRaw,
) -> RuntimeResult<()> {
    let old = unsafe { raw(array) };
    if old.num < 0 || (old.num > 0 && old.data.is_null()) {
        return Err(RuntimeError::new("Invalid FString array storage"));
    }
    let mut count = old.num;
    archive.serialize_compact_index(&mut count)?;
    if count < 0 {
        return Err(RuntimeError::new("Invalid FString array count"));
    }
    if archive.is_loading() {
        let previous = old.data.cast::<TArrayRaw>();
        for index in (0..old.num as usize).rev() {
            unsafe { destroy_owned_fstring(previous.add(index))?; }
        }
        let slots = unsafe { array_slots(array) };
        unsafe { crate::array_storage::release(slots, crate::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Unable to release FString array"))?;
        if count != 0 {
            let result = unsafe {
                crate::array_storage::set_exact(
                    slots,
                    count,
                    i32::try_from(std::mem::size_of::<TArrayRaw>())
                        .map_err(|_| RuntimeError::new("Invalid FString array element width"))?,
                    ptr::null(),
                    crate::array_storage::system_allocator(),
                )
            };
            if result.is_err() {
                unsafe { set_raw(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
                return Err(RuntimeError::new("Unable to allocate FString array"));
            }
            let fresh = unsafe { raw(array) }.data.cast::<TArrayRaw>();
            for index in 0..count as usize {
                unsafe { fresh.add(index).write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
            }
        }
    }
    let current = unsafe { raw(array) };
    let data = current.data.cast::<TArrayRaw>();
    for index in 0..count as usize {
        if data.is_null() {
            return Err(RuntimeError::new("Null FString array storage"));
        }
        unsafe { serialize_owned_fstring(access, archive, data.add(index))?; }
    }
    Ok(())
}

/// Destroys all initialized FString elements and their owning array allocation.
///
/// # Safety
/// `array` is a live array of initialized Core-owned FStrings.
pub(crate) unsafe fn destroy_owned_fstring_array(array: *mut TArrayRaw) -> RuntimeResult<()> {
    let value = unsafe { raw(array) };
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid FString array storage"));
    }
    let items = value.data.cast::<TArrayRaw>();
    let mut first_error = None;
    for index in (0..value.num as usize).rev() {
        if let Err(error) = unsafe { destroy_owned_fstring(items.add(index)) } {
            if first_error.is_none() { first_error = Some(error); }
        }
    }
    if let Err(error) = unsafe {
        crate::array_storage::release(array_slots(array), crate::array_storage::system_allocator())
    } {
        if first_error.is_none() { first_error = Some(RuntimeError::new("Unable to release FString array")); }
        let _ = error;
    }
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

unsafe fn array_slots(array: *mut TArrayRaw) -> crate::array_storage::Hp2ArraySlots {
    crate::array_storage::Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data) },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    }
}

/// Serializes the mapped FName entries in a Core-owned name array.
/// # Safety
/// `array` is a live `TArray<FName>` and every live slot is initialized.
pub(crate) unsafe fn serialize_names(archive: &ObjectArchive<'_>, array: *mut TArrayRaw) -> RuntimeResult<()> {
    let old = unsafe { raw(array) };
    let mut count = old.num;
    archive.serialize_compact_index(&mut count)?;
    if count < 0 { return Err(RuntimeError::new("Invalid enum name count")); }
    let data = if archive.is_loading() {
        let size = bytes_for(count, std::mem::size_of::<FName>())?;
        let p = if size == 0 { ptr::null_mut() } else { unsafe { libc::calloc(1, size) } };
        if size != 0 && p.is_null() { return Err(RuntimeError::new("Out of memory")); }
        if !old.data.is_null() { unsafe { libc::free(old.data); } }
        unsafe { set_raw(array, TArrayRaw { data: p, num: 0, max: count }); }
        p.cast::<FName>()
    } else { old.data.cast::<FName>() };
    if count > 0 && data.is_null() { return Err(RuntimeError::new("Null enum names")); }
    for index in 0..count as usize {
        let slot = unsafe { data.add(index) };
        if archive.is_loading() { unsafe { publish_count(array, index as i32 + 1); } }
        unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*slot).index))?; }
    }
    Ok(())
}

/// Serializes a `TArray<FDependency>` through the native array and element operators.
///
/// # Safety
/// `array` is a live dependency array. Each live dependency has the generated
/// Core record layout and mapped class pointers are valid for this archive.
pub(crate) unsafe fn serialize_dependencies(archive: &ObjectArchive<'_>, array: *mut TArrayRaw) -> RuntimeResult<()> {
    let old = unsafe { raw(array) };
    let mut count = old.num;
    archive.serialize_compact_index(&mut count)?;
    if count < 0 { return Err(RuntimeError::new("Invalid dependency count")); }
    let data = if archive.is_loading() {
        let bytes = bytes_for(count, std::mem::size_of::<hp2_abi::engine::generated::FDependency>())?;
        let p = if bytes == 0 { ptr::null_mut() } else { unsafe { libc::calloc(1, bytes) } };
        if bytes != 0 && p.is_null() { return Err(RuntimeError::new("Out of memory")); }
        if !old.data.is_null() { unsafe { libc::free(old.data); } }
        unsafe { set_raw(array, TArrayRaw { data: p, num: 0, max: count }); }
        p.cast::<hp2_abi::engine::generated::FDependency>()
    } else { old.data.cast::<hp2_abi::engine::generated::FDependency>() };
    if count > 0 && data.is_null() { return Err(RuntimeError::new("Null dependencies")); }
    for index in 0..count as usize {
        let dependency = unsafe { data.add(index) };
        if archive.is_loading() { unsafe { publish_count(array, index as i32 + 1); } }
        unsafe {
            archive.serialize_object_slot(ptr::addr_of_mut!((*dependency).class).cast())?;
            let mut deep = ptr::addr_of!((*dependency).deep).read_unaligned();
            archive.serialize_i32(&mut deep)?;
            ptr::addr_of_mut!((*dependency).deep).write_unaligned(deep);
            let mut crc = ptr::addr_of!((*dependency).script_text_crc).read_unaligned();
            archive.serialize_u32(&mut crc)?;
            ptr::addr_of_mut!((*dependency).script_text_crc).write_unaligned(crc);
        }
    }
    Ok(())
}

#[repr(C)]
struct ObjectIntPair {
    hash_next: i32,
    key: *mut hp2_abi::engine::generated::UObject,
    value: i32,
}

#[repr(C)]
struct ObjectCachePair {
    hash_next: i32,
    key: *mut hp2_abi::engine::generated::UObject,
    value: *mut hp2_abi::engine::generated::FClassNetCache,
}

/// Maps an object's local linker export index to its package-map index.
///
/// # Safety
/// `map` and `object` are live canonical records; package and pair arrays are valid.
pub(crate) unsafe fn object_to_index(
    map: *mut crate::core_records::PackageMapRecord,
    object: *mut hp2_abi::engine::generated::UObject,
) -> RuntimeResult<i32> {
    if map.is_null() || object.is_null() { return Ok(-1); }
    let linker = unsafe { ptr::addr_of!((*object)._linker).read().cast::<hp2_abi::engine::generated::UObject>() };
    let local_index = unsafe { ptr::addr_of!((*object)._linker_index).read() };
    if linker.is_null() || local_index == -1 { return Ok(-1); }
    let linker_map = unsafe { ptr::addr_of!((*map).linker_map).read_unaligned() };
    let pairs = unsafe { ptr::addr_of!(linker_map.pairs).read_unaligned() };
    if pairs.num < 0 || (pairs.num > 0 && pairs.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap linker map"));
    }
    let pair_data = pairs.data.cast::<ObjectIntPair>();
    let package_index = (0..pairs.num as usize).find_map(|index| {
        let pair = unsafe { pair_data.add(index) };
        (unsafe { ptr::addr_of!((*pair).key).read_unaligned() } == linker)
            .then(|| unsafe { ptr::addr_of!((*pair).value).read_unaligned() })
    });
    let Some(package_index) = package_index else { return Ok(-1); };
    let list = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
    if package_index < 0 || package_index >= list.num || list.data.is_null() {
        return Err(RuntimeError::new("PackageMap linker index is outside List"));
    }
    let info = unsafe { list.data.cast::<hp2_abi::engine::generated::FPackageInfo>().add(package_index as usize) };
    let object_count = unsafe { ptr::addr_of!((*info).object_count).read_unaligned() };
    if local_index >= object_count { return Ok(-1); }
    let object_base = unsafe { ptr::addr_of!((*info).object_base).read_unaligned() };
    Ok(object_base.wrapping_add(local_index))
}

/// Serializes the native UPackageMap fields in `List`, `LinkerMap`, `ClassFieldIndices` order.
///
/// # Safety
/// All map fields are live and their native `HashCount` invariant is valid.
pub(crate) unsafe fn serialize_package_map(
    archive: &ObjectArchive<'_>,
    map: *mut crate::core_records::PackageMapRecord,
) -> RuntimeResult<()> {
    unsafe {
        serialize_package_list(archive, ptr::addr_of_mut!((*map).list))?;
        serialize_pointer_int_map(archive, ptr::addr_of_mut!((*map).linker_map))?;
        serialize_pointer_cache_map(archive, ptr::addr_of_mut!((*map).class_field_indices))?;
    }
    Ok(())
}

unsafe fn serialize_package_list(archive: &ObjectArchive<'_>, list: *mut TArrayRaw) -> RuntimeResult<()> {
    let old = unsafe { raw(list) };
    let mut count = old.num;
    archive.serialize_compact_index(&mut count)?;
    if count < 0 { return Err(RuntimeError::new("Invalid PackageMap List count")); }
    let entries = if archive.is_loading() {
        if old.num < 0 || (old.num > 0 && old.data.is_null()) { return Err(RuntimeError::new("Invalid PackageMap List storage")); }
        let bytes = bytes_for(count, std::mem::size_of::<hp2_abi::engine::generated::FPackageInfo>())?;
        let data = if bytes == 0 { ptr::null_mut() } else { unsafe { libc::calloc(1, bytes) } };
        if bytes != 0 && data.is_null() { return Err(RuntimeError::new("Out of memory")); }
        let previous = old.data.cast::<hp2_abi::engine::generated::FPackageInfo>();
        for index in 0..old.num as usize { unsafe { release_fstring(ptr::addr_of_mut!((*previous.add(index)).url)); } }
        unsafe { libc::free(old.data); set_raw(list, TArrayRaw { data, num: 0, max: count }); }
        data.cast::<hp2_abi::engine::generated::FPackageInfo>()
    } else { old.data.cast::<hp2_abi::engine::generated::FPackageInfo>() };
    if count > 0 && entries.is_null() { return Err(RuntimeError::new("Null PackageMap List")); }
    for index in 0..count as usize {
        if archive.is_loading() { unsafe { publish_count(list, index as i32 + 1); } }
        let entry = unsafe { entries.add(index) };
        unsafe {
            archive.serialize_object_slot(ptr::addr_of_mut!((*entry).parent).cast())?;
            archive.serialize_object_slot(ptr::addr_of_mut!((*entry).linker).cast())?;
        }
    }
    Ok(())
}

unsafe fn release_fstring(array: *mut TArrayRaw) {
    let value = unsafe { raw(array) };
    unsafe { libc::free(value.data); set_raw(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
}

unsafe fn serialize_pointer_int_map(archive: &ObjectArchive<'_>, map: *mut crate::core_records::MapStorageRaw) -> RuntimeResult<()> {
    let pairs = unsafe { ptr::addr_of_mut!((*map).pairs) };
    let (data, count) = unsafe { serialize_pair_count(archive, pairs, std::mem::size_of::<ObjectIntPair>())? };
    let data = data.cast::<ObjectIntPair>();
    if count > 0 && data.is_null() { return Err(RuntimeError::new("Null LinkerMap pairs")); }
    for index in 0..count as usize {
        let pair = unsafe { data.add(index) };
        if archive.is_loading() { unsafe { publish_count(pairs, index as i32 + 1); } }
        unsafe {
            archive.serialize_object_slot(ptr::addr_of_mut!((*pair).key).cast())?;
            let mut value = ptr::addr_of!((*pair).value).read_unaligned();
            archive.serialize_i32(&mut value)?;
            ptr::addr_of_mut!((*pair).value).write_unaligned(value);
        }
    }
    if archive.is_loading() { unsafe { rehash_map(map, data.cast(), count, std::mem::size_of::<ObjectIntPair>())?; } }
    Ok(())
}

unsafe fn serialize_pointer_cache_map(archive: &ObjectArchive<'_>, map: *mut crate::core_records::MapStorageRaw) -> RuntimeResult<()> {
    let pairs = unsafe { ptr::addr_of_mut!((*map).pairs) };
    let (data, count) = unsafe { serialize_pair_count(archive, pairs, std::mem::size_of::<ObjectCachePair>())? };
    let data = data.cast::<ObjectCachePair>();
    if count > 0 && data.is_null() { return Err(RuntimeError::new("Null ClassFieldIndices pairs")); }
    for index in 0..count as usize {
        let pair = unsafe { data.add(index) };
        if archive.is_loading() { unsafe { publish_count(pairs, index as i32 + 1); } }
        // FClassNetCache* has an intentionally empty native archive operator.
        unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*pair).key).cast())?; }
    }
    if archive.is_loading() { unsafe { rehash_map(map, data.cast(), count, std::mem::size_of::<ObjectCachePair>())?; } }
    Ok(())
}

unsafe fn serialize_pair_count(
    archive: &ObjectArchive<'_>,
    pairs: *mut TArrayRaw,
    width: usize,
) -> RuntimeResult<(*mut c_void, i32)> {
    let old = unsafe { raw(pairs) };
    let mut count = old.num;
    archive.serialize_compact_index(&mut count)?;
    if count < 0 { return Err(RuntimeError::new("Invalid TMap pair count")); }
    if archive.is_loading() {
        let bytes = bytes_for(count, width)?;
        let data = if bytes == 0 { ptr::null_mut() } else { unsafe { libc::calloc(1, bytes) } };
        if bytes != 0 && data.is_null() { return Err(RuntimeError::new("Out of memory")); }
        unsafe { libc::free(old.data); set_raw(pairs, TArrayRaw { data, num: 0, max: count }); }
        Ok((data, count))
    } else {
        if count > 0 && old.data.is_null() { return Err(RuntimeError::new("Null TMap pairs")); }
        Ok((old.data, count))
    }
}
/// Initializes an empty native TMap with its established eight hash buckets.
///
/// # Safety
/// `map` points to a zero-initialized, exclusively owned map with no allocation.
pub(crate) unsafe fn initialize_map_storage(
    map: *mut crate::core_records::MapStorageRaw,
) -> RuntimeResult<()> {
    unsafe { rehash_map(map, ptr::null_mut(), 0, std::mem::size_of::<ObjectIntPair>()) }
}

/// Appends a canonical linker/index pair to a package map's live TMap storage.
///
/// The pair's key is the canonical export object and its value is its list index.
pub(crate) unsafe fn append_package_linker(
    map: *mut crate::core_records::PackageMapRecord,
    linker: *mut hp2_abi::engine::generated::UObject,
    index: i32,
) -> RuntimeResult<()> {
    if map.is_null() || linker.is_null() {
        return Err(RuntimeError::new("Null package-map linker"));
    }
    let raw = unsafe { ptr::addr_of_mut!((*map).linker_map) };
    let pairs = unsafe { ptr::addr_of_mut!((*raw).pairs) };
    let header = unsafe { pairs.read_unaligned() };
    if header.num < 0 || header.max < header.num || (header.num != 0 && header.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap linker map"));
    }
    let slots = crate::array_storage::Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*pairs).data) },
        num: unsafe { ptr::addr_of_mut!((*pairs).num) },
        max: unsafe { ptr::addr_of_mut!((*pairs).max) },
    };
    let pair_index = match unsafe {
        crate::array_storage::add_zeroed(
            slots,
            1,
            std::mem::size_of::<ObjectIntPair>() as i32,
            ptr::null(),
            crate::array_storage::system_allocator(),
        )
    } {
        Ok(index) => index,
        Err(_) => {
            unsafe {
                ptr::addr_of_mut!((*pairs).num).write_unaligned(header.num);
                ptr::addr_of_mut!((*pairs).max).write_unaligned(header.max);
            }
            return Err(RuntimeError::new("Out of memory"));
        }
    };
    let count = unsafe { ptr::addr_of!((*pairs).num).read_unaligned() };
    let entries = unsafe { ptr::addr_of!((*pairs).data).read_unaligned().cast::<ObjectIntPair>() };
    let entry = unsafe { entries.add(pair_index as usize) };
    unsafe {
        ptr::addr_of_mut!((*entry).hash_next).write_unaligned(-1);
        ptr::addr_of_mut!((*entry).key).write_unaligned(linker);
        ptr::addr_of_mut!((*entry).value).write_unaligned(index);
        rehash_map(raw, entries.cast(), count, std::mem::size_of::<ObjectIntPair>())?;
    }
    Ok(())
}
pub(crate) unsafe fn empty_package_linker_map(
    map: *mut crate::core_records::PackageMapRecord,
) -> RuntimeResult<()> {
    if map.is_null() { return Err(RuntimeError::new("Null package map")); }
    let storage = unsafe { ptr::addr_of_mut!((*map).linker_map) };
    let pairs = unsafe { ptr::addr_of_mut!((*storage).pairs) };
    let header = unsafe { pairs.read_unaligned() };
    if header.num < 0 || (header.num != 0 && header.data.is_null()) {
        return Err(RuntimeError::new("Invalid PackageMap linker map"));
    }
    unsafe {
        pairs.write_unaligned(TArrayRaw { num: 0, ..header });
        rehash_map(storage, header.data.cast(), 0, std::mem::size_of::<ObjectIntPair>())
    }
}


unsafe fn rehash_map(
    map: *mut crate::core_records::MapStorageRaw,
    pairs: *mut u8,
    count: i32,
    pair_width: usize,
) -> RuntimeResult<()> {
    let hash_count = unsafe { ptr::addr_of!((*map).hash_count).read_unaligned() };
    if hash_count < 8 || !(hash_count as u32).is_power_of_two() {
        return Err(RuntimeError::new("Invalid TMap hash count"));
    }
    let hash_slot = unsafe { ptr::addr_of_mut!((*map).hash) };
    let old_hash = unsafe { hash_slot.read_unaligned() };
    let fresh = unsafe {
        crate::map_storage::rehash_with(
            old_hash, hash_count,
            |slots| {
                let bytes = bytes_for(slots, std::mem::size_of::<i32>())?;
                let result = unsafe { libc::malloc(bytes) }.cast::<i32>();
                if result.is_null() { Err(RuntimeError::new("Out of memory")) } else { Ok(result) }
            },
            |old| { unsafe { libc::free(old.cast()); } Ok(()) },
            || Ok(count),
            |index| {
                let pair = unsafe { pairs.add(index as usize * pair_width) };
                let key = unsafe { ptr::read_unaligned(pair.add(8).cast::<*mut hp2_abi::engine::generated::UObject>()) };
                Ok(if key.is_null() { 0 } else { unsafe { ptr::addr_of!((*key).index).read_unaligned() } })
            },
            |index, next| {
                let pair = unsafe { pairs.add(index as usize * pair_width) };
                unsafe { pair.cast::<i32>().write_unaligned(next); }
                Ok(())
            },
        )?
    };
    unsafe { hash_slot.write_unaligned(fresh); }
    Ok(())
}
