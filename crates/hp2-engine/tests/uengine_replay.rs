//! Replays the original `UEngine` (`UnEngine.cpp` @ 6bd3da5) fixture through
//! the Rust port. `tests/fixtures/uengine_oracle.txt` is produced by
//! `Build/uengine_fixture_harness/gen_uengine.sh` from the unmodified TU; this
//! test drives `hp2_engine::uengine` with the same 44 scenarios and mock
//! services that print the same lines, then compares byte-for-byte.
//!
//! Mock state lives in a thread-local `Env`. Engine objects are opaque
//! addresses that are never dereferenced; only their identity and nullness
//! matter, as in the oracle's `state` dump.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use hp2_engine::uengine::{self, Hp2UEngineServices};

const EXPECTED: &str = include_str!("fixtures/uengine_oracle.txt");
const EXPECTED_RECORDS: usize = 347;
const EXPECTED_CALLS: usize = 229;
const EXPECTED_SCENARIOS: usize = 44;

const ENG_PTR: usize = 0x1000;
const VP_PTR: usize = 0x1100;
const CONSOLE_PTR: usize = 0x2000;
const PAWN_PTR: usize = 0x2100;
const HUD_PTR: usize = 0x2200;
const MENU_PTR: usize = 0x2300;
const INPUT_PTR: usize = 0x2400;
const SYS_PTR: usize = 0x2500;
const EXEC_PTR: usize = 0x2600;
const GLOG_PTR: usize = 0x2700;
const CONSOLE_OUT_PTR: usize = 0x2800;
const AUDIO_CLASS_PTR: usize = 0x3000;
const AUDIO_PTR: usize = 0x3100;
const ENGINE_CLASS_PTR: usize = 0x3200;
const URL_PTR: usize = 0x3300;
const CYLINDER_PTR: usize = 0x3400;
const OCYLINDER_PTR: usize = 0x3500;
const OVAL_PTR: usize = 0x3600;
const OOVAL_PTR: usize = 0x3700;
const BOX_PTR: usize = 0x3800;
const RENDER_PTR: usize = 0x3900;
const CLIENT_PTR: usize = 0x3a00;

/// `appSeconds()` script: exact multiples of 0.25 s, one per call.
const CLOCK_SECS: [f64; 17] = [
    0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.25, 3.5, 3.75, 4.0,
];
/// Ticks after which the scripted engine clears `GIsRunning`.
const TICK_STOP: i32 = 6;

thread_local! {
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
    static ENV: RefCell<Env> = RefCell::new(Env::new());
}

/// Mock globals, scripted answers, engine and viewport state.
struct Env {
    // Globals: GIsClient, GIsEditor, GIsRunning, GIsRequestingExit, GSys, GExec.
    is_client: bool,
    is_editor: bool,
    is_running: bool,
    requesting_exit: bool,
    gsys: bool,
    gexec: bool,
    // UEngine fields read by the oracle state dump.
    use_sound: i32,
    cache_megs: i32,
    audio: usize,
    render: usize,
    client: usize,
    cylinder: usize,
    ocylinder: usize,
    oval: usize,
    ooval: usize,
    the_box: usize,
    tick_rate: u32,
    // Scripted answers (`Script` in the harness).
    nosound: i32,
    audio_init: i32,
    config_lang: i32,
    key_type: i32,
    key_event: i32,
    pre: i32,
    proc_: i32,
    url_valid: i32,
    claim: i32,
    // Scenario counters.
    ticks: i32,
    clock_idx: usize,
    // Viewport wiring and the menu class chain (1-based handles).
    console: bool,
    actor: bool,
    hud: bool,
    menu: bool,
    chain: Vec<*const u32>,
}

impl Env {
    fn new() -> Self {
        Self {
            is_client: true,
            is_editor: false,
            is_running: true,
            requesting_exit: false,
            gsys: true,
            gexec: true,
            use_sound: 1,
            cache_megs: 5,
            audio: 0,
            render: 0,
            client: 0,
            cylinder: 0,
            ocylinder: 0,
            oval: 0,
            ooval: 0,
            the_box: 0,
            tick_rate: 0,
            nosound: 0,
            audio_init: 1,
            config_lang: 1,
            key_type: 0,
            key_event: 0,
            pre: 1,
            proc_: 1,
            url_valid: 1,
            claim: -1,
            ticks: 0,
            clock_idx: 0,
            console: false,
            actor: false,
            hud: false,
            menu: false,
            chain: Vec::new(),
        }
    }
}

fn env<R>(f: impl FnOnce(&mut Env) -> R) -> R {
    ENV.with_borrow_mut(f)
}

fn emit(line: String) {
    OUT.with_borrow_mut(|o| {
        o.push_str(&line);
        o.push('\n');
    });
}

