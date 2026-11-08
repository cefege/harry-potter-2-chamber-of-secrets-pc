//! Port of the string and CRT wrappers in `HarryPotter2/Unreal/Core/Src/UnAnsi.cpp`
//! (`appStrncpy` through `appFromUtf8InPlace`). The C++ shells forward here through
//! `hp2_rs_core_ansi_*` (`crates/hp2-ffi/src/core_ansi.rs`).
//!
//! Build facts this port relies on. Every supported Core TU is built with
//! `-DUNICODE=1` and `HP2_HOST_WCHAR_TCHAR` (`UnGnuG.h`): `TCHAR` is the host
//! `wchar_t`, 32 bits, signed or unsigned per target (see [`Tchar`]). `ANSICHAR`
//! is `char`, `INT` and `UBOOL` are `i32`, `DWORD` is `u32`, `SIZE_T` is `usize`,
//! and `long` is 64-bit on the LP64 targets. The non-UNICODE branches of the
//! original bodies are not compiled into any Core TU and are not ported.
//!
//! Every libc call the original body makes is made here through the same symbol
//! (`wcs*`, `towupper`, `wcstol`, `atof`), so return values and locale behavior are
//! the host's own; `wcscmp` specifies only the sign of its result. The port adds
//! only the logic around those calls: sizes, checks, and the UTF-8 codecs. C++
//! callbacks (`appFailAssert`, `appToAnsi`, the `appErrorf` size contract) go through
//! [`Hp2AnsiServices`]. A callback that raises a C++ exception returns status 1; the
//! port then stops at once and returns [`Threw`], and the C++ shell rethrows the
//! recorded exception.

use std::ffi::c_void;
use std::ptr::{null, null_mut};

/// `TCHAR` on this build: the host `wchar_t`, 32 bits (`HP2_HOST_WCHAR_TCHAR`).
/// Signed on macOS arm64 and Linux x86_64, unsigned on Linux aarch64 (AAPCS64).
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub type Tchar = i32;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub type Tchar = i32;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub type Tchar = u32;
#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
)))]
compile_error!("hp2-core ansi: no host wchar_t TCHAR mapping for this target (UnGnuG.h)");

/// Widens an ASCII byte literal (including its NUL) to a `TCHAR` array at
/// compile time: the `TEXT("...")` literal of the original source.
pub const fn wide<const N: usize>(bytes: &[u8; N]) -> [Tchar; N] {
    let mut result = [0; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as Tchar;
        index += 1;
    }
    result
}

/// Elements in one `appStaticString1024` slot, terminator included.
pub const STATIC_STRING_LEN: usize = 1024;

// The single appStaticString1024 ring, shared with every native caller.
static mut STATIC_STRINGS: [[Tchar; STATIC_STRING_LEN]; 128] = [[0; STATIC_STRING_LEN]; 128];
static mut STATIC_STRING_INDEX: usize = 0;

// The byte ring is shared by native appToAnsi and Rust-only CRT parsing.
static mut STATIC_ANSI_STRINGS: [[u8; 1024]; 128] = [[0; 1024]; 128];
static mut STATIC_ANSI_INDEX: usize = 0;

/// # Safety
/// Engine-thread serialized; storage expires after 128 subsequent ring calls.
pub unsafe fn ansi_static_string_1024() -> *mut u8 {
    unsafe {
        let index = (&raw const STATIC_ANSI_INDEX).read();
        (&raw mut STATIC_ANSI_INDEX).write((index + 1) & 127);
        let output = (&raw mut STATIC_ANSI_STRINGS)
            .cast::<u8>()
            .add(index * 1024);
        output.write(0);
        output
    }
}

/// The canonical appToAnsi policy: byte-range narrowing, '?' substitution and
/// a 1023-character limit, using the same ring as native callers.
/// # Safety
/// Input is null or terminated readable TCHARs; ring access is serialized.
pub unsafe fn to_ansi_string(input: *const Tchar) -> *const u8 {
    if input.is_null() {
        return null();
    }
    unsafe {
        let output = ansi_static_string_1024();
        let mut count = 0;
        while count < 1023 && input.add(count).read() != 0 {
            let value = input.add(count).read() as i64;
            output.add(count).write(if (0..256).contains(&value) {
                value as u8
            } else {
                b'?'
            });
            count += 1;
        }
        output.add(count).write(0);
        output
    }
}

