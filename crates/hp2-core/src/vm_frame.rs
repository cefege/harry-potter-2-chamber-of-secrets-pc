//! Plain FFrame initialization, diagnostic traversal and opcode consumption.
//! Native guards, typed script access and formatting/static scratch stay leaves.
use crate::ansi::{self, Hp2AnsiServices, Tchar};
use crate::vm_bytecode;
use crate::vm_registry::Hp2VmNativeHandler;
use hp2_abi::engine::generated::{FFrame, FStateFrame};
use std::{
    ffi::{c_char, c_void},
    mem::MaybeUninit,
};

const _: () = {
    assert!(std::mem::align_of::<FStateFrame>() == std::mem::align_of::<FFrame>());
    assert!(std::mem::offset_of!(FStateFrame, current_frame) == size_of::<FFrame>());
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmFrameInitializationServices {
    pub object_class: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub script_code: unsafe extern "C" fn(*mut c_void, i32, *mut *mut u8) -> i32,
}

static mut INITIALIZATION_SERVICES: Option<Hp2VmFrameInitializationServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_initialization_services(value: Hp2VmFrameInitializationServices) {
    unsafe {
        (&raw mut INITIALIZATION_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_initialization_services() -> Result<Hp2VmFrameInitializationServices, i32> {
    unsafe { (&raw const INITIALIZATION_SERVICES).read().ok_or(1) }
}

unsafe fn initialize_object_fields(storage: *mut FFrame, object: *mut c_void, node: *mut c_void) {
    unsafe {
        std::ptr::addr_of_mut!((*storage).node).write(node);
        std::ptr::addr_of_mut!((*storage).object).write(object);
        std::ptr::addr_of_mut!((*storage).code).write(std::ptr::null_mut());
        std::ptr::addr_of_mut!((*storage).locals).write(std::ptr::null_mut());
        std::ptr::addr_of_mut!((*storage).parent).write(std::ptr::null_mut());
    }
}

/// # Safety
/// Storage is writable and aligned for FFrame. A nonnull object and its class
/// getter remain live; that primitive getter must not unwind.
pub unsafe fn initialize_object(
    storage: *mut FFrame,
    object: *mut c_void,
    services: &Hp2VmFrameInitializationServices,
) {
    unsafe {
        let node = if object.is_null() {
            std::ptr::null_mut()
        } else {
            (services.object_class)(object)
        };
        initialize_object_fields(storage, object, node);
    }
}

/// Initialize the derived fields without touching LatentAction or padding.
///
/// # Safety
/// Storage is writable and aligned for FStateFrame, with its FFrame base already
/// initialized. The opaque class handle has the native constructor's lifetime.
pub unsafe fn initialize_state_fields(storage: *mut FStateFrame, class: *mut c_void) {
    unsafe {
        std::ptr::addr_of_mut!((*storage).current_frame).write(std::ptr::null_mut());
        std::ptr::addr_of_mut!((*storage).state_node).write(class);
        std::ptr::addr_of_mut!((*storage).probe_mask).write(u64::MAX);
    }
}

/// Allocate persistent state with the SDK constructor's initialized field set.
///
/// Object and class are stored as opaque handles, never dereferenced here.
/// LatentAction and padding retain the SDK's uninitialized bytes. Use raw field
/// accesses, never a Rust reference to or copy of the whole mirror. Release this
/// allocation only with free_state; SDK caller-owned frames have a separate life.
pub fn allocate_state(object: *mut c_void, class: *mut c_void) -> *mut FStateFrame {
    let mut storage = Box::<FStateFrame>::new_uninit();
    let frame = storage.as_mut_ptr();
    unsafe {
        initialize_object_fields(frame.cast(), object, class);
        initialize_state_fields(frame, class);
    }
    Box::into_raw(storage).cast()
}

/// # Safety
/// A nonnull storage pointer is a live, uniquely owned allocate_state result.
/// No native frame reference or field access may survive this call.
pub unsafe fn free_state(storage: *mut FStateFrame) {
    if !storage.is_null() {
        // MaybeUninit is essential: destruction must not read LatentAction or
        // padding, including an incompletely loaded frame after an archive error.
        unsafe { drop(Box::from_raw(storage.cast::<MaybeUninit<FStateFrame>>())) };
    }
}

/// # Safety
/// Storage is writable and aligned for FFrame. Node, object, locals, parent and
/// services satisfy the native constructor's lifetimes. script_code captures
/// native exceptions and produces a live cursor on success. It can reenter:
/// no frame reference spans it. On failure only the prior node/object writes
/// are guaranteed; storage must not be published as an initialized frame.
pub unsafe fn initialize(
    storage: *mut FFrame,
    object: *mut c_void,
    node: *mut c_void,
    code_offset: i32,
    locals: *mut u8,
    parent: *mut FFrame,
    services: &Hp2VmFrameInitializationServices,
) -> Result<(), i32> {
    unsafe {
        std::ptr::addr_of_mut!((*storage).node).write(node);
        std::ptr::addr_of_mut!((*storage).object).write(object);
        let mut code = std::ptr::null_mut();
        completed((services.script_code)(node, code_offset, &raw mut code))?;
        std::ptr::addr_of_mut!((*storage).code).write(code);
        std::ptr::addr_of_mut!((*storage).locals).write(locals);
        std::ptr::addr_of_mut!((*storage).parent).write(parent);
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmFrameServices {
    pub critical_name: i32,
    pub strict: *const i32,
    pub ansi: *const Hp2AnsiServices,
    pub buffer: unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub outer: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub format_description: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub error: unsafe extern "C" fn(*const c_void, *const c_void) -> i32,
    pub warning: unsafe extern "C" fn(*const c_void, *const c_void) -> i32,
    pub native_table: *const Hp2VmNativeHandler,
    /// `Stack.Logf(TEXT("%s"), *S)`: the full FOutputDevice::Logf path
    /// (SafeSuppressed(NAME_Log), 4096-char formatting buffer) ahead of
    /// FFrame::Serialize, not decomposed into primitives here.
    pub logf: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
}

static mut FRAME_SERVICES: Option<Hp2VmFrameServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmFrameServices) {
    unsafe {
        (&raw mut FRAME_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmFrameServices, i32> {
    unsafe { (&raw const FRAME_SERVICES).read().ok_or(1) }
}

fn completed(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/// # Safety
/// Native frame/text/services remain live; strict is readable unless event is
/// critical. Native callbacks capture exceptions rather than unwinding Rust.
pub unsafe fn serialize(
    frame: *const c_void,
    text: *const c_void,
    event: i32,
    services: &Hp2VmFrameServices,
) -> Result<(), i32> {
    unsafe {
        if event == services.critical_name || services.strict.read() != 0 {
            completed((services.error)(frame, text))
        } else {
            completed((services.warning)(frame, text))
        }
    }
}

/// # Safety
/// Native frame/parent links are live and acyclic. The static buffer and all
/// formatted text fit its native 1024-TCHAR capacity; callbacks preserve its
/// terminating NUL. Formatting captures exceptions and may change the next
/// parent/node/root-object fields, which are read live through raw pointers.
/// The result has the same scratch lifetime as native appStaticString1024.
pub unsafe fn describe(
    root: *const FFrame,
    services: &Hp2VmFrameServices,
) -> Result<*const c_void, i32> {
    unsafe {
        let mut buffer = std::ptr::null_mut();
        completed((services.buffer)(root.cast(), &mut buffer))?;
        let buffer = buffer.cast::<Tchar>();
        buffer.write(0);
        if !(*root).object.is_null() {
            let mut frame = root;
            while !frame.is_null() {
                let node = (*frame).node;
                if !node.is_null() {
                    // Scan once, rather than strcat followed by strlen, using
                    // the same checked ANSI primitive as the native wrapper.
                    let length = ansi::strlen(&*services.ansi, buffer).map_err(|_| 1)?;
                    let mut end = buffer.add(length as usize);
                    if frame != root {
                        end.write(b'<' as Tchar);
                        end = end.add(1);
                        end.write(0);
                    }
                    let outer = (services.outer)((*root).object);
                    completed((services.format_description)(end.cast(), node, outer))?;
                }
                frame = (*frame).parent;
            }
        }
        Ok(buffer.cast())
    }
}

/// Consume one opcode and invoke a direct Rust operation without a native handler table.
///
/// The live cursor is updated before `execute` is called. No frame reference is
/// held while the operation can recursively evaluate bytecode or reenter the host.
///
/// # Safety
/// `frame` is a live writable `FFrame` whose `code` slot points at one readable
/// opcode byte. `context` and `result` satisfy the selected opcode's contract.
pub unsafe fn step_with<T, E>(
    frame: *mut FFrame,
    context: *mut c_void,
    result: *mut c_void,
    execute: impl FnOnce(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<T, E>,
) -> Result<T, E> {
    let opcode = unsafe { vm_bytecode::read::<u8>(std::ptr::addr_of_mut!((*frame).code)) };
    execute(opcode, context, frame, result)
}

/// Evaluate one expression argument using a direct Rust operation callback.
///
/// # Safety
/// The frame, result and callback satisfy [`step_with`]. The stable argument slot
/// remains writable through all recursive evaluation and host reentry.
#[inline]
pub unsafe fn step_argument_with<T, E>(
    frame: *mut FFrame,
    result: *mut c_void,
    execute: impl FnOnce(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<T, E>,
) -> Result<T, E> {
    let context = unsafe { std::ptr::addr_of!((*frame).object).read() };
    unsafe { step_with(frame, context, result, execute) }
}

/// Consume the argument terminator and optional debug expression directly.
///
/// This shares the cursor protocol with [`finish_arguments`], but carries the
/// caller's ordinary Rust error type and never consults the foreign dispatch table.
///
/// # Safety
/// The frame cursor addresses an argument terminator and a readable following
/// opcode. Frame object and callback satisfy [`step_argument_with`].
#[inline]
pub unsafe fn finish_arguments_with<E>(
    frame: *mut FFrame,
    mut execute: impl FnMut(u8, *mut c_void, *mut FFrame, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    let cursor = unsafe { std::ptr::addr_of_mut!((*frame).code) };
    unsafe {
        cursor.write(cursor.read().add(1));
        if cursor.read().read() == 0x38 {
            step_argument_with(frame, std::ptr::null_mut(), |op, context, frame, result| {
                execute(op, context, frame, result)
            })?;
        }
    }
    Ok(())
}

pub unsafe fn step(
    frame: *mut FFrame,
    context: *mut c_void,
    result: *mut c_void,
    services: &Hp2VmFrameServices,
) -> Result<(), i32> {
    unsafe { step_raw(frame, context, result, services.native_table) }
}

/// Step through the live native table without constructing a services adapter.
///
/// # Safety
/// Frame, context, result and table satisfy step. The opcode is consumed before
/// loading its handler; callbacks may replace table entries for subsequent steps.
#[inline]
pub unsafe fn step_raw(
    frame: *mut FFrame,
    context: *mut c_void,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        step_with(frame, context, result, |opcode, context, frame, result| {
            let handler = native_table.add(opcode as usize).read();
            completed(handler(context, frame.cast(), result))
        })
    }
}

/// Step a fixed argument using the live frame object, without retaining a borrow.
///
/// # Safety
/// Frame, result and table satisfy step_raw. The caller owns a stable argument
/// slot with its native initialization contract until all arguments finish.
#[inline]
pub unsafe fn step_argument(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        step_raw(
            frame,
            std::ptr::addr_of!((*frame).object).read(),
            result,
            native_table,
        )
    }
}

/// Consume the live argument terminator and optional native debug expression.
///
/// # Safety
/// The live cursor has a terminator and a readable following opcode. Frame and
/// table satisfy step_raw; argument slots remain live through debug reentry.
#[inline]
pub unsafe fn finish_arguments(
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        let cursor = std::ptr::addr_of_mut!((*frame).code);
        cursor.write(cursor.read().add(1));
        if cursor.read().read() == 0x38 {
            step_argument(frame, std::ptr::null_mut(), native_table)?;
        }
        Ok(())
    }
}

/// Legacy `Stack.Node ? Stack.Code - &Stack.Node->Script(0) : INDEX_NONE`,
/// reusing the Script(0) leaf also used for frame construction. Must be called
/// before any operand is parsed, to match the native offset exactly.
///
/// # Safety
/// Frame is live and initialized; services.script_code satisfies the native
/// Script(0) leaf for a live Node.
pub unsafe fn callsite_offset(
    frame: *const FFrame,
    services: &Hp2VmFrameInitializationServices,
) -> Result<i32, i32> {
    unsafe {
        let node = (*frame).node;
        if node.is_null() {
            return Ok(-1);
        }
        let mut base: *mut u8 = std::ptr::null_mut();
        let status = (services.script_code)(node, 0, &mut base);
        if status != 0 {
            return Err(status);
        }
        Ok((*frame).code.offset_from(base) as i32)
    }
}

/// Native RNG trace scope services for Rand/FRand/VRand/RotRand/RandRange.
/// `scope_begin`/`scope_end` mirror oracle `FAppRandTraceNativeScope`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmNativeTraceServices {
    pub frame: Hp2VmFrameInitializationServices,
    pub scope_begin:
        unsafe extern "C" fn(*mut c_void, *const c_char, i32, i32, *mut *mut c_void) -> i32,
    pub scope_end: unsafe extern "C" fn(*mut c_void) -> i32,
}

/// Open the native RNG trace scope. Must be called after P_FINISH (after all
/// operands are parsed), with the offset computed before them by callsite_offset.
///
/// # Safety
/// Frame is live; native is null or a NUL-terminated string outliving the
/// scope. Services catch native exceptions.
pub unsafe fn open_native_trace_scope(
    frame: *const FFrame,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
    services: &Hp2VmNativeTraceServices,
) -> Result<*mut c_void, i32> {
    unsafe {
        let mut node = std::ptr::null_mut();
        let status = (services.scope_begin)(
            frame as *mut c_void,
            native,
            native_slot,
            callsite_offset,
            &mut node,
        );
        if status != 0 { Err(status) } else { Ok(node) }
    }
}

/// Close a scope opened by open_native_trace_scope. Must run on every path,
/// including when the scoped work fails.
///
/// # Safety
/// node is null or a live handle from open_native_trace_scope, closed once.
pub unsafe fn close_native_trace_scope(
    node: *mut c_void,
    services: &Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let status = (services.scope_end)(node);
        if status != 0 { Err(status) } else { Ok(()) }
    }
}
