//! Variable, context and assignment execution with live native VM publication.
use crate::vm_bytecode;
use std::{cell::UnsafeCell, ffi::c_void, ptr};
type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;

struct LvalueState {
    property: Object,
    address: *mut u8,
}
/// Per-CoreAccess reflected lvalue slots.
///
/// Access is engine-thread-only, including synchronous reentry. Keep returned
/// raw slots—not references to their contents—across callbacks; nested VM
/// execution may overwrite them. Unlike the legacy process singleton, each
/// direct owner must create and retain its own instance.
pub struct EngineLvalueStorage(UnsafeCell<LvalueState>);
impl EngineLvalueStorage {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(LvalueState {
            property: ptr::null_mut(),
            address: ptr::null_mut(),
        }))
    }
    pub fn property_slot(&self) -> *mut *mut c_void {
        unsafe { ptr::addr_of_mut!((*self.0.get()).property) }
    }
    pub fn address_slot(&self) -> *mut *mut u8 {
        unsafe { ptr::addr_of_mut!((*self.0.get()).address) }
    }
}
struct LegacyLvalueStorage(UnsafeCell<LvalueState>);
// SAFETY: the legacy process singleton is accessed only under the documented
// engine-thread serialization contract, including synchronous reentry.
unsafe impl Sync for LegacyLvalueStorage {}
static LVALUE: LegacyLvalueStorage = LegacyLvalueStorage(UnsafeCell::new(LvalueState {
    property: ptr::null_mut(),
    address: ptr::null_mut(),
}));
fn legacy_property_slot() -> *mut *mut c_void {
    unsafe { ptr::addr_of_mut!((*LVALUE.0.get()).property) }
}
fn legacy_address_slot() -> *mut *mut u8 {
    unsafe { ptr::addr_of_mut!((*LVALUE.0.get()).address) }
}

/// The legacy process-lifetime interpreter property slot.
pub fn property_slot() -> *mut *mut c_void {
    legacy_property_slot()
}
/// The legacy process-lifetime interpreter address slot.
pub fn address_slot() -> *mut *mut u8 {
    legacy_address_slot()
}


