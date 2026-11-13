//! Selected DrawHotKeyText offset RHS policies.
//! Retained VM comparison policy; not installed by the HUD overlay.
use std::{ffi::c_void, mem::MaybeUninit, ptr};
use hp2_abi::engine::generated::FFrame;
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_native_catalog::CoreNative,
    object_owner::CanonicalNativeBinding,
    ScriptCallArguments,
};

fn native_error(error: RuntimeError, native: CoreNative, opcode: u8) -> RuntimeError {
    error.with_context(&format!("Core native {:?} ({})", CanonicalNativeBinding::Core(native), opcode))
}

/// One selected float subtraction, or one canonical current child step.
///
/// # Safety
/// This is the currently executing admitted RHS frame. Its actual typed
/// operands, immutable script and result remain valid through reentry. The
/// destination is a writable aligned f32 slot. Neither frame nor capability
/// may escape, and no suspended parent may be used.
unsafe fn subtract(
    args: &ScriptCallArguments<'_>, frame: *mut FFrame, base: *const u8,
    site: usize, dst: *mut f32,
) -> RuntimeResult<()> {
    // SAFETY: current frame and immutable admitted allocation are live.
    let code = unsafe { ptr::addr_of!((*frame).code).read() };
    if code.cast_const() != base.wrapping_add(site) || unsafe { code.read() } != 0xaf {
        // SAFETY: no opcode consumed; Core steps the current child once.
        return unsafe { args.step_argument(frame, dst.cast()) };
    }
    // SAFETY: this is the current matched one-byte native 175 opcode.
    unsafe { ptr::addr_of_mut!((*frame).code).write(code.add(1)); }
    let result = (|| {
        // Match Core float native: three zero-initialized aligned slots, two
        // children then finish, and read only after synchronous callbacks.
        let mut values = [0.0_f32; 3];
        let slots = values.as_mut_ptr();
        // SAFETY: slots belong to this current native and remain live/reentrant.
        unsafe { args.step_argument(frame, slots.cast())?; }
        unsafe { args.step_argument(frame, slots.add(1).cast())?; }
        unsafe { args.finish_arguments(frame)?; }
        // SAFETY: the original native storage stays live through finish.
        let (left, right) = unsafe { (slots.read(), slots.add(1).read()) };
        unsafe { dst.write(left - right); }
        Ok(())
    })();
    result.map_err(|error| native_error(error, CoreNative::intUObjectexecSubtract_FloatFloat, 175))
}

/// Select an offset's actual outer FloatToInt and nested float subtraction.
///
/// # Safety
/// Core invokes this on the selected current Let RHS and its real i32 lvalue
/// or existing sink. Snapshot identity and all fully-writing typed continuations
/// are admitted after the guard and before this statement's effects. Storage and immutable
/// code remain live through reentry. No frame, operand capability, context or
/// destination escapes this invocation.
unsafe fn offset(
    args: &ScriptCallArguments<'_>, frame: *mut FFrame, object: *mut c_void,
    dst: *mut c_void, root: usize,
) -> RuntimeResult<()> {
    let (base, length) = args.body_script_snapshot();
    // SAFETY: current frame and cursor satisfy Core's operand contract.
    let cursor = unsafe { ptr::addr_of_mut!((*frame).code) };
    let code = unsafe { cursor.read() };
    if base.is_null() || length > isize::MAX as usize || root.checked_add(1).is_none_or(|p| p >= length)
        || (base as usize).checked_add(length).is_none()
        || code.cast_const() != base.wrapping_add(root)
        || unsafe { code.read() } != 0x45
    {
        // The canonical Let supplies its own RHS context, which can differ
        // from frame.object. Dispatch the one current consumed opcode there.
        // SAFETY: the current opcode and context belong to this active Let.
        let opcode = unsafe { code.read() };
        unsafe { cursor.write(code.add(1)); }
        return unsafe { args.dispatch_operand(opcode, object, frame, dst) };
    }
    // SAFETY: matched current FloatToInt is a one-byte opcode.
    unsafe { cursor.write(code.add(1)); }
    let result = (|| {
        let mut value = MaybeUninit::<f32>::uninit();
        // SAFETY: child produces one fully initialized f32 on success.
        unsafe { subtract(args, frame, base, root + 1, value.as_mut_ptr())?; }
        let value = unsafe { value.assume_init() };
        // SAFETY: actual Let i32 destination or installed sink is writable.
        unsafe { dst.cast::<i32>().write_unaligned(value as i32); }
        Ok(())
    })();
    result.map_err(|error| native_error(error, CoreNative::intUObjectexecFloatToInt, 0x45))
}

/// # Safety
/// Same active-scope, typed continuation and storage contract as `offset`.
pub(super) unsafe fn x_offset(
    _: CoreAccess<'_>, a: &ScriptCallArguments<'_>, f: *mut FFrame,
    object: *mut c_void, dst: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { offset(a, f, object, dst, 0x189) }
}

/// # Safety
/// Same active-scope, typed continuation and storage contract as `offset`.
pub(super) unsafe fn y_offset(
    _: CoreAccess<'_>, a: &ScriptCallArguments<'_>, f: *mut FFrame,
    object: *mut c_void, dst: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { offset(a, f, object, dst, 0x1a8) }
}

const _: hp2_core::ScriptAssignmentValue = x_offset;
const _: hp2_core::ScriptAssignmentValue = y_offset;
