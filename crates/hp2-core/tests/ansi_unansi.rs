//! Bit-exact replay of the `UnAnsi.cpp` oracle fixture (`Build/ansi_fixture_harness/gen_ansi.sh`).
//!
//! Every `case|` record carries its inputs. The test rebuilds the harness's buffers,
//! drives the Rust port with recording services in place of the C++ callbacks, and
//! re-encodes the results plus the hook lines in the harness's format. The emitted
//! text must equal the fixture line for line. Record counts are asserted exactly so a
//! silently shrunken fixture fails the test.
//!
//! Compiled on macOS only. The fixture is a macOS recording: for example `strncmp` of
//! `"ab"` with `n = 5` returns 99 from Apple's libc. Other hosts need their own record.
#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_void};
use std::ptr::null_mut;

use hp2_core::ansi::{self, Hp2AnsiServices, Threw};

const FIXTURE: &str = include_str!("fixtures/ansi_unansi.txt");

/// Harness buffer capacity (`HarnessCap`) in characters.
const CAP: usize = 16;
/// `__FILE__` of the original `UnAnsi.cpp` and its two `check()` lines (2213, 2229).
const FILE: &[u8] = b"UnAnsi.cpp\0";
const ATOI_CHECK_LINE: i32 = 2213;
const STRTOI_CHECK_LINE: i32 = 2229;

const EXPECTED_COUNTS: [(&str, usize); 18] = [
    ("strncpy", 6),
    ("strncat", 4),
    ("strlen", 5),
    ("strstr", 5),
    ("strchr", 4),
    ("strcat", 3),
    ("strcmp", 6),
    ("stricmp", 5),
    ("strcpy", 3),
    ("strupr", 3),
    ("strncmp", 4),
    ("strnicmp", 4),
    ("atoi", 11),
    ("atof", 10),
    ("strtoi", 11),
    ("to_utf8", 9),
    ("from_utf8", 12),
    ("checked_int_size", 5),
];
const EXPECTED_CASES: usize = 110;
/// Records whose C++ original throws: three atoi overflows, one strtoi overflow,
/// two checked_int_size overflows.
const EXPECTED_THREW: usize = 6;

thread_local! {
    static HOOKS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static ANSI_OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn hook(line: String) {
    HOOKS.with(|h| h.borrow_mut().push(format!("hook|{line}")));
}

/// `fail_assert` hook: records the check text, then reports that the service raised.
unsafe extern "C" fn fail_assert(expr: *const c_void, file: *const c_void, line: i32) -> i32 {
    let expr = unsafe { CStr::from_ptr(expr as *const c_char) }.to_string_lossy();
    let file = unsafe { CStr::from_ptr(file as *const c_char) }.to_string_lossy();
    hook(format!("fail_assert|expr={expr}|file={file}|line={line}"));
    1
}

/// `appToAnsi` stand-in: `ToAnsi` per character (0..0x100 maps to the byte, anything
/// else to `MAXSBYTE`), capped at 1023 characters, NUL-terminated. NULL yields NULL
/// with no hook, as the harness does.
unsafe extern "C" fn to_ansi(s: *const c_void, out: *mut *const c_void) -> i32 {
    let s = s as *const i32;
    if s.is_null() {
        unsafe { *out = std::ptr::null() };
        return 0;
    }
    let wide = unsafe { wide_until_nul(s) };
    hook(format!("to_ansi|in={}", enc_wide(Some(&wide))));
    let mut bytes: Vec<u8> = wide
        .iter()
        .take(1023)
        .map(|&c| {
            if (0..0x100).contains(&c) {
                c as u8
            } else {
                0x7f
            }
        })
        .collect();
    bytes.push(0);
    let ptr = ANSI_OUT.with(|b| {
        let mut b = b.borrow_mut();
        *b = bytes;
        b.as_ptr()
    });
    unsafe { *out = ptr as *const c_void };
    0
}

/// `appErrorf(TEXT("Size exceeds the signed 32-bit API contract"))` stand-in: the
/// `GError` device records the message, then the call raises.
unsafe extern "C" fn size_exceeds_int_contract() -> i32 {
    let msg = "Size exceeds the signed 32-bit API contract";
    let wide: Vec<i32> = msg.chars().map(|c| c as i32).collect();
    hook(format!("errorf|msg={}", enc_wide(Some(&wide))));
    1
}

unsafe fn wide_until_nul(s: *const i32) -> Vec<i32> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let c = unsafe { *s.add(i) };
        if c == 0 {
            return out;
        }
        out.push(c);
        i += 1;
    }
}

/// Wide text as the harness prints it: `%x` code points joined by `.`; empty -> `empty`;
/// NULL -> `null`.
fn enc_wide(p: Option<&[i32]>) -> String {
    match p {
        None => "null".into(),
        Some([]) => "empty".into(),
        Some(s) => s
            .iter()
            .map(|c| format!("{:x}", *c as u32))
            .collect::<Vec<_>>()
            .join("."),
    }
}

