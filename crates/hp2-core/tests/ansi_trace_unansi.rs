#![cfg(target_os = "macos")]
//! Replays `fixtures/ansi_trace_unansi.txt` (the oracle transcript recorded by
//! `Build/ansi_trace_fixture_harness`) against the Rust port, record by record.
//!
//! Process isolation: the port keeps process-global state (the `TraceState` mutex,
//! the patrol env snapshot read once through a `OnceLock`, libc `rand()`), and the
//! oracle truncates its trace files once per process. So every scenario runs in its
//! own child process, exactly like the oracle. The parent re-executes this test
//! binary with `--exact scenario_child`, sets the scenario's env on the child only
//! (the 11 trace env keys are removed first), and runs the child in a fresh temp
//! directory because the trace paths are cwd-relative. The child runs the scenario
//! with service fakes that print the same `hook|...` lines as the harness stand-ins,
//! dumps the trace files the way `DumpFile` does, and writes its transcript to a file
//! outside the run directory. The parent compares that transcript with the fixture
//! slice record by record and checks the per-scenario record, call, and file counts.
//! `scenario_child` is a no-op unless the child env vars are set.

use std::cell::RefCell;
use std::env;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Write as _;
use std::fs;
use std::mem::offset_of;
use std::path::Path;
use std::process::{self, Command};
use std::ptr::{null, null_mut};
use std::sync::{LazyLock, Mutex, MutexGuard};

use hp2_core::ansi::{Tchar, Threw};
use hp2_core::ansi_trace::{self as trace, Hp2AnsiTraceServices, TraceState};
use hp2_core::ansi_trace_patrol as patrol;

const FIXTURE: &str = include_str!("fixtures/ansi_trace_unansi.txt");
const SCENARIO_VAR: &str = "ANSI_TRACE_REPLAY_SCENARIO";
const OUT_VAR: &str = "ANSI_TRACE_REPLAY_OUT";
const TRACE_ENV: [&str; 11] = [
    "HP2_RNG_TRACE",
    "HP2_RNG_TRACE_LIMIT",
    "HP2_GLOBAL_TICK_TRACE",
    "HP2_FIRE_TEXTURE_TRACE",
    "HP2_STARTUP_RNG_PHASE_TRACE",
    "HP2_PATROL_STATE_TRACE",
    "HP2_PATROL_STATE_TRACE_ACTOR",
    "HP2_PATROL_STATE_TRACE_STATE_OWNER",
    "HP2_PATROL_STATE_TRACE_STATE",
    "HP2_PATROL_POINT_TRACE",
    "HP2_PATROL_POINT_TRACE_ACTOR",
];

const ANSI_SLOTS: usize = 128;
const ANSI_CAP: usize = 1024;
const PATH_SLOTS: usize = 8;
const PATH_CAP: usize = 512;
const PAYLOAD_BYTES: usize = 64;
const PAYLOAD_OFFSET: i32 = offset_of!(Obj, payload) as i32;

// ---- scenario table -------------------------------------------------------

/// One fixture scenario: record count of its `scenario|`..`end|` slice, call count,
/// and `file|` dump count, all taken from the fixture's `end|` line.
struct Scenario {
    name: &'static str,
    records: usize,
    calls: usize,
    files: usize,
    env: &'static [(&'static str, &'static str)],
}

/// `NAME` in env values is replaced by the scenario name.
const RNG_FILE: (&str, &str) = ("HP2_RNG_TRACE", "NAME.rng.txt");
const STARTUP_FILE: (&str, &str) = ("HP2_STARTUP_RNG_PHASE_TRACE", "NAME.startup.txt");
const PATROL_STATE_FILE: (&str, &str) = ("HP2_PATROL_STATE_TRACE", "NAME.patrol_state.txt");
const PATROL_POINT_FILE: (&str, &str) = ("HP2_PATROL_POINT_TRACE", "NAME.patrol_point.txt");

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "rng_plain",
        records: 22,
        calls: 6,
        files: 0,
        env: &[],
    },
    Scenario {
        name: "fp_contract_sensitivity",
        records: 186,
        calls: 88,
        files: 0,
        env: &[],
    },
    Scenario {
        name: "rng_trace_basic",
        records: 22,
        calls: 6,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "limit_unset",
        records: 24,
        calls: 7,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "limit_two",
        records: 24,
        calls: 7,
        files: 1,
        env: &[RNG_FILE, ("HP2_RNG_TRACE_LIMIT", "2")],
    },
    Scenario {
        name: "limit_text",
        records: 24,
        calls: 7,
        files: 1,
        env: &[RNG_FILE, ("HP2_RNG_TRACE_LIMIT", "abc")],
    },
    Scenario {
        name: "limit_zero",
        records: 24,
        calls: 7,
        files: 1,
        env: &[RNG_FILE, ("HP2_RNG_TRACE_LIMIT", "0")],
    },
    Scenario {
        name: "limit_huge",
        records: 24,
        calls: 7,
        files: 1,
        env: &[RNG_FILE, ("HP2_RNG_TRACE_LIMIT", "99999999999")],
    },
    Scenario {
        name: "write_fail",
        records: 16,
        calls: 3,
        files: 0,
        env: &[("HP2_RNG_TRACE", "missing_dir/NAME.rng.txt")],
    },
    Scenario {
        name: "startup_phase",
        records: 24,
        calls: 6,
        files: 1,
        env: &[STARTUP_FILE],
    },
    Scenario {
        name: "startup_and_rng",
        records: 26,
        calls: 7,
        files: 2,
        env: &[STARTUP_FILE, RNG_FILE],
    },
    Scenario {
        name: "replay_reset",
        records: 22,
        calls: 6,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "phase_scope",
        records: 17,
        calls: 3,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "scope_coverage",
        records: 55,
        calls: 10,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "relevance_branches",
        records: 27,
        calls: 6,
        files: 1,
        env: &[RNG_FILE],
    },
    Scenario {
        name: "patrol_state",
        records: 67,
        calls: 4,
        files: 1,
        env: &[PATROL_STATE_FILE],
    },
    Scenario {
        name: "patrol_state_filtered",
        records: 59,
        calls: 4,
        files: 1,
        env: &[
            PATROL_STATE_FILE,
            ("HP2_PATROL_STATE_TRACE_ACTOR", "ActorA"),
            ("HP2_PATROL_STATE_TRACE_STATE_OWNER", "ActorA"),
            ("HP2_PATROL_STATE_TRACE_STATE", "stateIdle"),
        ],
    },
    Scenario {
        name: "patrol_point",
        records: 49,
        calls: 3,
        files: 1,
        env: &[PATROL_POINT_FILE],
    },
    Scenario {
        name: "patrol_point_filtered",
        records: 42,
        calls: 3,
        files: 1,
        env: &[
            PATROL_POINT_FILE,
            ("HP2_PATROL_POINT_TRACE_ACTOR", "ActorA"),
        ],
    },
    Scenario {
        name: "raise_get_path_name",
        records: 45,
        calls: 3,
        files: 1,
        env: &[PATROL_POINT_FILE],
    },
    Scenario {
        name: "raise_export_text",
        records: 29,
        calls: 3,
        files: 1,
        env: &[PATROL_POINT_FILE],
    },
    Scenario {
        name: "raise_to_ansi",
        records: 42,
        calls: 3,
        files: 1,
        env: &[PATROL_POINT_FILE],
    },
    Scenario {
        name: "json_escape",
        records: 22,
        calls: 5,
        files: 1,
        env: &[STARTUP_FILE],
    },
];