/// # Safety
/// Engine-thread serialized. The returned buffer is overwritten on the 128th
/// subsequent call; no reference into the ring may survive a reentrant call.
pub unsafe fn static_string_1024() -> *mut Tchar {
    unsafe {
        let index = (&raw const STATIC_STRING_INDEX).read();
        (&raw mut STATIC_STRING_INDEX).write((index + 1) & 127);
        let result = (&raw mut STATIC_STRINGS)
            .cast::<Tchar>()
            .add(index * STATIC_STRING_LEN);
        result.write(0);
        result
    }
}

/// Legacy ANSI widening and static-buffer truncation for owned text consumers.
pub fn ansi_scalars(input: impl Iterator<Item = u8>) -> impl Iterator<Item = u32> {
    input.take(STATIC_STRING_LEN - 1).take_while(|byte| *byte != 0).map(u32::from)
}

/// Legacy byte widening, including its 1023-character static-buffer limit.
/// # Safety
/// Input is null or terminated readable bytes. See `static_string_1024`.
pub unsafe fn from_ansi(input: *const u8) -> *const Tchar {
    if input.is_null() {
        return null();
    }
    unsafe {
        let output = static_string_1024();
        let mut count = 0;
        let mut source = input;
        let bytes = std::iter::from_fn(|| {
            let byte = source.read();
            source = source.add(1);
            Some(byte)
        });
        for scalar in ansi_scalars(bytes) {
            output.add(count).write(scalar as Tchar);
            count += 1;
        }
        output.add(count).write(0);
        output
    }
}

/// Legacy WORD-to-host-TCHAR decoding: surrogate pairs combine; isolated
/// surrogates become U+FFFD. Output truncates after 1023 host characters.
/// # Safety
/// Input is null or terminated readable WORDs, possibly unaligned bytecode.
/// See `static_string_1024` for serialization and result lifetime.
pub unsafe fn from_unicode(input: *const u16) -> *const Tchar {
    if input.is_null() {
        return null();
    }
    unsafe {
        let output = static_string_1024();
        let mut source = input;
        let mut count = 0;
        while source.read_unaligned() != 0 && count < 1023 {
            let first = source.read_unaligned() as u32;
            source = source.add(1);
            let scalar = if (0xd800..=0xdbff).contains(&first) {
                let second = source.read_unaligned() as u32;
                if (0xdc00..=0xdfff).contains(&second) {
                    source = source.add(1);
                    0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
                } else {
                    0xfffd
                }
            } else if (0xdc00..=0xdfff).contains(&first) {
                0xfffd
            } else {
                first
            };
            output.add(count).write(scalar as Tchar);
            count += 1;
        }
        output.add(count).write(0);
        output
    }
}

// The appToUnicode ring: native-only callers, distinct from the TCHAR ring.
static mut STATIC_UNICODE_STRINGS: [[u16; 1024]; 128] = [[0; 1024]; 128];
static mut STATIC_UNICODE_INDEX: usize = 0;

