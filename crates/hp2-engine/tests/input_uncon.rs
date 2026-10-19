//! Replays the original `UInput` (UnIn.cpp @ 4914c7e) fixture through the
//! Rust port. `tests/fixtures/input_oracle.txt` is produced by
//! `Build/input_fixture_harness/gen_input.sh` from the unmodified TU; this
//! test drives `hp2_engine::input` with the same scenario and mock services
//! that print the same lines, then compares byte-for-byte.

use std::cell::RefCell;
use std::ffi::{c_char, c_void};
use std::sync::LazyLock;

use hp2_abi::engine::generated::UInput;
use hp2_engine::input::{self, Hp2InputServices};

const EXPECTED: &str = include_str!("fixtures/input_oracle.txt");
const EXPECTED_RECORDS: usize = 1162;
const EXPECTED_CALLS: usize = 121;

const K_BASE: usize = 2928;
const MEM_LEN: usize = K_BASE + 32;

const IST_NONE: i32 = 0;
const IST_PRESS: i32 = 1;
const IST_HOLD: i32 = 2;
const IST_RELEASE: i32 = 3;
const IST_AXIS: i32 = 4;

const KIND_PROPERTY: i32 = 0;
const KIND_BYTE: i32 = 1;
const KIND_FLOAT: i32 = 2;

const VP: usize = 0x1000;
const CLS_A: usize = 0x2000;
const CLS_B: usize = 0x3000;
const PROP_BASE: usize = 0x4000;
const GLOG: usize = 0x5000;
static ITEM: u64 = 0;

const NAMES: [&str; 23] = [
    "None", "Fire", "Jump", "Turn", "Strafe", "Look", "Stance", "Hidden", "Walk", "Sprint",
    "Cycle", "IK_A", "IK_B", "IK_C", "IK_D", "IK_E", "IK_F", "IK_G", "IK_H", "IK_I", "IK_J",
    "IK_K", "IK_L",
];
const BINDINGS: [&str; 12] = [
    "BUTTON Fire",
    "PULSE Jump|AXIS Turn SPEED=2",
    "AXIS Strafe SPEEDBASE=3 SPEED=0.5",
    "AXIS Look",
    "TOGGLE Fire",
    "Sprint",
    "KEYNAME 3",
    "",
    "OnRelease BUTTON Fire|BUTTON Hidden",
    "ZZZ",
    "BUTTON \"Jump\"",
    "BUTTON Walk",
];
const ALIAS_CMDS: [&str; 2] = ["BUTTON Fire|AXIS Turn SPEED=2", "TOGGLE Jump"];
const ALIAS_NAMES: [i32; 2] = [9, 10];

/// (class id, name index, is_byte (1) / is_float (2), offset, flags)
const PROPS: [(i32, i32, i32, usize, u32); 9] = [
    (1, 1, KIND_BYTE, 0, 4),
    (1, 2, KIND_BYTE, 1, 4),
    (1, 7, KIND_BYTE, 2, 0),
    (1, 3, KIND_FLOAT, 4, 4),
    (1, 4, KIND_FLOAT, 8, 4),
    (1, 5, KIND_FLOAT, 12, 4),
    (1, 6, KIND_FLOAT, 16, 0),
    (2, 8, KIND_BYTE, 20, 4),
    (2, 3, KIND_FLOAT, 24, 0),
];

struct Tables {
    key_chars: Vec<Vec<u32>>,
    bindings: Vec<Vec<u32>>,
    alias_cmds: Vec<Vec<u32>>,
    empty: Vec<u32>,
}

fn key_name_index(k: usize) -> i32 {
    if k < 12 { 11 + k as i32 } else { 0 }
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| Tables {
    key_chars: (0..255)
        .map(|k| wz(NAMES[key_name_index(k) as usize]))
        .collect(),
    bindings: (0..255)
        .map(|k| wz(if k < 12 { BINDINGS[k] } else { "" }))
        .collect(),
    alias_cmds: ALIAS_CMDS.iter().map(|s| wz(s)).collect(),
    empty: vec![0],
});

fn tables() -> &'static Tables {
    &TABLES
}

/// NUL-terminated `TEXT("...")` under `HP2_HOST_WCHAR_TCHAR`.
fn wz(s: &str) -> Vec<u32> {
    s.bytes().map(u32::from).chain([0]).collect()
}

/// Reads a NUL-terminated wide string, mapping non-ASCII to `?` like `pw`.
unsafe fn wtext(p: *const u32) -> String {
    let mut s = String::new();
    let mut i = 0;
    unsafe {
        while *p.add(i) != 0 {
            let c = *p.add(i);
            s.push(if c < 128 { c as u8 as char } else { '?' });
            i += 1;
        }
    }
    s
}

fn lower(c: u32) -> u32 {
    if (65..=90).contains(&c) { c + 32 } else { c }
}

fn is_alnum(c: u32) -> bool {
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c)
}

unsafe fn str_len(p: *const u32) -> usize {
    let mut n = 0;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
    }
    n
}

unsafe fn strnicmp(a: *const u32, b: *const u32, n: usize) -> i32 {
    unsafe {
        for i in 0..n {
            let ca = lower(*a.add(i));
            let cb = lower(*b.add(i));
            if ca != cb {
                return 1;
            }
            if ca == 0 {
                break;
            }
        }
    }
    0
}

