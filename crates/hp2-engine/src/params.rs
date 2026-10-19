//! Rust port of `HarryPotter2/Unreal/Engine/Src/UnParams.cpp` (pinned `5782e7f`):
//! `GetSUBSTRING`, `GetFVECTOR` (stream and match forms) and `GetFROTATOR`
//! (stream and match forms).
//!
//! `Parse`, `appAtof`, `appStrchr`, `appStrfind`, `appStrlen` and `appStrncpy`
//! stay in Core. They are reached through [`ParamsServices`], filled in by
//! `Hp2ParamsServices.cpp`.
//!
//! Divergences, all intentional:
//! - `guard`/`unguard` are debugging context only. They stay in the C++ shells.
//! - `appRound` is `static_cast<INT>(f)`. Out-of-range is UB in C++. On arm64 the
//!   `fcvtzs` instruction saturates and NaN becomes 0, which is what `as i32`
//!   does, so the fixture matches the -O3 object on this target.
//! - `TCHAR` is `wchar_t`: 4 bytes on macOS and Linux, 2 bytes on Windows. [`TChar`]
//!   mirrors that width. The Rust side only reads and compares units, so the
//!   signedness of `wchar_t` does not matter.
//! - The C++ `TCHAR Temp[80]` is uninitialised. `appStrncpy` always writes all 80
//!   units (`wcsncpy` pads, and the last unit is forced to 0), so the zeroed Rust
//!   buffer is never observed.
//! - FMA: `UnParams.o` (-O3 -g) has no fused multiply-add sites. objdump shows 6
//!   `fmul` and 0 `fmadd`/`fmsub`/`fnmadd`/`fnmsub`, so every float op here is plain.

use hp2_abi::FVector;
use hp2_abi::engine::generated::FRotator;

use crate::math::app_round;

/// `TCHAR` as the C++ side sees it (`wchar_t` under `UNICODE=1`).
#[cfg(windows)]
pub type TChar = u16;
/// `TCHAR` as the C++ side sees it (`wchar_t` under `UNICODE=1`).
#[cfg(not(windows))]
pub type TChar = i32;

/// C++ services (`Hp2ParamsServices` in `Hp2ParamsServices.cpp`), same field order.
/// Each entry is a Core call the port cannot own.
#[repr(C)]
pub struct ParamsServices {
    /// `UBOOL Parse(Stream, Match, FLOAT& Value)`. Writes `value` only on success.
    pub parse_float:
        unsafe extern "C" fn(stream: *const TChar, matchp: *const TChar, value: *mut f32) -> i32,
    /// `FLOAT appAtof(const TCHAR* Str)`.
    pub app_atof: unsafe extern "C" fn(s: *const TChar) -> f32,
    /// `TCHAR* appStrchr(const TCHAR* String, INT c)`.
    pub app_strchr: unsafe extern "C" fn(s: *const TChar, c: i32) -> *mut TChar,
    /// `const TCHAR* appStrfind(const TCHAR* Str, const TCHAR* Find)`.
    pub app_strfind: unsafe extern "C" fn(s: *const TChar, find: *const TChar) -> *const TChar,
    /// `INT appStrlen(const TCHAR* String)`.
    pub app_strlen: unsafe extern "C" fn(s: *const TChar) -> i32,
    /// `TCHAR* appStrncpy(TCHAR* Dest, const TCHAR* Src, INT MaxLen)`. The return value is unused.
    pub app_strncpy: unsafe extern "C" fn(dest: *mut TChar, src: *const TChar, max_len: i32),
}

const fn tc(b: u8) -> TChar {
    b as TChar
}

/// NUL-terminated `TCHAR` literal, equivalent to `TEXT("...")`.
const fn wide<const N: usize>(s: &str) -> [TChar; N] {
    let b = s.as_bytes();
    assert!(b.len() < N);
    let mut out = [0 as TChar; N];
    let mut i = 0;
    while i < b.len() {
        out[i] = b[i] as TChar;
        i += 1;
    }
    out
}

static MATCH_X: [TChar; 3] = wide("X=");
static MATCH_Y: [TChar; 3] = wide("Y=");
static MATCH_Z: [TChar; 3] = wide("Z=");
static MATCH_PITCH: [TChar; 7] = wide("PITCH=");
static MATCH_YAW: [TChar; 5] = wide("YAW=");
static MATCH_ROLL: [TChar; 6] = wide("ROLL=");

/// `GetSUBSTRING(Stream, Match, Value, MaxLen)` (UnParams.cpp:55-81).
///
/// # Safety
/// `stream` and `matchp` must be NUL-terminated. `value` must have room for
/// `max_len` units.
pub unsafe fn get_substring(
    svc: &ParamsServices,
    stream: *const TChar,
    matchp: *const TChar,
    value: *mut TChar,
    max_len: i32,
) -> i32 {
    // SAFETY: the services return pointers into the NUL-terminated inputs.
    unsafe {
        let found = (svc.app_strfind)(stream, matchp);
        if found.is_null() {
            return 0;
        }
        let start = found.add((svc.app_strlen)(matchp) as usize);
        if *start != tc(b'(') {
            return 0;
        }
        (svc.app_strncpy)(value, start.add(1), max_len);
        let temp = (svc.app_strchr)(value, i32::from(b')'));
        if !temp.is_null() {
            *temp = 0;
        }
    }
    1
}

