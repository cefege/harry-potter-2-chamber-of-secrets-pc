//! Property value policy over compiler-probed metadata and raw container views.
//! Legacy adapters preserve native virtual dispatch and typed destruction.
use crate::array_storage::{Hp2ArrayServices, Hp2ArraySlots};
use hp2_abi::engine::generated::{
    TArrayRaw, UArrayProperty, UBoolProperty, UByteProperty, UEnum, UField, UFixedArrayProperty,
    UObject, UProperty, UStruct, UStructProperty,
};
use std::{ffi::c_void, ptr};
type Object = *mut c_void;
/// `UnObj.h`'s property lifetime flag, also asserted by the native layout adapter.
const CPF_NEED_CTOR_LINK: u32 = 0x00400000;
type Result<T = ()> = std::result::Result<T, i32>;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2ValueKind {
    ValueGeneric,
    ValueByte,
    ValueInt,
    ValueBool,
    ValueFloat,
    ValueObject,
    ValueName,
    ValueString,
    ValueFixed,
    ValueArray,
    ValueStruct,
    // Appended to preserve the numeric ABI of all legacy property kinds.
    ValueMap,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2ValueOperation {
    ValueIdentical,
    ValueCopySingle,
    ValueCopyComplete,
    ValueDestroy,
    ValueSerialize,
    ValueNetSerialize,
    ValueMatches,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ValueInteger {
    ValueLoading,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ValuePointer {
    // Virtual inheritance and lazy StaticClass construction are native leaves.
    ValueSuper,
    ValuePropertyClass,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ValueCall {
    ValueCallIdentical,
    ValueCallCopySingle,
    ValueCallCopyComplete,
    ValueCallDestroy,
    ValueCallSerialize,
    ValueCallNetSerialize,
    ValueCallPreload,
    ValueCallBinary,
    ValueCallMapIndex,
    ValueCallObjectNet,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ValueStorage {
    ValueStringDestroy,
    ValueArrayDestroy,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2ValueWire {
    ValueWireByte,
    ValueWireInt,
    ValueWireFloat,
    ValueWireObject,
    ValueWireName,
    ValueWireString,
    ValueWireIndex,
    ValueWireBits,
    ValueWireBounded,
    ValueWireShort,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PropertyValueServices {
    pub none_name: i32,
    pub vector_name: i32,
    pub rotator_name: i32,
    pub plane_name: i32,
    pub arrays: *const Hp2ArrayServices,
    pub array_tag: *const c_void,
    pub string_tag: *const c_void,
    pub array_check_add: unsafe extern "C" fn(i32, i32, i32) -> i32,
    pub integer: unsafe extern "C" fn(Object, Hp2ValueInteger) -> i32,
    pub pointer: unsafe extern "C" fn(Object, Hp2ValuePointer, *mut Object) -> i32,
    pub call: unsafe extern "C" fn(Object, Object, Hp2ValueCall, Object, Object, *mut i32) -> i32,
    pub storage: unsafe extern "C" fn(Hp2ValueStorage, Object) -> i32,
    pub wire: unsafe extern "C" fn(Object, Hp2ValueWire, Object, u32) -> i32,
}
fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}
/// Engine-thread operations required by the property value policy.
///
/// Implementations use shared-reference capabilities and must not retain any
/// supplied raw pointer beyond the operation. Callback-capable methods may
/// reenter the engine; callers therefore keep no Rust reference into engine
/// storage across them.
pub trait PropertyValueOps {
    /// Error produced by a property operation.
    type Error;

    /// Name index for `None`.
    fn none_name(&self) -> i32;
    /// Name index for the `Vector` struct.
    fn vector_name(&self) -> i32;
    /// Name index for the `Rotator` struct.
    fn rotator_name(&self) -> i32;
    /// Name index for the `Plane` struct.
    fn plane_name(&self) -> i32;
    /// Allocation tag used for dynamic property arrays.
    fn array_tag(&self) -> *const c_void;
    /// Allocation tag used for FString storage.
    fn string_tag(&self) -> *const c_void;

    /// Resolve a property metadata pointer or its superclass.
    ///
    /// # Safety
    /// `object` must be a live engine record valid for `field`.
    unsafe fn pointer(
        &self,
        object: Object,
        field: Hp2ValuePointer,
    ) -> std::result::Result<Object, Self::Error>;
    /// Invoke the requested class/property/archive operation.
    ///
    /// # Safety
    /// All pointers must satisfy the selected operation's engine contract.
    unsafe fn call(
        &self,
        property: Object,
        context: Object,
        operation: Hp2ValueCall,
        a: Object,
        b: Object,
    ) -> std::result::Result<i32, Self::Error>;
    /// Query whether an archive is loading.
    ///
    /// # Safety
    /// `archive` must be valid for the engine's loading query.
    unsafe fn integer(
        &self,
        archive: Object,
        field: Hp2ValueInteger,
    ) -> std::result::Result<i32, Self::Error>;
    /// Serialize one value through the requested wire primitive.
    ///
    /// # Safety
    /// `value` must point to the wire representation required by `wire`.
    unsafe fn wire(
        &self,
        archive: Object,
        wire: Hp2ValueWire,
        value: Object,
        argument: u32,
    ) -> std::result::Result<(), Self::Error>;
    /// Run typed string or array lifetime behavior.
    ///
    /// # Safety
    /// `value` must be a live wrapper of the selected kind.
    unsafe fn storage(
        &self,
        kind: Hp2ValueStorage,
        value: Object,
    ) -> std::result::Result<(), Self::Error>;
    /// Validate an array addition before the value policy publishes its count.
    ///
    /// # Safety
    /// `count`, `num`, and `max` must be the current array values.
    unsafe fn array_check_add(
        &self,
        count: i32,
        num: i32,
        max: i32,
    ) -> std::result::Result<(), Self::Error>;
    /// Check an allocation byte count against the engine's signed-size limit.
    ///
    /// # Safety
    /// `bytes` must be the count calculated by the active engine operation.
    unsafe fn checked_int_size(
        &self,
        bytes: usize,
    ) -> std::result::Result<i32, Self::Error>;
    /// Reallocate property storage, returning the new data pointer on success.
    ///
    /// # Safety
    /// `data`, `max`, `width`, and `tag` must describe the live allocation.
    unsafe fn array_reallocate(
        &self,
        data: Object,
        bytes: usize,
        max: *const i32,
        width: i32,
        tag: *const c_void,
    ) -> std::result::Result<Object, Self::Error>;
    /// Free one allocation using the engine allocator.
    ///
    /// # Safety
    /// `data` must be null or a live allocation owned by this engine allocator.
    unsafe fn array_free(&self, data: Object) -> std::result::Result<(), Self::Error>;
}
struct LegacyPropertyValueOps<'a> {
    services: &'a Hp2PropertyValueServices,
}

impl PropertyValueOps for LegacyPropertyValueOps<'_> {
    type Error = i32;

    fn none_name(&self) -> i32 {
        self.services.none_name
    }

    fn vector_name(&self) -> i32 {
        self.services.vector_name
    }

    fn rotator_name(&self) -> i32 {
        self.services.rotator_name
    }

    fn plane_name(&self) -> i32 {
        self.services.plane_name
    }

    fn array_tag(&self) -> *const c_void {
        self.services.array_tag
    }

    fn string_tag(&self) -> *const c_void {
        self.services.string_tag
    }

    unsafe fn pointer(
        &self,
        object: Object,
        field: Hp2ValuePointer,
    ) -> std::result::Result<Object, Self::Error> {
        let mut result = std::ptr::null_mut();
        status(unsafe { (self.services.pointer)(object, field, &mut result) })?;
        Ok(result)
    }

    unsafe fn call(
        &self,
        property: Object,
        context: Object,
        operation: Hp2ValueCall,
        a: Object,
        b: Object,
    ) -> std::result::Result<i32, Self::Error> {
        let mut result = 0;
        status(unsafe {
            (self.services.call)(property, context, operation, a, b, &mut result)
        })?;
        Ok(result)
    }

    unsafe fn integer(
        &self,
        archive: Object,
        field: Hp2ValueInteger,
    ) -> std::result::Result<i32, Self::Error> {
        Ok(unsafe { (self.services.integer)(archive, field) })
    }

    unsafe fn wire(
        &self,
        archive: Object,
        wire: Hp2ValueWire,
        value: Object,
        argument: u32,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.wire)(archive, wire, value, argument) })
    }

    unsafe fn storage(
        &self,
        kind: Hp2ValueStorage,
        value: Object,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.storage)(kind, value) })
    }

    unsafe fn array_check_add(
        &self,
        count: i32,
        num: i32,
        max: i32,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.array_check_add)(count, num, max) })
    }

    unsafe fn checked_int_size(
        &self,
        bytes: usize,
    ) -> std::result::Result<i32, Self::Error> {
        let arrays = unsafe { *self.services.arrays };
        let mut result = 0;
        status(unsafe {
            (arrays.checked_int_size)(arrays.context, bytes, &mut result)
        })?;
        Ok(result)
    }

    unsafe fn array_reallocate(
        &self,
        data: Object,
        bytes: usize,
        max: *const i32,
        width: i32,
        tag: *const c_void,
    ) -> std::result::Result<Object, Self::Error> {
        let arrays = unsafe { *self.services.arrays };
        let mut result = std::ptr::null_mut();
        status(unsafe {
            (arrays.realloc)(
                arrays.context,
                data,
                bytes,
                max,
                width,
                tag,
                &mut result,
            )
        })?;
        Ok(result)
    }

    unsafe fn array_free(&self, data: Object) -> std::result::Result<(), Self::Error> {
        let arrays = unsafe { *self.services.arrays };
        status(unsafe { (arrays.free)(arrays.context, data) })
    }
}
fn at(p: Object, offset: i32) -> Object {
    p.cast::<u8>().wrapping_offset(offset as isize).cast()
}
fn optional_at(p: Object, offset: i32) -> Object {
    if p.is_null() { p } else { at(p, offset) }
}
unsafe fn ptr<O: PropertyValueOps + ?Sized>(
    s: &O,
    p: Object,
    f: Hp2ValuePointer,
) -> std::result::Result<Object, O::Error> {
    unsafe { s.pointer(p, f) }
}
unsafe fn call<O: PropertyValueOps + ?Sized>(
    s: &O,
    p: Object,
    c: Object,
    op: Hp2ValueCall,
    a: Object,
    b: Object,
) -> std::result::Result<i32, O::Error> {
    unsafe { s.call(p, c, op, a, b) }
}
unsafe fn offset(p: Object, index: i32) -> i32 {
    // SAFETY: The entrypoint requires live compiler-probed UProperty storage.
    unsafe {
        (*p.cast::<UProperty>())
            .offset
            .wrapping_add(index.wrapping_mul((*p.cast::<UProperty>()).element_size))
    }
}

