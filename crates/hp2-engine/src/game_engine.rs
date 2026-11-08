//! Port of `HarryPotter2/Unreal/Engine/Src/UnGame.cpp` (`UGameEngine`).
//!
//! The C++ shells in `UnGame.cpp` keep the UObject boilerplate, the
//! `guard`/`unguard` wrappers and the qualified base calls, and forward here.
//! Engine fields (`GLevel`, `GPendingLevel`, `Client`, `Replay`, ...) are
//! members of `UGameEngine`, so every accessor takes the engine handle. Field
//! reads and writes go through [`Hp2GameServices`], filled by
//! `Hp2GameServices.cpp`, so this module reads no engine layout.
//!
//! Every callback returns native status (zero means success). Rust propagates
//! failure without unwinding; the unchanged C++ caller rethrows inside its guard.

use std::ffi::{CStr, c_char, c_void};

/// `IST_Press` (`EngineClasses.h:397`).
const IST_PRESS: i32 = 1;
/// `IST_MAX` (`EngineClasses.h:401`). Marks a special input record.
const IST_MAX: i32 = 5;
/// `IK_Pause` (`EngineClasses.h:150`).
const IK_PAUSE: i32 = 19;
/// `IK_Escape` (`EngineClasses.h:158`).
const IK_ESCAPE: i32 = 27;
/// `IK_Delete` (`EngineClasses.h:177`).
const IK_DELETE: i32 = 46;
/// `IK_ScrollLock` (`EngineClasses.h:276`).
const IK_SCROLL_LOCK: i32 = 145;
/// `IK_Backslash` (`EngineClasses.h:351`).
const IK_BACKSLASH: i32 = 220;
/// `IK_MouseX` (`EngineClasses.h:359`).
const IK_MOUSE_X: i32 = 228;
/// `IK_MouseY` (`EngineClasses.h:360`).
const IK_MOUSE_Y: i32 = 229;
/// `MOUSE_FirstHit` (`UnCamera.h:195`).
const MOUSE_FIRST_HIT: u32 = 0x0008;
/// `MOUSE_LastRelease` (`UnCamera.h:196`).
const MOUSE_LAST_RELEASE: u32 = 0x0010;

