//! Checked `UGameEngine::Exec` policy. Pristine native ordering is retained from
//! UnGame.cpp:410–637 at 71dba98; the frozen SDK baseline already used Rust.
//!
//! Control flow and command parsing live here. Engine objects, FString/FURL
//! work, virtual calls, qualified base calls, and the C++ parse helpers are
//! reached through `Hp2GameExecServices` (`Hp2GameExecServices.cpp`). Each
//! branch keeps the original evaluation order, including the `ParseCommand`
//! side effects on the cursor.
//!
//! The existing LOADGAME repair intentionally differs from that pristine source:
//! missing/busy levels return early and appFormatLoadGameURL supplies the saved-slot
//! travel URL rather than handing a filesystem path to ServerTravel. Keep that fix.
//! Native FString/FURL continuations preserve stack/full-expression lifetimes.
//! Close rereads the complete live level/pending/driver/connection chain for FlushNet.
//! CANCEL deliberately retains InCancel=1 on failure, matching the native static.

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};

/// `ETravelType::TRAVEL_Absolute` (`EngineClasses.h:403`).
const TRAVEL_ABSOLUTE: i32 = 0;
/// `ETravelType::TRAVEL_Partial` (`EngineClasses.h:403`).
const TRAVEL_PARTIAL: i32 = 1;
/// `TCHAR LoadURL[32]` in the LOADGAME branch.
const LOAD_URL_LEN: usize = 32;
/// `TCHAR Filename[256]` in the SAVEGAME branch.
const FILENAME_LEN: usize = 256;
/// `TCHAR ClassName[256]` in the NM_Client SET check.
const CLASS_NAME_LEN: usize = 256;

/// `static UBOOL InCancel` inside the CANCEL branch (function-static, so
/// process-wide).
static IN_CANCEL: AtomicI32 = AtomicI32::new(0);