unsafe fn memory_copy(a: Object, b: Object, size: i32) {
    // SAFETY: Native value ranges are valid for size bytes; copy permits overlap.
    if size != 0 {
        unsafe { ptr::copy(b.cast::<u8>(), a.cast::<u8>(), size as usize) };
    }
}

unsafe fn string_data(value: Object) -> *mut i32 {
    static EMPTY: i32 = 0;
    // SAFETY: FString's probed raw container header remains live for this read.
    unsafe {
        if array_num(value) == 0 {
            (&raw const EMPTY).cast_mut()
        } else {
            array_data(value).cast()
        }
    }
}

unsafe fn enum_count(property: Object) -> i32 {
    // SAFETY: Byte properties and their optional enum satisfy the native contract.
    unsafe {
        let enumeration = (*property.cast::<UByteProperty>()).r#enum;
        if enumeration.is_null() {
            -1
        } else {
            (*enumeration.cast::<UEnum>()).names.num
        }
    }
}

unsafe fn struct_kind<O: PropertyValueOps + ?Sized>(structure: Object, s: &O) -> i32 {
    // SAFETY: UStruct inherits the compiler-probed UObject name field.
    let name = unsafe { i32::from_ne_bytes((*structure.cast::<UObject>()).name) };
    if name == s.vector_name() {
        1
    } else if name == s.rotator_name() {
        2
    } else if name == s.plane_name() {
        3
    } else {
        0
    }
}