/// `GetFVECTOR(const TCHAR* Stream, FVector& Value)` (UnParams.cpp:25-50).
///
/// # Safety
/// `stream` must be NUL-terminated.
pub unsafe fn get_fvector_stream(
    svc: &ParamsServices,
    stream: *const TChar,
    value: &mut FVector,
) -> i32 {
    let mut num_vects = 0;
    // Old format.
    // SAFETY: `stream` is NUL-terminated; the match literals are static.
    unsafe {
        num_vects += (svc.parse_float)(stream, MATCH_X.as_ptr(), &mut value.x);
        num_vects += (svc.parse_float)(stream, MATCH_Y.as_ptr(), &mut value.y);
        num_vects += (svc.parse_float)(stream, MATCH_Z.as_ptr(), &mut value.z);
    }

    // New format: "x,y,z". Partial writes before a missing comma are kept.
    if num_vects == 0 {
        // SAFETY: each pointer is one past a comma that `appStrchr` located inside `stream`.
        unsafe {
            value.x = (svc.app_atof)(stream);
            let mut s = (svc.app_strchr)(stream, i32::from(b',')).cast_const();
            if s.is_null() {
                return 0;
            }
            s = s.add(1);
            value.y = (svc.app_atof)(s);
            s = (svc.app_strchr)(s, i32::from(b',')).cast_const();
            if s.is_null() {
                return 0;
            }
            s = s.add(1);
            value.z = (svc.app_atof)(s);
        }
        num_vects = 3;
    }
    i32::from(num_vects == 3)
}

/// `GetFVECTOR(const TCHAR* Stream, const TCHAR* Match, FVector& Value)`
/// (UnParams.cpp:86-100).
///
/// # Safety
/// `stream` and `matchp` must be NUL-terminated.
pub unsafe fn get_fvector(
    svc: &ParamsServices,
    stream: *const TChar,
    matchp: *const TChar,
    value: &mut FVector,
) -> i32 {
    let mut temp = [0 as TChar; 80];
    // SAFETY: `temp` has 80 units, as `GetSUBSTRING` requires.
    if unsafe { get_substring(svc, stream, matchp, temp.as_mut_ptr(), 80) } == 0 {
        return 0;
    }
    // SAFETY: `temp` is NUL-terminated by `appStrncpy` and `appStrchr`.
    unsafe { get_fvector_stream(svc, temp.as_ptr(), value) }
}

/// `GetFROTATOR(const TCHAR* Stream, FRotator& Rotation, INT ScaleFactor)`
/// (UnParams.cpp:105-140).
///
/// # Safety
/// `stream` must be NUL-terminated.
pub unsafe fn get_frotator_stream(
    svc: &ParamsServices,
    stream: *const TChar,
    rotation: &mut FRotator,
    scale_factor: i32,
) -> i32 {
    // `Temp * ScaleFactor` converts the INT to FLOAT, then multiplies.
    let scale = scale_factor as f32;
    let mut temp: f32 = 0.0;
    let mut n = 0;

    // Old format.
    // SAFETY: `stream` is NUL-terminated; the match literals are static.
    unsafe {
        if (svc.parse_float)(stream, MATCH_PITCH.as_ptr(), &mut temp) != 0 {
            rotation.pitch = app_round(temp * scale);
            n += 1;
        }
        if (svc.parse_float)(stream, MATCH_YAW.as_ptr(), &mut temp) != 0 {
            rotation.yaw = app_round(temp * scale);
            n += 1;
        }
        if (svc.parse_float)(stream, MATCH_ROLL.as_ptr(), &mut temp) != 0 {
            rotation.roll = app_round(temp * scale);
            n += 1;
        }
    }

    // New format: "pitch,yaw,roll". Partial writes before a missing comma are kept.
    if n == 0 {
        // SAFETY: each pointer is one past a comma that `appStrchr` located inside `stream`.
        unsafe {
            rotation.pitch = app_round((svc.app_atof)(stream) * scale);
            let mut s = (svc.app_strchr)(stream, i32::from(b',')).cast_const();
            if s.is_null() {
                return 0;
            }
            s = s.add(1);
            rotation.yaw = app_round((svc.app_atof)(s) * scale);
            s = (svc.app_strchr)(s, i32::from(b',')).cast_const();
            if s.is_null() {
                return 0;
            }
            s = s.add(1);
            rotation.roll = app_round((svc.app_atof)(s) * scale);
        }
        return 1;
    }
    0
}

/// `GetFROTATOR(const TCHAR* Stream, const TCHAR* Match, FRotator& Value, INT ScaleFactor)`
/// (UnParams.cpp:145-160).
///
/// # Safety
/// `stream` and `matchp` must be NUL-terminated.
pub unsafe fn get_frotator(
    svc: &ParamsServices,
    stream: *const TChar,
    matchp: *const TChar,
    rotation: &mut FRotator,
    scale_factor: i32,
) -> i32 {
    let mut temp = [0 as TChar; 80];
    // SAFETY: `temp` has 80 units, as `GetSUBSTRING` requires.
    if unsafe { get_substring(svc, stream, matchp, temp.as_mut_ptr(), 80) } == 0 {
        return 0;
    }
    // SAFETY: `temp` is NUL-terminated by `appStrncpy` and `appStrchr`.
    unsafe { get_frotator_stream(svc, temp.as_ptr(), rotation, scale_factor) }
}