/// `appStrfind`: case-insensitive, requires a non-alphanumeric lead-in.
unsafe fn str_find(s: *const u32, find: *const u32) -> *const u32 {
    unsafe {
        let f0 = *find;
        let f = if !(97..=122).contains(&f0) {
            f0
        } else {
            f0 - 32
        };
        let length = str_len(find) - 1;
        let fp = find.add(1);
        let mut sp = s;
        let mut c = *sp;
        sp = sp.add(1);
        let mut alnum = false;
        while c != 0 {
            if (97..=122).contains(&c) {
                c -= 32;
            }
            if !alnum && c == f && strnicmp(sp, fp, length) == 0 {
                return sp.sub(1);
            }
            alnum = (65..=90).contains(&c) || (48..=57).contains(&c);
            c = *sp;
            sp = sp.add(1);
        }
    }
    std::ptr::null()
}

/// C `atof` over the narrowed (≤255 char) prefix, rounded to `FLOAT`.
fn c_atof(bytes: &[u8]) -> f32 {
    let mut i = 0;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
        i += 1;
    }
    let start = i;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut digits = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        if j < bytes.len() && bytes[j].is_ascii_digit() {
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    let text = std::str::from_utf8(&bytes[start..i]).unwrap_or("0");
    text.parse::<f64>().unwrap_or(0.0) as f32
}

unsafe fn narrow_atof(p: *const u32) -> f32 {
    let mut buf = Vec::new();
    unsafe {
        while *p.add(buf.len()) != 0 && buf.len() < 255 {
            buf.push(*p.add(buf.len()) as u8);
        }
    }
    c_atof(&buf)
}

#[derive(Clone)]
struct St {
    act: i32,
    delta: f32,
    running: i32,
    actor: i32,
    cls: i32,
    pawn: i32,
    player: i32,
    input: i32,
    mem: [u8; 32],
    keys: [u8; 255],
}

