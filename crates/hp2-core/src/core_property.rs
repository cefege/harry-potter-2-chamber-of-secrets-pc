//! Direct reflected-property operations over canonical Core records and archives.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::NativeRecordKind;
use crate::object_archive::ObjectArchive;
use crate::property_value::{self, Hp2ValueKind, Hp2ValueOperation, PropertyValueOps};
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UObject, UObjectProperty, UProperty, UStructProperty};
use std::ffi::c_void;
use std::ptr;

type Object = *mut c_void;

fn value_error(code: i32) -> RuntimeError {
    RuntimeError::new("Core property operation failed").with_context(&format!("status {code}"))
}

/// Selects the value policy from the actual canonical property class.
///
/// # Safety
/// `property` must point to a live reflected property record in Core storage.
pub(crate) unsafe fn value_kind(access: CoreAccess<'_>, property: *mut UProperty) -> RuntimeResult<Hp2ValueKind> {
    if property.is_null() {
        return Err(RuntimeError::new("Null property"));
    }
    let kind = unsafe { access.record_kind(property.cast::<UObject>())? };
    Ok(match kind {
        NativeRecordKind::ByteProperty => Hp2ValueKind::ValueByte,
        NativeRecordKind::IntProperty => Hp2ValueKind::ValueInt,
        NativeRecordKind::BoolProperty => Hp2ValueKind::ValueBool,
        NativeRecordKind::FloatProperty => Hp2ValueKind::ValueFloat,
        NativeRecordKind::ObjectProperty | NativeRecordKind::ClassProperty => Hp2ValueKind::ValueObject,
        NativeRecordKind::NameProperty => Hp2ValueKind::ValueName,
        NativeRecordKind::StringProperty => Hp2ValueKind::ValueString,
        NativeRecordKind::FixedArrayProperty => Hp2ValueKind::ValueFixed,
        NativeRecordKind::ArrayProperty => Hp2ValueKind::ValueArray,
        NativeRecordKind::MapProperty => Hp2ValueKind::ValueMap,
        NativeRecordKind::StructProperty => Hp2ValueKind::ValueStruct,
        NativeRecordKind::Property => Hp2ValueKind::ValueGeneric,
        _ => return Err(RuntimeError::new("Invalid property class")),
    })
}

struct DirectPropertyRecordOps<'access, 'archive, 'data> {
    access: CoreAccess<'access>,
    property: *mut UProperty,
    archive: &'archive ObjectArchive<'data>,
}

impl crate::native_property_offsets::PropertyRecordOps for DirectPropertyRecordOps<'_, '_, '_> {
    type Error = RuntimeError;

    fn integer(&self, field: crate::native_property_offsets::Hp2NativePropertyInteger) -> RuntimeResult<i32> {
        use crate::native_property_offsets::Hp2NativePropertyInteger::*;
        // SAFETY: the synchronous serializer owns a live canonical property.
        Ok(unsafe { match field {
            Offset => (*self.property).offset,
            CppProperty => (*self.property).cpp_property,
            Flags => (*self.property).property_flags as i32,
            Loading => i32::from(self.archive.is_loading()),
            Version => self.archive.flags().version as i32,
            Name => i32::from_ne_bytes((*self.property.cast::<UObject>()).name),
        } })
    }

    fn archive(&self, field: crate::native_property_offsets::Hp2NativePropertyWire) -> RuntimeResult<()> {
        use crate::native_property_offsets::Hp2NativePropertyWire::*;
        // SAFETY: raw field slots remain live over mapped-name/object reentry.
        unsafe { match field {
            Super => self.access.serialize_metadata(
                self.property.cast(), self.archive, crate::class_metadata::Hp2MetadataKind::MetadataField,
            )?,
            ArrayDim => {
                let mut value = (*self.property).array_dim;
                self.archive.serialize_i32(&mut value)?;
                (*self.property).array_dim = value;
            }
            Flags => {
                let mut value = (*self.property).property_flags;
                self.archive.serialize_u32(&mut value)?;
                (*self.property).property_flags = value;
            }
            Category => self.archive.serialize_name_slot(ptr::addr_of_mut!((*self.property).category).cast())?,
            RepOffset => {
                let mut value = (*self.property).rep_offset;
                self.archive.serialize_u16(&mut value)?;
                (*self.property).rep_offset = value;
            }
        } }
        Ok(())
    }

    fn registered_offset(&self) -> RuntimeResult<Option<i32>> {
        // Native declarations are immutable descriptions of these same runtime
        // objects; no process-global legacy offset registry is consulted.
        let (owner, name) = unsafe {
            ((*self.property.cast::<UObject>()).outer, i32::from_ne_bytes((*self.property.cast::<UObject>()).name))
        };
        if let Some(offset) = self.access.cpp_property_offset(owner.cast(), name) {
            return Ok(Some(offset));
        }
        if owner.is_null() || unsafe { self.access.record_kind(owner.cast())? } != NativeRecordKind::Class {
            return Ok(None);
        }
        let metadata = unsafe { self.access.class_metadata(owner.cast())? };
        let Some(spec) = metadata.native_spec else { return Ok(None); };
        for declaration in spec.properties {
            if self.access.intern_str(declaration.name, crate::name_owner::FindName::Find)? == name {
                return Ok(Some(declaration.offset as i32));
            }
        }
        Ok(None)
    }

    fn set_flags(&self, flags: u32) -> RuntimeResult<()> {
        // SAFETY: the property is live, and no object borrow crosses a hook.
        unsafe { (*self.property).property_flags = flags; }
        Ok(())
    }

    fn publish(&self, offset: i32, cpp: i32) -> RuntimeResult<()> {
        // SAFETY: only UProperty's load-fixup slots are changed.
        unsafe {
            (*self.property).offset = offset;
            (*self.property).cpp_property = cpp;
            (*self.property).constructor_link_next = ptr::null_mut();
        }
        Ok(())
    }
}

