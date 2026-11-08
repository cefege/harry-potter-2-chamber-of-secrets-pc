//! Mock `Hp2GameExecServices` for the `UGameEngine::Exec` replay (`game_engine_exec.rs`).
//!
//! Each service emits the record the matching harness hook prints. Harness globals that
//! only `exec` reads (`g_*_ret`, `g_browse_err`, `g_next_url`, `g_enter_text`, the live
//! net driver's connection, the level info netmode and game) live in `EX`;
//! `knobs_reset` restores the harness defaults before each case.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr::null_mut;

use hp2_engine::game_exec::Hp2GameExecServices;

use crate::mock::*;

/// Extra object handles for the exec table, outside the mock's shared address map.
pub const GAME: usize = 0x2180;
pub const CLASS: usize = 0x5000;
pub const GLOG: usize = 0x6000;
pub const SINK: usize = 0x6100;

/// `TEXT("Localized")`, NUL-terminated, returned by the `localize_progress` mock.
static LOCALIZED: [u32; 10] = [0x4C, 0x6F, 0x63, 0x61, 0x6C, 0x69, 0x7A, 0x65, 0x64, 0];

/// Exec-only harness knobs. Defaults match harness `knobs_reset()`.
pub struct Ex {
    pub gis_server: bool,
    pub gis_client: bool,
    pub tick_rate: f32,
    pub max_tick: f32,
    pub netmode: i32,
    pub level_game: bool,
    pub next_url: Option<Vec<u32>>,
    pub enter_text: Vec<u32>,
    pub netdrv_conn: usize,
    pub browse_ret: i32,
    /// `g_browse_err`: `None` leaves the error untouched; `Some` sets it on failure.
    pub browse_err: Option<Vec<u32>>,
    pub level_exec_ret: i32,
    pub script_exec_ret: i32,
    pub exec_ret: i32,
    pub try_skip_ret: i32,
    pub format_ret: i32,
    pub has_class: bool,
    pub child_actor: i32,
    pub child_gi: i32,
    /// Live `FString` error handles; must be zero after every case.
    live_errors: i32,
}

impl Default for Ex {
    fn default() -> Self {
        Ex {
            gis_server: false,
            gis_client: false,
            tick_rate: 30.5,
            max_tick: 60.0,
            netmode: 0,
            level_game: false,
            next_url: None,
            enter_text: Vec::new(),
            netdrv_conn: 0,
            browse_ret: 0,
            browse_err: None,
            level_exec_ret: 0,
            script_exec_ret: 0,
            exec_ret: 0,
            try_skip_ret: 0,
            format_ret: 0,
            has_class: false,
            child_actor: 0,
            child_gi: 0,
            live_errors: 0,
        }
    }
}

thread_local! {
    static EX: RefCell<Ex> = RefCell::new(Ex::default());
}

/// Harness `knobs_reset()`.
pub fn knobs_reset() {
    EX.with(|e| *e.borrow_mut() = Ex::default());
}

pub fn knob<R>(f: impl FnOnce(&mut Ex) -> R) -> R {
    EX.with(|e| f(&mut e.borrow_mut()))
}

pub fn live_errors() -> i32 {
    knob(|k| k.live_errors)
}

/// Harness `exec_case` wiring; `lvl` false clears `GLevel`, `game` sets `LevelInfo->Game`.
#[derive(Clone, Copy)]
pub struct Wiring {
    pub vp: bool,
    pub srv: bool,
    pub cli: bool,
    /// 0 none, 1 net driver, 2 +server connection, 3 +channel 0.
    pub level_net: u8,
    /// 0 none, 1 pending net driver, 2 +connection, 3 +channel 0.
    pub pend: u8,
    pub lvl: bool,
    pub game: bool,
    pub netmode: i32,
}

pub const BASE: Wiring = Wiring {
    vp: false,
    srv: false,
    cli: true,
    level_net: 0,
    pend: 0,
    lvl: true,
    game: false,
    netmode: 0,
};