unsafe fn array_slots(value: Object) -> Hp2ArraySlots {
    let array = value.cast::<TArrayRaw>();
    // SAFETY: The live probed header has independently writable packed slots.
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

unsafe fn array_num(value: Object) -> i32 {
    // SAFETY: Native containers may be packed to four-byte alignment.
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).num).read_unaligned() }
}

unsafe fn array_data(value: Object) -> Object {
    // SAFETY: Native container pointer slots may be only four-byte aligned.
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).data).read_unaligned() }
}

unsafe fn array_max(value: Object) -> i32 {
    // SAFETY: Native containers may be packed to four-byte alignment.
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).max).read_unaligned() }
}

unsafe fn array_reallocate<O: PropertyValueOps + ?Sized>(
    s: &O,
    slots: Hp2ArraySlots,
    width: i32,
    tag: *const c_void,
) -> std::result::Result<(), O::Error> {
    unsafe {
        let max = slots.max.read();
        let bytes = (max as usize).wrapping_mul(width as usize);
        let data = slots.data.read_unaligned();
        let data = s.array_reallocate(data, bytes, slots.max, width, tag)?;
        slots.data.write_unaligned(data);
    }
    Ok(())
}

unsafe fn array_empty<O: PropertyValueOps + ?Sized>(
    s: &O,
    a: Object,
    size: i32,
    slack: i32,
) -> std::result::Result<(), O::Error> {
    // Preserve the original no-element-destruction policy and publication order.
    unsafe {
        let slots = array_slots(a);
        slots.num.write(0);
        slots.max.write(slack);
        array_reallocate(s, slots, size, s.array_tag())
    }
}

