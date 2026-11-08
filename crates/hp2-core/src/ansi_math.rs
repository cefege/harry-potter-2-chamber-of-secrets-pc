//! Port of the math wrappers, the memory wrappers, and `appGetGMTRef` in
//! `HarryPotter2/Unreal/Core/Src/UnAnsi.cpp`. The C++ shells forward here through
//! `hp2_rs_core_ansi_*` (`crates/hp2-ffi/src/core_ansi_math.rs`).
//!
//! Every libm and libc call the original body makes is made through the same
//! symbol, so return values and NaN payloads are the host's own. `DOUBLE` is `f64`,
//! `FLOAT` is `f32`, `INT` and `UBOOL` are `i32`, `SIZE_T` is `usize`. A C `int`
//! count is sign-extended to `size_t` exactly as the C++ implicit conversion does.
//!
//! Not ported here (see the C++ shells for the reason): `appQsort`, because its
//! comparator is a C++ callback that may raise; `appFloor` and `appRound`, whose
//! out-of-line bodies are compiled out on every `__GNUG__` build; `appMemzero`,
//! likewise compiled out (`DEFINED_appMemzero` in `UnGnuG.h`); `appTimestamp`
//! (`_MSC_VER` only); `appSprintf` and `appGetVarArgs` (varargs).

use std::ffi::{c_long, c_void};

use crate::ansi::Tchar;

/// Host libc. Each symbol is the call the original body makes.
mod libc {
    use std::ffi::{c_int, c_long, c_void};

    unsafe extern "C" {
        pub fn time(t: *mut c_long) -> c_long;
        pub fn gmtime(t: *const c_long) -> *mut c_void;
        pub fn mktime(tm: *mut c_void) -> c_long;
        pub fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int;
        pub fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
        pub fn memset(dest: *mut c_void, c: c_int, n: usize) -> *mut c_void;
        pub fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
    }
}

/// Host libm. Each symbol is the call the original body makes, at `double` precision.
mod libm {
    unsafe extern "C" {
        pub fn exp(x: f64) -> f64;
        pub fn log(x: f64) -> f64;
        pub fn fmod(x: f64, y: f64) -> f64;
        pub fn sin(x: f64) -> f64;
        pub fn cos(x: f64) -> f64;
        pub fn acos(x: f64) -> f64;
        pub fn tan(x: f64) -> f64;
        pub fn atan(x: f64) -> f64;
        pub fn atan2(y: f64, x: f64) -> f64;
        pub fn sqrt(x: f64) -> f64;
        pub fn pow(x: f64, y: f64) -> f64;
    }
}

/// `appExp`.
pub fn exp(value: f64) -> f64 {
    unsafe { libm::exp(value) }
}

/// `appLoge`.
pub fn loge(value: f64) -> f64 {
    unsafe { libm::log(value) }
}

/// `appFmod(Y, X)`: `fmod(Y, X)`.
pub fn fmod(y: f64, x: f64) -> f64 {
    unsafe { libm::fmod(y, x) }
}

/// `appSin`.
pub fn sin(value: f64) -> f64 {
    unsafe { libm::sin(value) }
}

/// `appCos`.
pub fn cos(value: f64) -> f64 {
    unsafe { libm::cos(value) }
}

/// `appAcos`.
pub fn acos(value: f64) -> f64 {
    unsafe { libm::acos(value) }
}

/// `appTan`.
pub fn tan(value: f64) -> f64 {
    unsafe { libm::tan(value) }
}

/// `appAtan`.
pub fn atan(value: f64) -> f64 {
    unsafe { libm::atan(value) }
}

/// `appAtan2(Y, X)`: `atan2(Y, X)`.
pub fn atan2(y: f64, x: f64) -> f64 {
    unsafe { libm::atan2(y, x) }
}

/// `appSqrt`.
pub fn sqrt(value: f64) -> f64 {
    unsafe { libm::sqrt(value) }
}

/// `appPow(A, B)`: `pow(A, B)`.
pub fn pow(a: f64, b: f64) -> f64 {
    unsafe { libm::pow(a, b) }
}

/// `appIsNan` on the non-`_MSC_VER` branch: `isnan(A)==1`.
pub fn is_nan(a: f64) -> i32 {
    i32::from(a.is_nan())
}

