//! `ULevel` network section of `Engine/Src/UnLevel.cpp`: `Exec`, `Listen`,
//! `IsServer`, the `NotifyAccepting*`/`NotifyAccepted*` callbacks,
//! `WelcomePlayer`, `NotifyReceivedText`, `NotifySendingFile`.
//!
//! Every engine call goes through the `Hp2LevelNetServices` table, one service
//! per original external call, in original order. `ULevel` scalars
//! (`NetDriver`, `DemoRecDriver`, `Engine`) are read and written through the
//! generated mirror. Log text is formatted here as TCHAR (`u32`) buffers and
//! forwarded verbatim by the C++ adapters.
//!
//! `NotifyReceivedFile` stays in C++ (`appErrorf` only).

use std::ffi::{CStr, c_void};

use hp2_abi::engine::generated::ULevel;

/// `ACCEPTC_Reject` / `ACCEPTC_Accept` / `ACCEPTC_Ignore` (UnLevel.h).
pub const ACCEPTC_REJECT: i32 = 0;
pub const ACCEPTC_ACCEPT: i32 = 1;
pub const ACCEPTC_IGNORE: i32 = 2;
/// `NM_DedicatedServer` / `NM_ListenServer` (Engine enums).
pub const NM_DEDICATED_SERVER: u8 = 1;
pub const NM_LISTEN_SERVER: u8 = 2;
/// `USOCK_Closed`.
pub const USOCK_CLOSED: i32 = 1;
/// `TRAVEL_Absolute` (EngineClasses.h).
pub const TRAVEL_ABSOLUTE: i32 = 0;
/// `ROLE_AutonomousProxy` (EngineClasses.h).
pub const ROLE_AUTONOMOUS_PROXY: i32 = 3;
/// `CHTYPE_Control` / `CHTYPE_Actor` / `CHTYPE_File` (UnNet.h).
pub const CHTYPE_CONTROL: i32 = 1;
pub const CHTYPE_ACTOR: i32 = 2;
pub const CHTYPE_FILE: i32 = 3;
/// `ENGINE_MIN_NET_VERSION` / `ENGINE_VERSION` (UnObjVer.h).
pub const ENGINE_MIN_NET_VERSION: i32 = 400;
pub const ENGINE_VERSION: i32 = 433;

const FILE_NAME: &CStr = c"UnLevel.cpp";