/// `appToUnicode` on host-`wchar_t` builds: encodes UTF-16 into the next slot
/// of its own 128-entry ring. Code points above U+FFFF become surrogate pairs;
/// surrogates, negative units and values above U+10FFFF become U+FFFD. Output
/// stops at 1023 units, and a pair that would reach unit 1023 ends the string.
///
/// # Safety
/// Input is null or terminated readable TCHARs. Ring access is engine-thread
/// serialized; a result expires after 128 subsequent calls.
pub unsafe fn to_unicode(input: *const Tchar) -> *const u16 {
    if input.is_null() {
        return null();
    }
    unsafe {
        let index = (&raw const STATIC_UNICODE_INDEX).read();
        (&raw mut STATIC_UNICODE_INDEX).write((index + 1) & 127);
        let output = (&raw mut STATIC_UNICODE_STRINGS)
            .cast::<u16>()
            .add(index * 1024);
        let mut source = input;
        let mut count = 0;
        while source.read() != 0 && count < 1023 {
            // The original widens through `UPTRINT`, so negative units are
            // out of every accepted range.
            let scalar = source.read() as i64;
            source = source.add(1);
            if (0..=0xd7ff).contains(&scalar) || (0xe000..=0xffff).contains(&scalar) {
                output.add(count).write(scalar as u16);
                count += 1;
            } else if (0x10000..=0x10ffff).contains(&scalar) {
                if count + 2 >= 1024 {
                    break;
                }
                let offset = scalar - 0x10000;
                output.add(count).write((0xd800 + (offset >> 10)) as u16);
                output
                    .add(count + 1)
                    .write((0xdc00 + (offset & 0x3ff)) as u16);
                count += 2;
            } else {
                output.add(count).write(0xfffd);
                count += 1;
            }
        }
        output.add(count).write(0);
        output
    }
}

/// `appIsPureAnsi` on the supported `UNICODE` builds: every non-NUL TCHAR must
/// fit in the byte range `[0, 255]`.
///
/// # Safety
/// `source` must point to a readable NUL-terminated TCHAR string.
pub unsafe fn is_pure_ansi(source: *const Tchar) -> bool {
    let mut cursor = source;
    loop {
        let scalar = unsafe { *cursor } as i64;
        if scalar == 0 {
            return true;
        }
        if !(0..=0xff).contains(&scalar) {
            return false;
        }
        cursor = unsafe { cursor.add(1) };
    }
}

/// `appTrimSpaces`: remove trailing ASCII spaces from at most the first 1024
/// bytes, preserving the original routine's one-byte floor.
///
/// # Safety
/// `string` must be writable and readable until its NUL terminator or for at
/// least 1024 bytes if no terminator occurs sooner.
pub unsafe fn trim_spaces(string: *mut u8) {
    let mut end = 0usize;
    while end < 1024 && unsafe { *string.add(end) } != 0 {
        end += 1;
    }
    if end > 0 {
        end -= 1;
    }
    while end > 0 && unsafe { *string.add(end) } == b' ' {
        unsafe { string.add(end).write(0) };
        end -= 1;
    }
}

#[cfg(test)]
mod pure_ansi_tests {
    use super::{Tchar, is_pure_ansi};

    #[test]
    fn matches_the_ansi_byte_range_and_rejects_out_of_range_tchars() {
        let empty = [0 as Tchar];
        let lower = [0xff as Tchar, 0 as Tchar];
        let above = [0x100 as Tchar, 0 as Tchar];
        #[cfg(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        ))]
        let negative = [-1 as Tchar, 0 as Tchar];
        // SAFETY: each array is a valid NUL-terminated TCHAR string.
        unsafe {
            assert!(is_pure_ansi(empty.as_ptr()));
            assert!(is_pure_ansi(lower.as_ptr()));
            assert!(!is_pure_ansi(above.as_ptr()));
            #[cfg(any(
                all(target_os = "macos", target_arch = "aarch64"),
                all(target_os = "linux", target_arch = "x86_64")
            ))]
            assert!(!is_pure_ansi(negative.as_ptr()));
        }
    }
}

#[cfg(test)]
mod trim_spaces_tests {
    use super::trim_spaces;

    fn trim(mut bytes: Vec<u8>) -> Vec<u8> {
        // SAFETY: each test buffer is writable and NUL-terminated or has at
        // least 1024 readable bytes.
        unsafe { trim_spaces(bytes.as_mut_ptr()) };
        bytes
    }

