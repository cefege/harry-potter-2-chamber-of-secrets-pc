//! Replays the net scenarios of `fixtures/level_oracle.txt` against
//! `hp2_engine::level_net`.
//!
//! Each `net.*` scenario runs the original `ULevel` Net body with the services
//! that scenario reaches. Every other service panics if called. The expected
//! fixture record sequence and `=>` return value come from the fixture.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, FURL, UChannel, UGameEngine, ULevel, UNetConnection, UNetDriver, UObject,
};
use hp2_engine::level_net::{self, Hp2LevelNetContinuation, Hp2LevelNetServices};
use hp2_engine::level_spawn_actor::Hp2SpawnActorContext;

#[path = "level_spawn_actor/mock.rs"]
mod level_spawn_actor_mock;

const FIXTURE: &str = include_str!("fixtures/level_oracle.txt");

/// Number of `Hp2LevelNetServices` fields (all `unsafe extern "C" fn` pointers).
const SERVICE_COUNT: usize = 71;

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

thread_local! {
    static SERVER_CONNECTION: Cell<bool> = const { Cell::new(false) };
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
    static PRELOGIN_REJECT: Cell<bool> = const { Cell::new(false) };
    static REQUEST_URL: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static CLIENT_TRAVEL: RefCell<String> = const { RefCell::new(String::new()) };
    static NET_SPEED: Cell<i32> = const { Cell::new(0) };
    static CHALLENGE_RESPONSE: Cell<i32> = const { Cell::new(0) };
    static DEBUGF: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

thread_local! {
    static NET_DRIVER_MEM: RefCell<Box<UNetDriver>> =
        RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static CONNECTION_MEM: RefCell<Box<UNetConnection>> =
        RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static CHANNEL_MEM: RefCell<Box<UChannel>> =
        RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static GAME_ENGINE_MEM: RefCell<Box<UGameEngine>> =
        RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static LEVEL_INFO_MEM: RefCell<Box<ALevelInfo>> =
        RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static URL_MEM: RefCell<Box<FURL>> = RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
    static ACTOR_MEM: RefCell<Box<AActor>> = RefCell::new(Box::new(unsafe { std::mem::zeroed() }));
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

unsafe extern "C" fn debugf_dev_net(text: *const u32) -> i32 {
    (|| {
        // SAFETY: forwarded from level_net.
        let s = unsafe { decode(text) };
        DEBUGF.with(|d| d.borrow_mut().push(format!("debugf\tdev_net\t{s}")));
    })();
    0
}


fn wide(text: &str) -> Vec<u32> {
    text.encode_utf16().map(u32::from).chain([0]).collect()
}
unsafe extern "C" fn output_device_log(_ar: *mut c_void, text: *const u32) -> i32 {
    (|| {
        // SAFETY: level_net forwards a NUL-terminated TCHAR string.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
        });
    })();
    0
}

unsafe extern "C" fn parse_command(
    stream: *mut *const u32,
    keyword: *const u32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let command = EXEC_COMMAND.with(|c| c.borrow().clone());
            // SAFETY: keyword is a NUL-terminated TCHAR string.
            let keyword = decode(keyword);
            let Some(rest) = command.strip_prefix(&keyword) else {
                return 0;
            };
            if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
                return 0;
            }
            // SAFETY: the command buffer remains alive for the duration of the call.
            *stream = (*stream).add(keyword.encode_utf16().count());
            1
        })();
    }
    0
}

unsafe extern "C" fn parse_token_fstring(
    _stream: *mut *const u32,
    _token: *mut c_void,
    _escape: i32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 { DEMO_FILE.with(Cell::get) as i32 })();
    }
    0
}

extern "C" fn fake_handle() -> *mut c_void {
    0x80 as *mut c_void
}

fn net_driver_ptr() -> *mut c_void {
    NET_DRIVER_MEM.with(|m| m.borrow().as_ref() as *const UNetDriver as *mut c_void)
}

/// Syncs the shared `UNetDriver` mock memory from the scenario's thread-local
/// flags. Must run after `SERVER_CONNECTION`/`TRAVEL_PAUSE` are set for the
/// scenario and before the `level_net` call that reads them.
fn sync_net_driver() {
    NET_DRIVER_MEM.with(|m| {
        let mut d = m.borrow_mut();
        d.server_connection = if SERVER_CONNECTION.with(Cell::get) {
            fake_handle()
        } else {
            std::ptr::null_mut()
        };
        d.server_travel_pause = TRAVEL_PAUSE.with(Cell::get);
        d.max_client_rate = 5000;
        d.master_map = fake_handle();
    });
}

fn connection_ptr() -> *mut c_void {
    CONNECTION_MEM.with(|m| m.borrow().as_ref() as *const UNetConnection as *mut c_void)
}

/// Links the shared mock `UNetConnection` to the shared mock `UNetDriver`,
/// for call paths that reach `connection_driver`.
fn sync_connection_driver() {
    CONNECTION_MEM.with(|m| m.borrow_mut().driver = net_driver_ptr());
}

fn channel_ptr() -> *mut c_void {
    CHANNEL_MEM.with(|m| m.borrow().as_ref() as *const UChannel as *mut c_void)
}

