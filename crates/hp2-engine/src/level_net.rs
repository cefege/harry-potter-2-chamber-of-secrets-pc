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
//! All native leaves, including fatal diagnostics, return checked status.

use std::ffi::{CStr, c_void};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AGameInfo, ALevelInfo, FName, FRotator, FURL, UChannel, UGameEngine, ULevel,
    UNetConnection, UNetDriver, UObject,
};

use hp2_core::native_text::{TextPart as A, fmt, scoped};
use hp2_core::lit;

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

/// Every leaf returns zero on success; values are written only on success.
/// Native callbacks capture exceptions before returning across a Rust frame.
#[repr(C)]
pub struct Hp2LevelNetServices {
    pub debugf_log: unsafe extern "C" fn(text: *const u32) -> i32,
    pub debugf_dev_net: unsafe extern "C" fn(text: *const u32) -> i32,
    pub debugf_net_come_go: unsafe extern "C" fn(text: *const u32) -> i32,
    pub output_device_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    pub output_device_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    pub connection_log: unsafe extern "C" fn(conn: *mut c_void, text: *const u32) -> i32,
    pub check_failed: unsafe extern "C" fn(expr: *const i8, file: *const i8, line: i32) -> i32,
    pub app_errorf: unsafe extern "C" fn(text: *const u32) -> i32,
    pub localize_error_assign:
        unsafe extern "C" fn(level: *mut c_void, error: *mut c_void, key: *const i8) -> i32,
    pub uobject_get_name: unsafe extern "C" fn(obj: *mut c_void, result: *mut *const u32) -> i32,
    pub uobject_get_full_name:
        unsafe extern "C" fn(obj: *mut c_void, result: *mut *const u32) -> i32,
    pub app_timestamp: unsafe extern "C" fn(result: *mut *const u32) -> i32,
    pub app_cycles: unsafe extern "C" fn(result: *mut u32) -> i32,
    pub app_atoi: unsafe extern "C" fn(text: *const u32, result: *mut i32) -> i32,
    pub app_strchr: unsafe extern "C" fn(text: *const u32, ch: u32, result: *mut *const u32) -> i32,
    pub fstring_text: unsafe extern "C" fn(h: *mut c_void, result: *mut *const u32) -> i32,
    pub fstring_append_text: unsafe extern "C" fn(h: *mut c_void, text: *const u32) -> i32,
    pub fstring_ne_text:
        unsafe extern "C" fn(h: *mut c_void, text: *const u32, result: *mut i32) -> i32,
    pub fstring_caps_in_str:
        unsafe extern "C" fn(h: *mut c_void, needle: *const u32, result: *mut i32) -> i32,
    pub parse_command:
        unsafe extern "C" fn(stream: *mut *const u32, keyword: *const u32, result: *mut i32) -> i32,
    pub parse_token_fstring: unsafe extern "C" fn(
        stream: *mut *const u32,
        token: *mut c_void,
        escape: i32,
        result: *mut i32,
    ) -> i32,
    pub parse_token_buf: unsafe extern "C" fn(
        stream: *mut *const u32,
        buf: *mut u32,
        max: i32,
        escape: i32,
        result: *mut i32,
    ) -> i32,
    pub parse_int: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        out: *mut i32,
        result: *mut i32,
    ) -> i32,
    pub parse_text: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        buf: *mut u32,
        max: i32,
        result: *mut i32,
    ) -> i32,
    pub parse_guid: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        out: *mut u32,
        result: *mut i32,
    ) -> i32,
    pub level_exec_net: unsafe extern "C" fn(
        drv: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        result: *mut i32,
    ) -> i32,
    pub level_exec_demo: unsafe extern "C" fn(
        drv: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        result: *mut i32,
    ) -> i32,
    pub static_load_class:
        unsafe extern "C" fn(base: i32, name: *const u32, result: *mut *mut c_void) -> i32,
    pub construct_net_driver:
        unsafe extern "C" fn(cls: *mut c_void, result: *mut *mut c_void) -> i32,
    pub static_construct_net_driver:
        unsafe extern "C" fn(cls: *mut c_void, result: *mut *mut c_void) -> i32,
    pub netdriver_init_listen: unsafe extern "C" fn(
        drv: *mut c_void,
        level: *mut c_void,
        url: *mut c_void,
        error: *mut c_void,
        result: *mut i32,
    ) -> i32,
    pub netdriver_delete: unsafe extern "C" fn(drv: *mut c_void) -> i32,
    pub cast_game_engine:
        unsafe extern "C" fn(engine: *mut c_void, result: *mut *mut c_void) -> i32,
    pub game_engine_cancel_pending: unsafe extern "C" fn(ge: *mut c_void) -> i32,
    pub game_build_server_master_map:
        unsafe extern "C" fn(ge: *mut c_void, drv: *mut c_void, level: *mut c_void) -> i32,
    pub game_engine_server_actors_num:
        unsafe extern "C" fn(ge: *mut c_void, result: *mut i32) -> i32,
    pub game_engine_server_actor_text:
        unsafe extern "C" fn(ge: *mut c_void, i: i32, result: *mut *const u32) -> i32,
    pub demo_play_pending_level_new:
        unsafe extern "C" fn(ge: *mut c_void, url: *mut c_void, result: *mut *mut c_void) -> i32,
    pub pending_level_error_text:
        unsafe extern "C" fn(p: *mut c_void, result: *mut *const u32) -> i32,
    pub pending_level_delete: unsafe extern "C" fn(ge: *mut c_void) -> i32,
    pub engine_client_viewports_num:
        unsafe extern "C" fn(client: *mut c_void, result: *mut i32) -> i32,
    pub engine_client_viewport:
        unsafe extern "C" fn(client: *mut c_void, i: i32, result: *mut *mut c_void) -> i32,
    pub engine_set_client_travel: unsafe extern "C" fn(
        engine: *mut c_void,
        player: *mut c_void,
        url: *const u32,
        items: i32,
        travel: i32,
    ) -> i32,
    pub engine_challenge_response:
        unsafe extern "C" fn(engine: *mut c_void, challenge: u32, result: *mut i32) -> i32,
    pub level_get_level_info:
        unsafe extern "C" fn(level: *mut c_void, result: *mut *mut c_void) -> i32,
    pub spawn_play_actor: unsafe extern "C" fn(
        level: *mut c_void,
        conn: *mut c_void,
        role: i32,
        url: *mut c_void,
        error: *mut c_void,
        result: *mut *mut c_void,
    ) -> i32,
    pub level_welcome_player: unsafe extern "C" fn(level: *mut c_void, conn: *mut c_void) -> i32,
    pub game_pre_login: unsafe extern "C" fn(
        game: *mut c_void,
        conn: *mut c_void,
        options: *const u32,
        error: *mut c_void,
        fail_code: *mut c_void,
    ) -> i32,
    pub tfield_iter_valid: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub tfield_iter_next: unsafe extern "C" fn(it: *mut c_void) -> i32,
    pub uproperty_name_stricmp:
        unsafe extern "C" fn(it: *mut c_void, name: *const u32, result: *mut i32) -> i32,
    pub uproperty_is_config: unsafe extern "C" fn(it: *mut c_void, result: *mut i32) -> i32,
    pub uproperty_import_text:
        unsafe extern "C" fn(it: *mut c_void, text: *const u32, obj: *mut c_void) -> i32,
    pub connection_flush_net: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    pub connection_send_package_map: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    pub connection_channel_close: unsafe extern "C" fn(conn: *mut c_void, i: i32) -> i32,
    pub connection_set_request_url:
        unsafe extern "C" fn(conn: *mut c_void, text: *const u32) -> i32,
    pub connection_request_url_text:
        unsafe extern "C" fn(conn: *mut c_void, result: *mut *const u32) -> i32,
    pub connection_actor_player_name:
        unsafe extern "C" fn(conn: *mut c_void, result: *mut *const u32) -> i32,
    pub package_map_copy: unsafe extern "C" fn(pm: *mut c_void, master: *mut c_void) -> i32,
    pub package_map_compute: unsafe extern "C" fn(pm: *mut c_void) -> i32,
    pub package_map_list_num: unsafe extern "C" fn(pm: *mut c_void, result: *mut i32) -> i32,
    pub package_info_guid_eq:
        unsafe extern "C" fn(pm: *mut c_void, i: i32, guid: *const u32, result: *mut i32) -> i32,
    pub package_info_remote_generation:
        unsafe extern "C" fn(pm: *mut c_void, i: i32, result: *mut *mut i32) -> i32,
    pub package_info_url_text:
        unsafe extern "C" fn(pm: *mut c_void, i: i32, result: *mut *const u32) -> i32,
    pub package_map_list_remove: unsafe extern "C" fn(pm: *mut c_void, i: i32) -> i32,
    pub fstring_right_ne_text:
        unsafe extern "C" fn(h: *mut c_void, n: i32, text: *const u32, result: *mut i32) -> i32,
    pub fstring_remove_segment: unsafe extern "C" fn(h: *mut c_void, start: i32, count: i32) -> i32,
    pub string_scope: unsafe extern "C" fn(
        source: *mut c_void,
        context: *mut c_void,
        body: Hp2LevelNetContinuation,
    ) -> i32,
    pub url_scope: unsafe extern "C" fn(
        text: *const u32,
        context: *mut c_void,
        body: Hp2LevelNetContinuation,
    ) -> i32,
    pub property_scope: unsafe extern "C" fn(
        klass: *mut c_void,
        context: *mut c_void,
        body: Hp2LevelNetContinuation,
    ) -> i32,
}