    #[test]
    fn preserves_the_legacy_trailing_space_and_scan_limit_rules() {
        assert_eq!(&trim(b"\0".to_vec())[..1], b"\0");
        assert_eq!(&trim(b"word  \0".to_vec())[..5], b"word\0");
        assert_eq!(&trim(b"   \0".to_vec())[..4], b" \0\0\0");
        assert_eq!(&trim(b"word\t \0".to_vec())[..7], b"word\t\0\0");

        let mut bounded = vec![b'x'; 1025];
        bounded[1023] = b' ';
        bounded[1024] = 0;
        let bounded = trim(bounded);
        assert_eq!(bounded[1022], b'x');
        assert_eq!(bounded[1023], 0);
        assert_eq!(bounded[1024], 0);
    }
}

/// `MAXINT`.
pub const MAXINT: i32 = i32::MAX;

/// Expression text of the `check()` in `appAtoi` and `appStrtoi`, as the C++
/// preprocessor stringizes it.
const INT_RANGE_EXPR: &[u8] =
    b"Value>=-static_cast<long>(MAXINT)-1 && Value<=static_cast<long>(MAXINT)\0";

/// C++ callbacks the port needs. Filled in `Core/Src/Hp2AnsiServices.cpp`; each
/// entry is a leaf call into the original C++ function.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2AnsiServices {
    /// `appFailAssert(Expr, File, Line)`. `expr` and `file` are NUL-terminated ANSI.
    /// Returns 0 when it completed, 1 when it raised a C++ exception.
    pub fail_assert:
        unsafe extern "C" fn(expr: *const c_void, file: *const c_void, line: i32) -> i32,
    /// `appToAnsi(Str)`: writes the static ANSI buffer (NULL for a NULL `Str`) to `*out`.
    /// Returns 0 when it completed, 1 when it raised a C++ exception.
    pub to_ansi: unsafe extern "C" fn(str: *const c_void, out: *mut *const c_void) -> i32,
    /// The `appErrorf(TEXT("Size exceeds the signed 32-bit API contract"))` of
    /// `appCheckedIntSize`. Returns 0 when it returned, 1 when it raised.
    pub size_exceeds_int_contract: unsafe extern "C" fn() -> i32,
}

/// Copied real ANSI leaves and the original integer diagnostic location.
#[derive(Clone, Copy)]
pub struct PublishedAnsi {
    pub services: Hp2AnsiServices,
    pub int_parse_file: *const c_void,
    pub int_parse_line: i32,
}

static mut PUBLISHED: Option<PublishedAnsi> = None;

/// # Safety
/// Engine-thread serialized. Callbacks and diagnostic storage remain live until
/// all calls using this publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: PublishedAnsi) {
    unsafe {
        (&raw mut PUBLISHED).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication. The copy holds no borrow across callbacks.
pub unsafe fn published_services() -> Result<PublishedAnsi, i32> {
    unsafe { (&raw const PUBLISHED).read().ok_or(1) }
}

unsafe extern "C" fn os_fail_assert(expr: *const c_void, file: *const c_void, line: i32) -> i32 {
    use std::io::Write;
    unsafe {
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(b"Assertion failed: ");
        let _ = stderr.write_all(std::ffi::CStr::from_ptr(expr.cast()).to_bytes());
        let _ = stderr.write_all(b" [File:");
        let _ = stderr.write_all(std::ffi::CStr::from_ptr(file.cast()).to_bytes());
        let _ = writeln!(stderr, "] [Line:{line}]");
    }
    1
}

unsafe extern "C" fn os_to_ansi(input: *const c_void, out: *mut *const c_void) -> i32 {
    unsafe {
        out.write(to_ansi_string(input.cast()).cast());
    }
    0
}

unsafe extern "C" fn os_size_error() -> i32 {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"Size exceeds the signed 32-bit API contract\n");
    1
}

/// Install actual CRT conversion and OS stderr error leaves for a Rust-only
/// process. Mixed runtimes publish their native exception-preserving leaves.
/// # Safety
/// Engine-thread serialized; no call may still use a replaced publication.
pub unsafe fn initialize_os_services() {
    unsafe {
        publish_services(PublishedAnsi {
            services: Hp2AnsiServices {
                fail_assert: os_fail_assert,
                to_ansi: os_to_ansi,
                size_exceeds_int_contract: os_size_error,
            },
            int_parse_file: c"hp2-core/ansi.rs".as_ptr().cast(),
            int_parse_line: line!() as i32,
        });
    }
}