/// Syncs the shared mock `UChannel`'s connection link and fields, and links
/// its connection to the shared mock driver.
fn sync_channel(ch_type: i32, ch_index: i32) {
    CHANNEL_MEM.with(|m| {
        let mut c = m.borrow_mut();
        c.connection = connection_ptr();
        c.ch_type = ch_type;
        c.ch_index = ch_index;
    });
    sync_connection_driver();
}

fn game_engine_ptr() -> *mut c_void {
    GAME_ENGINE_MEM.with(|m| m.borrow().as_ref() as *const UGameEngine as *mut c_void)
}

/// Syncs the shared mock `UGameEngine`'s `Client` field from `CLIENT_PRESENT`.
fn sync_game_engine() {
    GAME_ENGINE_MEM.with(|m| {
        m.borrow_mut().client = if CLIENT_PRESENT.with(Cell::get) {
            fake_handle()
        } else {
            std::ptr::null_mut()
        };
    });
}

fn actor_ptr() -> *mut c_void {
    ACTOR_MEM.with(|m| m.borrow().as_ref() as *const AActor as *mut c_void)
}

fn url_ptr() -> *mut c_void {
    URL_MEM.with(|m| m.borrow().as_ref() as *const FURL as *mut c_void)
}

/// Address of the shared mock `FURL`'s `Map` field, matching what the
/// converted `furl_map` leaf computes from `url_ptr()`.
fn url_map_ptr() -> *mut c_void {
    URL_MEM.with(|m| {
        let r = m.borrow();
        std::ptr::addr_of!(r.map) as *mut c_void
    })
}

fn level_info_ptr() -> *mut c_void {
    LEVEL_INFO_MEM.with(|m| m.borrow().as_ref() as *const ALevelInfo as *mut c_void)
}

/// Address of the shared mock `ALevelInfo`'s `NextURL` field, matching what
/// the converted `level_info_next_url` leaf computes from `level_info_ptr()`.
fn level_info_next_url_ptr() -> *mut c_void {
    LEVEL_INFO_MEM.with(|m| {
        let r = m.borrow();
        std::ptr::addr_of!(r.next_url) as *mut c_void
    })
}

/// Syncs the shared mock `ALevelInfo`'s fields read by converted leaves that
/// are driven by scenario thread-locals, called on every `level_get_level_info`.
fn sync_level_info() {
    LEVEL_INFO_MEM.with(|m| {
        let mut li = m.borrow_mut();
        li.set_b_lone_player(LONE_PLAYER.with(Cell::get) != 0);
        li.game = fake_handle();
    });
}

/// Marks the shared mock `ULevel`'s `_Linker` as present, for scenarios
/// driven by `LINKER_PRESENT`.
unsafe fn set_level_linker_present(level: &mut ULevel) {
    let obj = (level as *mut ULevel).cast::<UObject>();
    unsafe { (*obj)._linker = fake_handle() };
}


unsafe extern "C" fn fstring_ne_text(h: *mut c_void, text: *const u32, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            if DEMO_FILE.with(Cell::get) {
                0
            } else if h == level_info_next_url_ptr() {
                NEXT_URL_NONEMPTY.with(Cell::get) as i32
            } else {
                (decode(h.cast()) != decode(text)) as i32
            }
        })();
    }
    0
}

unsafe extern "C" fn fstring_append_text(_h: *mut c_void, _text: *const u32) -> i32 {
    (|| {})();
    0
}

unsafe extern "C" fn debugf_log(text: *const u32) -> i32 {
    (|| {
        // SAFETY: forwarded TCHAR string is NUL-terminated.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
        });
    })();
    0
}

unsafe extern "C" fn output_device_logf(_ar: *mut c_void, text: *const u32) -> i32 {
    (|| {
        // SAFETY: forwarded TCHAR string is NUL-terminated.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("debugf\tlog\t{}", unsafe { decode(text) }))
        });
    })();
    0
}

unsafe extern "C" fn static_load_class(
    _base: i32,
    name: *const u32,
    result: *mut *mut c_void,
) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void {
            // SAFETY: the class name is a NUL-terminated TCHAR string.
            DEBUGF.with(|d| {
                d.borrow_mut()
                    .push(format!("load_class\t{}", decode(name)))
            });
            0x90 as *mut c_void
        })();
    }
    0
}

unsafe extern "C" fn construct_net_driver(_cls: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void {
            DEBUGF.with(|d| d.borrow_mut().push("construct_object\tdemo".to_string()));
            0xA0 as *mut c_void
        })();
    }
    0
}

unsafe extern "C" fn netdriver_init_listen(
    _drv: *mut c_void,
    _level: *mut c_void,
    _url: *mut c_void,
    error: *mut c_void,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let result = DEMO_INIT_OK.with(Cell::get) as i32;
            if result == 0 {
                ERROR_TEXT.with(|error| *error.borrow_mut() = "Port busy".to_string());
                if DEMO_FILE.with(Cell::get) {
                    // The recording path uses the mutable FString from `fstring_new`;
                    // Listen passes the static `PORT_BUSY` fixture buffer instead.
                    assign_wide(error.cast(), "Port busy");
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
        })();
    }
    0
}
unsafe extern "C" fn static_construct_net_driver(
    _cls: *mut c_void,
    result: *mut *mut c_void,
) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void {
            DEBUGF.with(|d| d.borrow_mut().push("construct_object\tnet".to_string()));
            net_driver_ptr()
        })();
    }
    0
}


