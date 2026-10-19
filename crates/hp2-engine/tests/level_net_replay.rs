//! Replays the net scenarios of `fixtures/level_oracle.txt` against
//! `hp2_engine::level_net`.
//!
//! Each `net.*` scenario runs the original `ULevel` Net body with the services
//! that scenario reaches. Every other service panics if called. The expected
//! fixture record sequence and `=>` return value come from the fixture.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use hp2_abi::engine::generated::ULevel;
use hp2_engine::level_net::{self, Hp2LevelNetServices};

const FIXTURE: &str = include_str!("fixtures/level_oracle.txt");

/// Number of `Hp2LevelNetServices` fields (all `unsafe extern "C" fn` pointers).
const SERVICE_COUNT: usize = 114;

/// `"Engine.Level"` as a NUL-terminated TCHAR buffer (`object_full_name`).
static ENGINE_LEVEL: [u32; 13] = [
    'E' as u32, 'n' as u32, 'g' as u32, 'i' as u32, 'n' as u32, 'e' as u32, '.' as u32, 'L' as u32,
    'e' as u32, 'v' as u32, 'e' as u32, 'l' as u32, 0,
];
static DEMO1: [u32; 10] = [
    'd' as u32, 'e' as u32, 'm' as u32, 'o' as u32, '1' as u32, '.' as u32, 'd' as u32, 'e' as u32,
    'm' as u32, 0,
];
static DEMO2: [u32; 10] = [
    'd' as u32, 'e' as u32, 'm' as u32, 'o' as u32, '2' as u32, '.' as u32, 'd' as u32, 'e' as u32,
    'm' as u32, 0,
];
static DEM_SUFFIX: [u32; 5] = ['.' as u32, 'd' as u32, 'e' as u32, 'm' as u32, 0];
static PORT_BUSY: [u32; 10] = [
    'P' as u32, 'o' as u32, 'r' as u32, 't' as u32, ' ' as u32, 'b' as u32, 'u' as u32, 's' as u32,
    'y' as u32, 0,
];
static LEVEL_NAME: [u32; 6] = [
    'L' as u32, 'e' as u32, 'v' as u32, 'e' as u32, 'l' as u32, 0,
];
static TIMESTAMP: [u32; 21] = [
    '2' as u32, '0' as u32, '0' as u32, '4' as u32, '.' as u32, '0' as u32, '5' as u32, '.' as u32,
    '0' as u32, '6' as u32, '-' as u32, '1' as u32, '4' as u32, '.' as u32, '3' as u32, '1' as u32,
    '.' as u32, '0' as u32, '7' as u32, 0, 0,
];
static REMOTE_ADDRESS: [u32; 15] = [
    '1' as u32, '9' as u32, '2' as u32, '.' as u32, '0' as u32, '.' as u32, '2' as u32, '.' as u32,
    '7' as u32, ':' as u32, '7' as u32, '7' as u32, '7' as u32, '7' as u32, 0,
];
static EMPTY_TEXT: [u32; 1] = [0];
static SWITCH_URL: [u32; 2] = ['x' as u32, 0];