/// Trivial mirror field reads/writes: UE1 non-virtual accessors and plain
/// struct members, each verified against its generated ABI mirror.
impl Hp2LevelNetServices {
    /// `UObject::GetOuter()` (`UnObj.h`): `{return Outer;}`, non-virtual trivial.
    unsafe fn uobject_get_outer(&self, obj: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*obj.cast::<UObject>()).outer })
    }
    /// `UObject::GetLinker()` (`UnObj.h`): `{return _Linker;}`, non-virtual trivial.
    unsafe fn level_get_linker(&self, level: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*level.cast::<UObject>())._linker })
    }
    /// `&FURL::Map`: address only, no read of the underlying `FString`.
    unsafe fn furl_map(&self, h: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { std::ptr::addr_of_mut!((*h.cast::<FURL>()).map).cast::<c_void>() })
    }
    /// `UNetDriver::ServerConnection`: a plain field, no virtual dispatch.
    unsafe fn netdriver_server_connection(&self, drv: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*drv.cast::<UNetDriver>()).server_connection })
    }
    /// `UNetDriver::ServerTravelPause`: a plain field, no virtual dispatch.
    unsafe fn netdriver_server_travel_pause(&self, drv: *mut c_void) -> Result<f32, ()> {
        Ok(unsafe { (*drv.cast::<UNetDriver>()).server_travel_pause })
    }
    /// `UNetDriver::MaxClientRate`: a plain field, no virtual dispatch.
    unsafe fn netdriver_max_client_rate(&self, drv: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*drv.cast::<UNetDriver>()).max_client_rate })
    }
    /// `UNetDriver::MasterMap`: a plain field, no virtual dispatch.
    unsafe fn netdriver_master_map(&self, drv: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*drv.cast::<UNetDriver>()).master_map })
    }
    /// `UGameEngine::GPendingLevel`: a plain field, no virtual dispatch.
    unsafe fn game_engine_gpending_level(&self, ge: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*ge.cast::<UGameEngine>()).g_pending_level })
    }
    /// `UGameEngine::GPendingLevel = V`: a plain field assignment.
    unsafe fn game_engine_set_gpending_level(
        &self,
        ge: *mut c_void,
        v: *mut c_void,
    ) -> Result<(), ()> {
        unsafe { (*ge.cast::<UGameEngine>()).g_pending_level = v };
        Ok(())
    }
    /// `UDemoPlayPendingLevel::DemoRecDriver`: `DemoRecDriver` is declared on the
    /// shared `ULevelBase`, at the same offset for `ULevel` and
    /// `UDemoPlayPendingLevel`; `UDemoPlayPendingLevel` (288 bytes) is large
    /// enough that the `ULevel` mirror's field offset (216) stays in bounds.
    unsafe fn pending_level_demo_rec_driver(&self, p: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*p.cast::<ULevel>()).base.demo_rec_driver })
    }
    /// `UEngine::Client`: declared on `UEngine`, at the same offset for every
    /// `UEngine` subtype; cast through the `UGameEngine` mirror.
    unsafe fn engine_client(&self, engine: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).client })
    }
    /// `&ULevel::URL`: address only, no read of the underlying `FURL`.
    unsafe fn level_url(&self, level: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { std::ptr::addr_of_mut!((*level.cast::<ULevel>()).base.url).cast::<c_void>() })
    }
    /// `ALevelInfo::Game`: a plain field, no virtual dispatch.
    unsafe fn level_info_game(&self, li: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*li.cast::<ALevelInfo>()).game })
    }
    /// `&ALevelInfo::NextURL`: address only, no read of the underlying `FURL`.
    unsafe fn level_info_next_url(&self, li: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { std::ptr::addr_of_mut!((*li.cast::<ALevelInfo>()).next_url).cast::<c_void>() })
    }
    /// `ALevelInfo::bLonePlayer`: a plain bitfield, no virtual dispatch.
    unsafe fn level_info_b_lone_player(&self, li: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*li.cast::<ALevelInfo>()).b_lone_player() as i32 })
    }
    /// `ALevelInfo::NextSwitchCountdown = V`: a plain field assignment.
    unsafe fn level_info_set_next_switch_countdown(
        &self,
        li: *mut c_void,
        v: f32,
    ) -> Result<(), ()> {
        unsafe { (*li.cast::<ALevelInfo>()).next_switch_countdown = v };
        Ok(())
    }
    /// `ALevelInfo::NetMode = V`: a plain field assignment.
    unsafe fn level_info_set_net_mode(&self, li: *mut c_void, v: u8) -> Result<(), ()> {
        unsafe { (*li.cast::<ALevelInfo>()).net_mode = v };
        Ok(())
    }
    /// `AGameInfo::bWorldLog`: a plain bitfield, no virtual dispatch.
    unsafe fn game_info_b_world_log(&self, game: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*game.cast::<AGameInfo>()).b_world_log() as i32 })
    }
    /// `UObject::GetClass()` (`UnObj.h`): `{return Class;}`, non-virtual trivial.
    unsafe fn actor_get_class(&self, actor: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*actor.cast::<UObject>()).class })
    }
    /// `UNetConnection::Driver`: a plain field, no virtual dispatch.
    unsafe fn connection_driver(&self, conn: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).driver })
    }
    /// `UNetConnection::PackageMap`: a plain field, no virtual dispatch.
    unsafe fn connection_package_map(&self, conn: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).package_map })
    }
    /// `UNetConnection::RemoteVersion`: a plain field, no virtual dispatch.
    unsafe fn connection_remote_version(&self, conn: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).remote_version })
    }
    /// `UNetConnection::RemoteVersion = V`: a plain field assignment.
    unsafe fn connection_set_remote_version(&self, conn: *mut c_void, v: i32) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).remote_version = v };
        Ok(())
    }
    /// `UNetConnection::State = USOCK_Closed`: a plain field assignment.
    unsafe fn connection_set_state_closed(&self, conn: *mut c_void) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).state = USOCK_CLOSED };
        Ok(())
    }
    /// `UNetConnection::NegotiatedVer = V`: a plain field assignment.
    unsafe fn connection_set_negotiated_ver(&self, conn: *mut c_void, v: i32) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).negotiated_ver = v };
        Ok(())
    }
    /// `UNetConnection::Challenge`: a plain field, no virtual dispatch.
    unsafe fn connection_challenge(&self, conn: *mut c_void) -> Result<u32, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).challenge as u32 })
    }
    /// `UNetConnection::Challenge = V`: a plain field assignment.
    unsafe fn connection_set_challenge(&self, conn: *mut c_void, v: u32) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).challenge = v as i32 };
        Ok(())
    }
    /// `UNetConnection::UserFlags = V`: a plain field assignment.
    unsafe fn connection_set_user_flags(&self, conn: *mut c_void, v: i32) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).user_flags = v };
        Ok(())
    }
    /// `UNetConnection::CurrentNetSpeed = V`: a plain field assignment.
    unsafe fn connection_set_current_net_speed(
        &self,
        conn: *mut c_void,
        v: i32,
    ) -> Result<(), ()> {
        unsafe { (*conn.cast::<UNetConnection>()).current_net_speed = v };
        Ok(())
    }
    /// `UNetConnection::CurrentNetSpeed`: a plain field, no virtual dispatch.
    unsafe fn connection_current_net_speed(&self, conn: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).current_net_speed })
    }
    /// `UNetConnection::Actor`: a plain field, no virtual dispatch.
    unsafe fn connection_actor(&self, conn: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).actor })
    }
    /// `UChannel::Connection`: a plain field, no virtual dispatch.
    unsafe fn channel_connection(&self, ch: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { (*ch.cast::<UChannel>()).connection })
    }
    /// `UChannel::ChType`: a plain field, no virtual dispatch.
    unsafe fn channel_ch_type(&self, ch: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*ch.cast::<UChannel>()).ch_type })
    }
    /// `UChannel::ChIndex`: a plain field, no virtual dispatch.
    unsafe fn channel_ch_index(&self, ch: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { (*ch.cast::<UChannel>()).ch_index })
    }
}