unsafe extern "C" fn cast_game_engine(_engine: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void { fake_handle() })();
    }
    0
}

unsafe extern "C" fn game_build_server_master_map(
    _ge: *mut c_void,
    _drv: *mut c_void,
    _level: *mut c_void,
) -> i32 {
    (|| {
        DEBUGF.with(|d| d.borrow_mut().push("build_master_map".to_string()));
    })();
    0
}

unsafe extern "C" fn game_engine_server_actors_num(_ge: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 { (ACTOR_MODE.with(Cell::get) != 0) as i32 })();
    }
    0
}

unsafe extern "C" fn engine_client_viewports_num(_client: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 { 1 })();
    }
    0
}

unsafe extern "C" fn engine_client_viewport(
    _client: *mut c_void,
    _index: i32,
    result: *mut *mut c_void,
) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void { fake_handle() })();
    }
    0
}

unsafe extern "C" fn engine_set_client_travel(
    _engine: *mut c_void,
    _viewport: *mut c_void,
    url: *const u32,
    _items: i32,
    _travel: i32,
) -> i32 {
    (|| {
        CLIENT_TRAVEL.with(|travel| *travel.borrow_mut() = unsafe { decode(url) });
    })();
    0
}





unsafe extern "C" fn engine_challenge_response(
    _engine: *mut c_void,
    challenge: u32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let response = CHALLENGE_RESPONSE.with(Cell::get);
            DEBUGF.with(|records| {
                records
                    .borrow_mut()
                    .push(format!("challenge_response\t{challenge}\t{response}"))
            });
            response
        })();
    }
    0
}

unsafe extern "C" fn parse_guid(
    _text: *const u32,
    _key: *const u32,
    out: *mut u32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            for index in 0..4 {
                *out.add(index) = 0;
            }
            1
        })();
    }
    0
}

unsafe extern "C" fn package_map_list_num(_pm: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 { 0 })();
    }
    0
}

unsafe extern "C" fn level_get_level_info(_level: *mut c_void, result: *mut *mut c_void) -> i32 {
    unsafe {
        *result = (|| -> *mut c_void {
            sync_level_info();
            level_info_ptr()
        })();
    }
    0
}



unsafe extern "C" fn game_engine_server_actor_text(
    _ge: *mut c_void,
    _i: i32,
    result: *mut *const u32,
) -> i32 {
    unsafe {
        *result = (|| -> *const u32 { ENGINE_LEVEL.as_ptr() })();
    }
    0
}

unsafe extern "C" fn parse_token_buf(
    _stream: *mut *const u32,
    buf: *mut u32,
    _max: i32,
    _escape: i32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let mode = ACTOR_MODE.with(Cell::get);
            let call = PARSE_TOKEN_CALL.with(|n| {
                let previous = n.get();
                n.set(previous + 1);
                previous
            });
            let token: &[u32] = match (mode, call) {
                (1, 0) | (2..=5, 0) => &[
                    'E' as u32, 'n' as u32, 'g' as u32, 'i' as u32, 'n' as u32, 'e' as u32,
                    '.' as u32, 'H' as u32, 'e' as u32, 'l' as u32, 'p' as u32, 'e' as u32,
                    'r' as u32, 0,
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
                *buf.add(i) = *ch;
            }
            1
        })();
    }
    0
}

unsafe extern "C" fn level_spawn_actor_construct(
    _ctx: *const Hp2SpawnActorContext,
    _template: usize,
    out: *mut usize,
) -> i32 {
    unsafe {
        *out = (|| -> usize {
            if ACTOR_MODE.with(Cell::get) == 1 {
                DEBUGF.with(|d| d.borrow_mut().push("spawn_helper\tnull".to_string()));
                0
            } else {
                DEBUGF.with(|d| d.borrow_mut().push("spawn_helper\tactor".to_string()));
                actor_ptr() as usize
            }
        })();
    }
    0
}

unsafe extern "C" fn app_strchr(text: *const u32, ch: u32, result: *mut *const u32) -> i32 {
    unsafe {
        *result = (|| -> *const u32 {
            let mut p = text;
            loop {
                let value = *p;
                if value == ch || value == 0 {
                    return if value == ch { p } else { std::ptr::null() };
                }
                p = p.add(1);
            }
        })();
    }
    0
}


unsafe extern "C" fn tfield_iter_valid(_it: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 { FIELD_ITER_VALID.with(Cell::get) as i32 })();
    }
    0
}

unsafe extern "C" fn tfield_iter_next(_it: *mut c_void) -> i32 {
    (|| {
        FIELD_ITER_VALID.with(|valid| valid.set(false));
    })();
    0
}