/// C++ services for `UGameEngine`, filled by `Hp2GameServices.cpp`.
/// Handles are `void*`; `UBOOL` is `i32`. Accessors that take `engine` read
/// or write the member of that `UGameEngine`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2GameServices {
    // --- UGameEngine members ---
    /// `GLevel`, as `ULevel*`.
    pub level: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `GLevel = v`.
    pub set_level: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void) -> i32,
    /// `GPendingLevel`, as `UPendingLevel*`.
    pub pending: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `GPendingLevel = v`.
    pub set_pending: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void) -> i32,
    /// `Client`, as `UClient*`.
    pub client: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,

    // --- UClient ---
    /// `Client->Viewports.Num()`. Dereferences `client` unguarded, as the C++ does.
    pub client_viewport_count: unsafe extern "C" fn(client: *mut c_void, out: *mut i32) -> i32,
    /// `Client->Viewports(0)`. Dereferences `client` unguarded.
    pub client_viewport0: unsafe extern "C" fn(client: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Client->CaptureMouse`. Returns `UBOOL`.
    pub client_capture_mouse: unsafe extern "C" fn(client: *mut c_void, out: *mut i32) -> i32,

    // --- UViewport ---
    /// `Viewport->Actor`, as `APlayerPawn*`.
    pub viewport_actor: unsafe extern "C" fn(viewport: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Viewport->bShowWindowsMouse`. Returns `UBOOL`.
    pub viewport_show_windows_mouse:
        unsafe extern "C" fn(viewport: *mut c_void, out: *mut i32) -> i32,
    /// `Viewport->SetMouseCapture(capture, exclusive, foreground)`.
    pub viewport_set_mouse_capture: unsafe extern "C" fn(
        viewport: *mut c_void,
        capture: i32,
        exclusive: i32,
        foreground: i32,
    ) -> i32,
    /// `Viewport->WindowsMouseX = x`, `WindowsMouseY = y`.
    pub viewport_set_windows_mouse:
        unsafe extern "C" fn(viewport: *mut c_void, x: f32, y: f32) -> i32,

    // --- ULevel and ULevelInfo ---
    /// `ULevel::NetDriver`. Dereferences `level` unguarded, as the C++ does.
    pub level_net_driver: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::NetDriver = v`.
    pub level_set_net_driver: unsafe extern "C" fn(level: *mut c_void, v: *mut c_void) -> i32,
    /// `ULevel::GetLevelInfo()`, as `ULevelInfo*`.
    pub level_info: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevelInfo::Pauser == TEXT("")`. Returns `UBOOL`.
    pub level_info_pauser_empty: unsafe extern "C" fn(info: *mut c_void, out: *mut i32) -> i32,
    /// `ULevelInfo::bInSpecialPauseMode = !bInSpecialPauseMode`.
    /// Dereferences unguarded, as the C++ does.
    pub level_toggle_special_pause: unsafe extern "C" fn(level: *mut c_void) -> i32,

    // --- APlayerPawn ---
    /// `Actor->bShowMenu`. Returns `UBOOL`.
    pub pawn_show_menu: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `Actor->eventShowUpgradeMenu()`.
    pub pawn_show_upgrade_menu: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Actor->ProgressMessage[slot] = text` and `ProgressColor[slot] = (255,255,255)`.
    pub pawn_set_progress:
        unsafe extern "C" fn(actor: *mut c_void, slot: i32, text: *const u32) -> i32,
    /// `Actor->ProgressTimeOut = Actor->Level->TimeSeconds + seconds`.
    pub pawn_set_progress_timeout: unsafe extern "C" fn(actor: *mut c_void, seconds: f32) -> i32,

    // --- Net objects (UNetDriver, UNetConnection, UChannel, UPendingLevel) ---
    /// `delete driver` (virtual destructor) on a `UNetDriver`.
    pub net_driver_delete: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    /// `UPendingLevel::NetDriver`.
    pub pending_net_driver:
        unsafe extern "C" fn(pending: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UNetDriver::ServerConnection`.
    pub net_driver_server_connection:
        unsafe extern "C" fn(driver: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UNetConnection::Channels[0]`.
    pub connection_channel0: unsafe extern "C" fn(conn: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UChannel::Close()`.
    pub channel_close: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    /// `UNetConnection::FlushNet()`.
    pub connection_flush_net: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    /// `delete pending` (virtual destructor) on a `UPendingLevel`.
    pub delete_pending: unsafe extern "C" fn(pending: *mut c_void) -> i32,

    // --- Qualified base calls and log lines ---
    /// `UEngine::Exit()`, qualified base call.
    pub engine_exit_base: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::Destroy()`, qualified base call (`Super::Destroy()`).
    pub engine_destroy_base: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::Key(Viewport, Key)`, qualified base call. Returns `UBOOL`.
    pub engine_key_base: unsafe extern "C" fn(
        engine: *mut c_void,
        viewport: *mut c_void,
        key: i32,
        out: *mut i32,
    ) -> i32,
    /// `UEngine::InputEvent(Viewport, iKey, State, Delta)`, qualified base call.
    /// Returns `UBOOL`.
    pub engine_input_event_base: unsafe extern "C" fn(
        engine: *mut c_void,
        viewport: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
        out: *mut i32,
    ) -> i32,
    /// `debugf( NAME_Exit, TEXT("Game engine shut down") )`.
    pub log_game_shut_down: unsafe extern "C" fn() -> i32,

    // --- Replay (`UGameEngine::Replay`, FReplay) ---
    /// `Replay.Recording()`. Returns `UBOOL`.
    pub replay_recording: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `Replay.Replaying()`. Returns `UBOOL`.
    pub replay_replaying: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `Replay.InReplay()`. Returns `UBOOL`.
    pub replay_in_replay: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `Replay.Paused()`. Returns `UBOOL`.
    pub replay_paused: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `Replay.Stop()`.
    pub replay_stop: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `Replay.Pause(on)`.
    pub replay_pause: unsafe extern "C" fn(engine: *mut c_void, on: i32) -> i32,
    /// `Replay.Pause(true, 1)` (single step).
    pub replay_pause_step: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `Replay.SerializeFrameInput(key, state, delta)`. Recording only.
    pub replay_serialize_input:
        unsafe extern "C" fn(engine: *mut c_void, key: i32, state: i32, delta: f32) -> i32,

    // --- Paint progress ---
    /// `GIsDrawing`. The C++ adapter returns the `bool` as `INT` 0/1.
    pub engine_is_drawing: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `FPlane LoadFog(0.f,0.1f,0.25f,0.2f); Exchange(viewport->Actor->FlashFog, LoadFog);`
    /// `Draw(viewport);` then `Exchange` again, as one C++ unit (`Draw` is virtual).
    pub draw_with_load_fog: unsafe extern "C" fn(engine: *mut c_void, viewport: *mut c_void) -> i32,
    /// Virtual `Engine->CancelPending()`, including its original native guard.
    pub engine_cancel_pending: unsafe extern "C" fn(engine: *mut c_void) -> i32,

    // --- UpdateConnectingMessage / SetClientTravel ---
    /// `check(Expr)` failure, replicating `appFailAssert` at `UnGame.cpp`'s original line.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    /// `CastChecked<UViewport>(Player)->TravelURL=NextURL; ->TravelType=TravelType;`
    /// `->bTravelItems=bItems;`
    pub viewport_cast_set_travel: unsafe extern "C" fn(
        player: *mut c_void,
        next_url: *const u32,
        travel_type: i32,
        b_items: i32,
    ) -> i32,
    /// `GPendingLevel->DemoRecDriver`, as `UNetDriver*`.
    pub pending_demo_rec_driver:
        unsafe extern "C" fn(pending: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Actor->ProgressTimeOut < Actor->Level->TimeSeconds`. Returns `UBOOL`.
    pub pawn_progress_expired: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `appSprintf(Msg1,TEXT("")); appSprintf(Msg2,*GPendingLevel->URL.Map);` into
    /// 256-`TCHAR` buffers.
    pub pending_format_demo_message:
        unsafe extern "C" fn(pending: *mut c_void, msg1: *mut u32, msg2: *mut u32) -> i32,
    /// `appSprintf(Msg1,LocalizeProgress("ConnectingText"));`
    /// `appSprintf(Msg2,LocalizeProgress("ConnectingURL"),*GPendingLevel->URL.Host,`
    /// `*GPendingLevel->URL.Map);`
    pub pending_format_connecting_message:
        unsafe extern "C" fn(pending: *mut c_void, msg1: *mut u32, msg2: *mut u32) -> i32,
}

// These macros return immediately on native failure. Outputs are read only after
// successful callbacks; C++ owns the exception and all native object lifetimes.
macro_rules! call {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let status = ($f)($($arg),*);
        if status != 0 { return Err(status); }
    }};
}
macro_rules! value {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        let status = ($f)($($arg,)* out.as_mut_ptr());
        if status != 0 { return Err(status); }
        out.assume_init()
    }};
}

/// `UGameEngine::ChallengeResponse(Challenge)`. Pure integer math.
pub fn challenge_response(challenge: i32) -> i32 {
    challenge.wrapping_mul(237) ^ 0x93fe_92ce_u32 as i32 ^ (challenge >> 16) ^ (challenge << 16)
}

/// Owner-agnostic leaves for the source `UGameEngine::Exit` sequence.
pub(crate) trait GameExitOps {
    type Error;
    fn engine_exit_base(&mut self, engine: *mut c_void) -> Result<(), Self::Error>;
    fn level(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn level_net_driver(&mut self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn delete_net_driver(&mut self, driver: *mut c_void) -> Result<(), Self::Error>;
    fn clear_level_net_driver(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
}

/// Shared `UGameEngine::Exit` policy over native or canonical Engine leaves.
///
/// # Safety
/// `engine` and every object returned by `ops` remain live for this operation.
pub(crate) unsafe fn exit_with_ops<O: GameExitOps>(
    ops: &mut O,
    engine: *mut c_void,
) -> Result<(), O::Error> {
    ops.engine_exit_base(engine)?;
    let level = ops.level(engine)?;
    let driver = ops.level_net_driver(level)?;
    if !driver.is_null() {
        ops.delete_net_driver(driver)?;
        ops.clear_level_net_driver(level)?;
    }
    Ok(())
}

struct NativeGameExitOps<'a>(&'a Hp2GameServices);
impl GameExitOps for NativeGameExitOps<'_> {
    type Error = i32;
    fn engine_exit_base(&mut self, engine: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.engine_exit_base, engine); }
        Ok(())
    }
    fn level(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.level, engine)) }
    }
    fn level_net_driver(&mut self, level: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.level_net_driver, level)) }
    }
    fn delete_net_driver(&mut self, driver: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.net_driver_delete, driver); }
        Ok(())
    }
    fn clear_level_net_driver(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.level_set_net_driver, level, std::ptr::null_mut()); }
        Ok(())
    }
}

/// `UGameEngine::Exit()` after the C++ `guard`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`, and `s` must be a valid table.
pub unsafe fn exit(s: &Hp2GameServices, engine: *mut c_void) -> Result<(), i32> {
    unsafe { exit_with_ops(&mut NativeGameExitOps(s), engine) }
}

pub(crate) trait CancelPendingOps {
    type Error;
    fn pending(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn driver(&mut self, pending: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn server_connection(&mut self, driver: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn channel_zero(&mut self, connection: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn close_channel(&mut self, channel: *mut c_void) -> Result<(), Self::Error>;
    fn flush_connection(&mut self, connection: *mut c_void) -> Result<(), Self::Error>;
    fn delete_pending(&mut self, pending: *mut c_void) -> Result<(), Self::Error>;
    fn set_pending(&mut self, engine: *mut c_void, pending: *mut c_void) -> Result<(), Self::Error>;
}

struct NativeCancelPendingOps<'a>(&'a Hp2GameServices);
impl CancelPendingOps for NativeCancelPendingOps<'_> {
    type Error = i32;
    fn pending(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { value!(self.0.pending, engine) })
    }
    fn driver(&mut self, pending: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { value!(self.0.pending_net_driver, pending) })
    }
    fn server_connection(&mut self, driver: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { value!(self.0.net_driver_server_connection, driver) })
    }
    fn channel_zero(&mut self, connection: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { value!(self.0.connection_channel0, connection) })
    }
    fn close_channel(&mut self, channel: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.channel_close, channel); }
        Ok(())
    }
    fn flush_connection(&mut self, connection: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.connection_flush_net, connection); }
        Ok(())
    }
    fn delete_pending(&mut self, pending: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.delete_pending, pending); }
        Ok(())
    }
    fn set_pending(&mut self, engine: *mut c_void, pending: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.set_pending, engine, pending); }
        Ok(())
    }
}