/// A native stack scope lends its typed local only for the duration of this call.
pub type Hp2LevelNetContinuation = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

// A failed leaf immediately returns from the current policy/continuation. No
// cleanup callback runs in Rust: native stack scopes perform their own unwinding.
macro_rules! value {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        if ($callback)($($arg,)* out.as_mut_ptr()) != 0 { return Err(()); }
        out.assume_init()
    }};
}
macro_rules! action {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        if ($callback)($($arg),*) != 0 { return Err(()); }
    }};
}

unsafe fn with_string<T>(
    s: &Hp2LevelNetServices,
    source: *mut c_void,
    body: impl FnOnce(*mut c_void) -> Result<T, ()>,
) -> Result<T, ()> {
    unsafe {
        scoped(
            |context, resume| (s.string_scope)(source, context, resume),
            body,
        )
    }
}
unsafe fn with_url<T>(
    s: &Hp2LevelNetServices,
    text: *const u32,
    body: impl FnOnce(*mut c_void) -> Result<T, ()>,
) -> Result<T, ()> {
    unsafe { scoped(|context, resume| (s.url_scope)(text, context, resume), body) }
}
unsafe fn with_properties<T>(
    s: &Hp2LevelNetServices,
    class: *mut c_void,
    body: impl FnOnce(*mut c_void) -> Result<T, ()>,
) -> Result<T, ()> {
    unsafe {
        scoped(
            |context, resume| (s.property_scope)(class, context, resume),
            body,
        )
    }
}

