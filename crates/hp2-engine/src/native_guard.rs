//! Unwind history for Rust bodies whose original C++ ran under
//! `guard(...)` + `unguard`/`unguardexec` (`Core/Inc/UnFile.h`).
//!
//! On this build `unguard` is `catch(char*){throw;} catch(...){appUnwindf(TEXT("%s"), Name); throw;}`.
//! `TCHAR` is 32-bit, so engine throws (`appErrorf`, `appThrowf`,
//! `appFailAssert`) always take the `catch(...)` arm. The Rust equivalent is
//! therefore: on the error path, call [`unwind_guard`] once with the guard's
//! display name, then return the same `Err`. Nested guards unwind innermost
//! first, matching the C++ catch order. This replaces routing Rust-to-Rust
//! calls back through C++ shell methods just to pick up their guard frames.
//! Signals request deferred exit or terminate the process; they do not unwind
//! through native or Rust guard frames.
use std::ffi::{CStr, c_char, c_void};

/// Published once from `Core/Src/UnCorSc.cpp`. `unwind` reproduces
/// `appUnwindf(TEXT("%s"), Name)`, plus (when `with_frame` is nonzero) the
/// `unguardexec` frame line `appUnwindf(TEXT("(%s @ %s : %04X)"),
/// Stack.Object->GetFullName(), Stack.Node->GetFullName(),
/// Stack.Code - &Stack.Node->Script(0))`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2EngineNativeGuardServices {
    pub unwind: unsafe extern "C" fn(*mut c_void, *const c_char, i32) -> i32,
}

static mut SERVICES: Option<Hp2EngineNativeGuardServices> = None;

/// Store the unwind table.
///
/// # Safety
/// Publication is serialized with engine execution and happens once, before
/// any guarded Rust body runs.
pub unsafe fn publish(services: Hp2EngineNativeGuardServices) {
    unsafe { (&raw mut SERVICES).write(Some(services)) };
}

/// Log unwind history for one failing guarded body, exactly once, on its
/// error path, before returning that body's own (already captured) status.
/// The leaf's own status is ignored, as Core's guarded VM ops do.
///
/// # Safety
/// When `with_frame` is true, `frame` is the live native `FFrame` of the
/// failing call. Called at most once per guarded invocation.
pub unsafe fn unwind_guard(frame: *mut c_void, name: &'static CStr, with_frame: bool) {
    unsafe {
        if let Some(services) = (&raw const SERVICES).read() {
            let _ = (services.unwind)(frame, name.as_ptr(), with_frame as i32);
        }
    }
}

/// Run `body` under the guard `name`: on `Err`, log the unwind entry and pass
/// the status through unchanged.
///
/// # Safety
/// As [`unwind_guard`] with `with_frame = false`.
pub unsafe fn guarded<T, E>(name: &'static CStr, body: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let result = body();
    if result.is_err() {
        unsafe { unwind_guard(std::ptr::null_mut(), name, false) };
    }
    result
}