pub(crate) fn cancel_pending_with_ops<O: CancelPendingOps>(
    engine: *mut c_void, ops: &mut O,
) -> Result<(), O::Error> {
    let pending = ops.pending(engine)?;
    if pending.is_null() { return Ok(()); }
    let driver = ops.driver(pending)?;
    if !driver.is_null() {
        let connection = ops.server_connection(driver)?;
        if !connection.is_null() {
            let channel = ops.channel_zero(connection)?;
            if !channel.is_null() {
                ops.close_channel(channel)?;
                let current = ops.pending(engine)?;
                let driver = ops.driver(current)?;
                let connection = ops.server_connection(driver)?;
                ops.flush_connection(connection)?;
            }
        }
    }
    let current = ops.pending(engine)?;
    ops.delete_pending(current)?;
    ops.set_pending(engine, std::ptr::null_mut())
}

/// `UGameEngine::CancelPending()` after the C++ `guard`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`. `s` must be valid.
pub unsafe fn cancel_pending(s: &Hp2GameServices, engine: *mut c_void) -> Result<(), i32> {
    cancel_pending_with_ops(engine, &mut NativeCancelPendingOps(s))
}

pub(crate) trait GameDestroyOps {
    type Error;
    fn has_pending(&mut self, engine: *mut c_void) -> Result<bool, Self::Error>;
    fn cancel_pending(&mut self, engine: *mut c_void) -> Result<(), Self::Error>;
    fn clear_level(&mut self, engine: *mut c_void) -> Result<(), Self::Error>;
    fn log_shutdown(&mut self) -> Result<(), Self::Error>;
    fn destroy_base(&mut self, engine: *mut c_void) -> Result<(), Self::Error>;
}
impl GameDestroyOps for NativeCancelPendingOps<'_> {
    type Error = i32;
    fn has_pending(&mut self, engine: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { !value!(self.0.pending, engine).is_null() })
    }
    fn cancel_pending(&mut self, engine: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.engine_cancel_pending, engine); }
        Ok(())
    }
    fn clear_level(&mut self, engine: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.set_level, engine, std::ptr::null_mut()); }
        Ok(())
    }
    fn log_shutdown(&mut self) -> Result<(), i32> {
        unsafe { call!(self.0.log_game_shut_down); }
        Ok(())
    }
    fn destroy_base(&mut self, engine: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.engine_destroy_base, engine); }
        Ok(())
    }
}
pub(crate) fn destroy_with_ops<O: GameDestroyOps>(
    engine: *mut c_void, ops: &mut O,
) -> Result<(), O::Error> {
    if ops.has_pending(engine)? { ops.cancel_pending(engine)?; }
    ops.clear_level(engine)?;
    ops.log_shutdown()?;
    ops.destroy_base(engine)
}