/// Harness `exec_case` setup (everything before `begin`). Knobs are not reset here.
pub fn wire(w: Wiring) {
    world_base(w.vp);
    knob(|k| {
        k.gis_server = w.srv;
        k.gis_client = w.cli;
        k.tick_rate = 30.5;
        k.netmode = w.netmode;
        k.level_game = w.game;
    });
    model(|m| {
        if !w.lvl {
            m.g_level = 0;
        }
        if w.level_net >= 1 {
            m.level_net_driver = NETDRV;
        }
    });
    if w.level_net >= 2 {
        knob(|k| k.netdrv_conn = CONN);
    }
    if w.level_net >= 3 {
        model(|m| m.conn_channel0 = CHAN);
    }
    if w.pend > 0 {
        set_pending(w.pend >= 1, w.pend >= 2, w.pend >= 3);
    }
}

/// Stand-in for a stack-scoped `FString` error: NUL-terminated text behind an opaque handle.
struct ErrBuf {
    text: Vec<u32>,
}

/// # Safety
/// `error` must be a live handle owned by `ex_travel_scope`.
unsafe fn set_err(error: *mut c_void, text: &[u32]) {
    // SAFETY: caller contract.
    let buf = unsafe { &mut *(error as *mut ErrBuf) };
    buf.text = text.to_vec();
    buf.text.push(0);
}

/// # Safety
/// `dst` must be writable for `s.len() + 1` code units.
unsafe fn write_tchar(dst: *mut u32, s: &str) {
    for (i, b) in s.bytes().enumerate() {
        // SAFETY: caller contract.
        unsafe { *dst.add(i) = b as u32 };
    }
    // SAFETY: caller contract.
    unsafe { *dst.add(s.len()) = 0 };
}

fn is_blank(c: u32) -> bool {
    c == 0x20 || c == 0x09
}

fn is_alnum(c: u32) -> bool {
    char::from_u32(c).is_some_and(|ch| ch.is_ascii_alphanumeric())
}

fn fold_case(c: u32) -> u32 {
    char::from_u32(c).map_or(c, |ch| ch.to_ascii_lowercase() as u32)
}

/// Harness `ParseCommand`: case-insensitive keyword at a word boundary; advances the cursor.
///
/// # Safety
/// `slot` must point at a NUL-terminated stream cursor; `token` must be NUL-terminated.
unsafe fn match_command(slot: *mut *const u32, token: *const u32) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let want = read_tchar(token);
        let mut p = *slot;
        while is_blank(*p) {
            p = p.add(1);
        }
        for (i, &w) in want.iter().enumerate() {
            let c = *p.add(i);
            if c == 0 || fold_case(c) != fold_case(w) {
                return 0;
            }
        }
        let end = p.add(want.len());
        if is_alnum(*end) {
            return 0;
        }
        let mut q = end;
        while is_blank(*q) {
            q = q.add(1);
        }
        *slot = q;
        1
    }
}

/// Harness `ParseToken` (no escapes): quoted or blank-delimited; writes a NUL-terminated token.
///
/// # Safety
/// `stream` must point at a NUL-terminated cursor; `out` must hold `max_len` code units.
unsafe fn match_token(stream: *mut *const u32, out: *mut u32, max_len: i32) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let mut p = *stream;
        while is_blank(*p) {
            p = p.add(1);
        }
        let quoted = *p == '"' as u32;
        if quoted {
            p = p.add(1);
        }
        let cap = max_len.max(1) as usize - 1;
        let mut n = 0usize;
        loop {
            let c = *p;
            if c == 0 {
                break;
            }
            if quoted {
                if c == '"' as u32 {
                    p = p.add(1);
                    break;
                }
            } else if is_blank(c) {
                break;
            }
            if n < cap {
                *out.add(n) = c;
                n += 1;
            }
            p = p.add(1);
        }
        *out.add(n) = 0;
        *stream = p;
        (n != 0) as i32
    }
}