/// Fixed primitive numeric formats used by FString::Printf. The native numeric
/// segment has 512 TCHARs; the same host wide CRT initializes a terminated
/// prefix. On CRT failure appVswprintfCompat publishes an empty string.
pub fn format_int(value: i32, output: &mut [std::mem::MaybeUninit<Tchar>; 512]) {
    let format = [37 as Tchar, 105, 0];
    let count = unsafe {
        libc::swprintf(
            output.as_mut_ptr().cast(),
            output.len(),
            format.as_ptr(),
            value,
        )
    };
    if count < 0 {
        output[0].write(0);
    }
}

pub fn format_float(value: f32, output: &mut [std::mem::MaybeUninit<Tchar>; 512]) {
    let format = [37 as Tchar, 102, 0];
    let count = unsafe {
        libc::swprintf(
            output.as_mut_ptr().cast(),
            output.len(),
            format.as_ptr(),
            value as f64,
        )
    };
    if count < 0 {
        output[0].write(0);
    }
}

/// Host libc. Each symbol is the call the original body makes, so return values and
/// locale behavior are the host's own.
pub(crate) mod libc {
    use super::Tchar;
    use std::ffi::{c_char, c_long};

    unsafe extern "C" {
        pub fn wcslen(s: *const Tchar) -> usize;
        pub fn wcscpy(dest: *mut Tchar, src: *const Tchar) -> *mut Tchar;
        pub fn wcsncpy(dest: *mut Tchar, src: *const Tchar, n: usize) -> *mut Tchar;
        pub fn wcscat(dest: *mut Tchar, src: *const Tchar) -> *mut Tchar;
        pub fn wcsstr(s: *const Tchar, find: *const Tchar) -> *mut Tchar;
        pub fn wcschr(s: *const Tchar, c: Tchar) -> *mut Tchar;
        pub fn wcscmp(a: *const Tchar, b: *const Tchar) -> i32;
        pub fn wcsncmp(a: *const Tchar, b: *const Tchar, n: usize) -> i32;
        pub fn wcscasecmp(a: *const Tchar, b: *const Tchar) -> i32;
        pub fn wcsncasecmp(a: *const Tchar, b: *const Tchar, n: usize) -> i32;
        pub fn wcstol(s: *const Tchar, end: *mut *mut Tchar, base: i32) -> c_long;
        /// `wint_t` has the same width and signedness as `wchar_t` on both targets.
        pub fn towupper(c: Tchar) -> Tchar;
        pub fn atof(s: *const c_char) -> f64;
        pub fn swprintf(dest: *mut Tchar, count: usize, format: *const Tchar, ...) -> i32;
    }
}

/// A service call raised a C++ exception. The C++ side has recorded it; the port
/// makes no further service calls, reports status 1, and the C++ shell rethrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Threw;

/// `appCheckedIntSize(Size)`: above `MAXINT` it reports the contract violation
/// through the services, then truncates to `INT` as the original does.
pub fn checked_int_size(svc: &Hp2AnsiServices, size: usize) -> Result<i32, Threw> {
    if size > MAXINT as usize {
        // SAFETY: the services table is filled by the C++ side and outlives the call.
        if unsafe { (svc.size_exceeds_int_contract)() } != 0 {
            return Err(Threw);
        }
    }
    Ok(size as i32)
}

/// `appStrncpy`: `wcsncpy(Dest, Src, MaxLen)`, then `Dest[MaxLen-1] = 0`.
///
/// Out-of-range policy: `MaxLen <= 0` is undefined in the original (`Dest[-1]`).
/// The port writes the same index without a guard.
///
/// # Safety
/// `dest` must have room for `max_len` wide characters and `src` must be a
/// NUL-terminated (or sufficiently long) wide string.
pub unsafe fn strncpy(dest: *mut Tchar, src: *const Tchar, max_len: i32) -> *mut Tchar {
    unsafe {
        libc::wcsncpy(dest, src, max_len as usize);
        *dest.offset(max_len as isize - 1) = 0;
        dest
    }
}