/// `UGameEngine::Destroy()` before `Super::Destroy()`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`. `s` must be valid.
pub unsafe fn destroy(s: &Hp2GameServices, engine: *mut c_void) -> Result<(), i32> {
    destroy_with_ops(engine, &mut NativeCancelPendingOps(s))
}

/// `UGameEngine::SetProgress(Str1, Str2, Seconds)`. Strings are NUL-terminated `TCHAR`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`. `s` must be valid.
pub unsafe fn set_progress(
    s: &Hp2GameServices,
    engine: *mut c_void,
    str1: *const u32,
    str2: *const u32,
    seconds: f32,
) -> Result<(), i32> {
    // SAFETY: the native caller retains the GameEngine and its progress strings.
    unsafe { set_progress_with(&mut NativeProgressOps(s), engine, str1, str2, seconds) }
}

/// `UGameEngine::MouseDelta(Viewport, ClickFlags, DX, DY)`. No replay work.
///
/// # Safety
/// `engine` and `viewport` must be live engine objects. `s` must be valid.
pub unsafe fn mouse_delta(
    s: &Hp2GameServices,
    engine: *mut c_void,
    viewport: *mut c_void,
    click_flags: u32,
) -> Result<(), i32> {
    unsafe {
        let client = value!(s.client, engine);
        let level = value!(s.level, engine);
        if click_flags & MOUSE_FIRST_HIT != 0
            && !client.is_null()
            && value!(s.client_viewport_count, client) == 1
            && !level.is_null()
            && value!(s.level_info_pauser_empty, value!(s.level_info, level)) != 0
            && value!(s.pawn_show_menu, value!(s.viewport_actor, viewport)) == 0
            && value!(s.viewport_show_windows_mouse, viewport) == 0
        {
            call!(s.viewport_set_mouse_capture, viewport, 1, 1, 1);
        } else if click_flags & MOUSE_LAST_RELEASE != 0
            && value!(s.client_capture_mouse, client) == 0
        {
            call!(s.viewport_set_mouse_capture, viewport, 0, 0, 0);
        }
    }
    Ok(())
}