// ---- fake engine world ----------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Object,
    Class,
    Function,
    State,
    BoolProp,
}

/// Stand-in for an engine `UObject`. Boxed and leaked per scenario; the payload
/// holds the bool bytes that `ExportText` reads at `object + offset`.
struct Obj {
    id: String,
    name: Vec<Tchar>,
    class: *mut Obj,
    outer: *mut Obj,
    kind: Kind,
    children: *mut Obj,
    next: *mut Obj,
    offset: i32,
    payload: [u8; PAYLOAD_BYTES],
}

/// Stand-in for `FFrame`: the node (function or state) and the object it runs on.
struct Frame {
    node: *mut Obj,
    object: *mut Obj,
    locals: [u8; 256],
}

/// Per-process fake world and transcript. Thread-local because the service
/// callbacks are context-free `extern "C"` functions.
#[derive(Default)]
struct Rig {
    lines: Vec<String>,
    calls: usize,
    files: usize,
    statics: Vec<(&'static str, *mut Obj)>,
    raise: Option<(&'static str, &'static str)>,
    ansi_ring: Vec<[u8; ANSI_CAP]>,
    ansi_next: usize,
    path_ring: Vec<[Tchar; PATH_CAP]>,
    path_next: usize,
}

impl Rig {
    fn fresh() -> Self {
        Rig {
            ansi_ring: vec![[0; ANSI_CAP]; ANSI_SLOTS],
            path_ring: vec![[0; PATH_CAP]; PATH_SLOTS],
            ..Rig::default()
        }
    }
}

thread_local! {
    static RIG: RefCell<Rig> = RefCell::new(Rig::default());
}

/// The port's trace state. Mirrors the FFI layer's `STATE`, including which entries
/// initialize first (`with_state`) and which do not (`with_state_raw`).
static STATE: LazyLock<Mutex<TraceState>> = LazyLock::new(|| Mutex::new(TraceState::default()));

fn lock_state() -> MutexGuard<'static, TraceState> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn with_state<R>(body: impl FnOnce(&mut TraceState) -> R) -> R {
    let mut st = lock_state();
    if !st.initialized {
        trace::initialize(&mut st);
    }
    body(&mut st)
}

fn with_state_raw<R>(body: impl FnOnce(&mut TraceState) -> R) -> R {
    let mut st = lock_state();
    body(&mut st)
}

/// The services table handed to the port. Each row prints the same `hook|` line the
/// harness stand-in prints and returns the value the fixture records.
static SVC: Hp2AnsiTraceServices = Hp2AnsiTraceServices {
    obj_class: svc_obj_class,
    obj_outer: svc_obj_outer,
    obj_name: svc_obj_name,
    obj_path_name: svc_obj_path_name,
    find_field: svc_find_field,
    is_a: svc_is_a,
    prop_offset: svc_prop_offset,
    prop_bit_mask: svc_prop_bit_mask,
    prop_export_text: svc_prop_export_text,
    frame_node: svc_frame_node,
    frame_object: svc_frame_object,
    frame_locals: svc_frame_locals,
    to_ansi: svc_to_ansi,
    check_fail: svc_check_fail,
    fname_text: svc_fname_text,
    struct_children: svc_struct_children,
};

fn push(line: String) {
    RIG.with_borrow_mut(|r| r.lines.push(line));
}

fn raises(hook: &str, id: &str) -> bool {
    RIG.with_borrow(|r| r.raise.is_some_and(|(h, m)| h == hook && m == id))
}

fn set_raise(raise: Option<(&'static str, &'static str)>) {
    RIG.with_borrow_mut(|r| r.raise = raise);
}

/// Allocates an object without printing it.
fn alloc(id: &str, class: *mut Obj, outer: *mut Obj, kind: Kind, offset: i32) -> *mut Obj {
    Box::into_raw(Box::new(Obj {
        id: id.to_owned(),
        name: wide_nul(id),
        class,
        outer,
        kind,
        children: null_mut(),
        next: null_mut(),
        offset,
        payload: [0; PAYLOAD_BYTES],
    }))
}

