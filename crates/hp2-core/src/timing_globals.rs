//! Shared timing counters used by canonical owners and the native ABI during cutover.
//!
//! These are the original exported symbols, not per-owner copies. As in the
//! native engine, callers synchronize access through the application thread.

use std::marker::PhantomData;
use std::rc::Rc;

pub static mut GTempTime: i64 = 0;

pub static mut GTicks: i64 = 1;
/// The process-wide nested script-entry tag shared with native Engine callbacks.
pub static mut GScriptEntryTag: i32 = 0;

/// The original process-wide accumulated script-cycle counter.
pub static mut GScriptCycles: i32 = 0;

/// Returns the accumulated canonical script-cycle count.
///
/// # Safety
/// Read only on the engine application thread, which serializes all script-counter access.
pub unsafe fn script_cycles() -> i32 {
    unsafe { GScriptCycles }
}

/// Clears accumulated script-cycle statistics without altering live nesting.
///
/// # Safety
/// Call only on the engine application thread, which serializes all script-counter access.
pub unsafe fn reset_script_cycles() {
    unsafe { GScriptCycles = 0; }
}

/// Clears the shared script-entry nesting tag to match `ULevel::InitStats`.
///
/// # Safety
/// Call only on the engine application thread, which serializes script-counter access.
pub unsafe fn reset_script_entry_tag() {
    unsafe { GScriptEntryTag = 0; }
}

/// A nested script-timing scope that balances the process-wide application-thread tag.
#[must_use = "dropping the scope balances the shared script timer"]
#[derive(Debug)]
pub struct ScriptScope(PhantomData<Rc<()>>);

/// Begins a script-timing scope, with clocking active only at outermost depth.
///
/// # Safety
/// Call and drop the returned scope on the engine application thread.
pub unsafe fn script_scope() -> ScriptScope {
    enter_script_scope();
    ScriptScope(PhantomData)
}

impl Drop for ScriptScope {
    fn drop(&mut self) {
        leave_script_scope();
    }
}

/// Enters a script scope, starting the clock only at the outermost level.
pub(crate) fn enter_script_scope() {
    unsafe {
        GScriptEntryTag = GScriptEntryTag.wrapping_add(1);
        if GScriptEntryTag == 1 && hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            GScriptCycles = GScriptCycles.wrapping_sub(crate::unix_clock::cycles() as i32);
        }
    }
}

/// Leaves a script scope, stopping the clock only after the outermost level exits.
pub(crate) fn leave_script_scope() {
    unsafe {
        GScriptEntryTag = GScriptEntryTag.wrapping_sub(1);
        if GScriptEntryTag == 0 && hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            // The native `unclock` macro subtracts its 34-cycle call overhead.
            GScriptCycles =
                GScriptCycles.wrapping_add(crate::unix_clock::cycles().wrapping_sub(34) as i32);
        }
    }
}