/// Serializes UProperty's inherited record and the selected subtype's metadata.
///
/// # Safety
/// `property` is a live canonical record of `kind`; mapped references and the
/// archive stay live through recursive loading. No body borrow crosses hooks.
pub(crate) unsafe fn serialize_record(
    access: CoreAccess<'_>, property: *mut UProperty, archive: &ObjectArchive<'_>, kind: NativeRecordKind,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{UArrayProperty, UByteProperty, UClassProperty, UFixedArrayProperty, UMapProperty, UObjectProperty};
    let ops = DirectPropertyRecordOps { access, property, archive };
    crate::native_property_offsets::serialize_with(&ops)?;
    // SAFETY: the selected kind establishes each frozen subtype layout. Object
    // mapping receives raw slots, never a Rust reference into a live body.
    unsafe {
        match kind {
            NativeRecordKind::ByteProperty => {
                archive.serialize_object_slot(ptr::addr_of_mut!((*property.cast::<UByteProperty>()).r#enum).cast())?;
            }
            NativeRecordKind::BoolProperty if !archive.is_loading() && !archive.is_saving() => {
                let mut mask = (*property.cast::<UBoolProperty>()).bit_mask;
                archive.serialize_u32(&mut mask)?;
                (*property.cast::<UBoolProperty>()).bit_mask = mask;
            }
            NativeRecordKind::ObjectProperty | NativeRecordKind::ClassProperty => {
                archive.serialize_object_slot(ptr::addr_of_mut!((*property.cast::<UObjectProperty>()).property_class).cast())?;
                if kind == NativeRecordKind::ClassProperty {
                    archive.serialize_object_slot(ptr::addr_of_mut!((*property.cast::<UClassProperty>()).meta_class).cast())?;
                    if (*property.cast::<UClassProperty>()).meta_class.is_null() {
                        return Err(RuntimeError::new("Class property has no MetaClass"));
                    }
                }
            }
            NativeRecordKind::FixedArrayProperty => {
                let fixed = property.cast::<UFixedArrayProperty>();
                archive.serialize_object_slot(ptr::addr_of_mut!((*fixed).inner).cast())?;
                let mut count = (*fixed).count;
                archive.serialize_i32(&mut count)?;
                (*fixed).count = count;
                if (*fixed).inner.is_null() { return Err(RuntimeError::new("Fixed array property has no Inner")); }
            }
            NativeRecordKind::ArrayProperty => {
                let array = property.cast::<UArrayProperty>();
                archive.serialize_object_slot(ptr::addr_of_mut!((*array).inner).cast())?;
                if (*array).inner.is_null() { return Err(RuntimeError::new("Array property has no Inner")); }
            }
            NativeRecordKind::MapProperty => {
                let map = property.cast::<UMapProperty>();
                archive.serialize_object_slot(ptr::addr_of_mut!((*map).key).cast())?;
                archive.serialize_object_slot(ptr::addr_of_mut!((*map).value).cast())?;
                if (*map).key.is_null() { return Err(RuntimeError::new("Map property has no Key")); }
                if (*map).value.is_null() { return Err(RuntimeError::new("Map property has no Value")); }
            }
            NativeRecordKind::StructProperty => {
                archive.serialize_object_slot(ptr::addr_of_mut!((*property.cast::<UStructProperty>()).r#struct).cast())?;
            }
            _ => {}
        }
    }
    Ok(())
}

struct DirectValueOps<'access, 'archive, 'data> {
    access: CoreAccess<'access>,
    archive: Option<&'archive ObjectArchive<'data>>,
    none_name: i32,
    vector_name: i32,
    rotator_name: i32,
    plane_name: i32,
    map: *mut crate::core_records::PackageMapRecord,
}

impl PropertyValueOps for DirectValueOps<'_, '_, '_> {
    type Error = RuntimeError;

    fn none_name(&self) -> i32 { self.none_name }
    fn vector_name(&self) -> i32 { self.vector_name }
    fn rotator_name(&self) -> i32 { self.rotator_name }
    fn plane_name(&self) -> i32 { self.plane_name }
    fn array_tag(&self) -> *const c_void { b"CorePropertyArray\0".as_ptr().cast() }
    fn string_tag(&self) -> *const c_void { b"CorePropertyString\0".as_ptr().cast() }

    unsafe fn pointer(&self, object: Object, field: property_value::Hp2ValuePointer) -> RuntimeResult<Object> {
        use property_value::Hp2ValuePointer::*;
        if object.is_null() { return Err(RuntimeError::new("Null property pointer source")); }
        let result = unsafe { match field {
            ValueSuper => (*object.cast::<hp2_abi::engine::generated::UField>()).super_field,
            ValuePropertyClass => (*object.cast::<UObject>()).class,
        }};
        Ok(result)
    }

    unsafe fn call(&self, property: Object, _context: Object, operation: property_value::Hp2ValueCall, a: Object, b: Object) -> RuntimeResult<i32> {
        use property_value::Hp2ValueCall::*;
        if property.is_null() { return Err(RuntimeError::new("Null property operation")); }
        let property = property.cast::<UProperty>();
        match operation {
            ValueCallPreload => {
                if let Some(archive) = self.archive.filter(|archive| archive.is_trans()) {
                    unsafe { archive.preload_transaction_object(property.cast())?; }
                } else {
                    unsafe { self.access.preload(property.cast())?; }
                }
                Ok(1)
            }
            ValueCallBinary => {
                let archive = self.archive.ok_or_else(|| RuntimeError::new("Property binary archive is unavailable"))?;
                unsafe { self.access.binary_properties(property.cast(), archive, a.cast())?; }
                Ok(1)
            }
            ValueCallObjectNet => {
                let archive = self.archive.ok_or_else(|| RuntimeError::new("Property net archive is unavailable"))?;
                // SAFETY: this operation is selected only for a live object
                // property; the caller keeps both its value and class live.
                unsafe {
                    let class = (*property.cast::<UObjectProperty>()).property_class.cast();
                    archive.serialize_net_object(a.cast(), class)
                }
            }
            ValueCallMapIndex => {
                if self.map.is_null() {
                    return Err(RuntimeError::new("Net property serialization requires a package map"));
                }
                unsafe { crate::core_array::object_to_index(self.map, property.cast()) }
            }
            ValueCallNetSerialize => {
                let map = self.map;
                let archive = self.archive.ok_or_else(|| RuntimeError::new("Property net archive is unavailable"))?;
                unsafe { self.access.property_net_serialize(property, map, archive, a, b, 0) }
            }
            ValueCallIdentical | ValueCallCopySingle | ValueCallCopyComplete | ValueCallDestroy | ValueCallSerialize => {
                let op = match operation {
                    ValueCallIdentical => Hp2ValueOperation::ValueIdentical,
                    ValueCallCopySingle => Hp2ValueOperation::ValueCopySingle,
                    ValueCallCopyComplete => Hp2ValueOperation::ValueCopyComplete,
                    ValueCallDestroy => Hp2ValueOperation::ValueDestroy,
                    ValueCallSerialize => Hp2ValueOperation::ValueSerialize,
                    _ => unreachable!(),
                };
                unsafe { self.access.property_value(op, property, a, b, 0, self.archive) }
            }
        }
    }

    unsafe fn integer(&self, _archive: Object, field: property_value::Hp2ValueInteger) -> RuntimeResult<i32> {
        match field {
            property_value::Hp2ValueInteger::ValueLoading => Ok(i32::from(self.archive.is_some_and(ObjectArchive::is_loading))),
        }
    }

    unsafe fn wire(&self, _archive: Object, wire: property_value::Hp2ValueWire, value: Object, argument: u32) -> RuntimeResult<()> {
        use property_value::Hp2ValueWire::*;
        let archive = self.archive.ok_or_else(|| RuntimeError::new("Property wire archive is unavailable"))?;
        if value.is_null() { return Err(RuntimeError::new("Null property wire value")); }
        unsafe {
            match wire {
                ValueWireByte => archive.serialize_u8(&mut *value.cast::<u8>())?,
                ValueWireInt => archive.serialize_i32(&mut *value.cast::<i32>())?,
                ValueWireFloat => archive.serialize_f32(&mut *value.cast::<f32>())?,
                ValueWireObject => archive.serialize_object_slot(value.cast())?,
                ValueWireName => archive.serialize_name_slot(value.cast())?,
                ValueWireString => {
                    let raw = value.cast::<TArrayRaw>();
                    if archive.mode() == crate::object_archive::ArchiveMode::MemoryCount {
                        let count = usize::try_from(ptr::addr_of!((*raw).num).read_unaligned())
                            .map_err(|_| RuntimeError::new("Invalid FString element count"))?;
                        let capacity = usize::try_from(ptr::addr_of!((*raw).max).read_unaligned())
                            .map_err(|_| RuntimeError::new("Invalid FString capacity"))?;
                        archive.count_bytes(count * std::mem::size_of::<u32>(), capacity * std::mem::size_of::<u32>())?;
                    } else {
                        self.access.serialize_owned_fstring(archive, raw)?;
                    }
                }
                ValueWireShort => archive.serialize_u16(&mut *value.cast::<u16>())?,
                ValueWireBits => {
                    let bytes = usize::try_from(argument.saturating_add(7) / 8)
                        .map_err(|_| RuntimeError::new("Invalid property bit count"))?;
                    if bytes > 8 { return Err(RuntimeError::new("Property bit count exceeds scalar wire width")); }
                    let mut padded = [0u8; 9];
                    ptr::copy_nonoverlapping(value.cast::<u8>(), padded.as_mut_ptr(), bytes);
                    archive.serialize_bits(&mut padded, argument)?;
                    ptr::copy_nonoverlapping(padded.as_ptr(), value.cast::<u8>(), bytes);
                }
                ValueWireBounded => archive.serialize_bounded(&mut *value.cast::<u32>(), argument)?,
                ValueWireIndex => archive.serialize_compact_index(&mut *value.cast::<i32>())?,
            }
        }
        Ok(())
    }

    unsafe fn storage(&self, kind: property_value::Hp2ValueStorage, value: Object) -> RuntimeResult<()> {
        if value.is_null() { return Err(RuntimeError::new("Null typed property value")); }
        let allocator = crate::array_storage::system_allocator();
        match kind {
            property_value::Hp2ValueStorage::ValueStringDestroy | property_value::Hp2ValueStorage::ValueArrayDestroy => unsafe {
                let raw = value.cast::<TArrayRaw>();
                let slots = crate::array_storage::Hp2ArraySlots {
                    data: ptr::addr_of_mut!((*raw).data), num: ptr::addr_of_mut!((*raw).num), max: ptr::addr_of_mut!((*raw).max),
                };
                crate::array_storage::release(slots, allocator).map_err(value_error)
            },
        }
    }

    unsafe fn array_check_add(&self, count: i32, num: i32, max: i32) -> RuntimeResult<()> {
        let total = num.checked_add(count).ok_or_else(|| RuntimeError::new("Array size overflow"))?;
        if count < 0 || num < 0 || max < num || total < 0 { return Err(RuntimeError::new("Array size out of range")); }
        Ok(())
    }
    unsafe fn checked_int_size(&self, bytes: usize) -> RuntimeResult<i32> {
        let services = crate::array_storage::system_allocator();
        let mut size = 0;
        let status = unsafe { (services.checked_int_size)(services.context, bytes, &mut size) };
        if status == 0 { Ok(size) } else { Err(value_error(status)) }
    }
    unsafe fn array_reallocate(&self, data: Object, bytes: usize, max: *const i32, width: i32, tag: *const c_void) -> RuntimeResult<Object> {
        if max.is_null() { return Err(RuntimeError::new("Null array capacity")); }
        let services = crate::array_storage::system_allocator();
        let mut output = ptr::null_mut();
        let status = unsafe { (services.realloc)(services.context, data, bytes, max, width, tag, &mut output) };
        if status == 0 { Ok(output) } else { Err(value_error(status)) }
    }
    unsafe fn array_free(&self, data: Object) -> RuntimeResult<()> {
        if data.is_null() { return Ok(()); }
        let services = crate::array_storage::system_allocator();
        let status = unsafe { (services.free)(services.context, data) };
        if status == 0 { Ok(()) } else { Err(value_error(status)) }
    }
}

impl CoreAccess<'_> {
    /// Executes one reflected value operation using canonical Core metadata.
    ///
    /// # Safety
    /// Property and value pointers must be live, correctly aligned reflected records/storage.
    pub unsafe fn property_value(
        self,
        operation: Hp2ValueOperation,
        property: *mut UProperty,
        a: *mut c_void,
        b: *mut c_void,
        index: i32,
        archive: Option<&ObjectArchive<'_>>,
    ) -> RuntimeResult<i32> {
        let kind = unsafe { value_kind(self, property)? };
        let ops = DirectValueOps {
            access: self,
            archive,
            none_name: self.intern_str("None", crate::name_owner::FindName::Find)?,
            vector_name: self.intern_str("Vector", crate::name_owner::FindName::Find)?,
            rotator_name: self.intern_str("Rotator", crate::name_owner::FindName::Find)?,
            plane_name: self.intern_str("Plane", crate::name_owner::FindName::Find)?,
            map: ptr::null_mut(),
        };
        unsafe { property_value::run_with(kind, operation, property.cast(), ptr::null_mut(), a, b, index, &ops) }
    }

    /// Runs network property serialization with the canonical package-map owner.
    ///
    /// # Safety
    /// `property`, `map`, and value storage must remain live through archive
    /// callbacks. The map must be the engine's current package-map record.
    pub unsafe fn property_net_serialize(
        self,
        property: *mut UProperty,
        map: *mut crate::core_records::PackageMapRecord,
        archive: &ObjectArchive<'_>,
        a: *mut c_void,
        b: *mut c_void,
        index: i32,
    ) -> RuntimeResult<i32> {
        let kind = unsafe { value_kind(self, property)? };
        let ops = DirectValueOps {
            access: self,
            archive: Some(archive),
            none_name: self.intern_str("None", crate::name_owner::FindName::Find)?,
            vector_name: self.intern_str("Vector", crate::name_owner::FindName::Find)?,
            rotator_name: self.intern_str("Rotator", crate::name_owner::FindName::Find)?,
            plane_name: self.intern_str("Plane", crate::name_owner::FindName::Find)?,
            map,
        };
        unsafe {
            property_value::run_with(
                kind,
                Hp2ValueOperation::ValueNetSerialize,
                property.cast(),
                map.cast(),
                a,
                b,
                index,
                &ops,
            )
        }
    }
    /// Returns the reflected property's address within its stable object allocation.
    ///
    /// # Safety
    /// Both pointers identify live canonical records, and the property's offset
    /// lies inside the object's complete allocation.
    pub unsafe fn property_address(self, object: *mut UObject, property: *mut UProperty) -> RuntimeResult<*mut u8> {
        if object.is_null() || property.is_null() {
            return Err(RuntimeError::new("Null reflected property address"));
        }
        let offset = unsafe { (*property).offset };
        if offset < 0 {
            return Err(RuntimeError::new("Negative reflected property offset"));
        }
        Ok(object.cast::<u8>().wrapping_add(offset as usize))
    }
}

/// Serializes one already-addressed property value using its Rust-owned type policy.
///
/// # Safety
/// `property` and `value` must identify live property metadata and its indexed value storage.
pub(crate) unsafe fn serialize_property_value(access: CoreAccess<'_>, property: *mut UProperty, archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let result = unsafe { access.property_value(Hp2ValueOperation::ValueSerialize, property, value, ptr::null_mut(), 0, Some(archive))? };
    if result == 0 { Err(RuntimeError::new("Property serialization failed")) } else { Ok(()) }
}

impl CoreAccess<'_> {
/// Serializes reflected binary properties into the caller's complete class record.
///
/// # Safety
/// `structure` is live and `data` addresses its property storage.
pub unsafe fn binary_properties(self, structure: *mut hp2_abi::engine::generated::UStruct, archive: &ObjectArchive<'_>, data: *mut u8) -> RuntimeResult<()> {
    let root = ArchiveRoot::new(self, archive, structure.cast(), data.cast(), ptr::null_mut(), 0)?;
    let ops = DirectArchiveOps { root: &root };
    unsafe { crate::property_archive::binary_with(structure.cast(), (&root as *const ArchiveRoot).cast_mut().cast(), data.cast(), &ops) }
}

/// Serializes reflected tagged properties using class-owned defaults.
///
/// # Safety
/// `structure`, `data`, and optional defaults are live records with their declared byte extents.
pub unsafe fn tagged_properties(self, structure: *mut hp2_abi::engine::generated::UStruct, archive: &ObjectArchive<'_>, data: *mut u8, defaults: *const u8, defaults_count: i32) -> RuntimeResult<()> {
    let (defaults, defaults_count) = if defaults.is_null()
        && unsafe { self.record_kind(structure.cast::<UObject>())? } == NativeRecordKind::Class
    {
        let record = structure.cast::<crate::core_records::ClassRecord>();
        let slots = unsafe { ptr::addr_of!((*record).defaults).read_unaligned() };
        (unsafe { slots.data.cast() }, slots.num)
    } else {
        (defaults.cast_mut(), defaults_count)
    };
    let root = ArchiveRoot::new(self, archive, structure.cast(), data.cast(), defaults.cast(), defaults_count)?;
    let ops = DirectArchiveOps { root: &root };
    unsafe { crate::property_archive::tagged_with_defaults(
        structure.cast(), (&root as *const ArchiveRoot).cast_mut().cast(), data.cast(),
        archive.is_persistent(), defaults.cast(), defaults_count, &ops,
    ) }
}

}

