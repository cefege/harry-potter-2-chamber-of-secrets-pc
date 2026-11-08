//! Bootstrap text conversion from `Core/Src/UnUnix.cpp`.
//!
//! Unlike `ansi`'s bounded codecs, these routines replace invalid scalars and
//! malformed UTF-8 instead of failing. The decoder consumes valid continuation
//! prefixes even when the complete sequence is invalid; `from_utf8_lossy` is not
//! equivalent. `path::native_path_to_utf8` also collapses slashes, so cannot be
//! used here. Only the existing host `Tchar` representation is shared.
//!
//! There are no Rust allocations. Each conversion requests exactly the legacy
//! capacity from the native system allocator and writes directly into it. No
//! callback or fallible operation follows successful allocation. The completed
//! pointer is transferred to the caller, which must release it with native
//! libc `free`, never the engine allocator.

use crate::ansi::Tchar;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;

/// Native diagnostic selection; the callback prints and aborts.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixTextFailure {
    /// `Unreal: path is too large to encode as UTF-8\n`.
    EncodeOverflow,
    /// `Unreal: UTF-8 string is too large to decode\n`.
    DecodeOverflow,
}

/// Immutable bootstrap leaves; no engine object borrow or arena lock is held.
///
/// Callbacks must contain all native exceptions and must never unwind through
/// Rust. A nonzero status is returned unchanged and no output is published.
/// `allocate` must return a nonnull, suitably TCHAR-aligned system allocation
/// of exactly `size` bytes on success. On failure it retains responsibility for
/// any allocation and must not transfer ownership through `out`. It must leave
/// the source string intact. `fatal` normally prints the diagnostic selected by
/// `error_kind` and aborts; if it returns, it must report a nonzero status.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixTextServices {
    pub opaque: *mut c_void,
    pub allocate:
        unsafe extern "C" fn(opaque: *mut c_void, size: usize, out: *mut *mut c_void) -> i32,
    pub fatal: unsafe extern "C" fn(opaque: *mut c_void, error_kind: Hp2UnixTextFailure) -> i32,
}

/// Count TCHAR code units before NUL, treating null as the empty string.
///
/// # Safety
/// A nonnull `text` must be a readable, aligned, NUL-terminated TCHAR string.
pub unsafe fn tchar_length(text: *const Tchar) -> usize {
    if text.is_null() {
        return 0;
    }
    let mut count = 0;
    unsafe {
        while *text.add(count) != 0 {
            count += 1;
        }
    }
    count
}

unsafe fn overflow(services: &Hp2UnixTextServices, kind: Hp2UnixTextFailure) -> i32 {
    let status = unsafe { (services.fatal)(services.opaque, kind) };
    // A returning fatal leaf must never turn an unproduced output into success.
    if status == 0 { 1 } else { status }
}

unsafe fn allocate(services: &Hp2UnixTextServices, size: usize) -> Result<*mut c_void, i32> {
    let mut output = null_mut();
    let status = unsafe { (services.allocate)(services.opaque, size, &mut output) };
    if status == 0 { Ok(output) } else { Err(status) }
}

/// Encode into one native allocation of exactly `TCHAR_count * 4 + 1` bytes.
/// Invalid UTF-32 scalars and unpaired UTF-16 surrogates become U+FFFD.
///
/// # Safety
/// `text` is null or an aligned, readable NUL-terminated TCHAR string, stable
/// through the allocation callback. `services` satisfies its documented leaf
/// contract and remains immutable throughout the call. The allocation must not
/// overlap the source. On success the returned pointer belongs to the caller.
pub unsafe fn to_utf8(services: &Hp2UnixTextServices, text: *const Tchar) -> Result<*mut u8, i32> {
    unsafe {
        let count = tchar_length(text);
        if count > (usize::MAX - 1) / 4 {
            return Err(overflow(services, Hp2UnixTextFailure::EncodeOverflow));
        }
        let bytes = allocate(services, count * 4 + 1)?.cast::<u8>();
        let mut dest = bytes;
        let mut source = text;
        while !source.is_null() && *source != 0 {
            let mut scalar;
            if size_of::<Tchar>() == size_of::<u16>() {
                let first = *source as u16 as u32;
                source = source.add(1);
                if (0xd800..=0xdbff).contains(&first) {
                    let second = *source as u16 as u32;
                    if (0xdc00..=0xdfff).contains(&second) {
                        source = source.add(1);
                        scalar = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
                    } else {
                        scalar = 0xfffd;
                    }
                } else {
                    scalar = if (0xdc00..=0xdfff).contains(&first) {
                        0xfffd
                    } else {
                        first
                    };
                }
            } else {
                let host_char = *source as i64;
                source = source.add(1);
                scalar = if (0..=0x10ffff).contains(&host_char) {
                    host_char as u32
                } else {
                    0xfffd
                };
                if (0xd800..=0xdfff).contains(&scalar) {
                    scalar = 0xfffd;
                }
            }

            if scalar <= 0x7f {
                dest.write(scalar as u8);
                dest = dest.add(1);
            } else if scalar <= 0x7ff {
                dest.write((0xc0 | (scalar >> 6)) as u8);
                dest.add(1).write((0x80 | (scalar & 0x3f)) as u8);
                dest = dest.add(2);
            } else if scalar <= 0xffff {
                dest.write((0xe0 | (scalar >> 12)) as u8);
                dest.add(1).write((0x80 | ((scalar >> 6) & 0x3f)) as u8);
                dest.add(2).write((0x80 | (scalar & 0x3f)) as u8);
                dest = dest.add(3);
            } else {
                dest.write((0xf0 | (scalar >> 18)) as u8);
                dest.add(1).write((0x80 | ((scalar >> 12) & 0x3f)) as u8);
                dest.add(2).write((0x80 | ((scalar >> 6) & 0x3f)) as u8);
                dest.add(3).write((0x80 | (scalar & 0x3f)) as u8);
                dest = dest.add(4);
            }
        }
        dest.write(0);
        Ok(bytes)
    }
}

