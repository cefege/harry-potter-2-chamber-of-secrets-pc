//! Bit-exact replay of the `UnAnsi.cpp` math and memory oracle fixture
//! (`Build/ansi_math_fixture_harness/gen_ansi_math.sh`).
//!
//! Every `case|` record carries its inputs. The test parses them, drives
//! `hp2_core::ansi_math` record by record, and re-encodes each result in the
//! harness's format. The emitted text must equal the fixture line for line.
//! Per-op record counts are asserted exactly so a silently shrunken fixture fails.
//!
//! Compiled on macOS only. The fixture was recorded on macOS arm64: libm results,
//! `gmtime`/`mktime` and the POSIX `TZ` handling are platform specific.
#![cfg(target_os = "macos")]

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::str::FromStr;

use hp2_core::ansi::Tchar;
use hp2_core::ansi_math;

const FIXTURE: &str = include_str!("fixtures/ansi_math_unansi.txt");

const EXPECTED_TOTAL: usize = 207;
const EXPECTED_COUNTS: [(&str, usize); 19] = [
    ("exp", 15),
    ("loge", 15),
    ("sin", 15),
    ("cos", 15),
    ("acos", 15),
    ("tan", 15),
    ("atan", 15),
    ("sqrt", 15),
    ("is_nan", 15),
    ("ceil", 15),
    ("fmod", 9),
    ("atan2", 9),
    ("pow", 9),
    ("memcmp", 6),
    ("mem_is_zero", 5),
    ("memmove", 6),
    ("memset", 4),
    ("memcpy", 4),
    ("gmt_ref", 5),
];

type Unary = fn(f64) -> f64;
type Binary = fn(f64, f64) -> f64;

const UNARY: [(&str, Unary); 8] = [
    ("exp", ansi_math::exp),
    ("loge", ansi_math::loge),
    ("sin", ansi_math::sin),
    ("cos", ansi_math::cos),
    ("acos", ansi_math::acos),
    ("tan", ansi_math::tan),
    ("atan", ansi_math::atan),
    ("sqrt", ansi_math::sqrt),
];

const BINARY: [(&str, Binary); 3] = [
    ("fmod", ansi_math::fmod),
    ("atan2", ansi_math::atan2),
    ("pow", ansi_math::pow),
];

/// Value of `key=` among the `|`-separated fields of a record.
fn field<'a>(fields: &[&'a str], key: &str) -> &'a str {
    fields
        .iter()
        .find_map(|f| f.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
        .unwrap_or_else(|| panic!("missing field `{key}`"))
}

fn num<T: FromStr>(s: &str) -> T
where
    T::Err: Debug,
{
    s.parse().expect("decimal field")
}

fn hex_f64(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex f64"))
}

fn hex_f32(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).expect("hex f32"))
}