/// `appStrncat`: appends `Src` into whatever room is left in `Dest` after `strlen`.
///
/// # Safety
/// `dest` must be a NUL-terminated buffer of at least `max_len` characters and
/// `src` must be a NUL-terminated wide string.
pub unsafe fn strncat(
    svc: &Hp2AnsiServices,
    dest: *mut Tchar,
    src: *const Tchar,
    max_len: i32,
) -> Result<*mut Tchar, Threw> {
    unsafe {
        let len = strlen(svc, dest)?;
        let new_dest = dest.add(len as usize);
        let remaining = max_len.wrapping_sub(len);
        if remaining > 0 {
            strncpy(new_dest, src, remaining);
            *new_dest.offset(remaining as isize - 1) = 0;
        }
        Ok(dest)
    }
}

/// `appStrlen`: `appCheckedIntSize(wcslen(String))`.
///
/// # Safety
/// `s` must be a NUL-terminated wide string.
pub unsafe fn strlen(svc: &Hp2AnsiServices, s: *const Tchar) -> Result<i32, Threw> {
    unsafe { checked_int_size(svc, libc::wcslen(s)) }
}

/// `appStrstr`: `wcsstr(String, Find)`. An empty `Find` returns `String`.
///
/// # Safety
/// Both arguments must be NUL-terminated wide strings.
pub unsafe fn strstr(s: *const Tchar, find: *const Tchar) -> *mut Tchar {
    unsafe { libc::wcsstr(s, find) }
}

/// `appStrchr`: `wcschr(String, c)`. Like libc, `c == 0` matches the terminator.
///
/// # Safety
/// `s` must be a NUL-terminated wide string.
pub unsafe fn strchr(s: *const Tchar, c: i32) -> *mut Tchar {
    unsafe { libc::wcschr(s, c as Tchar) }
}

/// `appStrcat`: `wcscat(Dest, Src)`.
///
/// # Safety
/// `dest` must have room for the concatenated result; both must be NUL-terminated.
pub unsafe fn strcat(dest: *mut Tchar, src: *const Tchar) -> *mut Tchar {
    unsafe { libc::wcscat(dest, src) }
}

/// `appStrcmp`: `wcscmp(String1, String2)`. The result is libc's own; only its sign is
/// specified by the standard.
///
/// # Safety
/// Both arguments must be NUL-terminated wide strings.
pub unsafe fn strcmp(a: *const Tchar, b: *const Tchar) -> i32 {
    unsafe { libc::wcscmp(a, b) }
}

/// `appStricmp`: `wcscasecmp(String1, String2)`.
///
/// # Safety
/// Both arguments must be NUL-terminated wide strings.
pub unsafe fn stricmp(a: *const Tchar, b: *const Tchar) -> i32 {
    unsafe { libc::wcscasecmp(a, b) }
}

/// `appStrcpy`: `wcscpy(Dest, Src)`.
///
/// # Safety
/// `dest` must have room for `src` including its NUL.
pub unsafe fn strcpy(dest: *mut Tchar, src: *const Tchar) -> *mut Tchar {
    unsafe { libc::wcscpy(dest, src) }
}

/// `appStrupr`: upper-cases each character through `towupper` in place.
///
/// # Safety
/// `s` must be a writable NUL-terminated wide string.
pub unsafe fn strupr(s: *mut Tchar) -> *mut Tchar {
    unsafe {
        let mut p = s;
        while *p != 0 {
            *p = libc::towupper(*p);
            p = p.add(1);
        }
        s
    }
}

/// The `check()` of `appAtoi`/`appStrtoi`. A failing check reports through
/// `appFailAssert`. If that returns, the value is truncated and returned, as the
/// original does; if it raises, the port returns [`Threw`].
fn check_int_range(
    svc: &Hp2AnsiServices,
    value: i64,
    file: *const c_void,
    line: i32,
) -> Result<i32, Threw> {
    if !(value >= -(MAXINT as i64) - 1 && value <= MAXINT as i64) {
        // SAFETY: the services table is filled by the C++ side and outlives the call.
        if unsafe { (svc.fail_assert)(INT_RANGE_EXPR.as_ptr().cast(), file, line) } != 0 {
            return Err(Threw);
        }
    }
    Ok(value as i32)
}