unsafe fn array_add<O: PropertyValueOps + ?Sized>(
    s: &O,
    a: Object,
    size: i32,
    count: i32,
    zero: bool,
) -> std::result::Result<(), O::Error> {
    unsafe {
        s.array_check_add(count, array_num(a), array_max(a))?;
        let slots = array_slots(a);
        let index = slots.num.read();
        let num = index.wrapping_add(count);
        slots.num.write(num);
        if num > slots.max.read() {
            slots
                .max
                .write(num.wrapping_add(num.wrapping_mul(3) / 8).wrapping_add(32));
            array_reallocate(s, slots, size, s.array_tag())?;
        }
        if zero {
            let bytes = (count as usize).wrapping_mul(size as usize);
            let bytes = s.checked_int_size(bytes)? as usize;
            if bytes != 0 {
                let data = slots.data.read_unaligned().cast::<u8>();
                ptr::write_bytes(
                    data.add((index as usize).wrapping_mul(size as usize)),
                    0,
                    bytes,
                );
            }
        }
    }
    Ok(())
}

unsafe fn string_assign<O: PropertyValueOps + ?Sized>(
    s: &O,
    a: Object,
    b: Object,
) -> std::result::Result<(), O::Error> {
    unsafe {
        let dst = array_slots(a);
        let src = array_slots(b);
        if dst.data == src.data {
            return Ok(());
        }
        let count = src.num.read();
        dst.num.write(count);
        dst.max.write(count);
        array_reallocate(s, dst, std::mem::size_of::<i32>() as i32, s.string_tag())?;
        if dst.num.read() != 0 {
            let bytes = (dst.num.read() as usize)
                .wrapping_mul(std::mem::size_of::<i32>());
            let bytes = s.checked_int_size(bytes)? as usize;
            let src_data = string_data(b);
            let dst_data = array_data(a).cast::<i32>();
            ptr::copy_nonoverlapping(
                src_data.cast::<u8>(),
                dst_data.cast::<u8>(),
                bytes,
            );
        }
    }
    Ok(())
}
// This is TFieldIterator's live cursor, not a snapshot of reflected fields.
struct Fields {
    owner: Object,
    field: Object,
}
impl Fields {
    unsafe fn new<O: PropertyValueOps + ?Sized>(
        owner: Object,
        s: &O,
    ) -> std::result::Result<Self, O::Error> {
        let mut f = Self {
            owner,
            field: if owner.is_null() {
                owner
            } else {
                unsafe { (*owner.cast::<UStruct>()).children }
            },
        };
        unsafe {
            f.seek(s)?;
        }
        Ok(f)
    }
    unsafe fn seek<O: PropertyValueOps + ?Sized>(
        &mut self,
        s: &O,
    ) -> std::result::Result<(), O::Error> {
        unsafe {
            while !self.owner.is_null() {
                while !self.field.is_null() {
                    if crate::object_model::is_a(
                        self.field,
                        ptr(s, self.field, Hp2ValuePointer::ValuePropertyClass)?,
                    ) {
                        return Ok(());
                    }
                    self.field = (*self.field.cast::<UField>()).next;
                }
                self.owner = ptr(s, self.owner, Hp2ValuePointer::ValueSuper)?;
                if !self.owner.is_null() {
                    self.field = (*self.owner.cast::<UStruct>()).children;
                }
            }
        }
        Ok(())
    }
    unsafe fn advance<O: PropertyValueOps + ?Sized>(
        &mut self,
        s: &O,
    ) -> std::result::Result<(), O::Error> {
        self.field = unsafe { (*self.field.cast::<UField>()).next };
        unsafe { self.seek(s) }
    }
}

/// Runs the shared property policy through the existing C callback table.
///
/// # Safety
/// All objects and values must satisfy the native property's layout/lifetime contract.
pub unsafe fn run(
    kind: Hp2ValueKind,
    op: Hp2ValueOperation,
    p: Object,
    c: Object,
    a: Object,
    b: Object,
    index: i32,
    s: &Hp2PropertyValueServices,
) -> Result<i32> {
    let legacy = LegacyPropertyValueOps { services: s };
    unsafe { run_with(kind, op, p, c, a, b, index, &legacy) }
}