/// Checked native primitives for UGameEngine::Exec. This is a clean ABI cutover;
/// no unchecked legacy table is retained.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2GameExecServices {
    /// `GIsServer` as `UBOOL`.
    pub is_server: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GIsClient` as `UBOOL`.
    pub is_client: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GLog` as `FOutputDevice*`.
    pub glog: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `GLevel` (`ULevel*`) of the engine.
    pub engine_level: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `GPendingLevel` (`UPendingLevel*`) of the engine.
    pub engine_pending: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Client` (`UClient*`) of the engine.
    pub engine_client: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `CurrentTickRate`.
    pub engine_current_tick_rate: unsafe extern "C" fn(engine: *mut c_void, out: *mut f32) -> i32,
    /// `GetMaxTickRate()` (virtual).
    pub engine_get_max_tick_rate: unsafe extern "C" fn(engine: *mut c_void, out: *mut f32) -> i32,
    /// `SetProgress(Str1, Str2, Seconds)` (virtual on `UGameEngine`).
    pub engine_set_progress: unsafe extern "C" fn(
        engine: *mut c_void,
        str1: *const u32,
        str2: *const u32,
        seconds: f32,
    ) -> i32,
    /// `CancelPending()` (virtual).
    pub engine_cancel_pending: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `SaveGame(Position)` (virtual).
    pub engine_save_game: unsafe extern "C" fn(engine: *mut c_void, position: i32) -> i32,
    /// `Exec(Cmd, Ar)` on `this` (virtual dispatch).
    pub engine_exec: unsafe extern "C" fn(
        engine: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `UEngine::Exec(Cmd, Ar)` (qualified, non-virtual base call).
    pub engine_exec_base: unsafe extern "C" fn(
        engine: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// Native stack FString lifetime surrounding the entire travel command branch.
    pub travel_scope: unsafe extern "C" fn(
        engine: *mut c_void,
        ar: *mut c_void,
        text: *const u32,
        branch: u32,
        out: *mut i32,
    ) -> i32,
    /// Native FURL full-expression lifetime, including the short-circuit error test.
    pub url_scope: unsafe extern "C" fn(
        engine: *mut c_void,
        text: *const u32,
        travel: i32,
        copy_last: i32,
        error: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `Error == TEXT("")` as `UBOOL` (1 when the error string is empty).
    pub browse_error_is_empty: unsafe extern "C" fn(error: *const c_void, out: *mut i32) -> i32,
    /// `*Error`: NUL-terminated text of the error string.
    pub browse_error_text: unsafe extern "C" fn(error: *const c_void, out: *mut *const u32) -> i32,
    /// `Ar.Logf(Fmt, Arg)` with one string argument.
    pub output_logf_s:
        unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, arg: *const u32) -> i32,
    /// `SetClientTravel(Viewport, URL, 0, Travel)`.
    pub engine_set_client_travel: unsafe extern "C" fn(
        engine: *mut c_void,
        viewport: *mut c_void,
        url: *const u32,
        travel: i32,
    ) -> i32,
    /// `SetClientTravel(Viewport, *LastURL.String(), 0, Travel)`.
    pub engine_set_client_travel_last_url:
        unsafe extern "C" fn(engine: *mut c_void, viewport: *mut c_void, travel: i32) -> i32,
    /// `Client->Viewports.Num()`.
    pub client_viewport_count: unsafe extern "C" fn(client: *mut c_void, out: *mut i32) -> i32,
    /// `Client->Viewports(0)` as `UViewport*`.
    pub client_viewport0: unsafe extern "C" fn(client: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::NetDriver`.
    pub level_net_driver: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::GetLevelInfo()` as `ALevelInfo*`.
    pub level_info: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::CleanupDestroyed(1)`.
    pub level_cleanup_destroyed: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `ULevel::SavePersistentActors(GetLevelInfo()->LevelEnterText)`.
    pub level_save_persistent_actors: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `ULevel::Exec(Cmd, Ar)` (virtual).
    pub level_exec: unsafe extern "C" fn(
        level: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `ALevelInfo::NextURL != TEXT("")`.
    pub level_info_next_url_nonempty: unsafe extern "C" fn(info: *mut c_void, out: *mut i32) -> i32,
    /// `ALevelInfo::NetMode == NM_Client`.
    pub level_info_is_net_client: unsafe extern "C" fn(info: *mut c_void, out: *mut i32) -> i32,
    /// `ALevelInfo::Game` as `AGameInfo*`.
    pub level_info_game: unsafe extern "C" fn(info: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ALevelInfo::eventServerTravel(URL, 0)`.
    pub level_info_server_travel: unsafe extern "C" fn(info: *mut c_void, url: *const u32) -> i32,
    /// `AActor::eventBroadcastMessage(Msg, 1, NAME_None)`.
    pub level_info_broadcast_message:
        unsafe extern "C" fn(info: *mut c_void, msg: *const u32) -> i32,
    /// `AGameInfo::ScriptConsoleExec(Cmd, Ar, NULL)`.
    pub game_script_console_exec: unsafe extern "C" fn(
        game: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `UPendingLevel::NetDriver`.
    pub pending_net_driver:
        unsafe extern "C" fn(pending: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UPendingLevel::TrySkipFile()` (virtual).
    pub pending_try_skip_file: unsafe extern "C" fn(pending: *mut c_void, out: *mut i32) -> i32,
    /// `UNetDriver::ServerConnection`.
    pub net_driver_server_connection:
        unsafe extern "C" fn(driver: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UNetConnection::Channels[0]`.
    pub connection_channel0:
        unsafe extern "C" fn(connection: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UChannel::Close()`.
    pub channel_close: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    /// `UNetConnection::FlushNet()`.
    pub connection_flush_net: unsafe extern "C" fn(connection: *mut c_void) -> i32,
    /// `FindObject<UClass>(ANY_PACKAGE, name)`; null when absent.
    pub find_class_any_package:
        unsafe extern "C" fn(name: *const u32, out: *mut *mut c_void) -> i32,
    /// `UClass::IsChildOf(AActor::StaticClass())` as `UBOOL`.
    pub class_is_child_of_actor: unsafe extern "C" fn(cls: *mut c_void, out: *mut i32) -> i32,
    /// `UClass::IsChildOf(AGameInfo::StaticClass())` as `UBOOL`.
    pub class_is_child_of_game_info: unsafe extern "C" fn(cls: *mut c_void, out: *mut i32) -> i32,
    /// `ParseCommand(Stream, Match)` (advances `*stream`).
    pub parse_command:
        unsafe extern "C" fn(stream: *mut *const u32, m: *const u32, out: *mut i32) -> i32,
    /// `ParseToken(Stream, Out, MaxLen, UseEscape)` (advances `*stream`).
    pub parse_token: unsafe extern "C" fn(
        stream: *mut *const u32,
        out: *mut u32,
        max_len: i32,
        use_escape: i32,
        result_out: *mut i32,
    ) -> i32,
    /// `appAtoi(S)`.
    pub app_atoi: unsafe extern "C" fn(s: *const u32, out: *mut i32) -> i32,
    /// `appRequestExit(Force)`.
    pub app_request_exit: unsafe extern "C" fn(force: i32) -> i32,
    /// `appLaunchURL(URL, Parms, &Error)`; the error text is discarded.
    pub app_launch_url: unsafe extern "C" fn(url: *const u32, parms: *const u32) -> i32,
    /// `LocalizeProgress(Key)` (ANSI key overload).
    pub localize_progress: unsafe extern "C" fn(key: *const c_char, out: *mut *const u32) -> i32,
    /// One live GFileManager->MakeDirectory operation, SavePath (0) or SaveSlotPath (1).
    pub make_save_directory: unsafe extern "C" fn(slot: i32) -> i32,
    /// `appSprintf(out, "%s" PATH_SEPARATOR "Save%i.usa", *GSys->SaveSlotPath, slot)`.
    pub save_slot_filename: unsafe extern "C" fn(slot: i32, out: *mut u32) -> i32,
    /// `debugf(NAME_Log, fmt, value)` with an `%i` argument.
    pub debugf_log_i: unsafe extern "C" fn(fmt: *const u32, value: i32) -> i32,
    /// `debugf(NAME_Log, fmt, value)` with a `%s` argument.
    pub debugf_log_s: unsafe extern "C" fn(fmt: *const u32, value: *const u32) -> i32,
    /// `LoadPersistentActorCache()`.
    pub load_persistent_actor_cache: unsafe extern "C" fn() -> i32,
    /// `appFormatLoadGameURL(slot, out, capacity)` as `UBOOL`.
    pub format_load_game_url:
        unsafe extern "C" fn(slot: i32, out: *mut u32, capacity: i32, result_out: *mut i32) -> i32,
    /// `Ar.Log(Text)`.
    pub output_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    /// `Ar.Logf(Fmt, Value)` with one `FLOAT` argument.
    pub output_logf_f: unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, value: f32) -> i32,
}

// Status is checked before the out slot is read. No native state is borrowed.
macro_rules! checked {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { ($f)($($arg,)* out.as_mut_ptr()) };
        if status != 0 { return Err(status); }
        unsafe { out.assume_init() }
    }};
}
macro_rules! checked_void {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let status = unsafe { ($f)($($arg),*) };
        if status != 0 { return Err(status); }
    }};
}