/// Allocates and prints the `obj|` line the harness prints for each constructor.
fn new_obj(id: &str, class: *mut Obj, outer: *mut Obj, kind: Kind, offset: i32) -> *mut Obj {
    let p = alloc(id, class, outer, kind, offset);
    push(format!(
        "obj|{id}|class={}|outer={}",
        id_of(class),
        id_of(outer)
    ));
    p
}

fn class(name: &str) -> *mut Obj {
    RIG.with_borrow(|r| {
        r.statics
            .iter()
            .find(|e| e.0 == name)
            .map(|e| e.1)
            .unwrap_or_else(|| panic!("no static class {name}"))
    })
}

fn new_object(id: &str, outer: *mut Obj, class: *mut Obj) -> *mut Obj {
    new_obj(id, class, outer, Kind::Object, 0)
}

fn new_class(id: &str) -> *mut Obj {
    new_obj(id, class("UClass"), null_mut(), Kind::Class, 0)
}

fn new_function(id: &str, outer: *mut Obj) -> *mut Obj {
    new_obj(id, class("UFunction"), outer, Kind::Function, 0)
}

fn new_state(id: &str, outer: *mut Obj) -> *mut Obj {
    new_obj(id, class("UState"), outer, Kind::State, 0)
}

fn new_bool_prop(id: &str, outer: *mut Obj, offset: i32) -> *mut Obj {
    new_obj(id, class("UBoolProperty"), outer, Kind::BoolProp, offset)
}

fn new_frame(object: *mut Obj, node: *mut Obj) -> *mut c_void {
    Box::into_raw(Box::new(Frame {
        node,
        object,
        locals: [0; 256],
    }))
    .cast()
}

fn set_children(owner: *mut Obj, prop: *mut Obj) {
    // SAFETY: `owner` is a leaked box from `alloc`; no reference to it is live here.
    unsafe { (*owner).children = prop };
}

fn set_payload_byte(obj: *mut Obj, index: usize, value: u8) {
    // SAFETY: as in `set_children`.
    unsafe { (*obj).payload[index] = value };
}

fn ptr(p: *mut Obj) -> *const c_void {
    p.cast_const().cast()
}

/// Reborrows an object pointer.
///
/// # Safety
/// `p` must be a pointer returned by `alloc` (or null-checked by the caller).
unsafe fn obj<'a>(p: *const c_void) -> &'a Obj {
    unsafe { &*p.cast::<Obj>() }
}

fn id_of(p: *const Obj) -> String {
    if p.is_null() {
        "none".to_owned()
    } else {
        // SAFETY: non-null object pointers in this world are leaked boxes from `alloc`.
        unsafe { (*p).id.clone() }
    }
}

/// `GetPathName`: the outer chain joined with `.`, outermost first.
fn path_of(o: &Obj) -> String {
    let mut parts = vec![o.id.as_str()];
    let mut cur = o.outer;
    while !cur.is_null() {
        // SAFETY: outer chains are leaked boxes from `alloc`.
        let c = unsafe { &*cur };
        parts.push(c.id.as_str());
        cur = c.outer;
    }
    parts.reverse();
    parts.join(".")
}

fn wide_nul(s: &str) -> Vec<Tchar> {
    s.chars()
        .map(|c| c as Tchar)
        .chain(std::iter::once(0))
        .collect()
}

/// Reads a NUL-terminated `Tchar` string.
///
/// # Safety
/// `p` must point to a NUL-terminated buffer.
unsafe fn read_tchars(p: *const Tchar) -> Vec<Tchar> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        // SAFETY: the caller guarantees NUL termination.
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return out;
        }
        out.push(c);
        i += 1;
    }
}

fn text_of(chars: &[Tchar]) -> String {
    chars
        .iter()
        .map(|&c| char::from_u32(c as u32).unwrap_or('\u{fffd}'))
        .collect()
}

// ---- escaping (mirrors the harness EscText / EscBytes) ---------------------

fn esc_code(out: &mut String, c: u32) {
    match c {
        0x5c => out.push_str("\\\\"),
        0x0a => out.push_str("\\n"),
        0x0d => out.push_str("\\r"),
        0x09 => out.push_str("\\t"),
        0x7c => out.push_str("\\x7c"),
        0x00..=0x1f | 0x7f..=0xff => {
            let _ = write!(out, "\\x{c:02x}");
        }
        0x100.. => {
            let _ = write!(out, "\\x{{{c:x}}}");
        }
        _ => out.push(char::from(c as u8)),
    }
}

fn esc_text(chars: &[Tchar]) -> String {
    let mut out = String::new();
    for &c in chars {
        esc_code(&mut out, c as u32);
    }
    out
}

fn esc_str(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        esc_code(&mut out, c as u32);
    }
    out
}

fn esc_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        esc_code(&mut out, u32::from(b));
    }
    out
}

// ---- services (`unsafe extern "C"`, one per `Hp2AnsiTraceServices` row) ----

unsafe extern "C" fn svc_obj_class(p: *const c_void) -> *mut c_void {
    unsafe { obj(p).class.cast() }
}

unsafe extern "C" fn svc_obj_outer(p: *const c_void) -> *mut c_void {
    unsafe { obj(p).outer.cast() }
}

unsafe extern "C" fn svc_obj_name(p: *const c_void) -> *const c_void {
    unsafe { obj(p).name.as_ptr().cast() }
}