unsafe extern "C" fn uproperty_name_stricmp(
    _it: *mut c_void,
    name: *const u32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let name = decode(name);
            let expected_match = ACTOR_MODE.with(Cell::get) != 5;
            if expected_match && name == "Flag" {
                0
            } else {
                1
            }
        })();
    }
    0
}

unsafe extern "C" fn uproperty_is_config(_it: *mut c_void, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 { (ACTOR_MODE.with(Cell::get) == 3) as i32 })();
    }
    0
}

unsafe extern "C" fn uproperty_import_text(
    _it: *mut c_void,
    text: *const u32,
    _obj: *mut c_void,
) -> i32 {
    (|| {
        // SAFETY: imported text is a NUL-terminated TCHAR string.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("import_text\tFlag\t{}", unsafe { decode(text) }))
        });
    })();
    0
}

unsafe extern "C" fn netdriver_delete(_drv: *mut c_void) -> i32 {
    (|| {
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
    })();
    0
}
unsafe extern "C" fn level_exec_net(
    _drv: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            // SAFETY: command is a NUL-terminated TCHAR string.
            DEBUGF.with(|d| {
                d.borrow_mut()
                    .push(format!("net_exec\tnet\t{}", decode(cmd)))
            });
            EXEC_NET_RESULT.with(Cell::get)
        })();
    }
    0
}

unsafe extern "C" fn level_exec_demo(
    _drv: *mut c_void,
    cmd: *const u32,
    _ar: *mut c_void,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            // SAFETY: command is a NUL-terminated TCHAR string.
            DEBUGF.with(|d| {
                d.borrow_mut()
                    .push(format!("net_exec\tdemo\t{}", decode(cmd)))
            });
            EXEC_DEMO_RESULT.with(Cell::get)
        })();
    }
    0
}
unsafe extern "C" fn localize_error_assign(
    _level: *mut c_void,
    _error: *mut c_void,
    key: *const i8,
) -> i32 {
    (|| {
        let key = unsafe { std::ffi::CStr::from_ptr(key) }.to_string_lossy();
        let error = match key.as_ref() {
            "NetAlready" => "Already listening",
            "NetListen" => "Listen failed",
            _ => "",
        };
        ERROR_TEXT.with(|value| *value.borrow_mut() = error.to_string());
        DEBUGF.with(|d| d.borrow_mut().push(format!("localize_error\t{key}")));
    })();
    0
}

unsafe extern "C" fn uobject_get_name(obj: *mut c_void, result: *mut *const u32) -> i32 {
    unsafe {
        *result = (|| -> *const u32 {
            // The outer package is always named "Level" (welcome logs). The level is named only after the
            // scenario's level_with_levelinfo() (level_harness.cpp:621); begin() zeroes it, so it is "" otherwise.
            if obj == fake_handle() || LEVEL_NAMED.with(Cell::get) {
                LEVEL_NAME.as_ptr()
            } else {
                EMPTY_TEXT.as_ptr()
            }
        })();
    }
    0
}


unsafe extern "C" fn app_timestamp(result: *mut *const u32) -> i32 {
    unsafe {
        *result = (|| -> *const u32 { TIMESTAMP.as_ptr() })();
    }
    0
}

unsafe extern "C" fn debugf_net_come_go(text: *const u32) -> i32 {
    (|| {
        // SAFETY: level_net forwards a NUL-terminated TCHAR string.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("debugf\tnet_come_go\t{}", unsafe { decode(text) }))
        });
    })();
    0
}



unsafe extern "C" fn package_map_copy(_pm: *mut c_void, _master: *mut c_void) -> i32 {
    (|| {
        DEBUGF.with(|d| d.borrow_mut().push("package_map_copy".to_string()));
    })();
    0
}

unsafe extern "C" fn connection_send_package_map(_conn: *mut c_void) -> i32 {
    (|| {
        DEBUGF.with(|d| d.borrow_mut().push("send_package_map".to_string()));
    })();
    0
}



unsafe extern "C" fn connection_log(_conn: *mut c_void, text: *const u32) -> i32 {
    (|| {
        // SAFETY: forwarded TCHAR string is NUL-terminated.
        DEBUGF.with(|d| {
            d.borrow_mut()
                .push(format!("conn_log\t{}", unsafe { decode(text) }))
        });
    })();
    0
}

unsafe extern "C" fn connection_flush_net(_conn: *mut c_void) -> i32 {
    (|| {
        DEBUGF.with(|d| d.borrow_mut().push("conn_flush".to_string()));
    })();
    0
}

unsafe extern "C" fn app_atoi(text: *const u32, result: *mut i32) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let text = decode(text);
            text.trim().parse().unwrap_or(0)
        })();
    }
    0
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
unsafe extern "C" fn connection_channel_close(_conn: *mut c_void, _i: i32) -> i32 {
    (|| {
        DEBUGF.with(|d| d.borrow_mut().push("channel_close".to_string()));
    })();
    0
}