/// `UGameEngine::MousePosition(Viewport, ClickFlags, X, Y)`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`. `viewport` may be null, as the C++ checks it.
pub unsafe fn mouse_position(
    s: &Hp2GameServices,
    engine: *mut c_void,
    viewport: *mut c_void,
    x: f32,
    y: f32,
) -> Result<(), i32> {
    unsafe {
        if viewport.is_null() {
            return Ok(());
        }
        call!(s.viewport_set_windows_mouse, viewport, x, y);
        if value!(s.replay_recording, engine) != 0 {
            call!(s.replay_serialize_input, engine, IK_MOUSE_X, IST_MAX, x);
            call!(s.replay_serialize_input, engine, IK_MOUSE_Y, IST_MAX, y);
        }
    }
    Ok(())
}

/// `UGameEngine::Key(Viewport, Key)`. Returns `UBOOL`.
///
/// # Safety
/// `engine` and `viewport` must be live engine objects. `s` must be valid.
pub unsafe fn key(
    s: &Hp2GameServices,
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
) -> Result<i32, i32> {
    unsafe {
        if value!(s.engine_key_base, engine, viewport, key) != 0 {
            if value!(s.replay_recording, engine) != 0 {
                call!(s.replay_serialize_input, engine, key, IST_MAX, 0.0);
            }
            return Ok(1);
        }
        Ok(0)
    }
}

