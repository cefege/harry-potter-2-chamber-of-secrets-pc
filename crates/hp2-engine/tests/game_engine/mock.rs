//! Mock `Hp2GameServices` for the `UGameEngine` replay tests.
//!
//! The model mirrors the object graph that `Build/game_fixture_harness/game_harness.cpp`
//! builds in static memory: the same fields, the same initial values, and the same
//! trace tokens printed by the C++ hooks. Engine objects are opaque addresses that are
//! never dereferenced. A service the oracle would dereference through a null or unknown
//! pointer panics here, which mirrors the oracle's crash.

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void};

use hp2_engine::game_engine::Hp2GameServices;

pub const ENG: usize = 0x1000;
pub const LEVEL: usize = 0x2000;
pub const LEVELINFO: usize = 0x2100;
pub const CLIENT: usize = 0x3000;
pub const VP0: usize = 0x3100;
pub const VP1: usize = 0x3200;
pub const PAWN: usize = 0x3300;
pub const PENDING: usize = 0x4000;
pub const NETDRV: usize = 0x4100;
pub const PENDNETDRV: usize = 0x4200;
pub const CONN: usize = 0x4300;
pub const CHAN: usize = 0x4400;
pub const FILEAR: usize = 0x4500;

#[derive(Clone, Default)]
pub struct Viewport {
    pub actor: usize,
    pub show_windows_mouse: bool,
    pub wmx: f32,
    pub wmy: f32,
    pub travel_url: Vec<u32>,
    pub travel_type: i32,
    pub b_items: bool,
}

#[derive(Clone, Default)]
pub struct Pawn {
    pub show_menu: bool,
    pub flash_fog: [f32; 4],
    pub msg: [Vec<u32>; 2],
    pub col: [[i32; 3]; 2],
    pub timeout: f32,
}

#[derive(Clone, Default)]
pub struct Model {
    // UGameEngine
    pub g_level: usize,
    pub g_pending: usize,
    pub client: usize,
    pub is_drawing: bool,
    // FReplay
    pub file_ar: usize,
    pub b_recording: bool,
    pub b_in_replay: bool,
    pub b_paused: bool,
    pub pause_count: i32,
    // ULevel / ULevelInfo
    pub level_net_driver: usize,
    pub level_special_pause: bool,
    pub pauser: Vec<u32>,
    pub time_seconds: f32,
    // UPendingLevel, UNetDriver, UNetConnection, UChannel
    pub pending_net_driver: usize,
    pub pending_demo_rec_driver: usize,
    pub pendnetdrv_conn: usize,
    pub conn_channel0: usize,
    // UClient
    pub client_viewports: Vec<usize>,
    pub client_capture_mouse: bool,
    // UViewport, APlayerPawn
    pub vp0: Viewport,
    pub vp1: Viewport,
    pub pawn: Pawn,
    // Base-call return values (`g_key_ret`, `g_ie_ret` in the harness)
    pub key_ret: i32,
    pub ie_ret: i32,
}

thread_local! {
    static MODEL: RefCell<Model> = RefCell::new(Model { key_ret: 1, ie_ret: 1, ..Model::default() });
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
    static CASE: Cell<usize> = const { Cell::new(0) };
}

fn with<R>(f: impl FnOnce(&mut Model) -> R) -> R {
    MODEL.with(|m| f(&mut m.borrow_mut()))
}

/// Mutates the mock model directly, as the harness does with its static objects.
pub fn model<R>(f: impl FnOnce(&mut Model) -> R) -> R {
    with(f)
}

pub fn emit(line: &str) {
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.push_str(line);
        o.push('\n');
    });
}

/// Returns and clears the accumulated oracle-format output.
pub fn take_output() -> String {
    OUT.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

pub fn engine() -> *mut c_void {
    ENG as *mut c_void
}

pub fn ptr(addr: usize) -> *mut c_void {
    addr as *mut c_void
}

pub fn addr(p: *mut c_void) -> usize {
    p as usize
}

/// Harness `nm()`: names for the fake objects, `null` for zero, `other` otherwise.
pub fn nm(a: usize) -> &'static str {
    match a {
        0 => "null",
        ENG => "eng",
        LEVEL => "level",
        LEVELINFO => "levelinfo",
        CLIENT => "client",
        VP0 => "vp0",
        VP1 => "vp1",
        PAWN => "pawn",
        PENDING => "pending",
        NETDRV => "netdrv",
        PENDNETDRV => "pendnetdrv",
        CONN => "conn",
        CHAN => "chan",
        FILEAR => "filear",
        _ => "other",
    }
}