/// Harness `appAtoi` on a NUL-terminated string (no record).
fn c_atoi(s: &[u32]) -> i32 {
    let mut i = 0;
    while i < s.len() && matches!(s[i], 0x20 | 0x09 | 0x0a | 0x0b | 0x0c | 0x0d) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == '-' as u32 || s[i] == '+' as u32) {
        neg = s[i] == '-' as u32;
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && (0x30..=0x39).contains(&s[i]) {
        v = (v * 10 + (s[i] - 0x30) as i64).min(i32::MAX as i64 + 1);
        i += 1;
    }
    if neg {
        v = -v;
    }
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn name_game(a: usize) -> &'static str {
    if a == GAME { "game" } else { nm(a) }
}

fn base_exec(cmd: *const u32) -> i32 {
    let r = knob(|k| k.exec_ret);
    emit(&format!("base_exec {} ret={r}", pfs(&read_tchar(cmd))));
    r
}

// Local model accessors isolate this replay from other service-table migrations.
unsafe extern "C" fn ex_local_svc_level(engine: *mut c_void, result_out: *mut *mut c_void) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            ptr(model(|m| m.g_level))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_pending(
    engine: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            ptr(model(|m| m.g_pending))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_client(engine: *mut c_void, result_out: *mut *mut c_void) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            ptr(model(|m| m.client))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_client_viewport_count(
    client: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(client), CLIENT, "UClient");
            model(|m| m.client_viewports.len() as i32)
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_client_viewport0(
    client: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(client), CLIENT, "UClient");
            ptr(model(|m| m.client_viewports[0]))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_level_net_driver(
    level: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(level), LEVEL, "ULevel");
            ptr(model(|m| m.level_net_driver))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_level_info(
    level: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(level), LEVEL, "ULevel");
            ptr(LEVELINFO)
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_pending_net_driver(
    pending: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(pending), PENDING, "UPendingLevel");
            ptr(model(|m| m.pending_net_driver))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_connection_channel0(
    conn: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(conn), CONN, "UNetConnection");
            ptr(model(|m| m.conn_channel0))
        })());
    }
    0
}

unsafe extern "C" fn ex_local_svc_channel_close(channel: *mut c_void) -> i32 {
    emit(&format!("channel_close {}", nm(addr(channel))));

    0
}

unsafe extern "C" fn ex_local_svc_connection_flush_net(conn: *mut c_void) -> i32 {
    emit(&format!("connection_flush_net {}", nm(addr(conn))));

    0
}

// ---- services ----

unsafe extern "C" fn ex_is_server(result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| knob(|k| k.gis_server as i32))());
    }
    0
}

unsafe extern "C" fn ex_is_client(result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| knob(|k| k.gis_client as i32))());
    }
    0
}

unsafe extern "C" fn ex_glog(result_out: *mut *mut c_void) -> i32 {
    unsafe {
        result_out.write((|| ptr(GLOG))());
    }
    0
}

unsafe extern "C" fn ex_engine_current_tick_rate(engine: *mut c_void, result_out: *mut f32) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            knob(|k| k.tick_rate)
        })());
    }
    0
}

unsafe extern "C" fn ex_engine_get_max_tick_rate(engine: *mut c_void, result_out: *mut f32) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            let v = knob(|k| k.max_tick);
            emit(&format!("get_max_tick_rate {}", fbits(v)));
            v
        })());
    }
    0
}

unsafe extern "C" fn ex_engine_set_progress(
    engine: *mut c_void,
    str1: *const u32,
    str2: *const u32,
    seconds: f32,
) -> i32 {
    check_eng(engine);
    emit(&format!(
        "set_progress {} {} {}",
        pfs(&read_tchar(str1)),
        pfs(&read_tchar(str2)),
        fbits(seconds)
    ));

    0
}

unsafe extern "C" fn ex_engine_cancel_pending(engine: *mut c_void) -> i32 {
    check_eng(engine);
    emit("cancel_pending");

    0
}

