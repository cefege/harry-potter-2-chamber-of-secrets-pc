//! Port of the process-identity helpers in `Core/Src/UnUnix.cpp`:
//! `appUnixGetCurrentDirectory`/`appUnixGetHostName`/`appUnixGetLoginName`,
//! `appSetBaseDir`/`appSetUserDir`/`appBaseDir`/`appUserDir`,
//! `appComputerName`/`appUserName`/`appPackage`, and `appGetLocalIP`.

use std::ffi::c_char;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use crate::ansi::Tchar;
use crate::unix_alloc;
use crate::unix_path;
use crate::unix_text::{self, Hp2UnixTextServices};

/// `getlogin_r` and `gethostbyname` are not in the `libc` crate (both are
/// legacy POSIX/BSD sockets APIs this platform still uses: `getlogin_r`
/// instead of the non-reentrant `getlogin`, `gethostbyname` instead of
/// `getaddrinfo`, matching the original's exact calls). `libc::hostent`
/// already has the right field layout, so only the two functions need a
/// local declaration.
mod net {
    use std::ffi::{c_char, c_int};

    unsafe extern "C" {
        pub fn gethostbyname(name: *const c_char) -> *mut libc::hostent;
        pub fn getlogin_r(buf: *mut c_char, bufsize: libc::size_t) -> c_int;
    }
}

/// `appUnixGetCurrentDirectory`: grows a `system_malloc`'d buffer across
/// `ERANGE` until `getcwd` fits, or returns null.
///
/// # Safety
/// None beyond the `getcwd` contract.
unsafe fn get_current_directory() -> *mut c_char {
    let mut capacity: usize = 256;
    // SAFETY: the system allocator contract.
    let mut buffer = unsafe { unix_alloc::system_malloc(capacity) }.cast::<c_char>();
    loop {
        // SAFETY: `buffer` is valid for `capacity` bytes.
        if !unsafe { libc::getcwd(buffer, capacity) }.is_null() {
            return buffer;
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno != libc::ERANGE || capacity > usize::MAX / 2 {
            // SAFETY: `buffer` came from the native system allocator.
            unsafe { libc::free(buffer.cast()) };
            return null_mut();
        }
        capacity *= 2;
        // SAFETY: the system allocator contract; `buffer` is a live allocation.
        buffer = unsafe { unix_alloc::system_realloc(buffer.cast(), capacity) }.cast();
    }
}

/// `appUnixGetHostName`: a single `system_malloc`'d buffer sized from
/// `sysconf(_SC_HOST_NAME_MAX)` (or 256 if unavailable), NUL-forced at the
/// end regardless of what `gethostname` wrote.
///
/// # Safety
/// None beyond the `gethostname`/`sysconf` contract.
unsafe fn get_host_name() -> *mut c_char {
    // SAFETY: `sysconf` has no pointer precondition.
    let maximum: libc::c_long = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) };
    if maximum > 0 && maximum as u64 >= usize::MAX as u64 {
        return null_mut();
    }
    let capacity: usize = if maximum > 0 {
        maximum as usize + 1
    } else {
        256
    };
    // SAFETY: the system allocator contract.
    let buffer = unsafe { unix_alloc::system_malloc(capacity) }.cast::<c_char>();
    // SAFETY: `buffer` is valid for `capacity` bytes.
    if unsafe { libc::gethostname(buffer, capacity) } != 0 {
        // SAFETY: `buffer` came from the native system allocator.
        unsafe { libc::free(buffer.cast()) };
        return null_mut();
    }
    // SAFETY: `capacity - 1` is a valid index into `buffer`.
    unsafe { *buffer.add(capacity - 1) = 0 };
    buffer
}

/// `appUnixGetLoginName`: grows a `system_malloc`'d buffer across `ERANGE`
/// (the direct `getlogin_r` return code, not `errno`) until it fits, sized
/// initially from `sysconf(_SC_LOGIN_NAME_MAX)` (or 256 if unavailable).
///
/// # Safety
/// None beyond the `getlogin_r`/`sysconf` contract.
unsafe fn get_login_name() -> *mut c_char {
    // SAFETY: `sysconf` has no pointer precondition.
    let maximum: libc::c_long = unsafe { libc::sysconf(libc::_SC_LOGIN_NAME_MAX) };
    if maximum > 0 && maximum as u64 >= usize::MAX as u64 {
        return null_mut();
    }
    let mut capacity: usize = if maximum > 0 {
        maximum as usize + 1
    } else {
        256
    };
    // SAFETY: the system allocator contract.
    let mut buffer = unsafe { unix_alloc::system_malloc(capacity) }.cast::<c_char>();
    loop {
        // SAFETY: `buffer` is valid for `capacity` bytes.
        let error = unsafe { net::getlogin_r(buffer, capacity) };
        if error == 0 {
            return buffer;
        }
        if error != libc::ERANGE || capacity > usize::MAX / 2 {
            // SAFETY: `buffer` came from the native system allocator.
            unsafe { libc::free(buffer.cast()) };
            return null_mut();
        }
        capacity *= 2;
        // SAFETY: the system allocator contract; `buffer` is a live allocation.
        buffer = unsafe { unix_alloc::system_realloc(buffer.cast(), capacity) }.cast();
    }
}

