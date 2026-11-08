//! Bytecode control flow and constant publication. Native storage stays opaque;
//! all cursors are live addresses and are reloaded after reentrant callbacks.
use crate::{
    ansi::{self, Tchar},
    array_storage::{Hp2ArrayServices, Hp2ArraySlots},
    string_storage,
    vm_bytecode::{advance, peek, read},
};
use hp2_abi::engine::generated::TArrayRaw;
use std::{ffi::c_void, ptr};
type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2VmFlowOp {
    VmFlowDebugInfo,
    VmFlowEndFunctionParms,
    VmFlowStop,
    VmFlowSwitch,
    VmFlowCase,
    VmFlowJump,
    VmFlowJumpIfNot,
    VmFlowAssert,
    VmFlowGotoLabel,
    VmFlowIntConst,
    VmFlowFloatConst,
    VmFlowNameConst,
    VmFlowByteConst,
    VmFlowIntZero,
    VmFlowIntOne,
    VmFlowTrue,
    VmFlowFalse,
    VmFlowNoObject,
    VmFlowIntConstByte,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmFlowServices {
    pub string_size: usize,
    pub slow_checks: u32,
    pub case_token: u32,
    pub none_name: i32,
    pub guard_enabled: u32,
    pub runaway_suppressed: unsafe extern "C" fn(*mut i32) -> i32,
    pub runaway_error: unsafe extern "C" fn(Object, i32) -> i32,
    pub step: unsafe extern "C" fn(Object, Object) -> i32,
    pub script_base: unsafe extern "C" fn(Object, *mut *mut u8) -> i32,
    pub script_at: unsafe extern "C" fn(Object, u32, *mut *mut u8) -> i32,
    pub bad_case: unsafe extern "C" fn() -> i32,
    pub branch: unsafe extern "C" fn(Object, Object, i32, i32, i32, i32) -> i32,
    pub assertion: unsafe extern "C" fn(Object, i32) -> i32,
    pub goto_label: unsafe extern "C" fn(Object, i32, *mut i32) -> i32,
    pub missing_label: unsafe extern "C" fn(Object, i32) -> i32,
    pub assign_string: unsafe extern "C" fn(Object, *const u8, u32) -> i32,
    pub strings_equal: unsafe extern "C" fn(Object, Object, *mut i32) -> i32,
    pub destroy_string: unsafe extern "C" fn(Object) -> i32,
    pub is_return: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub trace_return: unsafe extern "C" fn(Object, i32) -> i32,
    pub debugger: unsafe extern "C" fn() -> Object,
    pub debug_info: unsafe extern "C" fn(Object, Object, Object, Object, i32, i32) -> i32,
}
static mut FLOW_SERVICES: Option<Hp2VmFlowServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmFlowServices) {
    unsafe {
        (&raw mut FLOW_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmFlowServices> {
    unsafe { (&raw const FLOW_SERVICES).read().ok_or(1) }
}
fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}
unsafe fn scan(cursor: *mut *mut u8, unicode: bool) {
    unsafe {
        if unicode {
            while peek::<u16>(cursor) != 0 {
                advance(cursor, 2);
            }
            advance(cursor, 2);
        } else {
            while peek::<u8>(cursor) != 0 {
                advance(cursor, 1);
            }
            advance(cursor, 1);
        }
    }
}
unsafe fn offset(frame: Object, cursor: *mut *mut u8, s: &Hp2VmFlowServices) -> Result<i32> {
    unsafe {
        let mut base = ptr::null_mut();
        status((s.script_base)(frame, &mut base))?;
        Ok(if base.is_null() {
            -1
        } else {
            cursor.read().offset_from(base) as i32
        })
    }
}
unsafe fn jump(frame: Object, cursor: *mut *mut u8, target: u16, s: &Hp2VmFlowServices) -> Result {
    unsafe {
        let mut address = ptr::null_mut();
        status((s.script_at)(frame, target as u32, &mut address))?;
        cursor.write(address);
        Ok(())
    }
}
/// Execute a services-free opcode, leaving all storage untouched if unsupported.
///
/// # Safety
/// Cursor addresses a writable live frame code slot and valid opcode operands.
/// Result has the opcode's native width and may alias bytecode or the code slot.
/// EndFunctionParms and Stop ignore result. Operand reads publish the advanced
/// cursor before the result write; no frame reference may exclude that aliasing.
pub unsafe fn execute_plain(op: Hp2VmFlowOp, cursor: *mut *mut u8, result: Object) -> bool {
    use Hp2VmFlowOp::*;
    unsafe {
        match op {
            VmFlowEndFunctionParms => advance(cursor, -1),
            VmFlowStop => cursor.write(ptr::null_mut()),
            VmFlowIntConst | VmFlowFloatConst | VmFlowNameConst => {
                let bits = read::<u32>(cursor);
                result.cast::<u32>().write_unaligned(bits);
            }
            VmFlowByteConst => {
                let value = read::<u8>(cursor);
                result.cast::<u8>().write(value);
            }
            VmFlowIntConstByte => {
                let value = read::<u8>(cursor);
                result.cast::<i32>().write_unaligned(value as i32);
            }
            VmFlowIntZero | VmFlowFalse => result.cast::<u32>().write_unaligned(0),
            VmFlowIntOne | VmFlowTrue => result.cast::<i32>().write_unaligned(1),
            VmFlowNoObject => result.cast::<Object>().write_unaligned(ptr::null_mut()),
            _ => return false,
        }
        true
    }
}

/// Read a raw four-byte object index operand through the canonical live object
/// table, without the Step protocol. Mirrors the legacy `FFrame::ReadObject()`.
///
/// # Safety
/// Access is engine-thread serialized. Storage is live and cursor addresses
/// four live operand bytes in writable bytecode.
pub unsafe fn read_object_operand(
    cursor: *mut *mut u8,
    storage: *const crate::object_storage::ObjectStorage,
) -> Object {
    unsafe {
        let index = peek::<i32>(cursor);
        let value = (*storage).at(index);
        advance(cursor, 4);
        value
    }
}

/// Resolve a four-byte object index through the canonical live object table.
///
/// # Safety
/// Access is engine-thread serialized. Storage is live, cursor addresses a
/// writable code slot and four operand bytes, and result accepts a full pointer.
/// Result may alias the operand or cursor; publish the cursor before the value.
pub unsafe fn execute_object_constant(
    cursor: *mut *mut u8,
    result: Object,
    storage: *const crate::object_storage::ObjectStorage,
) {
    unsafe {
        let value = read_object_operand(cursor, storage);
        result.cast::<Object>().write_unaligned(value);
    }
}

/// Assign a literal through the canonical FString policy and allocation leaves,
/// then scan the LIVE cursor, not the source captured before allocation.
///
/// # Safety
/// Engine-thread serialized (the conversion ring is shared with appFromAnsi and
/// appFromUnicode). Cursor is writable and its live value always addresses a
/// terminated literal of the selected width, including after callbacks.
/// Result is initialized FString/TArrayRaw storage, with independently writable
/// pointer/INT slots; it may alias bytecode or the cursor if the live ranges stay
/// valid. Existing destination allocations belong to arrays. Callbacks may
/// reenter and overwrite ring slots, cursor or destination; no references to
/// those objects survive them. Failure preserves the canonical partial array
/// publication and does not perform the final cursor scan.
pub unsafe fn execute_string_constant(
    unicode: bool,
    cursor: *mut *mut u8,
    result: *mut TArrayRaw,
    arrays: Hp2ArrayServices,
) -> Result {
    const TAG: [Tchar; 8] = [70, 83, 116, 114, 105, 110, 103, 0];
    unsafe {
        let source = cursor.read();
        let text = if unicode {
            ansi::from_unicode(source.cast())
        } else {
            ansi::from_ansi(source)
        };
        let slots = Hp2ArraySlots {
            data: ptr::addr_of_mut!((*result).data),
            num: ptr::addr_of_mut!((*result).num),
            max: ptr::addr_of_mut!((*result).max),
        };
        string_storage::assign_with_allocator(
            slots,
            None,
            text,
            false,
            arrays,
            TAG.as_ptr().cast(),
        )?;
        scan(cursor, unicode);
        Ok(())
    }
}

/// # Safety
/// Cursor addresses a writable live frame code slot with valid operands. Result
/// has the opcode's native width and may alias bytecode. Scratch is native-owned:
/// DebugInfo requires a constructed FString in first; Switch two 1024-byte
/// buffers, Case one, each aligned for FString. Other opcodes ignore scratch.
/// Services are immutable; callbacks may mutate frame/cursor/storage and return
/// a first-error status. Guarded jumps share serialized process-wide counters;
/// the guard runs before touching operands or calling any opcode services.
pub unsafe fn execute(
    op: Hp2VmFlowOp,
    object: Object,
    frame: Object,
    cursor: *mut *mut u8,
    result: Object,
    first: Object,
    second: Object,
    s: &Hp2VmFlowServices,
) -> Result {
    use Hp2VmFlowOp::*;
    unsafe {
        if execute_plain(op, cursor, result) {
            return Ok(());
        }
        match op {
            VmFlowDebugInfo => {
                if read::<i32>(cursor) != 100 {
                    advance(cursor, -5);
                    return Ok(());
                }
                let line = read::<i32>(cursor);
                let input = read::<i32>(cursor);
                status((s.assign_string)(first, cursor.read(), 0))?;
                scan(cursor, false);
                let mut is_return = 0;
                status((s.is_return)(first, &mut is_return))?;
                if is_return != 0 {
                    status((s.trace_return)(frame, line))?;
                }
                let debugger = (s.debugger)();
                if !debugger.is_null() {
                    status((s.debug_info)(debugger, object, frame, first, line, input))?;
                }
            }
            VmFlowJump => {
                if s.guard_enabled != 0 {
                    crate::vm_guard::check_runaway(frame, s.runaway_suppressed, s.runaway_error)?;
                }
                let target = read::<u16>(cursor);
                jump(frame, cursor, target, s)?;
            }
            VmFlowJumpIfNot => {
                if s.guard_enabled != 0 {
                    crate::vm_guard::check_runaway(frame, s.runaway_suppressed, s.runaway_error)?;
                }
                // A missing node yields INDEX_NONE, not INDEX_NONE - 1.
                let mut base = ptr::null_mut();
                status((s.script_base)(frame, &mut base))?;
                let opcode = if base.is_null() {
                    -1
                } else {
                    cursor.read().offset_from(base) as i32 - 1
                };
                let target = read::<u16>(cursor);
                let mut value = 0i32;
                status((s.step)(frame, (&raw mut value).cast()))?;
                let next = offset(frame, cursor, s)?;
                status((s.branch)(
                    frame,
                    object,
                    opcode,
                    target as i32,
                    next,
                    value,
                ))?;
                if value == 0 {
                    jump(frame, cursor, target, s)?;
                }
            }
            VmFlowAssert => {
                let line = read::<u16>(cursor);
                let mut value = 0u32;
                status((s.step)(frame, (&raw mut value).cast()))?;
                if value == 0 {
                    status((s.assertion)(frame, line as i32))?;
                }
            }
            VmFlowGotoLabel => {
                let mut name = s.none_name;
                status((s.step)(frame, (&raw mut name).cast()))?;
                let mut found = 0;
                status((s.goto_label)(object, name, &mut found))?;
                if found == 0 {
                    status((s.missing_label)(frame, name))?;
                }
            }
            VmFlowCase => {
                if read::<u16>(cursor) != u16::MAX {
                    ptr::write_bytes(first.cast::<u8>(), 0, s.string_size);
                    status((s.step)(frame, first))?;
                    // Legacy Case deliberately does not destroy string scratch.
                }
            }
            VmFlowSwitch => {
                let size = read::<u8>(cursor) as usize;
                ptr::write_bytes(second.cast::<u8>(), 0, s.string_size);
                ptr::write_bytes(first.cast::<u8>(), 0, s.string_size);
                status((s.step)(frame, first))?;
                loop {
                    if s.slow_checks != 0 && peek::<u8>(cursor) as u32 != s.case_token {
                        status((s.bad_case)())?;
                    }
                    advance(cursor, 1);
                    let next = read::<u16>(cursor);
                    if next == u16::MAX {
                        break;
                    }
                    status((s.step)(frame, second))?;
                    let equal = if size == 0 {
                        let mut equal = 0;
                        status((s.strings_equal)(first, second, &mut equal))?;
                        equal != 0
                    } else {
                        (0..size).all(|i| {
                            first.cast::<u8>().add(i).read() == second.cast::<u8>().add(i).read()
                        })
                    };
                    if equal {
                        break;
                    }
                    jump(frame, cursor, next, s)?;
                }
                if size == 0 {
                    status((s.destroy_string)(first))?;
                    status((s.destroy_string)(second))?;
                }
            }
            VmFlowEndFunctionParms
            | VmFlowStop
            | VmFlowIntConst
            | VmFlowFloatConst
            | VmFlowNameConst
            | VmFlowByteConst
            | VmFlowIntZero
            | VmFlowIntOne
            | VmFlowTrue
            | VmFlowFalse
            | VmFlowNoObject
            | VmFlowIntConstByte => unreachable!("services-free opcode handled above"),
        }
        Ok(())
    }
}