struct ArchiveRoot<'access, 'archive, 'data> {
    access: CoreAccess<'access>,
    archive: &'archive ObjectArchive<'data>,
    structure: *mut UObject,
    data: *mut u8,
    defaults: *mut c_void,
    defaults_count: i32,
    string_property: i32,
    str_property: i32,
    struct_property: i32,
    property_name: std::cell::Cell<i32>,
    array_index: std::cell::Cell<i32>,
}

impl<'access, 'archive, 'data> ArchiveRoot<'access, 'archive, 'data> {
    fn new(
        access: CoreAccess<'access>,
        archive: &'archive ObjectArchive<'data>,
        structure: *mut UObject,
        data: *mut u8,
        defaults: *mut c_void,
        defaults_count: i32,
    ) -> RuntimeResult<Self> {
        let string_property = class_name_id(access, "StringProperty")?;
        let str_property = class_name_id(access, "StrProperty")?;
        let struct_property = class_name_id(access, "StructProperty")?;
        Ok(Self {
            access, archive, structure, data, defaults, defaults_count,
            string_property, str_property, struct_property,
            property_name: std::cell::Cell::new(0),
            array_index: std::cell::Cell::new(-1),
        })
    }
}

pub(crate) fn class_name_id(access: CoreAccess<'_>, name: &str) -> RuntimeResult<i32> {
    access.intern_str(name, crate::name_owner::FindName::Find)
}