pub fn fbits(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn fog_bits(f: &[f32; 4]) -> String {
    f.iter().map(|x| fbits(*x)).collect::<Vec<_>>().join(",")
}

/// Harness `pfs()`: quoted, non-ASCII code units as `?`.
pub fn pfs(s: &[u32]) -> String {
    let body: String = s
        .iter()
        .map(|&c| if c < 128 { char::from(c as u8) } else { '?' })
        .collect();
    format!("\"{body}\"")
}

/// TCHAR text without a terminator (for `FString` members).
pub fn tc(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

/// NUL-terminated TCHAR text, as `TEXT("...")` is passed to `SetProgress`.
pub fn tcz(s: &str) -> Vec<u32> {
    let mut v = tc(s);
    v.push(0);
    v
}

pub fn read_tchar(text: *const u32) -> Vec<u32> {
    let mut out = Vec::new();
    // SAFETY: callers pass a NUL-terminated TCHAR buffer that outlives the call.
    unsafe {
        let mut p = text;
        while *p != 0 {
            out.push(*p);
            p = p.add(1);
        }
    }
    out
}

fn vp_mut(m: &mut Model, vp: usize) -> &mut Viewport {
    match vp {
        VP0 => &mut m.vp0,
        VP1 => &mut m.vp1,
        _ => panic!("not a live viewport: {vp:#x}"),
    }
}

pub fn check_eng(engine: *mut c_void) {
    assert_eq!(addr(engine), ENG, "engine handle");
}

/// Harness `begin()`: prints the case header and advances the index.
pub fn begin(knobs: &str) {
    begin_family("group1", knobs);
}

/// Harness `begin()` for a family: prints the case header and advances the index.
pub fn begin_family(family: &str, knobs: &str) {
    let n = CASE.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    emit(&format!("case {family} {n} {knobs}"));
}

/// Harness `result %d`.
pub fn result(r: i32) {
    emit(&format!("result {r}"));
}

/// Harness `dump_state()`: one `state ...` line over the whole model.
pub fn dump_state() {
    let line = with(|m| {
        let mut s = String::from("state");
        s += &format!(
            " eng.GLevel={} eng.GPendingLevel={} eng.Client={} eng.GIsDrawing={}",
            nm(m.g_level),
            nm(m.g_pending),
            nm(m.client),
            m.is_drawing as i32
        );
        s += &format!(
            " replay.FileAr={} replay.bRecording={} replay.bInReplay={} replay.bPaused={} replay.PauseCount={}",
            nm(m.file_ar),
            m.b_recording as i32,
            m.b_in_replay as i32,
            m.b_paused as i32,
            m.pause_count
        );
        s += &format!(
            " level.NetDriver={} level.bInSpecialPauseMode={}",
            nm(m.level_net_driver),
            m.level_special_pause as i32
        );
        s += &format!(
            " pending.NetDriver={} pendnetdrv.ServerConnection={}",
            nm(m.pending_net_driver),
            nm(m.pendnetdrv_conn)
        );
        s += &format!(" conn.Channels0={}", nm(m.conn_channel0));
        s += &format!(
            " client.Viewports={} client.CaptureMouse={}",
            m.client_viewports.len(),
            m.client_capture_mouse as i32
        );
        s += &format!(
            " vp0.Actor={} vp0.bShowWindowsMouse={} vp0.WindowsMouseX={} vp0.WindowsMouseY={}",
            nm(m.vp0.actor),
            m.vp0.show_windows_mouse as i32,
            fbits(m.vp0.wmx),
            fbits(m.vp0.wmy)
        );
        s += &format!(" vp1.Actor={}", nm(m.vp1.actor));
        s += &format!(
            " pawn.bShowMenu={} pawn.FlashFog={}",
            m.pawn.show_menu as i32,
            fog_bits(&m.pawn.flash_fog)
        );
        s += &format!(
            " pawn.Msg0={} pawn.Col0={},{},{}",
            pfs(&m.pawn.msg[0]),
            m.pawn.col[0][0],
            m.pawn.col[0][1],
            m.pawn.col[0][2]
        );
        s += &format!(
            " pawn.Msg1={} pawn.Col1={},{},{}",
            pfs(&m.pawn.msg[1]),
            m.pawn.col[1][0],
            m.pawn.col[1][1],
            m.pawn.col[1][2]
        );
        s += &format!(" pawn.ProgressTimeOut={}", fbits(m.pawn.timeout));
        s += &format!(" levelinfo.Pauser={}", pfs(&m.pauser));
        s += &format!(" levelinfo.TimeSeconds={}", fbits(m.time_seconds));
        s
    });
    emit(&line);
}

/// Harness `reset_world()` + `world_base()`: level, client, vp0, and pawn wired.
/// The base-call return values survive the reset, as the harness globals do.
pub fn world_base(add_vp0: bool) {
    with(|m| {
        let (key_ret, ie_ret) = (m.key_ret, m.ie_ret);
        *m = Model {
            key_ret,
            ie_ret,
            ..Model::default()
        };
        m.g_level = LEVEL;
        m.client = CLIENT;
        m.time_seconds = 1.25;
        if add_vp0 {
            m.client_viewports.push(VP0);
            m.vp0.actor = PAWN;
        }
        m.pawn.flash_fog = [0.5, 0.25, 0.125, 1.0];
        m.pawn.show_menu = false;
        m.vp0.show_windows_mouse = false;
        m.vp0.wmx = 0.0;
        m.vp0.wmy = 0.0;
    });
}

/// Harness `set_pending(net, conn, chan)`.
pub fn set_pending(net: bool, conn: bool, chan: bool) {
    with(|m| {
        m.g_pending = PENDING;
        if net {
            m.pending_net_driver = PENDNETDRV;
            if conn {
                m.pendnetdrv_conn = CONN;
                if chan {
                    m.conn_channel0 = CHAN;
                }
            }
        }
    });
}

/// Harness `set_replay(recording, playing, in_replay, paused, pause_count)`.
pub fn set_replay(recording: bool, playing: bool, in_replay: bool, paused: bool, pause_count: i32) {
    with(|m| {
        if recording || playing {
            m.file_ar = FILEAR;
            m.b_recording = recording;
        }
        m.b_in_replay = in_replay;
        m.b_paused = paused;
        m.pause_count = pause_count;
    });
}

/// Harness `g_key_ret`.
pub fn set_key_ret(v: i32) {
    with(|m| m.key_ret = v);
}

/// Harness `g_ie_ret`.
pub fn set_ie_ret(v: i32) {
    with(|m| m.ie_ret = v);
}

// ---- services ----

pub unsafe extern "C" fn svc_level(engine: *mut c_void) -> *mut c_void {
    check_eng(engine);
    ptr(with(|m| m.g_level))
}

unsafe extern "C" fn svc_set_level(engine: *mut c_void, v: *mut c_void) {
    check_eng(engine);
    with(|m| m.g_level = addr(v));
}

pub unsafe extern "C" fn svc_pending(engine: *mut c_void) -> *mut c_void {
    check_eng(engine);
    ptr(with(|m| m.g_pending))
}

unsafe extern "C" fn svc_set_pending(engine: *mut c_void, v: *mut c_void) {
    check_eng(engine);
    with(|m| m.g_pending = addr(v));
}

pub unsafe extern "C" fn svc_client(engine: *mut c_void) -> *mut c_void {
    check_eng(engine);
    ptr(with(|m| m.client))
}

pub unsafe extern "C" fn svc_client_viewport_count(client: *mut c_void) -> i32 {
    assert_eq!(addr(client), CLIENT, "UClient");
    with(|m| m.client_viewports.len() as i32)
}

pub unsafe extern "C" fn svc_client_viewport0(client: *mut c_void) -> *mut c_void {
    assert_eq!(addr(client), CLIENT, "UClient");
    ptr(with(|m| m.client_viewports[0]))
}

unsafe extern "C" fn svc_client_capture_mouse(client: *mut c_void) -> i32 {
    assert_eq!(addr(client), CLIENT, "UClient");
    with(|m| m.client_capture_mouse as i32)
}

unsafe extern "C" fn svc_viewport_actor(viewport: *mut c_void) -> *mut c_void {
    ptr(with(|m| vp_mut(m, addr(viewport)).actor))
}

unsafe extern "C" fn svc_viewport_show_windows_mouse(viewport: *mut c_void) -> i32 {
    with(|m| vp_mut(m, addr(viewport)).show_windows_mouse as i32)
}

unsafe extern "C" fn svc_viewport_set_mouse_capture(
    viewport: *mut c_void,
    capture: i32,
    exclusive: i32,
    foreground: i32,
) {
    emit(&format!(
        "set_mouse_capture {} {capture} {exclusive} {foreground}",
        nm(addr(viewport))
    ));
}

unsafe extern "C" fn svc_viewport_set_windows_mouse(viewport: *mut c_void, x: f32, y: f32) {
    with(|m| {
        let vp = vp_mut(m, addr(viewport));
        vp.wmx = x;
        vp.wmy = y;
    });
}

pub unsafe extern "C" fn svc_level_net_driver(level: *mut c_void) -> *mut c_void {
    assert_eq!(addr(level), LEVEL, "ULevel");
    ptr(with(|m| m.level_net_driver))
}

unsafe extern "C" fn svc_level_set_net_driver(level: *mut c_void, v: *mut c_void) {
    assert_eq!(addr(level), LEVEL, "ULevel");
    with(|m| m.level_net_driver = addr(v));
}

pub unsafe extern "C" fn svc_level_info(level: *mut c_void) -> *mut c_void {
    assert_eq!(addr(level), LEVEL, "ULevel");
    ptr(LEVELINFO)
}

unsafe extern "C" fn svc_level_info_pauser_empty(info: *mut c_void) -> i32 {
    assert_eq!(addr(info), LEVELINFO, "ULevelInfo");
    with(|m| m.pauser.is_empty() as i32)
}

unsafe extern "C" fn svc_level_toggle_special_pause(level: *mut c_void) {
    assert_eq!(addr(level), LEVEL, "ULevel");
    with(|m| m.level_special_pause = !m.level_special_pause);
}

unsafe extern "C" fn svc_pawn_show_menu(actor: *mut c_void) -> i32 {
    assert_eq!(addr(actor), PAWN, "APlayerPawn");
    with(|m| m.pawn.show_menu as i32)
}

unsafe extern "C" fn svc_pawn_show_upgrade_menu(actor: *mut c_void) {
    assert_eq!(addr(actor), PAWN, "APlayerPawn");
    emit(&format!("pawn_show_upgrade_menu {}", nm(addr(actor))));
}

unsafe extern "C" fn svc_pawn_set_progress(actor: *mut c_void, slot: i32, text: *const u32) {
    assert_eq!(addr(actor), PAWN, "APlayerPawn");
    let s = read_tchar(text);
    with(|m| {
        let i = slot as usize;
        m.pawn.msg[i] = s;
        m.pawn.col[i] = [255, 255, 255];
    });
}

unsafe extern "C" fn svc_pawn_set_progress_timeout(actor: *mut c_void, seconds: f32) {
    assert_eq!(addr(actor), PAWN, "APlayerPawn");
    with(|m| m.pawn.timeout = m.time_seconds + seconds);
}

unsafe extern "C" fn svc_net_driver_delete(driver: *mut c_void) {
    emit(&format!("net_driver_delete {}", nm(addr(driver))));
}

pub unsafe extern "C" fn svc_pending_net_driver(pending: *mut c_void) -> *mut c_void {
    assert_eq!(addr(pending), PENDING, "UPendingLevel");
    ptr(with(|m| m.pending_net_driver))
}

unsafe extern "C" fn svc_net_driver_server_connection(driver: *mut c_void) -> *mut c_void {
    let conn = match addr(driver) {
        PENDNETDRV => with(|m| m.pendnetdrv_conn),
        // The live net driver never gets a server connection in this group.
        NETDRV => 0,
        other => panic!("not a live net driver: {other:#x}"),
    };
    ptr(conn)
}

pub unsafe extern "C" fn svc_connection_channel0(conn: *mut c_void) -> *mut c_void {
    assert_eq!(addr(conn), CONN, "UNetConnection");
    ptr(with(|m| m.conn_channel0))
}

pub unsafe extern "C" fn svc_channel_close(channel: *mut c_void) {
    emit(&format!("channel_close {}", nm(addr(channel))));
}

pub unsafe extern "C" fn svc_connection_flush_net(conn: *mut c_void) {
    emit(&format!("connection_flush_net {}", nm(addr(conn))));
}

unsafe extern "C" fn svc_delete_pending(pending: *mut c_void) {
    emit(&format!("delete_pending {}", nm(addr(pending))));
}

unsafe extern "C" fn svc_engine_exit_base(engine: *mut c_void) {
    check_eng(engine);
    emit("engine_exit_base");
}

unsafe extern "C" fn svc_engine_destroy_base(engine: *mut c_void) {
    check_eng(engine);
    emit("engine_destroy_base");
}

unsafe extern "C" fn svc_engine_key_base(
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
) -> i32 {
    check_eng(engine);
    emit(&format!("engine_key_base {} {key}", nm(addr(viewport))));
    with(|m| m.key_ret)
}

unsafe extern "C" fn svc_engine_input_event_base(
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    check_eng(engine);
    emit(&format!(
        "engine_input_event_base {} {key} {state} {}",
        nm(addr(viewport)),
        fbits(delta)
    ));
    with(|m| m.ie_ret)
}

unsafe extern "C" fn svc_log_game_shut_down() {
    emit("log_game_shut_down");
}

unsafe extern "C" fn svc_replay_recording(engine: *mut c_void) -> i32 {
    check_eng(engine);
    with(|m| (m.file_ar != 0 && m.b_recording) as i32)
}

unsafe extern "C" fn svc_replay_replaying(engine: *mut c_void) -> i32 {
    check_eng(engine);
    with(|m| (m.file_ar != 0 && !m.b_recording) as i32)
}

unsafe extern "C" fn svc_replay_in_replay(engine: *mut c_void) -> i32 {
    check_eng(engine);
    with(|m| m.b_in_replay as i32)
}

unsafe extern "C" fn svc_replay_paused(engine: *mut c_void) -> i32 {
    check_eng(engine);
    with(|m| (m.b_paused && m.pause_count == 0) as i32)
}

unsafe extern "C" fn svc_replay_stop(engine: *mut c_void) {
    check_eng(engine);
    emit("replay_stop");
    with(|m| {
        m.file_ar = 0;
        m.b_paused = false;
    });
}

unsafe extern "C" fn svc_replay_pause(engine: *mut c_void, on: i32) {
    check_eng(engine);
    // FReplay::Pause(bPause, Count = 0)
    with(|m| {
        m.b_paused = on != 0;
        m.pause_count = 0;
    });
}

unsafe extern "C" fn svc_replay_pause_step(engine: *mut c_void) {
    check_eng(engine);
    // FReplay::Pause(true, 1)
    with(|m| {
        m.b_paused = true;
        m.pause_count = 1;
    });
}

unsafe extern "C" fn svc_replay_serialize_input(
    engine: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) {
    check_eng(engine);
    with(|m| {
        if m.file_ar != 0 {
            m.b_in_replay = true;
        }
    });
    emit(&format!(
        "serialize_frame_input {key} {state} {}",
        fbits(delta)
    ));
}

unsafe extern "C" fn svc_engine_is_drawing(engine: *mut c_void) -> i32 {
    check_eng(engine);
    with(|m| m.is_drawing as i32)
}

unsafe extern "C" fn svc_draw_with_load_fog(engine: *mut c_void, viewport: *mut c_void) {
    check_eng(engine);
    let vp = addr(viewport);
    with(|m| {
        assert_eq!(vp_mut(m, vp).actor, PAWN, "Draw: viewport has no pawn");
        // FPlane LoadFog(0.f,0.1f,0.25f,0.2f); Exchange(Actor->FlashFog, LoadFog);
        let load_fog = [0.0_f32, 0.1, 0.25, 0.2];
        let saved = std::mem::replace(&mut m.pawn.flash_fog, load_fog);
        // EngDraw prints the actor's FlashFog, which now holds LoadFog.
        emit(&format!(
            "draw {} fog={}",
            nm(vp),
            fog_bits(&m.pawn.flash_fog)
        ));
        // Exchange back.
        m.pawn.flash_fog = saved;
    });
}

unsafe extern "C" fn checked_level(engine: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_level(engine);
    }
    0
}

unsafe extern "C" fn checked_set_level(engine: *mut c_void, v: *mut c_void) -> i32 {
    unsafe {
        svc_set_level(engine, v);
    }
    0
}

unsafe extern "C" fn checked_pending(engine: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_pending(engine);
    }
    0
}