/// `UGameEngine::InputEvent(Viewport, iKey, State, Delta)`. Returns `UBOOL`.
///
/// # Safety
/// `engine` and `viewport` must be live engine objects. `s` must be valid.
pub unsafe fn input_event(
    s: &Hp2GameServices,
    engine: *mut c_void,
    viewport: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> Result<i32, i32> {
    unsafe {
        // Special pause toggle on the Delete key.
        if state == IST_PRESS && key == IK_DELETE {
            call!(s.level_toggle_special_pause, value!(s.level, engine));
        }

        if value!(s.replay_replaying, engine) != 0 && value!(s.replay_in_replay, engine) == 0 {
            // Ignore real input, except for replay control keys.
            if state == IST_PRESS && key == IK_ESCAPE {
                call!(s.replay_stop, engine);
            } else if state == IST_PRESS && key == IK_PAUSE {
                let paused = value!(s.replay_paused, engine) != 0;
                call!(s.replay_pause, engine, i32::from(!paused));
            } else if state == IST_PRESS
                && key == IK_SCROLL_LOCK
                && value!(s.replay_paused, engine) != 0
            {
                // Single step.
                call!(s.replay_pause_step, engine);
            }
            return Ok(1);
        } else if value!(s.replay_recording, engine) != 0
            && state == IST_PRESS
            && key == IK_BACKSLASH
        {
            call!(s.replay_stop, engine);
            return Ok(1);
        }

        // Intercept, and record handled input keys.
        if value!(s.replay_recording, engine) != 0 {
            call!(s.replay_serialize_input, engine, key, state, delta);
        }
        Ok(value!(
            s.engine_input_event_base,
            engine,
            viewport,
            key,
            state,
            delta
        ))
    }
}

pub(crate) trait PaintProgressOps {
    type Error;
    unsafe fn engine_is_drawing(&mut self, engine: *mut c_void) -> std::result::Result<bool, Self::Error>;
    unsafe fn client(&mut self, engine: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn viewport0(&mut self, client: *mut c_void) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn draw_with_load_fog(&mut self, engine: *mut c_void, viewport: *mut c_void) -> std::result::Result<(), Self::Error>;
}

pub(crate) unsafe fn paint_progress_with<O: PaintProgressOps>(
    ops: &mut O,
    engine: *mut c_void,
) -> std::result::Result<(), O::Error> {
    if unsafe { ops.engine_is_drawing(engine)? } { return Ok(()); }
    let client = unsafe { ops.client(engine)? };
    let viewport = unsafe { ops.viewport0(client)? };
    unsafe { ops.draw_with_load_fog(engine, viewport) }
}

struct NativePaintProgressOps<'a>(&'a Hp2GameServices);

impl PaintProgressOps for NativePaintProgressOps<'_> {
    type Error = i32;
    unsafe fn engine_is_drawing(&mut self, engine: *mut c_void) -> Result<bool, i32> {
        unsafe { Ok(value!(self.0.engine_is_drawing, engine) != 0) }
    }
    unsafe fn client(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client, engine)) }
    }
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client_viewport0, client)) }
    }
    unsafe fn draw_with_load_fog(&mut self, engine: *mut c_void, viewport: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.draw_with_load_fog, engine, viewport); }
        Ok(())
    }
}

/// `UGameEngine::PaintProgress()` after the C++ `guard`.
///
/// # Safety
/// `engine` must be a live `UGameEngine` with a non-null `Client` holding a viewport.
/// `s` must be valid.
pub unsafe fn paint_progress(s: &Hp2GameServices, engine: *mut c_void) -> Result<(), i32> {
    unsafe { paint_progress_with(&mut NativePaintProgressOps(s), engine) }
}