fn pv(n: usize) -> *mut c_void {
    ptr::without_provenance_mut(n)
}

fn eng() -> *mut c_void {
    pv(ENG_PTR)
}

fn vp() -> *mut c_void {
    pv(VP_PTR)
}

fn glog() -> *mut c_void {
    pv(GLOG_PTR)
}

/// NUL-terminated `TEXT("...")` as a `TCHAR` (`u32`) array.
fn wz(s: &str) -> Vec<u32> {
    s.bytes().map(u32::from).chain([0]).collect()
}

/// Leaks a NUL-terminated wide string for mock getters that return `const TCHAR*`.
fn leak_w(s: &str) -> *const u32 {
    Box::leak(wz(s).into_boxed_slice()).as_ptr()
}

/// Reads a NUL-terminated wide string, mapping non-ASCII to `?` like `pw`.
///
/// # Safety
/// `p` must point to a NUL-terminated `u32` string.
unsafe fn wtext(p: *const u32) -> String {
    let mut s = String::new();
    let mut i = 0usize;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            break;
        }
        s.push(if c < 128 { c as u8 as char } else { '?' });
        i += 1;
    }
    s
}

fn is_ascii_alnum(c: u32) -> bool {
    (u32::from(b'a')..=u32::from(b'z')).contains(&c)
        || (u32::from(b'A')..=u32::from(b'Z')).contains(&c)
        || (u32::from(b'0')..=u32::from(b'9')).contains(&c)
}

/// ASCII lowercase, as `wcsncasecmp`/`_wcsicmp` in the C locale.
fn lower(c: u32) -> u32 {
    if (u32::from(b'A')..=u32::from(b'Z')).contains(&c) {
        c + 32
    } else {
        c
    }
}

/// `appToUpper` for ASCII.
fn upper(c: u32) -> u32 {
    if (u32::from(b'a')..=u32::from(b'z')).contains(&c) {
        c - 32
    } else {
        c
    }
}

/// # Safety
/// `p` must be NUL-terminated.
unsafe fn str_len(p: *const u32) -> usize {
    let mut n = 0usize;
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    n
}

/// `appStrnicmp`: 0 when the first `n` characters match ignoring ASCII case.
///
/// # Safety
/// Both pointers must be readable up to `n` characters or the first NUL.
unsafe fn strnicmp(a: *const u32, b: *const u32, n: usize) -> i32 {
    for i in 0..n {
        let ca = lower(unsafe { *a.add(i) });
        let cb = lower(unsafe { *b.add(i) });
        if ca != cb {
            return 1;
        }
        if ca == 0 {
            return 0;
        }
    }
    0
}

/// `appStricmp`: 0 when the whole strings match ignoring ASCII case.
///
/// # Safety
/// Both pointers must be NUL-terminated.
unsafe fn stricmp(a: *const u32, b: *const u32) -> i32 {
    let mut i = 0usize;
    loop {
        let ca = lower(unsafe { *a.add(i) });
        let cb = lower(unsafe { *b.add(i) });
        if ca != cb {
            return 1;
        }
        if ca == 0 {
            return 0;
        }
        i += 1;
    }
}

// ---- mock services -------------------------------------------------------

unsafe extern "C" fn m_is_client() -> i32 {
    env(|e| e.is_client as i32)
}

unsafe extern "C" fn m_is_editor() -> i32 {
    env(|e| e.is_editor as i32)
}

unsafe extern "C" fn m_is_running() -> i32 {
    env(|e| e.is_running as i32)
}

unsafe extern "C" fn m_set_is_running(v: i32) {
    env(|e| e.is_running = v != 0);
}

unsafe extern "C" fn m_is_requesting_exit() -> i32 {
    env(|e| e.requesting_exit as i32)
}

unsafe extern "C" fn m_glog() -> *mut c_void {
    glog()
}

unsafe extern "C" fn m_cache_size_megs(_e: *mut c_void) -> i32 {
    env(|e| e.cache_megs)
}

unsafe extern "C" fn m_use_sound(_e: *mut c_void) -> i32 {
    env(|e| e.use_sound)
}

unsafe extern "C" fn m_audio(_e: *mut c_void) -> *mut c_void {
    pv(env(|e| e.audio))
}

unsafe extern "C" fn m_set_audio(_e: *mut c_void, v: *mut c_void) {
    env(|e| e.audio = v.addr());
}

unsafe extern "C" fn m_render(_e: *mut c_void) -> *mut c_void {
    pv(env(|e| e.render))
}

unsafe extern "C" fn m_set_render(_e: *mut c_void, v: *mut c_void) {
    env(|e| e.render = v.addr());
}

unsafe extern "C" fn m_client(_e: *mut c_void) -> *mut c_void {
    pv(env(|e| e.client))
}

