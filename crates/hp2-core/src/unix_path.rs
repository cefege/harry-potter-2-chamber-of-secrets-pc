//! Port of the pathname helpers in `Core/Src/UnUnix.cpp`: `appUnixPath` (the
//! `PATH(s)` macro), and `appUnixNormalizeRootText`/`appUnixCanonicalRoot`,
//! which back `appSetBaseDir`/`appSetUserDir`.

use std::ffi::c_char;
use std::io::Write;
use std::mem::size_of;
use std::ptr::null_mut;

use crate::ansi::Tchar;
use crate::unix_alloc;
use crate::unix_text::{self, Hp2UnixTextServices};

static mut UNIX_PATH: *mut c_char = null_mut();
static mut UNIX_PATH_CAPACITY: usize = 0;

/// `appUnixPath`: caches one growable ANSI buffer (never shrunk, never freed,
/// not thread-safe), copies `path` into it, and rewrites `\` to `/` in place.
/// Returns null only when `strlen(path)` is `SIZE_MAX` (the original's
/// unreachable-in-practice `appErrorf` path); the caller must raise that
/// error itself, since it needs native `GError` dispatch.
///
/// # Safety
/// `path` must be null or a readable NUL-terminated ANSI string. Single
/// caller at a time: the cached buffer and its capacity are process-global.
pub unsafe fn unix_path(path: *const c_char) -> Option<*mut c_char> {
    // SAFETY: `path` is null or NUL-terminated by contract.
    let length = if path.is_null() {
        0
    } else {
        unsafe { libc::strlen(path) }
    };
    if length == usize::MAX {
        return None;
    }
    unsafe {
        if (&raw const UNIX_PATH_CAPACITY).read() < length + 1 {
            (&raw mut UNIX_PATH_CAPACITY).write(length + 1);
            let grown = unix_alloc::system_realloc(
                (&raw const UNIX_PATH).read().cast(),
                (&raw const UNIX_PATH_CAPACITY).read(),
            );
            (&raw mut UNIX_PATH).write(grown.cast());
        }
        let buffer = (&raw const UNIX_PATH).read();
        if path.is_null() {
            buffer.write(0);
        } else {
            std::ptr::copy_nonoverlapping(path, buffer, length + 1);
        }
        let mut cursor = buffer;
        while *cursor != 0 {
            if *cursor == b'\\' as c_char {
                *cursor = b'/' as c_char;
            }
            cursor = cursor.add(1);
        }
        Some(buffer)
    }
}

/// `fputs("Unreal: root path is too large\n", stderr); abort();`
fn abort_root_too_large() -> ! {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(b"Unreal: root path is too large\n");
    drop(stderr);
    std::process::abort();
}

/// `appUnixNormalizeRootText`: collapses `\`/repeated `/` into a single `/`,
/// trims a trailing slash to exactly one, in a freshly `system_malloc`'d
/// buffer. Matches the original: the result is never freed (it feeds
/// `GUnixBaseDir`/`GUnixUserDir`, which intentionally retain whatever buffer
/// was last published rather than risk invalidating a pointer already handed
/// to early bootstrap code).
///
/// # Safety
/// `dir` must be null or a readable NUL-terminated TCHAR string.
unsafe fn normalize_root_text(dir: *const Tchar) -> *mut Tchar {
    // SAFETY: `dir` is null or NUL-terminated by contract; `tchar_length`
    // treats null as empty.
    let source_length = unsafe { unix_text::tchar_length(dir) };
    if source_length > (usize::MAX / size_of::<Tchar>()) - 2 {
        abort_root_too_large();
    }
    // SAFETY: the system allocator contract; the result is intentionally
    // never freed (see doc comment above).
    let result = unsafe {
        unix_alloc::system_malloc((source_length + 2) * size_of::<Tchar>()).cast::<Tchar>()
    };

    let mut dest_length = 0usize;
    let mut source = dir;
    loop {
        // SAFETY: `source` is null (treated as empty) or NUL-terminated.
        let raw = if source.is_null() {
            0
        } else {
            unsafe { *source }
        };
        if raw == 0 {
            break;
        }
        let ch = if raw == b'\\' as Tchar {
            b'/' as Tchar
        } else {
            raw
        };
        if ch == b'/' as Tchar
            && dest_length > 0
            // SAFETY: `dest_length - 1` is a previously written slot.
            && unsafe { *result.add(dest_length - 1) } == b'/' as Tchar
        {
            source = unsafe { source.add(1) };
            continue;
        }
        // SAFETY: capacity reserved above (`source_length + 2` elements).
        unsafe { *result.add(dest_length) = ch };
        dest_length += 1;
        source = unsafe { source.add(1) };
    }

    // SAFETY: every index touched below is within the reserved capacity.
    unsafe {
        while dest_length > 1 && *result.add(dest_length - 1) == b'/' as Tchar {
            dest_length -= 1;
        }
        if dest_length > 0 && *result.add(dest_length - 1) != b'/' as Tchar {
            *result.add(dest_length) = b'/' as Tchar;
            dest_length += 1;
        }
        *result.add(dest_length) = 0;
    }
    result
}

/// `appUnixCanonicalRoot`.
///
/// # Safety
/// `dir` must be null or a readable NUL-terminated TCHAR string, stable
/// through the services callbacks. `services` must satisfy its documented
/// leaf contract.
pub(crate) unsafe fn canonical_root(
    services: &Hp2UnixTextServices,
    dir: *const Tchar,
) -> Result<*mut Tchar, i32> {
    // SAFETY: `dir` is null or NUL-terminated by contract.
    let empty = dir.is_null() || unsafe { *dir } == 0;
    if empty {
        return Ok(unsafe { normalize_root_text(dir) });
    }

    // SAFETY: `dir` is nonnull and NUL-terminated here; `services` is valid
    // by contract.
    let utf8_dir = unsafe { unix_text::to_utf8(services, dir) }?;
    // SAFETY: `utf8_dir` is a fresh NUL-terminated native allocation.
    unsafe { unix_text::normalize_path(utf8_dir) };
    // SAFETY: `utf8_dir` is NUL-terminated; `realpath` with a null resolved
    // buffer mallocs its own result.
    let resolved_utf8 = unsafe { libc::realpath(utf8_dir.cast(), null_mut()) };
    // SAFETY: `utf8_dir` came from the native system allocator.
    unsafe { libc::free(utf8_dir.cast()) };
    if resolved_utf8.is_null() {
        return Ok(unsafe { normalize_root_text(dir) });
    }

    // SAFETY: `realpath` returns a NUL-terminated string on success; `services`
    // is valid by contract.
    let resolved_dir = unsafe { unix_text::from_utf8(services, resolved_utf8.cast()) };
    // SAFETY: `resolved_utf8` came from `realpath`'s native `malloc`; freed
    // unconditionally so a decode failure does not leak it.
    unsafe { libc::free(resolved_utf8.cast()) };
    let resolved_dir = resolved_dir?;
    // SAFETY: `resolved_dir` is a NUL-terminated native allocation.
    let result = unsafe { normalize_root_text(resolved_dir) };
    // SAFETY: `resolved_dir` came from the native system allocator.
    unsafe { libc::free(resolved_dir.cast()) };
    Ok(result)
}
