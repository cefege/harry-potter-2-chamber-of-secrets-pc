//! Port of the reachable (`__STATIC_LINK`) halves of `appGetDllHandle`,
//! `appFreeDllHandle`, and `appGetDllExport` in `Core/Src/UnUnix.cpp`. The
//! `check(DllHandle)`/`check(ProcName)` preconditions stay in the C++ call
//! sites for correct `__FILE__`/`__LINE__` capture.

use std::ffi::{c_char, c_void};
use std::ptr::null;

use crate::ansi::Tchar;
use crate::unix_text::{self, Hp2UnixTextServices};

/// `appGetDllHandle` under the permanent `__STATIC_LINK` build: `dlopen(NULL,
/// RTLD_NOW)`, the handle to the main executable's own symbol table.
///
/// # Safety
/// None beyond the `dlopen` contract.
pub unsafe fn get_dll_handle() -> *mut c_void {
    // SAFETY: `dlopen(NULL, ...)` has no pointer precondition.
    unsafe { libc::dlopen(null(), libc::RTLD_NOW) }
}

/// `appFreeDllHandle`: `dlclose(DllHandle)`, return value ignored (matches
/// the original).
///
/// # Safety
/// `handle` must be a valid handle previously returned by `dlopen`.
pub unsafe fn free_dll_handle(handle: *mut c_void) {
    // SAFETY: caller contract.
    unsafe {
        libc::dlclose(handle);
    }
}

/// `appGetDllExport`: clears `dlerror()`, encodes `proc_name` to UTF-8, calls
/// `dlsym`, and reads `dlerror()` once more. Returns `(symbol, error)`; `error`
/// is null unless `dlerror` reported one, and when non-null is the dynamic
/// linker's own static/TLS buffer (never freed by the caller, matching the
/// original).
///
/// # Safety
/// `handle` must be a valid `dlopen` handle. `proc_name` must be a readable
/// NUL-terminated TCHAR string. `services` must satisfy its documented leaf
/// contract.
pub unsafe fn get_dll_export(
    services: &Hp2UnixTextServices,
    handle: *mut c_void,
    proc_name: *const Tchar,
) -> Result<(*mut c_void, *const c_char), i32> {
    // SAFETY: `proc_name` is readable/NUL-terminated by contract; `services`
    // is valid by contract.
    let proc_name_utf8 = unsafe { unix_text::to_utf8(services, proc_name) }?;
    // SAFETY: clearing any prior error, matching the original's `dlerror();`.
    unsafe { libc::dlerror() };
    // SAFETY: `handle` is valid by contract; `proc_name_utf8` is NUL-terminated.
    let symbol = unsafe { libc::dlsym(handle, proc_name_utf8.cast()) };
    // SAFETY: reading the dynamic linker's thread-local error state.
    let error = unsafe { libc::dlerror() };
    // SAFETY: `proc_name_utf8` came from the native system allocator; freeing
    // it does not touch `dlerror`'s unrelated buffer.
    unsafe { libc::free(proc_name_utf8.cast()) };
    Ok((symbol, error))
}
