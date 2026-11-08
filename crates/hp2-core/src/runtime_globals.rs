//! Runtime flags shared by canonical owners and the native ABI during cutover.
//!
//! These are the original process-wide symbols, not per-owner copies.
use std::{marker::PhantomData, rc::Rc};

/// The original process-wide slow-task flag, shared by native and Core owners.
pub static mut GIsSlowTask: i32 = 0;

static mut SLOW_TASK_COUNT: i32 = 0;

/// A non-transferable scope for the process-wide nested feedback slow-task state.
#[must_use = "dropping the scope balances the shared slow-task state"]
#[derive(Debug)]
pub struct SlowTaskScope(PhantomData<Rc<()>>);

/// Begins one nested feedback slow task.
///
/// # Safety
/// Call and drop the returned scope on the engine application thread.
pub unsafe fn slow_task_scope() -> SlowTaskScope {
    unsafe {
        SLOW_TASK_COUNT = SLOW_TASK_COUNT.wrapping_add(1);
        GIsSlowTask = i32::from(SLOW_TASK_COUNT > 0);
    }
    SlowTaskScope(PhantomData)
}

/// Returns the current nested slow-task depth.
///
/// # Safety
/// Read on the engine application thread, which serializes all slow-task access.
pub unsafe fn slow_task_count() -> i32 {
    unsafe { SLOW_TASK_COUNT }
}

/// Reads the process-wide feedback slow-task flag.
///
/// # Safety
/// Read on the engine application thread, which serializes all slow-task access.
pub unsafe fn is_slow_task() -> bool {
    unsafe { GIsSlowTask != 0 }
}

impl Drop for SlowTaskScope {
    fn drop(&mut self) {
        // SAFETY: !Send/!Sync keeps the scope on the application thread that created it.
        unsafe {
            SLOW_TASK_COUNT = SLOW_TASK_COUNT.wrapping_sub(1);
            GIsSlowTask = i32::from(SLOW_TASK_COUNT > 0);
        }
    }
}
pub static mut GUglyHackFlags: u32 = 0;

/// Reads the source renderer's temporary behavior flags.
///
/// # Safety
/// The application thread must synchronize this access with all native and Rust writers.
pub unsafe fn ugly_hack_flags() -> u32 {
    // SAFETY: synchronization is the caller's responsibility, as for the original global.
    unsafe { GUglyHackFlags }
}

/// Replaces the source renderer's temporary behavior flags.
///
/// # Safety
/// The application thread must synchronize this access with all native and Rust readers and writers.
pub unsafe fn set_ugly_hack_flags(flags: u32) {
    // SAFETY: synchronization is the caller's responsibility, as for the original global.
    unsafe { GUglyHackFlags = flags };
}
