//! Named health-background white-channel RHS policy.
//!
//! This policy is not runtime-qualified. Core's current Let host owns each
//! real LHS, null warning/sink, destination and outer errors.
use std::{ffi::c_void, mem::MaybeUninit, ptr};

use hp2_abi::engine::generated::FFrame;
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_native_catalog::CoreNative,
    object_owner::CanonicalNativeBinding,
    ScriptCallArguments,
};

const INT_TO_BYTE: u8 = 0x3e;

// The fallback is the current Let RHS's original object/context protocol.
// Unlike an ordinary nested argument, that context is supplied by the Let host.
//
// SAFETY: the current frame, result, context and live operand satisfy the
// original Core dispatch contract. The current opcode is readable, and this
// operation is synchronous; no frame, capability or address escapes.
unsafe fn dispatch_current(
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    context: *mut c_void,
    destination: *mut c_void,
) -> RuntimeResult<()> {
    let cursor = unsafe { ptr::addr_of_mut!((*frame).code) };
    let code = unsafe { cursor.read() };
    let opcode = unsafe { code.read() };
    // SAFETY: consume precisely the current opcode, as Core's step does.
    unsafe { cursor.write(code.add(1)); }
    unsafe { args.dispatch_operand(opcode, context, frame, destination) }
}

// Select only the named health conversion at its original current root.
//
// SAFETY: the enclosing admitted current Let owns the original immutable
// script, frame, actual u8 lvalue/sink, records, and all continuations. Any
// live child admitted at this conversion fully writes aligned i32 storage on
// success. The current scope and staging addresses remain live across reentry
// and cannot escape or be used while the invocation is suspended.
unsafe fn white_channel(
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    context: *mut c_void,
    destination: *mut c_void,
    root: usize,
) -> RuntimeResult<()> {
    let (base, length) = args.body_script_snapshot();
    let cursor = unsafe { ptr::addr_of_mut!((*frame).code) };
    let code = unsafe { cursor.read() };
    let selected = if base.is_null()
        || length > isize::MAX as usize
        || root >= length
        || (base as usize).checked_add(length).is_none()
    {
        false
    } else {
        // SAFETY: root is within the retained admitted original allocation.
        let expected = unsafe { base.add(root) };
        code.cast_const() == expected && unsafe { expected.read() } == INT_TO_BYTE
    };
    if !selected {
        // SAFETY: no opcode was consumed; dispatch the actual current RHS once
        // using the context already supplied by Core's Let owner.
        return unsafe { dispatch_current(args, frame, context, destination) };
    }

    // SAFETY: this is the exact current admitted conversion opcode, not a
    // predicted child or successor. Core owns the live continuation after it.
    unsafe { cursor.write(code.add(1)); }
    let mut integer = MaybeUninit::<i32>::uninit();
    // SAFETY: the actual live child fully writes this aligned i32 on success.
    // The raw staging address is borrowed only during the synchronous step.
    unsafe { args.step_argument(frame, integer.as_mut_ptr().cast()) }.map_err(|error| {
        error.with_context(&format!(
            "Core native {:?} ({})",
            CanonicalNativeBinding::Core(CoreNative::intUObjectexecIntToByte),
            INT_TO_BYTE,
        ))
    })?;
    // SAFETY: the child succeeded and initialized integer. The original Let
    // supplies its actual one-byte writable lvalue or canonical sink.
    unsafe { destination.cast::<u8>().write_unaligned(integer.assume_init() as u8); }
    Ok(())
}

/// The health black-background red-channel assignment.
///
/// # Safety
/// The admitted current Let and its operands satisfy `white_channel`.
pub(super) unsafe fn white_red(
    _: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame,
    context: *mut c_void, destination: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { white_channel(args, frame, context, destination, 0x12e) }
}

/// The health black-background green-channel assignment.
///
/// # Safety
/// The admitted current Let and its operands satisfy `white_channel`.
pub(super) unsafe fn white_green(
    _: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame,
    context: *mut c_void, destination: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { white_channel(args, frame, context, destination, 0x145) }
}

/// The health black-background blue-channel assignment.
///
/// # Safety
/// The admitted current Let and its operands satisfy `white_channel`.
pub(super) unsafe fn white_blue(
    _: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame,
    context: *mut c_void, destination: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { white_channel(args, frame, context, destination, 0x15c) }
}

const _: hp2_core::ScriptAssignmentValue = white_red;
const _: hp2_core::ScriptAssignmentValue = white_green;
const _: hp2_core::ScriptAssignmentValue = white_blue;
