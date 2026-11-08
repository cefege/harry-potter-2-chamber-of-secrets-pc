//! Legacy FPropertyTag construction, header wire policy and packed-bool payloads.
//! Native archive primitives/counting-archive scope retain their own stack lifetimes.

use crate::property_archive::Hp2PropertyArchiveTag;
use std::ffi::c_void;
use std::ptr;

type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;
pub type Hp2PropertyTagCountContinuation = unsafe extern "C" fn(Object, Object) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PropertyTagInteger {
    TagPropertyType,
    TagPropertyName,
    TagStructName,
    TagBitMask,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PropertyTagKind {
    TagIsStruct,
    TagIsBool,
    TagIsExactBool,
}
#[repr(C)]
pub struct Hp2PropertyTagServices {
    pub bool_property: i32,
    pub struct_property: i32,
    pub integer: unsafe extern "C" fn(Object, Hp2PropertyTagInteger, *mut i32) -> i32,
    pub kind: unsafe extern "C" fn(Object, Hp2PropertyTagKind) -> i32,
    pub count_scope:
        unsafe extern "C" fn(Object, Hp2PropertyTagCountContinuation, Object, *mut i32) -> i32,
    pub name: unsafe extern "C" fn(Object, *mut i32, Object) -> i32,
    pub byte: unsafe extern "C" fn(Object, *mut u8) -> i32,
    pub word: unsafe extern "C" fn(Object, *mut u16) -> i32,
    pub int: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub loading: unsafe extern "C" fn(Object) -> i32,
    pub check: unsafe extern "C" fn(i32) -> i32,
    pub serialize_item: unsafe extern "C" fn(Object, Object, Object) -> i32,
}

fn size_info(size: i32) -> u8 {
    match size {
        1 => 0x00,
        2 => 0x10,
        4 => 0x20,
        12 => 0x30,
        16 => 0x40,
        value if value <= 255 => 0x50,
        // The legacy format stores this field as a 16-bit value, including 65536 -> 0.
        value if value <= 65536 => 0x60,
        _ => 0x70,
    }
}

fn index_header(index: i32) -> (u8, [u8; 3], usize) {
    let first = if index <= 127 {
        index as u8
    } else if index <= 16383 {
        ((index >> 8).wrapping_add(0x80)) as u8
    } else {
        ((index >> 24).wrapping_add(0xc0)) as u8
    };
    let rest = if index <= 127 {
        [0; 3]
    } else if index <= 16383 {
        [index as u8, 0, 0]
    } else {
        [(index >> 16) as u8, (index >> 8) as u8, index as u8]
    };
    (first, rest, index_byte_count(first))
}

fn index_byte_count(first: u8) -> usize {
    if first & 0x80 == 0 { 0 } else if first & 0xc0 == 0x80 { 1 } else { 3 }
}

fn decode_index(first: u8, rest: [u8; 3]) -> (i32, usize) {
    if first & 0x80 == 0 {
        (i32::from(first), 0)
    } else if first & 0xc0 == 0x80 {
        (((i32::from(first) & 0x7f) << 8) + i32::from(rest[0]), 1)
    } else {
        (((i32::from(first) & 0x3f) << 24)
            + (i32::from(rest[0]) << 16)
            + (i32::from(rest[1]) << 8)
            + i32::from(rest[2]), 3)
    }
}
fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}
unsafe fn integer(
    s: &Hp2PropertyTagServices,
    property: Object,
    field: Hp2PropertyTagInteger,
) -> Result<i32> {
    let mut result = 0;
    status(unsafe { (s.integer)(property, field, &mut result) })?;
    Ok(result)
}
struct CountContext<'a> {
    tag: *const Hp2PropertyArchiveTag,
    property: Object,
    value: Object,
    services: &'a Hp2PropertyTagServices,
}
unsafe extern "C" fn count_body(context: Object, archive: Object) -> i32 {
    let context = unsafe { &*context.cast::<CountContext<'_>>() };
    unsafe {
        payload(
            context.tag,
            archive,
            context.property,
            context.value,
            context.services,
        )
    }
    .err()
    .unwrap_or(0)
}

/// # Safety
/// Tag storage, property/value objects and synchronous callbacks remain valid through reentry.
pub unsafe fn construct(
    tag: *mut Hp2PropertyArchiveTag,
    archive: Object,
    property: Object,
    index: i32,
    value: Object,
    s: &Hp2PropertyTagServices,
) -> Result {
    // C++ initializes in declaration order: Type, Info, Name, ItemName, Size, ArrayIndex.
    unsafe {
        (*tag).property_type = integer(s, property, Hp2PropertyTagInteger::TagPropertyType)? as u8;
    }
    unsafe {
        (*tag).info = integer(s, property, Hp2PropertyTagInteger::TagPropertyType)? as u8;
    }
    unsafe {
        (*tag).name = integer(s, property, Hp2PropertyTagInteger::TagPropertyName)?;
    }
    unsafe {
        (*tag).item_name = 0;
        (*tag).size = 0;
        (*tag).array_index = index;
    }
    if unsafe { (s.kind)(property, Hp2PropertyTagKind::TagIsStruct) } != 0 {
        unsafe {
            (*tag).item_name = integer(s, property, Hp2PropertyTagInteger::TagStructName)?;
        }
    }
    let mut context = CountContext {
        tag,
        property,
        value,
        services: s,
    };
    let mut size = 0;
    status(unsafe {
        (s.count_scope)(
            archive,
            count_body,
            (&mut context as *mut CountContext<'_>).cast(),
            &mut size,
        )
    })?;
    unsafe {
        (*tag).size = size;
    }
    let size_info = size_info(size);
    unsafe {
        (*tag).info |= size_info;
    }
    let is_bool = unsafe { (s.kind)(property, Hp2PropertyTagKind::TagIsBool) } != 0;
    if unsafe { (*tag).array_index } != 0
        || (is_bool
            && unsafe {
                *value.cast::<u32>()
                    & integer(s, property, Hp2PropertyTagInteger::TagBitMask)? as u32
                    != 0
            })
    {
        unsafe {
            (*tag).info |= 0x80;
        }
    }
    Ok(())
}

/// # Safety
/// Name/Info and any explicit size/index fields are initialized by successful archive reads.
/// For loading, other incoming tag fields may be uninitialized and are never read here.
pub unsafe fn wire(
    tag: *mut Hp2PropertyArchiveTag,
    archive: Object,
    previous_name: Object,
    s: &Hp2PropertyTagServices,
) -> Result {
    status(unsafe { (s.name)(archive, &raw mut (*tag).name, previous_name) })?;
    if unsafe { (*tag).name } == 0 {
        return Ok(());
    }
    // Archive mode is a native flag getter, not a virtual serialization operation.
    let loading = unsafe { (s.loading)(archive) } != 0;
    status(unsafe { (s.byte)(archive, &raw mut (*tag).info) })?;
    unsafe {
        (*tag).property_type = (*tag).info & 0x0f;
    }
    if unsafe { (*tag).property_type } as i32 == s.struct_property {
        status(unsafe { (s.name)(archive, &raw mut (*tag).item_name, std::ptr::null_mut()) })?;
    }
    match unsafe { (*tag).info } & 0x70 {
        0x00 => unsafe {
            (*tag).size = 1;
        },
        0x10 => unsafe {
            (*tag).size = 2;
        },
        0x20 => unsafe {
            (*tag).size = 4;
        },
        0x30 => unsafe {
            (*tag).size = 12;
        },
        0x40 => unsafe {
            (*tag).size = 16;
        },
        0x50 => {
            let mut size = if loading {
                0
            } else {
                (unsafe { (*tag).size }) as u8
            };
            status(unsafe { (s.byte)(archive, &mut size) })?;
            unsafe {
                (*tag).size = size as i32;
            }
        }
        0x60 => {
            let mut size = if loading {
                0
            } else {
                (unsafe { (*tag).size }) as u16
            };
            status(unsafe { (s.word)(archive, &mut size) })?;
            unsafe {
                (*tag).size = size as i32;
            }
        }
        _ => {
            let mut size = if loading { 0 } else { unsafe { (*tag).size } };
            status(unsafe { (s.int)(archive, &mut size) })?;
            unsafe {
                (*tag).size = size;
            }
        }
    }
    if unsafe { (*tag).info } & 0x80 != 0
        && unsafe { (*tag).property_type } as i32 != s.bool_property
    {
        let original = if loading { 0 } else { unsafe { (*tag).array_index } };
        let (mut first, mut rest, mut count) = index_header(original);
        status(unsafe { (s.byte)(archive, &mut first) })?;
        if loading {
            count = index_byte_count(first);
        }
        for byte in rest.iter_mut().take(count) {
            status(unsafe { (s.byte)(archive, byte) })?;
        }
        let (index, _) = decode_index(first, rest);
        unsafe { (*tag).array_index = index; }
    } else {
        unsafe { (*tag).array_index = 0; }
    }
    Ok(())
}

/// # Safety
/// The native property kind/mask and value addresses match the supplied objects.
pub unsafe fn payload(
    tag: *const Hp2PropertyArchiveTag,
    archive: Object,
    property: Object,
    value: Object,
    s: &Hp2PropertyTagServices,
) -> Result {
    if unsafe { (s.kind)(property, Hp2PropertyTagKind::TagIsExactBool) } != 0 {
        let mask = unsafe { integer(s, property, Hp2PropertyTagInteger::TagBitMask) }? as u32;
        if mask == 0 {
            status(unsafe { (s.check)(0) })?;
        }
        if unsafe { (s.loading)(archive) } != 0 {
            unsafe {
                let bits = value.cast::<u32>();
                *bits = if (*tag).info & 0x80 != 0 {
                    *bits | mask
                } else {
                    *bits & !mask
                };
            }
        }
        Ok(())
    } else {
        status(unsafe { (s.serialize_item)(property, archive, value) })
    }
}

/// Constructs a tag by counting its payload through the real Rust archive.
///
/// # Safety
/// `property` and `value` identify live property metadata and its selected element.
pub(crate) unsafe fn construct_core(
    access: crate::core_access::CoreAccess<'_>,
    archive: &crate::object_archive::ObjectArchive<'_>,
    property: *mut hp2_abi::engine::generated::UProperty,
    index: i32,
    value: *mut c_void,
) -> crate::core_access::RuntimeResult<Hp2PropertyArchiveTag> {
    let class_index = unsafe { crate::core_property::property_id(access, property)? };
    let name = unsafe { i32::from_ne_bytes((*property.cast::<hp2_abi::engine::generated::UObject>()).name) };
    let kind = unsafe { crate::core_property::value_kind(access, property)? };
    let mut tag = Hp2PropertyArchiveTag { property_type: class_index as u8, info: class_index as u8, name, item_name: 0, size: 0, array_index: index };
    if kind == crate::property_value::Hp2ValueKind::ValueStruct {
        let structure = unsafe { (*property.cast::<hp2_abi::engine::generated::UStructProperty>()).r#struct };
        if structure.is_null() { return Err(crate::core_access::RuntimeError::new("Null struct property type")); }
        tag.item_name = unsafe { i32::from_ne_bytes((*structure.cast::<hp2_abi::engine::generated::UObject>()).name) };
    }
    let counter = archive.counting();
    let result = unsafe { access.property_value(crate::property_value::Hp2ValueOperation::ValueSerialize, property, value, ptr::null_mut(), 0, Some(&counter))? };
    if result == 0 { return Err(crate::core_access::RuntimeError::new("Property size counting failed")); }
    tag.size = i32::try_from(counter.tell()?).map_err(|_| crate::core_access::RuntimeError::new("Property payload size overflow"))?;
    tag.info |= size_info(tag.size);
    if index != 0 { tag.info |= 0x80; }
    if kind == crate::property_value::Hp2ValueKind::ValueBool
        && unsafe { *value.cast::<u32>() & (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as u32 != 0 }
    {
        tag.info |= 0x80;
    }
    Ok(tag)
}

/// Reads or writes one legacy tag header through the bounded Rust archive.
///
/// # Safety
/// Saving requires a fully initialized tag. Loading may initialize only the
/// terminator name before returning for the terminal record.
pub(crate) unsafe fn wire_core(
    access: crate::core_access::CoreAccess<'_>,
    tag: *mut Hp2PropertyArchiveTag,
    archive: &crate::object_archive::ObjectArchive<'_>,
) -> crate::core_access::RuntimeResult<()> {
    if tag.is_null() { return Err(crate::core_access::RuntimeError::new("Null property tag")); }
    let loading = archive.is_loading();
    unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*tag).name))?; }
    if unsafe { (*tag).name } == 0 { return Ok(()); }
    let mut info = if loading { 0 } else { unsafe { (*tag).info } };
    archive.serialize_u8(&mut info)?;
    unsafe {
        (*tag).info = info;
        (*tag).property_type = info & 0x0f;
    }
    let struct_id = unsafe { i32::from_ne_bytes((*access.native_class("StructProperty")?.cast::<hp2_abi::engine::generated::UObject>()).name) };
    if i32::from(unsafe { (*tag).property_type }) == struct_id {
        archive.serialize_name_slot(unsafe { ptr::addr_of_mut!((*tag).item_name) })?;
    }
    let mut size = if loading { 0 } else { unsafe { (*tag).size } };
    match info & 0x70 {
        0x00 => size = 1,
        0x10 => size = 2,
        0x20 => size = 4,
        0x30 => size = 12,
        0x40 => size = 16,
        0x50 => { let mut v = size as u8; archive.serialize_u8(&mut v)?; size = i32::from(v); }
        0x60 => { let mut v = size as u16; archive.serialize_u16(&mut v)?; size = i32::from(v); }
        _ => archive.serialize_i32(&mut size)?,
    }
    unsafe { (*tag).size = size; }
    let bool_id = unsafe { i32::from_ne_bytes((*access.native_class("BoolProperty")?.cast::<hp2_abi::engine::generated::UObject>()).name) };
    if info & 0x80 != 0 && i32::from(unsafe { (*tag).property_type }) != bool_id {
        let original = if loading { 0 } else { unsafe { (*tag).array_index } };
        let (mut first, mut rest, mut count) = index_header(original);
        archive.serialize_u8(&mut first)?;
        if loading {
            count = index_byte_count(first);
        }
        for byte in rest.iter_mut().take(count) {
            archive.serialize_u8(byte)?;
        }
        let (decoded, _) = decode_index(first, rest);
        unsafe { (*tag).array_index = decoded; }
    } else {
        unsafe { (*tag).array_index = 0; }
    }
    Ok(())
}