/// Literal `TEXT("...")` as a static NUL-terminated `u32` (TCHAR) array.
macro_rules! w {
    ($s:literal) => {{
        const N: usize = $s.len() + 1;
        static WIDE: [u32; N] = wide::<N>($s);
        WIDE.as_ptr()
    }};
}

const fn wide<const N: usize>(s: &str) -> [u32; N] {
    let bytes = s.as_bytes();
    let mut out = [0u32; N];
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i] as u32;
        i += 1;
    }
    out
}

/// `appIsDigit(TCHAR c)` (`UnFile.h:300`).
fn app_is_digit(c: u32) -> bool {
    let c = c as i32;
    c >= b'0' as i32 && c <= b'9' as i32
}

/// `Client && Client->Viewports.Num()`.
unsafe fn has_client_viewports(s: &Hp2GameExecServices, engine: *mut c_void) -> Result<bool, i32> {
    unsafe {
        let client = checked!(s.engine_client, engine);
        Ok(!client.is_null() && checked!(s.client_viewport_count, client) != 0)
    }
}

/// `Client->Viewports(0)`. The caller has checked `has_client_viewports`.
unsafe fn client_viewport0(
    s: &Hp2GameExecServices,
    engine: *mut c_void,
) -> Result<*mut c_void, i32> {
    unsafe {
        Ok(checked!(
            s.client_viewport0,
            checked!(s.engine_client, engine)
        ))
    }
}