unsafe extern "C" fn svc_obj_path_name(p: *const c_void, out: *mut *const c_void) -> i32 {
    let o = unsafe { obj(p) };
    let id = o.id.clone();
    if raises("GetPathName", &id) {
        push(format!("hook|GetPathName|obj={id}|raised=1"));
        return 1;
    }
    let path = path_of(o);
    let slot = RIG.with_borrow_mut(|r| {
        let i = r.path_next % PATH_SLOTS;
        r.path_next += 1;
        let text: Vec<Tchar> = path
            .chars()
            .take(PATH_CAP - 1)
            .map(|c| c as Tchar)
            .collect();
        let row = &mut r.path_ring[i];
        row[..text.len()].copy_from_slice(&text);
        row[text.len()] = 0;
        row.as_ptr()
    });
    unsafe { *out = slot.cast() };
    push(format!("hook|GetPathName|obj={id}|path={}", esc_str(&path)));
    0
}

unsafe extern "C" fn svc_find_field(
    owner: *const c_void,
    name: *const c_void,
    kind: i32,
) -> *mut c_void {
    let want = unsafe { read_tchars(name.cast()) };
    let mut cur = unsafe { obj(owner) }.children;
    while !cur.is_null() {
        // SAFETY: child links are leaked boxes from `alloc`.
        let c = unsafe { &*cur };
        // Kind 0 is UProperty and kind 1 is UBoolProperty. Every property in this
        // world is a bool, so both kinds match the same objects.
        let property = matches!(kind, 0 | 1) && c.kind == Kind::BoolProp;
        if property && c.name[..c.name.len() - 1] == want[..] {
            return cur.cast();
        }
        cur = c.next;
    }
    null_mut()
}

unsafe extern "C" fn svc_is_a(p: *const c_void, kind: i32) -> i32 {
    let k = unsafe { obj(p) }.kind;
    // 0 UFunction, 1 UState, 2 UObjectProperty (none in this world), 3 UProperty.
    let hit = match kind {
        0 => k == Kind::Function,
        1 => k == Kind::State,
        3 => k == Kind::BoolProp,
        _ => false,
    };
    i32::from(hit)
}

unsafe extern "C" fn svc_prop_offset(p: *const c_void) -> i32 {
    unsafe { obj(p) }.offset
}

unsafe extern "C" fn svc_prop_bit_mask(_p: *const c_void) -> u32 {
    // The harness HProp is built with the four-argument UBoolProperty constructor,
    // which sets BitMask(1) (HarryPotter2/Unreal/Core/Inc/UnType.h:176).
    1
}

unsafe extern "C" fn svc_prop_export_text(
    prop: *const c_void,
    buf: *mut c_void,
    _count: i32,
    data: *const c_void,
    out: *mut i32,
) -> i32 {
    let id = unsafe { obj(prop) }.id.clone();
    if raises("ExportText", &id) {
        push(format!(
            "hook|ExportText|prop={id}|index=0|flags=2|raised=1"
        ));
        return 1;
    }
    let text = if unsafe { *data.cast::<u8>() } != 0 {
        "True"
    } else {
        "False"
    };
    let buf = buf.cast::<Tchar>();
    for (i, b) in text.bytes().enumerate() {
        unsafe { *buf.add(i) = Tchar::from(b) };
    }
    unsafe {
        *buf.add(text.len()) = 0;
        *out = 1;
    }
    push(format!(
        "hook|ExportText|prop={id}|index=0|flags=2|ret=1|value={text}"
    ));
    0
}

unsafe extern "C" fn svc_frame_node(p: *const c_void) -> *mut c_void {
    unsafe { (*p.cast::<Frame>()).node.cast() }
}

unsafe extern "C" fn svc_frame_object(p: *const c_void) -> *mut c_void {
    unsafe { (*p.cast::<Frame>()).object.cast() }
}

unsafe extern "C" fn svc_frame_locals(p: *const c_void) -> *mut u8 {
    unsafe { (*p.cast_mut().cast::<Frame>()).locals.as_mut_ptr() }
}

unsafe extern "C" fn svc_to_ansi(input: *const c_void, out: *mut *const c_char) -> i32 {
    if input.is_null() {
        push("hook|appToAnsi|in=null|out=null".to_owned());
        unsafe { *out = null() };
        return 0;
    }
    let text = unsafe { read_tchars(input.cast()) };
    let raw = text_of(&text);
    let esc_in = esc_text(&text);
    if raises("appToAnsi", &raw) {
        push(format!("hook|appToAnsi|in={esc_in}|raised=1"));
        return 1;
    }
    let cut = text.len().min(ANSI_CAP - 1);
    let (slot, bytes) = RIG.with_borrow_mut(|r| {
        let i = r.ansi_next % ANSI_SLOTS;
        r.ansi_next += 1;
        let row = &mut r.ansi_ring[i];
        for (k, &c) in text[..cut].iter().enumerate() {
            row[k] = c as u8;
        }
        row[cut] = 0;
        (row.as_ptr().cast::<c_char>(), row[..cut].to_vec())
    });
    unsafe { *out = slot };
    push(format!(
        "hook|appToAnsi|in={esc_in}|out={}",
        esc_bytes(&bytes)
    ));
    0
}

unsafe extern "C" fn svc_check_fail(expr: *const c_char, line: i32) -> i32 {
    let text = unsafe { CStr::from_ptr(expr) }.to_bytes();
    push(format!(
        "hook|appFailAssert|expr={}|line={line}",
        esc_bytes(text)
    ));
    1
}

unsafe extern "C" fn svc_fname_text(p: *const c_void) -> *const c_void {
    if p.is_null() {
        return null();
    }
    // Every scenario passes a zeroed FName; anything else is not modelled.
    let bytes = unsafe { std::slice::from_raw_parts(p.cast::<u8>(), 8) };
    assert!(
        bytes.iter().all(|&b| b == 0),
        "non-zero FName is not modelled"
    );
    null()
}