unsafe extern "C" fn checked_set_pending(engine: *mut c_void, v: *mut c_void) -> i32 {
    unsafe {
        svc_set_pending(engine, v);
    }
    0
}

unsafe extern "C" fn checked_client(engine: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_client(engine);
    }
    0
}

unsafe extern "C" fn checked_client_viewport_count(client: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_client_viewport_count(client);
    }
    0
}

unsafe extern "C" fn checked_client_viewport0(client: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_client_viewport0(client);
    }
    0
}

unsafe extern "C" fn checked_client_capture_mouse(client: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_client_capture_mouse(client);
    }
    0
}

unsafe extern "C" fn checked_viewport_actor(viewport: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_viewport_actor(viewport);
    }
    0
}

unsafe extern "C" fn checked_viewport_show_windows_mouse(
    viewport: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = svc_viewport_show_windows_mouse(viewport);
    }
    0
}

unsafe extern "C" fn checked_viewport_set_mouse_capture(
    viewport: *mut c_void,
    capture: i32,
    exclusive: i32,
    foreground: i32,
) -> i32 {
    unsafe {
        svc_viewport_set_mouse_capture(viewport, capture, exclusive, foreground);
    }
    0
}

unsafe extern "C" fn checked_viewport_set_windows_mouse(
    viewport: *mut c_void,
    x: f32,
    y: f32,
) -> i32 {
    unsafe {
        svc_viewport_set_windows_mouse(viewport, x, y);
    }
    0
}