unsafe extern "C" fn ex_engine_save_game(engine: *mut c_void, position: i32) -> i32 {
    check_eng(engine);
    emit(&format!("save_game {position}"));

    0
}

unsafe extern "C" fn ex_engine_exec(
    engine: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            base_exec(cmd)
        })());
    }
    0
}

unsafe extern "C" fn ex_engine_exec_base(
    engine: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            check_eng(engine);
            base_exec(cmd)
        })());
    }
    0
}

unsafe extern "C" fn ex_browse_error_is_empty(error: *const c_void, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| {
            // SAFETY: handle came owned by `ex_travel_scope`.
            let buf = unsafe { &*(error as *const ErrBuf) };
            (buf.text.len() == 1) as i32
        })());
    }
    0
}

unsafe extern "C" fn ex_browse_error_text(
    error: *const c_void,
    result_out: *mut *const u32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            // SAFETY: handle came owned by `ex_travel_scope`.
            let buf = unsafe { &*(error as *const ErrBuf) };
            buf.text.as_ptr()
        })());
    }
    0
}

unsafe extern "C" fn ex_engine_browse(
    engine: *mut c_void,
    url: *const u32,
    travel: i32,
    error: *mut c_void,
) -> i32 {
    check_eng(engine);
    let r = knob(|k| k.browse_ret);
    emit(&format!(
        "browse {} travel={travel} ret={r}",
        pfs(&read_tchar(url))
    ));
    if r == 0
        && let Some(text) = knob(|k| k.browse_err.clone())
    {
        // SAFETY: `error` is a live handle passed by `exec`.
        unsafe { set_err(error, &text) };
    }
    r
}

unsafe extern "C" fn ex_engine_browse_last_url(engine: *mut c_void, error: *mut c_void) -> i32 {
    check_eng(engine);
    let r = knob(|k| k.browse_ret);
    emit(&format!("browse_last_url ret={r}"));
    if r == 0
        && let Some(text) = knob(|k| k.browse_err.clone())
    {
        // SAFETY: `error` is a live handle passed by `exec`.
        unsafe { set_err(error, &text) };
    }
    r
}

unsafe extern "C" fn ex_output_logf_s(_ar: *mut c_void, fmt: *const u32, arg: *const u32) -> i32 {
    emit(&format!(
        "logf_str {} {}",
        pfs(&read_tchar(fmt)),
        pfs(&read_tchar(arg))
    ));

    0
}

unsafe extern "C" fn ex_engine_set_client_travel(
    engine: *mut c_void,
    viewport: *mut c_void,
    url: *const u32,
    travel: i32,
) -> i32 {
    check_eng(engine);
    emit(&format!(
        "set_client_travel {} {} items=0 travel={travel}",
        nm(addr(viewport)),
        pfs(&read_tchar(url))
    ));

    0
}

unsafe extern "C" fn ex_engine_set_client_travel_last_url(
    engine: *mut c_void,
    viewport: *mut c_void,
    travel: i32,
) -> i32 {
    check_eng(engine);
    emit(&format!(
        "set_client_travel_last_url {} travel={travel}",
        nm(addr(viewport))
    ));

    0
}

unsafe extern "C" fn ex_level_cleanup_destroyed(level: *mut c_void) -> i32 {
    assert_eq!(addr(level), LEVEL, "ULevel");
    emit(&format!("cleanup_destroyed {} 1", nm(addr(level))));

    0
}

unsafe extern "C" fn ex_level_save_persistent_actors(level: *mut c_void) -> i32 {
    assert_eq!(addr(level), LEVEL, "ULevel");
    let text = knob(|k| k.enter_text.clone());
    emit(&format!(
        "save_persistent_actors {} {}",
        nm(addr(level)),
        pfs(&text)
    ));
    0
}