/// Services table. Every field is one external call from the Net section.
/// Layout must match `Hp2LevelNetServices` in `Hp2LevelServices.h`.
#[repr(C)]
pub struct Hp2LevelNetServices {
    // --- debug / output / errors ---
    /// `debugf(text)` (default name).
    pub debugf_log: unsafe extern "C" fn(text: *const u32),
    /// `debugf(NAME_DevNet, "%s", text)`.
    pub debugf_dev_net: unsafe extern "C" fn(text: *const u32),
    /// `debugf(NAME_NetComeGo, "%s", text)`.
    pub debugf_net_come_go: unsafe extern "C" fn(text: *const u32),
    /// `Ar.Log(text)`.
    pub output_device_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `Ar.Logf("%s", text)`.
    pub output_device_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `Connection->Logf("%s", text)`.
    pub connection_log: unsafe extern "C" fn(conn: *mut c_void, text: *const u32),
    /// `appFailAssert(expr, file, line)`, captured in C++ and reported as a nonzero status.
    pub check_failed: unsafe extern "C" fn(expr: *const i8, file: *const i8, line: i32) -> i32,
    /// `appErrorf(text)`, captured in C++ and reported as a nonzero status.
    pub app_errorf: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `Error = Level->LocalizeError(key)`.
    pub localize_error_assign:
        unsafe extern "C" fn(level: *mut c_void, error: *mut c_void, key: *const i8),
    // --- UObject / app ---
    pub uobject_get_name: unsafe extern "C" fn(obj: *mut c_void) -> *const u32,
    pub uobject_get_full_name: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    pub uobject_get_outer: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `ULevel::GetLinker()`.
    pub level_get_linker: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    pub app_timestamp: unsafe extern "C" fn() -> *const u32,
    pub app_cycles: unsafe extern "C" fn() -> u32,
    pub app_atoi: unsafe extern "C" fn(text: *const u32) -> i32,
    pub app_strchr: unsafe extern "C" fn(text: *const u32, ch: u32) -> *const u32,
    // --- FString (heap handles, owned unless noted) ---
    pub fstring_new: unsafe extern "C" fn() -> *mut c_void,
    pub fstring_delete: unsafe extern "C" fn(h: *mut c_void),
    pub fstring_text: unsafe extern "C" fn(h: *mut c_void) -> *const u32,
    pub fstring_assign: unsafe extern "C" fn(dst: *mut c_void, src: *mut c_void),
    pub fstring_append_text: unsafe extern "C" fn(h: *mut c_void, text: *const u32),
    pub fstring_ne_text: unsafe extern "C" fn(h: *mut c_void, text: *const u32) -> i32,
    /// `h.Right(n)`, owned result.
    pub fstring_right: unsafe extern "C" fn(h: *mut c_void, n: i32) -> *mut c_void,
    /// `h.Left(n)`, owned result.
    pub fstring_left: unsafe extern "C" fn(h: *mut c_void, n: i32) -> *mut c_void,
    /// `h.Mid(n)`, owned result.
    pub fstring_mid: unsafe extern "C" fn(h: *mut c_void, n: i32) -> *mut c_void,
    /// `a + b`, owned result.
    pub fstring_add: unsafe extern "C" fn(a: *mut c_void, b: *mut c_void) -> *mut c_void,
    /// `h.Caps().InStr(needle)`.
    pub fstring_caps_in_str: unsafe extern "C" fn(h: *mut c_void, needle: *const u32) -> i32,
    // --- FURL (heap handles, owned) ---
    pub furl_new: unsafe extern "C" fn() -> *mut c_void,
    /// `FURL(NULL, text, travel)`.
    pub furl_new_travel: unsafe extern "C" fn(text: *const u32, travel: i32) -> *mut c_void,
    pub furl_delete: unsafe extern "C" fn(h: *mut c_void),
    /// `URL.Map` (not owned).
    pub furl_map: unsafe extern "C" fn(h: *mut c_void) -> *mut c_void,
    // --- Parse helpers (stream pointers updated in place) ---
    pub parse_command: unsafe extern "C" fn(stream: *mut *const u32, keyword: *const u32) -> i32,
    pub parse_token_fstring:
        unsafe extern "C" fn(stream: *mut *const u32, token: *mut c_void, escape: i32) -> i32,
    pub parse_token_buf:
        unsafe extern "C" fn(stream: *mut *const u32, buf: *mut u32, max: i32, escape: i32) -> i32,
    pub parse_int: unsafe extern "C" fn(text: *const u32, key: *const u32, out: *mut i32) -> i32,
    pub parse_text:
        unsafe extern "C" fn(text: *const u32, key: *const u32, buf: *mut u32, max: i32) -> i32,
    pub parse_guid: unsafe extern "C" fn(text: *const u32, key: *const u32, out: *mut u32) -> i32,
    // --- Level / net driver ---
    /// `NetDriver->Exec(cmd, ar)`.
    pub level_exec_net:
        unsafe extern "C" fn(drv: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `DemoRecDriver->Exec(cmd, ar)`.
    pub level_exec_demo:
        unsafe extern "C" fn(drv: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `StaticLoadClass(base, NULL, name, NULL, LOAD_NoFail, NULL)`.
    /// `base`: 0 = `UNetDriver`, 1 = `AActor`.
    pub static_load_class: unsafe extern "C" fn(base: i32, name: *const u32) -> *mut c_void,
    /// `ConstructObject<UNetDriver>(cls)`.
    pub construct_net_driver: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    /// `StaticConstructObject(cls)` cast to `UNetDriver*`.
    pub static_construct_net_driver: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    /// `drv->InitListen(level, url, error)`.
    pub netdriver_init_listen: unsafe extern "C" fn(
        drv: *mut c_void,
        level: *mut c_void,
        url: *mut c_void,
        error: *mut c_void,
    ) -> i32,
    /// `delete drv`.
    pub netdriver_delete: unsafe extern "C" fn(drv: *mut c_void),
    pub netdriver_server_connection: unsafe extern "C" fn(drv: *mut c_void) -> *mut c_void,
    pub netdriver_server_travel_pause: unsafe extern "C" fn(drv: *mut c_void) -> f32,
    pub netdriver_max_client_rate: unsafe extern "C" fn(drv: *mut c_void) -> i32,
    pub netdriver_master_map: unsafe extern "C" fn(drv: *mut c_void) -> *mut c_void,
    // --- Engine / game engine ---
    /// `CastChecked<UGameEngine>(engine)`.
    pub cast_game_engine: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    pub game_engine_gpending_level: unsafe extern "C" fn(ge: *mut c_void) -> *mut c_void,
    pub game_engine_set_gpending_level: unsafe extern "C" fn(ge: *mut c_void, v: *mut c_void),
    /// `GameEngine->CancelPending()` (virtual).
    pub game_engine_cancel_pending: unsafe extern "C" fn(ge: *mut c_void),
    /// `GameEngine->BuildServerMasterMap(drv, level)` (virtual).
    pub game_build_server_master_map:
        unsafe extern "C" fn(ge: *mut c_void, drv: *mut c_void, level: *mut c_void),
    pub game_engine_server_actors_num: unsafe extern "C" fn(ge: *mut c_void) -> i32,
    /// `*GameEngine->ServerActors(i)`.
    pub game_engine_server_actor_text: unsafe extern "C" fn(ge: *mut c_void, i: i32) -> *const u32,
    /// `new UDemoPlayPendingLevel(ge, url)`.
    pub demo_play_pending_level_new:
        unsafe extern "C" fn(ge: *mut c_void, url: *mut c_void) -> *mut c_void,
    pub pending_level_demo_rec_driver: unsafe extern "C" fn(p: *mut c_void) -> *mut c_void,
    pub pending_level_error_text: unsafe extern "C" fn(p: *mut c_void) -> *const u32,
    /// `delete GameEngine->GPendingLevel` (through the static pointer type).
    pub pending_level_delete: unsafe extern "C" fn(ge: *mut c_void),
    pub engine_client: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    pub engine_client_viewports_num: unsafe extern "C" fn(client: *mut c_void) -> i32,
    pub engine_client_viewport: unsafe extern "C" fn(client: *mut c_void, i: i32) -> *mut c_void,
    /// `Engine->SetClientTravel(player, url, items, travel)` (virtual).
    pub engine_set_client_travel: unsafe extern "C" fn(
        engine: *mut c_void,
        player: *mut c_void,
        url: *const u32,
        items: i32,
        travel: i32,
    ),
    /// `Engine->ChallengeResponse(challenge)` (virtual).
    pub engine_challenge_response: unsafe extern "C" fn(engine: *mut c_void, challenge: u32) -> i32,
    // --- Level ---
    /// `level->SpawnActor(cls)` (virtual).
    pub level_spawn_actor:
        unsafe extern "C" fn(level: *mut c_void, cls: *mut c_void) -> *mut c_void,
    /// `level->URL` (owned by the level; not freed by caller).
    pub level_url: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `level->GetLevelInfo()`.
    pub level_get_level_info: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `level->SpawnPlayActor(conn, role, url, error)` (virtual).
    pub spawn_play_actor: unsafe extern "C" fn(
        level: *mut c_void,
        conn: *mut c_void,
        role: i32,
        url: *mut c_void,
        error: *mut c_void,
    ) -> *mut c_void,
    /// `level->WelcomePlayer(conn)` (virtual; resolves to the C++ shell).
    pub level_welcome_player: unsafe extern "C" fn(level: *mut c_void, conn: *mut c_void),
    // --- LevelInfo / Game ---
    pub level_info_game: unsafe extern "C" fn(li: *mut c_void) -> *mut c_void,
    /// `NextURL` (not owned).
    pub level_info_next_url: unsafe extern "C" fn(li: *mut c_void) -> *mut c_void,
    pub level_info_b_lone_player: unsafe extern "C" fn(li: *mut c_void) -> i32,
    pub level_info_set_next_switch_countdown: unsafe extern "C" fn(li: *mut c_void, v: f32),
    pub level_info_set_net_mode: unsafe extern "C" fn(li: *mut c_void, v: u8),
    pub game_info_b_world_log: unsafe extern "C" fn(game: *mut c_void) -> i32,
    /// `game->eventPreLogin(options, address, error, fail_code)` (virtual).
    pub game_pre_login: unsafe extern "C" fn(
        game: *mut c_void,
        options: *const u32,
        address: *const u32,
        error: *mut c_void,
        fail_code: *mut c_void,
    ),
    // --- Unreal reflection ---
    pub actor_get_class: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `TFieldIterator<UProperty>(cls)`, heap iterator.
    pub tfield_iter_new: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    pub tfield_iter_valid: unsafe extern "C" fn(it: *mut c_void) -> i32,
    pub tfield_iter_next: unsafe extern "C" fn(it: *mut c_void),
    pub tfield_iter_delete: unsafe extern "C" fn(it: *mut c_void),
    /// `appStricmp(It->GetName(), name)`.
    pub uproperty_name_stricmp: unsafe extern "C" fn(it: *mut c_void, name: *const u32) -> i32,
    /// `It->PropertyFlags & CPF_Config` as `!= 0`.
    pub uproperty_is_config: unsafe extern "C" fn(it: *mut c_void) -> i32,
    /// `It->ImportText(text, (BYTE*)obj + It->Offset, 0)`.
    pub uproperty_import_text:
        unsafe extern "C" fn(it: *mut c_void, text: *const u32, obj: *mut c_void),
    // --- Connection ---
    pub connection_driver: unsafe extern "C" fn(conn: *mut c_void) -> *mut c_void,
    pub connection_package_map: unsafe extern "C" fn(conn: *mut c_void) -> *mut c_void,
    pub connection_flush_net: unsafe extern "C" fn(conn: *mut c_void),
    /// `conn->SendPackageMap()`.
    pub connection_send_package_map: unsafe extern "C" fn(conn: *mut c_void),
    /// `conn->Channels[i]->Close()`.
    pub connection_channel_close: unsafe extern "C" fn(conn: *mut c_void, i: i32),
    pub connection_remote_version: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    pub connection_set_remote_version: unsafe extern "C" fn(conn: *mut c_void, v: i32),
    /// `conn->State = USOCK_Closed`.
    pub connection_set_state_closed: unsafe extern "C" fn(conn: *mut c_void),
    pub connection_set_negotiated_ver: unsafe extern "C" fn(conn: *mut c_void, v: i32),
    pub connection_challenge: unsafe extern "C" fn(conn: *mut c_void) -> u32,
    pub connection_set_challenge: unsafe extern "C" fn(conn: *mut c_void, v: u32),
    pub connection_set_user_flags: unsafe extern "C" fn(conn: *mut c_void, v: i32),
    pub connection_set_current_net_speed: unsafe extern "C" fn(conn: *mut c_void, v: i32),
    pub connection_current_net_speed: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    pub connection_set_request_url: unsafe extern "C" fn(conn: *mut c_void, text: *const u32),
    /// `RequestURL` text (not owned).
    pub connection_request_url_text: unsafe extern "C" fn(conn: *mut c_void) -> *const u32,
    pub connection_actor: unsafe extern "C" fn(conn: *mut c_void) -> *mut c_void,
    /// `Connection->Actor->PlayerReplicationInfo->PlayerName` text.
    pub connection_actor_player_name: unsafe extern "C" fn(conn: *mut c_void) -> *const u32,
    /// `LowLevelGetRemoteAddress()`, owned FString handle.
    pub connection_lowlevel_remote_address: unsafe extern "C" fn(conn: *mut c_void) -> *mut c_void,
    // --- PackageMap ---
    /// `pm->Copy(master)`.
    pub package_map_copy: unsafe extern "C" fn(pm: *mut c_void, master: *mut c_void),
    /// `pm->Compute()`.
    pub package_map_compute: unsafe extern "C" fn(pm: *mut c_void),
    pub package_map_list_num: unsafe extern "C" fn(pm: *mut c_void) -> i32,
    pub package_info_guid_eq:
        unsafe extern "C" fn(pm: *mut c_void, i: i32, guid: *const u32) -> i32,
    /// `&pm->List(i).RemoteGeneration`.
    pub package_info_remote_generation: unsafe extern "C" fn(pm: *mut c_void, i: i32) -> *mut i32,
    pub package_info_url_text: unsafe extern "C" fn(pm: *mut c_void, i: i32) -> *const u32,
    /// `pm->List.Remove(i)`.
    pub package_map_list_remove: unsafe extern "C" fn(pm: *mut c_void, i: i32),
    // --- Channel ---
    pub channel_connection: unsafe extern "C" fn(ch: *mut c_void) -> *mut c_void,
    pub channel_ch_type: unsafe extern "C" fn(ch: *mut c_void) -> i32,
    pub channel_ch_index: unsafe extern "C" fn(ch: *mut c_void) -> i32,
}

// ---------------------------------------------------------------------------
// TCHAR helpers
// ---------------------------------------------------------------------------

/// Text part for `fmt`.
enum A {
    L(&'static str),
    P(*const u32),
    I(i32),
}

/// Reads a NUL-terminated TCHAR string (without the NUL). Null is empty.
///
/// # Safety
/// `p`, when non-null, must point to a NUL-terminated TCHAR string.
unsafe fn read(p: *const u32) -> Vec<u32> {
    let mut v = Vec::new();
    if p.is_null() {
        return v;
    }
    let mut i = 0usize;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            break;
        }
        v.push(c);
        i += 1;
    }
    v
}

/// Formats parts into a NUL-terminated TCHAR buffer.
///
/// # Safety
/// Every `A::P` must be a valid NUL-terminated TCHAR string or null.
unsafe fn fmt(parts: &[A]) -> Vec<u32> {
    let mut v = Vec::new();
    for a in parts {
        match a {
            A::L(s) => v.extend(s.chars().map(|c| c as u32)),
            A::P(p) => v.extend(unsafe { read(*p) }),
            A::I(i) => v.extend(i.to_string().chars().map(|c| c as u32)),
        }
    }
    v.push(0);
    v
}

/// NUL-terminated TCHAR buffer from a literal.
fn lit(s: &str) -> Vec<u32> {
    let mut v: Vec<u32> = s.chars().map(|c| c as u32).collect();
    v.push(0);
    v
}

/// Owned FString handle; freed on drop.
struct Fs<'a> {
    s: &'a Hp2LevelNetServices,
    p: *mut c_void,
}

