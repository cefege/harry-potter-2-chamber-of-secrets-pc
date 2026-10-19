//! Replays the original-C++ oracle from `Build/params_fixture_harness/` against
//! `hp2_engine::params` (`GetSUBSTRING`, `GetFVECTOR`, `GetFROTATOR`, both forms).
//!
//! `fixtures/params_getters.txt` holds 2024 records. Each record stores the exact
//! inputs, then the expected return value and outputs. FVector and FRotator outputs
//! are compared by bit pattern. The Core helpers the port reaches through
//! `ParamsServices` (`Parse`, `appAtof`, `appStrfind`, `appStrlen`, `appStrchr`,
//! `appStrncpy`) are re-implemented here as verbatim ports of the harness copies,
//! with libc `atof` for the float conversion.

// `TChar` is `i32` here but `u16` on Windows, so the `as i32` casts are not no-ops there.
#![allow(clippy::unnecessary_cast)]

use std::ffi::c_char;

use hp2_abi::FVector;
use hp2_abi::engine::generated::FRotator;
use hp2_engine::params::{
    ParamsServices, TChar, get_frotator, get_frotator_stream, get_fvector, get_fvector_stream,
    get_substring,
};

const FIXTURE: &str = include_str!("fixtures/params_getters.txt");
const EXPECTED_RECORDS: usize = 2024;
const EXPECTED_FVEC_STREAM: usize = 407;
const EXPECTED_FVEC_MATCH: usize = 404;
const EXPECTED_SUBSTRING: usize = 403;
const EXPECTED_FROT_STREAM: usize = 407;
const EXPECTED_FROT_MATCH: usize = 403;

const SUBSTRING_PREFILL: TChar = 90; // 'Z'

unsafe extern "C" {
    fn atof(s: *const c_char) -> f64;
}

/// `appStrlen` over a NUL-terminated `TCHAR` string (wcslen).
unsafe fn tlen(s: *const TChar) -> i32 {
    let mut n = 0usize;
    unsafe {
        while *s.add(n) != 0 {
            n += 1;
        }
    }
    n as i32
}

#[inline]
fn lower(c: i32) -> i32 {
    if (65..=90).contains(&c) { c + 32 } else { c }
}

/// `appStrnicmp(A, B, Count)` == `wcsncasecmp`. Returns true when equal.
unsafe fn strnicmp(a: *const TChar, b: *const TChar, count: i32) -> bool {
    unsafe {
        for i in 0..count.max(0) as usize {
            let x = lower(*a.add(i) as i32);
            let y = lower(*b.add(i) as i32);
            if x != y {
                return false;
            }
            if x == 0 {
                break;
            }
        }
    }
    true
}

unsafe extern "C" fn app_strlen(s: *const TChar) -> i32 {
    unsafe { tlen(s) }
}

unsafe extern "C" fn app_strchr(s: *const TChar, c: i32) -> *mut TChar {
    unsafe {
        let mut p = s;
        loop {
            if *p as i32 == c {
                return p as *mut TChar;
            }
            if *p == 0 {
                return std::ptr::null_mut();
            }
            p = p.add(1);
        }
    }
}

unsafe extern "C" fn app_strncpy(dest: *mut TChar, src: *const TChar, max_len: i32) {
    unsafe {
        let n = max_len as usize;
        let mut ended = false;
        for i in 0..n {
            let c = if ended {
                0
            } else {
                let v = *src.add(i);
                if v == 0 {
                    ended = true;
                }
                v
            };
            *dest.add(i) = c;
        }
        *dest.add(n - 1) = 0;
    }
}

/// `const TCHAR* appStrfind(Str, Find)` (params_harness.cpp, verbatim port).
unsafe extern "C" fn app_strfind(s: *const TChar, find: *const TChar) -> *const TChar {
    unsafe {
        let mut alnum = false;
        let fv = *find as i32;
        let f = if !(97..=122).contains(&fv) {
            fv
        } else {
            fv - 32
        };
        let length = tlen(find) - 1;
        let find1 = find.add(1);
        let mut sp = s.add(1);
        let mut c = *s as i32;
        while c != 0 {
            if (97..=122).contains(&c) {
                c -= 32;
            }
            if !alnum && c == f && strnicmp(sp, find1, length) {
                return sp.sub(1);
            }
            alnum = (65..=90).contains(&c) || (48..=57).contains(&c);
            c = *sp as i32;
            sp = sp.add(1);
        }
        std::ptr::null()
    }
}

/// `FLOAT appAtof(const TCHAR* Str)` = `atof(appToAnsi(Str))`. Inputs are ASCII.
unsafe extern "C" fn app_atof(s: *const TChar) -> f32 {
    unsafe {
        let mut buf = [0u8; 1024];
        let mut n = 0usize;
        while n < 1023 && *s.add(n) != 0 {
            buf[n] = *s.add(n) as u8;
            n += 1;
        }
        buf[n] = 0;
        atof(buf.as_ptr() as *const c_char) as f32
    }
}

/// `UBOOL Parse(Stream, Match, FLOAT& Value)` (params_harness.cpp, verbatim port).
unsafe extern "C" fn parse_float(
    stream: *const TChar,
    matchp: *const TChar,
    value: *mut f32,
) -> i32 {
    unsafe {
        let temp = app_strfind(stream, matchp);
        if temp.is_null() {
            return 0;
        }
        *value = app_atof(temp.add(tlen(matchp) as usize));
        1
    }
}