unsafe extern "C" fn checked_level_net_driver(level: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_level_net_driver(level);
    }
    0
}

unsafe extern "C" fn checked_level_set_net_driver(level: *mut c_void, v: *mut c_void) -> i32 {
    unsafe {
        svc_level_set_net_driver(level, v);
    }
    0
}

unsafe extern "C" fn checked_level_info(level: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_level_info(level);
    }
    0
}

unsafe extern "C" fn checked_level_info_pauser_empty(info: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_level_info_pauser_empty(info);
    }
    0
}

unsafe extern "C" fn checked_level_toggle_special_pause(level: *mut c_void) -> i32 {
    unsafe {
        svc_level_toggle_special_pause(level);
    }
    0
}

unsafe extern "C" fn checked_pawn_show_menu(actor: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_pawn_show_menu(actor);
    }
    0
}

unsafe extern "C" fn checked_pawn_show_upgrade_menu(actor: *mut c_void) -> i32 {
    unsafe {
        svc_pawn_show_upgrade_menu(actor);
    }
    0
}

unsafe extern "C" fn checked_pawn_set_progress(
    actor: *mut c_void,
    slot: i32,
    text: *const u32,
) -> i32 {
    unsafe {
        svc_pawn_set_progress(actor, slot, text);
    }
    0
}