impl<'a> Fs<'a> {
    fn new(s: &'a Hp2LevelNetServices) -> Self {
        Self {
            s,
            p: unsafe { (s.fstring_new)() },
        }
    }
    /// Takes ownership of an already-created handle.
    fn own(s: &'a Hp2LevelNetServices, p: *mut c_void) -> Self {
        Self { s, p }
    }
}

impl Drop for Fs<'_> {
    fn drop(&mut self) {
        unsafe { (self.s.fstring_delete)(self.p) }
    }
}

/// Owned FURL handle; freed on drop.
struct Furl<'a> {
    s: &'a Hp2LevelNetServices,
    p: *mut c_void,
}

impl<'a> Furl<'a> {
    fn new(s: &'a Hp2LevelNetServices) -> Self {
        Self {
            s,
            p: unsafe { (s.furl_new)() },
        }
    }
    fn new_travel(s: &'a Hp2LevelNetServices, text: *const u32, travel: i32) -> Self {
        Self {
            s,
            p: unsafe { (s.furl_new_travel)(text, travel) },
        }
    }
}

impl Drop for Furl<'_> {
    fn drop(&mut self) {
        unsafe { (self.s.furl_delete)(self.p) }
    }
}

/// `check(expr)` from UnFile.h. A captured engine exception propagates as `Err`.
fn check(s: &Hp2LevelNetServices, expr: &CStr, line: i32, failed: bool) -> bool {
    failed && unsafe { (s.check_failed)(expr.as_ptr(), FILE_NAME.as_ptr(), line) != 0 }
}