/// `check(Expr)` failure, replicating `appFailAssert` at the original `UnGame.cpp` line.
fn check(s: &Hp2GameServices, ok: bool, expr: &CStr, line: i32) -> Result<(), i32> {
    if ok {
        return Ok(());
    }
    let status = unsafe { (s.check_failed)(expr.as_ptr(), line) };
    Err(if status != 0 { status } else { 1 })
}

/// `UGameEngine::SetClientTravel(Player, NextURL, bItems, TravelType)` after the C++
/// `guard`. `check(Player)` is at the original `UnGame.cpp:1805` (`da56c30`).
///
/// # Safety
/// `s` must be valid. `player` and `next_url` are passed through to the C++ leaf, which
/// performs the original `CastChecked<UViewport>` and field writes.
pub unsafe fn set_client_travel(
    s: &Hp2GameServices,
    player: *mut c_void,
    next_url: *const u32,
    b_items: i32,
    travel_type: i32,
) -> Result<(), i32> {
    unsafe { set_client_travel_with(player, next_url, b_items, travel_type,
        |valid| check(s, valid, c"Player", 1805),
        |player, url, travel, items| {
            call!(s.viewport_cast_set_travel, player, url, travel, items);
            Ok(())
        })
    }
}

/// Shared source assertion and viewport travel dispatch, independent of C++ vptrs.
///
/// # Safety
/// Player and URL remain live through providers; the store performs CastChecked<UViewport>.
pub(crate) unsafe fn set_client_travel_with<E>(
    player: *mut c_void, next_url: *const u32, items: i32, travel: i32,
    check_player: impl FnOnce(bool) -> Result<(), E>,
    store: impl FnOnce(*mut c_void, *const u32, i32, i32) -> Result<(), E>,
) -> Result<(), E> {
    check_player(!player.is_null())?;
    store(player, next_url, travel, items)
}

/// Typed leaves for the shared `UGameEngine::UpdateConnectingMessage` policy.
pub(crate) trait ConnectingMessageOps {
    type Error;
    unsafe fn pending(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn client(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor(&mut self, viewport: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn progress_expired(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn pending_is_demo(&mut self, pending: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn format_demo(
        &mut self, pending: *mut c_void, msg1: *mut u32, msg2: *mut u32,
    ) -> Result<(), Self::Error>;
    unsafe fn format_connecting(
        &mut self, pending: *mut c_void, msg1: *mut u32, msg2: *mut u32,
    ) -> Result<(), Self::Error>;
    unsafe fn set_progress(
        &mut self, engine: *mut c_void, msg1: *const u32, msg2: *const u32, seconds: f32,
    ) -> Result<(), Self::Error>;
}

/// Runs source-ordered connecting-message selection and updates the player progress fields.
///
/// # Safety
/// The backend retains all objects and message buffers through the synchronous call.
pub(crate) unsafe fn update_connecting_message_with<O: ConnectingMessageOps>(
    ops: &mut O,
    engine: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let pending = ops.pending(engine)?;
        if pending.is_null() { return Ok(()); }
        let client = ops.client(engine)?;
        if client.is_null() || ops.viewport_count(client)? == 0 { return Ok(()); }
        let viewport = ops.viewport0(client)?;
        let actor = ops.actor(viewport)?;
        if !ops.progress_expired(actor)? { return Ok(()); }
        let mut msg1 = [0u32; 256];
        let mut msg2 = [0u32; 256];
        if ops.pending_is_demo(pending)? {
            ops.format_demo(pending, msg1.as_mut_ptr(), msg2.as_mut_ptr())?;
        } else {
            ops.format_connecting(pending, msg1.as_mut_ptr(), msg2.as_mut_ptr())?;
        }
        ops.set_progress(engine, msg1.as_ptr(), msg2.as_ptr(), 60.0)?;
    }
    Ok(())
}

struct NativeConnectingMessageOps<'a>(&'a Hp2GameServices);
impl ConnectingMessageOps for NativeConnectingMessageOps<'_> {
    type Error = i32;
    unsafe fn pending(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.pending, engine)) }
    }
    unsafe fn client(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client, engine)) }
    }
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, i32> {
        unsafe { Ok(value!(self.0.client_viewport_count, client)) }
    }
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client_viewport0, client)) }
    }
    unsafe fn actor(&mut self, viewport: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.viewport_actor, viewport)) }
    }
    unsafe fn progress_expired(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        unsafe { Ok(value!(self.0.pawn_progress_expired, actor) != 0) }
    }
    unsafe fn pending_is_demo(&mut self, pending: *mut c_void) -> Result<bool, i32> {
        unsafe { Ok(!value!(self.0.pending_demo_rec_driver, pending).is_null()) }
    }
    unsafe fn format_demo(&mut self, pending: *mut c_void, msg1: *mut u32, msg2: *mut u32) -> Result<(), i32> {
        unsafe { call!(self.0.pending_format_demo_message, pending, msg1, msg2); }
        Ok(())
    }
    unsafe fn format_connecting(&mut self, pending: *mut c_void, msg1: *mut u32, msg2: *mut u32) -> Result<(), i32> {
        unsafe { call!(self.0.pending_format_connecting_message, pending, msg1, msg2); }
        Ok(())
    }
    unsafe fn set_progress(
        &mut self, engine: *mut c_void, msg1: *const u32, msg2: *const u32, seconds: f32,
    ) -> Result<(), i32> {
        unsafe { super::game_engine::set_progress(self.0, engine, msg1, msg2, seconds) }
    }
}