/// Runs property value operations through a Rust-owned capability implementation.
///
/// # Safety
/// All objects and values must satisfy the compiler-probed property's layout,
/// lifetime, and operation contract. Hooks may reenter; their implementation
/// must preserve every live raw slot required by the active operation.
pub unsafe fn run_with<O: PropertyValueOps>(
    kind: Hp2ValueKind,
    op: Hp2ValueOperation,
    p: Object,
    c: Object,
    a: Object,
    b: Object,
    index: i32,
    s: &O,
) -> std::result::Result<i32, O::Error> {
    use Hp2ValueCall::*;
    use Hp2ValueKind::*;
    use Hp2ValueOperation::*;
    use Hp2ValueStorage::*;
    use Hp2ValueWire::*;
    unsafe {
        if kind == ValueMap {
            // UnProp.cpp's UMapProperty value operations intentionally have
            // their array/lifetime implementation commented out. Identical,
            // serialization, copy and destruction have no value effect;
            // NetSerializeItem reports success without touching map storage.
            return Ok(1);
        }
        if op == ValueMatches {
            let ofs = offset(p, index);
            return call(s, p, c, ValueCallIdentical, at(a, ofs), optional_at(b, ofs));
        }
        if op == ValueCopyComplete {
            if kind == ValueByte {
                let n = (*p.cast::<UProperty>()).array_dim;
                if n == 1 {
                    copy_scalar(kind, a, b);
                } else {
                    memory_copy(a, b, n);
                }
            } else if matches!(kind, ValueInt | ValueFloat | ValueName | ValueObject) {
                // Preserve forward assignment (including overlap) and live ArrayDim
                // without a native getter crossing for each copied element.
                let dim = ptr::addr_of_mut!((*p.cast::<UProperty>()).array_dim);
                if dim.read() == 1 {
                    copy_scalar(kind, a, b);
                } else {
                    let size = if kind == ValueObject {
                        std::mem::size_of::<Object>() as i32
                    } else {
                        4
                    };
                    let mut i: i32 = 0;
                    while i < dim.read() {
                        let ofs = i.wrapping_mul(size);
                        copy_scalar(kind, at(a, ofs), at(b, ofs));
                        i = i.wrapping_add(1);
                    }
                }
            } else {
                let mut i: i32 = 0;
                while i < (*p.cast::<UProperty>()).array_dim {
                    let ofs = i.wrapping_mul((*p.cast::<UProperty>()).element_size);
                    call(s, p, c, ValueCallCopySingle, at(a, ofs), at(b, ofs))?;
                    i = i.wrapping_add(1);
                }
            }
            return Ok(1);
        }
        if kind == ValueFixed {
            if op == ValueNetSerialize {
                // UFixedArrayProperty::NetSerializeItem is an intentional native success no-op.
                return Ok(1);
            }
            let mut i: i32 = 0;
            while i < (*p.cast::<UFixedArrayProperty>()).count {
                let inner = (*p.cast::<UFixedArrayProperty>()).inner;
                let ofs = i.wrapping_mul((*inner.cast::<UProperty>()).element_size);
                let action = match op {
                    ValueIdentical => ValueCallIdentical,
                    ValueCopySingle => ValueCallCopyComplete,
                    ValueDestroy => ValueCallDestroy,
                    ValueSerialize => ValueCallSerialize,
                    _ => unreachable!(),
                };
                let result = call(s, inner, c, action, at(a, ofs), optional_at(b, ofs))?;
                if op == ValueIdentical && result == 0 {
                    return Ok(0);
                }
                i = i.wrapping_add(1);
            }
            return Ok(1);
        }
        if kind == ValueArray {
            if op == ValueNetSerialize {
                // UArrayProperty::NetSerializeItem is an intentional native success no-op.
                return Ok(1);
            }
            return array(op, p, c, a, b, s);
        }
        if kind == ValueStruct {
            return structure(op, p, c, a, b, s);
        }
        match op {
            ValueIdentical => Ok(i32::from(match kind {
                ValueByte => {
                    a.cast::<u8>().read()
                        == if b.is_null() { 0 } else { b.cast::<u8>().read() }
                }
                ValueGeneric => {
                    let size = (*p.cast::<UProperty>()).element_size.max(0) as usize;
                    let left = std::slice::from_raw_parts(a.cast::<u8>(), size);
                    if b.is_null() { left.iter().all(|byte| *byte == 0) }
                    else { left == std::slice::from_raw_parts(b.cast::<u8>(), size) }
                }
                ValueInt => {
                    a.cast::<i32>().read()
                        == if b.is_null() { 0 } else { b.cast::<i32>().read() }
                }
                ValueFloat => {
                    a.cast::<f32>().read()
                        == if b.is_null() {
                            0.0
                        } else {
                            b.cast::<f32>().read()
                        }
                }
                ValueBool => {
                    ((a.cast::<u32>().read()
                        ^ if b.is_null() {
                            0
                        } else {
                            b.cast::<u32>().read()
                        })
                        & (*p.cast::<UBoolProperty>()).bit_mask as u32)
                        == 0
                }
                ValueObject => {
                    a.cast::<Object>().read()
                        == if b.is_null() {
                            std::ptr::null_mut()
                        } else {
                            b.cast::<Object>().read()
                        }
                }
                ValueName => {
                    a.cast::<i32>().read()
                        == if b.is_null() {
                            s.none_name()
                        } else {
                            b.cast::<i32>().read()
                        }
                }
                ValueString => {
                    let x = string_data(a);
                    let empty = 0;
                    let y = if b.is_null() {
                        (&empty as *const i32).cast()
                    } else {
                        string_data(b).cast_const()
                    };
                    crate::ansi::stricmp(x.cast(), y.cast()) == 0
                }
                _ => unreachable!(),
            })),
            ValueCopySingle => {
                match kind {
                    ValueGeneric => memory_copy(a, b, (*p.cast::<UProperty>()).element_size),
                    ValueBool => {
                        let mask = (*p.cast::<UBoolProperty>()).bit_mask as u32;
                        a.cast::<u32>().write(
                            (a.cast::<u32>().read() & !mask) | (b.cast::<u32>().read() & mask),
                        );
                    }
                    ValueString => string_assign(s, a, b)?,
                    _ => copy_scalar(kind, a, b),
                }
                Ok(1)
            }
            ValueDestroy => {
                if kind == ValueString {
                    let mut i: i32 = 0;
                    while i < (*p.cast::<UProperty>()).array_dim {
                        s.storage(
                            ValueStringDestroy,
                            at(a, i.wrapping_mul((*p.cast::<UProperty>()).element_size)),
                        )?;
                        i = i.wrapping_add(1);
                    }
                }
                Ok(1)
            }
            ValueSerialize | ValueNetSerialize => {
                if kind == ValueGeneric {
                    call(s, p, c, ValueCallSerialize, a, b)?;
                    return Ok(1);
                }
                if kind == ValueObject && op == ValueNetSerialize {
                    return call(s, p, c, ValueCallObjectNet, a, b);
                }
                if kind == ValueBool {
                    let mask = (*p.cast::<UBoolProperty>()).bit_mask as u32;
                    let mut value = u8::from(a.cast::<u32>().read() & mask != 0);
                    s.wire(
                        c,
                        if op == ValueSerialize {
                            ValueWireByte
                        } else {
                            ValueWireBits
                        },
                        (&mut value as *mut u8).cast(),
                        1,
                    )?;
                    // Re-read both mask and data after the potentially reentrant archive call.
                    let mask = (*p.cast::<UBoolProperty>()).bit_mask as u32;
                    let old = a.cast::<u32>().read();
                    a.cast::<u32>()
                        .write(if value != 0 { old | mask } else { old & !mask });
                    return Ok(1);
                }
                let wire = match kind {
                    ValueByte => ValueWireByte,
                    ValueInt => ValueWireInt,
                    ValueFloat => ValueWireFloat,
                    ValueObject => ValueWireObject,
                    ValueName => ValueWireName,
                    ValueString => ValueWireString,
                    _ => unreachable!(),
                };
                if kind == ValueByte && op == ValueNetSerialize {
                    let count = enum_count(p);
                    let bits = if count == -1 {
                        8
                    } else {
                        crate::bits::ceil_log_two(count as u32) as u32
                    };
                    s.wire(c, ValueWireBits, a, bits)?;
                } else {
                    s.wire(c, wire, a, 0)?;
                }
                Ok(1)
            }
            _ => unreachable!(),
        }
    }
}
unsafe fn copy_scalar(kind: Hp2ValueKind, a: Object, b: Object) {
    use Hp2ValueKind::*;
    unsafe {
        match kind {
            ValueByte => a.cast::<u8>().write(b.cast::<u8>().read()),
            ValueInt | ValueName => a.cast::<i32>().write(b.cast::<i32>().read()),
            ValueFloat => a.cast::<f32>().write(b.cast::<f32>().read()),
            ValueObject => a.cast::<Object>().write(b.cast::<Object>().read()),
            _ => unreachable!(),
        }
    }
}
unsafe fn array<O: PropertyValueOps + ?Sized>(
    op: Hp2ValueOperation,
    p: Object,
    c: Object,
    a: Object,
    b: Object,
    s: &O,
) -> std::result::Result<i32, O::Error> {
    use Hp2ValueCall::*;
    use Hp2ValueInteger::*;
    use Hp2ValueOperation::*;
    use Hp2ValueStorage::*;
    unsafe {
        match op {
            ValueIdentical => {
                let n = array_num(a);
                if n != if b.is_null() { 0 } else { array_num(b) } {
                    return Ok(0);
                }
                let size = (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).element_size;
                let x = array_data(a);
                let y = if b.is_null() { b } else { array_data(b) };
                for i in 0..n {
                    if call(
                        s,
                        (*p.cast::<UArrayProperty>()).inner,
                        c,
                        ValueCallIdentical,
                        at(x, i.wrapping_mul(size)),
                        if b.is_null() {
                            b
                        } else {
                            at(y, i.wrapping_mul(size))
                        },
                    )? == 0
                    {
                        return Ok(0);
                    }
                }
            }
            ValueSerialize => {
                let size = (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).element_size;
                let mut n = array_num(a);
                s.wire(
                    c,
                    Hp2ValueWire::ValueWireIndex,
                    (&mut n as *mut i32).cast(),
                    0,
                )?;
                if s.integer(c, ValueLoading)? != 0 {
                    array_empty(s, a, size, 0)?;
                    array_add(s, a, size, n, false)?;
                }
                let data = array_data(a);
                for i in 0..n {
                    call(
                        s,
                        (*p.cast::<UArrayProperty>()).inner,
                        c,
                        ValueCallSerialize,
                        at(data, i.wrapping_mul(size)),
                        b,
                    )?;
                }
            }
            ValueCopySingle => {
                let size = (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).element_size;
                // Deliberately does not destruct old elements, matching the original copy.
                array_empty(s, a, size, array_num(b))?;
                if (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).property_flags
                    & CPF_NEED_CTOR_LINK
                    != 0
                {
                    array_add(s, a, size, array_num(b), true)?;
                    let src = array_data(b);
                    let dst = array_data(a);
                    let mut i: i32 = 0;
                    while i < array_num(a) {
                        call(
                            s,
                            (*p.cast::<UArrayProperty>()).inner,
                            c,
                            ValueCallCopyComplete,
                            at(dst, i.wrapping_mul(size)),
                            at(src, i.wrapping_mul(size)),
                        )?;
                        i = i.wrapping_add(1);
                    }
                } else {
                    array_add(s, a, size, array_num(b), false)?;
                    memory_copy(
                        array_data(a),
                        array_data(b),
                        array_num(b).wrapping_mul(size),
                    );
                }
            }
            ValueDestroy => {
                if (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).property_flags
                    & CPF_NEED_CTOR_LINK
                    != 0
                {
                    let data = array_data(a);
                    let size =
                        (*(*p.cast::<UArrayProperty>()).inner.cast::<UProperty>()).element_size;
                    let mut i: i32 = 0;
                    while i < array_num(a) {
                        call(
                            s,
                            (*p.cast::<UArrayProperty>()).inner,
                            c,
                            ValueCallDestroy,
                            at(data, i.wrapping_mul(size)),
                            b,
                        )?;
                        i = i.wrapping_add(1);
                    }
                }
                s.storage(ValueArrayDestroy, a)?;
            }
            _ => unreachable!(),
        }
        Ok(1)
    }
}
unsafe fn structure<O: PropertyValueOps + ?Sized>(
    op: Hp2ValueOperation,
    p: Object,
    c: Object,
    a: Object,
    b: Object,
    s: &O,
) -> std::result::Result<i32, O::Error> {
    use Hp2ValueCall::*;
    use Hp2ValueOperation::*;
    unsafe {
        if op == ValueSerialize {
            call(
                s,
                (*p.cast::<UStructProperty>()).r#struct,
                c,
                ValueCallPreload,
                a,
                b,
            )?;
            call(
                s,
                (*p.cast::<UStructProperty>()).r#struct,
                c,
                ValueCallBinary,
                a,
                b,
            )?;
            return Ok(1);
        }
        if op == ValueDestroy {
            let mut field =
                (*(*p.cast::<UStructProperty>()).r#struct.cast::<UStruct>()).constructor_link;
            while !field.is_null() {
                let mut i: i32 = 0;
                while i < (*p.cast::<UProperty>()).array_dim {
                    let ofs = i
                        .wrapping_mul((*p.cast::<UProperty>()).element_size)
                        .wrapping_add((*field.cast::<UProperty>()).offset);
                    call(s, field, c, ValueCallDestroy, at(a, ofs), b)?;
                    i = i.wrapping_add(1);
                }
                field = (*field.cast::<UProperty>()).constructor_link_next;
            }
            return Ok(1);
        }
        if op == ValueNetSerialize {
            let kind = struct_kind((*p.cast::<UStructProperty>()).r#struct, s);
            if kind != 0 {
                return compressed(kind, c, a, s);
            }
        }
        let mut fields = Fields::new((*p.cast::<UStructProperty>()).r#struct, s)?;
        while !fields.field.is_null() {
            let field = fields.field;
            if op == ValueCopySingle {
                let ofs = (*field.cast::<UProperty>()).offset;
                call(s, field, c, ValueCallCopyComplete, at(a, ofs), at(b, ofs))?;
            } else if op != ValueNetSerialize || call(s, field, c, ValueCallMapIndex, a, b)? != -1 {
                let mut i: i32 = 0;
                while i < (*field.cast::<UProperty>()).array_dim {
                    let ofs = offset(field, i);
                    let result = call(
                        s,
                        field,
                        c,
                        if op == ValueIdentical {
                            ValueCallIdentical
                        } else {
                            ValueCallNetSerialize
                        },
                        at(a, ofs),
                        optional_at(b, ofs),
                    )?;
                    if op == ValueIdentical && result == 0 {
                        return Ok(0);
                    }
                    i = i.wrapping_add(1);
                }
            }
            fields.advance(s)?;
        }
        Ok(1)
    }
}
unsafe fn compressed<O: PropertyValueOps + ?Sized>(
    kind: i32,
    c: Object,
    a: Object,
    s: &O,
) -> std::result::Result<i32, O::Error> {
    use Hp2ValueWire::*;
    unsafe {
        match kind {
            1 => {
                // appRound on this ARM64 build is truncating FCVTZS, not nearest rounding.
                let x = a.cast::<f32>().read() as i32;
                let y = a.cast::<f32>().add(1).read() as i32;
                let z = a.cast::<f32>().add(2).read() as i32;
                let max = x.wrapping_abs().max(y.wrapping_abs()).max(z.wrapping_abs());
                let mut bits =
                    (crate::bits::ceil_log_two(max.wrapping_add(1) as u32) as u32).clamp(1, 16) - 1;
                s.wire(
                    c,
                    ValueWireBounded,
                    (&mut bits as *mut u32).cast(),
                    16,
                )?;
                let bias = 1i32.wrapping_shl(bits.wrapping_add(1));
                let bound = 1u32.wrapping_shl(bits.wrapping_add(2));
                let mut values = [
                    x.wrapping_add(bias) as u32,
                    y.wrapping_add(bias) as u32,
                    z.wrapping_add(bias) as u32,
                ];
                for i in 0..3 {
                    s.wire(
                        c,
                        ValueWireBounded,
                        values.as_mut_ptr().add(i).cast(),
                        bound,
                    )?;
                }
                if s.integer(c, Hp2ValueInteger::ValueLoading)? != 0 {
                    for (i, value) in values.into_iter().enumerate() {
                        a.cast::<f32>()
                            .add(i)
                            .write((value as i32).wrapping_sub(bias) as f32);
                    }
                }
            }
            2 => {
                let mut values = [
                    (a.cast::<i32>().read() >> 8) as u8,
                    (a.cast::<i32>().add(1).read() >> 8) as u8,
                    (a.cast::<i32>().add(2).read() >> 8) as u8,
                ];
                for i in 0..3 {
                    let mut present = u8::from(values[i] != 0);
                    s.wire(
                        c,
                        ValueWireBits,
                        (&mut present as *mut u8).cast(),
                        1,
                    )?;
                    if present != 0 {
                        s.wire(
                            c,
                            ValueWireByte,
                            values.as_mut_ptr().add(i).cast(),
                            0,
                        )?;
                    }
                    // Absent components retain their initial compressed byte, even on load.
                }
                if s.integer(c, Hp2ValueInteger::ValueLoading)? != 0 {
                    for (i, value) in values.into_iter().enumerate() {
                        a.cast::<i32>().add(i).write((value as i32) << 8);
                    }
                }
            }
            3 => {
                let mut values = [0i16; 4];
                for (i, value) in values.iter_mut().enumerate() {
                    *value = (a.cast::<f32>().add(i).read() as i32) as i16;
                }
                for i in 0..4 {
                    s.wire(
                        c,
                        ValueWireShort,
                        values.as_mut_ptr().add(i).cast(),
                        0,
                    )?;
                }
                if s.integer(c, Hp2ValueInteger::ValueLoading)? != 0 {
                    for (i, value) in values.into_iter().enumerate() {
                        a.cast::<f32>().add(i).write(value as f32);
                    }
                }
            }
            _ => unreachable!(),
        }
        Ok(1)
    }
}