unsafe extern "C" fn ex_level_exec(
    level: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(level), LEVEL, "ULevel");
            let r = knob(|k| k.level_exec_ret);
            emit(&format!("level_exec {} ret={r}", pfs(&read_tchar(cmd))));
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_level_info_next_url_nonempty(
    info: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(info), LEVELINFO, "ALevelInfo");
            knob(|k| k.next_url.as_ref().is_some_and(|v| !v.is_empty()) as i32)
        })());
    }
    0
}

unsafe extern "C" fn ex_level_info_is_net_client(info: *mut c_void, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(info), LEVELINFO, "ALevelInfo");
            knob(|k| (k.netmode == 3) as i32)
        })());
    }
    0
}

unsafe extern "C" fn ex_level_info_game(info: *mut c_void, result_out: *mut *mut c_void) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(info), LEVELINFO, "ALevelInfo");
            if knob(|k| k.level_game) {
                ptr(GAME)
            } else {
                null_mut()
            }
        })());
    }
    0
}

unsafe extern "C" fn ex_level_info_server_travel(info: *mut c_void, url: *const u32) -> i32 {
    assert_eq!(addr(info), LEVELINFO, "ALevelInfo");
    emit(&format!(
        "server_travel {} {}",
        nm(addr(info)),
        pfs(&read_tchar(url))
    ));

    0
}

unsafe extern "C" fn ex_level_info_broadcast_message(info: *mut c_void, msg: *const u32) -> i32 {
    assert_eq!(addr(info), LEVELINFO, "ALevelInfo");
    emit(&format!(
        "broadcast {} {}",
        nm(addr(info)),
        pfs(&read_tchar(msg))
    ));

    0
}

unsafe extern "C" fn ex_game_script_console_exec(
    game: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            let r = knob(|k| k.script_exec_ret);
            emit(&format!(
                "script_console_exec {} {} ret={r}",
                name_game(addr(game)),
                pfs(&read_tchar(cmd))
            ));
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_pending_try_skip_file(pending: *mut c_void, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(pending), PENDING, "UPendingLevel");
            let r = knob(|k| k.try_skip_ret);
            emit(&format!("try_skip_file {} ret={r}", nm(addr(pending))));
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_net_driver_server_connection(
    driver: *mut c_void,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            let conn = match addr(driver) {
                PENDNETDRV => model(|m| m.pendnetdrv_conn),
                NETDRV => knob(|k| k.netdrv_conn),
                other => panic!("not a net driver: {other:#x}"),
            };
            ptr(conn)
        })());
    }
    0
}

unsafe extern "C" fn ex_find_class_any_package(
    name: *const u32,
    result_out: *mut *mut c_void,
) -> i32 {
    unsafe {
        result_out.write((|| {
            let has = knob(|k| k.has_class);
            emit(&format!(
                "find_class {} ret={}",
                pfs(&read_tchar(name)),
                has as i32
            ));
            if has { ptr(CLASS) } else { null_mut() }
        })());
    }
    0
}

unsafe extern "C" fn ex_class_is_child_of_actor(cls: *mut c_void, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(cls), CLASS, "UClass");
            let r = knob(|k| k.child_actor);
            emit(&format!("is_child_of 0 ret={r}"));
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_class_is_child_of_game_info(cls: *mut c_void, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| {
            assert_eq!(addr(cls), CLASS, "UClass");
            let r = knob(|k| k.child_gi);
            emit(&format!("is_child_of 1 ret={r}"));
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_parse_command(
    stream: *mut *const u32,
    m: *const u32,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            // SAFETY: `exec` passes a live cursor and NUL-terminated keyword.
            unsafe { match_command(stream, m) }
        })());
    }
    0
}

unsafe extern "C" fn ex_parse_token(
    stream: *mut *const u32,
    out: *mut u32,
    max_len: i32,
    _use_escape: i32,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            // SAFETY: `exec` passes a live cursor and an output buffer of `max_len` units.
            let ok = unsafe { match_token(stream, out, max_len) };
            emit(&format!("parse_token ok={ok} {}", pfs(&read_tchar(out))));
            ok
        })());
    }
    0
}