fn clamp_c(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

// ---------------------------------------------------------------------------
// ULevel::Exec
// ---------------------------------------------------------------------------

/// `ULevel::Exec(Cmd, Ar)`.
///
/// # Safety
/// `level` must point to a live `ULevel`; `cmd` a TCHAR string; `ar` a live
/// `FOutputDevice`.
pub unsafe fn exec(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
    cmd0: *const u32,
    ar: *mut c_void,
) -> i32 {
    let lv = level.cast::<ULevel>();
    let mut cmd = cmd0;
    unsafe {
        let nd = (*lv).net_driver;
        if !nd.is_null() && (s.level_exec_net)(nd, cmd, ar) != 0 {
            return 1;
        }
        let dr = (*lv).demo_rec_driver;
        if !dr.is_null() && (s.level_exec_demo)(dr, cmd, ar) != 0 {
            return 1;
        }

        let kw = lit("DEMOREC");
        if (s.parse_command)(&mut cmd, kw.as_ptr()) != 0 {
            let url = Furl::new(s);
            if (s.parse_token_fstring)(&mut cmd, (s.furl_map)(url.p), 0) != 0 {
                let map = (s.furl_map)(url.p);
                let dem = lit(".dem");
                {
                    let right = Fs::own(s, (s.fstring_right)(map, 4));
                    if (s.fstring_ne_text)(right.p, dem.as_ptr()) != 0 {
                        (s.fstring_append_text)(map, dem.as_ptr());
                    }
                }
                (s.debugf_log)(
                    fmt(&[
                        A::L("Attempting to record demo "),
                        A::P((s.fstring_text)(map)),
                    ])
                    .as_ptr(),
                );

                let cls =
                    (s.static_load_class)(0, lit("ini:Engine.Engine.DemoRecordingDevice").as_ptr());
                let drv = (s.construct_net_driver)(cls);
                (*lv).demo_rec_driver = drv;
                let error = Fs::new(s);
                if (s.netdriver_init_listen)(drv, level, url.p, error.p) == 0 {
                    (s.output_device_logf)(
                        ar,
                        fmt(&[
                            A::L("Demo recording failed: "),
                            A::P((s.fstring_text)(error.p)),
                        ])
                        .as_ptr(),
                    );
                    (s.netdriver_delete)(drv);
                    (*lv).demo_rec_driver = std::ptr::null_mut();
                } else {
                    (s.output_device_logf)(
                        ar,
                        fmt(&[
                            A::L("Demo recording started to "),
                            A::P((s.fstring_text)(map)),
                        ])
                        .as_ptr(),
                    );
                }
            } else {
                (s.output_device_log)(ar, lit("You must specify a filename").as_ptr());
            }
            return 1;
        } else if (s.parse_command)(&mut cmd, lit("DEMOPLAY").as_ptr()) != 0 {
            let temp = Fs::new(s);
            if (s.parse_token_fstring)(&mut cmd, temp.p, 0) != 0 {
                let i = (s.fstring_caps_in_str)(temp.p, lit(".DEM").as_ptr());
                if i != -1 {
                    let left = Fs::own(s, (s.fstring_left)(temp.p, i));
                    let mid = Fs::own(s, (s.fstring_mid)(temp.p, i + 4));
                    let sum = Fs::own(s, (s.fstring_add)(left.p, mid.p));
                    (s.fstring_assign)(temp.p, sum.p);
                }
                let url = Furl::new_travel(s, (s.fstring_text)(temp.p), TRAVEL_ABSOLUTE);
                let dem = lit(".dem");
                (s.fstring_append_text)((s.furl_map)(url.p), dem.as_ptr());
                (s.debugf_log)(
                    fmt(&[
                        A::L("Attempting to play demo "),
                        A::P((s.fstring_text)((s.furl_map)(url.p))),
                    ])
                    .as_ptr(),
                );
                let ge = (s.cast_game_engine)((*lv).engine);
                if !(s.game_engine_gpending_level)(ge).is_null() {
                    (s.game_engine_cancel_pending)(ge);
                }
                let pl = (s.demo_play_pending_level_new)(ge, url.p);
                (s.game_engine_set_gpending_level)(ge, pl);
                if (s.pending_level_demo_rec_driver)((s.game_engine_gpending_level)(ge)).is_null() {
                    let p = (s.game_engine_gpending_level)(ge);
                    (s.output_device_logf)(
                        ar,
                        fmt(&[
                            A::L("Demo playback failed: "),
                            A::P((s.pending_level_error_text)(p)),
                        ])
                        .as_ptr(),
                    );
                    (s.pending_level_delete)(ge);
                    (s.game_engine_set_gpending_level)(ge, std::ptr::null_mut());
                }
            } else {
                (s.output_device_log)(ar, lit("You must specify a filename").as_ptr());
            }
            return 1;
        }
        0
    }
}

// ---------------------------------------------------------------------------
// ULevel::Listen / IsServer
// ---------------------------------------------------------------------------

/// `ULevel::Listen(FString& Error)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `error` a live `FString` handle.
pub unsafe fn listen(s: &Hp2LevelNetServices, level: *mut c_void, error: *mut c_void) -> i32 {
    let lv = level.cast::<ULevel>();
    unsafe {
        if !(*lv).net_driver.is_null() {
            (s.localize_error_assign)(level, error, c"NetAlready".as_ptr());
            return 0;
        }
        if (s.level_get_linker)(level).is_null() {
            (s.localize_error_assign)(level, error, c"NetListen".as_ptr());
            return 0;
        }

        let cls = (s.static_load_class)(0, lit("ini:Engine.Engine.NetworkDevice").as_ptr());
        let nd = (s.static_construct_net_driver)(cls);
        (*lv).net_driver = nd;
        if (s.netdriver_init_listen)(nd, level, (s.level_url)(level), error) == 0 {
            (s.debugf_log)(
                fmt(&[A::L("Failed to listen: "), A::P((s.fstring_text)(error))]).as_ptr(),
            );
            (s.netdriver_delete)(nd);
            (*lv).net_driver = std::ptr::null_mut();
            return 0;
        }

        let ge = (s.cast_game_engine)((*lv).engine);
        (s.game_build_server_master_map)(ge, nd, level);

        let mut i = 0i32;
        while i < (s.game_engine_server_actors_num)(ge) {
            let mut ptr = (s.game_engine_server_actor_text)(ge, i);
            let mut buf = [0u32; 240];
            if (s.parse_token_buf)(&mut ptr, buf.as_mut_ptr(), 240, 1) != 0 {
                (s.debugf_log)(fmt(&[A::L("Spawning: "), A::P(buf.as_ptr())]).as_ptr());
                let hc = (s.static_load_class)(1, buf.as_ptr());
                let actor = (s.level_spawn_actor)(level, hc);
                while !actor.is_null()
                    && (s.parse_token_buf)(&mut ptr, buf.as_mut_ptr(), 240, 1) != 0
                {
                    let eq = (s.app_strchr)(buf.as_ptr(), '=' as u32);
                    if !eq.is_null() {
                        let idx = eq.offset_from(buf.as_ptr()) as usize;
                        buf[idx] = 0;
                        let value = buf.as_ptr().add(idx + 1);
                        let it = (s.tfield_iter_new)((s.actor_get_class)(actor));
                        while (s.tfield_iter_valid)(it) != 0 {
                            if (s.uproperty_name_stricmp)(it, buf.as_ptr()) == 0
                                && (s.uproperty_is_config)(it) != 0
                            {
                                (s.uproperty_import_text)(it, value, actor);
                            }
                            (s.tfield_iter_next)(it);
                        }
                        (s.tfield_iter_delete)(it);
                    }
                }
            }
            i += 1;
        }

        let nm = if (s.engine_client)((*lv).engine).is_null() {
            NM_DEDICATED_SERVER
        } else {
            NM_LISTEN_SERVER
        };
        (s.level_info_set_net_mode)((s.level_get_level_info)(level), nm);
        let pause = (s.netdriver_server_travel_pause)((*lv).net_driver);
        (s.level_info_set_next_switch_countdown)((s.level_get_level_info)(level), pause);
        1
    }
}

/// `ULevel::IsServer()`.
///
/// # Safety
/// `level` must be a live `ULevel`.
pub unsafe fn is_server(s: &Hp2LevelNetServices, level: *mut c_void) -> i32 {
    let lv = level.cast::<ULevel>();
    unsafe {
        let nd = (*lv).net_driver;
        let a = nd.is_null() || (s.netdriver_server_connection)(nd).is_null();
        if !a {
            return 0;
        }
        let dr = (*lv).demo_rec_driver;
        (dr.is_null() || (s.netdriver_server_connection)(dr).is_null()) as i32
    }
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

/// `ULevel::NotifyAcceptingConnection()`.
///
/// # Safety
/// `level` must be a live `ULevel`.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn notify_accepting_connection(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
) -> Result<i32, ()> {
    let lv = level.cast::<ULevel>();
    unsafe {
        let nd = (*lv).net_driver;
        if check(s, c"NetDriver", 1328, nd.is_null()) {
            return Err(());
        }
        if !(s.netdriver_server_connection)(nd).is_null() {
            (s.debugf_dev_net)(lit("NotifyAcceptingConnection: Client refused").as_ptr());
            return Ok(ACCEPTC_REJECT);
        }
        let next = (s.level_info_next_url)((s.level_get_level_info)(level));
        if (s.fstring_ne_text)(next, lit("").as_ptr()) != 0 {
            (s.debugf_dev_net)(
                fmt(&[
                    A::L("NotifyAcceptingConnection: Server "),
                    A::P((s.uobject_get_name)(level)),
                    A::L(" refused"),
                ])
                .as_ptr(),
            );
            return Ok(ACCEPTC_IGNORE);
        }
        (s.debugf_dev_net)(
            fmt(&[
                A::L("NotifyAcceptingConnection: Server "),
                A::P((s.uobject_get_name)(level)),
                A::L(" accept"),
            ])
            .as_ptr(),
        );
        Ok(ACCEPTC_ACCEPT)
    }
}

/// `ULevel::NotifyAcceptedConnection(Connection)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `conn` a live `UNetConnection`.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn notify_accepted_connection(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
) -> Result<(), ()> {
    let lv = level.cast::<ULevel>();
    unsafe {
        let nd = (*lv).net_driver;
        if check(s, c"NetDriver!=NULL", 1356, nd.is_null()) {
            return Err(());
        }
        if check(
            s,
            c"NetDriver->ServerConnection==NULL",
            1357,
            !(s.netdriver_server_connection)(nd).is_null(),
        ) {
            return Err(());
        }
        let name = (s.uobject_get_name)(level);
        let ts = (s.app_timestamp)();
        let addr = Fs::own(s, (s.connection_lowlevel_remote_address)(conn));
        (s.debugf_net_come_go)(
            fmt(&[
                A::L("Open "),
                A::P(name),
                A::L(" "),
                A::P(ts),
                A::L(" "),
                A::P((s.fstring_text)(addr.p)),
            ])
            .as_ptr(),
        );
        Ok(())
    }
}