#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2VmVariableOp {
    VmVariableLocal,
    VmVariableInstance,
    VmVariableDefault,
    VmVariableClassContext,
    VmVariableArrayElement,
    VmVariableDynArrayElement,
    VmVariableDynArrayCount,
    VmVariableBool,
    VmVariableStructMember,
    VmVariableNativeParm,
    VmVariableLet,
    VmVariableLetBool,
    VmVariableSelf,
    VmVariableContext,
    VmVariableStructCmpEq,
    VmVariableStructCmpNe,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2VmVariableField {
    VmVariableOffset,
    VmVariableArrayDim,
    VmVariableElementSize,
    VmVariableBitMask,
    VmVariableStructSize,
    VmVariableIsBool,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2VmVariableLink {
    VmVariableArrayCast,
    VmVariableInner,
    VmVariableConstructor,
    VmVariableConstructorNext,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2VmVariableCopy {
    VmVariableCopyComplete,
    VmVariableCopySingle,
    VmVariableCopyMember,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2VmVariableWarning {
    VmVariableNullClass,
    VmVariableNullContext,
    VmVariableBounds,
    VmVariableNullAssignment,
}
#[repr(C)]
pub struct Hp2VmVariableExecution {
    pub frame: Object,
    pub object: Object,
    pub code: *mut *mut u8,
    pub property: *mut Object,
    pub address: *mut *mut u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmVariableServices {
    pub frame_object: unsafe extern "C" fn(Object) -> Object,
    pub locals: unsafe extern "C" fn(Object) -> *mut u8,
    pub resolve: unsafe extern "C" fn(i32, *mut Object) -> i32,
    pub field: unsafe extern "C" fn(Object, Hp2VmVariableField) -> i32,
    pub link: unsafe extern "C" fn(Object, Hp2VmVariableLink) -> Object,
    pub step: unsafe extern "C" fn(Object, Object, Object) -> i32,
    pub dispatch: unsafe extern "C" fn(Object, Object, u8) -> i32,
    pub opcode_offset: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub trace: unsafe extern "C" fn(Object, Object, Hp2VmVariableOp, i32) -> i32,
    pub copy: unsafe extern "C" fn(Object, Object, *mut u8, Hp2VmVariableCopy) -> i32,
    pub warning: unsafe extern "C" fn(Object, Hp2VmVariableWarning, i32, i32) -> i32,
    pub default_address: unsafe extern "C" fn(Object, i32, *mut *mut u8) -> i32,
    pub default_object: unsafe extern "C" fn(Object, *mut Object) -> i32,
    pub array_num: unsafe extern "C" fn(Object) -> i32,
    pub array_data: unsafe extern "C" fn(Object) -> *mut u8,
    pub array_add: unsafe extern "C" fn(Object, i32, i32) -> i32,
    pub struct_outer: unsafe extern "C" fn(Object, *mut Object) -> i32,
    pub scratch: unsafe extern "C" fn(
        *const Hp2VmVariableExecution,
        Hp2VmVariableOp,
        Object,
        Object,
        i32,
        i32,
        Object,
    ) -> i32,
    pub destroy: unsafe extern "C" fn(Object, *mut u8) -> i32,
    pub compare: unsafe extern "C" fn(Object, *mut u8, *mut u8, *mut u32) -> i32,
    pub invalid_bool: unsafe extern "C" fn() -> i32,
    pub assignment_sink: *mut u8,
    pub assignment_zero_size: usize,
    /// Logs unwind history for ClassContext/ArrayElement/StructMember only,
    /// on their error path. `object` is the live native frame; the second
    /// argument selects which of the three native display names to log.
    pub unwind_guard: unsafe extern "C" fn(Object, i32) -> i32,
}
static mut VARIABLE_SERVICES: Option<Hp2VmVariableServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmVariableServices) {
    unsafe {
        (&raw mut VARIABLE_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmVariableServices> {
    unsafe { (&raw const VARIABLE_SERVICES).read().ok_or(1) }
}
fn status(value: i32) -> Result {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Result-based leaves for the shared variable/context/assignment kernel.
///
/// Implementations access canonical object/property/array owners directly.
/// Methods may reenter the VM; they must not retain borrows of live records,
/// frames, collection elements or metadata after returning.
pub trait VmVariableStorage {
    type Error;

    fn frame_object(&self, frame: Object) -> Object;
    fn locals(&self, frame: Object) -> *mut u8;
    fn resolve(&self, index: i32) -> std::result::Result<Object, Self::Error>;
    fn field(&self, property: Object, field: Hp2VmVariableField) -> i32;
    fn link(&self, property: Object, link: Hp2VmVariableLink) -> Object;
    fn dispatch(&self, frame: Object, object: Object, opcode: u8)
        -> std::result::Result<(), Self::Error>;
    fn opcode_offset(&self, frame: Object) -> std::result::Result<i32, Self::Error>;
    fn trace(
        &self,
        frame: Object,
        object: Object,
        op: Hp2VmVariableOp,
        offset: i32,
    ) -> std::result::Result<(), Self::Error>;
    fn copy(
        &self,
        property: Object,
        destination: Object,
        source: *mut u8,
        mode: Hp2VmVariableCopy,
    ) -> std::result::Result<(), Self::Error>;
    fn warning(
        &self,
        frame: Object,
        warning: Hp2VmVariableWarning,
        a: i32,
        b: i32,
    ) -> std::result::Result<(), Self::Error>;
    fn default_address(&self, object: Object, offset: i32)
        -> std::result::Result<*mut u8, Self::Error>;
    fn default_object(&self, class: Object) -> std::result::Result<Object, Self::Error>;
    fn array_num(&self, array: Object) -> i32;
    fn array_data(&self, array: Object) -> *mut u8;
    fn array_add(
        &self,
        array: Object,
        element_size: i32,
        count: i32,
    ) -> std::result::Result<(), Self::Error>;
    fn struct_outer(&self, property: Object) -> std::result::Result<Object, Self::Error>;
    /// Run direct scratch continuation synchronously with separate aligned buffers.
    /// Legacy adapters may route through their existing scratch callback instead.
    fn with_scratch(
        &self,
        execution: &Hp2VmVariableExecution,
        op: Hp2VmVariableOp,
        property: Object,
        structure: Object,
        size: usize,
        second_size: usize,
        result: Object,
        run: &mut dyn FnMut(*mut u8, *mut u8) -> std::result::Result<(), Self::Error>,
    ) -> std::result::Result<(), Self::Error>;
    fn destroy(&self, property: Object, address: *mut u8)
        -> std::result::Result<(), Self::Error>;
    fn compare(
        &self,
        structure: Object,
        first: *mut u8,
        second: *mut u8,
    ) -> std::result::Result<u32, Self::Error>;
    fn invalid_bool(&self) -> std::result::Result<(), Self::Error>;
    fn assignment_sink(&self) -> (*mut u8, usize);
}

#[derive(Clone, Copy)]
struct LegacyVariableStorage(Hp2VmVariableServices);

impl VmVariableStorage for LegacyVariableStorage {
    type Error = i32;

    fn frame_object(&self, frame: Object) -> Object {
        unsafe { (self.0.frame_object)(frame) }
    }
    fn locals(&self, frame: Object) -> *mut u8 {
        unsafe { (self.0.locals)(frame) }
    }
    fn resolve(&self, index: i32) -> Result<Object> {
        let mut object = ptr::null_mut();
        status(unsafe { (self.0.resolve)(index, &mut object) })?;
        Ok(object)
    }
    fn field(&self, property: Object, field: Hp2VmVariableField) -> i32 {
        unsafe { (self.0.field)(property, field) }
    }
    fn link(&self, property: Object, link: Hp2VmVariableLink) -> Object {
        unsafe { (self.0.link)(property, link) }
    }
    fn dispatch(&self, frame: Object, object: Object, opcode: u8) -> Result {
        status(unsafe { (self.0.dispatch)(frame, object, opcode) })
    }
    fn opcode_offset(&self, frame: Object) -> Result<i32> {
        let mut offset = 0;
        status(unsafe { (self.0.opcode_offset)(frame, &mut offset) })?;
        Ok(offset)
    }
    fn trace(
        &self,
        frame: Object,
        object: Object,
        op: Hp2VmVariableOp,
        offset: i32,
    ) -> Result {
        status(unsafe { (self.0.trace)(frame, object, op, offset) })
    }
    fn copy(
        &self,
        property: Object,
        destination: Object,
        source: *mut u8,
        mode: Hp2VmVariableCopy,
    ) -> Result {
        status(unsafe { (self.0.copy)(property, destination, source, mode) })
    }
    fn warning(
        &self,
        frame: Object,
        warning: Hp2VmVariableWarning,
        a: i32,
        b: i32,
    ) -> Result {
        status(unsafe { (self.0.warning)(frame, warning, a, b) })
    }
    fn default_address(&self, object: Object, offset: i32) -> Result<*mut u8> {
        let mut address = ptr::null_mut();
        status(unsafe { (self.0.default_address)(object, offset, &mut address) })?;
        Ok(address)
    }
    fn default_object(&self, class: Object) -> Result<Object> {
        let mut object = ptr::null_mut();
        status(unsafe { (self.0.default_object)(class, &mut object) })?;
        Ok(object)
    }
    fn array_num(&self, array: Object) -> i32 {
        unsafe { (self.0.array_num)(array) }
    }
    fn array_data(&self, array: Object) -> *mut u8 {
        unsafe { (self.0.array_data)(array) }
    }
    fn array_add(&self, array: Object, element_size: i32, count: i32) -> Result {
        status(unsafe { (self.0.array_add)(array, element_size, count) })
    }
    fn struct_outer(&self, property: Object) -> Result<Object> {
        let mut outer = ptr::null_mut();
        status(unsafe { (self.0.struct_outer)(property, &mut outer) })?;
        Ok(outer)
    }
    fn with_scratch(
        &self,
        execution: &Hp2VmVariableExecution,
        op: Hp2VmVariableOp,
        property: Object,
        structure: Object,
        size: usize,
        second_size: usize,
        result: Object,
        _run: &mut dyn FnMut(*mut u8, *mut u8) -> Result,
    ) -> Result {
        status(unsafe {
            (self.0.scratch)(
                execution,
                op,
                property,
                structure,
                size as i32,
                second_size as i32,
                result,
            )
        })
    }
    fn destroy(&self, property: Object, address: *mut u8) -> Result {
        status(unsafe { (self.0.destroy)(property, address) })
    }
    fn compare(&self, structure: Object, first: *mut u8, second: *mut u8) -> Result<u32> {
        let mut compared = 0;
        status(unsafe { (self.0.compare)(structure, first, second, &mut compared) })?;
        Ok(compared)
    }
    fn invalid_bool(&self) -> Result {
        status(unsafe { (self.0.invalid_bool)() })
    }
    fn assignment_sink(&self) -> (*mut u8, usize) {
        (self.0.assignment_sink, self.0.assignment_zero_size)
    }
}

unsafe fn operand<S: VmVariableStorage>(
    e: &Hp2VmVariableExecution,
    s: &S,
) -> std::result::Result<Object, S::Error> {
    unsafe {
        let object = s.resolve(vm_bytecode::peek::<i32>(e.code))?;
        vm_bytecode::advance(e.code, 4);
        Ok(object)
    }
}

unsafe fn offset<S: VmVariableStorage>(property: Object, s: &S) -> isize {
    unsafe { s.field(property, Hp2VmVariableField::VmVariableOffset) as isize }
}

/// Execute one reflected variable/context/assignment operation against direct
/// Result-based storage leaves. The policy is shared with the legacy adapter.
///
/// # Safety
/// The frame, object, code cursor, property/address slots and result are live raw
/// engine storage. `step` may reenter and mutate all of them; no Rust reference
/// into these records may survive a callback.
pub unsafe fn execute_with<S: VmVariableStorage, F>(
    op: Hp2VmVariableOp,
    e: &Hp2VmVariableExecution,
    result: Object,
    s: &S,
    step: &mut F,
) -> std::result::Result<(), S::Error>
where
    F: FnMut(Object, Object, Object) -> std::result::Result<(), S::Error>,
{
    use Hp2VmVariableCopy::*;
    use Hp2VmVariableField::*;
    use Hp2VmVariableLink::*;
    use Hp2VmVariableOp::*;
    use Hp2VmVariableWarning::*;
    unsafe {
        match op {
            VmVariableLocal | VmVariableInstance | VmVariableDefault => {
                let callsite = if op != VmVariableLocal { s.opcode_offset(e.frame)? } else { 0 };
                e.property.write(operand(e, s)?);
                let displacement = offset(e.property.read(), s);
                let address = if op == VmVariableLocal {
                    s.locals(e.frame).offset(displacement)
                } else if op == VmVariableInstance {
                    e.object.cast::<u8>().offset(displacement)
                } else {
                    s.default_address(e.object, displacement as i32)?
                };
                e.address.write(address);
                if op != VmVariableLocal {
                    s.trace(e.frame, e.object, op, callsite)?;
                }
                if !result.is_null() {
                    s.copy(e.property.read(), result, e.address.read(), VmVariableCopyComplete)?;
                }
            }
            VmVariableNativeParm => {
                let property = operand(e, s)?;
                if !result.is_null() {
                    let address = s.locals(e.frame).offset(offset(property, s));
                    e.address.write(address);
                    s.copy(property, result, address, VmVariableCopyComplete)?;
                }
            }
            VmVariableClassContext | VmVariableContext => {
                let mut context: Object = ptr::null_mut();
                let receiver = if op == VmVariableClassContext { s.frame_object(e.frame) } else { e.object };
                step(e.frame, receiver, (&raw mut context).cast())?;
                if !context.is_null() {
                    vm_bytecode::advance(e.code, 3);
                    if op == VmVariableClassContext {
                        context = s.default_object(context)?;
                    }
                    step(e.frame, context, result)?;
                } else {
                    s.warning(
                        e.frame,
                        if op == VmVariableClassContext { VmVariableNullClass } else { VmVariableNullContext },
                        0,
                        0,
                    )?;
                    let skip = vm_bytecode::read::<u16>(e.code);
                    let size = vm_bytecode::read::<u8>(e.code);
                    vm_bytecode::advance(e.code, skip as isize);
                    e.address.write(ptr::null_mut());
                    e.property.write(ptr::null_mut());
                    if !result.is_null() {
                        ptr::write_bytes(result.cast::<u8>(), 0, size as usize);
                    }
                }
            }
            VmVariableArrayElement | VmVariableDynArrayElement => {
                let mut index = 0i32;
                step(e.frame, s.frame_object(e.frame), (&raw mut index).cast())?;
                e.property.write(ptr::null_mut());
                step(e.frame, e.object, ptr::null_mut())?;
                if op == VmVariableArrayElement {
                    if !e.property.read().is_null() && !e.address.read().is_null() {
                        if index >= s.field(e.property.read(), VmVariableArrayDim) || index < 0 {
                            s.warning(e.frame, VmVariableBounds, index, s.field(e.property.read(), VmVariableArrayDim))?;
                            let maximum = s.field(e.property.read(), VmVariableArrayDim).wrapping_sub(1);
                            index = if index < 0 { 0 } else if index < maximum { index } else { maximum };
                        }
                        let displacement = index.wrapping_mul(s.field(e.property.read(), VmVariableElementSize));
                        e.address.write(e.address.read().offset(displacement as isize));
                        if !result.is_null() {
                            s.copy(e.property.read(), result, e.address.read(), VmVariableCopySingle)?;
                        }
                    }
                } else {
                    let property = s.link(e.property.read(), VmVariableArrayCast);
                    if !property.is_null() && !e.address.read().is_null() {
                        let array = e.address.read().cast();
                        if index < 0 {
                            s.warning(e.frame, VmVariableBounds, index, s.array_num(array))?;
                            index = 0;
                        } else if index >= s.array_num(array) {
                            s.array_add(array, s.field(s.link(property, VmVariableInner), VmVariableElementSize), index.wrapping_add(1).wrapping_sub(s.array_num(array)))?;
                        }
                        let displacement = index.wrapping_mul(s.field(s.link(property, VmVariableInner), VmVariableElementSize));
                        e.address.write(s.array_data(array).offset(displacement as isize));
                        if !result.is_null() {
                            s.copy(s.link(property, VmVariableInner), result, e.address.read(), VmVariableCopySingle)?;
                        }
                    }
                }
            }
            VmVariableDynArrayCount => {
                step(e.frame, e.object, ptr::null_mut())?;
                if !e.property.read().is_null() && !e.address.read().is_null() {
                    result.cast::<i32>().write(s.array_num(e.address.read().cast()));
                }
            }
            VmVariableBool => {
                let callsite = s.opcode_offset(e.frame)?;
                let opcode = vm_bytecode::read::<u8>(e.code);
                let property = s.resolve(vm_bytecode::peek::<i32>(e.code))?;
                s.dispatch(e.frame, e.object, opcode)?;
                e.property.write(property);
                s.trace(e.frame, e.object, op, callsite)?;
                if !result.is_null() {
                    let address = e.address.read();
                    let value = !address.is_null()
                        && (address.cast::<u32>().read() & s.field(e.property.read(), VmVariableBitMask) as u32) != 0;
                    result.cast::<u32>().write(u32::from(value));
                }
            }
            VmVariableLet => {
                e.address.write(ptr::null_mut());
                step(e.frame, s.frame_object(e.frame), ptr::null_mut())?;
                if e.address.read().is_null() {
                    s.warning(e.frame, VmVariableNullAssignment, 0, 0)?;
                    let (sink, size) = s.assignment_sink();
                    e.address.write(sink);
                    ptr::write_bytes(e.address.read(), 0, size);
                }
                step(e.frame, s.frame_object(e.frame), e.address.read().cast())?;
            }
            VmVariableLetBool => {
                e.address.write(ptr::null_mut());
                e.property.write(ptr::null_mut());
                step(e.frame, s.frame_object(e.frame), ptr::null_mut())?;
                let address = e.address.read().cast::<u32>();
                let property = e.property.read();
                let mut value = 0u32;
                step(e.frame, s.frame_object(e.frame), (&raw mut value).cast())?;
                if !address.is_null() {
                    if s.field(property, VmVariableIsBool) == 0 { s.invalid_bool()?; }
                    let mask = s.field(property, VmVariableBitMask) as u32;
                    address.write(if value != 0 { address.read() | mask } else { address.read() & !mask });
                }
            }
            VmVariableSelf => result.cast::<Object>().write(e.object),
            VmVariableStructMember | VmVariableStructCmpEq | VmVariableStructCmpNe => {
                let property = operand(e, s)?;
                let structure = if op == VmVariableStructMember { s.struct_outer(property)? } else { property };
                let size = s.field(structure, VmVariableStructSize);
                s.with_scratch(
                    e,
                    op,
                    property,
                    structure,
                    size as usize,
                    if op == VmVariableStructMember { 0 } else { size as usize },
                    result,
                    &mut |first, second| unsafe {
                        scratch_with(op, e, property, structure, first, second, result, s, step)
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// Legacy service-table adapter; direct CoreAccess uses [`execute_with`].
///
/// # Safety
/// Same raw storage and callback-lifetime requirements as [`execute_with`].
pub unsafe fn execute(
    op: Hp2VmVariableOp,
    e: &Hp2VmVariableExecution,
    result: Object,
    services: &Hp2VmVariableServices,
) -> Result {
    let storage = LegacyVariableStorage(*services);
    unsafe {
        execute_with(op, e, result, &storage, &mut |frame, object, output| {
            status((services.step)(frame, object, output))
        })
    }
}

/// Shared scratch-body kernel used by direct and legacy variable execution.
///
/// # Safety
/// Both buffers are distinct, aligned, writable and at least the live struct
/// size. They are valid only for this synchronous continuation.
pub unsafe fn scratch_with<S: VmVariableStorage, F>(
    op: Hp2VmVariableOp,
    e: &Hp2VmVariableExecution,
    property: Object,
    structure: Object,
    first: *mut u8,
    second: *mut u8,
    result: Object,
    s: &S,
    step: &mut F,
) -> std::result::Result<(), S::Error>
where
    F: FnMut(Object, Object, Object) -> std::result::Result<(), S::Error>,
{
    use Hp2VmVariableOp::*;
    unsafe {
        let size = s.field(structure, Hp2VmVariableField::VmVariableStructSize) as usize;
        ptr::write_bytes(first, 0, size);
        if op == VmVariableStructMember {
            e.address.write(ptr::null_mut());
            step(e.frame, e.object, first.cast())?;
            e.property.write(property);
            if !e.address.read().is_null() {
                e.address.write(e.address.read().offset(offset(property, s)));
            }
            if !result.is_null() {
                s.copy(property, result, first.offset(offset(property, s)), Hp2VmVariableCopy::VmVariableCopyMember)?;
            }
            let mut p = s.link(structure, Hp2VmVariableLink::VmVariableConstructor);
            while !p.is_null() {
                s.destroy(p, first.offset(offset(p, s)))?;
                p = s.link(p, Hp2VmVariableLink::VmVariableConstructorNext);
            }
        } else {
            ptr::write_bytes(second, 0, size);
            step(e.frame, e.object, first.cast())?;
            step(e.frame, e.object, second.cast())?;
            let compared = s.compare(structure, first, second)?;
            result.cast::<u32>().write(if op == VmVariableStructCmpNe {
                u32::from(compared == 0)
            } else { compared });
        }
    }
    Ok(())
}

/// Legacy scratch callback API retained for native callers.
///
/// # Safety
/// Buffers are valid live scratch storage and `services` remain callable.
pub unsafe fn scratch(
    op: Hp2VmVariableOp,
    e: &Hp2VmVariableExecution,
    property: Object,
    structure: Object,
    first: *mut u8,
    second: *mut u8,
    result: Object,
    services: &Hp2VmVariableServices,
) -> Result {
    let storage = LegacyVariableStorage(*services);
    unsafe {
        scratch_with(op, e, property, structure, first, second, result, &storage, &mut |frame, object, output| {
            status((services.step)(frame, object, output))
        })
    }
}