fn encode_f64(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn encode_f32(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// Buffer bytes as lowercase hex pairs; an empty buffer is `empty`.
fn encode_bytes(b: &[u8]) -> String {
    if b.is_empty() {
        return "empty".to_owned();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn decode_bytes(s: &str) -> Vec<u8> {
    if s == "empty" {
        return Vec::new();
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ascii"), 16).expect("hex byte")
        })
        .collect()
}

/// Parses one fixture record, runs the Rust port, and re-encodes the record.
fn replay(line: &str) -> String {
    let fields: Vec<&str> = line.split('|').collect();
    let op = fields[1];

    if let Some((_, f)) = UNARY.iter().find(|(name, _)| *name == op) {
        let input = hex_f64(field(&fields, "in"));
        return format!(
            "case|{op}|in={}|out={}",
            encode_f64(input),
            encode_f64(f(input))
        );
    }
    if let Some((_, f)) = BINARY.iter().find(|(name, _)| *name == op) {
        let a = hex_f64(field(&fields, "a"));
        let b = hex_f64(field(&fields, "b"));
        return format!(
            "case|{op}|a={}|b={}|out={}",
            encode_f64(a),
            encode_f64(b),
            encode_f64(f(a, b))
        );
    }

    match op {
        "is_nan" => {
            let input = hex_f64(field(&fields, "in"));
            format!(
                "case|is_nan|in={}|out={}",
                encode_f64(input),
                ansi_math::is_nan(input)
            )
        }
        "ceil" => {
            let input = hex_f32(field(&fields, "in"));
            format!(
                "case|ceil|in={}|out={}",
                encode_f32(input),
                ansi_math::ceil(input)
            )
        }
        "memcmp" => {
            let a = decode_bytes(field(&fields, "a"));
            let b = decode_bytes(field(&fields, "b"));
            let n = num::<i32>(field(&fields, "n"));
            // SAFETY: both buffers hold 16 bytes and `n` is at most 16 in every record.
            let out = unsafe { ansi_math::memcmp(a.as_ptr().cast(), b.as_ptr().cast(), n) };
            format!(
                "case|memcmp|a={}|b={}|n={n}|out={out}",
                encode_bytes(&a),
                encode_bytes(&b)
            )
        }
        "mem_is_zero" => {
            let v = decode_bytes(field(&fields, "v"));
            let n = num::<i32>(field(&fields, "n"));
            // SAFETY: `v` holds 16 bytes and `n` is at most 16 in every record.
            let out = unsafe { ansi_math::mem_is_zero(v.as_ptr().cast(), n) };
            format!("case|mem_is_zero|v={}|n={n}|out={out}", encode_bytes(&v))
        }
        "memmove" => {
            let before = decode_bytes(field(&fields, "before"));
            let dst = num::<usize>(field(&fields, "dst"));
            let src = num::<usize>(field(&fields, "src"));
            let n = num::<i32>(field(&fields, "n"));
            let mut buf = before.clone();
            let base = buf.as_mut_ptr();
            // SAFETY: `dst`, `src` and `n` keep both regions inside the 16-byte buffer.
            let ret = unsafe {
                ansi_math::memmove(base.add(dst).cast(), base.add(src).cast_const().cast(), n)
            };
            let ret_off = (ret as usize) - (base as usize);
            format!(
                "case|memmove|before={}|dst={dst}|src={src}|n={n}|after={}|ret={ret_off}",
                encode_bytes(&before),
                encode_bytes(&buf)
            )
        }
        "memset" => {
            let before = decode_bytes(field(&fields, "before"));
            let dst = num::<usize>(field(&fields, "dst"));
            let c = num::<i32>(field(&fields, "c"));
            let n = num::<i32>(field(&fields, "n"));
            let mut buf = before.clone();
            let base = buf.as_mut_ptr();
            // SAFETY: `dst` and `n` keep the write inside the 16-byte buffer.
            unsafe { ansi_math::memset(base.add(dst).cast(), c, n) };
            format!(
                "case|memset|before={}|dst={dst}|c={c}|n={n}|after={}",
                encode_bytes(&before),
                encode_bytes(&buf)
            )
        }
        "memcpy" => {
            let dst_before = decode_bytes(field(&fields, "dst_before"));
            let src = decode_bytes(field(&fields, "src"));
            let dst = num::<usize>(field(&fields, "dst"));
            let src_off = num::<usize>(field(&fields, "src_off"));
            let n = num::<i32>(field(&fields, "n"));
            let mut dst_buf = dst_before.clone();
            let dst_base = dst_buf.as_mut_ptr();
            let src_base = src.as_ptr();
            // SAFETY: offsets and `n` keep both 16-byte buffers in range; the regions do not overlap.
            unsafe {
                ansi_math::memcpy(dst_base.add(dst).cast(), src_base.add(src_off).cast(), n);
            }
            format!(
                "case|memcpy|dst_before={}|src={}|dst={dst}|src_off={src_off}|n={n}|dst_after={}",
                encode_bytes(&dst_before),
                encode_bytes(&src),
                encode_bytes(&dst_buf)
            )
        }
        "gmt_ref" => {
            let tz = field(&fields, "tz");
            unsafe extern "C" {
                fn tzset();
            }
            // The test binary runs this on one thread only: it is the sole test in the file.
            unsafe {
                std::env::set_var("TZ", tz);
                tzset();
            }
            let mut buf: Vec<Tchar> = vec![0; 1024];
            // SAFETY: `buf` has the 1024 writable Tchar slots the port's contract requires.
            unsafe { ansi_math::gmt_ref(buf.as_mut_ptr()) };
            let text: String = buf
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| char::from_u32(c as u32).expect("ascii text"))
                .collect();
            format!("case|gmt_ref|tz={tz}|ret={text}")
        }
        other => panic!("unknown op `{other}`"),
    }
}

#[test]
fn ansi_math_replays_fixture_record_by_record() {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;
    for (index, line) in FIXTURE.lines().enumerate() {
        assert!(
            line.starts_with("case|"),
            "fixture line {} is not a case record",
            index + 1
        );
        let op = line.split('|').nth(1).expect("op field");
        *counts.entry(op.to_owned()).or_default() += 1;
        total += 1;
        assert_eq!(replay(line), line, "fixture line {}", index + 1);
    }

    assert_eq!(total, EXPECTED_TOTAL, "total case records");
    assert_eq!(counts.len(), EXPECTED_COUNTS.len(), "distinct ops");
    for (op, expected) in EXPECTED_COUNTS {
        assert_eq!(
            counts.get(op).copied().unwrap_or(0),
            expected,
            "record count for `{op}`"
        );
    }
}