/// Decode into one native allocation of `(byte_count + 1) * sizeof(TCHAR)`.
/// NUL terminates input; null is an empty input. Invalid leads consume one byte.
/// Recognized leads consume every valid continuation prefix before replacement,
/// including complete overlong, surrogate, and out-of-range sequences. The first
/// non-continuation byte is left for the next iteration, and NUL is not consumed.
///
/// # Safety
/// `text` is null or a readable NUL-terminated byte string, stable through the
/// allocation callback. `services` satisfies its documented leaf contract and
/// stays immutable. The allocation must not overlap the source. The caller owns
/// the returned pointer only on success.
pub unsafe fn from_utf8(
    services: &Hp2UnixTextServices,
    text: *const u8,
) -> Result<*mut Tchar, i32> {
    unsafe {
        let mut count = 0;
        if !text.is_null() {
            while *text.add(count) != 0 {
                count += 1;
            }
        }
        if count > usize::MAX / size_of::<Tchar>() - 1 {
            return Err(overflow(services, Hp2UnixTextFailure::DecodeOverflow));
        }
        let chars = allocate(services, (count + 1) * size_of::<Tchar>())?.cast::<Tchar>();
        let mut dest = chars;
        let mut source = text;
        while !source.is_null() && *source != 0 {
            let first = *source;
            source = source.add(1);
            let (mut scalar, extra, minimum) = if first < 0x80 {
                (u32::from(first), 0, 0)
            } else if (0xc2..=0xdf).contains(&first) {
                (u32::from(first & 0x1f), 1, 0x80)
            } else if (0xe0..=0xef).contains(&first) {
                (u32::from(first & 0x0f), 2, 0x800)
            } else if (0xf0..=0xf4).contains(&first) {
                (u32::from(first & 0x07), 3, 0x10000)
            } else {
                (0xfffd, 0, 0)
            };
            let mut valid = true;
            for _ in 0..extra {
                let continuation = *source;
                if continuation == 0 || (continuation & 0xc0) != 0x80 {
                    valid = false;
                    break;
                }
                source = source.add(1);
                scalar = (scalar << 6) | u32::from(continuation & 0x3f);
            }
            if !valid
                || scalar < minimum
                || scalar > 0x10ffff
                || (0xd800..=0xdfff).contains(&scalar)
            {
                scalar = 0xfffd;
            }
            if size_of::<Tchar>() == size_of::<u16>() && scalar > 0xffff {
                scalar -= 0x10000;
                dest.write((0xd800 + (scalar >> 10)) as Tchar);
                dest = dest.add(1);
                dest.write((0xdc00 + (scalar & 0x3ff)) as Tchar);
                dest = dest.add(1);
            } else {
                dest.write(scalar as Tchar);
                dest = dest.add(1);
            }
        }
        dest.write(0);
        Ok(chars)
    }
}

/// Replace backslashes with forward slashes in place; do not collapse slashes.
///
/// # Safety
/// `text` must be a writable NUL-terminated byte string with no concurrent access.
pub unsafe fn normalize_path(mut text: *mut u8) {
    unsafe {
        while *text != 0 {
            if *text == b'\\' {
                *text = b'/';
            }
            text = text.add(1);
        }
    }
}