unsafe extern "C" fn m_set_client(_e: *mut c_void, v: *mut c_void) {
    env(|e| e.client = v.addr());
}

unsafe extern "C" fn m_set_current_tick_rate(_e: *mut c_void, v: f32) {
    emit(format!("set_current_tick_rate\t{:08x}", v.to_bits()));
    env(|e| e.tick_rate = v.to_bits());
}

fn new_object(kind: &str) {
    emit(format!("new_object\t{kind}"));
}

unsafe extern "C" fn m_create_cylinder(_e: *mut c_void) {
    new_object("UCylinder");
    env(|e| e.cylinder = CYLINDER_PTR);
}

unsafe extern "C" fn m_create_ocylinder(_e: *mut c_void) {
    new_object("UOrientedCylinder");
    env(|e| e.ocylinder = OCYLINDER_PTR);
}

unsafe extern "C" fn m_create_oval_cylinder(_e: *mut c_void) {
    new_object("UAlignedOvalCylinder");
    env(|e| e.oval = OVAL_PTR);
}

unsafe extern "C" fn m_create_ooval_cylinder(_e: *mut c_void) {
    new_object("UOrientedOvalCylinder");
    env(|e| e.ooval = OOVAL_PTR);
}

unsafe extern "C" fn m_create_box(_e: *mut c_void) {
    new_object("UBox");
    env(|e| e.the_box = BOX_PTR);
}

unsafe extern "C" fn m_add_to_root(_e: *mut c_void) {
    emit("add_to_root".into());
}

unsafe extern "C" fn m_remove_from_root(_e: *mut c_void) {
    emit("remove_from_root".into());
}

unsafe extern "C" fn m_engine_init(e: *mut c_void) {
    unsafe { uengine::init(&SVC, e) }
}

unsafe extern "C" fn m_engine_tick(_e: *mut c_void, delta: f32) {
    emit(format!("engine_tick\t{:08x}", delta.to_bits()));
    env(|e| {
        e.ticks += 1;
        if e.ticks == TICK_STOP {
            e.is_running = false;
        }
    });
}

unsafe extern "C" fn m_engine_get_max_tick_rate(_e: *mut c_void) -> f32 {
    unsafe { uengine::get_max_tick_rate(&SVC) }
}

unsafe extern "C" fn m_engine_flush(e: *mut c_void, allow_precache: i32) {
    unsafe { uengine::flush(&SVC, e, allow_precache) }
}

unsafe extern "C" fn m_load_engine_class() -> *mut c_void {
    emit("load_engine_class".into());
    pv(ENGINE_CLASS_PTR)
}

unsafe extern "C" fn m_construct_engine(_cls: *mut c_void) -> *mut c_void {
    emit("construct_engine".into());
    eng()
}

unsafe extern "C" fn m_load_audio_class() -> *mut c_void {
    emit("load_audio_class".into());
    pv(AUDIO_CLASS_PTR)
}

unsafe extern "C" fn m_construct_audio(_cls: *mut c_void) -> *mut c_void {
    emit("construct_audio".into());
    pv(AUDIO_PTR)
}

unsafe extern "C" fn m_audio_delete(_audio: *mut c_void) {
    emit("audio_delete".into());
}

unsafe extern "C" fn m_audio_init(_audio: *mut c_void) -> i32 {
    emit("audio_init".into());
    env(|e| e.audio_init)
}

unsafe extern "C" fn m_log_audio_init_failed() {
    emit("log_audio_init_failed".into());
}

unsafe extern "C" fn m_log_engine_initialized() {
    emit("log_engine_initialized".into());
}

unsafe extern "C" fn m_parse_cmdline_param(param: *const u32) -> i32 {
    let text = unsafe { wtext(param) };
    emit(format!("parse_param\t{text}"));
    env(|e| e.nosound)
}