impl St {
    fn new() -> Self {
        St {
            act: IST_NONE,
            delta: 0.0,
            running: 1,
            actor: 1,
            cls: 1,
            pawn: 0,
            player: 0,
            input: 1,
            mem: [0; 32],
            keys: [0; 255],
        }
    }
    fn setb(&mut self, off: usize, v: u8) {
        self.mem[off] = v;
    }
    fn setf(&mut self, off: usize, v: f32) {
        self.mem[off..off + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

#[derive(Default)]
struct Env {
    running: i32,
    actor: bool,
    cls: i32,
    pawn: bool,
    player: i32,
    input: bool,
    uin: usize,
    actor_mem: usize,
    caches: Vec<(usize, Box<[u64]>)>,
}

thread_local! {
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
    static ENV: RefCell<Env> = RefCell::new(Env::default());
}

fn emit(line: String) {
    OUT.with_borrow_mut(|o| {
        o.push_str(&line);
        o.push('\n');
    });
}

fn env<R>(f: impl FnOnce(&mut Env) -> R) -> R {
    ENV.with_borrow_mut(f)
}

// ---- mock services -------------------------------------------------------

unsafe extern "C" fn m_viewport_actor(vp: *mut c_void) -> *mut c_void {
    let _ = vp;
    let (a, m) = env(|e| (e.actor, e.actor_mem));
    if a {
        m as *mut c_void
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "C" fn m_viewport_input(vp: *mut c_void) -> *mut c_void {
    let _ = vp;
    let (i, u) = env(|e| (e.input, e.uin));
    if i {
        u as *mut c_void
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "C" fn m_viewport_update_input(vp: *mut c_void, reset: i32) {
    let _ = vp;
    emit(format!("event\tviewport_update\t{reset}"));
}

unsafe extern "C" fn m_viewport_exec(vp: *mut c_void, cmd: *const u32, ar: *mut c_void) {
    let _ = (vp, ar);
    emit(format!("event\tviewport_exec\t{}", unsafe { wtext(cmd) }));
}

unsafe extern "C" fn m_log_initialized(vp: *mut c_void) {
    let _ = vp;
    emit("event\tinit_log".to_string());
}

unsafe extern "C" fn m_actor_viewport(actor: *mut c_void) -> *mut c_void {
    let _ = actor;
    let (p, pl) = env(|e| (e.pawn, e.player));
    if p && pl == 1 {
        VP as *mut c_void
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "C" fn m_object_class(obj: *mut c_void) -> *mut c_void {
    let _ = obj;
    match env(|e| e.cls) {
        1 => CLS_A as *mut c_void,
        2 => CLS_B as *mut c_void,
        _ => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn m_is_running() -> i32 {
    env(|e| e.running)
}

unsafe extern "C" fn m_glog() -> *mut c_void {
    GLOG as *mut c_void
}

unsafe extern "C" fn m_check_failed(expr: *const c_char, line: i32) {
    panic!("input check failed: {:?} at line {line}", unsafe {
        std::ffi::CStr::from_ptr(expr)
    });
}

unsafe extern "C" fn m_output_log(ar: *mut c_void, text: *const u32) {
    let _ = ar;
    emit(format!("log\t{}", unsafe { wtext(text) }));
}

unsafe extern "C" fn m_output_logf(ar: *mut c_void, text: *const u32) {
    unsafe { m_output_log(ar, text) }
}

/// `UnMisc.cpp` `ParseLine`, non-exact mode.
unsafe extern "C" fn m_parse_line(stream: *mut *const u32, out: *mut u32, max: i32) -> i32 {
    let mut max = max;
    let mut result = out;
    let mut ignore = false;
    let mut quoted = false;
    let mut got = false;
    unsafe {
        *result = 0;
        loop {
            let s = *stream;
            let c = *s;
            if c == 0 || c == 10 || c == 13 {
                break;
            }
            max -= 1;
            if max <= 0 {
                break;
            }
            if !quoted && c == u32::from(b'/') && *s.add(1) == u32::from(b'/') {
                ignore = true;
            }
            if !quoted && c == u32::from(b'|') {
                break;
            }
            quoted ^= c == 34;
            got = true;
            if !ignore {
                *result = c;
                result = result.add(1);
            }
            *stream = s.add(1);
        }
        while matches!(**stream, 10 | 13 | 124) {
            *stream = (*stream).add(1);
        }
        *result = 0;
        (**stream != 0 || got) as i32
    }
}

/// `UnMisc.cpp` `ParseCommand`.
unsafe extern "C" fn m_parse_command(stream: *mut *const u32, m: *const u32) -> i32 {
    unsafe {
        let mut p = *stream;
        while *p == 32 || *p == 9 {
            p = p.add(1);
        }
        *stream = p;
        let n = str_len(m);
        if strnicmp(p, m, n) == 0 {
            let q = p.add(n);
            if !is_alnum(*q) {
                let mut r = q;
                while *r == 32 || *r == 9 {
                    r = r.add(1);
                }
                *stream = r;
                return 1;
            }
        }
        0
    }
}

/// `UnMisc.cpp` `ParseToken`.
unsafe extern "C" fn m_parse_token(
    stream: *mut *const u32,
    out: *mut u32,
    max: i32,
    use_escape: i32,
) -> i32 {
    let max = max as usize;
    let mut len = 0usize;
    unsafe {
        let mut p = *stream;
        while *p == 32 || *p == 9 {
            p = p.add(1);
        }
        if *p == 34 {
            p = p.add(1);
            while *p != 0 && *p != 34 && len + 1 < max {
                let mut c = *p;
                p = p.add(1);
                if c == 92 && use_escape != 0 {
                    c = *p;
                    p = p.add(1);
                    if c == 0 {
                        break;
                    }
                }
                if len + 1 < max {
                    *out.add(len) = c;
                    len += 1;
                }
            }
            if *p == 34 {
                p = p.add(1);
            }
        } else {
            while *p != 0 && *p != 32 && *p != 9 {
                if len + 1 < max {
                    *out.add(len) = *p;
                    len += 1;
                }
                p = p.add(1);
            }
        }
        *out.add(len) = 0;
        *stream = p;
    }
    (len != 0) as i32
}

unsafe extern "C" fn m_parse_float(stream: *const u32, m: *const u32, out: *mut f32) -> i32 {
    unsafe {
        let found = str_find(stream, m);
        if found.is_null() {
            return 0;
        }
        *out = narrow_atof(found.add(str_len(m)));
    }
    1
}

unsafe extern "C" fn m_app_atoi(s: *const u32) -> i32 {
    let mut i = 0;
    let mut neg = false;
    let mut v: i64 = 0;
    unsafe {
        while matches!(*s.add(i), 32 | 9 | 10 | 13) {
            i += 1;
        }
        if *s.add(i) == u32::from(b'-') {
            neg = true;
            i += 1;
        } else if *s.add(i) == u32::from(b'+') {
            i += 1;
        }
        while (48..=57).contains(&*s.add(i)) {
            v = v * 10 + i64::from(*s.add(i) - 48);
            i += 1;
        }
    }
    (if neg { -v } else { v }) as i32
}

unsafe extern "C" fn m_app_strlen(s: *const u32) -> i32 {
    unsafe { str_len(s) as i32 }
}

unsafe extern "C" fn m_app_sprintf_ik(dest: *mut u32, name: *const u32) {
    let mut v: Vec<u32> = "IK_".bytes().map(u32::from).collect();
    v.extend(unsafe { std::slice::from_raw_parts(name, str_len(name)) });
    v.push(0);
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), dest, v.len()) };
}

fn name_index(text: *const u32) -> i32 {
    let s = unsafe { wtext(text) };
    NAMES.iter().position(|n| *n == s).map_or(0, |i| i as i32)
}

unsafe extern "C" fn m_fname_find(text: *const u32) -> i32 {
    name_index(text)
}

unsafe extern "C" fn m_alias_name_index(obj: *mut c_void, index: i32) -> i32 {
    let _ = obj;
    if (0..2).contains(&index) {
        ALIAS_NAMES[index as usize]
    } else {
        0
    }
}

unsafe extern "C" fn m_alias_command(obj: *mut c_void, index: i32) -> *const u32 {
    let _ = obj;
    let t = tables();
    if (0..2).contains(&index) {
        t.alias_cmds[index as usize].as_ptr()
    } else {
        t.empty.as_ptr()
    }
}

unsafe extern "C" fn m_binding_len(obj: *mut c_void, key: i32) -> i32 {
    let _ = obj;
    tables().bindings[key as usize].len() as i32 - 1
}

unsafe extern "C" fn m_binding_chars(obj: *mut c_void, key: i32) -> *const u32 {
    let _ = obj;
    tables().bindings[key as usize].as_ptr()
}

/// Handle to a heap `FString` built like `"%s Speed=%f"`.
unsafe extern "C" fn m_direct_axis_command(obj: *mut c_void, key: i32, length: f32) -> *mut c_void {
    let _ = obj;
    let binding = BINDINGS
        .get(key as usize)
        .filter(|_| key < 12)
        .copied()
        .unwrap_or("");
    let text = format!("{binding} Speed={length:.6}");
    Box::into_raw(Box::new(wz(&text))) as *mut c_void
}

unsafe extern "C" fn m_fstring_chars(handle: *mut c_void) -> *const u32 {
    unsafe { (*(handle as *const Vec<u32>)).as_ptr() }
}

unsafe extern "C" fn m_fstring_free(handle: *mut c_void) {
    unsafe { drop(Box::from_raw(handle as *mut Vec<u32>)) };
}

unsafe extern "C" fn m_key_name_chars(input_keys: *mut c_void, key: i32) -> *const u32 {
    let _ = input_keys;
    tables().key_chars[key as usize].as_ptr()
}

/// `InputKeys->Names.FindItem`, after the `FName(..., FNAME_Find)` none check.
unsafe extern "C" fn m_key_name_find_item(
    input_keys: *mut c_void,
    text: *const u32,
    out: *mut i32,
) -> i32 {
    let _ = input_keys;
    let name = name_index(text);
    if name == 0 {
        return 0;
    }
    match (0..255).find(|&k| key_name_index(k as usize) == name) {
        Some(k) => {
            unsafe { *out = k };
            1
        }
        None => 0,
    }
}

fn prop_index(p: *mut c_void) -> usize {
    (p as usize - PROP_BASE) / 16
}

fn class_id(klass: *mut c_void) -> i32 {
    match klass as usize {
        CLS_A => 1,
        CLS_B => 2,
        _ => 0,
    }
}

unsafe extern "C" fn m_class_nth_property(klass: *mut c_void, kind: i32, n: i32) -> *mut c_void {
    let c = class_id(klass);
    if c == 0 {
        return std::ptr::null_mut();
    }
    let mut seen = 0;
    for (i, p) in PROPS.iter().enumerate() {
        if p.0 != c || (kind != KIND_PROPERTY && p.2 != kind) {
            continue;
        }
        if seen == n {
            return (PROP_BASE + i * 16) as *mut c_void;
        }
        seen += 1;
    }
    std::ptr::null_mut()
}

unsafe extern "C" fn m_property_flags(prop: *mut c_void) -> u32 {
    PROPS[prop_index(prop)].4
}

unsafe extern "C" fn m_property_fname_index(prop: *mut c_void) -> i32 {
    PROPS[prop_index(prop)].1
}

unsafe extern "C" fn m_property_offset(prop: *mut c_void) -> i32 {
    (K_BASE + PROPS[prop_index(prop)].3) as i32
}

unsafe extern "C" fn m_property_is_byte(prop: *mut c_void) -> i32 {
    (PROPS[prop_index(prop)].2 == KIND_BYTE) as i32
}

unsafe extern "C" fn m_property_is_float(prop: *mut c_void) -> i32 {
    (PROPS[prop_index(prop)].2 == KIND_FLOAT) as i32
}

unsafe extern "C" fn m_cache_get(klass: *mut c_void, item: *mut *mut c_void) -> *mut c_void {
    let found = env(|e| {
        e.caches
            .iter_mut()
            .find(|(k, _)| *k == klass as usize)
            .map(|(_, b)| b.as_mut_ptr() as *mut c_void)
    });
    if let Some(p) = found {
        unsafe { *item = std::ptr::addr_of!(ITEM).cast_mut().cast() };
        return p;
    }
    std::ptr::null_mut()
}

unsafe extern "C" fn m_cache_create(
    klass: *mut c_void,
    item: *mut *mut c_void,
    size: usize,
) -> *mut c_void {
    emit(format!("event\tcache_create\tsize={size}"));
    let mut block = vec![0u64; size.div_ceil(8)].into_boxed_slice();
    let p = block.as_mut_ptr() as *mut c_void;
    env(|e| e.caches.push((klass as usize, block)));
    unsafe { *item = std::ptr::addr_of!(ITEM).cast_mut().cast() };
    p
}

unsafe extern "C" fn m_cache_unlock(item: *mut c_void) {
    let _ = item;
}

static SVC: Hp2InputServices = Hp2InputServices {
    viewport_actor: m_viewport_actor,
    viewport_input: m_viewport_input,
    viewport_update_input: m_viewport_update_input,
    viewport_exec: m_viewport_exec,
    log_initialized: m_log_initialized,
    actor_viewport: m_actor_viewport,
    object_class: m_object_class,
    is_running: m_is_running,
    glog: m_glog,
    check_failed: m_check_failed,
    output_log: m_output_log,
    output_logf: m_output_logf,
    parse_line: m_parse_line,
    parse_command: m_parse_command,
    parse_token: m_parse_token,
    parse_float: m_parse_float,
    app_atoi: m_app_atoi,
    app_strlen: m_app_strlen,
    app_sprintf_ik: m_app_sprintf_ik,
    fname_find: m_fname_find,
    alias_name_index: m_alias_name_index,
    alias_command: m_alias_command,
    binding_len: m_binding_len,
    binding_chars: m_binding_chars,
    direct_axis_command: m_direct_axis_command,
    fstring_chars: m_fstring_chars,
    fstring_free: m_fstring_free,
    key_name_chars: m_key_name_chars,
    key_name_find_item: m_key_name_find_item,
    class_nth_property: m_class_nth_property,
    property_flags: m_property_flags,
    property_fname_index: m_property_fname_index,
    property_offset: m_property_offset,
    property_is_byte: m_property_is_byte,
    property_is_float: m_property_is_float,
    cache_get: m_cache_get,
    cache_create: m_cache_create,
    cache_unlock: m_cache_unlock,
};

// ---- replay driver -------------------------------------------------------

struct Replay {
    uin: Box<UInput>,
    mem: Box<[u64]>,
    actor: *mut c_void,
}

impl Replay {
    fn new() -> Self {
        // SAFETY: UInput is repr(C) plain data; all-zero is a valid value.
        let mut uin: Box<UInput> = Box::new(unsafe { std::mem::zeroed() });
        let mut mem = vec![0u64; MEM_LEN.div_ceil(8)].into_boxed_slice();
        let actor = mem.as_mut_ptr() as *mut c_void;
        env(|e| {
            e.uin = &mut *uin as *mut UInput as usize;
            e.actor_mem = actor as usize;
        });
        Replay { uin, mem, actor }
    }

    fn uin_ptr(&mut self) -> *mut UInput {
        &mut *self.uin
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: `mem` is MEM_LEN bytes of initialized u64 storage.
        unsafe { std::slice::from_raw_parts(self.mem.as_ptr().cast::<u8>(), MEM_LEN) }
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as above, uniquely borrowed.
        unsafe { std::slice::from_raw_parts_mut(self.mem.as_mut_ptr().cast::<u8>(), MEM_LEN) }
    }

    fn apply(&mut self, s: &St) {
        env(|e| {
            e.pawn = s.pawn != 0;
            e.running = s.running;
            e.actor = s.actor != 0;
            e.cls = s.cls;
            e.player = s.player;
            e.input = s.input != 0;
        });
        self.bytes_mut()[K_BASE..K_BASE + 32].copy_from_slice(&s.mem);
        let uin = self.uin_ptr();
        // SAFETY: `uin` points to the live boxed UInput.
        unsafe {
            (*uin).viewport = VP as *mut c_void;
            (*uin).action = s.act;
            (*uin).delta = s.delta;
            (*uin).key_down_table = s.keys;
        }
    }

    fn dump(&self, tag: &str) {
        let uin = &*self.uin;
        let (running, actor, cls) = env(|e| (e.running, e.actor, e.cls));
        let class = match cls {
            1 => "A",
            2 => "B",
            _ => "-",
        };
        let mem: String = self.bytes()[K_BASE..K_BASE + 32]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let keys: Vec<String> = uin
            .key_down_table
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != 0)
            .map(|(k, v)| format!("{k}:{v}"))
            .collect();
        let keys = if keys.is_empty() {
            "-".to_string()
        } else {
            keys.join(",")
        };
        emit(format!(
            "{tag}\tact={}\tdelta={:08x}\trunning={running}\tactor={}\tclass={class}\tmem={mem}\tkeys={keys}",
            uin.action,
            uin.delta.to_bits(),
            usize::from(actor),
        ));
    }

    fn begin(&mut self, op: &str, args: &str, s: &St) {
        self.apply(s);
        env(|e| e.caches.clear());
        emit(format!("call\top={op}{args}"));
        self.dump("pre");
    }

    fn finish(&mut self, ret: String) {
        emit(format!("ret\t{ret}"));
        self.dump("post");
    }

    fn exec(&mut self, s: &St, cmd: &str) {
        self.begin("exec", &format!("\tcmd={cmd}"), s);
        let w = wz(cmd);
        let uin = self.uin_ptr();
        let r = unsafe { input::exec(&SVC, uin, w.as_ptr(), GLOG as *mut c_void) };
        self.finish(r.to_string());
    }

    fn exec_commands(&mut self, s: &St, cmd: &str) {
        self.begin("exec_commands", &format!("\tcmd={cmd}"), s);
        let w = wz(cmd);
        let uin = self.uin_ptr();
        unsafe { input::exec_input_commands(&SVC, uin, w.as_ptr(), GLOG as *mut c_void) };
        self.finish("-".to_string());
    }

    fn process(&mut self, s: &St, key: i32, act: i32, delta: f32) {
        let args = format!("\tkey={key}\tact={act}\tdelta={}", delta.to_bits());
        self.begin("process", &args, s);
        let uin = self.uin_ptr();
        let r = unsafe { input::process(&SVC, uin, GLOG as *mut c_void, key, act, delta) };
        self.finish(r.to_string());
    }

    fn preprocess(&mut self, s: &St, key: i32, act: i32, delta: f32) {
        let args = format!("\tkey={key}\tact={act}\tdelta={}", delta.to_bits());
        self.begin("preprocess", &args, s);
        let uin = self.uin_ptr();
        let r = unsafe { input::pre_process(uin, key, act) };
        self.finish(r.to_string());
    }

    fn direct_axis(&mut self, s: &St, key: i32, length: f32, delta: f32) {
        let args = format!(
            "\tkey={key}\tlength={}\tdelta={}",
            length.to_bits(),
            delta.to_bits()
        );
        self.begin("direct_axis", &args, s);
        let uin = self.uin_ptr();
        unsafe { input::direct_axis(&SVC, uin, key, length, delta) };
        self.finish("-".to_string());
    }

    fn read_input(&mut self, s: &St, delta: f32) {
        self.begin("read_input", &format!("\tdelta={}", delta.to_bits()), s);
        let uin = self.uin_ptr();
        unsafe { input::read_input(&SVC, uin, delta) };
        self.finish("-".to_string());
    }

    fn reset_input(&mut self, s: &St) {
        self.begin("reset_input", "", s);
        let uin = self.uin_ptr();
        unsafe { input::reset_input(&SVC, uin) };
        self.finish("-".to_string());
    }

    fn init(&mut self, s: &St) {
        self.begin("init", "", s);
        let uin = self.uin_ptr();
        unsafe { input::init(&SVC, uin, VP as *mut c_void) };
        self.finish("-".to_string());
    }

    fn get_key_name(&mut self, s: &St, key: i32) {
        self.begin("get_key_name", &format!("\tkey={key}"), s);
        let uin = self.uin_ptr();
        let t = unsafe { input::get_key_name(&SVC, uin, key) };
        self.finish(format!("text={}", unsafe { wtext(t) }));
    }

    fn find_key_name(&mut self, s: &St, name: &str) {
        self.begin("find_key_name", &format!("\tname={name}"), s);
        let w = wz(name);
        let mut k: i32 = -1;
        let uin = self.uin_ptr();
        let r = unsafe { input::find_key_name(&SVC, uin, w.as_ptr(), &mut k) };
        self.finish(if r != 0 {
            format!("1\tkey={k}")
        } else {
            "0".to_string()
        });
    }

    fn find_button(&mut self, s: &St, name: &str) {
        self.begin("find_button", &format!("\tname={name}"), s);
        let w = wz(name);
        let uin = self.uin_ptr();
        let p = unsafe { input::find_button_name(&SVC, uin, self.actor, w.as_ptr()) };
        self.finish(offset_ret(p as usize, self.actor as usize));
    }

    fn find_axis(&mut self, s: &St, name: &str) {
        self.begin("find_axis", &format!("\tname={name}"), s);
        let w = wz(name);
        let uin = self.uin_ptr();
        let p = unsafe { input::find_axis_name(&SVC, uin, self.actor, w.as_ptr()) };
        self.finish(offset_ret(p as usize, self.actor as usize));
    }

    fn key_state(&mut self, s: &St, key: u8) {
        let args = format!(
            "\tkey={key}\tpawn={}\tplayer={}\tinput={}",
            s.pawn, s.player, s.input
        );
        self.begin("key_state", &args, s);
        let r = unsafe { input::current_key_state(&SVC, self.actor, key) };
        self.finish(r.to_string());
    }
}

fn offset_ret(p: usize, actor: usize) -> String {
    if p == 0 {
        "none".to_string()
    } else {
        format!("off={}", p as isize - actor as isize)
    }
}

fn print_header() {
    emit("header\tversion=1\tmem_base=2928\tmem_size=32\tik_max=255\talias_max=40".into());
    for (i, n) in NAMES.iter().enumerate() {
        emit(format!("name\tindex={i}\ttext={n}"));
    }
    for k in 0..255usize {
        let name_index = key_name_index(k);
        let text = if k < 12 {
            NAMES[name_index as usize]
        } else {
            "None"
        };
        emit(format!(
            "key\tindex={k}\tname_index={name_index}\ttext={text}"
        ));
    }
    emit("class\tname=A\tid=1".into());
    emit("class\tname=B\tid=2".into());
    for (i, (c, nidx, kind, off, flags)) in PROPS.into_iter().enumerate() {
        let cname = if c == 1 { "A" } else { "B" };
        let kname = if kind == KIND_BYTE { "byte" } else { "float" };
        emit(format!(
            "prop\tclass={cname}\tindex={i}\tname_index={nidx}\tkind={kname}\toffset={}\tflags={flags:x}",
            K_BASE + off
        ));
    }
    for i in 0..40usize {
        if i < 2 {
            emit(format!(
                "alias\tindex={i}\tname_index={}\tcommand={}",
                ALIAS_NAMES[i], ALIAS_CMDS[i]
            ));
        } else {
            emit(format!("alias\tindex={i}\tname_index=0\tcommand="));
        }
    }
    for (k, binding) in BINDINGS
        .iter()
        .chain(std::iter::repeat(&""))
        .take(255)
        .enumerate()
    {
        let text = binding;
        emit(format!("binding\tindex={k}\ttext={text}"));
    }
}

fn replay() -> String {
    OUT.with_borrow_mut(|o| o.clear());
    print_header();
    let mut r = Replay::new();

    // exec batches
    let mut a = St::new();
    a.act = IST_PRESS;
    r.exec(&a, "BUTTON Fire");
    a.act = IST_RELEASE;
    a.setb(0, 0x80);
    r.exec(&a, "BUTTON Fire");
    a.act = IST_HOLD;
    a.setb(1, 0);
    r.exec(&a, "BUTTON Jump");
    a.act = IST_PRESS;
    r.exec(&a, "BUTTON Nope");
    let mut s = a.clone();
    s.actor = 0;
    r.exec(&s, "BUTTON Fire");
    r.exec(&a, "BUTTON");
    r.exec(&a, "BUTTON Hidden");
    r.exec(&a, "BUTTON Walk");
    let mut s = a.clone();
    s.cls = 2;
    s.setb(20, 0);
    r.exec(&s, "BUTTON Walk");
    r.exec(&a, "BUTTON \"Fire\"");
    a.act = IST_PRESS;
    a.setb(1, 0);
    r.exec(&a, "PULSE Jump");
    a.act = IST_RELEASE;
    r.exec(&a, "PULSE Jump");
    a.act = IST_PRESS;
    r.exec(&a, "PULSE Nope");
    r.exec(&a, "PULSE");
    a.act = IST_PRESS;
    a.setb(0, 0);
    r.exec(&a, "TOGGLE Fire");
    a.act = IST_RELEASE;
    r.exec(&a, "TOGGLE Fire");
    r.exec(&a, "TOGGLE Nope");
    a.act = IST_AXIS;
    a.delta = 0.5;
    a.setf(4, 1.0);
    r.exec(&a, "AXIS Turn SPEED=2");
    a.act = IST_HOLD;
    r.exec(&a, "AXIS Turn SPEEDBASE=3 SPEED=2");
    r.exec(&a, "AXIS Strafe SPEEDBASE=0.5");
    a.act = IST_AXIS;
    r.exec(&a, "AXIS Strafe speed=abc");
    a.act = IST_PRESS;
    r.exec(&a, "AXIS Look");
    a.act = IST_AXIS;
    r.exec(&a, "AXIS Stance");
    let mut s = a.clone();
    s.actor = 0;
    r.exec(&s, "AXIS Look");
    r.exec(&a, "AXIS");
    a.act = IST_PRESS;
    for cmd in [
        "KEYNAME 3",
        "KEYNAME 20",
        "KEYNAME -1",
        "KEYNAME 255",
        "KEYNAME 0",
        "KEYNAME 11",
        "KEYBINDING A",
        "KEYBINDING B",
        "KEYBINDING Q",
        "KEYBINDING Z",
        "KEYBINDING L",
        "KEYBINDING ",
        "Sprint",
        "Cycle",
        "Nope",
        "",
        "   ",
        "Sprint extra",
    ] {
        r.exec(&a, cmd);
    }

    // exec_commands
    a.act = IST_PRESS;
    r.exec_commands(&a, "BUTTON Fire|PULSE Jump");
    a.act = IST_RELEASE;
    a.setb(0, 0x80);
    r.exec_commands(&a, "OnRelease BUTTON Fire|BUTTON Jump");
    r.exec_commands(&a, "BUTTON Fire|OnRelease PULSE Jump");
    a.act = IST_PRESS;
    r.exec_commands(&a, "// comment|BUTTON Fire");
    r.exec_commands(&a, "BUTTON Fire // x");
    r.exec_commands(&a, "Some viewport command");
    let long = "v".repeat(300);
    r.exec_commands(&a, &long);
    a.act = IST_HOLD;
    r.exec_commands(&a, "Sprint");
    r.exec_commands(&a, "");

    // process
    let mut a = St::new();
    a.act = IST_NONE;
    r.process(&a, 0, IST_PRESS, 0.5);
    r.process(&a, 2, IST_PRESS, 0.25);
    r.process(&a, 1, IST_HOLD, 0.1);
    r.process(&a, 5, IST_PRESS, 0.0);
    r.process(&a, 8, IST_RELEASE, 0.0);
    r.process(&a, 7, IST_PRESS, 0.0);
    r.process(&a, 9, IST_PRESS, 0.0);
    r.process(&a, 12, IST_PRESS, 0.0);
    r.process(&a, 254, IST_PRESS, 0.0);

    // preprocess
    let mut s = a.clone();
    r.preprocess(&s, 3, IST_PRESS, 0.0);
    s = a.clone();
    s.keys[3] = 1;
    r.preprocess(&s, 3, IST_PRESS, 0.0);
    r.preprocess(&s, 3, IST_RELEASE, 0.0);
    s = a.clone();
    r.preprocess(&s, 3, IST_RELEASE, 0.0);
    s = a.clone();
    s.keys[3] = 1;
    r.preprocess(&s, 3, IST_HOLD, 0.5);
    s = a.clone();
    r.preprocess(&s, 3, IST_NONE, 0.0);

    // direct_axis
    r.direct_axis(&a, 2, 0.5, 0.25);
    r.direct_axis(&a, 0, 1.0, 0.0);
    r.direct_axis(&a, 7, 1.0, 0.5);
    r.direct_axis(&a, 3, -2.5, 0.0);
    r.direct_axis(&a, 1, 0.125, 0.75);

    // read_input
    let mut s = St::new();
    s.setb(0, 0);
    s.setf(4, 1.0);
    s.setf(8, 2.0);
    s.setf(12, -3.0);
    s.setf(16, 4.0);
    s.keys[0] = 1;
    r.read_input(&s, 0.5);
    let mut s = St::new();
    s.setf(4, 1.0);
    s.setf(8, 2.0);
    s.setf(12, -3.0);
    s.setf(16, 4.0);
    r.read_input(&s, -1.0);
    let mut s = St::new();
    s.setf(4, 1.0);
    s.setf(8, 2.0);
    s.setf(12, -3.0);
    s.setf(16, 4.0);
    r.read_input(&s, 0.0);
    let mut s = St::new();
    s.setf(4, 1.0);
    s.running = 0;
    r.read_input(&s, 0.5);
    let mut s = St::new();
    s.cls = 2;
    s.setb(20, 1);
    s.setf(24, 2.0);
    r.read_input(&s, 0.25);
    let mut s = St::new();
    s.keys[1] = 1;
    s.keys[2] = 1;
    s.setf(4, 1.0);
    r.read_input(&s, 0.1);

    // reset / init
    let mut s = St::new();
    s.act = IST_HOLD;
    s.delta = 0.5;
    s.keys[3] = 1;
    s.keys[200] = 1;
    s.setb(0, 1);
    s.setb(1, 1);
    s.setb(2, 1);
    s.setf(4, 1.0);
    s.setf(8, 2.0);
    s.setf(12, 3.0);
    s.setf(16, 4.0);
    r.reset_input(&s);
    let mut s = St::new();
    s.setb(20, 1);
    s.setf(24, 5.0);
    s.cls = 2;
    r.reset_input(&s);
    let mut s = St::new();
    s.keys[3] = 1;
    s.setb(0, 1);
    s.setf(4, 2.0);
    r.init(&s);

    // key names
    let a = St::new();
    for k in [0, 3, 11, 12, 254, 255, -1] {
        r.get_key_name(&a, k);
    }
    for name in ["A", "K", "L", "Z", "l", "", "IK_A"] {
        r.find_key_name(&a, name);
    }

    // buttons / axes
    for name in ["Fire", "Jump", "Hidden", "Turn", "Walk"] {
        r.find_button(&a, name);
    }
    let mut s = a.clone();
    s.cls = 2;
    r.find_button(&s, "Walk");
    r.find_button(&s, "Fire");
    r.find_button(&a, "Nope");
    for name in ["Turn", "Strafe", "Look", "Stance", "Fire"] {
        r.find_axis(&a, name);
    }
    let mut s = a.clone();
    s.cls = 2;
    r.find_axis(&s, "Turn");
    r.find_axis(&s, "Walk");

    // key state
    let mut s = St::new();
    s.pawn = 1;
    s.player = 1;
    s.keys[3] = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 0;
    s.player = 1;
    s.keys[3] = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 2;
    s.keys[3] = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 0;
    s.keys[3] = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 1;
    s.input = 0;
    s.keys[3] = 1;
    r.key_state(&s, 3);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 1;
    s.keys[254] = 1;
    r.key_state(&s, 254);
    let mut s = St::new();
    s.pawn = 1;
    s.player = 1;
    s.keys[254] = 1;
    r.key_state(&s, 255);

    // axis scaling (mul_add sites)
    let mut s = St::new();
    s.act = IST_AXIS;
    s.delta = 8.7;
    s.setf(4, 0.2);
    r.exec(&s, "AXIS Turn SPEEDBASE=1.15 SPEED=0.13");
    let mut s = St::new();
    s.act = IST_AXIS;
    s.delta = 0.39;
    s.setf(4, 0.185);
    r.exec(&s, "AXIS Turn SPEED=0.55");
    let mut s = St::new();
    s.act = IST_HOLD;
    s.delta = 0.1;
    s.setf(4, 0.26);
    r.exec(&s, "AXIS Turn SPEED=0.15");

    OUT.with_borrow(|o| o.clone())
}

#[test]
fn input_uncon_replays_original_fixture() {
    let out = replay();
    assert_eq!(out.lines().count(), EXPECTED_RECORDS, "record count");
    assert_eq!(
        out.lines().filter(|l| l.starts_with("call\t")).count(),
        EXPECTED_CALLS,
        "call count"
    );
    assert_eq!(out, EXPECTED);
}

/// Coverage check from the fixture itself: for each axis branch, does some
/// recorded `AXIS` call produce a bitwise different `mul_add` vs `(a*b)+c`?
#[test]
fn input_axis_branches_have_fused_vs_unfused_difference() {
    let lines: Vec<&str> = EXPECTED.lines().collect();
    // [SpeedBase, IST_Axis, IST_Hold]
    let mut seen = [false; 3];
    for (i, line) in lines.iter().enumerate() {
        let Some(cmd) = line.strip_prefix("call\top=exec\tcmd=AXIS ") else {
            continue;
        };
        let pre = lines[i + 1];
        let field = |name: &str| -> &str {
            pre.split('\t')
                .find_map(|f| f.strip_prefix(name))
                .unwrap_or("")
        };
        let act: i32 = field("act=").parse().unwrap();
        let delta = f32::from_bits(u32::from_str_radix(field("delta="), 16).unwrap());
        let mem = field("mem=");
        let axis_off = match cmd.split_whitespace().next().unwrap_or("") {
            "Turn" => 4,
            "Strafe" => 8,
            "Look" => 12,
            _ => continue,
        };
        if field("class=") != "A" {
            continue;
        }
        let bytes: Vec<u8> = (0..mem.len() / 2)
            .map(|j| u8::from_str_radix(&mem[2 * j..2 * j + 2], 16).unwrap())
            .collect();
        let axis = f32::from_ne_bytes(bytes[axis_off..axis_off + 4].try_into().unwrap());
        let token = |key: &str| -> f32 {
            cmd.split_whitespace()
                .find_map(|t| t.strip_prefix(key))
                .map_or(0.0, |v| v.parse::<f64>().unwrap_or(0.0) as f32)
        };
        let speed = cmd
            .split_whitespace()
            .find_map(|t| t.strip_prefix("SPEED="))
            .map_or(1.0, |v| v.parse::<f64>().unwrap_or(0.0) as f32);
        let speed_base = token("SPEEDBASE=");
        let (branch, fused, unfused) = if speed_base > 0.0 {
            let p = delta * speed_base;
            (0, p.mul_add(speed, axis), p * speed + axis)
        } else if act == IST_AXIS {
            let p = 0.01_f32 * delta;
            (1, p.mul_add(speed, axis), p * speed + axis)
        } else if act == IST_HOLD {
            (2, delta.mul_add(speed, axis), delta * speed + axis)
        } else {
            continue;
        };
        if fused.to_bits() != unfused.to_bits() {
            seen[branch] = true;
        }
    }
    assert!(seen[0], "SpeedBase branch: no fused/unfused difference");
    assert!(seen[1], "IST_Axis branch: no fused/unfused difference");
    assert!(seen[2], "IST_Hold branch: no fused/unfused difference");
}