unsafe extern "C" fn svc_struct_children(p: *const c_void) -> *mut c_void {
    unsafe { obj(p).children.cast() }
}

// ---- harness Run* wrappers ------------------------------------------------

fn call(label: &str) {
    RIG.with_borrow_mut(|r| r.calls += 1);
    push(format!("call|{label}"));
}

fn finish(label: &str, outcome: Result<String, Threw>) {
    let text = outcome.unwrap_or_else(|_| "threw=1".to_owned());
    push(format!("result|{label}|{text}"));
}

fn run_int(label: &str, f: impl FnOnce() -> Result<i32, Threw>) {
    call(label);
    finish(label, f().map(|v| format!("ret={v}")));
}

fn run_float(label: &str, f: impl FnOnce() -> Result<f32, Threw>) {
    call(label);
    finish(label, f().map(|v| format!("ret=0x{:08x}", v.to_bits())));
}

fn run_u64(label: &str, f: impl FnOnce() -> u64) {
    call(label);
    push(format!("result|{label}|ret={}", f()));
}

fn run_void(label: &str, f: impl FnOnce() -> Result<(), Threw>) {
    call(label);
    finish(label, f().map(|()| "ok".to_owned()));
}

fn mark_scope(label: &str) {
    push(format!("scope|{label}"));
}

fn must<T>(r: Result<T, Threw>, what: &str) -> T {
    r.unwrap_or_else(|_| panic!("{what} raised; the fixture has no such case"))
}

// ---- port call wrappers (mirror the FFI layer's lock use) -----------------

fn app_rand() -> Result<i32, Threw> {
    with_state(|st| trace::rand_value(&SVC, st))
}

fn app_frand() -> Result<f32, Threw> {
    with_state(|st| trace::frand(&SVC, st))
}

fn app_frand_from_to(from: f32, to: f32) -> Result<f32, Threw> {
    with_state(|st| trace::frand_from_to(&SVC, st, from, to))
}

fn app_range_f(min: f32, max: f32) -> Result<f32, Threw> {
    with_state(|st| trace::rand_range_float(&SVC, st, min, max))
}

fn app_range_i(min: i32, max: i32) -> Result<i32, Threw> {
    with_state(|st| trace::rand_range_int(&SVC, st, min, max))
}

fn ordinal() -> u64 {
    with_state(|st| trace::get_ordinal(st))
}

fn r_tick(label: &str, index: u64, phase: &[u8]) {
    run_void(label, || {
        with_state(|st| trace::set_tick(st, index, Some(phase)));
        Ok(())
    });
}

fn r_flush() {
    run_void("appFlushRandTrace()", || {
        with_state(trace::flush);
        Ok(())
    });
}

fn r_boundary(label: &str, phase: &[u8]) {
    run_void(label, || {
        with_state(|st| trace::set_startup_lifecycle_boundary(st, Some(phase)));
        Ok(())
    });
}

fn r_map(label: &str, map: &str, url: &str) {
    let map = wide_nul(map);
    let url = wide_nul(url);
    run_void(label, || {
        with_state(|st| unsafe { trace::set_startup_map(&SVC, st, map.as_ptr(), url.as_ptr()) })
            .map(|_| ())
    });
}

// ---- scenario bodies (one per `Sc*`/`Run*` in the harness) -----------------

fn rng_plain() {
    run_int("appRand()", app_rand);
    run_float("appFrand()", app_frand);
    run_float("appFrand(FLOAT,FLOAT)", || app_frand_from_to(0.0, 10.0));
    run_float("appRandRange(FLOAT,FLOAT)", || app_range_f(0.25, 1.5));
    run_int("appRandRange(INT,INT)", || app_range_i(3, 9));
    run_u64("appGetRandTraceOrdinal()", ordinal);
}

const LO: [f32; 12] = [
    0.1, 0.3, 1.7, -2.3, 0.7, 3.9, -0.6, 12.345, 0.05, 9.99, -7.1, 5.5,
];
const HI: [f32; 12] = [
    0.7, 2.9, 4.3, 1.1, 9.1, -0.2, 3.3, 31.4159, 0.25, 0.61, 2.2, -8.8,
];

fn fp_contract_sensitivity() {
    for (&a, &b) in LO.iter().zip(HI.iter()) {
        run_float("appFrand(FLOAT,FLOAT)", || app_frand_from_to(a, b));
        run_float("appRandRange(FLOAT,FLOAT)", || app_range_f(a, b));
    }
    for _ in 0..32 {
        run_float("appFrand(FLOAT,FLOAT)", || app_frand_from_to(-3.7, 11.3));
        run_float("appRandRange(FLOAT,FLOAT)", || app_range_f(-3.7, 11.3));
    }
}

fn rng_trace_basic() {
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");
    run_int("appRand()", app_rand);
    run_float("appFrand()", app_frand);
    run_int("appRandRange(INT,INT)", || app_range_i(1, 6));
    r_flush();
    run_u64("appGetRandTraceOrdinal()", ordinal);
}

/// The `HP2_RNG_TRACE_LIMIT` value is supplied by the child env, not the body.
fn rng_limit() {
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");
    for _ in 0..5 {
        run_int("appRand()", app_rand);
    }
    r_flush();
}

fn write_fail() {
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");
    run_int("appRand()", app_rand);
    r_flush();
}

fn startup_phase() {
    r_map("appSetStartupRandTraceMap(Map,URL)", "Map", "URL");
    r_boundary("appSetStartupRandTraceLifecycleBoundary(Boot)", b"Boot");
    r_tick("appSetRandTraceTick(0,Startup)", 0, b"Startup");
    run_int("appRand()", app_rand);
    run_float("appRandRange(FLOAT,FLOAT)", || app_range_f(0.0, 2.0));
    r_flush();
}