fn name_string(access: CoreAccess<'_>, index: i32) -> RuntimeResult<String> {
    Ok(access.name_text(index)?.into_iter().map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect())
}

fn object_full_name(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<String> {
    if object.is_null() { return Err(RuntimeError::new("Null object in property warning")); }
    let class = unsafe { (*object).class.cast::<UObject>() };
    let class_name = if class.is_null() { String::new() } else { name_string(access, unsafe { i32::from_ne_bytes((*class).name) })? };
    let mut path = Vec::new();
    let mut current = object;
    while !current.is_null() {
        path.push(name_string(access, unsafe { i32::from_ne_bytes((*current).name) })?);
        current = unsafe { (*current).outer.cast() };
    }
    path.reverse();
    let path = path.join(".");
    if class_name.is_empty() { Ok(path) } else { Ok(format!("{class_name} {path}")) }
}

pub(crate) unsafe fn property_id(access: CoreAccess<'_>, property: *mut UProperty) -> RuntimeResult<i32> {
    if unsafe { access.record_kind(property.cast())? } == NativeRecordKind::ClassProperty {
        class_name_id(access, "ObjectProperty")
    } else {
        let class = unsafe { (*property.cast::<UObject>()).class };
        if class.is_null() { Err(RuntimeError::new("Property has no class")) }
        else { Ok(unsafe { i32::from_ne_bytes((*class.cast::<UObject>()).name) }) }
    }
}

struct TagScope<'root, 'access, 'archive, 'data> {
    root: &'root ArchiveRoot<'access, 'archive, 'data>,
    tag: std::mem::MaybeUninit<crate::property_archive::Hp2PropertyArchiveTag>,
    property_name: std::cell::Cell<i32>,
    array_index: std::cell::Cell<i32>,
}

struct DirectArchiveOps<'root, 'access, 'archive, 'data> {
    root: &'root ArchiveRoot<'access, 'archive, 'data>,
}

impl DirectArchiveOps<'_, '_, '_, '_> {
    unsafe fn tag(&self, context: Object) -> RuntimeResult<*mut crate::property_archive::Hp2PropertyArchiveTag> {
        if context.is_null() { return Err(RuntimeError::new("Null property tag context")); }
        let scope = context.cast::<TagScope<'_, '_, '_, '_>>();
        Ok(unsafe { ptr::addr_of_mut!((*scope).tag).cast() })
    }

}

impl crate::property_archive::PropertyArchiveOps for DirectArchiveOps<'_, '_, '_, '_> {
    type Error = RuntimeError;

    fn string_property(&self) -> i32 { self.root.string_property }
    fn str_property(&self) -> i32 { self.root.str_property }
    fn struct_property(&self) -> i32 { self.root.struct_property }
    unsafe fn pointer(&self, object: Object, field: crate::property_archive::Hp2PropertyArchivePointer) -> RuntimeResult<Object> {
        use crate::property_archive::Hp2PropertyArchivePointer::*;
        if object.is_null() { return Ok(ptr::null_mut()); }
        let result = unsafe { match field {
            ArchiveChildren => (*object.cast::<hp2_abi::engine::generated::UStruct>()).children,
            ArchiveNext => (*object.cast::<hp2_abi::engine::generated::UField>()).next,
            ArchiveSuper => (*object.cast::<hp2_abi::engine::generated::UField>()).super_field,
            ArchivePropertyLink => (*object.cast::<hp2_abi::engine::generated::UStruct>()).property_link,
            ArchivePropertyLinkNext => (*object.cast::<UProperty>()).property_link_next,
        }};
        Ok(result)
    }
    unsafe fn integer(&self, object: Object, field: crate::property_archive::Hp2PropertyArchiveInteger) -> RuntimeResult<i32> {
        use crate::property_archive::Hp2PropertyArchiveInteger::*;
        use hp2_abi::engine::generated::*;
        if object.is_null() {
            return Err(RuntimeError::new("Null property archive integer source"));
        }
        Ok(unsafe { match field {
            ArchiveName => i32::from_ne_bytes((*object.cast::<UObject>()).name),
            ArchiveIsProperty => i32::from(is_property_kind(unsafe { self.root.access.record_kind(object.cast())? })),
            ArchiveArrayDim => (*object.cast::<UProperty>()).array_dim,
            ArchiveOffset => (*object.cast::<UProperty>()).offset,
            ArchiveElementSize => (*object.cast::<UProperty>()).element_size,
            ArchiveType => return unsafe { property_id(self.root.access, object.cast()) },
            ArchiveVersion => i32::from(self.root.archive.flags().version),
            ArchiveLoading => i32::from(self.root.archive.is_loading()),
        }})
    }
    unsafe fn should_serialize(&self, property: Object, _context: Object) -> RuntimeResult<bool> {
        let flags = unsafe { (*property.cast::<UProperty>()).property_flags };
        Ok(flags & 0x1000 == 0 && !(flags & 0x2000 != 0 && self.root.archive.is_persistent()))
    }
    unsafe fn name_matches(&self, object: Object, name: crate::property_archive::Hp2PropertyArchiveName) -> RuntimeResult<bool> {
        let name_index = unsafe { i32::from_ne_bytes((*object.cast::<UObject>()).name) };
        let text = self.root.access.name_text(name_index)?;
        let expected = match name {
            crate::property_archive::Hp2PropertyArchiveName::ArchiveFloatParams => "FloatParams",
            crate::property_archive::Hp2PropertyArchiveName::ArchiveColorParams => "ColorParams",
            crate::property_archive::Hp2PropertyArchiveName::ArchiveRand => "Rand",
            crate::property_archive::Hp2PropertyArchiveName::ArchiveAuxAnims => "AuxAnims",
        };
        Ok(text.len() == expected.len() && text.iter().zip(expected.bytes()).all(|(a, b)| {
            char::from_u32(*a).is_some_and(|c| c.eq_ignore_ascii_case(&(b as char)))
        }))
    }
    unsafe fn struct_name(&self, property: Object) -> RuntimeResult<i32> {
        let structure = unsafe { (*property.cast::<UStructProperty>()).r#struct };
        if structure.is_null() { return Err(RuntimeError::new("Null struct property type")); }
        Ok(unsafe { i32::from_ne_bytes((*structure.cast::<UObject>()).name) })
    }
    unsafe fn defaults(&self, _context: Object) -> RuntimeResult<(Object, i32)> {
        Ok((self.root.defaults, self.root.defaults_count))
    }
    unsafe fn note_name(&self, context: Object, name: i32) -> RuntimeResult<()> {
        if context == (self.root as *const ArchiveRoot<'_, '_, '_>).cast_mut().cast() {
            self.root.property_name.set(name);
        } else {
            unsafe { (*context.cast::<TagScope<'_, '_, '_, '_>>()).property_name.set(name); }
        }
        Ok(())
    }
    unsafe fn note_index(&self, context: Object, index: i32) -> RuntimeResult<()> {
        if context == (self.root as *const ArchiveRoot<'_, '_, '_>).cast_mut().cast() {
            self.root.array_index.set(index);
        } else {
            unsafe { (*context.cast::<TagScope<'_, '_, '_, '_>>()).array_index.set(index); }
        }
        Ok(())
    }
    unsafe fn serialize_item(&self, _context: Object, property: Object, value: Object) -> RuntimeResult<()> {
        unsafe { serialize_property_value(self.root.access, property.cast(), self.root.archive, value) }
    }
    unsafe fn matches(&self, property: Object, data: Object, defaults: Object, index: i32) -> RuntimeResult<i32> {
        unsafe {
            self.root.access.property_value(
                Hp2ValueOperation::ValueMatches,
                property.cast(),
                data,
                defaults,
                index,
                None,
            )
        }
    }
    unsafe fn read_tag(&self, context: Object) -> RuntimeResult<*const crate::property_archive::Hp2PropertyArchiveTag> {
        let tag = unsafe { self.tag(context)? };
        unsafe { crate::property_tag::wire_core(self.root.access, tag, self.root.archive)?; }
        Ok(tag.cast_const())
    }

    unsafe fn write_tag(&self, context: Object) -> RuntimeResult<()> {
        let tag = unsafe { self.tag(context)? };
        unsafe { crate::property_tag::wire_core(self.root.access, tag, self.root.archive) }
    }

    unsafe fn tagged_item(&self, context: Object, property: Object, value: Object) -> RuntimeResult<()> {
        let tag = unsafe { self.tag(context)? };
        unsafe { crate::property_tag::payload_core(self.root.access, tag, self.root.archive, property.cast(), value) }
    }

    unsafe fn read_ansi_byte(&self, _context: Object) -> RuntimeResult<u8> {
        let mut value = 0;
        self.root.archive.serialize_u8(&mut value)?;
        Ok(value)
    }

    unsafe fn assign_fixed_string(&self, context: Object, property: Object, text: *const i32) -> RuntimeResult<()> {
        if text.is_null() { return Err(RuntimeError::new("Null fixed FString text")); }
        let tag = unsafe { self.tag(context)? };
        let property = property.cast::<UProperty>();
        let offset = unsafe { (*property).offset.wrapping_add((*tag).array_index.wrapping_mul((*property).element_size)) };
        let value = unsafe { self.root.data.wrapping_offset(offset as isize) };
        let raw = value.cast::<TArrayRaw>();
        let slots = unsafe {
            crate::array_storage::Hp2ArraySlots {
                data: ptr::addr_of_mut!((*raw).data),
                num: ptr::addr_of_mut!((*raw).num),
                max: ptr::addr_of_mut!((*raw).max),
            }
        };
        let allocator = crate::array_storage::system_allocator();
        unsafe {
            crate::string_storage::assign_with_allocator(
                slots, None, text, false, allocator, b"CorePropertyString\0".as_ptr().cast(),
            )
        }.map_err(value_error)
    }

    unsafe fn warn(&self, context: Object, property: Object, warning: crate::property_archive::Hp2PropertyArchiveWarning) -> RuntimeResult<()> {
        let tag = unsafe { self.tag(context)? };
        let tag_name = name_string(self.root.access, unsafe { (*tag).name })?;
        let struct_name = name_string(self.root.access, unsafe { i32::from_ne_bytes((*self.root.structure).name) })?;
        let mut message = match warning {
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveMissingProperty => {
                format!("Property {tag_name} of {} not found", object_full_name(self.root.access, self.root.structure)?)
            }
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveTypeMismatch => {
                let actual = unsafe { property_id(self.root.access, property.cast())? };
                format!("Type mismatch in {tag_name} of {struct_name}: file {}, class {actual}", (*tag).property_type)
            }
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveArrayBounds => {
                format!("Array bounds in {tag_name} of {struct_name}: {}/{}", unsafe { (*tag).array_index }, unsafe { (*property.cast::<UProperty>()).array_dim })
            }
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveStructMismatch => {
                let actual = unsafe { (*property.cast::<UStructProperty>()).r#struct };
                let actual_name = if actual.is_null() { String::new() } else { name_string(self.root.access, unsafe { i32::from_ne_bytes((*actual.cast::<UObject>()).name) })? };
                let item_name = name_string(self.root.access, unsafe { (*tag).item_name })?;
                format!("Property {tag_name} of {struct_name} struct type mismatch {item_name}/{actual_name}")
            }
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveNotSerializable => {
                format!("Property {tag_name} of {struct_name} is not serialiable")
            }
            crate::property_archive::Hp2PropertyArchiveWarning::ArchiveSkipping => {
                format!("Skipping {} bytes of type {}", unsafe { (*tag).size }, unsafe { (*tag).property_type })
            }
        };
        let message: Vec<u32> = message.drain(..).map(u32::from).collect();
        self.root.access.emit_warning(&message)
    }

    unsafe fn is_xlevel(&self, context: Object) -> RuntimeResult<bool> {
        let tag = unsafe { self.tag(context)? };
        let name = self.root.access.name_text(unsafe { (*tag).name })?;
        let expected = "XLevel";
        Ok(name.len() == expected.len() && name.iter().zip(expected.bytes()).all(|(a, b)| {
            char::from_u32(*a).is_some_and(|c| c.eq_ignore_ascii_case(&(b as char)))
        }))
    }

    unsafe fn read_byte(&self, _context: Object) -> RuntimeResult<()> {
        let mut byte = 0;
        self.root.archive.serialize_u8(&mut byte)
    }

    unsafe fn end_names(&self, _context: Object) -> RuntimeResult<()> {
        let mut none = self.root.access.intern_str("None", crate::name_owner::FindName::Find)?;
        unsafe { self.root.archive.serialize_name_slot(&mut none) }
    }

    unsafe fn stream<F>(&self, _context: Object, _loading: bool, body: F) -> RuntimeResult<()>
    where F: FnOnce() -> RuntimeResult<()> {
        body()
    }

    unsafe fn load_tag_scope<F>(&self, _context: Object, body: F) -> RuntimeResult<()>
    where F: FnOnce(Object) -> RuntimeResult<()> {
        let mut scope = TagScope {
            root: self.root,
            tag: std::mem::MaybeUninit::uninit(),
            property_name: std::cell::Cell::new(0),
            array_index: std::cell::Cell::new(-1),
        };
        body((&mut scope as *mut TagScope<'_, '_, '_, '_>).cast())
    }

    unsafe fn save_tag_scope<F>(&self, _context: Object, property: Object, index: i32, value: Object, body: F) -> RuntimeResult<()>
    where F: FnOnce(Object) -> RuntimeResult<()> {
        let property = property.cast::<UProperty>();
        let tag = unsafe { crate::property_tag::construct_core(self.root.access, self.root.archive, property, index, value)? };
        let name = unsafe { i32::from_ne_bytes((*property.cast::<UObject>()).name) };
        let mut scope = TagScope {
            root: self.root,
            tag: std::mem::MaybeUninit::new(tag),
            property_name: std::cell::Cell::new(name),
            array_index: std::cell::Cell::new(index),
        };
        body((&mut scope as *mut TagScope<'_, '_, '_, '_>).cast())
    }
}

fn is_property_kind(kind: NativeRecordKind) -> bool {
    matches!(
        kind,
        NativeRecordKind::Property
            | NativeRecordKind::ByteProperty
            | NativeRecordKind::IntProperty
            | NativeRecordKind::BoolProperty
            | NativeRecordKind::FloatProperty
            | NativeRecordKind::ObjectProperty
            | NativeRecordKind::ClassProperty
            | NativeRecordKind::NameProperty
            | NativeRecordKind::StringProperty
            | NativeRecordKind::FixedArrayProperty
            | NativeRecordKind::ArrayProperty
            | NativeRecordKind::MapProperty
            | NativeRecordKind::StructProperty
    )
}