unsafe extern "C" fn checked_pawn_set_progress_timeout(actor: *mut c_void, seconds: f32) -> i32 {
    unsafe {
        svc_pawn_set_progress_timeout(actor, seconds);
    }
    0
}

unsafe extern "C" fn checked_net_driver_delete(driver: *mut c_void) -> i32 {
    unsafe {
        svc_net_driver_delete(driver);
    }
    0
}

unsafe extern "C" fn checked_pending_net_driver(
    pending: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = svc_pending_net_driver(pending);
    }
    0
}

unsafe extern "C" fn checked_net_driver_server_connection(
    driver: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = svc_net_driver_server_connection(driver);
    }
    0
}

unsafe extern "C" fn checked_connection_channel0(conn: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = svc_connection_channel0(conn);
    }
    0
}

unsafe extern "C" fn checked_channel_close(channel: *mut c_void) -> i32 {
    unsafe {
        svc_channel_close(channel);
    }
    0
}

unsafe extern "C" fn checked_connection_flush_net(conn: *mut c_void) -> i32 {
    unsafe {
        svc_connection_flush_net(conn);
    }
    0
}

unsafe extern "C" fn checked_delete_pending(pending: *mut c_void) -> i32 {
    unsafe {
        svc_delete_pending(pending);
    }
    0
}

