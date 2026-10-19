//! Replays the original-C++ oracle from `Build/cam_mgr_fixture_harness/` against
//! `hp2_engine::cam_mgr` (`UClient::GetEffectiveFovAngle`, `PostEditChange`, `Exec`, `Flush`).
//!
//! `fixtures/cam_mgr_exec.txt` holds 240 `fov`, 160 `postedit`, 280 `exec` and 120 `flush`
//! records. The mock services replay the harness object graph: `Viewports` has `nv` entries,
//! `Viewports(0)->Actor` is non-null iff `act=1`, and render devices are ids 0..2 (`-1` is
//! null). Each traced callee checks the object it received and records `?client` or `?rd`
//! on a mismatch, so a wrong pointer changes the trace and fails the comparison.

use std::cell::RefCell;
use std::ffi::c_void;

use hp2_engine::cam_mgr::{self, CamMgrServices};
use hp2_engine::params::TChar;

const FIXTURE: &str = include_str!("fixtures/cam_mgr_exec.txt");
const EXPECTED_FOV: usize = 240;
const EXPECTED_POSTEDIT: usize = 160;
const EXPECTED_EXEC: usize = 280;
const EXPECTED_FLUSH: usize = 120;

const CLIENT: usize = 0x1000;
const ACTOR: usize = 0x2000;
const RD_BASE: usize = 0x3000;
const RD_STRIDE: usize = 0x100;
const RD_COUNT: usize = 3;

/// Command table from the fixture header (`cmd=` indexes into this).
const CMDS: [&str; 7] = [
    "BRIGHTNESS",
    "brightness",
    "BRIGHTNESS now",
    "  BRIGHTNESS",
    "BRIGHTNESSX",
    "NOPE",
    "",
];

#[derive(Default)]
struct Mock {
    nv: i32,
    act: bool,
    rd_ids: Vec<i32>,
    trace: Vec<String>,
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::default());
}

fn with<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

/// Drains the trace the way the harness prints it: comma-joined, `-` when empty.
fn take_trace() -> String {
    with(|m| {
        let t = std::mem::take(&mut m.trace);
        if t.is_empty() {
            "-".to_owned()
        } else {
            t.join(",")
        }
    })
}

/// Appends `tok` to the trace, prefixed by `?client` when `client` is not the harness client.
fn record(client: *mut c_void, tok: String) {
    with(|m| {
        if client as usize != CLIENT {
            m.trace.push("?client".to_owned());
        }
        m.trace.push(tok);
    });
}

/// `ParseCommand(&Stream, TEXT("BRIGHTNESS"))`: skip blanks, match the keyword
/// case-insensitively, then require NUL, space or tab.
unsafe fn parse_brightness(cmd: *const TChar) -> i32 {
    let at = |i: usize| unsafe { *cmd.add(i) as u32 };
    let sp = u32::from(b' ');
    let tab = u32::from(b'\t');
    let mut p = 0;
    while at(p) == sp || at(p) == tab {
        p += 1;
    }
    for &want in b"BRIGHTNESS" {
        let mut ch = at(p);
        if (u32::from(b'a')..=u32::from(b'z')).contains(&ch) {
            ch -= 32;
        }
        if ch != u32::from(want) {
            return 0;
        }
        p += 1;
    }
    let end = at(p);
    i32::from(end == 0 || end == sp || end == tab)
}

unsafe extern "C" fn m_parse(cmd: *const TChar) -> i32 {
    unsafe { parse_brightness(cmd) }
}

unsafe extern "C" fn m_engine_flush(client: *mut c_void, allow: i32) {
    record(client, format!("EF{allow}"));
}

unsafe extern "C" fn m_save_config(client: *mut c_void) {
    record(client, "SC".to_owned());
}

unsafe extern "C" fn m_viewports_num(_client: *mut c_void) -> i32 {
    with(|m| m.nv)
}

unsafe extern "C" fn m_viewport0_actor(_client: *mut c_void) -> *mut c_void {
    with(|m| {
        if m.act {
            ACTOR as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    })
}

unsafe extern "C" fn m_brightness_message(client: *mut c_void, level: i32) {
    record(client, format!("MSG:Brightness level {level}/10/b0"));
}

unsafe extern "C" fn m_viewport_render_dev(_client: *mut c_void, index: i32) -> *mut c_void {
    with(|m| match m.rd_ids.get(index as usize) {
        Some(&id) if id >= 0 => (RD_BASE + id as usize * RD_STRIDE) as *mut c_void,
        _ => std::ptr::null_mut(),
    })
}

unsafe extern "C" fn m_render_dev_flush(rd: *mut c_void, allow: i32) {
    let off = (rd as usize).wrapping_sub(RD_BASE);
    let id = off / RD_STRIDE;
    let tok = if off.is_multiple_of(RD_STRIDE) && id < RD_COUNT {
        format!("RF{id}{allow}")
    } else {
        "?rd".to_owned()
    };
    with(|m| m.trace.push(tok));
}

fn services() -> CamMgrServices {
    CamMgrServices {
        parse_brightness_command: m_parse,
        client_engine_flush: m_engine_flush,
        client_save_config: m_save_config,
        client_viewports_num: m_viewports_num,
        client_viewport0_actor: m_viewport0_actor,
        client_brightness_message: m_brightness_message,
        client_viewport_render_dev: m_viewport_render_dev,
        render_dev_flush: m_render_dev_flush,
    }
}

/// NUL-terminated wide copy of a command string (`TCHAR` is `wchar_t`).
fn wide(s: &str) -> Vec<TChar> {
    s.chars()
        .map(|c| c as u32 as TChar)
        .chain(std::iter::once(0))
        .collect()
}

fn field<'a>(tok: &'a str, key: &str) -> &'a str {
    tok.strip_prefix(key)
        .unwrap_or_else(|| panic!("expected `{key}` in `{tok}`"))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("bad hex `{s}`"))
}

