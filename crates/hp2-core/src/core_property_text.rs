//! Direct property-text operations over Rust-owned Core records.

use crate::ansi::Tchar;
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::name_owner::FindName;
use crate::property_text::{self, Hp2TextCall as Call, Hp2TextFormat as Format, Hp2TextInteger as Integer, Hp2TextKind as Kind, Hp2TextOperation as Operation, Hp2TextPointer as Pointer, Hp2TextWarning as Warning, PropertyTextOps};
use hp2_abi::engine::generated::{UArrayProperty, UClass, UField, UObject, UProperty, UStruct};
use std::ffi::c_void;
use std::ptr;
unsafe extern "C" {
    fn wcstol(text: *const Tchar, end: *mut *mut Tchar, base: i32) -> std::ffi::c_long;
}

type Object = *mut c_void;

struct DirectOps<'a> { access: CoreAccess<'a>, scratch: std::cell::RefCell<Vec<u32>>, class_name: i32 }
impl<'a> DirectOps<'a> {
    fn new(access: CoreAccess<'a>) -> RuntimeResult<Self> {
        Ok(Self { access, scratch: std::cell::RefCell::new(Vec::new()), class_name: access.intern_str("Class", FindName::Find)? })
    }
}

fn text_units(text: *const Tchar) -> RuntimeResult<Vec<u32>> {
    if text.is_null() { return Err(RuntimeError::new("Null property text")); }
    let mut result = Vec::new();
    let mut current = text;
    // SAFETY: the parser supplies a NUL-terminated TCHAR string.
    unsafe { while *current != 0 { result.push(*current as u32); current = current.add(1); } }
    Ok(result)
}

fn terminated(units: &[u32]) -> Vec<Tchar> {
    let mut result = Vec::with_capacity(units.len() + 1);
    result.extend(units.iter().map(|unit| *unit as Tchar));
    result.push(0);
    result
}

unsafe fn property_name(access: CoreAccess<'_>, property: *mut UProperty) -> RuntimeResult<Vec<u32>> {
    if property.is_null() { return Err(RuntimeError::new("Null property")); }
    let id = i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name });
    access.name_text(id)
}

unsafe fn property_class(access: CoreAccess<'_>, property: *mut UProperty) -> RuntimeResult<String> {
    let class = unsafe { (*property.cast::<UObject>()).class.cast::<UObject>() };
    if class.is_null() { return Err(RuntimeError::new("Property has no class")); }
    let name = i32::from_ne_bytes(unsafe { (*class).name });
    let units = access.name_text(name)?;
    Ok(units.into_iter().filter_map(char::from_u32).collect())
}

fn checked_bound_add(total: usize, additional: usize) -> RuntimeResult<usize> {
    total.checked_add(additional).ok_or_else(|| RuntimeError::new("Property text output size overflow"))
}