/// ANSI bytes as concatenated lowercase hex pairs; empty -> `empty`; NULL -> `null`.
fn enc_ansi(p: Option<&[u8]>) -> String {
    match p {
        None => "null".into(),
        Some([]) => "empty".into(),
        Some(s) => s.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

fn dec_wide(s: &str) -> Option<Vec<i32>> {
    match s {
        "null" => None,
        "empty" => Some(Vec::new()),
        _ => Some(
            s.split('.')
                .map(|h| i32::from_str_radix(h, 16).expect("wide hex"))
                .collect(),
        ),
    }
}

fn dec_ansi(s: &str) -> Option<Vec<u8>> {
    match s {
        "null" => None,
        "empty" => Some(Vec::new()),
        _ => Some(
            s.as_bytes()
                .chunks(2)
                .map(|p| {
                    u8::from_str_radix(std::str::from_utf8(p).expect("ascii"), 16)
                        .expect("ansi hex")
                })
                .collect(),
        ),
    }
}

/// `InitWide`: 16 cells filled with 'X', `init` copied, NUL at `init.len()`.
fn init_wide(init: Option<&[i32]>) -> [i32; CAP] {
    let mut d = [i32::from(b'X'); CAP];
    let init = init.unwrap_or(&[]);
    d[..init.len()].copy_from_slice(init);
    d[init.len()] = 0;
    d
}

fn nul_terminated(s: &[i32]) -> Vec<i32> {
    let mut v = s.to_vec();
    v.push(0);
    v
}

fn ptr_of(buf: &Option<Vec<i32>>) -> *const i32 {
    buf.as_ref().map_or(std::ptr::null(), |v| v.as_ptr())
}

/// Pointer result as `null` or the element offset from `base`.
fn ret_off(p: *const i32, base: *const i32) -> String {
    if p.is_null() {
        "null".into()
    } else {
        format!("off:{}", (p as isize - base as isize) / 4)
    }
}

fn case(op: &str, fields: &[String], ret: &str, threw: u8) -> String {
    let mut line = format!("case|{op}");
    for f in fields {
        line.push('|');
        line.push_str(f);
    }
    line.push('|');
    line.push_str(ret);
    line.push_str(&format!("|threw={threw}"));
    line
}

/// Replays one `case|` record, returning the re-encoded case line. Hook lines the
/// services recorded are collected from `HOOKS` by the caller.
fn run_case(line: &str, svc: &Hp2AnsiServices) -> String {
    let mut parts = line.split('|');
    assert_eq!(parts.next(), Some("case"), "not a case record: {line}");
    let op = parts.next().expect("op");
    let f: BTreeMap<&str, &str> = parts
        .map(|p| {
            p.split_once('=')
                .unwrap_or_else(|| panic!("bad field {p} in {line}"))
        })
        .collect();
    let field = |k: &str| {
        *f.get(k)
            .unwrap_or_else(|| panic!("{op}: missing {k} in {line}"))
    };
    let int = |k: &str| field(k).parse::<i64>().expect("integer field");

    match op {
        "strncpy" => {
            let src = dec_wide(field("src"));
            let max = int("max") as i32;
            let src_buf = src.as_deref().map(nul_terminated);
            let mut d = init_wide(None);
            let ret = unsafe { ansi::strncpy(d.as_mut_ptr(), ptr_of(&src_buf), max) };
            case(
                op,
                &[
                    format!("src={}", enc_wide(src.as_deref())),
                    format!("max={max}"),
                ],
                &format!(
                    "ret={}|buf={}",
                    ret_off(ret, d.as_ptr()),
                    enc_wide(Some(&d[..]))
                ),
                0,
            )
        }
        "strncat" => {
            let dest = dec_wide(field("dest"));
            let src = dec_wide(field("src"));
            let max = int("max") as i32;
            let src_buf = src.as_deref().map(nul_terminated);
            let mut d = init_wide(dest.as_deref());
            let r = unsafe { ansi::strncat(svc, d.as_mut_ptr(), ptr_of(&src_buf), max) };
            let (ret, threw) = match r {
                Ok(p) => (format!("ret={}", ret_off(p, d.as_ptr())), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(
                op,
                &[
                    format!("dest={}", enc_wide(dest.as_deref())),
                    format!("src={}", enc_wide(src.as_deref())),
                    format!("max={max}"),
                ],
                &format!("{ret}|buf={}", enc_wide(Some(&d[..]))),
                threw,
            )
        }
        "strlen" => {
            let s = dec_wide(field("s"));
            let buf = s.as_deref().map(nul_terminated);
            let r = unsafe { ansi::strlen(svc, ptr_of(&buf)) };
            let (ret, threw) = match r {
                Ok(n) => (format!("ret={n}"), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(op, &[format!("s={}", enc_wide(s.as_deref()))], &ret, threw)
        }
        "strstr" => {
            let s = dec_wide(field("s"));
            let find = dec_wide(field("find"));
            let sb = s.as_deref().map(nul_terminated);
            let fb = find.as_deref().map(nul_terminated);
            let ret = unsafe { ansi::strstr(ptr_of(&sb), ptr_of(&fb)) };
            case(
                op,
                &[
                    format!("s={}", enc_wide(s.as_deref())),
                    format!("find={}", enc_wide(find.as_deref())),
                ],
                &format!("ret={}", ret_off(ret, ptr_of(&sb))),
                0,
            )
        }
        "strchr" => {
            let s = dec_wide(field("s"));
            let c = int("c") as i32;
            let sb = s.as_deref().map(nul_terminated);
            let ret = unsafe { ansi::strchr(ptr_of(&sb), c) };
            case(
                op,
                &[format!("s={}", enc_wide(s.as_deref())), format!("c={c}")],
                &format!("ret={}", ret_off(ret, ptr_of(&sb))),
                0,
            )
        }
        "strcat" => {
            let dest = dec_wide(field("dest"));
            let src = dec_wide(field("src"));
            let src_buf = src.as_deref().map(nul_terminated);
            let mut d = init_wide(dest.as_deref());
            let ret = unsafe { ansi::strcat(d.as_mut_ptr(), ptr_of(&src_buf)) };
            case(
                op,
                &[
                    format!("dest={}", enc_wide(dest.as_deref())),
                    format!("src={}", enc_wide(src.as_deref())),
                ],
                &format!(
                    "ret={}|buf={}",
                    ret_off(ret, d.as_ptr()),
                    enc_wide(Some(&d[..]))
                ),
                0,
            )
        }
        "strcmp" | "stricmp" => {
            let a = dec_wide(field("a"));
            let b = dec_wide(field("b"));
            let ab = a.as_deref().map(nul_terminated);
            let bb = b.as_deref().map(nul_terminated);
            let ret = unsafe {
                if op == "strcmp" {
                    ansi::strcmp(ptr_of(&ab), ptr_of(&bb))
                } else {
                    ansi::stricmp(ptr_of(&ab), ptr_of(&bb))
                }
            };
            case(
                op,
                &[
                    format!("a={}", enc_wide(a.as_deref())),
                    format!("b={}", enc_wide(b.as_deref())),
                ],
                &format!("ret={ret}"),
                0,
            )
        }
        "strcpy" => {
            let src = dec_wide(field("src"));
            let src_buf = src.as_deref().map(nul_terminated);
            let mut d = init_wide(None);
            let ret = unsafe { ansi::strcpy(d.as_mut_ptr(), ptr_of(&src_buf)) };
            case(
                op,
                &[format!("src={}", enc_wide(src.as_deref()))],
                &format!(
                    "ret={}|buf={}",
                    ret_off(ret, d.as_ptr()),
                    enc_wide(Some(&d[..]))
                ),
                0,
            )
        }
        "strupr" => {
            let s = dec_wide(field("s"));
            let mut d = init_wide(s.as_deref());
            let ret = unsafe { ansi::strupr(d.as_mut_ptr()) };
            case(
                op,
                &[format!("s={}", enc_wide(s.as_deref()))],
                &format!(
                    "ret={}|buf={}",
                    ret_off(ret, d.as_ptr()),
                    enc_wide(Some(&d[..]))
                ),
                0,
            )
        }
        "strncmp" | "strnicmp" => {
            let a = dec_wide(field("a"));
            let b = dec_wide(field("b"));
            let n = int("n") as i32;
            let ab = a.as_deref().map(nul_terminated);
            let bb = b.as_deref().map(nul_terminated);
            let ret = unsafe {
                if op == "strncmp" {
                    ansi::strncmp(ptr_of(&ab), ptr_of(&bb), n)
                } else {
                    ansi::strnicmp(ptr_of(&ab), ptr_of(&bb), n)
                }
            };
            case(
                op,
                &[
                    format!("a={}", enc_wide(a.as_deref())),
                    format!("b={}", enc_wide(b.as_deref())),
                    format!("n={n}"),
                ],
                &format!("ret={ret}"),
                0,
            )
        }
        "atoi" => {
            let s = dec_wide(field("s"));
            let sb = s.as_deref().map(nul_terminated);
            let r = unsafe {
                ansi::atoi(
                    svc,
                    ptr_of(&sb),
                    FILE.as_ptr() as *const c_void,
                    ATOI_CHECK_LINE,
                )
            };
            let (ret, threw) = match r {
                Ok(v) => (format!("ret={v}"), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(op, &[format!("s={}", enc_wide(s.as_deref()))], &ret, threw)
        }
        "atof" => {
            let s = dec_wide(field("s"));
            let sb = s.as_deref().map(nul_terminated);
            let r = unsafe { ansi::atof(svc, ptr_of(&sb)) };
            let (ret, threw) = match r {
                Ok(v) => (format!("ret=0x{:08x}", v.to_bits()), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(op, &[format!("s={}", enc_wide(s.as_deref()))], &ret, threw)
        }
        "strtoi" => {
            let s = dec_wide(field("s"));
            let base = int("base") as i32;
            let with_end = field("end") != "unused";
            let sb = nul_terminated(s.as_deref().expect("strtoi start"));
            let mut slot: *mut i32 = null_mut();
            let end_ptr: *mut *mut i32 = if with_end { &mut slot } else { null_mut() };
            let r = unsafe {
                ansi::strtoi(
                    svc,
                    sb.as_ptr(),
                    end_ptr,
                    base,
                    FILE.as_ptr() as *const c_void,
                    STRTOI_CHECK_LINE,
                )
            };
            let end = if with_end {
                format!("end={}", ret_off(slot, sb.as_ptr()))
            } else {
                "end=unused".into()
            };
            let (ret, threw) = match r {
                Ok(v) => (format!("ret={v}"), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(
                op,
                &[
                    format!("s={}", enc_wide(s.as_deref())),
                    format!("base={base}"),
                    end,
                ],
                &ret,
                threw,
            )
        }
        "to_utf8" => {
            let src = dec_wide(field("src"));
            let cap = int("cap") as i32;
            let src_buf = src.as_deref().map(nul_terminated);
            let mut d = [b'X' as i8; CAP];
            let ret = unsafe { ansi::to_utf8_in_place(d.as_mut_ptr(), ptr_of(&src_buf), cap) };
            let bytes: Vec<u8> = d.iter().map(|&b| b as u8).collect();
            case(
                op,
                &[
                    format!("src={}", enc_wide(src.as_deref())),
                    format!("cap={cap}"),
                ],
                &format!("ret={ret}|buf={}", enc_ansi(Some(&bytes))),
                0,
            )
        }
        "from_utf8" => {
            let src = dec_ansi(field("src"));
            let cap = int("cap") as i32;
            let src_buf = src
                .as_deref()
                .map(|s| s.iter().map(|&b| b as i8).chain([0]).collect::<Vec<i8>>());
            let src_ptr = src_buf.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
            let mut d = init_wide(None);
            let ret = unsafe { ansi::from_utf8_in_place(d.as_mut_ptr(), src_ptr, cap) };
            case(
                op,
                &[
                    format!("src={}", enc_ansi(src.as_deref())),
                    format!("cap={cap}"),
                ],
                &format!("ret={ret}|buf={}", enc_wide(Some(&d[..]))),
                0,
            )
        }
        "checked_int_size" => {
            let size = int("size") as u64;
            let r = ansi::checked_int_size(svc, size as usize);
            let (ret, threw) = match r {
                Ok(v) => (format!("ret={v}"), 0),
                Err(Threw) => ("ret=none".into(), 1),
            };
            case(op, &[format!("size={size}")], &ret, threw)
        }
        other => panic!("unknown op {other}"),
    }
}

#[test]
fn ansi_unansi_fixture_replays_bit_exact() {
    let svc = Hp2AnsiServices {
        fail_assert,
        to_ansi,
        size_exceeds_int_contract,
    };
    let fixture: Vec<&str> = FIXTURE.lines().collect();
    let mut emitted: Vec<String> = Vec::with_capacity(fixture.len());
    let mut per_op: BTreeMap<&str, usize> = BTreeMap::new();
    let mut threw = 0usize;

    for line in fixture.iter().filter(|l| l.starts_with("case|")) {
        HOOKS.with(|h| h.borrow_mut().clear());
        let out = run_case(line, &svc);
        if out.ends_with("|threw=1") {
            threw += 1;
        }
        let op = line.split('|').nth(1).expect("op");
        *per_op.entry(op).or_default() += 1;
        emitted.push(out);
        let hooks = HOOKS.with(|h| std::mem::take(&mut *h.borrow_mut()));
        emitted.extend(hooks);
    }

    assert_eq!(emitted, fixture, "replay diverged from the oracle fixture");
    let cases: usize = per_op.values().sum();
    assert_eq!(cases, EXPECTED_CASES, "case record count");
    assert_eq!(threw, EXPECTED_THREW, "threw record count");
    assert_eq!(per_op.len(), EXPECTED_COUNTS.len(), "op set");
    for (op, n) in EXPECTED_COUNTS {
        assert_eq!(per_op.get(op).copied().unwrap_or(0), n, "{op} record count");
    }
}