thread_local! {
    static SERVER_CONNECTION: Cell<bool> = const { Cell::new(false) };
    static CHANNEL_TYPE: Cell<i32> = const { Cell::new(0) };
    static CHANNEL_INDEX: Cell<i32> = const { Cell::new(0) };
    static EXEC_NET_RESULT: Cell<i32> = const { Cell::new(0) };
    static EXEC_DEMO_RESULT: Cell<i32> = const { Cell::new(0) };
    static EXEC_COMMAND: RefCell<String> = const { RefCell::new(String::new()) };
    static DEMO_FILE: Cell<bool> = const { Cell::new(false) };
    static DEMO_INIT_OK: Cell<bool> = const { Cell::new(false) };
    static LINKER_PRESENT: Cell<bool> = const { Cell::new(false) };
    static CLIENT_PRESENT: Cell<bool> = const { Cell::new(false) };
    static ACTOR_MODE: Cell<i32> = const { Cell::new(0) };
    static PARSE_TOKEN_CALL: Cell<i32> = const { Cell::new(0) };
    static FIELD_ITER_VALID: Cell<bool> = const { Cell::new(false) };
    static NEXT_URL_NONEMPTY: Cell<bool> = const { Cell::new(false) };
    static LEVEL_NAMED: Cell<bool> = const { Cell::new(false) };
    static LONE_PLAYER: Cell<i32> = const { Cell::new(0) };
    static NET_MODE: Cell<u8> = const { Cell::new(0) };
    static COUNTDOWN: Cell<f32> = const { Cell::new(0.0) };
    static TRAVEL_PAUSE: Cell<f32> = const { Cell::new(0.0) };
    static ERROR_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
    static FSTRING_CALL: Cell<usize> = const { Cell::new(0) };
    static FSTRING_BUFFERS: RefCell<[[u32; 128]; 2]> = const { RefCell::new([[0; 128]; 2]) };
    static PRELOGIN_REJECT: Cell<bool> = const { Cell::new(false) };
    static REQUEST_URL: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static CLIENT_TRAVEL: RefCell<String> = const { RefCell::new(String::new()) };
    static NET_SPEED: Cell<i32> = const { Cell::new(0) };
    static CHALLENGE_RESPONSE: Cell<i32> = const { Cell::new(0) };
    static DEBUGF: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

const _: () = assert!(
    std::mem::size_of::<Hp2LevelNetServices>() == SERVICE_COUNT * std::mem::size_of::<usize>()
);

unsafe extern "C" fn unused() {
    panic!("level_net replay: unexpected service call");
}

/// Reads a NUL-terminated TCHAR (`u32`) buffer into a `String`.
unsafe fn decode(text: *const u32) -> String {
    let mut out = String::new();
    let mut p = text;
    loop {
        // SAFETY: level_net passes NUL-terminated TCHAR buffers.
        let c = unsafe { *p };
        if c == 0 {
            break;
        }
        out.push(char::from_u32(c).expect("TCHAR"));
        // SAFETY: stays within the NUL-terminated buffer.
        p = unsafe { p.add(1) };
    }
    out
}

unsafe extern "C" fn debugf_dev_net(text: *const u32) {
    // SAFETY: forwarded from level_net.
    let s = unsafe { decode(text) };
    DEBUGF.with(|d| d.borrow_mut().push(format!("debugf\tdev_net\t{s}")));
}

unsafe extern "C" fn netdriver_server_connection(_drv: *mut c_void) -> *mut c_void {
    if SERVER_CONNECTION.with(Cell::get) {
        0x30 as *mut c_void
    } else {
        std::ptr::null_mut()
    }
}

fn wide(text: &str) -> Vec<u32> {
    text.encode_utf16().map(u32::from).chain([0]).collect()
}
unsafe extern "C" fn output_device_log(_ar: *mut c_void, text: *const u32) {
    // SAFETY: level_net forwards a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn parse_command(stream: *mut *const u32, keyword: *const u32) -> i32 {
    let command = EXEC_COMMAND.with(|c| c.borrow().clone());
    // SAFETY: keyword is a NUL-terminated TCHAR string.
    let keyword = unsafe { decode(keyword) };
    let Some(rest) = command.strip_prefix(&keyword) else {
        return 0;
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return 0;
    }
    // SAFETY: the command buffer remains alive for the duration of the call.
    unsafe { *stream = (*stream).add(keyword.encode_utf16().count()) };
    1
}

unsafe extern "C" fn parse_token_fstring(
    _stream: *mut *const u32,
    _token: *mut c_void,
    _escape: i32,
) -> i32 {
    DEMO_FILE.with(Cell::get) as i32
}

extern "C" fn fake_handle() -> *mut c_void {
    0x80 as *mut c_void
}

unsafe extern "C" fn furl_map(_url: *mut c_void) -> *mut c_void {
    if DEMO_FILE.with(Cell::get) {
        if EXEC_COMMAND.with(|c| c.borrow().contains("demo1")) {
            DEMO1.as_ptr() as *mut c_void
        } else {
            DEMO2.as_ptr() as *mut c_void
        }
    } else {
        fake_handle()
    }
}

unsafe extern "C" fn no_delete(_handle: *mut c_void) {}

unsafe extern "C" fn fstring_new() -> *mut c_void {
    FSTRING_CALL.with(|call| {
        let index = call.get();
        call.set(index + 1);
        FSTRING_BUFFERS.with(|buffers| {
            let mut buffers = buffers.borrow_mut();
            buffers[index].fill(0);
            let slot = unsafe { buffers.as_mut_ptr().add(index) };
            // SAFETY: buffers[index] above proved index < len, so the add stays in bounds.
            slot.cast::<u32>().cast::<c_void>()
        })
    })
}

unsafe extern "C" fn fstring_right(_h: *mut c_void, _n: i32) -> *mut c_void {
    DEM_SUFFIX.as_ptr() as *mut c_void
}

unsafe extern "C" fn fstring_ne_text(h: *mut c_void, text: *const u32) -> i32 {
    if DEMO_FILE.with(Cell::get) {
        0
    } else if h == SWITCH_URL.as_ptr() as *mut c_void {
        NEXT_URL_NONEMPTY.with(Cell::get) as i32
    } else {
        (unsafe { decode(h.cast()) } != unsafe { decode(text) }) as i32
    }
}

unsafe extern "C" fn fstring_append_text(_h: *mut c_void, _text: *const u32) {}

unsafe extern "C" fn debugf_log(text: *const u32) {
    // SAFETY: forwarded TCHAR string is NUL-terminated.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn output_device_logf(_ar: *mut c_void, text: *const u32) {
    // SAFETY: forwarded TCHAR string is NUL-terminated.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn static_load_class(_base: i32, name: *const u32) -> *mut c_void {
    // SAFETY: the class name is a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("load_class\t{}", unsafe { decode(name) }))
    });
    0x90 as *mut c_void
}

unsafe extern "C" fn construct_net_driver(_cls: *mut c_void) -> *mut c_void {
    DEBUGF.with(|d| d.borrow_mut().push("construct_object\tdemo".to_string()));
    0xA0 as *mut c_void
}

unsafe extern "C" fn netdriver_init_listen(
    _drv: *mut c_void,
    _level: *mut c_void,
    _url: *mut c_void,
    error: *mut c_void,
) -> i32 {
    let result = DEMO_INIT_OK.with(Cell::get) as i32;
    if result == 0 {
        ERROR_TEXT.with(|error| *error.borrow_mut() = "Port busy".to_string());
        if DEMO_FILE.with(Cell::get) {
            // The recording path uses the mutable FString from `fstring_new`;
            // Listen passes the static `PORT_BUSY` fixture buffer instead.
            unsafe { assign_wide(error.cast(), "Port busy") };
        }
    }
    let name = if DEMO_FILE.with(Cell::get) {
        if EXEC_COMMAND.with(|c| c.borrow().contains("demo1")) {
            "demo1.dem"
        } else {
            "demo2.dem"
        }
    } else {
        "Entry"
    };
    let driver = if DEMO_FILE.with(Cell::get) {
        "demo"
    } else {
        "net"
    };
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("init_listen\t{driver}\t{name}\t{result}"))
    });
    result
}
unsafe extern "C" fn static_construct_net_driver(_cls: *mut c_void) -> *mut c_void {
    DEBUGF.with(|d| d.borrow_mut().push("construct_object\tnet".to_string()));
    0xA0 as *mut c_void
}