// ---------------------------------------------------------------------------
// TCHAR helpers
// ---------------------------------------------------------------------------

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
) -> Result<i32, ()> {
    let lv = level.cast::<ULevel>();
    let mut cmd = cmd0;
    unsafe {
        let nd = (*lv).base.net_driver;
        if !nd.is_null() && value!(s.level_exec_net, nd, cmd, ar) != 0 {
            return Ok(1);
        }
        let dr = (*lv).base.demo_rec_driver;
        if !dr.is_null() && value!(s.level_exec_demo, dr, cmd, ar) != 0 {
            return Ok(1);
        }

        let kw = lit!("DEMOREC");
        if value!(s.parse_command, &mut cmd, kw.as_ptr()) != 0 {
            return with_url(s, std::ptr::null(), |url| {
                if value!(s.parse_token_fstring, &mut cmd, s.furl_map(url)?, 0) != 0 {
                    let map = s.furl_map(url)?;
                    let dem = lit!(".dem");
                    if value!(s.fstring_right_ne_text, map, 4, dem.as_ptr()) != 0 {
                        action!(s.fstring_append_text, map, dem.as_ptr());
                    }
                    action!(
                        s.debugf_log,
                        fmt(&[
                            A::L("Attempting to record demo "),
                            A::P(value!(s.fstring_text, map)),
                        ])
                        .as_ptr()
                    );

                    let cls = value!(
                        s.static_load_class,
                        0,
                        lit!("ini:Engine.Engine.DemoRecordingDevice").as_ptr()
                    );
                    let drv = value!(s.construct_net_driver, cls);
                    (*lv).base.demo_rec_driver = drv;
                    with_string(s, std::ptr::null_mut(), |error| {
                        if value!(s.netdriver_init_listen, drv, level, url, error) == 0 {
                            action!(
                                s.output_device_logf,
                                ar,
                                fmt(&[
                                    A::L("Demo recording failed: "),
                                    A::P(value!(s.fstring_text, error)),
                                ])
                                .as_ptr()
                            );
                            action!(s.netdriver_delete, drv);
                            (*lv).base.demo_rec_driver = std::ptr::null_mut();
                        } else {
                            action!(
                                s.output_device_logf,
                                ar,
                                fmt(&[
                                    A::L("Demo recording started to "),
                                    A::P(value!(s.fstring_text, map)),
                                ])
                                .as_ptr()
                            );
                        }
                        Ok(())
                    })?;
                } else {
                    action!(
                        s.output_device_log,
                        ar,
                        lit!("You must specify a filename").as_ptr()
                    );
                }
                Ok(1)
            });
        } else if value!(s.parse_command, &mut cmd, lit!("DEMOPLAY").as_ptr()) != 0 {
            return with_string(s, std::ptr::null_mut(), |temp| {
                if value!(s.parse_token_fstring, &mut cmd, temp, 0) != 0 {
                    let i = value!(s.fstring_caps_in_str, temp, lit!(".DEM").as_ptr());
                    if i != -1 {
                        action!(s.fstring_remove_segment, temp, i, 4);
                    }
                    with_url(s, value!(s.fstring_text, temp), |url| {
                        let dem = lit!(".dem");
                        action!(s.fstring_append_text, s.furl_map(url)?, dem.as_ptr());
                        action!(
                            s.debugf_log,
                            fmt(&[
                                A::L("Attempting to play demo "),
                                A::P(value!(s.fstring_text, s.furl_map(url)?)),
                            ])
                            .as_ptr()
                        );
                        let ge = value!(s.cast_game_engine, (*lv).base.engine);
                        if !s.game_engine_gpending_level(ge)?.is_null() {
                            action!(s.game_engine_cancel_pending, ge);
                        }
                        let pl = value!(s.demo_play_pending_level_new, ge, url);
                        s.game_engine_set_gpending_level(ge, pl)?;
                        if {
                            let pl2 = s.game_engine_gpending_level(ge)?;
                            s.pending_level_demo_rec_driver(pl2)?
                        }
                        .is_null()
                        {
                            let p = s.game_engine_gpending_level(ge)?;
                            action!(
                                s.output_device_logf,
                                ar,
                                fmt(&[
                                    A::L("Demo playback failed: "),
                                    A::P(value!(s.pending_level_error_text, p)),
                                ])
                                .as_ptr()
                            );
                            action!(s.pending_level_delete, ge);
                            s.game_engine_set_gpending_level(ge, std::ptr::null_mut())?;
                        }
                        Ok(())
                    })?;
                } else {
                    action!(
                        s.output_device_log,
                        ar,
                        lit!("You must specify a filename").as_ptr()
                    );
                }
                Ok(1)
            });
        }
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// ULevel::Listen / IsServer
// ---------------------------------------------------------------------------

/// Source Listen policy capabilities; native scopes and canonical owners use
/// the same sequence, including its configured-actor property traversal.
pub(crate) trait ListenOps: Sized {
    type Error;
    type Properties;
    unsafe fn localize_error(&mut self, level: *mut c_void, error: *mut c_void, key: &'static CStr) -> Result<(), Self::Error>;
    unsafe fn level_linker(&mut self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn load_class(&mut self, actor: bool, name: *const u32) -> Result<*mut c_void, Self::Error>;
    unsafe fn construct_driver(&mut self, class: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn init_driver(&mut self, driver: *mut c_void, level: *mut c_void, url: *mut c_void, error: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn delete_driver(&mut self, driver: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn error_text(&mut self, error: *mut c_void) -> Result<*const u32, Self::Error>;
    unsafe fn log(&mut self, text: *const u32) -> Result<(), Self::Error>;
    unsafe fn game_engine(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn build_master_map(&mut self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn server_actor_count(&mut self, engine: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn server_actor_text(&mut self, engine: *mut c_void, index: i32) -> Result<*const u32, Self::Error>;
    unsafe fn parse_token(&mut self, stream: &mut *const u32, buffer: &mut [u32; 240]) -> Result<bool, Self::Error>;
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn find_equals(&mut self, text: *const u32) -> Result<*const u32, Self::Error>;
    unsafe fn with_properties<T>(&mut self, class: *mut c_void,
        body: impl FnOnce(&mut Self, &mut Self::Properties) -> Result<T, Self::Error>) -> Result<T, Self::Error>;
    unsafe fn property_valid(&mut self, properties: &mut Self::Properties) -> Result<bool, Self::Error>;
    unsafe fn property_next(&mut self, properties: &mut Self::Properties) -> Result<(), Self::Error>;
    unsafe fn property_name_matches(&mut self, properties: &mut Self::Properties, name: *const u32) -> Result<bool, Self::Error>;
    unsafe fn property_is_config(&mut self, properties: &mut Self::Properties) -> Result<bool, Self::Error>;
    unsafe fn import_property(&mut self, properties: &mut Self::Properties, text: *const u32, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn level_info(&mut self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
}

/// # Safety
/// All pointers identify the live records/strings described by the source
/// Listen operation, including through synchronous owner reentry.
pub(crate) unsafe fn listen_with<O: ListenOps>(
    ops: &mut O, level: *mut c_void, error: *mut c_void,
) -> Result<i32, O::Error> {
    let lv = level.cast::<ULevel>();
    unsafe {
        if !(*lv).base.net_driver.is_null() {
            ops.localize_error(level, error, c"NetAlready")?;
            return Ok(0);
        }
        if ops.level_linker(level)?.is_null() {
            ops.localize_error(level, error, c"NetListen")?;
            return Ok(0);
        }
        let class = ops.load_class(false, lit!("ini:Engine.Engine.NetworkDevice").as_ptr())?;
        let driver = ops.construct_driver(class)?;
        (*lv).base.net_driver = driver;
        let url = std::ptr::addr_of_mut!((*lv).base.url).cast();
        if !ops.init_driver(driver, level, url, error)? {
            let text = ops.error_text(error)?;
            ops.log(fmt(&[A::L("Failed to listen: "), A::P(text)]).as_ptr())?;
            ops.delete_driver(driver)?;
            (*lv).base.net_driver = std::ptr::null_mut();
            return Ok(0);
        }
        let engine = ops.game_engine((*lv).base.engine)?;
        ops.build_master_map(engine, driver, level)?;
        let mut index = 0;
        while index < ops.server_actor_count(engine)? {
            let mut text = ops.server_actor_text(engine, index)?;
            let mut token = [0u32; 240];
            if ops.parse_token(&mut text, &mut token)? {
                ops.log(fmt(&[A::L("Spawning: "), A::P(token.as_ptr())]).as_ptr())?;
                let class = ops.load_class(true, token.as_ptr())?;
                let actor = ops.spawn_actor(level, class)?;
                while !actor.is_null() && ops.parse_token(&mut text, &mut token)? {
                    let equals = ops.find_equals(token.as_ptr())?;
                    if !equals.is_null() {
                        let position = equals.offset_from(token.as_ptr()) as usize;
                        token[position] = 0;
                        let value = token.as_ptr().add(position + 1);
                        let class = (*actor.cast::<UObject>()).class;
                        ops.with_properties(class, |ops, properties| {
                            while ops.property_valid(properties)? {
                                if ops.property_name_matches(properties, token.as_ptr())?
                                    && ops.property_is_config(properties)?
                                {
                                    ops.import_property(properties, value, actor)?;
                                }
                                ops.property_next(properties)?;
                            }
                            Ok(())
                        })?;
                    }
                }
            }
            index += 1;
        }
        let mode = if (*(*lv).base.engine.cast::<UGameEngine>()).client.is_null() {
            NM_DEDICATED_SERVER
        } else {
            NM_LISTEN_SERVER
        };
        (*ops.level_info(level)?.cast::<ALevelInfo>()).net_mode = mode;
        let pause = (*(*lv).base.net_driver.cast::<UNetDriver>()).server_travel_pause;
        (*ops.level_info(level)?.cast::<ALevelInfo>()).next_switch_countdown = pause;
        Ok(1)
    }
}

struct NativeListenOps<'a>(&'a Hp2LevelNetServices);
impl ListenOps for NativeListenOps<'_> {
    type Error = ();
    type Properties = *mut c_void;
    unsafe fn localize_error(&mut self, level: *mut c_void, error: *mut c_void, key: &'static CStr) -> Result<(), ()> {
        unsafe { action!(self.0.localize_error_assign, level, error, key.as_ptr()); }
        Ok(())
    }
    unsafe fn level_linker(&mut self, level: *mut c_void) -> Result<*mut c_void, ()> {
        unsafe { self.0.level_get_linker(level) }
    }
    unsafe fn load_class(&mut self, actor: bool, name: *const u32) -> Result<*mut c_void, ()> {
        Ok(unsafe { value!(self.0.static_load_class, i32::from(actor), name) })
    }
    unsafe fn construct_driver(&mut self, class: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { value!(self.0.static_construct_net_driver, class) })
    }
    unsafe fn init_driver(&mut self, driver: *mut c_void, level: *mut c_void, url: *mut c_void, error: *mut c_void) -> Result<bool, ()> {
        Ok(unsafe { value!(self.0.netdriver_init_listen, driver, level, url, error) } != 0)
    }
    unsafe fn delete_driver(&mut self, driver: *mut c_void) -> Result<(), ()> {
        unsafe { action!(self.0.netdriver_delete, driver); }
        Ok(())
    }
    unsafe fn error_text(&mut self, error: *mut c_void) -> Result<*const u32, ()> {
        Ok(unsafe { value!(self.0.fstring_text, error) })
    }
    unsafe fn log(&mut self, text: *const u32) -> Result<(), ()> {
        unsafe { action!(self.0.debugf_log, text); }
        Ok(())
    }
    unsafe fn game_engine(&mut self, engine: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { value!(self.0.cast_game_engine, engine) })
    }
    unsafe fn build_master_map(&mut self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> Result<(), ()> {
        unsafe { action!(self.0.game_build_server_master_map, engine, driver, level); }
        Ok(())
    }
    unsafe fn server_actor_count(&mut self, engine: *mut c_void) -> Result<i32, ()> {
        Ok(unsafe { value!(self.0.game_engine_server_actors_num, engine) })
    }
    unsafe fn server_actor_text(&mut self, engine: *mut c_void, index: i32) -> Result<*const u32, ()> {
        Ok(unsafe { value!(self.0.game_engine_server_actor_text, engine, index) })
    }
    unsafe fn parse_token(&mut self, stream: &mut *const u32, buffer: &mut [u32; 240]) -> Result<bool, ()> {
        Ok(unsafe { value!(self.0.parse_token_buf, stream, buffer.as_mut_ptr(), 240, 1) } != 0)
    }
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void) -> Result<*mut c_void, ()> {
        unsafe {
            crate::level_spawn_actor::virtual_spawn_actor(
                level, class, FName { index: 0 }, std::ptr::null_mut(), std::ptr::null_mut(),
                FVector { x: 0.0, y: 0.0, z: 0.0 }, FRotator { pitch: 0, yaw: 0, roll: 0 },
                std::ptr::null_mut(), 0, 0,
            ).map(|actor| actor as *mut c_void).map_err(|_| ())
        }
    }
    unsafe fn find_equals(&mut self, text: *const u32) -> Result<*const u32, ()> {
        Ok(unsafe { value!(self.0.app_strchr, text, u32::from(b'=')) })
    }
    unsafe fn with_properties<T>(&mut self, class: *mut c_void,
        body: impl FnOnce(&mut Self, &mut Self::Properties) -> Result<T, ()>) -> Result<T, ()> {
        let services = self.0;
        unsafe { with_properties(services, class, |mut properties| body(self, &mut properties)) }
    }
    unsafe fn property_valid(&mut self, properties: &mut Self::Properties) -> Result<bool, ()> {
        Ok(unsafe { value!(self.0.tfield_iter_valid, *properties) } != 0)
    }
    unsafe fn property_next(&mut self, properties: &mut Self::Properties) -> Result<(), ()> {
        unsafe { action!(self.0.tfield_iter_next, *properties); }
        Ok(())
    }
    unsafe fn property_name_matches(&mut self, properties: &mut Self::Properties, name: *const u32) -> Result<bool, ()> {
        Ok(unsafe { value!(self.0.uproperty_name_stricmp, *properties, name) } == 0)
    }
    unsafe fn property_is_config(&mut self, properties: &mut Self::Properties) -> Result<bool, ()> {
        Ok(unsafe { value!(self.0.uproperty_is_config, *properties) } != 0)
    }
    unsafe fn import_property(&mut self, properties: &mut Self::Properties, text: *const u32, actor: *mut c_void) -> Result<(), ()> {
        unsafe { action!(self.0.uproperty_import_text, *properties, text, actor); }
        Ok(())
    }
    unsafe fn level_info(&mut self, level: *mut c_void) -> Result<*mut c_void, ()> {
        Ok(unsafe { value!(self.0.level_get_level_info, level) })
    }
}

/// `ULevel::Listen(FString& Error)`.
///
/// # Safety
/// `level` must be a live `ULevel`; `error` a live `FString` handle.
pub unsafe fn listen(
    services: &Hp2LevelNetServices, level: *mut c_void, error: *mut c_void,
) -> Result<i32, ()> {
    unsafe { listen_with(&mut NativeListenOps(services), level, error) }
}

/// `ULevel::IsServer()`.
///
/// # Safety
/// `level` must be a live `ULevel`.
pub unsafe fn is_server(s: &Hp2LevelNetServices, level: *mut c_void) -> Result<i32, ()> {
    let lv = level.cast::<ULevel>();
    unsafe {
        let nd = (*lv).base.net_driver;
        let a = nd.is_null() || s.netdriver_server_connection(nd)?.is_null();
        if !a {
            return Ok(0);
        }
        let dr = (*lv).base.demo_rec_driver;
        Ok((dr.is_null() || s.netdriver_server_connection(dr)?.is_null()) as i32)
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
        let nd = (*lv).base.net_driver;
        if check(s, c"NetDriver", 1328, nd.is_null()) {
            return Err(());
        }
        if !s.netdriver_server_connection(nd)?.is_null() {
            action!(
                s.debugf_dev_net,
                lit!("NotifyAcceptingConnection: Client refused").as_ptr()
            );
            return Ok(ACCEPTC_REJECT);
        }
        let next = s.level_info_next_url(value!(s.level_get_level_info, level))?;
        if value!(s.fstring_ne_text, next, lit!("").as_ptr()) != 0 {
            action!(
                s.debugf_dev_net,
                fmt(&[
                    A::L("NotifyAcceptingConnection: Server "),
                    A::P(value!(s.uobject_get_name, level)),
                    A::L(" refused"),
                ])
                .as_ptr()
            );
            return Ok(ACCEPTC_IGNORE);
        }
        action!(
            s.debugf_dev_net,
            fmt(&[
                A::L("NotifyAcceptingConnection: Server "),
                A::P(value!(s.uobject_get_name, level)),
                A::L(" accept"),
            ])
            .as_ptr()
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
        let nd = (*lv).base.net_driver;
        if check(s, c"NetDriver!=NULL", 1356, nd.is_null()) {
            return Err(());
        }
        if check(
            s,
            c"NetDriver->ServerConnection==NULL",
            1357,
            !s.netdriver_server_connection(nd)?.is_null(),
        ) {
            return Err(());
        }
        let name = value!(s.uobject_get_name, level);
        let ts = value!(s.app_timestamp);
        with_string(s, conn, |addr| {
            action!(
                s.debugf_net_come_go,
                fmt(&[
                    A::L("Open "),
                    A::P(name),
                    A::L(" "),
                    A::P(ts),
                    A::L(" "),
                    A::P(value!(s.fstring_text, addr)),
                ])
                .as_ptr()
            );
            Ok(())
        })
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
    // SAFETY: the native caller supplies the source channel/connection/driver graph.
    unsafe {
        crate::level_net_owner::notify_accepting_channel_with(
            &mut NativeAcceptChannelOps(s), level.cast(), ch.cast(),
        ).map(i32::from)
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
) -> Result<(), ()> {
    let mut ops = NativeWelcomePlayerOps(s);
    unsafe { crate::level_net_owner::welcome_player_with(&mut ops, level, conn, optional) }
}

struct NativeWelcomePlayerOps<'a>(&'a Hp2LevelNetServices);

impl crate::level_net_owner::WelcomePlayerOps for NativeWelcomePlayerOps<'_> {
    type Error = ();

    unsafe fn copy_package_map(&mut self, conn: *mut c_void) -> Result<(), ()> {
        let connection = conn.cast::<UNetConnection>();
        let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
        let package_map = unsafe { (*connection).package_map };
        let master_map = unsafe { (*driver).master_map };
        if unsafe { (self.0.package_map_copy)(package_map, master_map) } == 0 { Ok(()) } else { Err(()) }
    }

    unsafe fn send_package_map(&mut self, conn: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.0.connection_send_package_map)(conn) } == 0 { Ok(()) } else { Err(()) }
    }

    unsafe fn welcome_values(&mut self, level: *mut c_void) -> Result<(Vec<u32>, i32), ()> {
        let outer = unsafe { (*level.cast::<UObject>()).outer };
        let mut name = std::mem::MaybeUninit::uninit();
        if unsafe { (self.0.uobject_get_name)(outer, name.as_mut_ptr()) } != 0 { return Err(()); }
        let name_ptr = unsafe { name.assume_init() };
        let mut info = std::mem::MaybeUninit::uninit();
        if unsafe { (self.0.level_get_level_info)(level, info.as_mut_ptr()) } != 0 { return Err(()); }
        let info = unsafe { info.assume_init().cast::<ALevelInfo>() };
        if info.is_null() { return Err(()); }
        let lone = unsafe { (*info).b_lone_player() as i32 };
        let mut units = Vec::new();
        let mut cursor = name_ptr;
        while unsafe { *cursor } != 0 { units.push(unsafe { *cursor }); cursor = unsafe { cursor.add(1) }; }
        Ok((units, lone))
    }

    unsafe fn connection_log(&mut self, conn: *mut c_void, message: &[u32]) -> Result<(), ()> {
        let mut text = message.to_vec();
        text.push(0);
        if unsafe { (self.0.connection_log)(conn, text.as_ptr()) } == 0 { Ok(()) } else { Err(()) }
    }

    unsafe fn flush_net(&mut self, conn: *mut c_void) -> Result<(), ()> {
        if unsafe { (self.0.connection_flush_net)(conn) } == 0 { Ok(()) } else { Err(()) }
    }
}

/// Low-level operations used by the shared received-text policy.
#[derive(Clone, Copy)]
#[repr(u32)]
pub(crate) enum NotifyTextOp {
    ParseCommand,
    ParseInt,
    ParseGuid,
    ParseText,
    AppAtoi,
    AppCycles,
    LevelEngine,
    LevelNetDriver,
    NetDriverServerConnection,
    CheckFailed,
    DebugDevNet,
    DebugLog,
    EngineClient,
    EngineClientViewportsNum,
    EngineClientViewport,
    EngineSetClientTravel,
    ConnectionSetUserFlags,
    ConnectionSetRemoteVersion,
    ConnectionSetNegotiatedVersion,
    LevelGetInfo,
    LevelInfoGame,
    GameWorldLog,
    ConnectionSetChallenge,
    ConnectionRemoteVersion,
    ConnectionSetClosed,
    ConnectionSetCurrentNetSpeed,
    ConnectionCurrentNetSpeed,
    NetDriverMaxClientRate,
    ConnectionPackageMap,
    PackageMapListNum,
    PackageInfoGuidEqual,
    PackageInfoRemoteGeneration,
    PackageInfoUrlText,
    PackageMapRemove,
    EngineChallengeResponse,
    ConnectionChallenge,
    ConnectionSetRequestUrl,
    ConnectionRequestUrlText,
    FStringNotEmpty,
    FStringText,
    ConnectionChannelClose,
    ConnectionFlushNet,
    ConnectionLog,
    LevelWelcomePlayer,
    PackageMapCompute,
    SpawnPlayActor,
    ConnectionActor,
    ConnectionActorPlayerName,
}

/// Supplies low-level text-notification operations and scoped engine temporaries.
pub(crate) trait NotifyReceivedTextOps {
    type Error;
    fn invalid_error(&mut self) -> Self::Error;

    unsafe fn call(
        &mut self,
        op: NotifyTextOp,
        a: usize,
        b: usize,
        c: usize,
    ) -> Result<usize, Self::Error>;

    unsafe fn with_pre_login<T>(
        &mut self,
        body: impl FnOnce(&mut Self, *mut c_void, *mut c_void) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;

    unsafe fn pre_login(
        &mut self,
        game: *mut c_void,
        conn: *mut c_void,
        options: *const u32,
        error: *mut c_void,
        fail_code: *mut c_void,
    ) -> Result<(), Self::Error>;

    unsafe fn with_url<T>(
        &mut self,
        text: *const u32,
        body: impl FnOnce(&mut Self, *mut c_void) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;

    unsafe fn with_join_error<T>(
        &mut self,
        body: impl FnOnce(&mut Self, *mut c_void) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
}

/// Executes `NotifyReceivedText` branch policy through the supplied owner.
///
/// # Safety
/// `level`, `conn`, and `text` must be live engine objects / TCHAR storage for
/// the duration of the call; each operation must honor its pointer arguments.
pub(crate) unsafe fn notify_received_text_with<O: NotifyReceivedTextOps>(
    ops: &mut O,
    level: *mut c_void,
    conn: *mut c_void,
    text0: *const u32,
) -> Result<(), O::Error> {
    macro_rules! call {
        ($op:ident, $a:expr, $b:expr, $c:expr) => {
            unsafe {
                ops.call(
                    NotifyTextOp::$op,
                    $a as usize,
                    $b as usize,
                    $c as usize,
                )?
            }
        };
    }
    macro_rules! ptr {
        ($op:ident, $a:expr, $b:expr, $c:expr) => {
            call!($op, $a, $b, $c) as *mut c_void
        };
    }
    macro_rules! log {
        ($op:ident, $conn:expr, $message:expr) => {{
            let message = $message;
            call!($op, $conn, message.as_ptr(), 0);
        }};
    }
    let mut text = text0;
    let parsed_userflag = call!(
        ParseCommand,
        &mut text as *mut *const u32,
        lit!("USERFLAG").as_ptr(),
        0
    ) != 0;
    if parsed_userflag {
        let user_flags = call!(AppAtoi, text, 0, 0) as i32;
        call!(ConnectionSetUserFlags, conn, user_flags, 0);
        return Ok(());
    }

    let driver = ptr!(LevelNetDriver, level, 0, 0);
    if !driver.is_null()
        && !ptr!(NetDriverServerConnection, driver, 0, 0).is_null()
    {
        log!(
            DebugDevNet,
            0,
            fmt(&[A::L("Level client received: "), A::P(text)])
        );
        if call!(ParseCommand, &mut text as *mut *const u32, lit!("FAILURE").as_ptr(), 0) != 0 {
            let engine = ptr!(LevelEngine, level, 0, 0);
            let client = ptr!(EngineClient, engine, 0, 0);
            if call!(EngineClientViewportsNum, client, 0, 0) == 0
                && call!(
                    CheckFailed,
                    c"Engine->Client->Viewports.Num()".as_ptr(),
                    1452,
                    0
                ) != 0
            {
                return Err(ops.invalid_error());
            }
            let viewport = ptr!(EngineClientViewport, client, 0, 0);
            call!(
                EngineSetClientTravel,
                engine,
                viewport,
                lit!("?failed").as_ptr()
            );
        }
    } else {
        log!(
            DebugDevNet,
            0,
            fmt(&[A::L("Level server received: "), A::P(text)])
        );
        if call!(ParseCommand, &mut text as *mut *const u32, lit!("HELLO").as_ptr(), 0) != 0 {
            let mut remote_min = 219i32;
            let mut remote_ver = 219i32;
            call!(ParseInt, text, lit!("MINVER=").as_ptr(), &mut remote_min as *mut i32);
            call!(ParseInt, text, lit!("VER=").as_ptr(), &mut remote_ver as *mut i32);
            call!(ConnectionSetRemoteVersion, conn, remote_ver, 0);
            if remote_ver < ENGINE_MIN_NET_VERSION || remote_min > ENGINE_VERSION {
                log!(
                    ConnectionLog,
                    conn,
                    fmt(&[
                        A::L("UPGRADE MINVER="),
                        A::I(ENGINE_MIN_NET_VERSION),
                        A::L(" VER="),
                        A::I(ENGINE_VERSION),
                    ])
                );
                call!(ConnectionChannelClose, conn, 0, 0);
                call!(ConnectionFlushNet, conn, 0, 0);
                if call!(ConnectionRemoteVersion, conn, 0, 0) as i32 <= 420 {
                    call!(ConnectionSetClosed, conn, 0, 0);
                }
                return Ok(());
            }
            let negotiated = remote_ver.min(ENGINE_VERSION);
            call!(ConnectionSetNegotiatedVersion, conn, negotiated, 0);
            let info = ptr!(LevelGetInfo, level, 0, 0);
            let game = ptr!(LevelInfoGame, info, 0, 0);
            let stats = call!(GameWorldLog, game, 0, 0) as i32;
            let challenge = call!(AppCycles, 0, 0, 0) as u32;
            call!(ConnectionSetChallenge, conn, challenge, 0);
            log!(
                ConnectionLog,
                conn,
                fmt(&[
                    A::L("CHALLENGE VER="),
                    A::I(negotiated),
                    A::L(" CHALLENGE="),
                    A::I(challenge as i32),
                    A::L(" STATS="),
                    A::I(stats),
                ])
            );
            call!(ConnectionFlushNet, conn, 0, 0);
        } else if call!(ParseCommand, &mut text as *mut *const u32, lit!("NETSPEED").as_ptr(), 0) != 0 {
            let rate = call!(AppAtoi, text, 0, 0) as i32;
            if rate >= 2000 {
                let max = call!(NetDriverMaxClientRate, driver, 0, 0) as i32;
                call!(
                    ConnectionSetCurrentNetSpeed,
                    conn,
                    clamp_c(rate, 2000, max),
                    0
                );
            }
            let current = call!(ConnectionCurrentNetSpeed, conn, 0, 0) as i32;
            log!(
                DebugLog,
                0,
                fmt(&[A::L("Client netspeed is "), A::I(current)])
            );
        } else if call!(ParseCommand, &mut text as *mut *const u32, lit!("HAVE").as_ptr(), 0) != 0 {
            let mut guid = [0u32; 4];
            call!(ParseGuid, text, lit!("GUID=").as_ptr(), guid.as_mut_ptr());
            let package_map = ptr!(ConnectionPackageMap, conn, 0, 0);
            let mut i = 0i32;
            while i < call!(PackageMapListNum, package_map, 0, 0) as i32 {
                if call!(PackageInfoGuidEqual, package_map, i, guid.as_ptr()) != 0 {
                    let generation =
                        ptr!(PackageInfoRemoteGeneration, package_map, i, 0).cast::<i32>();
                    call!(ParseInt, text, lit!("GEN=").as_ptr(), generation);
                }
                i += 1;
            }
        } else if call!(ParseCommand, &mut text as *mut *const u32, lit!("SKIP").as_ptr(), 0) != 0 {
            let mut guid = [0u32; 4];
            call!(ParseGuid, text, lit!("GUID=").as_ptr(), guid.as_mut_ptr());
            let package_map = ptr!(ConnectionPackageMap, conn, 0, 0);
            if !package_map.is_null() {
                let mut i = 0i32;
                while i < call!(PackageMapListNum, package_map, 0, 0) as i32 {
                    if call!(PackageInfoGuidEqual, package_map, i, guid.as_ptr()) != 0 {
                        let url = call!(PackageInfoUrlText, package_map, i, 0) as *const u32;
                        log!(
                            DebugLog,
                            0,
                            fmt(&[
                                A::L("User skipped download of '"),
                                A::P(url),
                                A::L("'"),
                            ])
                        );
                        call!(PackageMapRemove, package_map, i, 0);
                        break;
                    }
                    i += 1;
                }
            }
        } else if call!(ParseCommand, &mut text as *mut *const u32, lit!("LOGIN").as_ptr(), 0) != 0 {
            let mut response = 0i32;
            let parsed = call!(ParseInt, text, lit!("RESPONSE=").as_ptr(), &mut response as *mut i32) as i32;
            let rejected = parsed == 0 || {
                let engine = ptr!(LevelEngine, level, 0, 0);
                let challenge = call!(ConnectionChallenge, conn, 0, 0) as u32;
                let challenge_response =
                    call!(EngineChallengeResponse, engine, challenge, 0) as i32;
                ((challenge_response == 0) as i32) == response
            };
            if rejected {
                log!(ConnectionLog, conn, lit!("FAILURE CHALLENGE"));
                call!(ConnectionChannelClose, conn, 0, 0);
                call!(ConnectionFlushNet, conn, 0, 0);
                if call!(ConnectionRemoteVersion, conn, 0, 0) as i32 <= 420 {
                    call!(ConnectionSetClosed, conn, 0, 0);
                }
                return Ok(());
            }
            let mut options = [0u32; 1024];
            unsafe {
                ops.with_pre_login(|ops, error, fail_code| {
                    ops.call(NotifyTextOp::ParseText, text as usize, lit!("URL=").as_ptr() as usize, options.as_mut_ptr() as usize)?;
                    ops.call(NotifyTextOp::ConnectionSetRequestUrl, conn as usize, options.as_ptr() as usize, 0)?;
                    let request_url =
                        ops.call(NotifyTextOp::ConnectionRequestUrlText, conn as usize, 0, 0)? as *const u32;
                    let request_message = fmt(&[A::L("Login request: "), A::P(request_url)]);
                    ops.call(NotifyTextOp::DebugDevNet, 0, request_message.as_ptr() as usize, 0)?;
                    let mut end = 0usize;
                    while options[end] != 0 && options[end] != b'?' as u32 {
                        end += 1;
                    }
                    let info = ops.call(NotifyTextOp::LevelGetInfo, level as usize, 0, 0)? as *mut std::ffi::c_void;
                    let game = ops.call(NotifyTextOp::LevelInfoGame, info as usize, 0, 0)? as *mut std::ffi::c_void;
                    ops.pre_login(
                        game,
                        conn,
                        options.as_ptr().add(end),
                        error,
                        fail_code,
                    )?;
                    if ops.call(NotifyTextOp::FStringNotEmpty, error as usize, 0, 0)? != 0 {
                        let error_text = ops.call(
                            NotifyTextOp::FStringText,
                            error as usize,
                            0,
                            0,
                        )? as *const u32;
                        let fail_text = ops.call(
                            NotifyTextOp::FStringText,
                            fail_code as usize,
                            0,
                            0,
                        )? as *const u32;
                        let message = fmt(&[
                            A::L("PreLogin failure: "),
                            A::P(error_text),
                            A::L(" ("),
                            A::P(fail_text),
                            A::L(")"),
                        ]);
                        ops.call(
                            NotifyTextOp::DebugDevNet,
                            0,
                            message.as_ptr() as usize,
                            0,
                        )?;
                        let message = fmt(&[A::L("FAILURE "), A::P(error_text)]);
                        ops.call(
                            NotifyTextOp::ConnectionLog,
                            conn as usize,
                            message.as_ptr() as usize,
                            0,
                        )?;
                        if *fail_text != 0 {
                            let message = fmt(&[A::L("FAILCODE "), A::P(fail_text)]);
                            ops.call(
                                NotifyTextOp::ConnectionLog,
                                conn as usize,
                                message.as_ptr() as usize,
                                0,
                            )?;
                        }
                        ops.call(NotifyTextOp::ConnectionChannelClose, conn as usize, 0, 0)?;
                        ops.call(NotifyTextOp::ConnectionFlushNet, conn as usize, 0, 0)?;
                        if ops.call(
                            NotifyTextOp::ConnectionRemoteVersion,
                            conn as usize,
                            0,
                            0,
                        )? as i32
                            <= 420
                        {
                            ops.call(NotifyTextOp::ConnectionSetClosed, conn as usize, 0, 0)?;
                        }
                        return Ok(());
                    }
                    ops.call(
                        NotifyTextOp::LevelWelcomePlayer,
                        level as usize,
                        conn as usize,
                        0,
                    )?;
                    Ok(())
                })
            }?;
        } else if call!(ParseCommand, &mut text as *mut *const u32, lit!("JOIN").as_ptr(), 0) != 0
            && ptr!(ConnectionActor, conn, 0, 0).is_null()
        {
            let package_map = ptr!(ConnectionPackageMap, conn, 0, 0);
            call!(PackageMapCompute, package_map, 0, 0);
            unsafe {
                ops.with_join_error(|ops, error| {
                    let request_url =
                        ops.call(NotifyTextOp::ConnectionRequestUrlText, conn as usize, 0, 0)? as *const u32;
                    let join_message = fmt(&[A::L("Join request: "), A::P(request_url)]);
                    ops.call(NotifyTextOp::DebugDevNet, 0, join_message.as_ptr() as usize, 0)?;
                    ops.with_url(request_url, |ops, url| {
                        let actor = ops.call(
                            NotifyTextOp::SpawnPlayActor,
                            level as usize,
                            conn as usize,
                            url as usize,
                        )?;
                        if actor == 0 {
                            let error_text = ops.call(
                                NotifyTextOp::FStringText,
                                error as usize,
                                0,
                                0,
                            )? as *const u32;
                            let message =
                                fmt(&[A::L("Join failure: "), A::P(error_text)]);
                            ops.call(
                                NotifyTextOp::DebugDevNet,
                                0,
                                message.as_ptr() as usize,
                                0,
                            )?;
                            let message = fmt(&[A::L("FAILURE "), A::P(error_text)]);
                            ops.call(
                                NotifyTextOp::ConnectionLog,
                                conn as usize,
                                message.as_ptr() as usize,
                                0,
                            )?;
                            ops.call(NotifyTextOp::ConnectionChannelClose, conn as usize, 0, 0)?;
                            ops.call(NotifyTextOp::ConnectionFlushNet, conn as usize, 0, 0)?;
                            if ops.call(
                                NotifyTextOp::ConnectionRemoteVersion,
                                conn as usize,
                                0,
                                0,
                            )? as i32
                                <= 420
                            {
                                ops.call(NotifyTextOp::ConnectionSetClosed, conn as usize, 0, 0)?;
                            }
                        } else {
                            let player_name = ops.call(
                                NotifyTextOp::ConnectionActorPlayerName,
                                conn as usize,
                                0,
                                0,
                            )? as *const u32;
                            let message =
                                fmt(&[A::L("Join succeeded: "), A::P(player_name)]);
                            ops.call(
                                NotifyTextOp::DebugDevNet,
                                0,
                                message.as_ptr() as usize,
                                0,
                            )?;
                        }
                        Ok(())
                    })
                })
            }?;
        }
    }
    Ok(())
}

struct NativeNotifyReceivedTextOps<'a> {
    services: &'a Hp2LevelNetServices,
    join_error: *mut c_void,
}

impl NotifyReceivedTextOps for NativeNotifyReceivedTextOps<'_> {
    type Error = ();
    fn invalid_error(&mut self) {}

    unsafe fn call(
        &mut self,
        op: NotifyTextOp,
        a: usize,
        b: usize,
        c: usize,
    ) -> Result<usize, ()> {
        let mut out = std::mem::MaybeUninit::<usize>::new(0);
        let s = self.services;
        let p = |value: usize| value as *mut c_void;
        let t = |value: usize| value as *const u32;
        let status = unsafe {
            match op {
                NotifyTextOp::ParseCommand => (s.parse_command)(a as *mut _, t(b), out.as_mut_ptr().cast()),
                NotifyTextOp::ParseInt => (s.parse_int)(t(a), t(b), c as *mut i32, out.as_mut_ptr().cast()),
                NotifyTextOp::ParseGuid => (s.parse_guid)(t(a), t(b), c as *mut u32, out.as_mut_ptr().cast()),
                NotifyTextOp::ParseText => (s.parse_text)(t(a), t(b), c as *mut u32, 1024, out.as_mut_ptr().cast()),
                NotifyTextOp::AppAtoi => (s.app_atoi)(t(a), out.as_mut_ptr().cast()),
                NotifyTextOp::AppCycles => (s.app_cycles)(out.as_mut_ptr().cast()),
                NotifyTextOp::DebugDevNet => (s.debugf_dev_net)(t(b)),
                NotifyTextOp::DebugLog => (s.debugf_log)(t(b)),
                NotifyTextOp::LevelNetDriver => { out.write((*p(a).cast::<ULevel>()).base.net_driver as usize); 0 },
                NotifyTextOp::LevelEngine => { out.write((*p(a).cast::<ULevel>()).base.engine as usize); 0 },
                NotifyTextOp::NetDriverServerConnection => { out.write((*p(a).cast::<UNetDriver>()).server_connection as usize); 0 },
                NotifyTextOp::CheckFailed => {
                    out.write(
                        ((s.check_failed)(a as *const i8, FILE_NAME.as_ptr(), b as i32) != 0)
                            as usize,
                    );
                    0
                }
                NotifyTextOp::EngineClient => {
                    out.write((*p(a).cast::<UGameEngine>()).client as usize);
                    0
                }
                NotifyTextOp::EngineClientViewportsNum =>
                    (s.engine_client_viewports_num)(p(a), out.as_mut_ptr().cast()),
                NotifyTextOp::EngineSetClientTravel => (s.engine_set_client_travel)(p(a), p(b), t(c), 0, TRAVEL_ABSOLUTE),
                NotifyTextOp::EngineClientViewport =>
                    (s.engine_client_viewport)(p(a), b as i32, out.as_mut_ptr().cast()),
                NotifyTextOp::ConnectionSetUserFlags => {
                    (*p(a).cast::<UNetConnection>()).user_flags = b as i32;
                    0
                }
                NotifyTextOp::ConnectionSetRemoteVersion => {
                    (*p(a).cast::<UNetConnection>()).remote_version = b as i32;
                    0
                }
                NotifyTextOp::ConnectionSetNegotiatedVersion => {
                    (*p(a).cast::<UNetConnection>()).negotiated_ver = b as i32;
                    0
                }
                NotifyTextOp::LevelGetInfo => (s.level_get_level_info)(p(a), out.as_mut_ptr().cast()),
                NotifyTextOp::LevelInfoGame => { out.write((*p(a).cast::<ALevelInfo>()).game as usize); 0 },
                NotifyTextOp::GameWorldLog => { out.write((*p(a).cast::<AGameInfo>()).b_world_log() as usize); 0 },
                NotifyTextOp::ConnectionSetChallenge => { (*p(a).cast::<UNetConnection>()).challenge = b as u32 as i32; 0 },
                NotifyTextOp::ConnectionRemoteVersion => { out.write((*p(a).cast::<UNetConnection>()).remote_version as usize); 0 },
                NotifyTextOp::ConnectionSetClosed => { (*p(a).cast::<UNetConnection>()).state = USOCK_CLOSED; 0 },
                NotifyTextOp::ConnectionSetCurrentNetSpeed => { (*p(a).cast::<UNetConnection>()).current_net_speed = b as i32; 0 },
                NotifyTextOp::ConnectionCurrentNetSpeed => { out.write((*p(a).cast::<UNetConnection>()).current_net_speed as usize); 0 },
                NotifyTextOp::NetDriverMaxClientRate => { out.write((*p(a).cast::<UNetDriver>()).max_client_rate as usize); 0 },
                NotifyTextOp::ConnectionPackageMap => { out.write((*p(a).cast::<UNetConnection>()).package_map as usize); 0 },
                NotifyTextOp::PackageMapListNum => (s.package_map_list_num)(p(a), out.as_mut_ptr().cast()),
                NotifyTextOp::PackageInfoGuidEqual => (s.package_info_guid_eq)(p(a), b as i32, c as *const u32, out.as_mut_ptr().cast()),
                NotifyTextOp::PackageInfoRemoteGeneration => (s.package_info_remote_generation)(p(a), b as i32, out.as_mut_ptr().cast()),
                NotifyTextOp::PackageInfoUrlText => (s.package_info_url_text)(p(a), b as i32, out.as_mut_ptr().cast()),
                NotifyTextOp::PackageMapRemove => (s.package_map_list_remove)(p(a), b as i32),
                NotifyTextOp::EngineChallengeResponse => (s.engine_challenge_response)(p(a), b as u32, out.as_mut_ptr().cast()),
                NotifyTextOp::ConnectionChallenge => { out.write((*p(a).cast::<UNetConnection>()).challenge as u32 as usize); 0 },
                NotifyTextOp::ConnectionSetRequestUrl => (s.connection_set_request_url)(p(a), t(b)),
                NotifyTextOp::ConnectionRequestUrlText => (s.connection_request_url_text)(p(a), out.as_mut_ptr().cast()),
                NotifyTextOp::FStringNotEmpty => (s.fstring_ne_text)(p(a), lit!("").as_ptr(), out.as_mut_ptr().cast()),
                NotifyTextOp::FStringText => (s.fstring_text)(p(a), out.as_mut_ptr().cast()),
                NotifyTextOp::ConnectionChannelClose => (s.connection_channel_close)(p(a), b as i32),
                NotifyTextOp::ConnectionFlushNet => (s.connection_flush_net)(p(a)),
                NotifyTextOp::ConnectionLog => (s.connection_log)(p(a), t(b)),
                NotifyTextOp::LevelWelcomePlayer => (s.level_welcome_player)(p(a), p(b)),
                NotifyTextOp::PackageMapCompute => (s.package_map_compute)(p(a)),
                NotifyTextOp::SpawnPlayActor => {
                    let mut result = std::mem::MaybeUninit::uninit();
                    let status = (s.spawn_play_actor)(
                        p(a),
                        p(b),
                        ROLE_AUTONOMOUS_PROXY,
                        p(c),
                        self.join_error,
                        result.as_mut_ptr(),
                    );
                    if status == 0 {
                        out.write(result.assume_init() as usize);
                    }
                    status
                }
                NotifyTextOp::ConnectionActor => { out.write((*p(a).cast::<UNetConnection>()).actor as usize); 0 },
                NotifyTextOp::ConnectionActorPlayerName => (s.connection_actor_player_name)(p(a), out.as_mut_ptr().cast()),
            }
        };
        if status != 0 { return Err(()); }
        Ok(unsafe { out.assume_init() })
    }

    unsafe fn with_pre_login<T>(
        &mut self,
        body: impl FnOnce(&mut Self, *mut c_void, *mut c_void) -> Result<T, ()>,
    ) -> Result<T, ()> {
        unsafe {
            with_string(self.services, std::ptr::null_mut(), |error| {
                with_string(self.services, std::ptr::null_mut(), |fail_code| {
                    body(self, error, fail_code)
                })
            })
        }
    }

    unsafe fn pre_login(
        &mut self,
        game: *mut c_void,
        conn: *mut c_void,
        options: *const u32,
        error: *mut c_void,
        fail_code: *mut c_void,
    ) -> Result<(), ()> {
        if unsafe { (self.services.game_pre_login)(game, conn, options, error, fail_code) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    unsafe fn with_join_error<T>(
        &mut self,
        body: impl FnOnce(&mut Self, *mut c_void) -> Result<T, ()>,
    ) -> Result<T, ()> {
        unsafe {
            with_string(self.services, std::ptr::null_mut(), |error| {
                self.join_error = error;
                let result = body(self, error);
                self.join_error = std::ptr::null_mut();
                result
            })
        }
    }

    unsafe fn with_url<T>(
        &mut self,
        text: *const u32,
        body: impl FnOnce(&mut Self, *mut c_void) -> Result<T, ()>,
    ) -> Result<T, ()> {
        unsafe { with_url(self.services, text, |url| body(self, url)) }
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
    text: *const u32,
) -> Result<(), ()> {
    let mut ops = NativeNotifyReceivedTextOps {
        services: s,
        join_error: std::ptr::null_mut(),
    };
    unsafe { notify_received_text_with(&mut ops, level, conn, text) }
}



/// `ULevel::NotifySendingFile(Connection, Guid)`.
///
/// # Safety
/// `level` must be a live `ULevel`.
pub unsafe fn notify_sending_file(s: &Hp2LevelNetServices, level: *mut c_void) -> Result<i32, ()> {
    let lv = level.cast::<ULevel>();
    unsafe {
        let server = !s.netdriver_server_connection((*lv).base.net_driver)?.is_null();
        crate::level_net_owner::file_send_admission(server, |message| {
            if (s.debugf_dev_net)(message.as_ptr()) == 0 { Ok(()) } else { Err(()) }
        }).map(i32::from)
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
        if (s.app_errorf)(lit!("Level received unexpected file").as_ptr()) != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
}

struct NativeAcceptChannelOps<'a>(&'a Hp2LevelNetServices);

impl crate::level_net_owner::AcceptChannelOps for NativeAcceptChannelOps<'_> {
    type Error = ();
    fn check(&mut self, valid: bool, expression: &CStr, line: i32) -> Result<(), ()> {
        if check(self.0, expression, line, !valid) { Err(()) } else { Ok(()) }
    }
    unsafe fn with_full_name<T>(
        &mut self,
        object: *mut UObject,
        body: impl FnOnce(&mut Self, *const u32) -> Result<T, ()>,
    ) -> Result<T, ()> {
        // SAFETY: the common policy passes a native level and checks thunk status.
        let source = unsafe { value!(self.0.uobject_get_full_name, object.cast()) };
        body(self, source)
    }
    fn log(&mut self, message: &[u32]) -> Result<(), ()> {
        // SAFETY: fmt supplies a live NUL-terminated native TCHAR buffer.
        unsafe {
            if (self.0.debugf_dev_net)(message.as_ptr()) == 0 { Ok(()) } else { Err(()) }
        }
    }
}