unsafe extern "C" fn parse_int(
    _text: *const u32,
    key: *const u32,
    out: *mut i32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let key = decode(key);
            if key == "MINVER=" || key == "VER=" {
                *out = 219;
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
                *out = response;
                return 1;
            }
            0
        })();
    }
    0
}
unsafe extern "C" fn parse_text(
    _text: *const u32,
    key: *const u32,
    out: *mut u32,
    max: i32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (|| -> i32 {
            let key = decode(key);
            let command = EXEC_COMMAND.with(|command| command.borrow().clone());
            let Some((_, value)) = command.split_once(key.as_str()) else {
                *out = 0;
                return 0;
            };
            let value = value.trim_start();
            let chars = value
                .encode_utf16()
                .map(u32::from)
                .take(max.saturating_sub(1) as usize);
            let mut index = 0;
            for ch in chars {
                *out.add(index) = ch;
                index += 1;
            }
            *out.add(index) = 0;
            1
        })();
    }
    0
}

unsafe extern "C" fn connection_set_request_url(_conn: *mut c_void, text: *const u32) -> i32 {
    (|| {
        let text = unsafe { decode(text) };
        REQUEST_URL.with(|url| *url.borrow_mut() = wide(&text));
    })();
    0
}

unsafe extern "C" fn connection_request_url_text(
    _conn: *mut c_void,
    result: *mut *const u32,
) -> i32 {
    unsafe {
        *result = (|| -> *const u32 { REQUEST_URL.with(|url| url.borrow().as_ptr()) })();
    }
    0
}


unsafe extern "C" fn game_pre_login(
    _game: *mut c_void,
    _conn: *mut c_void,
    options: *const u32,
    error: *mut c_void,
    fail_code: *mut c_void,
) -> i32 {
    (|| {
        let options = unsafe { decode(options) };
        let address = unsafe { decode(REMOTE_ADDRESS.as_ptr()) };
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
    })();
    0
}

unsafe fn assign_wide(out: *mut u32, text: &str) {
    for (index, ch) in text.encode_utf16().map(u32::from).enumerate() {
        unsafe { *out.add(index) = ch };
    }
    unsafe { *out.add(text.encode_utf16().count()) = 0 };
}

unsafe extern "C" fn level_welcome_player(level: *mut c_void, conn: *mut c_void) -> i32 {
    let services = services();
    match unsafe { level_net::welcome_player(&services, level, conn, EMPTY_TEXT.as_ptr()) } {
        Ok(()) => 0,
        Err(()) => 1,
    }
}





/// Borrows the static `"Engine.Level"` text buffer.
unsafe extern "C" fn uobject_get_full_name(_obj: *mut c_void, result: *mut *const u32) -> i32 {
    unsafe {
        *result = (|| -> *const u32 { ENGINE_LEVEL.as_ptr() })();
    }
    0
}

unsafe extern "C" fn fstring_text(h: *mut c_void, result: *mut *const u32) -> i32 {
    unsafe {
        *result = (|| -> *const u32 {
            if h == url_map_ptr() {
                if EXEC_COMMAND.with(|c| c.borrow().contains("demo1")) {
                    DEMO1.as_ptr()
                } else {
                    DEMO2.as_ptr()
                }
            } else {
                h as *const u32
            }
        })();
    }
    0
}

unsafe extern "C" fn string_scope(
    source: *mut c_void,
    context: *mut c_void,
    body: Hp2LevelNetContinuation,
) -> i32 {
    let mut storage = [0u32; 128];
    let local = if source.is_null() {
        storage.as_mut_ptr().cast()
    } else {
        REMOTE_ADDRESS.as_ptr() as *mut c_void
    };
    unsafe { body(context, local) }
}
unsafe extern "C" fn url_scope(
    _text: *const u32,
    context: *mut c_void,
    body: Hp2LevelNetContinuation,
) -> i32 {
    unsafe { body(context, url_ptr()) }
}
unsafe extern "C" fn property_scope(
    _class: *mut c_void,
    context: *mut c_void,
    body: Hp2LevelNetContinuation,
) -> i32 {
    FIELD_ITER_VALID.with(|valid| valid.set(true));
    unsafe { body(context, fake_handle()) }
}
unsafe extern "C" fn fstring_right_ne_text(
    _h: *mut c_void,
    _n: i32,
    text: *const u32,
    result: *mut i32,
) -> i32 {
    unsafe {
        *result = (decode(DEM_SUFFIX.as_ptr()) != decode(text)) as i32;
    }
    0
}