unsafe extern "C" fn checked_engine_exit_base(engine: *mut c_void) -> i32 {
    unsafe {
        svc_engine_exit_base(engine);
    }
    0
}

unsafe extern "C" fn checked_engine_destroy_base(engine: *mut c_void) -> i32 {
    unsafe {
        svc_engine_destroy_base(engine);
    }
    0
}

unsafe extern "C" fn checked_engine_key_base(
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = svc_engine_key_base(engine, viewport, key);
    }
    0
}

unsafe extern "C" fn checked_engine_input_event_base(
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = svc_engine_input_event_base(engine, viewport, key, state, delta);
    }
    0
}

unsafe extern "C" fn checked_log_game_shut_down() -> i32 {
    unsafe {
        svc_log_game_shut_down();
    }
    0
}

unsafe extern "C" fn checked_replay_recording(engine: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_replay_recording(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_replaying(engine: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_replay_replaying(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_in_replay(engine: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_replay_in_replay(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_paused(engine: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_replay_paused(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_stop(engine: *mut c_void) -> i32 {
    unsafe {
        svc_replay_stop(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_pause(engine: *mut c_void, on: i32) -> i32 {
    unsafe {
        svc_replay_pause(engine, on);
    }
    0
}

unsafe extern "C" fn checked_replay_pause_step(engine: *mut c_void) -> i32 {
    unsafe {
        svc_replay_pause_step(engine);
    }
    0
}

unsafe extern "C" fn checked_replay_serialize_input(
    engine: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    unsafe {
        svc_replay_serialize_input(engine, key, state, delta);
    }
    0
}

unsafe extern "C" fn checked_engine_is_drawing(engine: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_engine_is_drawing(engine);
    }
    0
}

unsafe extern "C" fn checked_draw_with_load_fog(engine: *mut c_void, viewport: *mut c_void) -> i32 {
    unsafe {
        svc_draw_with_load_fog(engine, viewport);
    }
    0
}

unsafe extern "C" fn checked_engine_cancel_pending(engine: *mut c_void) -> i32 {
    match unsafe { hp2_engine::game_engine::cancel_pending(&services(), engine) } {
        Ok(()) => 0,
        Err(status) => status,
    }
}

pub unsafe extern "C" fn svc_pending_demo_rec_driver(pending: *mut c_void) -> *mut c_void {
    assert_eq!(addr(pending), PENDING, "UPendingLevel");
    ptr(with(|m| m.pending_demo_rec_driver))
}

unsafe extern "C" fn checked_pending_demo_rec_driver(
    pending: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        *out = svc_pending_demo_rec_driver(pending);
    }
    0
}

unsafe extern "C" fn svc_pawn_progress_expired(actor: *mut c_void) -> i32 {
    assert_eq!(addr(actor), PAWN, "APlayerPawn");
    with(|m| (m.pawn.timeout < m.time_seconds) as i32)
}

unsafe extern "C" fn checked_pawn_progress_expired(actor: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = svc_pawn_progress_expired(actor);
    }
    0
}

unsafe extern "C" fn svc_check_failed(expr: *const c_char, line: i32) {
    let text = unsafe { std::ffi::CStr::from_ptr(expr) }.to_string_lossy();
    emit(&format!("check_failed {text} {line}"));
}

unsafe extern "C" fn checked_check_failed(expr: *const c_char, line: i32) -> i32 {
    unsafe {
        svc_check_failed(expr, line);
    }
    1
}

unsafe extern "C" fn svc_viewport_cast_set_travel(
    player: *mut c_void,
    next_url: *const u32,
    travel_type: i32,
    b_items: i32,
) {
    let text = unsafe { read_tchar(next_url) };
    with(|m| {
        let vp = vp_mut(m, addr(player));
        vp.travel_url = text;
        vp.travel_type = travel_type;
        vp.b_items = b_items != 0;
    });
}

unsafe extern "C" fn checked_viewport_cast_set_travel(
    player: *mut c_void,
    next_url: *const u32,
    travel_type: i32,
    b_items: i32,
) -> i32 {
    unsafe {
        svc_viewport_cast_set_travel(player, next_url, travel_type, b_items);
    }
    0
}

fn write_tcz(dst: *mut u32, s: &str) {
    let v = tcz(s);
    // SAFETY: callers pass a 256-`u32` destination buffer, well over any mock string.
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), dst, v.len()) };
}

unsafe extern "C" fn svc_pending_format_demo_message(pending: *mut c_void, msg1: *mut u32, msg2: *mut u32) {
    assert_eq!(addr(pending), PENDING, "UPendingLevel");
    write_tcz(msg1, "");
    write_tcz(msg2, "mock-demo-map");
}

unsafe extern "C" fn checked_pending_format_demo_message(
    pending: *mut c_void,
    msg1: *mut u32,
    msg2: *mut u32,
) -> i32 {
    unsafe {
        svc_pending_format_demo_message(pending, msg1, msg2);
    }
    0
}

unsafe extern "C" fn svc_pending_format_connecting_message(
    pending: *mut c_void,
    msg1: *mut u32,
    msg2: *mut u32,
) {
    assert_eq!(addr(pending), PENDING, "UPendingLevel");
    write_tcz(msg1, "mock-connecting-text");
    write_tcz(msg2, "mock-connecting-url");
}

unsafe extern "C" fn checked_pending_format_connecting_message(
    pending: *mut c_void,
    msg1: *mut u32,
    msg2: *mut u32,
) -> i32 {
    unsafe {
        svc_pending_format_connecting_message(pending, msg1, msg2);
    }
    0
}

/// Table of the mock services, one entry per `Hp2GameServices` field.
pub fn services() -> Hp2GameServices {
    Hp2GameServices {
        level: checked_level,
        set_level: checked_set_level,
        pending: checked_pending,
        set_pending: checked_set_pending,
        client: checked_client,
        client_viewport_count: checked_client_viewport_count,
        client_viewport0: checked_client_viewport0,
        client_capture_mouse: checked_client_capture_mouse,
        viewport_actor: checked_viewport_actor,
        viewport_show_windows_mouse: checked_viewport_show_windows_mouse,
        viewport_set_mouse_capture: checked_viewport_set_mouse_capture,
        viewport_set_windows_mouse: checked_viewport_set_windows_mouse,
        level_net_driver: checked_level_net_driver,
        level_set_net_driver: checked_level_set_net_driver,
        level_info: checked_level_info,
        level_info_pauser_empty: checked_level_info_pauser_empty,
        level_toggle_special_pause: checked_level_toggle_special_pause,
        pawn_show_menu: checked_pawn_show_menu,
        pawn_show_upgrade_menu: checked_pawn_show_upgrade_menu,
        pawn_set_progress: checked_pawn_set_progress,
        pawn_set_progress_timeout: checked_pawn_set_progress_timeout,
        net_driver_delete: checked_net_driver_delete,
        pending_net_driver: checked_pending_net_driver,
        net_driver_server_connection: checked_net_driver_server_connection,
        connection_channel0: checked_connection_channel0,
        channel_close: checked_channel_close,
        connection_flush_net: checked_connection_flush_net,
        delete_pending: checked_delete_pending,
        engine_exit_base: checked_engine_exit_base,
        engine_destroy_base: checked_engine_destroy_base,
        engine_key_base: checked_engine_key_base,
        engine_input_event_base: checked_engine_input_event_base,
        log_game_shut_down: checked_log_game_shut_down,
        replay_recording: checked_replay_recording,
        replay_replaying: checked_replay_replaying,
        replay_in_replay: checked_replay_in_replay,
        replay_paused: checked_replay_paused,
        replay_stop: checked_replay_stop,
        replay_pause: checked_replay_pause,
        replay_pause_step: checked_replay_pause_step,
        replay_serialize_input: checked_replay_serialize_input,
        engine_is_drawing: checked_engine_is_drawing,
        draw_with_load_fog: checked_draw_with_load_fog,
        engine_cancel_pending: checked_engine_cancel_pending,
        check_failed: checked_check_failed,
        viewport_cast_set_travel: checked_viewport_cast_set_travel,
        pending_demo_rec_driver: checked_pending_demo_rec_driver,
        pawn_progress_expired: checked_pawn_progress_expired,
        pending_format_demo_message: checked_pending_format_demo_message,
        pending_format_connecting_message: checked_pending_format_connecting_message,
    }
}