fn check_fov(line: &str) {
    let t: Vec<&str> = line.split_whitespace().collect();
    // fov auth= vx= vy= maintain= -> ret=
    let auth = f32::from_bits(hex(field(t[1], "auth=")));
    let vx: i32 = field(t[2], "vx=").parse().unwrap();
    let vy: i32 = field(t[3], "vy=").parse().unwrap();
    let maintain: i32 = field(t[4], "maintain=").parse().unwrap();
    let want = hex(field(t[6], "ret="));
    let got = cam_mgr::get_effective_fov_angle(auth, vx, vy, maintain);
    assert_eq!(got.to_bits(), want, "{line}");
}

fn check_postedit(line: &str) {
    let t: Vec<&str> = line.split_whitespace().collect();
    // postedit bright= mip= density= -> bright= mip= density=
    let mut bright = f32::from_bits(hex(field(t[1], "bright=")));
    let mut mip = f32::from_bits(hex(field(t[2], "mip=")));
    let mut density: i32 = field(t[3], "density=").parse().unwrap();
    unsafe { cam_mgr::post_edit_change(&mut bright, &mut mip, &mut density) };
    assert_eq!(bright.to_bits(), hex(field(t[5], "bright=")), "{line}");
    assert_eq!(mip.to_bits(), hex(field(t[6], "mip=")), "{line}");
    let want_density: i32 = field(t[7], "density=").parse().unwrap();
    assert_eq!(density, want_density, "{line}");
}

fn check_exec(svc: &CamMgrServices, line: &str) {
    // The trace can contain spaces (`MSG:Brightness level ...`), so split it off first.
    let (head, trace) = line.rsplit_once(" trace=").expect("trace field");
    let t: Vec<&str> = head.split_whitespace().collect();
    // exec cmd= bright= nv= act= -> ret= bright=
    let cmd: usize = field(t[1], "cmd=").parse().unwrap();
    let mut bright = f32::from_bits(hex(field(t[2], "bright=")));
    let nv: i32 = field(t[3], "nv=").parse().unwrap();
    let act = field(t[4], "act=") == "1";
    let want_ret: i32 = field(t[6], "ret=").parse().unwrap();
    let want_bright = hex(field(t[7], "bright="));
    with(|m| {
        m.nv = nv;
        m.act = act;
        m.rd_ids.clear();
        m.trace.clear();
    });
    let cmd_buf = wide(CMDS[cmd]);
    let got_ret =
        unsafe { cam_mgr::exec(svc, CLIENT as *mut c_void, &mut bright, cmd_buf.as_ptr()) };
    let got_trace = take_trace();
    assert_eq!(got_ret, want_ret, "{line}");
    assert_eq!(bright.to_bits(), want_bright, "{line}");
    assert_eq!(got_trace, trace, "{line}");
}

fn check_flush(svc: &CamMgrServices, line: &str) {
    let (head, trace) = line.rsplit_once(" trace=").expect("trace field");
    let t: Vec<&str> = head.split_whitespace().collect();
    // flush allow= vps= ->
    let allow: i32 = field(t[1], "allow=").parse().unwrap();
    let vps = field(t[2], "vps=");
    let ids: Vec<i32> = if vps == "-" {
        Vec::new()
    } else {
        vps.split(',').map(|s| s.parse().unwrap()).collect()
    };
    with(|m| {
        m.nv = i32::try_from(ids.len()).unwrap();
        m.act = false;
        m.rd_ids = ids;
        m.trace.clear();
    });
    unsafe { cam_mgr::flush(svc, CLIENT as *mut c_void, allow) };
    assert_eq!(take_trace(), trace, "{line}");
}

#[test]
fn cam_mgr_replay_matches_original() {
    let svc = services();
    let (mut fov, mut post, mut exec, mut flush) = (0, 0, 0, 0);
    for line in FIXTURE.lines().filter(|l| !l.starts_with('#')) {
        match line.split_whitespace().next() {
            Some("fov") => {
                fov += 1;
                check_fov(line);
            }
            Some("postedit") => {
                post += 1;
                check_postedit(line);
            }
            Some("exec") => {
                exec += 1;
                check_exec(&svc, line);
            }
            Some("flush") => {
                flush += 1;
                check_flush(&svc, line);
            }
            other => panic!("unknown record kind {other:?}: {line}"),
        }
    }
    assert_eq!(fov, EXPECTED_FOV, "fov record count");
    assert_eq!(post, EXPECTED_POSTEDIT, "postedit record count");
    assert_eq!(exec, EXPECTED_EXEC, "exec record count");
    assert_eq!(flush, EXPECTED_FLUSH, "flush record count");
}