/// Serializes a tag payload, preserving the packed UBoolProperty encoding.
///
/// # Safety
/// `tag`, `property`, and `value` remain live for the synchronous operation.
pub(crate) unsafe fn payload_core(
    access: crate::core_access::CoreAccess<'_>,
    tag: *const Hp2PropertyArchiveTag,
    archive: &crate::object_archive::ObjectArchive<'_>,
    property: *mut hp2_abi::engine::generated::UProperty,
    value: *mut c_void,
) -> crate::core_access::RuntimeResult<()> {
    let kind = unsafe { crate::core_property::value_kind(access, property)? };
    let class = unsafe { (*property.cast::<hp2_abi::engine::generated::UObject>()).class };
    let bool_class = access.native_class("BoolProperty")?;
    if kind == crate::property_value::Hp2ValueKind::ValueBool && class == bool_class.cast() {
        let mask = unsafe { (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as u32 };
        if mask == 0 { return Err(crate::core_access::RuntimeError::new("Invalid boolean property bit mask")); }
        if archive.is_loading() {
            unsafe {
                let bits = value.cast::<u32>();
                *bits = if (*tag).info & 0x80 != 0 { *bits | mask } else { *bits & !mask };
            }
        }
        return Ok(());
    }
    let result = unsafe {
        access.property_value(
            crate::property_value::Hp2ValueOperation::ValueSerialize,
            property,
            value,
            ptr::null_mut(),
            0,
            Some(archive),
        )?
    };
    if result == 0 {
        Err(crate::core_access::RuntimeError::new("Property payload serialization failed"))
    } else {
        Ok(())
    }
}