/// Close the control channel, then reread the complete native root chain before FlushNet.
/// Close can replace GLevel/GPendingLevel, their driver, or ServerConnection.
unsafe fn close_server_channel(
    s: &Hp2GameExecServices,
    engine: *mut c_void,
    pending: bool,
) -> Result<(), i32> {
    unsafe {
        let root = if pending {
            checked!(s.engine_pending, engine)
        } else {
            checked!(s.engine_level, engine)
        };
        if root.is_null() {
            return Ok(());
        }
        let driver = if pending {
            checked!(s.pending_net_driver, root)
        } else {
            checked!(s.level_net_driver, root)
        };
        if driver.is_null() {
            return Ok(());
        }
        let connection = checked!(s.net_driver_server_connection, driver);
        if connection.is_null() {
            return Ok(());
        }
        let channel = checked!(s.connection_channel0, connection);
        if channel.is_null() {
            return Ok(());
        }
        checked_void!(s.channel_close, channel);
        let root = if pending {
            checked!(s.engine_pending, engine)
        } else {
            checked!(s.engine_level, engine)
        };
        if root.is_null() {
            return Ok(());
        }
        let driver = if pending {
            checked!(s.pending_net_driver, root)
        } else {
            checked!(s.level_net_driver, root)
        };
        if driver.is_null() {
            return Ok(());
        }
        let connection = checked!(s.net_driver_server_connection, driver);
        if connection.is_null() {
            return Ok(());
        }
        checked_void!(s.connection_flush_net, connection);
        Ok(())
    }
}

/// Called with a native FString Error alive, before testing Client.
/// # Safety
/// Handles and the immutable table must remain live for the continuation.
pub unsafe fn travel(
    s: &Hp2GameExecServices,
    engine: *mut c_void,
    ar: *mut c_void,
    text: *const u32,
    branch: u32,
    error: *mut c_void,
) -> Result<i32, i32> {
    unsafe {
        let (url, travel, copy_last, fmt) = match branch {
            0 => (text, TRAVEL_PARTIAL, 0, w!("Open failed: %s")),
            1 => (text, TRAVEL_ABSOLUTE, 0, w!("Start failed: %s")),
            2 => (
                w!("?failed"),
                TRAVEL_ABSOLUTE,
                0,
                w!("Disconnect failed: %s"),
            ),
            3 => (text, TRAVEL_ABSOLUTE, 1, w!("Reconnect failed: %s")),
            _ => return Err(1),
        };
        if has_client_viewports(s, engine)? {
            if branch == 2 {
                close_server_channel(s, engine, false)?;
                close_server_channel(s, engine, true)?;
            }
            if copy_last != 0 {
                checked_void!(
                    s.engine_set_client_travel_last_url,
                    engine,
                    client_viewport0(s, engine)?,
                    travel
                );
            } else {
                checked_void!(
                    s.engine_set_client_travel,
                    engine,
                    client_viewport0(s, engine)?,
                    url,
                    travel
                );
            }
        } else {
            let should_log = checked!(s.url_scope, engine, url, travel, copy_last, error);
            if should_log != 0 {
                checked_void!(
                    s.output_logf_s,
                    ar,
                    fmt,
                    checked!(s.browse_error_text, error)
                );
            }
        }
        Ok(1)
    }
}

/// FURL is destroyed after the complete Browse/error-test condition, before output.
/// # Safety
/// error is a live native FString. The by-value Browse parameter remains alive
/// throughout this callback, without an additional FURL copy.
pub unsafe fn browse_condition(
    s: &Hp2GameExecServices,
    browse_result: i32,
    error: *mut c_void,
) -> Result<i32, i32> {
    unsafe { Ok((browse_result == 0 && checked!(s.browse_error_is_empty, error) == 0) as i32) }
}

/// `GIsServer && !GIsClient`.
unsafe fn is_server_only(s: &Hp2GameExecServices) -> Result<bool, i32> {
    unsafe { Ok(checked!(s.is_server) != 0 && checked!(s.is_client) == 0) }
}
/// Cursor ParseCommand uses in the original GameEngine policy.
#[derive(Clone, Copy)]
pub(crate) enum ExecCursor {
    Command,
    Tail,
}

/// Command actions reached by the single shared GameEngine Exec policy.
#[derive(Clone, Copy)]
pub(crate) enum ExecAction {
    Open,
    Start,
    ServerTravel,
    Say,
    Disconnect,
    Reconnect,
    Exit,
    CurrentTickRate,
    MaxTickRate,
    GSpyLite,
    ObjectClean,
    SaveGame,
    LoadGame,
    SavePersistentActors,
    Cancel,
    LevelExec,
    GameInfoExec,
    RestrictSet,
    QualifiedEngineExec,
}