fn startup_and_rng() {
    r_map("appSetStartupRandTraceMap(Map,URL)", "Map", "URL");
    r_boundary("appSetStartupRandTraceLifecycleBoundary(Boot)", b"Boot");
    r_tick("appSetRandTraceTick(0,Startup)", 0, b"Startup");
    run_int("appRand()", app_rand);
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");
    run_int("appRand()", app_rand);
    r_flush();
}

fn replay_reset() {
    run_void("appSetRandTraceDiagnosticSeed(7,9)", || {
        with_state(|st| trace::set_diagnostic_seed(st, 7, 9));
        Ok(())
    });
    run_int("appRand()", app_rand);
    run_void("appResetRandTraceForReplay()", || {
        with_state_raw(trace::reset_for_replay);
        Ok(())
    });
    run_int("appRand()", app_rand);
    r_flush();
    run_u64("appGetRandTraceOrdinal()", ordinal);
}

fn phase_scope() {
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");
    mark_scope("FAppRandTracePhaseScope(Phase)");
    let scope = with_state(|st| trace::phase_scope_begin(st, Some(b"Phase")));
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe { trace::phase_scope_end(st, scope) });
    r_flush();
}

fn scope_coverage() {
    let pkg = new_object("PkgA", null_mut(), class("UObject"));
    let actor = new_object("ActorA", pkg, class("UObject"));
    let func = new_function("FnTick", actor);
    let frame = new_frame(actor, func);
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");

    mark_scope("FAppRandTraceOuterScope");
    let h = with_state(|st| trace::outer_scope_begin(st, ptr(actor), ptr(func)));
    run_int("appRand()", app_rand);
    unsafe { trace::context_scope_end(h) };

    mark_scope("FAppRandTraceNativeScope");
    let h = with_state(|st| unsafe {
        trace::native_scope_begin(&SVC, st, frame, c"NativeFn".as_ptr(), 3, 7)
    });
    run_int("appRand()", app_rand);
    unsafe { trace::context_scope_end(h) };

    mark_scope("FAppRandTraceNativeObjectScope");
    let h = with_state(|st| {
        trace::native_object_scope_begin(st, ptr(actor), c"NativeObjFn".as_ptr(), 4, 8)
    });
    run_int("appRand()", app_rand);
    unsafe { trace::context_scope_end(h) };

    mark_scope("FAppRandTracePreBeginScope");
    let h = must(
        with_state(|st| unsafe { trace::prebegin_scope_begin(&SVC, st, ptr(actor), ptr(func)) }),
        "prebegin scope",
    );
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe { trace::prebegin_scope_end(&SVC, st, h) });

    run_void("appRecordRandTracePreBeginSoftwareRendering(1)", || {
        with_state(|st| unsafe { trace::record_software_rendering(st, ptr(actor), true) });
        Ok(())
    });
    run_void("appRecordRandTracePreBeginSoftwareRendering(0)", || {
        with_state(|st| unsafe { trace::record_software_rendering(st, ptr(actor), false) });
        Ok(())
    });

    mark_scope("FAppRandTraceFunctionScope");
    let h = must(
        with_state(|st| unsafe {
            trace::function_scope_begin(&SVC, st, ptr(actor), ptr(func), frame)
        }),
        "function scope",
    );
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe { trace::function_scope_end(st, h) });

    mark_scope("FAppRandTraceRelevanceScope");
    let h = must(
        with_state(|st| unsafe {
            trace::relevance_scope_begin(&SVC, st, ptr(actor), ptr(func), frame)
        }),
        "relevance scope",
    );
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe { trace::relevance_scope_end(st, h) });

    r_flush();
}

fn relevance_branches() {
    let pkg = new_object("PkgA", null_mut(), class("UObject"));
    let actor = new_object("ActorA", pkg, class("UObject"));
    let func = new_function("FnTick", actor);
    let frame = new_frame(actor, func);
    let mut result: i32 = 0;
    let res: *mut i32 = &raw mut result;
    r_tick("appSetRandTraceTick(1,Tick)", 1, b"Tick");

    mark_scope("FAppRandTracePreBeginRelevanceScope");
    let h = with_state(|st| unsafe {
        trace::prebegin_relevance_scope_begin(&SVC, st, frame, ptr(actor), ptr(func), res.cast())
    });
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe { trace::prebegin_relevance_scope_end(st, h) });

    mark_scope("FAppRandTraceRelevanceCallScope");
    let h = with_state(|st| unsafe {
        trace::relevance_call_scope_begin(&SVC, st, frame, ptr(func), res.cast())
    });
    run_int("appRand()", app_rand);
    with_state_raw(|st| unsafe {
        must(
            trace::relevance_call_scope_end(&SVC, st, h),
            "relevance call end",
        )
    });

    run_void("appRecordRandTraceRelevanceReturnDebugInfo(42)", || {
        with_state(|st| unsafe { trace::record_relevance_return_debug_info(&SVC, st, frame, 42) });
        Ok(())
    });
    run_void("appRecordRandTraceRelevanceReturn()", || {
        with_state(|st| unsafe { trace::record_relevance_return(&SVC, st, frame, res.cast()) });
        Ok(())
    });
    r_flush();
}

