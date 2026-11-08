//! Process-wide script guards. The native VM is serialized, including reset;
//! callbacks may synchronously reenter it. Never borrow these counters across a
//! callback, restore them on failure, or replace them with thread-local state.

use std::ffi::c_void;

pub const HP2_VM_RUNAWAY_LIMIT: i32 = 10_000_000;
pub const HP2_VM_RECURSE_LIMIT: i32 = 250;

static mut RUNAWAY: i32 = 0;
static mut RECURSE: i32 = 0;

/// # Safety
/// Must be serialized with every guarded VM call. Zero is native DO_GUARD=0.
pub unsafe fn reset(enabled: u32) {
    if enabled != 0 {
        unsafe {
            RECURSE = 0;
            RUNAWAY = 0;
        }
    }
}

/// # Safety
/// Must be serialized with all guarded VM execution and resets.
#[inline]
pub(crate) unsafe fn step_runaway() -> bool {
    unsafe {
        RUNAWAY = RUNAWAY.wrapping_add(1);
        RUNAWAY > HP2_VM_RUNAWAY_LIMIT
    }
}

/// # Safety
/// Must be serialized; call only after a returning runaway diagnostic.
#[inline]
pub(crate) unsafe fn reset_runaway() {
    unsafe { RUNAWAY = 0; }
}

/// # Safety
/// The caller owns the serialized VM execution interval. Leaves catch native
/// exceptions and may reenter/reset the VM, but must not unwind across this ABI.
#[inline]
pub(crate) unsafe fn check_runaway(
    frame: *mut c_void,
    suppressed: unsafe extern "C" fn(*mut i32) -> i32,
    report: unsafe extern "C" fn(*mut c_void, i32) -> i32,
) -> Result<(), i32> {
    unsafe {
        if step_runaway() {
            let mut skip = 0;
            let status = suppressed(&mut skip);
            if status != 0 {
                return Err(status);
            }
            if skip == 0 {
                let status = report(frame, HP2_VM_RUNAWAY_LIMIT);
                if status != 0 {
                    return Err(status);
                }
            }
            // This assignment follows the returning diagnostic, even if that
            // diagnostic has recursively incremented or reset the live state.
            reset_runaway();
        }
        Ok(())
    }
}

#[inline]
pub(crate) unsafe fn enter_recursion() -> bool {
    unsafe {
        RECURSE = RECURSE.wrapping_add(1);
        RECURSE > HP2_VM_RECURSE_LIMIT
    }
}

#[inline]
pub(crate) unsafe fn leave_recursion() {
    unsafe {
        RECURSE = RECURSE.wrapping_sub(1);
    }
}