/// A table where every field panics, with the used services overridden.
fn services() -> Hp2LevelNetServices {
    let stub: unsafe extern "C" fn() = unused;
    let raw = [stub as usize; SERVICE_COUNT];
    // SAFETY: the struct is `SERVICE_COUNT` pointer-sized `repr(C)` fields and
    // `usize` has the same size and alignment as a function pointer.
    let mut s: Hp2LevelNetServices = unsafe { std::mem::transmute(raw) };
    s.string_scope = string_scope;
    s.url_scope = url_scope;
    s.property_scope = property_scope;
    s.fstring_right_ne_text = fstring_right_ne_text;
    s.check_failed = check_failed;
    s.debugf_dev_net = debugf_dev_net;
    s.uobject_get_full_name = uobject_get_full_name;
    s.fstring_text = fstring_text;
    s.level_exec_net = level_exec_net;
    s.level_exec_demo = level_exec_demo;
    s.output_device_log = output_device_log;
    s.parse_command = parse_command;
    s.parse_token_fstring = parse_token_fstring;
    s.debugf_log = debugf_log;
    s.output_device_logf = output_device_logf;
    s.fstring_ne_text = fstring_ne_text;
    s.fstring_append_text = fstring_append_text;
    s.static_load_class = static_load_class;
    s.construct_net_driver = construct_net_driver;
    s.netdriver_init_listen = netdriver_init_listen;
    s.netdriver_delete = netdriver_delete;
    s.static_construct_net_driver = static_construct_net_driver;
    s.cast_game_engine = cast_game_engine;
    s.game_build_server_master_map = game_build_server_master_map;
    s.game_engine_server_actors_num = game_engine_server_actors_num;
    s.engine_client_viewports_num = engine_client_viewports_num;
    s.engine_client_viewport = engine_client_viewport;
    s.engine_set_client_travel = engine_set_client_travel;
    s.engine_challenge_response = engine_challenge_response;
    s.parse_guid = parse_guid;
    s.package_map_list_num = package_map_list_num;
    s.parse_text = parse_text;
    s.connection_set_request_url = connection_set_request_url;
    s.connection_request_url_text = connection_request_url_text;
    s.game_pre_login = game_pre_login;
    s.level_welcome_player = level_welcome_player;
    s.level_get_level_info = level_get_level_info;
    s.game_engine_server_actor_text = game_engine_server_actor_text;
    s.parse_token_buf = parse_token_buf;
    level_spawn_actor_mock::publish(level_spawn_actor_construct);
    s.app_strchr = app_strchr;
    s.tfield_iter_valid = tfield_iter_valid;
    s.tfield_iter_next = tfield_iter_next;
    s.uproperty_name_stricmp = uproperty_name_stricmp;
    s.uproperty_is_config = uproperty_is_config;
    s.uproperty_import_text = uproperty_import_text;

    s.localize_error_assign = localize_error_assign;
    s.uobject_get_name = uobject_get_name;
    s.app_timestamp = app_timestamp;
    s.debugf_net_come_go = debugf_net_come_go;
    s.package_map_copy = package_map_copy;
    s.connection_send_package_map = connection_send_package_map;
    s.connection_log = connection_log;
    s.connection_flush_net = connection_flush_net;
    s.app_atoi = app_atoi;
    s.app_errorf = app_errorf;
    s.connection_channel_close = connection_channel_close;
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
    PRELOGIN_REJECT.with(|reject| reject.set(false));
    REQUEST_URL.with(|url| url.borrow_mut().clear());
    CLIENT_TRAVEL.with(|travel| travel.borrow_mut().clear());
    NET_SPEED.with(|speed| speed.set(0));
    CHALLENGE_RESPONSE.with(|response| response.set(0));
    NET_DRIVER_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    CONNECTION_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    CHANNEL_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    GAME_ENGINE_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    LEVEL_INFO_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    URL_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    ACTOR_MEM.with(|m| *m.borrow_mut() = Box::new(unsafe { std::mem::zeroed() }));
    let mut level = unsafe { std::mem::zeroed::<ULevel>() };
    let level_ptr = (&mut level as *mut ULevel).cast::<c_void>();
    // `UObject::GetOuter()` always returns a non-null constant in these scenarios.
    unsafe { (*level_ptr.cast::<UObject>()).outer = fake_handle() };
    let ret = match name {
        "net.notify_accepting_connection_client" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: level and connection-state services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepting_connection_switching" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            NEXT_URL_NONEMPTY.with(|c| c.set(true));
            // Harness calls level_with_levelinfo() here (level_harness.cpp:1508), naming the level.
            LEVEL_NAMED.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: level and next-URL services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepting_connection_accept" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            NEXT_URL_NONEMPTY.with(|c| c.set(false));
            // Harness calls level_with_levelinfo() here (level_harness.cpp:1514), naming the level.
            LEVEL_NAMED.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: level and next-URL services are mocked.
            unsafe { level_net::notify_accepting_connection(&s, level_ptr) }.unwrap()
        }
        "net.notify_accepted_connection" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: mocked level, connection, time, and address services are valid.
            unsafe { level_net::notify_accepted_connection(&s, level_ptr, 0x50 as _) }.unwrap();
            0
        }
        "net.welcome_player_default" => {
            LONE_PLAYER.with(|c| c.set(0));
            sync_net_driver();
            sync_connection_driver();
            // SAFETY: welcome services are mocked and do not dereference opaque handles.
            unsafe { level_net::welcome_player(&s, level_ptr, connection_ptr(), EMPTY_TEXT.as_ptr()) }
                .unwrap();
            0
        }
        "net.welcome_player_optional" => {
            LONE_PLAYER.with(|c| c.set(1));
            sync_net_driver();
            sync_connection_driver();
            let optional = wide("extra");
            // SAFETY: optional string and mocked services are live.
            unsafe { level_net::welcome_player(&s, level_ptr, connection_ptr(), optional.as_ptr()) }
                .unwrap();
            0
        }
        "net.notify_received_text_userflag" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "USERFLAG 7".to_string());
            let text = wide("USERFLAG 7");
            // SAFETY: message and connection remain live for the call.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            let flags = CONNECTION_MEM.with(|m| m.borrow().user_flags);
            DEBUGF.with(|d| d.borrow_mut().push(format!("user_flags\t{flags}")));
            0
        }
        "net.notify_received_text_server_hello" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "HELLO MINVER=219 VER=219".to_string());
            let text = wide("HELLO MINVER=219 VER=219");
            // SAFETY: message, connection, engine client, and version services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_client_failure" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "FAILURE rejected".to_string());
            let text = wide("FAILURE rejected");
            // SAFETY: all reached client-failure services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_netspeed_high" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "NETSPEED 7000".to_string());
            let text = wide("NETSPEED 7000");
            // SAFETY: all reached netspeed services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_netspeed_low" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "NETSPEED 1000".to_string());
            let text = wide("NETSPEED 1000");
            // SAFETY: all reached netspeed services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_have" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            EXEC_COMMAND.with(|c| {
                *c.borrow_mut() = "HAVE GUID=00000000000000000000000000000000".to_string()
            });
            let text = wide("HAVE GUID=00000000000000000000000000000000");
            // SAFETY: package-map and GUID parsing services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_skip" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            EXEC_COMMAND.with(|c| {
                *c.borrow_mut() = "SKIP GUID=00000000000000000000000000000000".to_string()
            });
            let text = wide("SKIP GUID=00000000000000000000000000000000");
            // SAFETY: package-map and GUID parsing services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_login_missing_response" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            sync_connection_driver();
            CONNECTION_MEM.with(|m| m.borrow_mut().remote_version = 219);
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN URL=?Name=Guest".to_string());
            let text = wide("LOGIN URL=?Name=Guest");
            // SAFETY: challenge and failure-response services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_login_precedence_false" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            sync_connection_driver();
            CONNECTION_MEM.with(|m| m.borrow_mut().remote_version = 219);
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=0".to_string());
            let text = wide("LOGIN RESPONSE=0");
            // SAFETY: challenge and failure-response services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_login_success" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            sync_connection_driver();
            CONNECTION_MEM.with(|m| m.borrow_mut().remote_version = 219);
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=1 URL=?Name=Guest".to_string());
            REQUEST_URL.with(|url| url.borrow_mut().clear());
            let text = wide("LOGIN RESPONSE=1 URL=?Name=Guest");
            // SAFETY: challenge, request URL, PreLogin, and Welcome services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_text_login_prelogin_reject" => {
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            sync_connection_driver();
            CONNECTION_MEM.with(|m| m.borrow_mut().remote_version = 219);
            CHALLENGE_RESPONSE.with(|c| c.set(1));
            PRELOGIN_REJECT.with(|c| c.set(true));
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "LOGIN RESPONSE=1 URL=?Name=Guest".to_string());
            REQUEST_URL.with(|url| url.borrow_mut().clear());
            let text = wide("LOGIN RESPONSE=1 URL=?Name=Guest");
            // SAFETY: challenge and PreLogin rejection services are mocked.
            unsafe {
                level_net::notify_received_text(&s, level_ptr, connection_ptr(), text.as_ptr())
            }
            .unwrap();
            0
        }
        "net.notify_received_file" => {
            // SAFETY: the mock records the error without unwinding across FFI.
            unsafe { level_net::notify_received_file(&s) }.unwrap();
            0
        }
        "net.notify_sending_file_server" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: `level` is a live, exclusively owned `ULevel`.
            unsafe { level_net::notify_sending_file(&s, level_ptr) }.unwrap()
        }
        "net.notify_sending_file_client" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: as above.
            unsafe { level_net::notify_sending_file(&s, level_ptr) }.unwrap()
        }
        "net.accepting_channel_client_refuse" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            sync_net_driver();
            sync_channel(level_net::CHTYPE_CONTROL, 0);
            // SAFETY: mocked channel/connection/driver memory backs the handle.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, channel_ptr()) }.unwrap()
        }
        "net.accepting_channel_server_control" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            sync_channel(level_net::CHTYPE_CONTROL, 0);
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, channel_ptr()) }.unwrap()
        }
        "net.accepting_channel_server_file" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            sync_channel(level_net::CHTYPE_FILE, 0);
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, channel_ptr()) }.unwrap()
        }
        "net.accepting_channel_server_refuse" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            sync_channel(level_net::CHTYPE_ACTOR, 1);
            // SAFETY: as above.
            unsafe { level_net::notify_accepting_channel(&s, level_ptr, channel_ptr()) }.unwrap()
        }
        "net.exec_net_handled" => {
            level.base.net_driver = 0x20 as *mut _;
            EXEC_NET_RESULT.with(|c| c.set(1));
            // SAFETY: the command is a live NUL-terminated TCHAR literal.
            unsafe { level_net::exec(&s, level_ptr, wide("NETHANDLED").as_ptr(), 0x60 as _) }
                .unwrap()
        }
        "net.exec_demo_handled" => {
            level.base.demo_rec_driver = 0x30 as *mut _;
            EXEC_DEMO_RESULT.with(|c| c.set(1));
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOHANDLED").as_ptr(), 0x60 as _) }
                .unwrap()
        }
        "net.exec_unhandled" => {
            level.base.net_driver = 0x20 as *mut _;
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("FOO").as_ptr(), 0x60 as _) }.unwrap()
        }
        "net.exec_demorec_no_filename" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC".to_string());
            // SAFETY: command and output device pointers are live for the call.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC").as_ptr(), 0x60 as _) }.unwrap()
        }
        "net.exec_demorec_init_fail" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC demo1".to_string());
            DEMO_FILE.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(false));
            // SAFETY: scenario-owned inputs and mocked services remain live.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC demo1").as_ptr(), 0x60 as _) }
                .unwrap()
        }
        "net.exec_demorec_started" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOREC demo2.dem".to_string());
            DEMO_FILE.with(|c| c.set(true));
            DEMO_INIT_OK.with(|c| c.set(true));
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOREC demo2.dem").as_ptr(), 0x60 as _) }
                .unwrap()
        }

        "net.exec_demoplay_no_filename" => {
            EXEC_COMMAND.with(|c| *c.borrow_mut() = "DEMOPLAY".to_string());
            // SAFETY: as above.
            unsafe { level_net::exec(&s, level_ptr, wide("DEMOPLAY").as_ptr(), 0x60 as _) }.unwrap()
        }
        "net.is_server_driver_server" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: `level` is live; the driver callback is mocked.
            unsafe { level_net::is_server(&s, level_ptr) }.unwrap()
        }
        "net.is_server_driver_client" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }.unwrap()
        }
        "net.is_server_demo_client" => {
            SERVER_CONNECTION.with(|c| c.set(true));
            sync_net_driver();
            level.base.demo_rec_driver = net_driver_ptr();
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }.unwrap()
        }
        "net.is_server_demo_present" => {
            SERVER_CONNECTION.with(|c| c.set(false));
            sync_net_driver();
            level.base.net_driver = net_driver_ptr();
            level.base.demo_rec_driver = net_driver_ptr();
            // SAFETY: as above.
            unsafe { level_net::is_server(&s, level_ptr) }.unwrap()
        }
        "net.listen_already_listening" => {
            level.base.net_driver = net_driver_ptr();
            // SAFETY: the level is live; localized error assignment is mocked.
            unsafe { level_net::listen(&s, level_ptr, 0x60 as _) }.unwrap()
        }
        "net.listen_no_linker" => {
            // SAFETY: as above; the linker callback reports the fixture's null linker.
            unsafe { level_net::listen(&s, level_ptr, 0x60 as _) }.unwrap()
        }
        "net.listen_init_fail" => {
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(false));
            sync_net_driver();
            // SAFETY: level/linker/init mocks model this fixture path.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_ok" | "net.listen_server_actor_blank" => {
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            if name == "net.listen_server_ok" {
                TRAVEL_PAUSE.with(|pause| pause.set(2.5));
            }
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: all reached level/game/driver services are mocked.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_dedicated_ok" => {
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(false));
            TRAVEL_PAUSE.with(|pause| pause.set(2.5));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_actor_spawn_null" => {
            ACTOR_MODE.with(|mode| mode.set(1));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: scenario-specific actor and listen services are mocked.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_actor_bare_token" => {
            ACTOR_MODE.with(|mode| mode.set(2));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_actor_config_field" => {
            ACTOR_MODE.with(|mode| mode.set(3));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_actor_non_config_field" => {
            ACTOR_MODE.with(|mode| mode.set(4));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
        }
        "net.listen_server_actor_name_mismatch" => {
            ACTOR_MODE.with(|mode| mode.set(5));
            PARSE_TOKEN_CALL.with(|call| call.set(0));
            LINKER_PRESENT.with(|c| c.set(true));
            unsafe { set_level_linker_present(&mut level) };
            DEMO_INIT_OK.with(|c| c.set(true));
            CLIENT_PRESENT.with(|c| c.set(true));
            sync_net_driver();
            sync_game_engine();
            level.base.engine = game_engine_ptr();
            // SAFETY: as above.
            unsafe { level_net::listen(&s, level_ptr, PORT_BUSY.as_ptr() as *mut c_void) }.unwrap()
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
        let speed = CONNECTION_MEM.with(|m| m.borrow().current_net_speed);
        NET_SPEED.with(|value| value.set(speed));
        NET_SPEED.with(|speed| lines.push(format!("net_speed\t{}", speed.get())));
    }
    if name.starts_with("net.listen_") {
        let error = ERROR_TEXT.with(|value| value.borrow().clone());
        lines.push(format!("error_text\t{error}"));
        lines.push(format!(
            "net_driver\t{}",
            u8::from(!level.base.net_driver.is_null())
        ));
        if ret == 1 {
            LEVEL_INFO_MEM.with(|m| {
                let li = m.borrow();
                NET_MODE.with(|mode| mode.set(li.net_mode));
                COUNTDOWN.with(|countdown| countdown.set(li.next_switch_countdown));
            });
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