fn patrol_state() {
    let pkg = new_object("PkgA", null_mut(), class("UObject"));
    let actor_a = new_object("ActorA", pkg, class("UObject"));
    let actor_b = new_object("ActorB", pkg, class("UObject"));
    let state = new_state("stateIdle", actor_a);
    let frame = new_frame(actor_a, state);
    let prop = new_bool_prop("PropFlag", actor_a, 0);
    let value: [u8; 16] = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    run_void(
        "appRecordPatrolStateTraceProperty(ActorA,read)",
        || unsafe {
            patrol::record_state_property(
                &SVC,
                frame,
                ptr(actor_a),
                ptr(prop),
                value.as_ptr().cast(),
                4,
                c"read".as_ptr(),
            )
        },
    );
    run_void("appRecordPatrolStateTraceBranch(ActorA)", || unsafe {
        patrol::record_state_branch(&SVC, frame, ptr(actor_a), 1, 2, 3, 1)
    });
    run_void(
        "appRecordPatrolStateTraceProperty(ActorB,read)",
        || unsafe {
            patrol::record_state_property(
                &SVC,
                frame,
                ptr(actor_b),
                ptr(prop),
                value.as_ptr().cast(),
                8,
                c"read".as_ptr(),
            )
        },
    );
    run_void("appRecordPatrolStateTraceBranch(ActorB)", || unsafe {
        patrol::record_state_branch(&SVC, frame, ptr(actor_b), 5, 6, 7, 0)
    });
}

/// Shared body of the `patrol_point*` and `raise_*` scenarios. The raise (hook,
/// match) pair stays set for the whole scenario, as the harness `GRaise` does.
fn patrol_point(raise: Option<(&'static str, &'static str)>) {
    set_raise(raise);
    let point = new_class("PatrolPoint");
    let requestor = new_class("PatrolRequestor");
    let pkg = new_object("PkgA", null_mut(), class("UObject"));
    let actor_a = new_object("ActorA", pkg, requestor);
    let actor_b = new_object("ActorB", pkg, point);
    let fallback = new_class("PatrolRequestorFallback");
    let actor_c = new_object("ActorC", pkg, fallback);
    // ExportText runs only for a found property: the requestor's class owns
    // FirstPatrolPoint_ObjectName at the payload offset, and the yielded object's
    // class owns Tag one byte further in.
    set_children(
        requestor,
        new_bool_prop("FirstPatrolPoint_ObjectName", requestor, PAYLOAD_OFFSET),
    );
    set_children(point, new_bool_prop("Tag", point, PAYLOAD_OFFSET + 1));
    set_payload_byte(actor_a, 0, 1);
    set_payload_byte(actor_b, 1, 1);
    let tag = [0u8; 8];

    run_void("appRecordPatrolPointIterator(ActorA,1)", || unsafe {
        patrol::record_point_iterator(
            &SVC,
            ptr(actor_a),
            ptr(point),
            tag.as_ptr().cast(),
            1,
            ptr(actor_b),
            2,
        )
    });
    run_void("appRecordPatrolPointIterator(ActorB,2)", || unsafe {
        patrol::record_point_iterator(
            &SVC,
            ptr(actor_b),
            ptr(point),
            tag.as_ptr().cast(),
            2,
            null(),
            3,
        )
    });
    run_void("appRecordPatrolPointIterator(ActorC,3)", || unsafe {
        patrol::record_point_iterator(
            &SVC,
            ptr(actor_c),
            ptr(point),
            tag.as_ptr().cast(),
            3,
            ptr(actor_b),
            4,
        )
    });
}

fn json_escape() {
    r_map("appSetStartupRandTraceMap(quoted)", "Map\"x\\y", "URL\n\t");
    r_boundary(
        "appSetStartupRandTraceLifecycleBoundary(ctrl)",
        b"B\tx\x01|",
    );
    r_tick("appSetRandTraceTick(0,quoted)", 0, b"P\"h");
    run_int("appRand()", app_rand);
    r_flush();
}

// ---- trace file dumps (mirror DumpTraceFiles / DumpFile) ------------------

unsafe extern "C" {
    fn fflush(stream: *mut c_void) -> c_int;
}

fn dump_file(file: &str) {
    // SAFETY: fflush(NULL) flushes all open stdio streams, as the harness does.
    unsafe { fflush(null_mut()) };
    match fs::read(file) {
        Ok(bytes) => {
            RIG.with_borrow_mut(|r| r.files += 1);
            push(format!("file|{file}|{}", esc_bytes(&bytes)));
        }
        Err(_) => push(format!("nofile|{file}")),
    }
}

fn dump_trace_files(name: &str) {
    for kind in [
        ".rng.txt",
        ".startup.txt",
        ".patrol_state.txt",
        ".patrol_point.txt",
    ] {
        dump_file(&format!("{name}{kind}"));
        dump_file(&format!("{name}{kind}.tmp"));
    }
}

// ---- child side -----------------------------------------------------------

fn reset_rig() {
    RIG.with_borrow_mut(|r| *r = Rig::fresh());
    for id in ["UObject", "UClass", "UFunction", "UState", "UBoolProperty"] {
        let p = alloc(id, null_mut(), null_mut(), Kind::Class, 0);
        RIG.with_borrow_mut(|r| r.statics.push((id, p)));
    }
}