/// `appAtoi`: `wcstol(Str, NULL, 10)` with the `check` on the result range.
/// `file` and `line` are the original `UnAnsi.cpp` `__FILE__` and check line.
///
/// # Safety
/// `s` must be a NUL-terminated wide string; `file` must be a NUL-terminated ANSI string.
pub unsafe fn atoi(
    svc: &Hp2AnsiServices,
    s: *const Tchar,
    file: *const c_void,
    line: i32,
) -> Result<i32, Threw> {
    match unsafe { atoi_checked(s) } {
        Ok(value) => Ok(value),
        Err(value) => check_int_range(svc, value, file, line),
    }
}

/// Parses the same decimal wide text as `appAtoi`, returning its INT-range failure.
///
/// # Safety
/// `s` must point to readable NUL-terminated wide text.
pub unsafe fn atoi_checked(s: *const Tchar) -> Result<i32, i64> {
    let value = unsafe { libc::wcstol(s, null_mut(), 10) };
    i32::try_from(value).map_err(|_| value)
}

/// `appToAnsi(Str)` through the services: the C++ static ANSI buffer.
fn to_ansi(svc: &Hp2AnsiServices, s: *const Tchar) -> Result<*const c_void, Threw> {
    let mut out: *const c_void = null();
    // SAFETY: `out` is a valid slot and the services table outlives the call.
    if unsafe { (svc.to_ansi)(s.cast(), &mut out) } != 0 {
        return Err(Threw);
    }
    Ok(out)
}

/// `appAtof`: `atof(appToAnsi(Str))`.
///
/// # Safety
/// `s` must be a NUL-terminated wide string (or NULL, which faults as in the original).
pub unsafe fn atof(svc: &Hp2AnsiServices, s: *const Tchar) -> Result<f32, Threw> {
    unsafe {
        let ansi = to_ansi(svc, s)?;
        Ok(libc::atof(ansi.cast()) as f32)
    }
}

/// `appStrtoi`: `wcstol(Start, End, Base)` with the same `check` as `appAtoi`.
///
/// # Safety
/// `start` must be a NUL-terminated wide string; `end` is NULL or points to a `TCHAR*` slot.
pub unsafe fn strtoi(
    svc: &Hp2AnsiServices,
    start: *const Tchar,
    end: *mut *mut Tchar,
    base: i32,
    file: *const c_void,
    line: i32,
) -> Result<i32, Threw> {
    unsafe {
        let value = libc::wcstol(start, end, base);
        check_int_range(svc, value, file, line)
    }
}

/// `appStrncmp`: `wcsncmp(A, B, Count)`. `Count` is converted to `size_t` as libc sees it.
///
/// # Safety
/// `a` and `b` must be NUL-terminated (or long enough to hit a difference or NUL within `count`).
pub unsafe fn strncmp(a: *const Tchar, b: *const Tchar, count: i32) -> i32 {
    unsafe { libc::wcsncmp(a, b, count as usize) }
}

/// `appStrnicmp`: `wcsncasecmp(A, B, Count)`.
///
/// # Safety
/// Same requirements as `wcsncasecmp`.
pub unsafe fn strnicmp(a: *const Tchar, b: *const Tchar, count: i32) -> i32 {
    unsafe { libc::wcsncasecmp(a, b, count as usize) }
}