unsafe extern "C" fn m_gsys() -> *mut c_void {
    if env(|e| e.gsys) {
        pv(SYS_PTR)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn m_gsys_exec(_sys: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("gsys_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 0) as i32)
}

unsafe extern "C" fn m_static_exec(cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("static_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 1) as i32)
}

unsafe extern "C" fn m_gcache_init(bytes: i32, align: i32) {
    emit(format!("cache_init\t{bytes}\t{align}"));
}

unsafe extern "C" fn m_gcache_exit(free_memory: i32) {
    emit(format!("cache_exit\t{free_memory}"));
}

unsafe extern "C" fn m_gcache_flush() {
    emit("gcache_flush".into());
}

unsafe extern "C" fn m_gcache_exec(cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("gcache_exec\t{}", unsafe { wtext(cmd) }));
    0
}

unsafe extern "C" fn m_gexec() -> *mut c_void {
    if env(|e| e.gexec) {
        pv(EXEC_PTR)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn m_gexec_exec(_exec: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("gexec_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 3) as i32)
}

unsafe extern "C" fn m_client_exec(_client: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("client_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 4) as i32)
}

unsafe extern "C" fn m_client_flush(_client: *mut c_void, allow_precache: i32) {
    emit(format!("client_flush\t{allow_precache}"));
}

unsafe extern "C" fn m_render_exec(_render: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("render_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 5) as i32)
}

unsafe extern "C" fn m_audio_exec(_audio: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    emit(format!("audio_exec\t{}", unsafe { wtext(cmd) }));
    env(|e| (e.claim == 6) as i32)
}

unsafe extern "C" fn m_engine_mem_init(bytes: i32, name: *const u32) {
    emit(format!("engine_mem_init\t{bytes}\t{}", unsafe {
        wtext(name)
    }));
}

unsafe extern "C" fn m_engine_mem_exit() {
    emit("engine_mem_exit".into());
}

unsafe extern "C" fn m_url_static_init() {
    emit("url_static_init".into());
}

unsafe extern "C" fn m_url_static_exit() {
    emit("url_static_exit".into());
}

unsafe extern "C" fn m_localize(
    section: *const c_char,
    key: *const c_char,
    package: *const u32,
) -> *const u32 {
    let (sec, key) = unsafe {
        (
            CStr::from_ptr(section).to_string_lossy().into_owned(),
            CStr::from_ptr(key).to_string_lossy().into_owned(),
        )
    };
    emit(format!("localize\t{sec}\t{key}\t{}", unsafe {
        wtext(package)
    }));
    if key == "Yes" {
        leak_w("yes")
    } else {
        leak_w("no")
    }
}

unsafe extern "C" fn m_app_to_upper(c: u32) -> u32 {
    upper(c)
}

unsafe extern "C" fn m_app_stricmp(a: *const u32, b: *const u32) -> i32 {
    unsafe { stricmp(a, b) }
}

unsafe extern "C" fn m_config_get_language(buf: *mut u32, len: i32) -> i32 {
    emit(format!("config_get_language\t{len}"));
    let ok = env(|e| e.config_lang);
    if ok != 0 {
        unsafe {
            *buf = u32::from(b'f');
            *buf.add(1) = u32::from(b'r');
            *buf.add(2) = 0;
        }
    }
    ok
}

unsafe extern "C" fn m_set_language(text: *const u32) {
    emit(format!("set_language\t{}", unsafe { wtext(text) }));
}

unsafe extern "C" fn m_app_seconds() -> i64 {
    let secs = env(|e| {
        let i = e.clock_idx;
        e.clock_idx += 1;
        CLOCK_SECS[i]
    });
    let raw = (secs * 4_294_967_296.0) as i64;
    emit(format!("app_seconds\t{:016x}", raw as u64));
    raw
}

/// `FTime::operator-`: `(v - t.v) / FIXTIME`, `FIXTIME` = `4294967296.f`.
unsafe extern "C" fn m_ftime_sub(a: i64, b: i64) -> f32 {
    (a.wrapping_sub(b) as f32) / 4_294_967_296.0_f32
}

unsafe extern "C" fn m_app_sleep(seconds: f32) {
    emit(format!("app_sleep\t{:08x}", seconds.to_bits()));
}

unsafe extern "C" fn m_viewport_console(_viewport: *mut c_void) -> *mut c_void {
    if env(|e| e.console) {
        pv(CONSOLE_PTR)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn m_viewport_actor(_viewport: *mut c_void) -> *mut c_void {
    if env(|e| e.actor) {
        pv(PAWN_PTR)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn m_viewport_input(_viewport: *mut c_void) -> *mut c_void {
    pv(INPUT_PTR)
}

unsafe extern "C" fn m_actor_hud(_actor: *mut c_void) -> *mut c_void {
    if env(|e| e.hud) {
        pv(HUD_PTR)
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn m_hud_main_menu(_hud: *mut c_void) -> *mut c_void {
    if env(|e| e.menu) {
        pv(MENU_PTR)
    } else {
        ptr::null_mut()
    }
}

/// `UObject::GetClass()` for the harness's fake menu chain. Handles are
/// 1-based indices into `Env::chain`; 0 is NULL.
unsafe extern "C" fn m_object_class(_obj: *mut c_void) -> *mut c_void {
    if env(|e| e.chain.is_empty()) {
        ptr::null_mut()
    } else {
        pv(1)
    }
}

unsafe extern "C" fn m_class_super(cls: *mut c_void) -> *mut c_void {
    let i = cls.addr();
    env(|e| {
        if i < e.chain.len() {
            pv(i + 1)
        } else {
            ptr::null_mut()
        }
    })
}

unsafe extern "C" fn m_class_name(cls: *mut c_void) -> *const u32 {
    let i = cls.addr();
    env(|e| {
        e.chain
            .get(i.wrapping_sub(1))
            .copied()
            .unwrap_or(ptr::null())
    })
}

unsafe extern "C" fn m_console_event_key_type(_console: *mut c_void, key: i32) -> i32 {
    emit(format!("console_key_type\t{key}"));
    env(|e| e.key_type)
}

unsafe extern "C" fn m_console_event_key_event(
    _console: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    emit(format!(
        "console_key_event\t{key}\t{state}\t{:08x}",
        delta.to_bits()
    ));
    env(|e| e.key_event)
}

unsafe extern "C" fn m_console_as_output(_console: *mut c_void) -> *mut c_void {
    pv(CONSOLE_OUT_PTR)
}

unsafe extern "C" fn m_input_pre_process(
    _input: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    emit(format!(
        "input_pre_process\t{key}\t{state}\t{:08x}",
        delta.to_bits()
    ));
    env(|e| e.pre)
}

unsafe extern "C" fn m_input_process(
    _input: *mut c_void,
    ar: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    let sink = if ar.addr() == GLOG_PTR {
        "glog"
    } else {
        "console"
    };
    emit(format!(
        "input_process\t{key}\t{state}\t{:08x}\tar={sink}",
        delta.to_bits()
    ));
    env(|e| e.proc_)
}

/// `ParseCommand` (`UnMisc.cpp`): skips leading blanks in `*stream` even when
/// the match fails; on a full match with a non-alnum follower, skips trailing
/// blanks and returns 1.
unsafe extern "C" fn m_parse_command(stream: *mut *const u32, m: *const u32) -> i32 {
    unsafe {
        let mut s = *stream;
        while *s == 32 || *s == 9 {
            s = s.add(1);
        }
        *stream = s;
        let len = str_len(m);
        if strnicmp(s, m, len) != 0 {
            return 0;
        }
        s = s.add(len);
        if is_ascii_alnum(*s) {
            // Partial match: `*stream` keeps the post-blank position.
            return 0;
        }
        while *s == 32 || *s == 9 {
            s = s.add(1);
        }
        *stream = s;
        1
    }
}

unsafe extern "C" fn m_output_log(_ar: *mut c_void, text: *const u32) {
    emit(format!("log\t{}", unsafe { wtext(text) }));
}

unsafe extern "C" fn m_output_logf(_ar: *mut c_void, text: *const u32) {
    emit(format!("logf\t{}", unsafe { wtext(text) }));
}

unsafe extern "C" fn m_output_logf_s(_ar: *mut c_void, fmt: *const u32, s: *const u32) {
    let (f, s) = unsafe { (wtext(fmt), wtext(s)) };
    emit(format!("logfs\t{f}\t{s}"));
}

unsafe extern "C" fn m_output_logf_i(_ar: *mut c_void, fmt: *const u32, i: i32) {
    emit(format!("logfi\t{}\t{i}", unsafe { wtext(fmt) }));
}

unsafe extern "C" fn m_output_logf_is(_ar: *mut c_void, fmt: *const u32, i: i32, s: *const u32) {
    let (f, s) = unsafe { (wtext(fmt), wtext(s)) };
    emit(format!("logfis\t{f}\t{i}\t{s}"));
}

unsafe extern "C" fn m_url_crack(cmd: *const u32) -> *mut c_void {
    emit(format!("url_crack\t{}", unsafe { wtext(cmd) }));
    pv(URL_PTR)
}

unsafe extern "C" fn m_url_free(_url: *mut c_void) {
    emit("url_free".into());
}

unsafe extern "C" fn m_url_valid(_url: *mut c_void) -> i32 {
    env(|e| e.url_valid)
}

unsafe extern "C" fn m_url_protocol(_url: *mut c_void) -> *const u32 {
    leak_w("unreal")
}

unsafe extern "C" fn m_url_host(_url: *mut c_void) -> *const u32 {
    leak_w("127.0.0.1")
}

unsafe extern "C" fn m_url_port(_url: *mut c_void) -> i32 {
    7777
}

unsafe extern "C" fn m_url_map(_url: *mut c_void) -> *const u32 {
    leak_w("Entry")
}

unsafe extern "C" fn m_url_num_options(_url: *mut c_void) -> i32 {
    2
}

unsafe extern "C" fn m_url_option(_url: *mut c_void, i: i32) -> *const u32 {
    leak_w(if i == 0 { "?Game=x" } else { "?Listen" })
}

unsafe extern "C" fn m_url_portal(_url: *mut c_void) -> *const u32 {
    leak_w("Portal")
}

unsafe extern "C" fn m_url_string(_url: *mut c_void) -> *const u32 {
    leak_w("unreal://127.0.0.1:7777/Entry?Game=x?Listen#Portal")
}

static SVC: Hp2UEngineServices = Hp2UEngineServices {
    is_client: m_is_client,
    is_editor: m_is_editor,
    is_running: m_is_running,
    set_is_running: m_set_is_running,
    is_requesting_exit: m_is_requesting_exit,
    glog: m_glog,
    cache_size_megs: m_cache_size_megs,
    use_sound: m_use_sound,
    audio: m_audio,
    set_audio: m_set_audio,
    render: m_render,
    set_render: m_set_render,
    client: m_client,
    set_client: m_set_client,
    set_current_tick_rate: m_set_current_tick_rate,
    create_cylinder: m_create_cylinder,
    create_ocylinder: m_create_ocylinder,
    create_oval_cylinder: m_create_oval_cylinder,
    create_ooval_cylinder: m_create_ooval_cylinder,
    create_box: m_create_box,
    add_to_root: m_add_to_root,
    remove_from_root: m_remove_from_root,
    engine_init: m_engine_init,
    engine_tick: m_engine_tick,
    engine_get_max_tick_rate: m_engine_get_max_tick_rate,
    engine_flush: m_engine_flush,
    load_engine_class: m_load_engine_class,
    construct_engine: m_construct_engine,
    load_audio_class: m_load_audio_class,
    construct_audio: m_construct_audio,
    audio_delete: m_audio_delete,
    audio_init: m_audio_init,
    log_audio_init_failed: m_log_audio_init_failed,
    log_engine_initialized: m_log_engine_initialized,
    parse_cmdline_param: m_parse_cmdline_param,
    gsys: m_gsys,
    gsys_exec: m_gsys_exec,
    static_exec: m_static_exec,
    gcache_init: m_gcache_init,
    gcache_exit: m_gcache_exit,
    gcache_flush: m_gcache_flush,
    gcache_exec: m_gcache_exec,
    gexec: m_gexec,
    gexec_exec: m_gexec_exec,
    client_exec: m_client_exec,
    client_flush: m_client_flush,
    render_exec: m_render_exec,
    audio_exec: m_audio_exec,
    engine_mem_init: m_engine_mem_init,
    engine_mem_exit: m_engine_mem_exit,
    url_static_init: m_url_static_init,
    url_static_exit: m_url_static_exit,
    localize: m_localize,
    app_to_upper: m_app_to_upper,
    app_stricmp: m_app_stricmp,
    config_get_language: m_config_get_language,
    set_language: m_set_language,
    app_seconds: m_app_seconds,
    ftime_sub: m_ftime_sub,
    app_sleep: m_app_sleep,
    viewport_console: m_viewport_console,
    viewport_actor: m_viewport_actor,
    viewport_input: m_viewport_input,
    actor_hud: m_actor_hud,
    hud_main_menu: m_hud_main_menu,
    object_class: m_object_class,
    class_super: m_class_super,
    class_name: m_class_name,
    console_event_key_type: m_console_event_key_type,
    console_event_key_event: m_console_event_key_event,
    console_as_output: m_console_as_output,
    input_pre_process: m_input_pre_process,
    input_process: m_input_process,
    parse_command: m_parse_command,
    output_log: m_output_log,
    output_logf: m_output_logf,
    output_logf_s: m_output_logf_s,
    output_logf_i: m_output_logf_i,
    output_logf_is: m_output_logf_is,
    url_crack: m_url_crack,
    url_free: m_url_free,
    url_valid: m_url_valid,
    url_protocol: m_url_protocol,
    url_host: m_url_host,
    url_port: m_url_port,
    url_map: m_url_map,
    url_num_options: m_url_num_options,
    url_option: m_url_option,
    url_portal: m_url_portal,
    url_string: m_url_string,
};

// ---- scenario driver -----------------------------------------------------

fn begin(name: &str) {
    emit(format!("scenario\t{name}"));
    env(|e| *e = Env::new());
}

fn dump_state() {
    let line = env(|e| {
        format!(
            "state\tAudio={} Render={} Client={} Cylinder={} OCylinder={} OvalCylinder={} OOvalCylinder={} Box={} CurrentTickRate={:08x} GIsRunning={}",
            (e.audio != 0) as i32,
            (e.render != 0) as i32,
            (e.client != 0) as i32,
            (e.cylinder != 0) as i32,
            (e.ocylinder != 0) as i32,
            (e.oval != 0) as i32,
            (e.ooval != 0) as i32,
            (e.the_box != 0) as i32,
            e.tick_rate,
            e.is_running as i32,
        )
    });
    emit(line);
}

/// Viewport wiring: Input is always present; Actor gates HUD, and HUD gates
/// the main menu, matching the original null-check order.
fn setup_viewport(console: bool, actor: bool, hud: bool, menu: bool) {
    env(|e| {
        e.console = console;
        e.actor = actor;
        e.hud = actor && hud;
        e.menu = e.hud && menu;
    });
}

fn set_chain(names: &[&str]) {
    env(|e| e.chain = names.iter().map(|n| leak_w(n)).collect());
}

fn exec_cmd(cmd: &str) -> i32 {
    let w = wz(cmd);
    unsafe { uengine::exec(&SVC, eng(), w.as_ptr(), glog()) }
}

fn key_cmd(key: i32) -> i32 {
    unsafe { uengine::key(&SVC, vp(), key) }
}

fn input_event_cmd(key: i32, state: i32) -> i32 {
    unsafe { uengine::input_event(&SVC, vp(), key, state, 0.125) }
}

fn replay() -> String {
    OUT.with_borrow_mut(|o| o.clear());

    // Init: client and non-client cache sizing, clamping, Localize yes/no.
    begin("init_default");
    unsafe { uengine::init(&SVC, eng()) };
    dump_state();
    begin("init_no_client");
    env(|e| e.is_client = false);
    unsafe { uengine::init(&SVC, eng()) };
    dump_state();
    begin("init_clamp_high");
    env(|e| e.cache_megs = 5000);
    unsafe { uengine::init(&SVC, eng()) };
    dump_state();
    begin("init_clamp_low");
    env(|e| e.cache_megs = 0);
    unsafe { uengine::init(&SVC, eng()) };
    dump_state();

    // InitAudio.
    begin("audio_ok");
    unsafe { uengine::init_audio(&SVC, eng()) };
    dump_state();
    begin("audio_fail");
    env(|e| e.audio_init = 0);
    unsafe { uengine::init_audio(&SVC, eng()) };
    dump_state();
    begin("audio_nosound");
    env(|e| e.nosound = 1);
    unsafe { uengine::init_audio(&SVC, eng()) };
    dump_state();
    begin("audio_no_usesound");
    env(|e| e.use_sound = 0);
    unsafe { uengine::init_audio(&SVC, eng()) };
    dump_state();
    begin("audio_not_client");
    env(|e| e.is_client = false);
    unsafe { uengine::init_audio(&SVC, eng()) };
    dump_state();

    // Exit / Destroy.
    begin("exit_with_audio");
    env(|e| e.audio = AUDIO_PTR);
    unsafe { uengine::exit(&SVC, eng()) };
    dump_state();
    begin("exit_without_audio");
    unsafe { uengine::exit(&SVC, eng()) };
    dump_state();
    begin("destroy");
    env(|e| {
        e.audio = AUDIO_PTR;
        e.render = RENDER_PTR;
        e.client = CLIENT_PTR;
    });
    unsafe { uengine::destroy(&SVC, eng()) };
    dump_state();

    // Flush.
    begin("flush_client");
    env(|e| e.client = CLIENT_PTR);
    unsafe {
        uengine::flush(&SVC, eng(), 1);
        uengine::flush(&SVC, eng(), 0);
    }
    dump_state();
    begin("flush_no_client");
    unsafe { uengine::flush(&SVC, eng(), 0) };
    dump_state();

    // GetMaxTickRate.
    begin("max_tick_editor");
    env(|e| e.is_editor = true);
    let r = unsafe { uengine::get_max_tick_rate(&SVC) };
    emit(format!("result\tmax_tick_editor\t{:08x}", r.to_bits()));
    dump_state();
    begin("max_tick_game");
    let r = unsafe { uengine::get_max_tick_rate(&SVC) };
    emit(format!("result\tmax_tick_game\t{:08x}", r.to_bits()));
    dump_state();

    // Exec: each claimer in chain order, then fall-through paths.
    const CLAIM_NAMES: [&str; 7] = [
        "sys", "static", "gcache", "gexec", "client", "render", "audio",
    ];
    for (k, cname) in CLAIM_NAMES.iter().enumerate() {
        let name = format!("exec_claim_{cname}");
        begin(&name);
        env(|e| {
            e.claim = k as i32;
            e.client = CLIENT_PTR;
            e.render = RENDER_PTR;
            e.audio = AUDIO_PTR;
        });
        let r = exec_cmd(&format!("CLAIM {cname}"));
        emit(format!("result\t{name}\t{r}"));
        dump_state();
    }
    begin("exec_null_optionals");
    env(|e| {
        e.gsys = false;
        e.gexec = false;
    });
    let r = exec_cmd("NOPE");
    emit(format!("result\texec_null_optionals\t{r}"));
    dump_state();
    begin("exec_flush");
    env(|e| e.client = CLIENT_PTR);
    let r = exec_cmd("FLUSH");
    emit(format!("result\texec_flush\t{r}"));
    dump_state();
    begin("exec_crackurl_valid");
    let r = exec_cmd("CRACKURL unreal://127.0.0.1:7777/Entry?Game=x?Listen#Portal");
    emit(format!("result\texec_crackurl_valid\t{r}"));
    dump_state();
    begin("exec_crackurl_bad");
    env(|e| e.url_valid = 0);
    let r = exec_cmd("CRACKURL bogus");
    emit(format!("result\texec_crackurl_bad\t{r}"));
    dump_state();
    begin("exec_unknown");
    let r = exec_cmd("NOPE");
    emit(format!("result\texec_unknown\t{r}"));
    dump_state();

    // Key: console absent, console declines, console accepts.
    begin("key_no_console");
    setup_viewport(false, false, false, false);
    emit(format!("result\tkey_no_console\t{}", key_cmd(121)));
    dump_state();
    begin("key_console_yes");
    setup_viewport(true, false, false, false);
    env(|e| e.key_type = 1);
    emit(format!("result\tkey_console_yes\t{}", key_cmd(121)));
    dump_state();
    begin("key_console_no");
    setup_viewport(true, false, false, false);
    env(|e| e.key_type = 0);
    emit(format!("result\tkey_console_no\t{}", key_cmd(121)));
    dump_state();

    // InputEvent: console, pre/process, and MainMenu yes/no translation.
    begin("ie_no_console");
    setup_viewport(false, false, false, false);
    emit(format!(
        "result\tie_no_console\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_console_release");
    setup_viewport(true, false, false, false);
    env(|e| e.key_event = 1);
    emit(format!(
        "result\tie_console_release\t{}",
        input_event_cmd(121, 3)
    ));
    dump_state();
    begin("ie_console_press");
    setup_viewport(true, false, false, false);
    env(|e| e.key_event = 1);
    emit(format!(
        "result\tie_console_press\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_console_pre_fail");
    setup_viewport(true, false, false, false);
    env(|e| e.pre = 0);
    emit(format!(
        "result\tie_console_pre_fail\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_console_process");
    setup_viewport(true, false, false, false);
    emit(format!(
        "result\tie_console_process\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_console_process_fail");
    setup_viewport(true, false, false, false);
    env(|e| e.proc_ = 0);
    emit(format!(
        "result\tie_console_process_fail\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_menu_yes");
    setup_viewport(true, true, true, true);
    set_chain(&["UnrealYesNoMenu"]);
    env(|e| e.key_event = 1);
    emit(format!("result\tie_menu_yes\t{}", input_event_cmd(121, 1)));
    dump_state();
    begin("ie_menu_no");
    setup_viewport(true, true, true, true);
    set_chain(&["Foo", "UnrealQuitMenu"]);
    env(|e| e.key_event = 1);
    emit(format!("result\tie_menu_no\t{}", input_event_cmd(110, 1)));
    dump_state();
    begin("ie_menu_other_class");
    setup_viewport(true, true, true, true);
    set_chain(&["Foo", "Bar"]);
    env(|e| e.key_event = 1);
    emit(format!(
        "result\tie_menu_other_class\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();
    begin("ie_menu_nomatch_key");
    setup_viewport(true, true, true, true);
    set_chain(&["UnrealYesNoMenu"]);
    env(|e| e.key_event = 1);
    emit(format!(
        "result\tie_menu_nomatch_key\t{}",
        input_event_cmd(120, 1)
    ));
    dump_state();
    begin("ie_hud_no_menu");
    setup_viewport(true, true, true, false);
    env(|e| e.key_event = 1);
    emit(format!(
        "result\tie_hud_no_menu\t{}",
        input_event_cmd(121, 1)
    ));
    dump_state();

    // Commandlet Main: editor sleep path with scripted clock, and the game path without language.
    begin("cmdlet_editor");
    env(|e| {
        e.is_editor = true;
        e.config_lang = 1;
    });
    let r = unsafe { uengine::commandlet_main(&SVC) };
    emit(format!("result\tcmdlet_editor\t{r}"));
    dump_state();
    begin("cmdlet_game_no_lang");
    env(|e| {
        e.is_editor = false;
        e.config_lang = 0;
    });
    let r = unsafe { uengine::commandlet_main(&SVC) };
    emit(format!("result\tcmdlet_game_no_lang\t{r}"));
    dump_state();

    OUT.with_borrow_mut(std::mem::take)
}

#[test]
fn uengine_replays_original_fixture() {
    let out = replay();
    assert_eq!(out.lines().count(), EXPECTED_RECORDS, "line count");
    let records = out
        .lines()
        .filter(|l| {
            !(l.starts_with("scenario\t") || l.starts_with("result\t") || l.starts_with("state\t"))
        })
        .count();
    assert_eq!(records, EXPECTED_CALLS, "record count");
    assert_eq!(
        out.lines().filter(|l| l.starts_with("scenario\t")).count(),
        EXPECTED_SCENARIOS,
        "scenario count"
    );
    assert_eq!(out, EXPECTED);
}