static EMPTY_DIR: [Tchar; 1] = [0];
static BASE_DIR: AtomicPtr<Tchar> = AtomicPtr::new(null_mut());
static USER_DIR: AtomicPtr<Tchar> = AtomicPtr::new(null_mut());

fn dir_or_empty(cell: &AtomicPtr<Tchar>) -> *const Tchar {
    let current = cell.load(Ordering::SeqCst);
    if current.is_null() {
        EMPTY_DIR.as_ptr()
    } else {
        current as *const Tchar
    }
}

/// `appSetBaseDir`.
///
/// # Safety
/// `dir` must be null or a readable NUL-terminated TCHAR string. `services`
/// must satisfy its documented leaf contract.
pub unsafe fn set_base_dir(services: &Hp2UnixTextServices, dir: *const Tchar) -> Result<(), i32> {
    // SAFETY: `dir`/`services` are valid by contract.
    let result = unsafe { unix_path::canonical_root(services, dir) }?;
    BASE_DIR.store(result, Ordering::SeqCst);
    Ok(())
}

/// `appSetUserDir`.
///
/// # Safety
/// `dir` must be null or a readable NUL-terminated TCHAR string. `services`
/// must satisfy its documented leaf contract.
pub unsafe fn set_user_dir(services: &Hp2UnixTextServices, dir: *const Tchar) -> Result<(), i32> {
    // SAFETY: `dir`/`services` are valid by contract.
    let result = unsafe { unix_path::canonical_root(services, dir) }?;
    USER_DIR.store(result, Ordering::SeqCst);
    Ok(())
}

/// `appBaseDir`: auto-populates from `getcwd` via `appSetBaseDir` the first
/// time it is read while still unset.
///
/// # Safety
/// `services` must satisfy its documented leaf contract.
pub unsafe fn base_dir(services: &Hp2UnixTextServices) -> Result<*const Tchar, i32> {
    let current = BASE_DIR.load(Ordering::SeqCst);
    // SAFETY: a non-null `BASE_DIR` is always a NUL-terminated allocation.
    let unset = current.is_null() || unsafe { *current } == 0;
    if unset {
        // SAFETY: this function's own contract.
        let utf8_base_dir = unsafe { get_current_directory() };
        if !utf8_base_dir.is_null() {
            // SAFETY: `utf8_base_dir` is NUL-terminated; `services` is valid.
            let decoded = unsafe { unix_text::from_utf8(services, utf8_base_dir.cast()) };
            // SAFETY: `utf8_base_dir` came from the native system allocator.
            unsafe { libc::free(utf8_base_dir.cast()) };
            let decoded = decoded?;
            // SAFETY: `decoded` is a NUL-terminated native allocation.
            let result = unsafe { set_base_dir(services, decoded) };
            // SAFETY: `decoded` came from the native system allocator.
            unsafe { libc::free(decoded.cast()) };
            result?;
        }
    }
    Ok(dir_or_empty(&BASE_DIR))
}


/// `appUserDir`: a plain read, no auto-population.
pub fn user_dir() -> *const Tchar {
    dir_or_empty(&USER_DIR)
}

static COMPUTER_NAME: AtomicPtr<Tchar> = AtomicPtr::new(null_mut());
static USER_NAME: AtomicPtr<Tchar> = AtomicPtr::new(null_mut());

/// `appComputerName`.
///
/// # Safety
/// `services` must satisfy its documented leaf contract.
pub unsafe fn computer_name(services: &Hp2UnixTextServices) -> Result<*const Tchar, i32> {
    let cached = COMPUTER_NAME.load(Ordering::SeqCst);
    if !cached.is_null() {
        return Ok(cached);
    }
    // SAFETY: this function's own contract.
    let utf8_name = unsafe { get_host_name() };
    let source: *const c_char = if utf8_name.is_null() {
        c"".as_ptr()
    } else {
        utf8_name
    };
    // SAFETY: `source` is NUL-terminated (either a real hostname or the
    // empty-string literal); `services` is valid by contract.
    let decoded = unsafe { unix_text::from_utf8(services, source.cast()) };
    // SAFETY: `libc::free` on a null pointer is a no-op, matching the
    // original's unconditional `free(Utf8Name)`.
    unsafe { libc::free(utf8_name.cast()) };
    let decoded = decoded?;
    COMPUTER_NAME.store(decoded, Ordering::SeqCst);
    Ok(decoded)
}