/// Result from one source-ordered operation: return, continue fallback, or allow Engine.Exec.
pub(crate) enum ExecStep {
    Return(bool),
    Continue,
    Allow,
}

/// Leaves used by both native and Core GameEngine Exec adapters.
pub(crate) trait ExecOps {
    type Command;
    type Error;

    unsafe fn parse_command(
        &mut self,
        command: &mut Self::Command,
        cursor: ExecCursor,
        keyword: &[u32],
    ) -> Result<bool, Self::Error>;
    unsafe fn is_server_only(&mut self) -> Result<bool, Self::Error>;
    unsafe fn action(
        &mut self,
        action: ExecAction,
        engine: *mut c_void,
        command: &mut Self::Command,
    ) -> Result<ExecStep, Self::Error>;
}

/// Runs the complete source-ordered `UGameEngine::Exec` command policy.
///
/// Branch leaves are supplied by an adapter; parsing and dispatch order are shared.
///
/// # Safety
/// Engine and command-owned records must remain valid through synchronous reentry.
pub(crate) unsafe fn exec_with_ops<O: ExecOps>(
    ops: &mut O,
    engine: *mut c_void,
    command: &mut O::Command,
) -> Result<bool, O::Error> {
    use ExecAction::*;
    use ExecCursor::*;

    macro_rules! parsed {
        ($cursor:ident, $word:literal, $action:ident) => {
            if unsafe { ops.parse_command(command, $cursor, hp2_core::lit!($word))? } {
                return match unsafe { ops.action($action, engine, command)? } {
                    ExecStep::Return(value) => Ok(value),
                    _ => unreachable!("recognized command leaves must return"),
                };
            }
        };
    }

    parsed!(Tail, "OPEN", Open);
    parsed!(Tail, "START", Start);

    if unsafe { ops.parse_command(command, Tail, hp2_core::lit!("SERVERTRAVEL"))? }
        && unsafe { ops.is_server_only()? }
    {
        return match unsafe { ops.action(ServerTravel, engine, command)? } {
            ExecStep::Return(value) => Ok(value),
            _ => unreachable!("recognized command leaves must return"),
        };
    }
    if unsafe { ops.is_server_only()? }
        && unsafe { ops.parse_command(command, Tail, hp2_core::lit!("SAY"))? }
    {
        return match unsafe { ops.action(Say, engine, command)? } {
            ExecStep::Return(value) => Ok(value),
            _ => unreachable!("recognized command leaves must return"),
        };
    }

    parsed!(Tail, "DISCONNECT", Disconnect);
    parsed!(Tail, "RECONNECT", Reconnect);

    if unsafe { ops.parse_command(command, Command, hp2_core::lit!("EXIT"))? }
        || unsafe { ops.parse_command(command, Command, hp2_core::lit!("QUIT"))? }
    {
        return match unsafe { ops.action(Exit, engine, command)? } {
            ExecStep::Return(value) => Ok(value),
            _ => unreachable!("recognized command leaves must return"),
        };
    }

    parsed!(Tail, "GETCURRENTTICKRATE", CurrentTickRate);
    parsed!(Tail, "GETMAXTICKRATE", MaxTickRate);
    parsed!(Tail, "GSPYLITE", GSpyLite);
    parsed!(Tail, "OBJCLEAN", ObjectClean);
    parsed!(Tail, "SAVEGAME", SaveGame);
    parsed!(Tail, "LOADGAME", LoadGame);
    parsed!(Tail, "SAVEPACTORS", SavePersistentActors);

    if unsafe { ops.parse_command(command, Command, hp2_core::lit!("CANCEL"))? } {
        return match unsafe { ops.action(Cancel, engine, command)? } {
            ExecStep::Return(value) => Ok(value),
            _ => unreachable!("recognized command leaves must return"),
        };
    }

    match unsafe { ops.action(LevelExec, engine, command)? } {
        ExecStep::Return(true) => return Ok(true),
        ExecStep::Return(false) | ExecStep::Continue => {}
        ExecStep::Allow => unreachable!("Level.Exec cannot allow Engine.Exec"),
    }
    match unsafe { ops.action(GameInfoExec, engine, command)? } {
        ExecStep::Return(true) => return Ok(true),
        ExecStep::Return(false) | ExecStep::Continue => {}
        ExecStep::Allow => unreachable!("GameInfo Exec cannot allow Engine.Exec"),
    }
    match unsafe { ops.action(RestrictSet, engine, command)? } {
        ExecStep::Return(value) => return Ok(value),
        ExecStep::Continue | ExecStep::Allow => {}
    }
    match unsafe { ops.action(QualifiedEngineExec, engine, command)? } {
        ExecStep::Return(value) => Ok(value),
        _ => unreachable!("qualified Engine.Exec must return"),
    }
}
struct NativeExecCommand {
    cmd: *const u32,
    tail: *const u32,
    output: *mut c_void,
}