/// `ULevel::NotifyAcceptingChannel(Channel)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `ch` a live `UChannel`.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn notify_accepting_channel(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
    ch: *mut c_void,
) -> Result<i32, ()> {
    unsafe {
        if check(s, c"Channel", 1370, ch.is_null()) {
            return Err(());
        }
        let conn = (s.channel_connection)(ch);
        if check(s, c"Channel->Connection", 1371, conn.is_null()) {
            return Err(());
        }
        if check(
            s,
            c"Channel->Connection->Driver",
            1372,
            (s.connection_driver)(conn).is_null(),
        ) {
            return Err(());
        }
        let driver = (s.connection_driver)(conn);

        if !(s.netdriver_server_connection)(driver).is_null() {
            if (s.channel_ch_type)(ch) == CHTYPE_ACTOR {
                return Ok(1);
            }
            (s.debugf_dev_net)(
                fmt(&[
                    A::L("Client refusing unwanted channel of type "),
                    A::I((s.channel_ch_type)(ch)),
                ])
                .as_ptr(),
            );
            return Ok(0);
        }

        let result = if (s.channel_ch_index)(ch) == 0 && (s.channel_ch_type)(ch) == CHTYPE_CONTROL {
            let full = Fs::own(s, (s.uobject_get_full_name)(level));
            (s.debugf_dev_net)(
                fmt(&[
                    A::L("NotifyAcceptingChannel Control "),
                    A::I((s.channel_ch_index)(ch)),
                    A::L(" server "),
                    A::P((s.fstring_text)(full.p)),
                    A::L(": Accepted"),
                ])
                .as_ptr(),
            );
            1
        } else if (s.channel_ch_type)(ch) == CHTYPE_FILE {
            let full = Fs::own(s, (s.uobject_get_full_name)(level));
            (s.debugf_dev_net)(
                fmt(&[
                    A::L("NotifyAcceptingChannel File "),
                    A::I((s.channel_ch_index)(ch)),
                    A::L(" server "),
                    A::P((s.fstring_text)(full.p)),
                    A::L(": Accepted"),
                ])
                .as_ptr(),
            );
            1
        } else {
            let full = Fs::own(s, (s.uobject_get_full_name)(level));
            (s.debugf_dev_net)(
                fmt(&[
                    A::L("NotifyAcceptingChannel "),
                    A::I((s.channel_ch_type)(ch)),
                    A::L(" "),
                    A::I((s.channel_ch_index)(ch)),
                    A::L(" server "),
                    A::P((s.fstring_text)(full.p)),
                    A::L(": Refused"),
                ])
                .as_ptr(),
            );
            0
        };
        Ok(result)
    }
}