/// `appCeil`: `(INT)ceil(Value)`.
///
/// The oracle compiles to `fcvtps w0, s0` on arm64: round toward +infinity, then a
/// saturating convert with NaN mapped to 0. That is `f64::ceil(value) as i32`. The
/// port is therefore bit-exact on arm64 only. Not built or checked on x86_64, where
/// the C++ conversion is expected to use `cvttss2si`, which returns `i32::MIN` for NaN
/// and out-of-range input; the port saturates there [INFERENCE].
pub fn ceil(value: f32) -> i32 {
    f64::from(value).ceil() as i32
}

/// `appMemcmp`: `memcmp(Buf1, Buf2, Count)`. The sign of the result is the host's.
///
/// # Safety
/// Both buffers must be readable for `count` bytes. A negative `count` is passed to
/// libc as a huge `size_t`, as the C++ implicit conversion does; zero reads nothing.
pub unsafe fn memcmp(buf1: *const c_void, buf2: *const c_void, count: i32) -> i32 {
    unsafe { libc::memcmp(buf1, buf2, count as usize) }
}

/// `appMemIsZero`: 1 when the first `count` bytes are all zero, else 0. A non-positive
/// `count` yields 1 without reading memory.
///
/// # Safety
/// `v` must be readable for `count` bytes when `count` is positive. A NULL `v` with
/// positive `count` faults, as the C++ loop does.
pub unsafe fn mem_is_zero(v: *const c_void, count: i32) -> i32 {
    let mut b = v.cast::<u8>();
    let mut remaining = count;
    while remaining > 0 {
        if unsafe { *b } != 0 {
            return 0;
        }
        b = unsafe { b.add(1) };
        remaining -= 1;
    }
    1
}

/// `appMemmove`: `memmove(Dest, Src, Count)`.
///
/// # Safety
/// Both regions must be valid for `count` bytes; they may overlap.
pub unsafe fn memmove(dest: *mut c_void, src: *const c_void, count: i32) -> *mut c_void {
    unsafe { libc::memmove(dest, src, count as usize) }
}

/// `appMemset`: `memset(Dest, C, Count)`.
///
/// # Safety
/// `dest` must be writable for `count` bytes.
pub unsafe fn memset(dest: *mut c_void, c: i32, count: i32) {
    unsafe {
        libc::memset(dest, c, count as usize);
    }
}

/// `appMemcpy`: `memcpy(Dest, Src, Count)`.
///
/// # Safety
/// `dest` must be writable and `src` readable for `count` bytes, without overlap.
pub unsafe fn memcpy(dest: *mut c_void, src: *const c_void, count: i32) {
    unsafe {
        libc::memcpy(dest, src, count as usize);
    }
}

/// `appGetGMTRef`'s offset: `diff = (ltime - gtime) / 3600`. The division is on
/// `time_t` (`long`) and truncates toward zero, then converts to `FLOAT`.
pub fn gmt_offset_hours(ltime: c_long, gtime: c_long) -> f32 {
    ((ltime - gtime) / 3600) as f32
}

/// `appSprintf(GMTRef, diff>0 ? "+%1.1f" : "%1.1f", diff)`. `diff` is always whole
/// hours, so `%1.1f` prints one decimal place of zero, as `{:.1}` does.
pub fn format_gmt_ref(diff: f32) -> String {
    if diff > 0.0 {
        format!("+{diff:.1}")
    } else {
        format!("{diff:.1}")
    }
}

/// `appGetGMTRef`'s body on the non-PSX path. Writes the NUL-terminated text into
/// `dest`, the static 1024-`TCHAR` buffer from `appStaticString1024()`.
///
/// # Safety
/// `dest` must be writable for the text plus its NUL. For a valid zone the offset is
/// within ±14 hours, so the text is at most 6 `Tchar`s. A failed `mktime` (-1) gives a
/// longer text, still far below the caller's 1024-entry static buffer
/// (`appStaticString1024()`), which is the buffer the C++ passes.
pub unsafe fn gmt_ref(dest: *mut Tchar) {
    let mut ltime: c_long = 0;
    unsafe {
        libc::time(&mut ltime);
    }
    let newtime = unsafe { libc::gmtime(&ltime) };
    let gtime = unsafe { libc::mktime(newtime) };
    let text = format_gmt_ref(gmt_offset_hours(ltime, gtime));
    for (i, byte) in text.bytes().enumerate() {
        unsafe {
            *dest.add(i) = Tchar::from(byte);
        }
    }
    unsafe {
        *dest.add(text.len()) = 0;
    }
}
