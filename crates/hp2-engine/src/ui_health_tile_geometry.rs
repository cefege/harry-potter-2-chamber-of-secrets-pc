//! Health DrawTile geometry on the current Core native-argument host.
//!
//! This module is not runtime-qualified. The caller must admit the original
//! body and actual native ABI before effects. Canonical execution must already
//! have valid typed operands, live records and writable storage at each actual
//! use. The tile handoff requires fully-writing typed continuations on success;
//! this requirement is inherited from the health-draw domain, not a new relative
//! restriction. This module does not establish that production admission.

use std::{mem::MaybeUninit, ptr};

use hp2_abi::engine::generated::FFrame;
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_native_catalog::CoreNative,
    object_owner::CanonicalNativeBinding,
    ScriptCallArguments,
};

// Logical offsets in the admitted StatusItemHealth.DrawItem body, not package offsets.
const SCALED_ICON_WIDTH: usize = 0x2d9;
const ICON_WIDTH_CONVERSION: usize = 0x2da;
const SCALED_SEGMENT_HEIGHT: usize = 0x2ef;
const ICON_SOURCE_WIDTH: usize = 0x305;
const MULTIPLY_FLOAT: u8 = 0xab;
const INT_TO_FLOAT: u8 = 0x40;

fn native_context(error: RuntimeError, native: CoreNative, opcode: u8) -> RuntimeError {
    error.with_context(&format!("Core native {:?} ({})", CanonicalNativeBinding::Core(native), opcode))
}

// Numeric bounds do not establish ownership or liveness. The caller admits the
// original allocation and keeps its bytes immutable throughout this scope.
fn snapshot_base(args: &ScriptCallArguments<'_>) -> Option<*const u8> {
    let (base, length) = args.body_script_snapshot();
    if base.is_null()
        || length > isize::MAX as usize
        || ICON_SOURCE_WIDTH >= length
        || (base as usize).checked_add(length).is_none()
    {
        return None;
    }
    Some(base)
}

/// Consume an admitted int-to-float opcode and step its actual integer child.
///
/// # Safety
/// `frame` is the current frame and `code` is its admitted conversion opcode.
/// The integer child fully initializes its aligned four-byte destination on
/// success. The destination and all actual operand records remain valid across
/// synchronous callbacks. No capability or pointer escapes this invocation.
unsafe fn icon_width_conversion(
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    code: *mut u8,
    destination: *mut f32,
) -> RuntimeResult<()> {
    // SAFETY: the caller checked this live opcode in the admitted allocation.
    unsafe { ptr::addr_of_mut!((*frame).code).write(code.add(1)); }
    let mut integer = MaybeUninit::<i32>::uninit();
    // SAFETY: the canonical child owns the current cursor and fully initializes
    // the aligned slot on success. It may reenter without a live Rust reference
    // to the frame or the slot.
    unsafe { args.step_argument(frame, integer.as_mut_ptr().cast()) }
        .map_err(|error| native_context(error, CoreNative::intUObjectexecIntToFloat, INT_TO_FLOAT))?;
    // SAFETY: a successful child fully initialized the integer. Core's
    // conversion writes the actual float destination with unaligned semantics.
    unsafe { destination.write_unaligned(integer.assume_init() as f32); }
    Ok(())
}

/// Evaluate the live first operand of the scaled icon width.
///
/// # Safety
/// The current frame and initialized float slot meet the native argument
/// contract. `base` is the admitted live immutable script allocation, and the
/// current continuation is valid and fully writing on success.
unsafe fn icon_width_operand(
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    base: *const u8,
    destination: *mut f32,
) -> RuntimeResult<()> {
    // SAFETY: the current frame is borrowed only for these raw field reads.
    let _object = unsafe { ptr::addr_of!((*frame).object).read() };
    let code = unsafe { ptr::addr_of!((*frame).code).read() };
    // SAFETY: snapshot_base checked this offset and the caller retains the
    // original allocation. Only dereference a matching, in-bounds live opcode.
    let conversion = unsafe { base.add(ICON_WIDTH_CONVERSION) };
    if code.cast_const() == conversion && unsafe { conversion.read() } == INT_TO_FLOAT {
        // SAFETY: the matching opcode and the caller's typed child contract
        // admit this conversion on the current argument host.
        unsafe { icon_width_conversion(args, frame, code, destination) }
    } else {
        // SAFETY: no opcode was consumed. The actual live float expression is
        // stepped once using Core's object-before-opcode sampling protocol.
        unsafe { args.step_argument(frame, destination.cast()) }
    }
}