unsafe extern "C" fn level_url(_level: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn cast_game_engine(_engine: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn game_build_server_master_map(
    _ge: *mut c_void,
    _drv: *mut c_void,
    _level: *mut c_void,
) {
    DEBUGF.with(|d| d.borrow_mut().push("build_master_map".to_string()));
}

unsafe extern "C" fn game_engine_server_actors_num(_ge: *mut c_void) -> i32 {
    (ACTOR_MODE.with(Cell::get) != 0) as i32
}

unsafe extern "C" fn engine_client(_engine: *mut c_void) -> *mut c_void {
    if CLIENT_PRESENT.with(Cell::get) {
        fake_handle()
    } else {
        std::ptr::null_mut()
    }
}
unsafe extern "C" fn engine_client_viewports_num(_client: *mut c_void) -> i32 {
    1
}

unsafe extern "C" fn engine_client_viewport(_client: *mut c_void, _index: i32) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn engine_set_client_travel(
    _engine: *mut c_void,
    _viewport: *mut c_void,
    url: *const u32,
    _items: i32,
    _travel: i32,
) {
    CLIENT_TRAVEL.with(|travel| *travel.borrow_mut() = unsafe { decode(url) });
}

unsafe extern "C" fn netdriver_max_client_rate(_driver: *mut c_void) -> i32 {
    5000
}

unsafe extern "C" fn connection_set_current_net_speed(_conn: *mut c_void, speed: i32) {
    NET_SPEED.with(|value| value.set(speed));
}

unsafe extern "C" fn connection_current_net_speed(_conn: *mut c_void) -> i32 {
    NET_SPEED.with(Cell::get)
}

unsafe extern "C" fn connection_challenge(_conn: *mut c_void) -> u32 {
    0
}

unsafe extern "C" fn engine_challenge_response(_engine: *mut c_void, challenge: u32) -> i32 {
    let response = CHALLENGE_RESPONSE.with(Cell::get);
    DEBUGF.with(|records| {
        records
            .borrow_mut()
            .push(format!("challenge_response\t{challenge}\t{response}"))
    });
    response
}

unsafe extern "C" fn parse_guid(_text: *const u32, _key: *const u32, out: *mut u32) -> i32 {
    for index in 0..4 {
        unsafe { *out.add(index) = 0 };
    }
    1
}

unsafe extern "C" fn package_map_list_num(_pm: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn level_get_level_info(_level: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn level_info_set_net_mode(_li: *mut c_void, mode: u8) {
    NET_MODE.with(|value| value.set(mode));
}

unsafe extern "C" fn level_info_set_next_switch_countdown(_li: *mut c_void, value: f32) {
    COUNTDOWN.with(|countdown| countdown.set(value));
}

unsafe extern "C" fn netdriver_server_travel_pause(_drv: *mut c_void) -> f32 {
    TRAVEL_PAUSE.with(Cell::get)
}
unsafe extern "C" fn game_engine_server_actor_text(_ge: *mut c_void, _i: i32) -> *const u32 {
    ENGINE_LEVEL.as_ptr()
}

unsafe extern "C" fn parse_token_buf(
    _stream: *mut *const u32,
    buf: *mut u32,
    _max: i32,
    _escape: i32,
) -> i32 {
    let mode = ACTOR_MODE.with(Cell::get);
    let call = PARSE_TOKEN_CALL.with(|n| {
        let previous = n.get();
        n.set(previous + 1);
        previous
    });
    let token: &[u32] = match (mode, call) {
        (1, 0) | (2..=5, 0) => &[
            'E' as u32, 'n' as u32, 'g' as u32, 'i' as u32, 'n' as u32, 'e' as u32, '.' as u32,
            'H' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 'e' as u32, 'r' as u32, 0,
        ],
        (2, 1) | (3..=5, 2) => &[
            'a' as u32, 'c' as u32, 't' as u32, 'o' as u32, 'r' as u32, 0,
        ],
        (3..=5, 1) => &[
            'F' as u32, 'l' as u32, 'a' as u32, 'g' as u32, '=' as u32, '5' as u32, 0,
        ],
        _ => return 0,
    };
    for (i, ch) in token.iter().enumerate() {
        unsafe { *buf.add(i) = *ch };
    }
    1
}

unsafe extern "C" fn level_spawn_actor(_level: *mut c_void, _cls: *mut c_void) -> *mut c_void {
    if ACTOR_MODE.with(Cell::get) == 1 {
        DEBUGF.with(|d| d.borrow_mut().push("spawn_helper\tnull".to_string()));
        std::ptr::null_mut()
    } else {
        DEBUGF.with(|d| d.borrow_mut().push("spawn_helper\tactor".to_string()));
        0xB0 as *mut c_void
    }
}

unsafe extern "C" fn app_strchr(text: *const u32, ch: u32) -> *const u32 {
    let mut p = text;
    loop {
        let value = unsafe { *p };
        if value == ch || value == 0 {
            return if value == ch { p } else { std::ptr::null() };
        }
        p = unsafe { p.add(1) };
    }
}

unsafe extern "C" fn actor_get_class(_actor: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn tfield_iter_new(_cls: *mut c_void) -> *mut c_void {
    FIELD_ITER_VALID.with(|valid| valid.set(true));
    fake_handle()
}

unsafe extern "C" fn tfield_iter_valid(_it: *mut c_void) -> i32 {
    FIELD_ITER_VALID.with(Cell::get) as i32
}

unsafe extern "C" fn tfield_iter_next(_it: *mut c_void) {
    FIELD_ITER_VALID.with(|valid| valid.set(false));
}

unsafe extern "C" fn tfield_iter_delete(_it: *mut c_void) {}

unsafe extern "C" fn uproperty_name_stricmp(_it: *mut c_void, name: *const u32) -> i32 {
    let name = unsafe { decode(name) };
    let expected_match = ACTOR_MODE.with(Cell::get) != 5;
    if expected_match && name == "Flag" {
        0
    } else {
        1
    }
}

unsafe extern "C" fn uproperty_is_config(_it: *mut c_void) -> i32 {
    (ACTOR_MODE.with(Cell::get) == 3) as i32
}

unsafe extern "C" fn uproperty_import_text(_it: *mut c_void, text: *const u32, _obj: *mut c_void) {
    // SAFETY: imported text is a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("import_text\tFlag\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn netdriver_delete(_drv: *mut c_void) {
    DEBUGF.with(|d| {
        d.borrow_mut().push(format!(
            "delete_driver\t{}",
            if DEMO_FILE.with(Cell::get) {
                "demo"
            } else {
                "net"
            }
        ))
    });
}
unsafe extern "C" fn level_exec_net(_drv: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    // SAFETY: command is a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("net_exec\tnet\t{}", unsafe { decode(cmd) }))
    });
    EXEC_NET_RESULT.with(Cell::get)
}

unsafe extern "C" fn level_exec_demo(_drv: *mut c_void, cmd: *const u32, _ar: *mut c_void) -> i32 {
    // SAFETY: command is a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("net_exec\tdemo\t{}", unsafe { decode(cmd) }))
    });
    EXEC_DEMO_RESULT.with(Cell::get)
}
unsafe extern "C" fn localize_error_assign(
    _level: *mut c_void,
    _error: *mut c_void,
    key: *const i8,
) {
    let key = unsafe { std::ffi::CStr::from_ptr(key) }.to_string_lossy();
    let error = match key.as_ref() {
        "NetAlready" => "Already listening",
        "NetListen" => "Listen failed",
        _ => "",
    };
    ERROR_TEXT.with(|value| *value.borrow_mut() = error.to_string());
    DEBUGF.with(|d| d.borrow_mut().push(format!("localize_error\t{key}")));
}

unsafe extern "C" fn level_get_linker(_level: *mut c_void) -> *mut c_void {
    if LINKER_PRESENT.with(Cell::get) {
        fake_handle()
    } else {
        std::ptr::null_mut()
    }
}
unsafe extern "C" fn uobject_get_name(obj: *mut c_void) -> *const u32 {
    // The outer package is always named "Level" (welcome logs). The level is named only after the
    // scenario's level_with_levelinfo() (level_harness.cpp:621); begin() zeroes it, so it is "" otherwise.
    if obj == fake_handle() || LEVEL_NAMED.with(Cell::get) {
        LEVEL_NAME.as_ptr()
    } else {
        EMPTY_TEXT.as_ptr()
    }
}

unsafe extern "C" fn level_info_next_url(_li: *mut c_void) -> *mut c_void {
    if NEXT_URL_NONEMPTY.with(Cell::get) {
        SWITCH_URL.as_ptr() as *mut c_void
    } else {
        EMPTY_TEXT.as_ptr() as *mut c_void
    }
}

unsafe extern "C" fn app_timestamp() -> *const u32 {
    TIMESTAMP.as_ptr()
}

unsafe extern "C" fn connection_lowlevel_remote_address(_conn: *mut c_void) -> *mut c_void {
    REMOTE_ADDRESS.as_ptr() as *mut c_void
}

unsafe extern "C" fn debugf_net_come_go(text: *const u32) {
    // SAFETY: level_net forwards a NUL-terminated TCHAR string.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("debugf\tnet_come_go\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn connection_package_map(_conn: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn netdriver_master_map(_drv: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn package_map_copy(_pm: *mut c_void, _master: *mut c_void) {
    DEBUGF.with(|d| d.borrow_mut().push("package_map_copy".to_string()));
}

unsafe extern "C" fn connection_send_package_map(_conn: *mut c_void) {
    DEBUGF.with(|d| d.borrow_mut().push("send_package_map".to_string()));
}

unsafe extern "C" fn uobject_get_outer(_obj: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn level_info_b_lone_player(_li: *mut c_void) -> i32 {
    LONE_PLAYER.with(Cell::get)
}

unsafe extern "C" fn connection_log(_conn: *mut c_void, text: *const u32) {
    // SAFETY: forwarded TCHAR string is NUL-terminated.
    DEBUGF.with(|d| {
        d.borrow_mut()
            .push(format!("conn_log\t{}", unsafe { decode(text) }))
    });
}

unsafe extern "C" fn connection_flush_net(_conn: *mut c_void) {
    DEBUGF.with(|d| d.borrow_mut().push("conn_flush".to_string()));
}

unsafe extern "C" fn app_atoi(text: *const u32) -> i32 {
    let text = unsafe { decode(text) };
    text.trim().parse().unwrap_or(0)
}

unsafe extern "C" fn connection_set_user_flags(_conn: *mut c_void, flags: i32) {
    DEBUGF.with(|d| d.borrow_mut().push(format!("user_flags\t{flags}")));
}

unsafe extern "C" fn check_failed(_expr: *const i8, _file: *const i8, _line: i32) -> i32 {
    0
}
unsafe extern "C" fn app_errorf(text: *const u32) -> i32 {
    // SAFETY: forwarded TCHAR string is NUL-terminated.
    let text = unsafe { decode(text) };
    DEBUGF.with(|d| {
        let mut d = d.borrow_mut();
        d.push(format!("error\t{text}"));
        d.push("debugf\tcritical\tULevel::NotifyReceivingFile".to_string());
    });
    0
}
unsafe extern "C" fn connection_channel_close(_conn: *mut c_void, _i: i32) {
    DEBUGF.with(|d| d.borrow_mut().push("channel_close".to_string()));
}

unsafe extern "C" fn connection_remote_version(_conn: *mut c_void) -> i32 {
    219
}

unsafe extern "C" fn connection_set_remote_version(_conn: *mut c_void, _version: i32) {}

unsafe extern "C" fn connection_set_state_closed(_conn: *mut c_void) {}

unsafe extern "C" fn parse_int(_text: *const u32, key: *const u32, out: *mut i32) -> i32 {
    let key = unsafe { decode(key) };
    if key == "MINVER=" || key == "VER=" {
        unsafe { *out = 219 };
        return 1;
    }
    if key == "RESPONSE=" {
        let command = EXEC_COMMAND.with(|command| command.borrow().clone());
        let Some(response) = command
            .split_once("RESPONSE=")
            .and_then(|(_, rest)| rest.split_whitespace().next())
            .and_then(|value| value.parse::<i32>().ok())
        else {
            return 0;
        };
        unsafe { *out = response };
        return 1;
    }
    0
}
unsafe extern "C" fn parse_text(
    _text: *const u32,
    key: *const u32,
    out: *mut u32,
    max: i32,
) -> i32 {
    let key = unsafe { decode(key) };
    let command = EXEC_COMMAND.with(|command| command.borrow().clone());
    let Some((_, value)) = command.split_once(key.as_str()) else {
        unsafe { *out = 0 };
        return 0;
    };
    let value = value.trim_start();
    let chars = value
        .encode_utf16()
        .map(u32::from)
        .take(max.saturating_sub(1) as usize);
    let mut index = 0;
    for ch in chars {
        unsafe { *out.add(index) = ch };
        index += 1;
    }
    unsafe { *out.add(index) = 0 };
    1
}

unsafe extern "C" fn connection_set_request_url(_conn: *mut c_void, text: *const u32) {
    let text = unsafe { decode(text) };
    REQUEST_URL.with(|url| *url.borrow_mut() = wide(&text));
}

unsafe extern "C" fn connection_request_url_text(_conn: *mut c_void) -> *const u32 {
    REQUEST_URL.with(|url| url.borrow().as_ptr())
}

unsafe extern "C" fn level_info_game(_li: *mut c_void) -> *mut c_void {
    fake_handle()
}

unsafe extern "C" fn game_pre_login(
    _game: *mut c_void,
    options: *const u32,
    address: *const u32,
    error: *mut c_void,
    fail_code: *mut c_void,
) {
    let options = unsafe { decode(options) };
    let address = unsafe { decode(address) };
    DEBUGF.with(|records| {
        records
            .borrow_mut()
            .push(format!("pre_login\t{options}\t{address}"))
    });
    if PRELOGIN_REJECT.with(Cell::get) {
        unsafe {
            assign_wide(error.cast(), "Denied");
            assign_wide(fail_code.cast(), "Bad");
        }
    }
}

unsafe fn assign_wide(out: *mut u32, text: &str) {
    for (index, ch) in text.encode_utf16().map(u32::from).enumerate() {
        unsafe { *out.add(index) = ch };
    }
    unsafe { *out.add(text.encode_utf16().count()) = 0 };
}

unsafe extern "C" fn level_welcome_player(level: *mut c_void, conn: *mut c_void) {
    let services = services();
    unsafe { level_net::welcome_player(&services, level, conn, EMPTY_TEXT.as_ptr()) };
}

unsafe extern "C" fn channel_connection(_ch: *mut c_void) -> *mut c_void {
    0x10 as *mut c_void
}

unsafe extern "C" fn connection_driver(_conn: *mut c_void) -> *mut c_void {
    0x20 as *mut c_void
}

unsafe extern "C" fn channel_ch_type(_ch: *mut c_void) -> i32 {
    CHANNEL_TYPE.with(Cell::get)
}

unsafe extern "C" fn channel_ch_index(_ch: *mut c_void) -> i32 {
    CHANNEL_INDEX.with(Cell::get)
}

/// Returns a handle whose `fstring_text` is the static `"Engine.Level"` buffer.
unsafe extern "C" fn uobject_get_full_name(_obj: *mut c_void) -> *mut c_void {
    ENGINE_LEVEL.as_ptr() as *mut c_void
}

unsafe extern "C" fn fstring_text(h: *mut c_void) -> *const u32 {
    h as *const u32
}

/// The handle is the static buffer, so there is nothing to free.
unsafe extern "C" fn fstring_delete(_h: *mut c_void) {}

/// A table where every field panics, with the used services overridden.
fn services() -> Hp2LevelNetServices {
    let stub: unsafe extern "C" fn() = unused;
    let raw = [stub as usize; SERVICE_COUNT];
    // SAFETY: the struct is `SERVICE_COUNT` pointer-sized `repr(C)` fields and
    // `usize` has the same size and alignment as a function pointer.
    let mut s: Hp2LevelNetServices = unsafe { std::mem::transmute(raw) };
    s.check_failed = check_failed;
    s.debugf_dev_net = debugf_dev_net;
    s.netdriver_server_connection = netdriver_server_connection;
    s.channel_connection = channel_connection;
    s.connection_driver = connection_driver;
    s.channel_ch_type = channel_ch_type;
    s.channel_ch_index = channel_ch_index;
    s.uobject_get_full_name = uobject_get_full_name;
    s.fstring_text = fstring_text;
    s.fstring_delete = fstring_delete;
    s.level_exec_net = level_exec_net;
    s.level_exec_demo = level_exec_demo;
    s.output_device_log = output_device_log;
    s.parse_command = parse_command;
    s.parse_token_fstring = parse_token_fstring;
    s.debugf_log = debugf_log;
    s.output_device_logf = output_device_logf;
    s.fstring_right = fstring_right;
    s.fstring_ne_text = fstring_ne_text;
    s.fstring_append_text = fstring_append_text;
    s.static_load_class = static_load_class;
    s.construct_net_driver = construct_net_driver;
    s.netdriver_init_listen = netdriver_init_listen;
    s.netdriver_delete = netdriver_delete;
    s.static_construct_net_driver = static_construct_net_driver;
    s.level_url = level_url;
    s.cast_game_engine = cast_game_engine;
    s.game_build_server_master_map = game_build_server_master_map;
    s.game_engine_server_actors_num = game_engine_server_actors_num;
    s.engine_client = engine_client;
    s.engine_client_viewports_num = engine_client_viewports_num;
    s.engine_client_viewport = engine_client_viewport;
    s.engine_set_client_travel = engine_set_client_travel;
    s.netdriver_max_client_rate = netdriver_max_client_rate;
    s.connection_set_current_net_speed = connection_set_current_net_speed;
    s.connection_current_net_speed = connection_current_net_speed;
    s.engine_challenge_response = engine_challenge_response;
    s.connection_challenge = connection_challenge;
    s.parse_guid = parse_guid;
    s.package_map_list_num = package_map_list_num;
    s.parse_text = parse_text;
    s.connection_set_request_url = connection_set_request_url;
    s.connection_request_url_text = connection_request_url_text;
    s.level_info_game = level_info_game;
    s.game_pre_login = game_pre_login;
    s.level_welcome_player = level_welcome_player;
    s.level_get_level_info = level_get_level_info;
    s.level_info_set_net_mode = level_info_set_net_mode;
    s.level_info_set_next_switch_countdown = level_info_set_next_switch_countdown;
    s.netdriver_server_travel_pause = netdriver_server_travel_pause;
    s.game_engine_server_actor_text = game_engine_server_actor_text;
    s.parse_token_buf = parse_token_buf;
    s.level_spawn_actor = level_spawn_actor;
    s.app_strchr = app_strchr;
    s.actor_get_class = actor_get_class;
    s.tfield_iter_new = tfield_iter_new;
    s.tfield_iter_valid = tfield_iter_valid;
    s.tfield_iter_next = tfield_iter_next;
    s.tfield_iter_delete = tfield_iter_delete;
    s.uproperty_name_stricmp = uproperty_name_stricmp;
    s.uproperty_is_config = uproperty_is_config;
    s.uproperty_import_text = uproperty_import_text;

    s.fstring_new = fstring_new;
    s.furl_new = fake_handle;
    s.furl_map = furl_map;
    s.furl_delete = no_delete;
    s.localize_error_assign = localize_error_assign;
    s.level_get_linker = level_get_linker;
    s.uobject_get_name = uobject_get_name;
    s.level_info_next_url = level_info_next_url;
    s.app_timestamp = app_timestamp;
    s.connection_lowlevel_remote_address = connection_lowlevel_remote_address;
    s.debugf_net_come_go = debugf_net_come_go;
    s.connection_package_map = connection_package_map;
    s.netdriver_master_map = netdriver_master_map;
    s.package_map_copy = package_map_copy;
    s.connection_send_package_map = connection_send_package_map;
    s.uobject_get_outer = uobject_get_outer;
    s.level_info_b_lone_player = level_info_b_lone_player;
    s.connection_log = connection_log;
    s.connection_flush_net = connection_flush_net;
    s.app_atoi = app_atoi;
    s.connection_set_user_flags = connection_set_user_flags;
    s.app_errorf = app_errorf;
    s.connection_channel_close = connection_channel_close;
    s.connection_remote_version = connection_remote_version;
    s.connection_set_remote_version = connection_set_remote_version;
    s.connection_set_state_closed = connection_set_state_closed;
    s.parse_int = parse_int;

    s
}

/// Runs one net scenario and returns its ordered records and optional return value.
fn run(name: &str) -> (Vec<String>, Option<i32>) {
    DEBUGF.with(|d| d.borrow_mut().clear());
    let s = services();
    EXEC_NET_RESULT.with(|c| c.set(0));
    EXEC_DEMO_RESULT.with(|c| c.set(0));
    DEMO_FILE.with(|c| c.set(false));
    DEMO_INIT_OK.with(|c| c.set(false));
    LINKER_PRESENT.with(|c| c.set(false));
    CLIENT_PRESENT.with(|c| c.set(false));
    SERVER_CONNECTION.with(|c| c.set(false));
    ACTOR_MODE.with(|mode| mode.set(0));
    PARSE_TOKEN_CALL.with(|call| call.set(0));
    FIELD_ITER_VALID.with(|valid| valid.set(false));
    NEXT_URL_NONEMPTY.with(|value| value.set(false));
    LEVEL_NAMED.with(|value| value.set(false));
    LONE_PLAYER.with(|value| value.set(0));
    NET_MODE.with(|value| value.set(0));
    COUNTDOWN.with(|value| value.set(0.0));
    TRAVEL_PAUSE.with(|value| value.set(0.0));
    ERROR_TEXT.with(|error| error.borrow_mut().clear());
    FSTRING_CALL.with(|call| call.set(0));
    FSTRING_BUFFERS.with(|buffers| {
        buffers
            .borrow_mut()
            .iter_mut()
            .for_each(|slot| slot.fill(0))
    });
    PRELOGIN_REJECT.with(|reject| reject.set(false));
    REQUEST_URL.with(|url| url.borrow_mut().clear());
    CLIENT_TRAVEL.with(|travel| travel.borrow_mut().clear());
    NET_SPEED.with(|speed| speed.set(0));
    CHALLENGE_RESPONSE.with(|response| response.set(0));
    let mut level = unsafe { std::mem::zeroed::<ULevel>() };
    let level_ptr = (&mut level as *mut ULevel).cast::<c_void>();
    let ret = match name {
        "net.notify_accepting_connection_client" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(true));
            // SAFETY: level and connection-state services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepting_connection_switching" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            NEXT_URL_NONEMPTY.with(|c| c.set(true));
            // Harness calls level_with_levelinfo() here (level_harness.cpp:1508), naming the level.
            LEVEL_NAMED.with(|c| c.set(true));
            // SAFETY: level and next-URL services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepting_connection_accept" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            NEXT_URL_NONEMPTY.with(|c| c.set(false));
            // Harness calls level_with_levelinfo() here (level_harness.cpp:1514), naming the level.
            LEVEL_NAMED.with(|c| c.set(true));
            // SAFETY: level and next-URL services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepted_connection" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            // SAFETY: mocked level, connection, time, and address services are valid.
            unsafe { level_net::notify_accepted_connection(&s, level_ptr, 0x50 as _) }.unwrap();
            0
        }
        "net.welcome_player_default" => {
            LONE_PLAYER.with(|c| c.set(0));
            // SAFETY: welcome services are mocked and do not dereference opaque handles.
            unsafe { level_net::welcome_player(&s, level_ptr, 0x50 as _, EMPTY_TEXT.as_ptr()) };
            0
        }
        "net.welcome_player_optional" => {
            LONE_PLAYER.with(|c| c.set(1));
            let optional = wide("extra");
            // SAFETY: optional string and mocked services are live.
            unsafe { level_net::welcome_player(&s, level_ptr, 0x50 as _, optional.as_ptr()) };
            0
        }
        "net.notify_received_text_userflag" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "USERFLAG 7".to_string());
            let text = wide("USERFLAG 7");
            // SAFETY: message and connection remain live for the call.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_server_hello" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "HELLO MINVER=219 VER=219".to_string());
            let text = wide("HELLO MINVER=219 VER=219");
            // SAFETY: message, connection, engine client, and version services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_client_failure" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "FAILURE rejected".to_string());
            let text = wide("FAILURE rejected");
            // SAFETY: all reached client-failure services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_netspeed_high" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "NETSPEED 7000".to_string());
            let text = wide("NETSPEED 7000");
            // SAFETY: all reached netspeed services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_netspeed_low" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "NETSPEED 1000".to_string());
            let text = wide("NETSPEED 1000");
            // SAFETY: all reached netspeed services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_have" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_COMMAND.with(|c| {
                *c.borrow_mut() = "HAVE GUID=00000000000000000000000000000000".to_string()
            });
            let text = wide("HAVE GUID=00000000000000000000000000000000");
            // SAFETY: package-map and GUID parsing services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_skip" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_COMMAND.with(|c| {
                *c.borrow_mut() = "SKIP GUID=00000000000000000000000000000000".to_string()
            });
            let text = wide("SKIP GUID=00000000000000000000000000000000");
            // SAFETY: package-map and GUID parsing services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_login_missing_response" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN URL=?Name=Guest".to_string());
            let text = wide("LOGIN URL=?Name=Guest");
            // SAFETY: challenge and failure-response services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_login_precedence_false" => {
            level.net_driver = 0x20 as *mut _;
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=0".to_string());
            let text = wide("LOGIN RESPONSE=0");
            // SAFETY: challenge and failure-response services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_login_success" => {
            level.net_driver = 0x20 as *mut _;
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=1 URL=?Name=Guest".to_string());
            FSTRING_CALL.with(|c| c.set(0));
            REQUEST_URL.with(|url| url.borrow_mut().clear());
            let text = wide("LOGIN RESPONSE=1 URL=?Name=Guest");
            // SAFETY: challenge, request URL, PreLogin, and Welcome services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_text_login_prelogin_reject" => {
            level.net_driver = 0x20 as *mut _;
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            PRELOGIN_REJECT.with(|c| c.set(true));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=1 URL=?Name=Guest".to_string());
            FSTRING_CALL.with(|c| c.set(0));
            REQUEST_URL.with(|url| url.borrow_mut().clear());
            let text = wide("LOGIN RESPONSE=1 URL=?Name=Guest");
            // SAFETY: challenge and PreLogin rejection services are mocked.
            unsafe { level_net::notify_received_text(&s, level_ptr, 0x50 as _, text.as_ptr()) };
            0
        }
        "net.notify_received_file" => {
            // SAFETY: the mock records the error without unwinding across FFI.
            unsafe { level_net::notify_received_file(&s) }.unwrap();
            0
        }
        "net.notify_sending_file_server" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            level.net_driver = 0x20 as *mut _;
            // SAFETY: `level` is a live, exclusively owned `ULevel`.
            unsafe { level_net::notify_sending_file(&s, level_ptr) }
        }
        "net.notify_sending_file_client" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            level.net_driver = 0x20 as *mut _;
            // SAFETY: as above.
            unsafe { level_net::notify_sending_file(&s, level_ptr) }
        }
        "net.accepting_channel_client_refuse" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            CHANNEL_TYPE.with(|c| c.set(level_net::CHTYPE_CONTROL));
            CHANNEL_INDEX.with(|c| c.set(0));
            // SAFETY: stub services do not dereference the level or channel.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, 0x50 as *mut c_void) }
                .unwrap()
        }
        "net.accepting_channel_server_control" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            CHANNEL_TYPE.with(|c| c.set(level_net::CHTYPE_CONTROL));
            CHANNEL_INDEX.with(|c| c.set(0));
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, 0x50 as *mut c_void) }
                .unwrap()
        }
        "net.accepting_channel_server_file" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            CHANNEL_TYPE.with(|c| c.set(level_net::CHTYPE_FILE));
            CHANNEL_INDEX.with(|c| c.set(0));
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, 0x50 as *mut c_void) }
                .unwrap()
        }
        "net.accepting_channel_server_refuse" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            CHANNEL_TYPE.with(|c| c.set(2));
            CHANNEL_INDEX.with(|c| c.set(1));
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, 0x50 as *mut c_void) }
                .unwrap()
        }
        "net.exec_net_handled" => {
            level.net_driver = 0x20 as *mut _;
            EXEC_NET_RESULT.with(|c| c.set(1));
            // SAFETY: the command is a live NUL-terminated TCHAR literal.
            unsafe { level_net::exec(&s, level_ptr, wide("NETHANDLED").as_ptr(), 0x60 as _) }
        }
        "net.exec_demo_handled" => {
            level.demo_rec_driver = 0x30 as *mut _;
            EXEC_DEMO_RESULT.with(|c| c.set(1));
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOHANDLED").as_ptr(), 0x60 as _) }
        }
        "net.exec_unhandled" => {
            level.net_driver = 0x20 as *mut _;
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("FOO").as_ptr(), 0x60 as _) }
        }
        "net.exec_demorec_no_filename" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC".to_string());
            // SAFETY: command and output device pointers are live for the call.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC").as_ptr(), 0x60 as _) }
        }
        "net.exec_demorec_init_fail" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC demo1".to_string());
            DEMO_FILE.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(false));
            // SAFETY: scenario-owned inputs and mocked services remain live.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC demo1").as_ptr(), 0x60 as _) }
        }
        "net.exec_demorec_started" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC demo2.dem".to_string());
            DEMO_FILE.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC demo2.dem").as_ptr(), 0x60 as _) }
        }

        "net.exec_demoplay_no_filename" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOPLAY".to_string());
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOPLAY").as_ptr(), 0x60 as _) }
        }
        "net.is_server_driver_server" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            // SAFETY: `level` is live; the driver callback is mocked.
            unsafe { level_net::is_server(&s, level_ptr) }
        }
        "net.is_server_driver_client" => {
            level.net_driver = 0x20 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }
        }
        "net.is_server_demo_client" => {
            level.demo_rec_driver = 0x30 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }
        }
        "net.is_server_demo_present" => {
            level.net_driver = 0x20 as *mut _;
            level.demo_rec_driver = 0x30 as *mut _;
            SERVER_CONNECTION.with(|c| c.set(false));
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }
        }
        "net.listen_already_listening" => {
            level.net_driver = 0x20 as *mut _;
            // SAFETY: the level is live; localized error assignment is mocked.
            unsafe { level_net::listen(&s, level_ptr, 0x60 as _) }
        }
        "net.listen_no_linker" => {
            // SAFETY: as above; the linker callback reports the fixture's null linker.
            unsafe { level_net::listen(&s, level_ptr, 0x60 as _) }
        }
        "net.listen_init_fail" => {
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(false));
            // SAFETY: level/linker/init mocks model this fixture path.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_ok" | "net.listen_server_actor_blank" => {
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            if name == "net.listen_server_ok" {
                TRAVEL_PAUSE.with(|pause| pause.set(2.5));
            }
            // SAFETY: all reached level/game/driver services are mocked.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_dedicated_ok" => {
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(false));
            TRAVEL_PAUSE.with(|pause| pause.set(2.5));
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_actor_spawn_null" => {
            ACTOR_MODE.with(|mode| mode.set(1));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            // SAFETY: scenario-specific actor and listen services are mocked.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_actor_bare_token" => {
            ACTOR_MODE.with(|mode| mode.set(2));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_actor_config_field" => {
            ACTOR_MODE.with(|mode| mode.set(3));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_actor_non_config_field" => {
            ACTOR_MODE.with(|mode| mode.set(4));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        "net.listen_server_actor_name_mismatch" => {
            ACTOR_MODE.with(|mode| mode.set(5));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }
        }
        other => panic!("unknown net scenario {other}"),
    };
    let mut lines = DEBUGF.with(|d| d.borrow().clone());
    if name == "net.notify_received_text_client_failure" {
        let travel = CLIENT_TRAVEL.with(|travel| travel.borrow().clone());
        lines.push(format!("client_travel\t{travel}"));
    }
    if name == "net.notify_received_text_netspeed_high"
        || name == "net.notify_received_text_netspeed_low"
    {
        NET_SPEED.with(|speed| lines.push(format!("net_speed\t{}", speed.get())));
    }
    if name.starts_with("net.listen_") {
        let error = ERROR_TEXT.with(|value| value.borrow().clone());
        lines.push(format!("error_text\t{error}"));
        lines.push(format!(
            "net_driver\t{}",
            u8::from(!level.net_driver.is_null())
        ));
        if ret == 1 {
            NET_MODE.with(|mode| lines.push(format!("net_mode\t{}", mode.get())));
            COUNTDOWN.with(|countdown| lines.push(format!("countdown\t{}", countdown.get())));
        }
    }
    let ret = if matches!(
        name,
        "net.notify_accepted_connection"
            | "net.welcome_player_default"
            | "net.welcome_player_optional"
            | "net.notify_received_text_client_failure"
            | "net.notify_received_text_netspeed_high"
            | "net.notify_received_text_netspeed_low"
            | "net.notify_received_text_have"
            | "net.notify_received_text_skip"
            | "net.notify_received_text_login_missing_response"
            | "net.notify_received_text_login_precedence_false"
            | "net.notify_received_text_login_success"
            | "net.notify_received_text_login_prelogin_reject"
            | "net.notify_received_text_userflag"
            | "net.notify_received_text_server_hello"
    ) {
        None
    } else {
        Some(ret)
    };
    (lines, ret)
}