/// `UGameEngine::UpdateConnectingMessage()` after the C++ `guard`.
///
/// # Safety
/// `engine` must be a live `UGameEngine`. `s` must be valid.
pub unsafe fn update_connecting_message(
    s: &Hp2GameServices,
    engine: *mut c_void,
) -> Result<(), i32> {
    unsafe { update_connecting_message_with(&mut NativeConnectingMessageOps(s), engine) }
}

pub(crate) trait ProgressOps {
    type Error;
    unsafe fn client(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor(&mut self, viewport: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn show_upgrade_menu(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn set_message(&mut self, actor: *mut c_void, slot: i32, text: *const u32) -> Result<(), Self::Error>;
    unsafe fn set_timeout(&mut self, actor: *mut c_void, seconds: f32) -> Result<(), Self::Error>;
}

/// Runs the common GameEngine progress policy without a virtual object call.
///
/// # Safety
/// The selected backend retains all records and text through synchronous calls.
pub(crate) unsafe fn set_progress_with<O: ProgressOps>(
    ops: &mut O,
    engine: *mut c_void,
    str1: *const u32,
    str2: *const u32,
    seconds: f32,
) -> Result<(), O::Error> {
    // SAFETY: backend operations receive only live caller-owned records.
    unsafe {
        let client = ops.client(engine)?;
        if client.is_null() || ops.viewport_count(client)? == 0 { return Ok(()); }
        let viewport = ops.viewport0(client)?;
        let actor = ops.actor(viewport)?;
        if seconds == -1.0 { ops.show_upgrade_menu(actor)?; }
        ops.set_message(actor, 0, str1)?;
        ops.set_message(actor, 1, str2)?;
        ops.set_timeout(actor, seconds)
    }
}

struct NativeProgressOps<'a>(&'a Hp2GameServices);

impl ProgressOps for NativeProgressOps<'_> {
    type Error = i32;
    unsafe fn client(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client, engine)) }
    }
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, i32> {
        unsafe { Ok(value!(self.0.client_viewport_count, client)) }
    }
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.client_viewport0, client)) }
    }
    unsafe fn actor(&mut self, viewport: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.viewport_actor, viewport)) }
    }
    unsafe fn show_upgrade_menu(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.pawn_show_upgrade_menu, actor); }
        Ok(())
    }
    unsafe fn set_message(&mut self, actor: *mut c_void, slot: i32, text: *const u32) -> Result<(), i32> {
        unsafe { call!(self.0.pawn_set_progress, actor, slot, text); }
        Ok(())
    }
    unsafe fn set_timeout(&mut self, actor: *mut c_void, seconds: f32) -> Result<(), i32> {
        unsafe { call!(self.0.pawn_set_progress_timeout, actor, seconds); }
        Ok(())
    }
}