/// Multiply the two actual float children, finishing before reading either.
///
/// # Safety
/// `frame` is current, `code` is an admitted multiply opcode, and `destination`
/// is the real aligned float slot. Each actual child and finish satisfies Core's
/// typed continuation contract and remains valid during synchronous reentry.
unsafe fn scaled_product(
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    base: *const u8,
    code: *mut u8,
    destination: *mut f32,
    icon_width: bool,
) -> RuntimeResult<()> {
    // SAFETY: the caller selected this multiply in the live script snapshot.
    unsafe { ptr::addr_of_mut!((*frame).code).write(code.add(1)); }
    let mut values = [0.0_f32; 3];
    let slots = ptr::addr_of_mut!(values).cast::<f32>();
    let result = (|| {
        if icon_width {
            // SAFETY: slot 0 is aligned, initialized and live through the child.
            unsafe { icon_width_operand(args, frame, base, slots)?; }
        } else {
            // SAFETY: slot 0 is the original typed child destination.
            unsafe { args.step_argument(frame, slots.cast())?; }
        }
        // SAFETY: slot 1 is independently aligned, initialized and live.
        unsafe { args.step_argument(frame, slots.add(1).cast())?; }
        // SAFETY: the actual live terminator and lookahead meet Core's finish
        // contract. No values are read before this callback returns.
        unsafe { args.finish_arguments(frame)?; }
        // SAFETY: all three slots were initialized, remain live, and may have
        // been modified by valid synchronous child or finish callbacks.
        let (left, right, _third) = unsafe { (slots.read(), slots.add(1).read(), slots.add(2).read()) };
        // SAFETY: the actual native float destination is aligned and writable.
        unsafe { destination.write(left * right); }
        Ok(())
    })();
    result.map_err(|error| native_context(error, CoreNative::intUObjectexecMultiply_FloatFloat, MULTIPLY_FLOAT))
}

/// Calculate `(icon USize as f32) * scale` from the live child expressions.
///
/// # Safety
/// The frame, script, opcode and destination satisfy `scaled_product`.
unsafe fn scaled_icon_width(args: &ScriptCallArguments<'_>, frame: *mut FFrame, base: *const u8, code: *mut u8, destination: *mut f32) -> RuntimeResult<()> {
    // SAFETY: the caller admitted native 171 and its actual typed continuation.
    unsafe { scaled_product(args, frame, base, code, destination, true) }
}

/// Calculate `segment height * scale` from the live float children.
///
/// # Safety
/// The frame, script, opcode and destination satisfy `scaled_product`.
unsafe fn scaled_segment_height(args: &ScriptCallArguments<'_>, frame: *mut FFrame, base: *const u8, code: *mut u8, destination: *mut f32) -> RuntimeResult<()> {
    // SAFETY: the caller admitted native 171 and its actual typed continuation.
    unsafe { scaled_product(args, frame, base, code, destination, false) }
}

/// Calculate `icon USize as f32` through the actual integer child.
///
/// # Safety
/// The current conversion and destination satisfy `icon_width_conversion`.
unsafe fn icon_source_width(args: &ScriptCallArguments<'_>, frame: *mut FFrame, code: *mut u8, destination: *mut f32) -> RuntimeResult<()> {
    // SAFETY: the caller admitted conversion 64 and its actual integer child.
    unsafe { icon_width_conversion(args, frame, code, destination) }
}

/// Select the live DrawTile float expression, not its argument ordinal.
///
/// # Safety
/// Core calls this only in the current synchronous `NativeTileFloat` scope.
/// The original script allocation is live and immutable for its captured
/// length; all real frame, object, code and destination storage satisfies the
/// canonical typed VM contract through callbacks. Every successful continuation
/// admitted by the tile handoff fully writes its typed destination. The caller
/// must establish that stronger condition before body effects; it is not proved
/// by this function or by matching a script offset. No frame, borrowed capability,
/// staging address or object pointer may escape, and no suspended frame may be
/// accessed. Unsupported valid roots are stepped canonically without consumption.
pub(super) unsafe fn evaluate(
    _: CoreAccess<'_>,
    args: &ScriptCallArguments<'_>,
    frame: *mut FFrame,
    destination: *mut f32,
) -> RuntimeResult<()> {
    let Some(base) = snapshot_base(args) else {
        // SAFETY: the caller supplies a valid current float expression.
        return unsafe { args.step_argument(frame, destination.cast()) };
    };
    // Sample the live object before the opcode, as Core's argument owner does.
    // SAFETY: the frame is current and remains live for this invocation.
    let _object = unsafe { ptr::addr_of!((*frame).object).read() };
    let code = unsafe { ptr::addr_of!((*frame).code).read() };
    // SAFETY: all selected offsets lie in the admitted original allocation.
    let width = unsafe { base.add(SCALED_ICON_WIDTH) };
    let height = unsafe { base.add(SCALED_SEGMENT_HEIGHT) };
    let source = unsafe { base.add(ICON_SOURCE_WIDTH) };
    if code.cast_const() == width && unsafe { width.read() } == MULTIPLY_FLOAT {
        // SAFETY: selected opcode and operand contracts are those of native 171.
        unsafe { scaled_icon_width(args, frame, base, code, destination) }
    } else if code.cast_const() == height && unsafe { height.read() } == MULTIPLY_FLOAT {
        // SAFETY: this multiply has two actual float children and no conversion.
        unsafe { scaled_segment_height(args, frame, base, code, destination) }
    } else if code.cast_const() == source && unsafe { source.read() } == INT_TO_FLOAT {
        // SAFETY: selected opcode and integer child satisfy conversion 64.
        unsafe { icon_source_width(args, frame, code, destination) }
    } else {
        // SAFETY: no opcode was consumed. Core steps the actual live expression
        // once, including any canonical error and context.
        unsafe { args.step_argument(frame, destination.cast()) }
    }
}

const _: hp2_core::NativeTileFloat = evaluate;