/// `appUserName`.
///
/// # Safety
/// `services` must satisfy its documented leaf contract.
pub unsafe fn user_name(services: &Hp2UnixTextServices) -> Result<*const Tchar, i32> {
    let cached = USER_NAME.load(Ordering::SeqCst);
    if !cached.is_null() {
        return Ok(cached);
    }
    // SAFETY: this function's own contract.
    let utf8_name = unsafe { get_login_name() };
    let source: *const c_char = if utf8_name.is_null() {
        c"".as_ptr()
    } else {
        utf8_name
    };
    // SAFETY: `source` is NUL-terminated; `services` is valid by contract.
    let decoded = unsafe { unix_text::from_utf8(services, source.cast()) };
    // SAFETY: `libc::free` on a null pointer is a no-op, matching the
    // original's unconditional `free(Utf8Name)`.
    unsafe { libc::free(utf8_name.cast()) };
    let decoded = decoded?;
    USER_NAME.store(decoded, Ordering::SeqCst);
    Ok(decoded)
}

const MODULE_LEN: usize = 32;
static mut CACHED_MODULE: [u8; MODULE_LEN] = [0; MODULE_LEN];
static PACKAGE_RESULT: AtomicPtr<Tchar> = AtomicPtr::new(null_mut());

/// `appPackage`: re-reads the live `GModule[32]` (owned and written by C++
/// bootstrap code in `SDLLaunch.cpp`/`UCC.cpp`, not by this module) each
/// call, recomputing only when it differs from the last cached snapshot.
///
/// # Safety
/// `module` must be readable for exactly [`MODULE_LEN`] (32) bytes. `services`
/// must satisfy its documented leaf contract. Single caller at a time: the
/// cache is process-global.
pub unsafe fn package(
    services: &Hp2UnixTextServices,
    module: *const u8,
) -> Result<*const Tchar, i32> {
    // SAFETY: `module` is readable for `MODULE_LEN` bytes by contract.
    let live: [u8; MODULE_LEN] = unsafe { std::ptr::read(module.cast()) };
    let cached_result = PACKAGE_RESULT.load(Ordering::SeqCst);
    // SAFETY: single-caller contract, matching the original's file-local
    // statics.
    let changed = unsafe { (&raw const CACHED_MODULE).read() } != live;
    if !cached_result.is_null() && !changed {
        return Ok(cached_result);
    }
    // SAFETY: single-caller contract.
    unsafe { (&raw mut CACHED_MODULE).write(live) };

    let mut length = 0usize;
    while length < MODULE_LEN && live[length] != 0 {
        length += 1;
    }
    // SAFETY: the system allocator contract.
    let terminated = unsafe { unix_alloc::system_malloc(length + 1) }.cast::<u8>();
    // SAFETY: `terminated` has room for `length + 1` bytes; `module` is
    // readable for at least `length` bytes (part of its 32-byte contract).
    unsafe {
        std::ptr::copy_nonoverlapping(module, terminated, length);
        *terminated.add(length) = 0;
    }
    // SAFETY: `terminated` is NUL-terminated; `services` is valid by contract.
    let decoded = unsafe { unix_text::from_utf8(services, terminated.cast()) };
    // SAFETY: `terminated` came from the native system allocator.
    unsafe { libc::free(terminated.cast()) };
    let decoded = decoded?;
    PACKAGE_RESULT.store(decoded, Ordering::SeqCst);
    Ok(decoded)
}

static LOCAL_IP: AtomicU32 = AtomicU32::new(0);

/// `appGetLocalIP`: resolves and caches this host's first IPv4 address via
/// its own independent `gethostname`/`gethostbyname` pair (not shared with
/// [`computer_name`]'s cache).
///
/// # Safety
/// None beyond the `gethostbyname` contract.
pub unsafe fn get_local_ip() -> u32 {
    let cached = LOCAL_IP.load(Ordering::SeqCst);
    if cached != 0 {
        return cached;
    }
    // SAFETY: this function's own contract.
    let hostname = unsafe { get_host_name() };
    if hostname.is_null() {
        return 0;
    }
    // SAFETY: `hostname` is NUL-terminated.
    let hostinfo = unsafe { net::gethostbyname(hostname) };
    let mut local_ip = 0u32;
    if !hostinfo.is_null() {
        // SAFETY: `hostinfo` is a valid `hostent` on success.
        let first_address = unsafe { *(*hostinfo).h_addr_list };
        if !first_address.is_null() {
            // SAFETY: an IPv4 `h_addr_list[0]` entry is at least 4 bytes;
            // `local_ip` is exactly 4 bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    first_address.cast::<u8>(),
                    (&raw mut local_ip).cast::<u8>(),
                    4,
                )
            };
        }
    }
    // SAFETY: `hostname` came from the native system allocator.
    unsafe { libc::free(hostname.cast()) };
    LOCAL_IP.store(local_ip, Ordering::SeqCst);
    local_ip
}