unsafe extern "C" fn ex_app_atoi(s: *const u32, result_out: *mut i32) -> i32 {
    unsafe {
        result_out.write((|| c_atoi(&read_tchar(s)))());
    }
    0
}

unsafe extern "C" fn ex_app_request_exit(force: i32) -> i32 {
    emit(&format!("app_request_exit {force}"));

    0
}

unsafe extern "C" fn ex_app_launch_url(url: *const u32, parms: *const u32) -> i32 {
    emit(&format!(
        "launch_url {} {}",
        pfs(&read_tchar(url)),
        pfs(&read_tchar(parms))
    ));

    0
}

unsafe extern "C" fn ex_localize_progress(key: *const c_char, result_out: *mut *const u32) -> i32 {
    unsafe {
        result_out.write((|| {
            // SAFETY: `exec` passes a NUL-terminated ANSI literal.
            let k = unsafe { CStr::from_ptr(key) }.to_string_lossy();
            emit(&format!("localize_progress {k}"));
            LOCALIZED.as_ptr()
        })());
    }
    0
}

unsafe extern "C" fn ex_make_save_directory(slot: i32) -> i32 {
    emit(if slot == 0 {
        "make_directory \"SavePath\""
    } else {
        "make_directory \"SaveSlotPath\""
    });

    0
}

unsafe extern "C" fn ex_save_slot_filename(slot: i32, out: *mut u32) -> i32 {
    let name = format!("SaveSlot{slot}.usa");
    emit(&format!("save_slot_filename {slot} {}", pfs(&tc(&name))));
    // SAFETY: `exec` passes a `MAX_SPRINTF`-sized output buffer.
    unsafe { write_tchar(out, &name) };

    0
}

unsafe extern "C" fn ex_debugf_log_i(fmt: *const u32, value: i32) -> i32 {
    emit(&format!("debugf_int {} {value}", pfs(&read_tchar(fmt))));

    0
}

unsafe extern "C" fn ex_debugf_log_s(fmt: *const u32, value: *const u32) -> i32 {
    emit(&format!(
        "debugf_str {} {}",
        pfs(&read_tchar(fmt)),
        pfs(&read_tchar(value))
    ));

    0
}

unsafe extern "C" fn ex_load_persistent_actor_cache() -> i32 {
    emit("load_persistent_actor_cache");

    0
}

unsafe extern "C" fn ex_format_load_game_url(
    slot: i32,
    out: *mut u32,
    _capacity: i32,
    result_out: *mut i32,
) -> i32 {
    unsafe {
        result_out.write((|| {
            let r = knob(|k| k.format_ret);
            emit(&format!("format_load_game_url {slot} ret={r}"));
            if r != 0 {
                // SAFETY: `exec` passes a buffer of at least `_capacity` units.
                unsafe { write_tchar(out, &format!("Load{slot}.usa")) };
            }
            r
        })());
    }
    0
}

unsafe extern "C" fn ex_output_log(_ar: *mut c_void, text: *const u32) -> i32 {
    emit(&format!("log {}", pfs(&read_tchar(text))));

    0
}

unsafe extern "C" fn ex_output_logf_f(_ar: *mut c_void, fmt: *const u32, value: f32) -> i32 {
    emit(&format!(
        "logf_float {} {}",
        pfs(&read_tchar(fmt)),
        fbits(value)
    ));

    0
}