struct NativeExecOps<'a>(&'a Hp2GameExecServices);

impl ExecOps for NativeExecOps<'_> {
    type Command = NativeExecCommand;
    type Error = i32;

    unsafe fn parse_command(
        &mut self,
        command: &mut Self::Command,
        cursor: ExecCursor,
        keyword: &[u32],
    ) -> Result<bool, i32> {
        let stream = match cursor {
            ExecCursor::Command => &mut command.cmd,
            ExecCursor::Tail => &mut command.tail,
        };
        Ok(unsafe { checked!(self.0.parse_command, stream, keyword.as_ptr()) != 0 })
    }

    unsafe fn is_server_only(&mut self) -> Result<bool, i32> {
        unsafe { is_server_only(self.0) }
    }

    unsafe fn action(
        &mut self,
        action: ExecAction,
        engine: *mut c_void,
        command: &mut Self::Command,
    ) -> Result<ExecStep, i32> {
        use ExecAction::*;
        unsafe {
            let s = self.0;
            let value = match action {
                Open | Start | Disconnect | Reconnect => {
                    let branch = match action { Open => 0, Start => 1, Disconnect => 2, _ => 3 };
                    let handled = checked!(s.travel_scope, engine, command.output, command.tail, branch);
                    return Ok(ExecStep::Return(handled != 0));
                }
                ServerTravel => {
                    let info = checked!(s.level_info, checked!(s.engine_level, engine));
                    checked_void!(s.level_info_server_travel, info, command.tail);
                    1
                }
                Say => {
                    let info = checked!(s.level_info, checked!(s.engine_level, engine));
                    checked_void!(s.level_info_broadcast_message, info, command.tail);
                    1
                }
                Exit => {
                    close_server_channel(s, engine, false)?;
                    close_server_channel(s, engine, true)?;
                    checked_void!(s.output_log, command.output, w!("Closing by request"));
                    checked_void!(s.app_request_exit, 0);
                    1
                }
                CurrentTickRate => {
                    checked_void!(s.output_logf_f, command.output, w!("%f"),
                        checked!(s.engine_current_tick_rate, engine));
                    1
                }
                MaxTickRate => {
                    checked_void!(s.output_logf_f, command.output, w!("%f"),
                        checked!(s.engine_get_max_tick_rate, engine));
                    1
                }
                GSpyLite => {
                    checked_void!(s.app_launch_url, w!("GSpyLite.exe"), w!(""));
                    1
                }
                ObjectClean => {
                    let level = checked!(s.engine_level, engine);
                    if !level.is_null() { checked_void!(s.level_cleanup_destroyed, level); }
                    1
                }
                SaveGame => {
                    if app_is_digit(*command.tail) {
                        checked_void!(s.make_save_directory, 0);
                        checked_void!(s.make_save_directory, 1);
                        let slot = checked!(s.app_atoi, command.tail);
                        let mut filename = [0u32; FILENAME_LEN];
                        checked_void!(s.save_slot_filename, slot, filename.as_mut_ptr());
                        checked_void!(s.debugf_log_s, w!("Saving game...(before call) Filename: %s"), filename.as_ptr());
                        checked_void!(s.engine_save_game, engine, checked!(s.app_atoi, command.tail));
                        checked!(s.engine_exec, engine, w!("RestartSounds"), checked!(s.glog));
                    }
                    1
                }
                LoadGame => {
                    if app_is_digit(*command.tail) {
                        let level = checked!(s.engine_level, engine);
                        if !level.is_null()
                            && checked!(s.level_info_next_url_nonempty, checked!(s.level_info, level)) == 0
                        {
                            checked_void!(s.load_persistent_actor_cache);
                            let slot = checked!(s.app_atoi, command.tail);
                            checked_void!(s.make_save_directory, 0);
                            checked_void!(s.make_save_directory, 1);
                            checked_void!(s.debugf_log_i, w!("Loading game slot %i"), slot);
                            let mut url = [0u32; LOAD_URL_LEN];
                            if checked!(s.format_load_game_url, slot, url.as_mut_ptr(), LOAD_URL_LEN as i32) != 0 {
                                let info = checked!(s.level_info, checked!(s.engine_level, engine));
                                checked_void!(s.level_info_server_travel, info, url.as_ptr());
                            }
                        }
                    }
                    1
                }
                SavePersistentActors => {
                    let level = checked!(s.engine_level, engine);
                    if level.is_null() { return Ok(ExecStep::Return(false)); }
                    checked_void!(s.make_save_directory, 0);
                    checked_void!(s.make_save_directory, 1);
                    checked_void!(s.level_save_persistent_actors, level);
                    1
                }
                Cancel => {
                    if IN_CANCEL.load(Ordering::Relaxed) == 0 {
                        IN_CANCEL.store(1, Ordering::Relaxed);
                        let pending = checked!(s.engine_pending, engine);
                        if !pending.is_null() {
                            if checked!(s.pending_try_skip_file, pending) != 0 {
                                IN_CANCEL.store(0, Ordering::Relaxed);
                                return Ok(ExecStep::Return(true));
                            }
                            checked_void!(s.engine_set_progress, engine,
                                checked!(s.localize_progress, c"CancelledConnect".as_ptr()),
                                w!(""), 2.0);
                        } else {
                            checked_void!(s.engine_set_progress, engine, w!(""), w!(""), 0.0);
                        }
                        checked_void!(s.engine_cancel_pending, engine);
                        IN_CANCEL.store(0, Ordering::Relaxed);
                    }
                    1
                }
                LevelExec => {
                    let level = checked!(s.engine_level, engine);
                    let handled = !level.is_null() && checked!(s.level_exec, level, command.cmd, command.output) != 0;
                    return Ok(if handled { ExecStep::Return(true) } else { ExecStep::Continue });
                }
                GameInfoExec => {
                    let level = checked!(s.engine_level, engine);
                    let handled = if level.is_null() { false } else {
                        let game = checked!(s.level_info_game, checked!(s.level_info, level));
                        !game.is_null() && checked!(s.game_script_console_exec, game, command.cmd, command.output) != 0
                    };
                    return Ok(if handled { ExecStep::Return(true) } else { ExecStep::Continue });
                }
                RestrictSet => {
                    let mut allow = true;
                    let level = checked!(s.engine_level, engine);
                    if !level.is_null() && checked!(s.level_info_is_net_client, checked!(s.level_info, level)) != 0 {
                        let mut tail = command.cmd;
                        if checked!(s.parse_command, &mut tail, w!("SET")) != 0 {
                            let mut name = [0u32; CLASS_NAME_LEN];
                            let cls = if checked!(s.parse_token, &mut tail, name.as_mut_ptr(), CLASS_NAME_LEN as i32, 1) != 0 {
                                checked!(s.find_class_any_package, name.as_ptr())
                            } else { ptr::null_mut() };
                            if !cls.is_null() && checked!(s.class_is_child_of_actor, cls) != 0
                                && checked!(s.class_is_child_of_game_info, cls) == 0 { allow = false; }
                        }
                    }
                    return Ok(if allow { ExecStep::Allow } else { ExecStep::Return(false) });
                }
                QualifiedEngineExec => {
                    i32::from(checked!(s.engine_exec_base, engine, command.cmd, command.output) != 0)
                }
            };
            Ok(ExecStep::Return(value != 0))
        }
    }
}

/// `UGameEngine::Exec(const TCHAR* Cmd, FOutputDevice& Ar)`. Returns `UBOOL`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`, `cmd` a NUL-terminated TCHAR string,
/// and `ar` a live `FOutputDevice*`.
pub unsafe fn exec(
    s: &Hp2GameExecServices,
    engine: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) -> Result<i32, i32> {
    let mut command = NativeExecCommand { cmd, tail: cmd, output: ar };
    let handled = unsafe { exec_with_ops(&mut NativeExecOps(s), engine, &mut command)? };
    Ok(i32::from(handled))
}