/// `appToUtf8InPlace`: encodes wide `Src` as UTF-8 into `Dest`, NUL-terminated,
/// within `DestCapacity` bytes. Returns 1 on success, 0 otherwise (with `Dest[0] = 0`).
///
/// # Safety
/// `dest` must have `capacity` writable bytes; `src` must be a NUL-terminated wide string.
pub unsafe fn to_utf8_in_place(dest: *mut i8, src: *const Tchar, capacity: i32) -> i32 {
    unsafe {
        if dest.is_null() || src.is_null() || capacity <= 0 {
            return 0;
        }
        let mut dest_index: i32 = 0;
        let mut src = src;
        while *src != 0 {
            // `static_cast<DWORD>(*Src)`: a negative wchar_t wraps to a large scalar and fails below.
            let scalar = *src as u32;
            let mut encoded = [0i8; 4];
            let encoded_length: usize;
            if scalar <= 0x7f {
                encoded[0] = scalar as i8;
                encoded_length = 1;
            } else if scalar <= 0x7ff {
                encoded[0] = (0xc0 | (scalar >> 6)) as i8;
                encoded[1] = (0x80 | (scalar & 0x3f)) as i8;
                encoded_length = 2;
            } else if scalar <= 0xffff && !(0xd800..=0xdfff).contains(&scalar) {
                encoded[0] = (0xe0 | (scalar >> 12)) as i8;
                encoded[1] = (0x80 | ((scalar >> 6) & 0x3f)) as i8;
                encoded[2] = (0x80 | (scalar & 0x3f)) as i8;
                encoded_length = 3;
            } else if scalar <= 0x10ffff {
                encoded[0] = (0xf0 | (scalar >> 18)) as i8;
                encoded[1] = (0x80 | ((scalar >> 12) & 0x3f)) as i8;
                encoded[2] = (0x80 | ((scalar >> 6) & 0x3f)) as i8;
                encoded[3] = (0x80 | (scalar & 0x3f)) as i8;
                encoded_length = 4;
            } else {
                *dest = 0;
                return 0;
            }
            if encoded_length as i32 >= capacity.wrapping_sub(dest_index) {
                *dest = 0;
                return 0;
            }
            for &byte in &encoded[..encoded_length] {
                *dest.add(dest_index as usize) = byte;
                dest_index += 1;
            }
            src = src.add(1);
        }
        *dest.add(dest_index as usize) = 0;
        1
    }
}

/// `appFromUtf8InPlace`: decodes UTF-8 `Src` into wide `Dest`, NUL-terminated,
/// within `DestCapacity` characters. Returns 1 on success, 0 otherwise (with `Dest[0] = 0`).
/// Overlong forms, surrogates, and scalars above U+10FFFF are rejected.
///
/// # Safety
/// `dest` must have `capacity` writable wide characters; `src` must be a NUL-terminated byte string.
pub unsafe fn from_utf8_in_place(dest: *mut Tchar, src: *const i8, capacity: i32) -> i32 {
    unsafe {
        if dest.is_null() || src.is_null() || capacity <= 0 {
            return 0;
        }
        let mut dest_index: i32 = 0;
        let mut src = src;
        while *src != 0 {
            let first = *src as u8;
            src = src.add(1);
            let mut scalar: u32;
            let continuations: usize;
            let minimum: u32;
            if first < 0x80 {
                scalar = u32::from(first);
                continuations = 0;
                minimum = 0;
            } else if (first & 0xe0) == 0xc0 {
                scalar = u32::from(first & 0x1f);
                continuations = 1;
                minimum = 0x80;
            } else if (first & 0xf0) == 0xe0 {
                scalar = u32::from(first & 0x0f);
                continuations = 2;
                minimum = 0x800;
            } else if (first & 0xf8) == 0xf0 {
                scalar = u32::from(first & 0x07);
                continuations = 3;
                minimum = 0x10000;
            } else {
                *dest = 0;
                return 0;
            }
            for _ in 0..continuations {
                let next = *src as u8;
                src = src.add(1);
                if next == 0 || (next & 0xc0) != 0x80 {
                    *dest = 0;
                    return 0;
                }
                scalar = (scalar << 6) | u32::from(next & 0x3f);
            }
            if scalar < minimum
                || scalar > 0x10ffff
                || (0xd800..=0xdfff).contains(&scalar)
                || dest_index >= capacity - 1
            {
                *dest = 0;
                return 0;
            }
            *dest.add(dest_index as usize) = scalar as Tchar;
            dest_index += 1;
        }
        *dest.add(dest_index as usize) = 0;
        1
    }
}