static SERVICES: ParamsServices = ParamsServices {
    parse_float,
    app_atof,
    app_strchr,
    app_strfind,
    app_strlen,
    app_strncpy,
};

/// `toTChar` in the harness: `(TCHAR)(unsigned char)s[i]`, truncated to `cap - 1` units.
fn to_tchars(s: &str, cap: usize) -> Vec<TChar> {
    let mut v: Vec<TChar> = s.bytes().take(cap - 1).map(|b| b as TChar).collect();
    v.push(0);
    v
}

fn sentinel_vec() -> FVector {
    FVector {
        x: 1.25,
        y: -2.5,
        z: 3.75,
    }
}

fn sentinel_rot() -> FRotator {
    FRotator {
        pitch: 111,
        yaw: -222,
        roll: 333,
    }
}

fn bits(s: &str) -> u32 {
    u32::from_str_radix(s, 16).expect("hex bits")
}

#[test]
fn params_getters_replay_matches_original() {
    let mut counts = [0usize; 5];
    let mut total = 0usize;

    for line in FIXTURE.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        total += 1;
        match f[0] {
            "fvec_stream" => {
                counts[0] += 1;
                assert_eq!(f.len(), 8, "{line}");
                let stream = to_tchars(f[1], 1024);
                let mut v = sentinel_vec();
                let ret = unsafe { get_fvector_stream(&SERVICES, stream.as_ptr(), &mut v) };
                assert_eq!(ret.to_string(), f[4], "{line}");
                assert_eq!(v.x.to_bits(), bits(f[5]), "{line}");
                assert_eq!(v.y.to_bits(), bits(f[6]), "{line}");
                assert_eq!(v.z.to_bits(), bits(f[7]), "{line}");
            }
            "fvec_match" => {
                counts[1] += 1;
                assert_eq!(f.len(), 8, "{line}");
                let stream = to_tchars(f[1], 1024);
                let matchp = to_tchars(f[2], 64);
                let mut v = sentinel_vec();
                let ret =
                    unsafe { get_fvector(&SERVICES, stream.as_ptr(), matchp.as_ptr(), &mut v) };
                assert_eq!(ret.to_string(), f[4], "{line}");
                assert_eq!(v.x.to_bits(), bits(f[5]), "{line}");
                assert_eq!(v.y.to_bits(), bits(f[6]), "{line}");
                assert_eq!(v.z.to_bits(), bits(f[7]), "{line}");
            }
            "substring" => {
                counts[2] += 1;
                assert_eq!(f.len(), 6, "{line}");
                let stream = to_tchars(f[1], 1024);
                let matchp = to_tchars(f[2], 64);
                let max_len: i32 = f[3].parse().expect("maxLen");
                let mut buf = [SUBSTRING_PREFILL; 80];
                let ret = unsafe {
                    get_substring(
                        &SERVICES,
                        stream.as_ptr(),
                        matchp.as_ptr(),
                        buf.as_mut_ptr(),
                        max_len,
                    )
                };
                assert_eq!(ret.to_string(), f[4], "{line}");
                let expected: Vec<i32> = f[5].split(',').map(|x| x.parse().unwrap()).collect();
                assert_eq!(expected.len(), max_len as usize, "{line}");
                for (i, e) in expected.iter().enumerate() {
                    assert_eq!(buf[i] as i32, *e, "{line} unit {i}");
                }
            }
            "frot_stream" => {
                counts[3] += 1;
                assert_eq!(f.len(), 8, "{line}");
                let stream = to_tchars(f[1], 1024);
                let scale: i32 = f[3].parse().unwrap();
                let mut r = sentinel_rot();
                let ret = unsafe { get_frotator_stream(&SERVICES, stream.as_ptr(), &mut r, scale) };
                assert_eq!(ret.to_string(), f[4], "{line}");
                assert_eq!(r.pitch.to_string(), f[5], "{line}");
                assert_eq!(r.yaw.to_string(), f[6], "{line}");
                assert_eq!(r.roll.to_string(), f[7], "{line}");
            }
            "frot_match" => {
                counts[4] += 1;
                assert_eq!(f.len(), 8, "{line}");
                let stream = to_tchars(f[1], 1024);
                let matchp = to_tchars(f[2], 64);
                let scale: i32 = f[3].parse().unwrap();
                let mut r = sentinel_rot();
                let ret = unsafe {
                    get_frotator(&SERVICES, stream.as_ptr(), matchp.as_ptr(), &mut r, scale)
                };
                assert_eq!(ret.to_string(), f[4], "{line}");
                assert_eq!(r.pitch.to_string(), f[5], "{line}");
                assert_eq!(r.yaw.to_string(), f[6], "{line}");
                assert_eq!(r.roll.to_string(), f[7], "{line}");
            }
            other => panic!("unknown kind {other}"),
        }
    }

    assert_eq!(total, EXPECTED_RECORDS, "total record count");
    assert_eq!(counts[0], EXPECTED_FVEC_STREAM, "fvec_stream count");
    assert_eq!(counts[1], EXPECTED_FVEC_MATCH, "fvec_match count");
    assert_eq!(counts[2], EXPECTED_SUBSTRING, "substring count");
    assert_eq!(counts[3], EXPECTED_FROT_STREAM, "frot_stream count");
    assert_eq!(counts[4], EXPECTED_FROT_MATCH, "frot_match count");
}