/// Runs one scenario in this process and returns its transcript lines.
fn run_scenario(name: &str) -> Vec<String> {
    reset_rig();
    push(format!("scenario|{name}"));
    match name {
        "rng_plain" => rng_plain(),
        "fp_contract_sensitivity" => fp_contract_sensitivity(),
        "rng_trace_basic" => rng_trace_basic(),
        "limit_unset" | "limit_two" | "limit_text" | "limit_zero" | "limit_huge" => rng_limit(),
        "write_fail" => write_fail(),
        "startup_phase" => startup_phase(),
        "startup_and_rng" => startup_and_rng(),
        "replay_reset" => replay_reset(),
        "phase_scope" => phase_scope(),
        "scope_coverage" => scope_coverage(),
        "relevance_branches" => relevance_branches(),
        "patrol_state" | "patrol_state_filtered" => patrol_state(),
        "patrol_point" | "patrol_point_filtered" => patrol_point(None),
        "raise_get_path_name" => patrol_point(Some(("GetPathName", "ActorB"))),
        "raise_export_text" => patrol_point(Some(("ExportText", "Tag"))),
        "raise_to_ansi" => patrol_point(Some(("appToAnsi", "PkgA.ActorA"))),
        "json_escape" => json_escape(),
        other => panic!("unknown scenario {other}"),
    }
    dump_trace_files(name);
    let (calls, files) = RIG.with_borrow(|r| (r.calls, r.files));
    push(format!("end|{name}|calls={calls}|files={files}"));
    RIG.with_borrow_mut(|r| std::mem::take(&mut r.lines))
}

/// Child entry point. A no-op unless the parent set the scenario env vars.
#[test]
fn scenario_child() {
    let (Ok(name), Ok(out)) = (env::var(SCENARIO_VAR), env::var(OUT_VAR)) else {
        return;
    };
    let lines = run_scenario(&name);
    fs::write(&out, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("write {out}: {e}"));
}

// ---- parent side ----------------------------------------------------------

struct Block {
    name: &'static str,
    first_line: usize,
    lines: Vec<&'static str>,
}

/// Splits the fixture into per-scenario slices from `scenario|` through `end|`.
fn fixture_blocks() -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<Block> = None;
    for (index, line) in FIXTURE.lines().enumerate() {
        if let Some(name) = line.strip_prefix("scenario|") {
            assert!(
                open.is_none(),
                "fixture line {}: scenario before end",
                index + 1
            );
            open = Some(Block {
                name,
                first_line: index + 1,
                lines: Vec::new(),
            });
        }
        let block = open
            .as_mut()
            .unwrap_or_else(|| panic!("fixture line {}: record outside a scenario", index + 1));
        block.lines.push(line);
        if line.starts_with("end|") {
            blocks.push(open.take().expect("open block"));
        }
    }
    assert!(open.is_none(), "fixture ends inside a scenario");
    blocks
}

fn run_child(sc: &Scenario, base: &Path) -> Result<Vec<String>, String> {
    let run_dir = base.join(sc.name);
    let transcript = base.join(format!("{}.transcript.txt", sc.name));
    let _ = fs::remove_dir_all(&run_dir);
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&transcript);

    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.args(["--exact", "scenario_child", "--test-threads=1"])
        .current_dir(&run_dir)
        .env(SCENARIO_VAR, sc.name)
        .env(OUT_VAR, &transcript);
    for key in TRACE_ENV {
        cmd.env_remove(key);
    }
    for (key, value) in sc.env {
        cmd.env(key, value.replace("NAME", sc.name));
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `--exact` with a wrong name exits 0 after "running 0 tests", so require the exact run.
    if !output.status.success()
        || !stdout.contains("running 1 test")
        || !stdout.contains("1 passed")
    {
        return Err(format!(
            "child did not run scenario_child ({}):\n{}{}",
            output.status,
            stdout,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let text = fs::read_to_string(&transcript).map_err(|e| format!("no transcript: {e}"))?;
    Ok(text.lines().map(str::to_owned).collect())
}

/// Record-by-record comparison. Reports the first mismatch and the total count.
fn compare(block: &Block, actual: &[String], failures: &mut Vec<String>) {
    let expected = &block.lines;
    let len = expected.len().max(actual.len());
    let differing: Vec<usize> = (0..len)
        .filter(|&i| expected.get(i).copied() != actual.get(i).map(String::as_str))
        .collect();
    if let Some(&first) = differing.first() {
        failures.push(format!(
            "{}: record {first} (fixture line {}): expected {:?}, actual {:?} ({} record(s) differ)",
            block.name,
            block.first_line + first,
            expected.get(first),
            actual.get(first),
            differing.len()
        ));
    }
}

#[test]
fn replays_fixture_record_by_record() {
    let blocks = fixture_blocks();
    assert_eq!(FIXTURE.lines().count(), 892, "fixture line count");
    assert_eq!(blocks.len(), SCENARIOS.len(), "fixture scenario count");
    let total: usize = SCENARIOS.iter().map(|s| s.records).sum();
    assert_eq!(total, 892, "table record total");

    let base = env::temp_dir().join(format!("hp2-ansi-trace-replay-{}", process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("create replay dir");

    let mut failures = Vec::new();
    for (sc, block) in SCENARIOS.iter().zip(&blocks) {
        assert_eq!(block.name, sc.name, "fixture scenario order");
        assert_eq!(
            block.lines.len(),
            sc.records,
            "{}: fixture record count",
            sc.name
        );
        let end = format!("end|{}|calls={}|files={}", sc.name, sc.calls, sc.files);
        assert_eq!(
            block.lines.last().copied(),
            Some(end.as_str()),
            "{}: end line",
            sc.name
        );
        assert_eq!(
            block
                .lines
                .iter()
                .filter(|l| l.starts_with("call|"))
                .count(),
            sc.calls,
            "{}: fixture call count",
            sc.name
        );
        assert_eq!(
            block
                .lines
                .iter()
                .filter(|l| l.starts_with("file|"))
                .count(),
            sc.files,
            "{}: fixture file count",
            sc.name
        );
        match run_child(sc, &base) {
            Ok(actual) => compare(block, &actual, &mut failures),
            Err(e) => failures.push(format!("{}: {e}", sc.name)),
        }
    }

    let _ = fs::remove_dir_all(&base);
    assert!(
        failures.is_empty(),
        "{} scenario(s) diverge from the fixture:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
