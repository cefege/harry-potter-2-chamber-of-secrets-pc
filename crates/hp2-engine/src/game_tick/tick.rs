//! `UGameEngine::Tick` body (oracle 1967–2246). The C++ shell keeps the
//! `guard`/`unguard` wrapper and forwards `DeltaSeconds`.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

use super::{AXIS_X, AXIS_Y, Hp2GameTickServices as S, IK_MOUSE_X, IK_MOUSE_Y, IST_MAX, max};

/// C++ `static UBOOL WasPaused=1;` inside `UGameEngine::Tick`.
static WAS_PAUSED: AtomicI32 = AtomicI32::new(1);

/// Clock on a field: `T -= appCycles()` (`UnFile.h:198`).
#[inline]
unsafe fn clock<O: TickOps>(s: &O, t: i32) -> Result<i32, O::Error> {
    unsafe { Ok(t.wrapping_sub(s.app_cycles()? as i32)) }
}

/// Unclock on a field: `T += appCycles()-34` (`UnFile.h:199`).
#[inline]
unsafe fn unclock<O: TickOps>(s: &O, t: i32) -> Result<i32, O::Error> {
    unsafe { Ok(t.wrapping_add(s.app_cycles()?.wrapping_sub(34) as i32)) }
}

/// Native/Core leaves required by the source-shared GetMaxTickRate policy.
pub(crate) trait MaxTickRateOps {
    type Error;
    fn parse_lan_play(&mut self) -> Result<bool, Self::Error>;
    fn is_client(&mut self) -> Result<bool, Self::Error>;
    unsafe fn lan_demo_server_max_tick_rate(
        &mut self,
        net: *mut c_void,
        demo: *mut c_void,
    ) -> Result<f32, Self::Error>;
}
/// Source-shared Tick host operations. Core implements these without entering the native ABI.
pub(crate) trait TickOps {
    type Error;
    unsafe fn replay_paused(&self, engine: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn replay_serialize_frame_tick(&self, engine: *mut c_void, delta: f32) -> Result<f32, Self::Error>;
    unsafe fn app_cycles(&self) -> Result<u32, Self::Error>;
    unsafe fn log_all_windows_closed(&self) -> Result<(), Self::Error>;
    unsafe fn app_request_exit(&self, code: i32) -> Result<(), Self::Error>;
    unsafe fn viewport_set_mouse_capture(&self, viewport: *mut c_void, capture: i32, clip: i32, focus_only: i32) -> Result<(), Self::Error>;
    unsafe fn static_tick(&self) -> Result<(), Self::Error>;
    unsafe fn gcache_tick(&self) -> Result<(), Self::Error>;
    unsafe fn level_tick_all(&self, level: *mut c_void, delta: f32) -> Result<(), Self::Error>;
    unsafe fn g_repair_save_pending(&self) -> Result<i32, Self::Error>;
    unsafe fn g_repair_save_slot(&self) -> Result<i32, Self::Error>;
    unsafe fn set_g_repair_save_pending(&self, value: i32) -> Result<(), Self::Error>;
    unsafe fn set_g_repair_save_slot(&self, value: i32) -> Result<(), Self::Error>;
    unsafe fn engine_save_game(&self, engine: *mut c_void, slot: i32) -> Result<(), Self::Error>;
    unsafe fn log_save_repair(&self, slot: i32) -> Result<(), Self::Error>;
    unsafe fn make_directory(&self, path: *const c_void, tree: i32) -> Result<(), Self::Error>;
    unsafe fn system_save_path(&self) -> Result<*const c_void, Self::Error>;
    unsafe fn system_save_slot_path(&self) -> Result<*const c_void, Self::Error>;
    unsafe fn log_saving_game(&self, filename: *const c_void) -> Result<(), Self::Error>;
    unsafe fn engine_exec_restart_sounds(&self, engine: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn engine_flush(&self, engine: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn server_switch_travel(&self, engine: *mut c_void, mode: i32) -> Result<(), Self::Error>;
    unsafe fn client_travel_browse(&self, engine: *mut c_void, viewport: *mut c_void, mode: i32) -> Result<(), Self::Error>;
    unsafe fn level_info_next_url_is_restart(&self, info: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn viewport_travel_is_restart(&self, viewport: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn pending_tick(&self, pending: *mut c_void, delta: f32) -> Result<(), Self::Error>;
    unsafe fn pending_set_failure_progress(&self, engine: *mut c_void, pending: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn pending_log_failure(&self, pending: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn console_connect_failure(&self, console: *mut c_void, pending: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn pending_level_delete(&self, pending: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn pending_load_scope(&self, engine: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn client_tick(&self, client: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn viewport_event_view_flash(&self, viewport: *mut c_void, delta: f32) -> Result<(), Self::Error>;
    unsafe fn replay_replaying(&self, engine: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn replay_serialize_frame_input(&self, engine: *mut c_void, key: &mut i32, state: &mut i32, delta: &mut f32) -> Result<i32, Self::Error>;
    unsafe fn engine_key(&self, engine: *mut c_void, viewport: *mut c_void, key: i32) -> Result<(), Self::Error>;
    unsafe fn engine_input_event(&self, engine: *mut c_void, viewport: *mut c_void, key: i32, state: i32, delta: f32) -> Result<(), Self::Error>;
    unsafe fn g_ticks_increment(&self) -> Result<(), Self::Error>;
    unsafe fn string_nonempty(&self, value: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_info_pauser_nonempty(&self, info: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn pending_error_nonempty(&self, pending: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn pending_fail_code_nonempty(&self, pending: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_info_next_url_nonempty(&self, info: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn viewport_travel_url_nonempty(&self, viewport: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_info(&self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
}


impl TickOps for S {
    type Error = i32;
    unsafe fn string_nonempty(&self, value: *mut c_void) -> Result<i32, i32> { unsafe { S::string_nonempty(self, value) } }
    unsafe fn level_info_pauser_nonempty(&self, info: *mut c_void) -> Result<i32, i32> { unsafe { super::level_info_pauser_nonempty(self, info) } }
    unsafe fn pending_error_nonempty(&self, pending: *mut c_void) -> Result<i32, i32> { unsafe { super::pending_error_nonempty(self, pending) } }
    unsafe fn pending_fail_code_nonempty(&self, pending: *mut c_void) -> Result<i32, i32> { unsafe { super::pending_fail_code_nonempty(self, pending) } }
    unsafe fn level_info_next_url_nonempty(&self, info: *mut c_void) -> Result<i32, i32> { unsafe { super::level_info_next_url_nonempty(self, info) } }
    unsafe fn viewport_travel_url_nonempty(&self, viewport: *mut c_void) -> Result<i32, i32> { unsafe { super::viewport_travel_url_nonempty(self, viewport) } }
    unsafe fn replay_paused(&self, e: *mut c_void) -> Result<i32, i32> { unsafe { S::replay_paused(self, e) } }
    unsafe fn replay_serialize_frame_tick(&self, e: *mut c_void, dt: f32) -> Result<f32, i32> { unsafe { S::replay_serialize_frame_tick(self, e, dt) } }
    unsafe fn app_cycles(&self) -> Result<u32, i32> { unsafe { S::app_cycles(self) } }
    unsafe fn log_all_windows_closed(&self) -> Result<(), i32> { unsafe { S::log_all_windows_closed(self) } }
    unsafe fn app_request_exit(&self, code: i32) -> Result<(), i32> { unsafe { S::app_request_exit(self, code) } }
    unsafe fn viewport_set_mouse_capture(&self, vp: *mut c_void, c: i32, clip: i32, focus: i32) -> Result<(), i32> { unsafe { S::viewport_set_mouse_capture(self, vp, c, clip, focus) } }
    unsafe fn static_tick(&self) -> Result<(), i32> { unsafe { S::static_tick(self) } }
    unsafe fn gcache_tick(&self) -> Result<(), i32> { unsafe { S::gcache_tick(self) } }
    unsafe fn level_tick_all(&self, level: *mut c_void, dt: f32) -> Result<(), i32> { unsafe { S::level_tick_all(self, level, dt) } }
    unsafe fn g_repair_save_pending(&self) -> Result<i32, i32> { unsafe { S::g_repair_save_pending(self) } }
    unsafe fn g_repair_save_slot(&self) -> Result<i32, i32> { unsafe { S::g_repair_save_slot(self) } }
    unsafe fn set_g_repair_save_pending(&self, value: i32) -> Result<(), i32> { unsafe { S::set_g_repair_save_pending(self, value) } }
    unsafe fn set_g_repair_save_slot(&self, value: i32) -> Result<(), i32> { unsafe { S::set_g_repair_save_slot(self, value) } }
    unsafe fn engine_save_game(&self, e: *mut c_void, slot: i32) -> Result<(), i32> { unsafe { S::engine_save_game(self, e, slot) } }
    unsafe fn log_save_repair(&self, slot: i32) -> Result<(), i32> { unsafe { S::log_save_repair(self, slot) } }
    unsafe fn make_directory(&self, path: *const c_void, tree: i32) -> Result<(), i32> { unsafe { S::make_directory(self, path, tree) } }
    unsafe fn system_save_path(&self) -> Result<*const c_void, i32> { unsafe { S::system_save_path(self) } }
    unsafe fn system_save_slot_path(&self) -> Result<*const c_void, i32> { unsafe { S::system_save_slot_path(self) } }
    unsafe fn log_saving_game(&self, filename: *const c_void) -> Result<(), i32> { unsafe { S::log_saving_game(self, filename) } }
    unsafe fn engine_exec_restart_sounds(&self, e: *mut c_void) -> Result<(), i32> { unsafe { S::engine_exec_restart_sounds(self, e) } }
    unsafe fn engine_flush(&self, e: *mut c_void) -> Result<(), i32> { unsafe { S::engine_flush(self, e) } }
    unsafe fn server_switch_travel(&self, e: *mut c_void, mode: i32) -> Result<(), i32> { unsafe { S::server_switch_travel(self, e, mode) } }
    unsafe fn client_travel_browse(&self, e: *mut c_void, vp: *mut c_void, mode: i32) -> Result<(), i32> { unsafe { S::client_travel_browse(self, e, vp, mode) } }
    unsafe fn level_info_next_url_is_restart(&self, li: *mut c_void) -> Result<i32, i32> { unsafe { S::level_info_next_url_is_restart(self, li) } }
    unsafe fn viewport_travel_is_restart(&self, vp: *mut c_void) -> Result<i32, i32> { unsafe { S::viewport_travel_is_restart(self, vp) } }
    unsafe fn pending_tick(&self, p: *mut c_void, dt: f32) -> Result<(), i32> { unsafe { S::pending_tick(self, p, dt) } }
    unsafe fn pending_set_failure_progress(&self, e: *mut c_void, p: *mut c_void) -> Result<(), i32> { unsafe { S::pending_set_failure_progress(self, e, p) } }
    unsafe fn pending_log_failure(&self, p: *mut c_void) -> Result<(), i32> { unsafe { S::pending_log_failure(self, p) } }
    unsafe fn console_connect_failure(&self, c: *mut c_void, p: *mut c_void) -> Result<(), i32> { unsafe { S::console_connect_failure(self, c, p) } }
    unsafe fn pending_level_delete(&self, p: *mut c_void) -> Result<(), i32> { unsafe { S::pending_level_delete(self, p) } }
    unsafe fn pending_load_scope(&self, e: *mut c_void) -> Result<(), i32> { unsafe { S::pending_load_scope(self, self, e) } }
    unsafe fn level_info(&self, level: *mut c_void) -> Result<*mut c_void, i32> { unsafe { S::level_info(self, level) } }
    unsafe fn client_tick(&self, c: *mut c_void) -> Result<(), i32> { unsafe { S::client_tick(self, c) } }
    unsafe fn viewport_event_view_flash(&self, vp: *mut c_void, dt: f32) -> Result<(), i32> { unsafe { S::viewport_event_view_flash(self, vp, dt) } }
    unsafe fn replay_replaying(&self, e: *mut c_void) -> Result<i32, i32> { unsafe { S::replay_replaying(self, e) } }
    unsafe fn replay_serialize_frame_input(&self, e: *mut c_void, key: &mut i32, state: &mut i32, dt: &mut f32) -> Result<i32, i32> { unsafe { S::replay_serialize_frame_input(self, e, key, state, dt) } }
    unsafe fn engine_key(&self, e: *mut c_void, vp: *mut c_void, key: i32) -> Result<(), i32> { unsafe { S::engine_key(self, e, vp, key) } }
    unsafe fn engine_input_event(&self, e: *mut c_void, vp: *mut c_void, key: i32, state: i32, dt: f32) -> Result<(), i32> { unsafe { S::engine_input_event(self, e, vp, key, state, dt) } }
    unsafe fn g_ticks_increment(&self) -> Result<(), i32> { unsafe { S::g_ticks_increment(self) } }
}

/// Process-wide cached `LanPlay` flag and body of `GetMaxTickRate`.
///
/// # Safety
/// `e` and every non-null engine object reached by the source reads must be live.
pub(crate) unsafe fn get_max_tick_rate_with<O: MaxTickRateOps>(
    ops: &mut O,
    e: *mut c_void,
) -> Result<f32, O::Error> {
    static LAN_PLAY: OnceLock<bool> = OnceLock::new();
    unsafe {
        let lan_play = if let Some(value) = LAN_PLAY.get() {
            *value
        } else {
            let value = ops.parse_lan_play()?;
            let _ = LAN_PLAY.set(value);
            value
        };
        let level = super::g_level(e);
        if !level.is_null() {
            let net = super::level_net_driver(level);
            if !net.is_null() && !ops.is_client()? {
                let rate = if lan_play {
                    super::net_driver_lan_server_max_tick_rate(net)
                } else {
                    super::net_driver_server_max_tick_rate(net)
                };
                return Ok(super::clamp(rate, 10.0, 120.0));
            }
            if !net.is_null() {
                let connection = super::net_driver_server_connection(net);
                if !connection.is_null() {
                    return Ok((super::connection_current_net_speed(connection) / 64) as f32);
                }
            }
            let demo = super::level_demo_rec_driver(level);
            if !demo.is_null() && super::demo_rec_driver_server_connection(demo).is_null() {
                let rate = if lan_play {
                    ops.lan_demo_server_max_tick_rate(net, demo)?
                } else {
                    super::demo_rec_driver_net_server_max_tick_rate(demo)
                };
                return Ok(super::clamp(rate, 10.0, 120.0));
            }
        }
        Ok(max(0.0, super::engine_frame_rate_limit(e)))
    }
}

struct NativeMaxTickRateOps<'a>(&'a S);
impl MaxTickRateOps for NativeMaxTickRateOps<'_> {
    type Error = i32;
    fn parse_lan_play(&mut self) -> Result<bool, i32> {
        unsafe { self.0.parse_param_lan_play().map(|value| value != 0) }
    }
    fn is_client(&mut self) -> Result<bool, i32> {
        unsafe { self.0.g_is_client().map(|value| value != 0) }
    }
    unsafe fn lan_demo_server_max_tick_rate(
        &mut self,
        net: *mut c_void,
        _demo: *mut c_void,
    ) -> Result<f32, i32> {
        unsafe { Ok(super::net_driver_lan_server_max_tick_rate(net) as f32) }
    }
}

/// `UGameEngine::GetMaxTickRate` Native service-table adapter.
///
/// # Safety
/// The service table and all object handles read by the source algorithm are live.
pub unsafe fn get_max_tick_rate(s: &S, e: *mut c_void) -> Result<f32, i32> {
    unsafe { get_max_tick_rate_with(&mut NativeMaxTickRateOps(s), e) }
}

/// Body of `UGameEngine::Tick`.
///
/// # Safety
/// Engine handles must be the live objects the C++ shell passes. Null
/// `Client`/`GLevel` dereferences crash as the original does.
pub unsafe fn tick_with<O: TickOps>(s: &O, e: *mut c_void, dt: f32) -> Result<(), O::Error> {
    unsafe {
        if s.replay_paused(e)? != 0 {
            return Ok(());
        }
        // SerializeFrameTick takes DeltaSeconds by reference; the replayed value is used below.
        let dt = s.replay_serialize_frame_tick(e, dt)?;

        // LocalTickCycles=0; clock(LocalTickCycles)?.
        let mut local_tick = clock(s, 0)?;

        let client = unsafe { super::engine_client(e) };
        if !client.is_null() && unsafe { super::client_viewport_count(client) } == 0 {
            s.log_all_windows_closed()?;
            s.app_request_exit(0)?;
            return Ok(());
        }

        // Pause state and mouse capture.
        if !client.is_null()
            && unsafe { super::client_viewport_count(client) } == 1
            && !unsafe { super::g_level(e) }.is_null()
        {
            let vp0 = unsafe { super::client_viewport(client, 0) };
            let is_paused = (unsafe {
                s.level_info_pauser_nonempty(s.level_info(super::g_level(e))?)?
            } != 0
                || unsafe { super::actor_show_menu(super::viewport_actor(vp0)) } != 0
                || unsafe { super::viewport_show_windows_mouse(vp0) } != 0) as i32;
            let was_paused = WAS_PAUSED.load(Ordering::Relaxed);
            if is_paused != 0 && was_paused == 0 {
                s.viewport_set_mouse_capture(vp0, 0, 0, 0)?;
            } else if was_paused != 0
                && is_paused == 0
                && unsafe { super::client_capture_mouse(client) } != 0
            {
                s.viewport_set_mouse_capture(vp0, 1, 1, 1)?;
            }
            WAS_PAUSED.store(is_paused, Ordering::Relaxed);
        } else {
            WAS_PAUSED.store(0, Ordering::Relaxed);
        }

        s.static_tick()?;
        s.gcache_tick()?;

        // TickLevel: GameCycles=0; clock(GameCycles)?; ... unclock(GameCycles)?.
        unsafe { super::set_engine_game_cycles(e, 0) };
        let v = clock(s, unsafe { super::engine_game_cycles(e) })?;
        unsafe { super::set_engine_game_cycles(e, v) };
        if !unsafe { super::g_level(e) }.is_null() {
            s.level_tick_all(unsafe { super::g_level(e) }, dt)?;
        }
        let entry = unsafe { super::g_entry(e) };
        if !entry.is_null() && entry != unsafe { super::g_level(e) } {
            s.level_tick_all(entry, dt)?;
        }
        let client = unsafe { super::engine_client(e) };
        if !client.is_null() && unsafe { super::client_viewport_count(client) } != 0 {
            let vp0 = unsafe { super::client_viewport(client, 0) };
            let actor_level = unsafe { super::actor_level(super::viewport_actor(vp0)) };
            if actor_level != unsafe { super::g_level(e) } {
                s.level_tick_all(actor_level, dt)?;
            }
        }
        let v = unclock(s, unsafe { super::engine_game_cycles(e) })?;
        unsafe { super::set_engine_game_cycles(e, v) };

        // SaveRepair (UnGame.cpp:2028–2044).
        if s.g_repair_save_pending()? != 0 && s.g_repair_save_slot()? != -1 {
            let repair_client = unsafe { super::engine_client(e) };
            if !repair_client.is_null() && unsafe { super::client_viewport_count(repair_client) } != 0 {
                let repair_player =
                    unsafe { super::viewport_actor(super::client_viewport(repair_client, 0)) };
                if !repair_player.is_null()
                    && unsafe { super::actor_level(repair_player) } == unsafe { super::g_level(e) }
                {
                    let repair_slot = s.g_repair_save_slot()?;
                    unsafe { super::set_player_queued_save(repair_player, 0) };
                    s.set_g_repair_save_pending(0)?;
                    s.set_g_repair_save_slot(-1)?;
                    s.engine_save_game(e, repair_slot)?;
                    s.log_save_repair(repair_slot)?;
                    s.app_request_exit(0)?;
                    return Ok(());
                }
            }
        }

        // PlayerSaving. The player is read before the Client test (oracle order).
        let client = unsafe { super::engine_client(e) };
        let player = unsafe { super::viewport_actor(super::client_viewport(client, 0)) };
        if !client.is_null()
            && unsafe { super::client_viewport_count(client) } != 0
            && unsafe { super::player_queued_save(player) } != 0
        {
            s.make_directory(s.system_save_path()?, 0)?;
            let slot_path = s.system_save_slot_path()? as *const u32;
            s.make_directory(slot_path.cast(), 0)?;

            // Both owners expose native UTF-32 TCHAR paths at this boundary.
            let mut filename = [0u32; crate::game_save::PATH_CAPACITY];
            let mut len = 0usize;
            loop {
                let ch = *slot_path.add(len);
                if ch == 0 {
                    break;
                }
                assert!(
                    len < filename.len() - 12,
                    "SaveSlotPath exceeds Tick filename buffer"
                );
                filename[len] = ch;
                len += 1;
            }
            filename[len] = std::path::MAIN_SEPARATOR as u32;
            len += 1;
            for &ch in b"Save0.usa" {
                assert!(
                    len < filename.len() - 1,
                    "Tick save filename exceeds buffer"
                );
                filename[len] = u32::from(ch);
                len += 1;
            }
            filename[len] = 0;
            s.log_saving_game(filename.as_ptr().cast())?;
            unsafe { super::set_player_queued_save(player, 0) };
            s.engine_save_game(e, 0)?;
            s.engine_exec_restart_sounds(e)?;
            s.engine_flush(e)?;
            return Ok(());
        }

        // ServerTravel.
        let level = unsafe { super::g_level(e) };
        if !level.is_null() {
            let li = s.level_info(level)?;
            if s.level_info_next_url_nonempty(li)? != 0 {
                let countdown = unsafe { super::level_info_next_switch_countdown(li) } - dt;
                unsafe { super::set_level_info_next_switch_countdown(li, countdown) };
                if countdown <= 0.0 {
                    // 0: "?RESTART", 1: export items, 2: no travel info.
                    let mode = if s.level_info_next_url_is_restart(li)? != 0 {
                        0
                    } else if unsafe { super::level_info_next_items(li) } != 0 {
                        1
                    } else {
                        2
                    };
                    s.server_switch_travel(e, mode)?;
                    return Ok(());
                }
            }
        }

        // ClientTravel.
        let client = unsafe { super::engine_client(e) };
        if !client.is_null()
            && unsafe { super::client_viewport_count(client) } != 0
            && s.viewport_travel_url_nonempty(super::client_viewport(client, 0))? != 0
        {
            let vp = unsafe { super::client_viewport(client, 0) };
            let mode = if s.viewport_travel_is_restart(vp)? != 0 {
                0
            } else if unsafe { super::viewport_travel_items(vp) } != 0 {
                1
            } else {
                2
            };
            s.client_travel_browse(e, vp, mode)?;
            return Ok(());
        }

        // TickPending.
        let pending = unsafe { super::g_pending(e) };
        if !pending.is_null() {
            s.pending_tick(pending, dt)?;
            let pending = unsafe { super::g_pending(e) };
            if s.pending_error_nonempty(pending)? != 0 {
                s.pending_set_failure_progress(e, pending)?;
                s.pending_log_failure(unsafe { super::g_pending(e) })?;
                if s.pending_fail_code_nonempty(super::g_pending(e))? != 0 {
                    let client = unsafe { super::engine_client(e) };
                    if !client.is_null() {
                        let vp0 = unsafe { super::client_viewport(client, 0) };
                        if !vp0.is_null() {
                            let console = unsafe { super::viewport_console(vp0) };
                            if !console.is_null() {
                                s.console_connect_failure(console, unsafe { super::g_pending(e) })?;
                            }
                        }
                    }
                }
                s.pending_level_delete(unsafe { super::g_pending(e) })?;
                unsafe { super::set_g_pending(e, std::ptr::null_mut()) };
            } else if unsafe { super::pending_success(pending) } != 0
                && unsafe { super::pending_files_needed(pending) } == 0
                && unsafe { super::pending_sent_join(pending) } == 0
            {
                // Native stack FString lives across exactly the load/progress/join continuation.
                s.pending_load_scope(e)?;
                let pending = unsafe { super::g_pending(e) };
                s.pending_level_delete(pending)?;
                unsafe { super::set_g_pending(e, std::ptr::null_mut()) };
            }
        }

        // ClientTick.
        let mut local_client = 0i32;
        let client = unsafe { super::engine_client(e) };
        if !client.is_null() {
            local_client = clock(s, local_client)?;
            let vp0 = unsafe { super::client_viewport(client, 0) };
            s.viewport_event_view_flash(vp0, dt)?;
            s.client_tick(client)?;
            local_client = unclock(s, local_client)?;
        }
        unsafe { super::set_engine_client_cycles(e, local_client) };

        local_tick = unclock(s, local_tick)?;
        unsafe { super::set_engine_tick_cycles(e, local_tick) };

        // Replay input, only while replaying (oracle: if( Replay.Replaying() )).
        if s.replay_replaying(e)? != 0 {
            let mut key = 0i32;
            let mut state = 0i32;
            let mut delta = 0f32;
            while s.replay_serialize_frame_input(e, &mut key, &mut state, &mut delta)? != 0 {
                if state == IST_MAX {
                    if key == IK_MOUSE_X {
                        unsafe { super::set_viewport_mouse(super::g_viewport(e), AXIS_X, delta) };
                    } else if key == IK_MOUSE_Y {
                        unsafe { super::set_viewport_mouse(super::g_viewport(e), AXIS_Y, delta) };
                    } else {
                        s.engine_key(e, unsafe { super::g_viewport(e) }, key)?;
                    }
                } else {
                    s.engine_input_event(e, unsafe { super::g_viewport(e) }, key, state, delta)?;
                }
            }
        }

        s.g_ticks_increment()?;
        Ok(())
    }
}

/// Runs Tick through the existing native service-table adapter.
///
/// # Safety
/// `s`, the engine handle, and every non-null object reached by policy reads are live.
pub unsafe fn tick(s: &S, e: *mut c_void, dt: f32) -> Result<(), i32> {
    unsafe { tick_with(s, e, dt) }
}

/// Runs only inside the native `FString Error` lifetime.
///
/// # Safety
/// `error` is the live native stack string supplied by `pending_load_scope`.
pub unsafe fn pending_load(s: &S, e: *mut c_void, error: *mut c_void) -> Result<(), i32> {
    unsafe {
        s.pending_load_map(e, unsafe { super::g_pending(e) }, error)?;
        if s.string_nonempty(error)? != 0 {
            s.set_progress(
                e,
                s.localize_connection_failed()?,
                s.string_data(error)?,
                4.0,
            )?;
        } else if unsafe { super::pending_lone_player(super::g_pending(e)) } == 0 {
            unsafe { super::level_info_set_action(s.level_info(super::g_level(e))?, 1) };
            unsafe { super::level_info_set_action(s.level_info(super::g_entry(e))?, 1) };
            let client = unsafe { super::engine_client(e) };
            if !client.is_null() {
                s.client_tick(client)?;
            }
            s.pending_send_join(unsafe { super::g_pending(e) })?;
            unsafe { super::pending_clear_net_driver(super::g_pending(e)) };
            unsafe { super::pending_clear_demo_driver(super::g_pending(e)) };
        }
        Ok(())
    }
}
