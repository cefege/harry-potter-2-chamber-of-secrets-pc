//! Immutable descriptors and frame helpers for canonical Rust VM natives.

use crate::core_access::{CoreAccess, RuntimeResult};
use hp2_abi::engine::generated::{FFrame, UObject};
use std::ffi::c_void;

/// Rust handler for a canonical VM native. The receiver and frame belong to the supplied Core.
pub type CanonicalVmNative = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut FFrame, *mut c_void,
) -> RuntimeResult<()>;

/// Immutable identity and handler for an externally supplied canonical VM native.
#[derive(Clone, Copy)]
pub struct VmNativeSpec {
    /// Declaring native class name, without the `U` prefix.
    pub class_name: &'static str,
    /// Reflected function name.
    pub function_name: &'static str,
    /// Serialized native index, or `None` for name-resolved natives.
    pub native_index: Option<u16>,
    /// Canonical Rust implementation.
    pub handler: CanonicalVmNative,
}

/// Evaluates one expression argument using the canonical VM and advances its frame.
///
/// # Safety
/// `frame` points to a live canonical frame. `result` satisfies the next expression's
/// result layout and remains live for synchronous script reentry.
pub unsafe fn step_argument(
    access: CoreAccess<'_>, frame: *mut FFrame, result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { access.vm_step_argument(frame, result) }
}

/// Evaluates one reference argument and returns its live lvalue address, falling back to
/// `scratch` when the expression is a value rather than an addressable variable.
///
/// # Safety
/// `frame` points to a live canonical frame and `scratch` satisfies the argument layout.
pub unsafe fn step_argument_out(
    access: CoreAccess<'_>, frame: *mut FFrame, scratch: *mut c_void,
) -> RuntimeResult<*mut c_void> {
    unsafe { access.vm_step_argument_out(frame, scratch) }
}

/// Consumes the function-parameter terminator and verifies that native arguments are complete.
///
/// # Safety
/// `frame` must point to a live canonical VM frame at the first unconsumed argument.
pub unsafe fn finish_arguments(access: CoreAccess<'_>, frame: *mut FFrame) -> RuntimeResult<()> {
    unsafe { access.vm_finish_arguments(frame) }
}

/// Applies native FFrame::Logf formatting, suppression, and strict-mode errors.
///
/// # Safety
/// `frame`, its object/node, and its code position are live canonical VM records.
pub unsafe fn log_frame(access: CoreAccess<'_>, frame: *mut FFrame, message: &[u32]) -> RuntimeResult<()> {
    crate::core_vm::log_frame(access, frame, "Log", message)
}

/// Applies FFrame::Log suppression and serialization without Logf's input clipping.
///
/// # Safety
/// `frame`, its object/node, and its code position are live canonical VM records.
pub unsafe fn log_frame_message(access: CoreAccess<'_>, frame: *mut FFrame, message: &[u32]) -> RuntimeResult<()> {
    crate::core_vm::log_frame_message(access, frame, "Log", message)
}

/// Runs source native foreach control after the argument terminator.
///
/// # Safety
/// `frame` and its bytecode remain live. Selection may reenter script, but retains no VM
/// scratch pointers. The selector must write null to its out argument on exhaustion.
pub unsafe fn run_iterator(
    access: CoreAccess<'_>, frame: *mut FFrame,
    get_next: impl FnMut() -> RuntimeResult<bool>,
) -> RuntimeResult<()> {
    unsafe { crate::core_vm::run_native_iterator(access, frame, get_next) }
}