unsafe extern "C" fn ex_travel_scope(
    engine: *mut c_void,
    ar: *mut c_void,
    text: *const u32,
    branch: u32,
    out: *mut i32,
) -> i32 {
    let mut error = ErrBuf { text: vec![0] };
    knob(|k| k.live_errors += 1);
    let result = unsafe {
        hp2_engine::game_exec::travel(
            &services(),
            engine,
            ar,
            text,
            branch,
            (&raw mut error).cast(),
        )
    };
    knob(|k| k.live_errors -= 1);
    match result {
        Ok(value) => {
            unsafe {
                out.write(value);
            }
            0
        }
        Err(status) => status,
    }
}
unsafe extern "C" fn ex_url_scope(
    engine: *mut c_void,
    text: *const u32,
    travel: i32,
    copy_last: i32,
    error: *mut c_void,
    out: *mut i32,
) -> i32 {
    let result = unsafe {
        let browsed = if copy_last != 0 {
            ex_engine_browse_last_url(engine, error)
        } else {
            ex_engine_browse(engine, text, travel, error)
        };
        hp2_engine::game_exec::browse_condition(&services(), browsed, error)
    };
    match result {
        Ok(value) => {
            unsafe {
                out.write(value);
            }
            0
        }
        Err(status) => status,
    }
}

/// The full exec service table, wired to the shared mock objects.
pub fn services() -> Hp2GameExecServices {
    Hp2GameExecServices {
        is_server: ex_is_server,
        is_client: ex_is_client,
        glog: ex_glog,
        engine_level: ex_local_svc_level,
        engine_pending: ex_local_svc_pending,
        engine_client: ex_local_svc_client,
        engine_current_tick_rate: ex_engine_current_tick_rate,
        engine_get_max_tick_rate: ex_engine_get_max_tick_rate,
        engine_set_progress: ex_engine_set_progress,
        engine_cancel_pending: ex_engine_cancel_pending,
        engine_save_game: ex_engine_save_game,
        engine_exec: ex_engine_exec,
        engine_exec_base: ex_engine_exec_base,
        travel_scope: ex_travel_scope,
        url_scope: ex_url_scope,
        browse_error_is_empty: ex_browse_error_is_empty,
        browse_error_text: ex_browse_error_text,
        output_logf_s: ex_output_logf_s,
        engine_set_client_travel: ex_engine_set_client_travel,
        engine_set_client_travel_last_url: ex_engine_set_client_travel_last_url,
        client_viewport_count: ex_local_svc_client_viewport_count,
        client_viewport0: ex_local_svc_client_viewport0,
        level_net_driver: ex_local_svc_level_net_driver,
        level_info: ex_local_svc_level_info,
        level_cleanup_destroyed: ex_level_cleanup_destroyed,
        level_save_persistent_actors: ex_level_save_persistent_actors,
        level_exec: ex_level_exec,
        level_info_next_url_nonempty: ex_level_info_next_url_nonempty,
        level_info_is_net_client: ex_level_info_is_net_client,
        level_info_game: ex_level_info_game,
        level_info_server_travel: ex_level_info_server_travel,
        level_info_broadcast_message: ex_level_info_broadcast_message,
        game_script_console_exec: ex_game_script_console_exec,
        pending_net_driver: ex_local_svc_pending_net_driver,
        pending_try_skip_file: ex_pending_try_skip_file,
        net_driver_server_connection: ex_net_driver_server_connection,
        connection_channel0: ex_local_svc_connection_channel0,
        channel_close: ex_local_svc_channel_close,
        connection_flush_net: ex_local_svc_connection_flush_net,
        find_class_any_package: ex_find_class_any_package,
        class_is_child_of_actor: ex_class_is_child_of_actor,
        class_is_child_of_game_info: ex_class_is_child_of_game_info,
        parse_command: ex_parse_command,
        parse_token: ex_parse_token,
        app_atoi: ex_app_atoi,
        app_request_exit: ex_app_request_exit,
        app_launch_url: ex_app_launch_url,
        localize_progress: ex_localize_progress,
        make_save_directory: ex_make_save_directory,
        save_slot_filename: ex_save_slot_filename,
        debugf_log_i: ex_debugf_log_i,
        debugf_log_s: ex_debugf_log_s,
        load_persistent_actor_cache: ex_load_persistent_actor_cache,
        format_load_game_url: ex_format_load_game_url,
        output_log: ex_output_log,
        output_logf_f: ex_output_logf_f,
    }
}