/// `ULevel::WelcomePlayer(Connection, Optional)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `conn` a live `UNetConnection`;
/// `optional` a TCHAR string.
pub unsafe fn welcome_player(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
    optional: *const u32,
) {
    unsafe {
        let drv = (s.connection_driver)(conn);
        let pm = (s.connection_package_map)(conn);
        (s.package_map_copy)(pm, (s.netdriver_master_map)(drv));
        (s.connection_send_package_map)(conn);
        if *optional != 0 {
            let outer = (s.uobject_get_outer)(level);
            (s.connection_log)(
                conn,
                fmt(&[
                    A::L("WELCOME LEVEL="),
                    A::P((s.uobject_get_name)(outer)),
                    A::L(" LONE="),
                    A::I((s.level_info_b_lone_player)((s.level_get_level_info)(
                        level,
                    ))),
                    A::L(" "),
                    A::P(optional),
                ])
                .as_ptr(),
            );
        } else {
            let outer = (s.uobject_get_outer)(level);
            (s.connection_log)(
                conn,
                fmt(&[
                    A::L("WELCOME LEVEL="),
                    A::P((s.uobject_get_name)(outer)),
                    A::L(" LONE="),
                    A::I((s.level_info_b_lone_player)((s.level_get_level_info)(
                        level,
                    ))),
                ])
                .as_ptr(),
            );
        }
        (s.connection_flush_net)(conn);
    }
}