#[test]
fn level_net_matches_oracle_fixture() {
    let mut checked = 0usize;
    let mut current: Option<String> = None;
    let mut expect_records: Vec<String> = Vec::new();
    let mut expect_ret: Option<i32> = None;

    let mut verify = |name: &str, records: &[String], ret: Option<i32>| {
        let (got_records, got_ret) = run(name);
        assert_eq!(got_records, records, "{name}: fixture records");
        assert_eq!(got_ret, ret, "{name}: return value");
        checked += 1;
    };

    for line in FIXTURE.lines() {
        if let Some(name) = line.strip_prefix("scenario\t") {
            if let Some(prev) = current.take()
                && prev.starts_with("net.")
            {
                verify(&prev, &expect_records, expect_ret);
            }
            current = Some(name.to_string());
            expect_records.clear();
            expect_ret = None;
        } else if current.as_deref().is_some_and(|c| c.starts_with("net.")) {
            if let Some(ret) = line.strip_prefix("=>\t") {
                expect_ret = Some(ret.parse().expect("return value"));
            } else if !line.starts_with("case\t") {
                expect_records.push(line.to_string());
            }
        }
    }
    if let Some(prev) = current.take()
        && prev.starts_with("net.")
    {
        verify(&prev, &expect_records, expect_ret);
    }

    let expected_net_scenarios = FIXTURE
        .lines()
        .filter(|line| line.starts_with("scenario\tnet."))
        .count();
    assert_eq!(
        checked, expected_net_scenarios,
        "all net scenarios were replayed"
    );
    assert_eq!(
        expected_net_scenarios, 46,
        "final fixture net scenario count"
    );
}

unsafe extern "C" fn check_failed_with_pending_exception(
    _expr: *const i8,
    _file: *const i8,
    _line: i32,
) -> i32 {
    1
}

#[test]
fn captured_check_failure_stops_network_body() {
    let mut s = services();
    s.check_failed = check_failed_with_pending_exception;
    let mut level = unsafe { std::mem::zeroed::<ULevel>() };
    // SAFETY: the zeroed level fails its NetDriver check before any other service call.
    assert_eq!(
        unsafe {
            level_net::notify_accepting_connection(&s, (&mut level as *mut ULevel).cast::<c_void>())
        },
        Err(())
    );
}