unsafe fn export_bound(access: CoreAccess<'_>, property: *mut UProperty, value: *mut u8, depth: usize) -> RuntimeResult<usize> {
    if depth > 64 { return Err(RuntimeError::new("Property text nesting is too deep")); }
    let class = unsafe { property_class(access, property)? };
    let kind = kind_for_class(&class);
    Ok(match kind {
        Kind::TextByte | Kind::TextInt | Kind::TextBool | Kind::TextFloat => 64,
        Kind::TextString => {
            let array = unsafe { ptr::read_unaligned(value.cast::<hp2_abi::engine::generated::TArrayRaw>()) };
            if array.num < 0 || (array.num > 0 && array.data.is_null()) { return Err(RuntimeError::new("Invalid FString storage")); }
            usize::try_from(array.num).unwrap_or(0).saturating_add(2)
        }
        Kind::TextName => {
            let name = unsafe { ptr::read_unaligned(value.cast::<hp2_abi::engine::generated::FName>()) };
            access.name_text(name.index)?.len().saturating_add(2)
        }
        Kind::TextObject | Kind::TextClass => {
            let mut object = unsafe { ptr::read_unaligned(value.cast::<*mut UObject>()) };
            let mut bound = 64usize;
            let mut hops = 0usize;
            if !object.is_null() {
                let class = unsafe { (*object).class.cast::<UObject>() };
                if !class.is_null() {
                    let class_name = i32::from_ne_bytes(unsafe { (*class).name });
                    bound = checked_bound_add(bound, access.name_text(class_name)?.len().saturating_add(1))?;
                }
            }
            while !object.is_null() {
                if hops >= 64 { return Err(RuntimeError::new("Object outer path is too deep")); }
                let name = i32::from_ne_bytes(unsafe { (*object).name });
                bound = checked_bound_add(bound, access.name_text(name)?.len().saturating_add(1))?;
                object = unsafe { (*object).outer.cast() };
                hops += 1;
            }
            bound
        }
        Kind::TextArray => {
            let array = unsafe { ptr::read_unaligned(value.cast::<hp2_abi::engine::generated::TArrayRaw>()) };
            let inner = unsafe { (*property.cast::<UArrayProperty>()).inner.cast::<UProperty>() };
            if inner.is_null() || array.num < 0 || (array.num > 0 && array.data.is_null()) { return Err(RuntimeError::new("Invalid dynamic array property value")); }
            let width = usize::try_from(unsafe { (*inner).element_size }).map_err(|_| RuntimeError::new("Invalid array element size"))?;
            let mut bound = 3usize.saturating_add((array.num.max(0) as usize).saturating_sub(1));
            for index in 0..array.num.max(0) as usize {
                let offset = index.checked_mul(width).ok_or_else(|| RuntimeError::new("Array text offset overflow"))?;
                bound = checked_bound_add(bound, unsafe { export_bound(access, inner, array.data.cast::<u8>().add(offset), depth + 1)? })?;
            }
            bound
        }
        Kind::TextFixed => {
            let fixed = unsafe { property.cast::<hp2_abi::engine::generated::UFixedArrayProperty>() };
            let inner = unsafe { (*fixed).inner.cast::<UProperty>() };
            let count = unsafe { (*fixed).count };
            if inner.is_null() || count < 0 { return Err(RuntimeError::new("Invalid fixed array property")); }
            let width = usize::try_from(unsafe { (*inner).element_size }).map_err(|_| RuntimeError::new("Invalid array element size"))?;
            let mut bound = 3usize.saturating_add((count.max(0) as usize).saturating_sub(1));
            for index in 0..count.max(0) as usize {
                let offset = index.checked_mul(width).ok_or_else(|| RuntimeError::new("Fixed array text offset overflow"))?;
                bound = checked_bound_add(bound, unsafe { export_bound(access, inner, value.add(offset), depth + 1)? })?;
            }
            bound
        }
        Kind::TextStruct => {
            let structure = unsafe { (*property.cast::<hp2_abi::engine::generated::UStructProperty>()).r#struct.cast::<UStruct>() };
            if structure.is_null() { return Err(RuntimeError::new("Struct property has no type")); }
            let mut field = unsafe { (*structure).children.cast::<hp2_abi::engine::generated::UProperty>() };
            let mut bound = 1usize;
            let mut count = 0usize;
            while !field.is_null() {
                if count >= 4096 { return Err(RuntimeError::new("Struct property field chain is invalid")); }
                let field_class = unsafe { property_class(access, field)? };
                if field_class.ends_with("Property") {
                    let size = unsafe { (*field).element_size.max(0) as usize };
                    let dim = unsafe { (*field).array_dim.max(0) as usize };
                    let base = unsafe { value.add((*field).offset.max(0) as usize) };
                    let mut item_bound = 0usize;
                    for index in 0..dim {
                        let offset = index.checked_mul(size).ok_or_else(|| RuntimeError::new("Struct field text offset overflow"))?;
                        item_bound = checked_bound_add(item_bound, unsafe { export_bound(access, field, base.add(offset), depth + 1)? })?;
                    }
                    let name = access.name_text(i32::from_ne_bytes(unsafe { (*field.cast::<UObject>()).name }))?;
                    bound = checked_bound_add(bound, item_bound.saturating_add(name.len()).saturating_add(16))?;
                }
                field = unsafe { (*field.cast::<hp2_abi::engine::generated::UField>()).next.cast() };
                count += 1;
            }
            if bound == 1 { 1 } else { bound.saturating_add(1) }
        }
        Kind::TextGeneric | Kind::TextMap => 1,
    })
}
fn destroy_element(access: CoreAccess<'_>, property: *mut UProperty, element: *mut u8) -> RuntimeResult<()> {
    // SAFETY: caller passes one initialized value with this reflected inner type.
    let result = unsafe {
        access.property_value(
            crate::property_value::Hp2ValueOperation::ValueDestroy,
            property,
            element.cast(),
            ptr::null_mut(),
            0,
            None,
        )?
    };
    if result == 0 { Err(RuntimeError::new("Property element destruction failed")) } else { Ok(()) }
}
fn snapshot_element(access: CoreAccess<'_>, property: *mut UProperty, source: *mut u8) -> RuntimeResult<*mut u8> {
    if source.is_null() { return Ok(ptr::null_mut()); }
    let width = usize::try_from(unsafe { (*property).element_size }).map_err(|_| RuntimeError::new("Invalid array element size"))?;
    let copy = unsafe { libc::calloc(1, width) }.cast::<u8>();
    if copy.is_null() && width != 0 { return Err(RuntimeError::new("Out of memory")); }
    let result = unsafe {
        access.property_value(crate::property_value::Hp2ValueOperation::ValueCopySingle, property, copy.cast(), source.cast(), 0, None)
    };
    match result {
        Ok(1) => Ok(copy),
        Ok(_) => { unsafe { libc::free(copy.cast()); } Err(RuntimeError::new("Property default snapshot failed")) }
        Err(error) => { unsafe { libc::free(copy.cast()); } Err(error) }
    }
}


impl PropertyTextOps for DirectOps<'_> {
    type Error = RuntimeError;
    fn none_name(&self) -> i32 { 0 }
    fn class_name(&self) -> i32 { self.class_name }
    fn delimited(&self) -> i32 { 2 }
    fn localized(&self) -> i32 { 1 }
    fn transient(&self) -> u32 { 0x0000_2000 }
    fn native(&self) -> u32 { 0x0000_1000 }
    fn out_parm(&self) -> u32 { 0x0000_0100 }

    fn integer(&self, property: Object, kind: Integer) -> i32 {
        if property.is_null() { return 0; }
        // SAFETY: parser integer queries only use live property/value records.
        unsafe {
            let p = property.cast::<UProperty>();
            match kind {
                Integer::TextDim => (*p).array_dim,
                Integer::TextSize => (*p).element_size,
                Integer::TextOffset => (*p).offset,
                Integer::TextMask => {
                    if property_class(self.access, p).is_ok_and(|class| class == "BoolProperty") {
                        (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as i32
                    } else { (*p).property_flags as i32 }
                }
                Integer::TextCount => (*property.cast::<hp2_abi::engine::generated::UFixedArrayProperty>()).count,
                Integer::TextArrayNum => ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::TArrayRaw>()).num).read_unaligned(),
                Integer::TextHasEnum => i32::from(!(*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum.is_null()),
                Integer::TextEnumCount => {
                    let enumeration = (*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum;
                    if enumeration.is_null() { 0 } else { (*enumeration.cast::<hp2_abi::engine::generated::UEnum>()).names.num }
                }
                Integer::TextNameId => i32::from_ne_bytes((*property.cast::<UObject>()).name),
                Integer::TextCategory => i32::from_ne_bytes((*p).category),
                Integer::TextFlags => (*p).property_flags as i32,
                Integer::TextIsProperty => 1,
                Integer::TextIsBool => i32::from(property_class(self.access, p).is_ok_and(|class| class == "BoolProperty")),
                Integer::TextIsString => i32::from(property_class(self.access, p).is_ok_and(|class| class == "StrProperty")),
            }
        }
    }

    fn pointer(&self, property: Object, kind: Pointer, index: i32) -> RuntimeResult<Object> {
        if property.is_null() { return Err(RuntimeError::new("Null property text pointer")); }
        let p = property.cast::<UProperty>();
        // SAFETY: these accesses follow the frozen engine record layouts. Returned
        // string data is canonical name text or stable reflected metadata.
        unsafe {
            match kind {
                Pointer::TextInner => {
                    let class = property_class(self.access, p)?;
                    if class == "FixedArrayProperty" {
                        Ok((*property.cast::<hp2_abi::engine::generated::UFixedArrayProperty>()).inner)
                    } else {
                        Ok((*property.cast::<UArrayProperty>()).inner)
                    }
                }
                Pointer::TextStructType => Ok((*property.cast::<hp2_abi::engine::generated::UStructProperty>()).r#struct),
                Pointer::TextChildren => Ok((*property.cast::<UStruct>()).children.cast()),
                Pointer::TextNext => Ok((*property.cast::<hp2_abi::engine::generated::UField>()).next.cast()),
                Pointer::TextSuper => Ok((*property.cast::<UField>()).super_field.cast()),
                Pointer::TextData => Ok(ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::TArrayRaw>()).data).read_unaligned().cast()),
                Pointer::TextStringData => {
                    let slots = property.cast::<hp2_abi::engine::generated::TArrayRaw>();
                    Ok(crate::native_text::fstring_text(slots).cast_mut().cast())
                }
                Pointer::TextNameData => {
                    let id = (*property.cast::<hp2_abi::engine::generated::FName>()).index;
                    // Name entry storage is canonical and does not move.
                    let entry = self.access.name_at(id);
                    if entry.is_null() { Ok(ptr::null_mut()) } else { Ok(ptr::addr_of_mut!((*entry).name).cast()) }
                }
                Pointer::TextPropertyName | Pointer::TextCppName => {
                    let id = i32::from_ne_bytes((*property.cast::<UObject>()).name);
                    let entry = self.access.name_at(id);
                    if entry.is_null() { Ok(ptr::null_mut()) } else { Ok(ptr::addr_of_mut!((*entry).name).cast()) }
                }
                Pointer::TextPropertyClass => Ok((*property.cast::<hp2_abi::engine::generated::UObjectProperty>()).property_class),
                Pointer::TextMetaClass => Ok((*property.cast::<hp2_abi::engine::generated::UClassProperty>()).meta_class),
                Pointer::TextClassName => {
                    let object = property.cast::<UObject>();
                    let class = (*object).class.cast::<UObject>();
                    if class.is_null() { Ok(ptr::null_mut()) } else {
                        let entry = self.access.name_at(i32::from_ne_bytes((*class).name));
                        Ok(if entry.is_null() { ptr::null_mut() } else { ptr::addr_of_mut!((*entry).name).cast() })
                    }
                }
                Pointer::TextObjectPath => {
                    let object = property.cast::<UObject>();
                    let mut current = object;
                    let mut chain = Vec::new();
                    while !current.is_null() {
                        chain.push(self.access.name_text(i32::from_ne_bytes((*current).name))?);
                        current = (*current).outer.cast();
                    }
                    chain.reverse();
                    let mut path = Vec::new();
                    for (i, component) in chain.iter().enumerate() {
                        if i > 0 { path.push(b'.' as u32); }
                        path.extend_from_slice(component);
                    }
                    path.push(0);
                    let mut scratch = self.scratch.borrow_mut();
                    scratch.clear();
                    scratch.extend_from_slice(&path);
                    Ok(scratch.as_mut_ptr().cast())
                }
                Pointer::TextEnumName => {
                    let enumeration = (*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum;
                    if enumeration.is_null() { return Ok(ptr::null_mut()); }
                    let names = ptr::read_unaligned(ptr::addr_of!((*enumeration.cast::<hp2_abi::engine::generated::UEnum>()).names));
                    if index < 0 || index >= names.num { return Ok(ptr::null_mut()); }
                    let id = ptr::read_unaligned(names.data.cast::<i32>().add(index as usize));
                    let entry = self.access.name_at(id);
                    Ok(if entry.is_null() { ptr::null_mut() } else { ptr::addr_of_mut!((*entry).name).cast() })
                }
                Pointer::TextTrue | Pointer::TextFalse => {
                    let token = if matches!(kind, Pointer::TextTrue) { "True" } else { "False" };
                    let id = self.access.intern_str(token, FindName::Find)?;
                    let entry = self.access.name_at(id);
                    Ok(if entry.is_null() { ptr::null_mut() } else { ptr::addr_of_mut!((*entry).name).cast() })
                }
                Pointer::TextKey => Ok(ptr::addr_of_mut!((*property.cast::<crate::core_records::MapPropertyRecord>()).key).cast()),
                Pointer::TextValue => Ok(ptr::addr_of_mut!((*property.cast::<crate::core_records::MapPropertyRecord>()).value).cast()),
            }
        }
    }

    fn call(&self, property: Object, operation: Call, a: Object, b: Object, c: Object, n: i32, flags: i32) -> RuntimeResult<(Object, i32)> {
        use Call::*;
        if property.is_null() && !matches!(operation, TextFindName | TextFindClass | TextFindObject | TextIsChild) { return Err(RuntimeError::new("Null property text operation")); }
        let p = property.cast::<UProperty>();
        // SAFETY: operands are live text-parser values and reflection records.
        unsafe {
            match operation {
                TextMatches => {
                    let result = self.access.property_value(crate::property_value::Hp2ValueOperation::ValueMatches, p, a, b, n, None)?;
                    Ok((ptr::null_mut(), result))
                }
                TextFindName => {
                    let units = text_units(a.cast())?;
                    Ok((ptr::null_mut(), self.access.find_name(&units)?))
                }
                TextAssignName => {
                    let units = text_units(b.cast())?;
                    let id = self.access.intern(&units, FindName::Add)?;
                    a.cast::<i32>().write_unaligned(id);
                    Ok((ptr::null_mut(), 0))
                }
                TextAssignString => {
                    let array = a.cast::<hp2_abi::engine::generated::TArrayRaw>();
                    let slots = crate::array_storage::Hp2ArraySlots {
                        data: ptr::addr_of_mut!((*array).data),
                        num: ptr::addr_of_mut!((*array).num),
                        max: ptr::addr_of_mut!((*array).max),
                    };
                    crate::string_storage::assign_with_allocator(
                        slots, None, b.cast(), false,
                        crate::array_storage::system_allocator(), ptr::null(),
                    ).map_err(|_| RuntimeError::new("Unable to assign FString"))?;
                    Ok((ptr::null_mut(), 0))
                }
                TextFindEnum => {
                    let enumeration = (*p.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum;
                    if enumeration.is_null() { return Ok((ptr::null_mut(), -1)); }
                    let names = ptr::read_unaligned(ptr::addr_of!((*enumeration.cast::<hp2_abi::engine::generated::UEnum>()).names));
                    let mut found = -1;
                    for index in 0..names.num.max(0) {
                        let fname = ptr::read_unaligned(names.data.cast::<hp2_abi::engine::generated::FName>().add(index as usize));
                        if fname.index == n { found = index; break; }
                    }
                    Ok((ptr::null_mut(), found))
                }
                TextFindObject | TextFindClass => {
                    let name = text_units(a.cast())?;
                    let expected = if matches!(operation, TextFindClass) {
                        self.access.native_class("Class")?
                    } else {
                        // The grammar passes the expected UClass here, not the UProperty.
                        property.cast::<UClass>()
                    };
                    let loaded = self.access.find_object_text(expected, &name)?;
                    Ok((loaded.cast(), i32::from(!loaded.is_null())))
                }
                TextIsClass => {
                    let meta_class = self.access.native_class("Class")?;
                    Ok((ptr::null_mut(), i32::from(!meta_class.is_null() && (*property.cast::<UObject>()).class == meta_class.cast())))
                }
                TextIsChild => {
                    let parent = a.cast::<UClass>();
                    Ok((ptr::null_mut(), i32::from(!parent.is_null() && crate::object_model::is_child_of(property, parent.cast()))))
                }
                TextArrayEmpty => {
                    let inner = (*property.cast::<UArrayProperty>()).inner.cast::<UProperty>();
                    if inner.is_null() || a.is_null() { return Err(RuntimeError::new("Invalid dynamic array storage")); }
                    let width = usize::try_from((*inner).element_size).map_err(|_| RuntimeError::new("Invalid array element size"))?;
                    crate::core_array::empty_array_with(a.cast(), width, |slot| destroy_element(self.access, inner, slot))?;
                    Ok((a, 1))
                }
                TextArrayAdd => {
                    let inner = (*property.cast::<UArrayProperty>()).inner.cast::<UProperty>();
                    if inner.is_null() || a.is_null() { return Err(RuntimeError::new("Invalid dynamic array storage")); }
                    let width = usize::try_from(n).map_err(|_| RuntimeError::new("Invalid array element size"))?;
                    if width == 0 || width != usize::try_from((*inner).element_size).map_err(|_| RuntimeError::new("Invalid array element size"))? { return Err(RuntimeError::new("Array element size mismatch")); }
                    // The shared ImportText kernel zeroes the published slot immediately.
                    let index = crate::core_array::append_array_uninitialized(a.cast(), width)?;
                    Ok((a, index))
                }
                TextCheckInner => {
                    let inner = self.pointer(property, Pointer::TextInner, 0)?;
                    if inner.is_null() { Err(RuntimeError::new("Array property has no inner property")) } else { Ok((inner, 1)) }
                }
                TextCheckMap => {
                    let map = property.cast::<crate::core_records::MapPropertyRecord>();
                    if (*map).key.is_null() || (*map).value.is_null() { Err(RuntimeError::new("Map property has no key or value")) } else { Ok((property, 1)) }
                }
                TextVirtualExport | TextVirtualExportItem | TextVirtualImport | TextVirtualCpp | TextVirtualPort => {
                    let class = property_class(self.access, p)?;
                    let kind = kind_for_class(&class);
                    let op = match operation {
                        TextVirtualExport => Operation::TextExport,
                        TextVirtualExportItem => Operation::TextExportItem,
                        TextVirtualImport => Operation::TextImport,
                        TextVirtualCpp => Operation::TextCppItem,
                        TextVirtualPort => Operation::TextPort,
                        _ => unreachable!(),
                    };
                    property_text::run_with(kind, op, property, a, b, c, n, flags, self)
                }
            }
        }
    }

    fn format(&self, out: Object, dest: *mut Tchar, kind: Format, a: *const Tchar, b: *const Tchar, n: i32, f: f64) -> RuntimeResult<i32> {
        // SAFETY: formatter owns output semantics and accepts only the parser's pointers.
        unsafe { crate::core_property_format::format(out, dest, kind, a, b, n, f) }
    }
    fn log(&self, out: Object, text: *const Tchar) -> RuntimeResult<()> {
        // SAFETY: formatter/log owner receives a stable NUL-terminated diagnostic.
        unsafe { crate::core_property_format::log(out, text) }
    }
    fn warning(&self, property: Object, warning: Warning, name: *const Tchar) -> RuntimeResult<()> {
        let named = if name.is_null() { Vec::new() } else { text_units(name)? };
        let message: Vec<u32> = match warning {
            Warning::TextQuotedOverflow => "ReadToken: Quoted string too long".chars().map(u32::from).collect(),
            Warning::TextBadQuote => "ReadToken: Bad quoted string".chars().map(u32::from).collect(),
            Warning::TextTokenOverflow => "ReadToken: Alphanumeric overflow".chars().map(u32::from).collect(),
            Warning::TextArrayElement => "ImportText: Illegal array element".chars().map(u32::from).collect(),
            Warning::TextMissingKey => "ImportText: Illegal or missing key name".chars().map(u32::from).collect(),
            Warning::TextUnknownElement => {
                let property_name = unsafe { property_name(self.access, property.cast())? };
                let mut value: Vec<u32> = "ImportText: Unknown element ".chars().map(u32::from).collect();
                value.extend_from_slice(&named);
                value.extend(" of struct ".chars().map(u32::from));
                value.extend(property_name);
                value
            }
            Warning::TextStructBadQuote => "ImportText: Bad quoted string".chars().map(u32::from).collect(),
            Warning::TextBadParenthesis => "ImportText: Bad parenthesised struct".chars().map(u32::from).collect(),
            Warning::TextIncompleteStruct => "ImportText: Incomplete parenthesised struct".chars().map(u32::from).collect(),
            Warning::TextBadTermination => "ImportText: Bad termination".chars().map(u32::from).collect(),
            Warning::TextMissingParenthesis => "ImportText: Struct missing '('".chars().map(u32::from).collect(),
        };
        self.access.emit_warning(&message)?;
        Ok(())
    }
    fn atoi(&self, text: *const Tchar) -> RuntimeResult<i32> {
        if text.is_null() { return Err(RuntimeError::new("Null integer property text")); }
        // SAFETY: the parser supplies a NUL-terminated TCHAR string.
        let value = unsafe { wcstol(text, ptr::null_mut(), 10) };
        i32::try_from(value).map_err(|_| RuntimeError::new("Integer property text is out of range"))
    }
    fn atof(&self, text: *const Tchar) -> RuntimeResult<f32> {
        let units = text_units(text)?;
        let mut bytes = Vec::with_capacity(units.len() + 1);
        for unit in units {
            bytes.push(if unit <= 255 { unit as u8 } else { b'?' });
        }
        bytes.push(0);
        // SAFETY: bytes is NUL-terminated and the C routine does not retain it.
        Ok(unsafe { libc::atof(bytes.as_ptr().cast()) as f32 })
    }
}

impl CoreAccess<'_> {
    /// Imports one complete property value from the existing property-text grammar.
    /// # Safety
    /// `property`, object storage, default storage, and input through NUL are live and correctly sized.
    pub(crate) unsafe fn property_import_text(self, property: *mut UProperty, object: *mut UObject, input: &[u32], default_object: *mut UObject, flags: u32) -> RuntimeResult<usize> {
        let value = unsafe { self.property_address(object, property)? };
        let default = if default_object.is_null() { ptr::null_mut() } else { unsafe { self.property_address(default_object, property)? } };
        unsafe { self.property_import_text_at(property, value, input, default, flags) }
    }

    /// Imports a property at an already-computed reflected byte address.
    /// # Safety
    /// The property and pointed value/default storage remain live through callbacks.
    pub unsafe fn property_import_text_at(self, property: *mut UProperty, value: *mut u8, input: &[u32], default_value: *mut u8, flags: u32) -> RuntimeResult<usize> {
        if property.is_null() || value.is_null() { return Err(RuntimeError::new("Null property text value")); }
        let class = unsafe { property_class(self, property)? };
        let kind = kind_for_class(&class);
        let mut text = terminated(input);
        let ops = DirectOps::new(self)?;
        let (remaining, _) = unsafe { property_text::run_with(kind, Operation::TextImport, property.cast(), text.as_mut_ptr().cast(), value.cast(), default_value.cast(), 0, flags as i32, &ops)? };
        if remaining.is_null() { return Ok(0); }
        let end = unsafe { remaining.cast::<Tchar>().offset_from(text.as_ptr()) };
        Ok(usize::try_from(end).unwrap_or(0))
    }
    unsafe fn import_with_default_snapshot(self, property: *mut UProperty, value: *mut u8, input: &[u32], default_value: *mut u8, flags: u32) -> RuntimeResult<usize> {
        let snapshot = snapshot_element(self, property, default_value)?;
        let imported = unsafe { self.property_import_text_at(property, value, input, snapshot, flags) };
        let destroyed = if snapshot.is_null() { Ok(()) } else {
            let result = destroy_element(self, property, snapshot);
            unsafe { libc::free(snapshot.cast()); }
            result
        };
        match imported {
            Err(error) => { let _ = destroyed; Err(error) }
            Ok(consumed) => { destroyed?; Ok(consumed) }
        }
    }

    /// Exports one complete property value into the caller's UTF-32 buffer.
    /// # Safety
    /// Reflected data and output must remain valid during synchronous operations.
    pub(crate) unsafe fn property_export_text(self, property: *mut UProperty, object: *mut UObject, default_object: *mut UObject, flags: u32) -> RuntimeResult<Vec<u32>> {
        let mut output = Vec::new();
        let value = unsafe { self.property_address(object, property)? };
        let default = if default_object.is_null() { ptr::null_mut() } else { unsafe { self.property_address(default_object, property)? } };
        unsafe { self.property_export_text_at(property, value, default, flags, &mut output)?; }
        Ok(output)
    }

    /// Exports from an already-computed value address.
    /// # Safety
    /// Property, values, and any nested reflected values remain live through callbacks.
    pub(crate) unsafe fn property_export_text_at(self, property: *mut UProperty, value: *mut u8, default_value: *mut u8, flags: u32, output: &mut Vec<u32>) -> RuntimeResult<()> {
        if property.is_null() || value.is_null() { return Err(RuntimeError::new("Null property text value")); }
        let class = unsafe { property_class(self, property)? };
        let kind = kind_for_class(&class);
        let bound = unsafe { export_bound(self, property, value, 0)? };
        let capacity = bound.checked_add(1).ok_or_else(|| RuntimeError::new("Property text output size overflow"))?.max(1);
        let mut buffer = vec![0_u32; capacity];
        let ops = DirectOps::new(self)?;
        let _ = unsafe {
            property_text::run_with(
                kind, Operation::TextExportItem, property.cast(),
                buffer.as_mut_ptr().cast(), value.cast(), default_value.cast(), 0,
                flags as i32, &ops,
            )?
        };
        let length = buffer.iter().position(|unit| *unit == 0)
            .ok_or_else(|| RuntimeError::new("Property text export exceeded its checked output buffer"))?;
        output.extend_from_slice(&buffer[..length]);
        Ok(())
    }

    /// Full UProperty::ExportText, including its false result when values match defaults.
    ///
    /// # Safety
    /// Data/Delta bases and metadata remain live through Offset + Index * ElementSize.
    pub(crate) unsafe fn property_export_text_optional_at(
        self, property: *mut UProperty, value: *mut u8, default_value: *mut u8,
        index: i32, flags: u32,
    ) -> RuntimeResult<Option<Vec<u32>>> {
        if property.is_null() || value.is_null() {
            return Err(RuntimeError::new("Null property ExportText value"));
        }
        if index < 0 || index >= unsafe { (*property).array_dim } {
            return Err(RuntimeError::new("Property ExportText index out of bounds"));
        }
        let class = unsafe { property_class(self, property)? };
        let kind = kind_for_class(&class);
        let width = usize::try_from(unsafe { (*property).element_size })
            .map_err(|_| RuntimeError::new("Invalid ExportText element size"))?;
        let property_offset = usize::try_from(unsafe { (*property).offset })
            .map_err(|_| RuntimeError::new("Invalid ExportText property offset"))?;
        let offset = (index as usize).checked_mul(width)
            .and_then(|offset| offset.checked_add(property_offset))
            .ok_or_else(|| RuntimeError::new("Property ExportText offset overflow"))?;
        let bound = unsafe { export_bound(self, property, value.add(offset), 0)? };
        let capacity = bound.checked_add(1)
            .ok_or_else(|| RuntimeError::new("Property ExportText output size overflow"))?.max(1);
        let mut output = vec![0_u32; capacity];
        let ops = DirectOps::new(self)?;
        let (_, exported) = unsafe {
            property_text::run_with(
                kind, Operation::TextExport, property.cast(), output.as_mut_ptr().cast(),
                value.cast(), default_value.cast(), index, flags as i32, &ops,
            )?
        };
        if exported == 0 { return Ok(None); }
        let length = output.iter().position(|unit| *unit == 0)
            .ok_or_else(|| RuntimeError::new("Property ExportText exceeded its checked output buffer"))?;
        output.truncate(length);
        Ok(Some(output))
    }

    /// Imports each text value into a dynamic array, replacing its previous contents.
    /// # Safety
    /// The array property, object and default object are live canonical records.
    pub(crate) unsafe fn property_import_array_values(self, property: *mut UArrayProperty, object: *mut UObject, values: &[Vec<u32>], default_object: *mut UObject, flags: u32) -> RuntimeResult<()> {
        if property.is_null() || object.is_null() { return Err(RuntimeError::new("Null dynamic array property value")); }
        let base_property = property.cast::<UProperty>();
        let address = unsafe { self.property_address(object, base_property)? };
        let inner = unsafe { (*property).inner.cast::<UProperty>() };
        if inner.is_null() { return Err(RuntimeError::new("Array property has no inner property")); }
        let width = usize::try_from(unsafe { (*inner).element_size }).map_err(|_| RuntimeError::new("Invalid array element size"))?;
        if width == 0 { return Err(RuntimeError::new("Invalid array element size")); }
        let default_address = if default_object.is_null() { ptr::null_mut() } else { unsafe { self.property_address(default_object, base_property)? } };
        let array = address.cast::<hp2_abi::engine::generated::TArrayRaw>();
        unsafe { crate::core_array::empty_array_with(array, width, |slot| destroy_element(self, inner, slot))?; }
        for text in values {
            let index = unsafe { crate::core_array::append_array_uninitialized(array, width)? };
            let slots = unsafe { ptr::read_unaligned(array) };
            let slot = unsafe { slots.data.cast::<u8>().add(index as usize * width) };
            // Every Core property has a valid empty value in zeroed storage,
            // matching native config-array import before Inner.ImportText.
            unsafe { slot.write_bytes(0, width); }
            let default_array = if default_address.is_null() { None } else { Some(unsafe { ptr::read_unaligned(default_address.cast::<hp2_abi::engine::generated::TArrayRaw>()) }) };
            let default_slot = default_array.and_then(|items| {
                if index < items.num && !items.data.is_null() {
                    Some(unsafe { items.data.cast::<u8>().add(index as usize * width) })
                } else { None }
            }).unwrap_or(ptr::null_mut());
            unsafe { self.import_with_default_snapshot(inner, slot, text, default_slot, flags)?; }
        }
        Ok(())
    }
    /// Imports a property element using its reflected fixed-array index.
    /// # Safety
    /// The containing value and optional default value are live for the duration of the call.
    pub(crate) unsafe fn property_import_text_index(self, property: *mut UProperty, value: *mut u8, input: &[u32], default_value: *mut u8, index: i32, flags: u32) -> RuntimeResult<usize> {
        if property.is_null() || value.is_null() { return Err(RuntimeError::new("Null indexed property text value")); }
        let dimension = unsafe { (*property).array_dim };
        if index < 0 || index >= dimension { return Err(RuntimeError::new("Property text index out of bounds")); }
        let element_size = usize::try_from(unsafe { (*property).element_size }).map_err(|_| RuntimeError::new("Invalid property element size"))?;
        let offset = usize::try_from(index).map_err(|_| RuntimeError::new("Property text index out of bounds"))?.checked_mul(element_size)
            .ok_or_else(|| RuntimeError::new("Property text offset overflow"))?;
        let default = if default_value.is_null() { ptr::null_mut() } else { unsafe { default_value.add(offset) } };
        unsafe { self.property_import_text_at(property, value.add(offset), input, default, flags) }
    }

    /// Imports one property from a raw address using the shared parser.
    /// # Safety
    /// Same requirements as `property_import_text_at`.
    pub(crate) unsafe fn property_import_raw_value(self, property: *mut UProperty, value: *mut u8, input: &[u32], default_value: *mut u8, flags: u32) -> RuntimeResult<usize> {
        unsafe { self.property_import_text_at(property, value, input, default_value, flags) }
    }

    /// Exports one property from a raw address using the shared parser.
    /// # Safety
    /// Same requirements as `property_export_text_at`.
    pub(crate) unsafe fn property_export_raw_value(self, property: *mut UProperty, value: *mut u8, default_value: *mut u8, flags: u32, output: &mut Vec<u32>) -> RuntimeResult<()> {
        unsafe { self.property_export_text_at(property, value, default_value, flags, output) }
    }

    /// Imports one initialized dynamic-array element.
    /// # Safety
    /// The array header, elements, inner property, and optional default header are live.
    pub(crate) unsafe fn property_import_array_element(self, property: *mut UArrayProperty, array: *mut hp2_abi::engine::generated::TArrayRaw, index: i32, input: &[u32], default_array: *const hp2_abi::engine::generated::TArrayRaw, flags: u32) -> RuntimeResult<usize> {
        if property.is_null() || array.is_null() || index < 0 { return Err(RuntimeError::new("Invalid dynamic array element")); }
        let inner = unsafe { (*property).inner.cast::<UProperty>() };
        if inner.is_null() { return Err(RuntimeError::new("Array property has no inner property")); }
        let width = usize::try_from(unsafe { (*inner).element_size }).map_err(|_| RuntimeError::new("Invalid array element size"))?;
        if width == 0 { return Err(RuntimeError::new("Invalid array element size")); }
        let slots = unsafe { ptr::read_unaligned(array) };
        if index >= slots.num || slots.data.is_null() { return Err(RuntimeError::new("Dynamic array element index out of bounds")); }
        let offset = usize::try_from(index).map_err(|_| RuntimeError::new("Dynamic array element index out of bounds"))?
            .checked_mul(width).ok_or_else(|| RuntimeError::new("Dynamic array element offset overflow"))?;
        let value = unsafe { slots.data.cast::<u8>().add(offset) };
        let default = if default_array.is_null() { ptr::null_mut() } else {
            let defaults = unsafe { ptr::read_unaligned(default_array) };
            if index >= defaults.num || defaults.data.is_null() { ptr::null_mut() } else { unsafe { defaults.data.cast::<u8>().add(offset) } }
        };
        unsafe { self.import_with_default_snapshot(inner, value, input, default, flags) }
    }

    /// Exports one fixed-array element.
    /// # Safety
    /// Same requirements as `property_export_text_at`.
    pub(crate) unsafe fn property_export_text_index(self, property: *mut UProperty, value: *mut u8, default_value: *mut u8, index: i32, flags: u32, output: &mut Vec<u32>) -> RuntimeResult<()> {
        if property.is_null() || value.is_null() { return Err(RuntimeError::new("Null indexed property text value")); }
        let dimension = unsafe { (*property).array_dim };
        if index < 0 || index >= dimension { return Err(RuntimeError::new("Property text index out of bounds")); }
        let element_size = usize::try_from(unsafe { (*property).element_size }).map_err(|_| RuntimeError::new("Invalid property element size"))?;
        let offset = usize::try_from(index).map_err(|_| RuntimeError::new("Property text index out of bounds"))?.checked_mul(element_size)
            .ok_or_else(|| RuntimeError::new("Property text offset overflow"))?;
        let default = if default_value.is_null() { ptr::null_mut() } else { unsafe { default_value.add(offset) } };
        unsafe { self.property_export_text_at(property, value.add(offset), default, flags, output) }
    }
}

fn kind_for_class(class: &str) -> Kind {
    match class {
        "ByteProperty" => Kind::TextByte,
        "IntProperty" => Kind::TextInt,
        "BoolProperty" => Kind::TextBool,
        "FloatProperty" => Kind::TextFloat,
        "ObjectProperty" => Kind::TextObject,
        "ClassProperty" => Kind::TextClass,
        "NameProperty" => Kind::TextName,
        "StrProperty" => Kind::TextString,
"FixedArrayProperty" => Kind::TextFixed,
        "ArrayProperty" => Kind::TextArray,
        "MapProperty" => Kind::TextMap,
        "StructProperty" => Kind::TextStruct,
        _ => Kind::TextGeneric,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_access::{CoreOptions, CoreOwner};
    use crate::core_records::SystemRecord;
    use crate::name_owner::FindName;

    #[test]
    fn full_export_preserves_default_comparison_and_data_delta_alias() {
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("full-property-export-{}", std::process::id()));
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/Default.ini"), "").unwrap();
        std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
        fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
        let owner = CoreOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
        }).unwrap();
        let access = owner.access();
        let class = access.native_class("Core.System").unwrap();
        let name = access.intern_str("SavePath", FindName::Find).unwrap();
        let property = unsafe { access.find_field(class.cast(), name, access.native_class("Core.Property").unwrap()) }.cast::<UProperty>();
        assert!(!property.is_null());
        unsafe {
            let first = access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut()).unwrap().cast::<SystemRecord>();
            let second = access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut()).unwrap().cast::<SystemRecord>();
            let text = |value: &str| value.chars().map(u32::from).chain([0]).collect::<Vec<_>>();
            access.assign_fstring(ptr::addr_of_mut!((*first).save_path), &text("fixture-primary")).unwrap();
            access.assign_fstring(ptr::addr_of_mut!((*second).save_path), &text("fixture-primary")).unwrap();
            assert_eq!(access.property_export_text_optional_at(property, first.cast(), second.cast(), 0, 2).unwrap(), None,
                "distinct equal defaults suppress full ExportText");
            let expected: Vec<u32> = "\"fixture-primary\"".chars().map(u32::from).collect();
            assert_eq!(access.property_export_text_optional_at(property, first.cast(), first.cast(), 0, 2).unwrap(), Some(expected.clone()),
                "Data == Delta forces export, even for an identical value");
            access.assign_fstring(ptr::addr_of_mut!((*second).save_path), &text("different-default")).unwrap();
            assert_eq!(access.property_export_text_optional_at(property, first.cast(), second.cast(), 0, 2).unwrap(), Some(expected),
                "different defaults export the live value at the native property offset");
        }
        owner.shutdown().unwrap();
    }
}