/// `ULevel::NotifyReceivedText(Connection, Text)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `conn` a live `UNetConnection`; `text`
/// a TCHAR string.
pub unsafe fn notify_received_text(
    s: &Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
    text0: *const u32,
) {
    let lv = level.cast::<ULevel>();
    let mut text = text0;
    unsafe {
        if (s.parse_command)(&mut text, lit("USERFLAG").as_ptr()) != 0 {
            (s.connection_set_user_flags)(conn, (s.app_atoi)(text));
        } else if !(*lv).net_driver.is_null()
            && !(s.netdriver_server_connection)((*lv).net_driver).is_null()
        {
            // We are the client.
            (s.debugf_dev_net)(fmt(&[A::L("Level client received: "), A::P(text)]).as_ptr());
            if (s.parse_command)(&mut text, lit("FAILURE").as_ptr()) != 0 {
                let eng = (*lv).engine;
                let client = (s.engine_client)(eng);
                check(
                    s,
                    c"Engine->Client->Viewports.Num()",
                    1452,
                    (s.engine_client_viewports_num)(client) == 0,
                );
                let vp = (s.engine_client_viewport)(client, 0);
                (s.engine_set_client_travel)(eng, vp, lit("?failed").as_ptr(), 0, TRAVEL_ABSOLUTE);
            }
        } else {
            // We are the server.
            (s.debugf_dev_net)(fmt(&[A::L("Level server received: "), A::P(text)]).as_ptr());
            if (s.parse_command)(&mut text, lit("HELLO").as_ptr()) != 0 {
                let mut remote_min = 219i32;
                let mut remote_ver = 219i32;
                (s.parse_int)(text, lit("MINVER=").as_ptr(), &mut remote_min);
                (s.parse_int)(text, lit("VER=").as_ptr(), &mut remote_ver);
                (s.connection_set_remote_version)(conn, remote_ver);
                if remote_ver < ENGINE_MIN_NET_VERSION || remote_min > ENGINE_VERSION {
                    (s.connection_log)(
                        conn,
                        fmt(&[
                            A::L("UPGRADE MINVER="),
                            A::I(ENGINE_MIN_NET_VERSION),
                            A::L(" VER="),
                            A::I(ENGINE_VERSION),
                        ])
                        .as_ptr(),
                    );
                    (s.connection_channel_close)(conn, 0);
                    (s.connection_flush_net)(conn);
                    if (s.connection_remote_version)(conn) <= 420 {
                        (s.connection_set_state_closed)(conn);
                    }
                    return;
                }
                let negotiated = remote_ver.min(ENGINE_VERSION);
                (s.connection_set_negotiated_ver)(conn, negotiated);

                let stats =
                    (s.game_info_b_world_log)((s.level_info_game)((s.level_get_level_info)(level)));
                let challenge = (s.app_cycles)();
                (s.connection_set_challenge)(conn, challenge);
                (s.connection_log)(
                    conn,
                    fmt(&[
                        A::L("CHALLENGE VER="),
                        A::I(negotiated),
                        A::L(" CHALLENGE="),
                        A::I(challenge as i32),
                        A::L(" STATS="),
                        A::I(stats),
                    ])
                    .as_ptr(),
                );
                (s.connection_flush_net)(conn);
            } else if (s.parse_command)(&mut text, lit("NETSPEED").as_ptr()) != 0 {
                let rate = (s.app_atoi)(text);
                if rate >= 2000 {
                    let max = (s.netdriver_max_client_rate)((*lv).net_driver);
                    (s.connection_set_current_net_speed)(conn, clamp_c(rate, 2000, max));
                }
                (s.debugf_log)(
                    fmt(&[
                        A::L("Client netspeed is "),
                        A::I((s.connection_current_net_speed)(conn)),
                    ])
                    .as_ptr(),
                );
            } else if (s.parse_command)(&mut text, lit("HAVE").as_ptr()) != 0 {
                let mut guid = [0u32; 4];
                (s.parse_guid)(text, lit("GUID=").as_ptr(), guid.as_mut_ptr());
                let pm = (s.connection_package_map)(conn);
                let mut i = 0i32;
                while i < (s.package_map_list_num)(pm) {
                    if (s.package_info_guid_eq)(pm, i, guid.as_ptr()) != 0 {
                        (s.parse_int)(
                            text,
                            lit("GEN=").as_ptr(),
                            (s.package_info_remote_generation)(pm, i),
                        );
                    }
                    i += 1;
                }
            } else if (s.parse_command)(&mut text, lit("SKIP").as_ptr()) != 0 {
                let mut guid = [0u32; 4];
                (s.parse_guid)(text, lit("GUID=").as_ptr(), guid.as_mut_ptr());
                let pm = (s.connection_package_map)(conn);
                if !pm.is_null() {
                    let mut i = 0i32;
                    while i < (s.package_map_list_num)(pm) {
                        if (s.package_info_guid_eq)(pm, i, guid.as_ptr()) != 0 {
                            (s.debugf_log)(
                                fmt(&[
                                    A::L("User skipped download of '"),
                                    A::P((s.package_info_url_text)(pm, i)),
                                    A::L("'"),
                                ])
                                .as_ptr(),
                            );
                            (s.package_map_list_remove)(pm, i);
                            break;
                        }
                        i += 1;
                    }
                }
            } else if (s.parse_command)(&mut text, lit("LOGIN").as_ptr()) != 0 {
                // Admit or deny the player here.
                let mut response = 0i32;
                let parsed = (s.parse_int)(text, lit("RESPONSE=").as_ptr(), &mut response);
                let rejected = parsed == 0 || {
                    // The RHS is conditional: the oracle never calls ChallengeResponse
                    // when parsing RESPONSE failed.
                    let cr =
                        (s.engine_challenge_response)((*lv).engine, (s.connection_challenge)(conn));
                    ((cr == 0) as i32) == response
                };
                if rejected {
                    (s.connection_log)(conn, lit("FAILURE CHALLENGE").as_ptr());
                    (s.connection_channel_close)(conn, 0);
                    (s.connection_flush_net)(conn);
                    if (s.connection_remote_version)(conn) <= 420 {
                        (s.connection_set_state_closed)(conn);
                    }
                    return;
                }
                let mut str_buf = [0u32; 1024];
                let error = Fs::new(s);
                let fail_code = Fs::new(s);
                (s.parse_text)(text, lit("URL=").as_ptr(), str_buf.as_mut_ptr(), 1024);
                (s.connection_set_request_url)(conn, str_buf.as_ptr());
                (s.debugf_dev_net)(
                    fmt(&[
                        A::L("Login request: "),
                        A::P((s.connection_request_url_text)(conn)),
                    ])
                    .as_ptr(),
                );
                let mut tmp = 0usize;
                while str_buf[tmp] != 0 && str_buf[tmp] != '?' as u32 {
                    tmp += 1;
                }
                let game = (s.level_info_game)((s.level_get_level_info)(level));
                let addr = Fs::own(s, (s.connection_lowlevel_remote_address)(conn));
                (s.game_pre_login)(
                    game,
                    str_buf.as_ptr().add(tmp),
                    (s.fstring_text)(addr.p),
                    error.p,
                    fail_code.p,
                );
                if (s.fstring_ne_text)(error.p, lit("").as_ptr()) != 0 {
                    (s.debugf_dev_net)(
                        fmt(&[
                            A::L("PreLogin failure: "),
                            A::P((s.fstring_text)(error.p)),
                            A::L(" ("),
                            A::P((s.fstring_text)(fail_code.p)),
                            A::L(")"),
                        ])
                        .as_ptr(),
                    );
                    (s.connection_log)(
                        conn,
                        fmt(&[A::L("FAILURE "), A::P((s.fstring_text)(error.p))]).as_ptr(),
                    );
                    if *(s.fstring_text)(fail_code.p) != 0 {
                        (s.connection_log)(
                            conn,
                            fmt(&[A::L("FAILCODE "), A::P((s.fstring_text)(fail_code.p))]).as_ptr(),
                        );
                    }
                    (s.connection_channel_close)(conn, 0);
                    (s.connection_flush_net)(conn);
                    if (s.connection_remote_version)(conn) <= 420 {
                        (s.connection_set_state_closed)(conn);
                    }
                    return;
                }
                (s.level_welcome_player)(level, conn);
            } else if (s.parse_command)(&mut text, lit("JOIN").as_ptr()) != 0
                && (s.connection_actor)(conn).is_null()
            {
                (s.package_map_compute)((s.connection_package_map)(conn));
                let error = Fs::new(s);
                (s.debugf_dev_net)(
                    fmt(&[
                        A::L("Join request: "),
                        A::P((s.connection_request_url_text)(conn)),
                    ])
                    .as_ptr(),
                );
                let url =
                    Furl::new_travel(s, (s.connection_request_url_text)(conn), TRAVEL_ABSOLUTE);
                if (s.spawn_play_actor)(level, conn, ROLE_AUTONOMOUS_PROXY, url.p, error.p)
                    .is_null()
                {
                    (s.debugf_dev_net)(
                        fmt(&[A::L("Join failure: "), A::P((s.fstring_text)(error.p))]).as_ptr(),
                    );
                    (s.connection_log)(
                        conn,
                        fmt(&[A::L("FAILURE "), A::P((s.fstring_text)(error.p))]).as_ptr(),
                    );
                    (s.connection_channel_close)(conn, 0);
                    (s.connection_flush_net)(conn);
                    if (s.connection_remote_version)(conn) <= 420 {
                        (s.connection_set_state_closed)(conn);
                    }
                } else {
                    (s.debugf_dev_net)(
                        fmt(&[
                            A::L("Join succeeded: "),
                            A::P((s.connection_actor_player_name)(conn)),
                        ])
                        .as_ptr(),
                    );
                }
            }
        }
    }
}

/// `ULevel::NotifySendingFile(Connection, Guid)`.
///
/// # Safety
/// `level` must be a live `ULevel`.
pub unsafe fn notify_sending_file(s: &Hp2LevelNetServices, level: *mut c_void) -> i32 {
    let lv = level.cast::<ULevel>();
    unsafe {
        if !(s.netdriver_server_connection)((*lv).net_driver).is_null() {
            (s.debugf_dev_net)(lit("Server requested file: Refused").as_ptr());
            0
        } else {
            (s.debugf_dev_net)(lit("Client requested file: Allowed").as_ptr());
            1
        }
    }
}

/// `ULevel::NotifyReceivedFile(Connection, PackageIndex, Error, Skipped)`.
///
/// The body is unconditional `appErrorf`; the arguments are unused, as in the original.
///
/// # Safety
/// `s` must be a fully populated services table.
#[allow(clippy::result_unit_err)] // Err(()) = C++ exception pending; the unit error carries no payload.
pub unsafe fn notify_received_file(s: &Hp2LevelNetServices) -> Result<(), ()> {
    unsafe {
        if (s.app_errorf)(lit("Level received unexpected file").as_ptr()) != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
}
