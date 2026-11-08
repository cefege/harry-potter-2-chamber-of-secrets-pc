//! Port of `HarryPotter2/Unreal/Engine/Src/UnLevTic.cpp` (level tick).
//!
//! Every service operation returns a checked status; policy stops at its first
//! failure. Native stack continuations own guards, trace scopes, FMemMarks,
//! iterators and collision results. Explicit mark pops occur only at the
//! original success points. Engine objects remain raw pointers across callbacks.

use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, APlayerPawn, FActorLink, UActorChannel, UChannel, ULevel,
    UNetConnection, UNetDriver,
};

use crate::coords::{self, FRotator};
use crate::math::app_round;
use crate::prim::{dot, safe_normal};
use crate::vec::size_squared;

/// `Hp2LevTicServices` mirror (`Engine/Inc/Hp2LevTicServices.h`), field order
/// must match the header exactly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LevTicServices {
    pub trace_event_tick: unsafe extern "C" fn(token: u64) -> i32,
    pub actor_is_a: unsafe extern "C" fn(actor: *mut c_void, kind: i32, out: *mut i32) -> i32,
    pub actor_event: unsafe extern "C" fn(actor: *mut c_void, event: i32, delta: f32) -> i32,
    pub actor_process_state: unsafe extern "C" fn(actor: *mut c_void, delta: f32) -> i32,
    pub actor_perform_physics: unsafe extern "C" fn(actor: *mut c_void, delta: f32) -> i32,
    pub actor_move_smooth: unsafe extern "C" fn(actor: *mut c_void, delta: *const f32) -> i32,
    pub actor_is_probing:
        unsafe extern "C" fn(actor: *mut c_void, probe: i32, out: *mut i32) -> i32,
    pub actor_creature_trace_tick: unsafe extern "C" fn(actor: *mut c_void, delta: f32) -> i32,
    pub actor_latent_action: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    pub actor_show_self: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_check_enemy_visible: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub actor_event_player_calc_view: unsafe extern "C" fn(
        viewer: *mut c_void,
        location: *mut f32,
        rotation: *mut i32,
        out: *mut *mut c_void,
    ) -> i32,
    pub player_read_input: unsafe extern "C" fn(player: *mut c_void, delta: f32) -> i32,
    pub pawn_zero_input_floats: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    pub pawn_begin_modern_tps: unsafe extern "C" fn(
        pawn: *mut c_void,
        raw_base_x: *mut f32,
        raw_strafe: *mut f32,
        raw_camera_yaw: *mut f32,
        raw_camera_pitch: *mut f32,
        out: *mut i32,
    ) -> i32,
    pub pawn_finish_modern_tps: unsafe extern "C" fn(
        pawn: *mut c_void,
        active: i32,
        raw_base_x: f32,
        raw_strafe: f32,
        raw_camera_yaw: f32,
        raw_camera_pitch: f32,
        delta: f32,
    ) -> i32,
    pub pawn_apply_modern_tps: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    pub actor_get_anim:
        unsafe extern "C" fn(actor: *mut c_void, sequence: *const u8, out: *mut *mut c_void) -> i32,
    pub anim_notify_count: unsafe extern "C" fn(seq: *mut c_void, out: *mut i32) -> i32,
    pub anim_notify_get: unsafe extern "C" fn(
        seq: *mut c_void,
        index: i32,
        out_time: *mut f32,
        out_function: *mut u8,
    ) -> i32,
    pub actor_find_function:
        unsafe extern "C" fn(actor: *mut c_void, name: *const u8, out: *mut *mut c_void) -> i32,
    pub actor_process_event: unsafe extern "C" fn(actor: *mut c_void, function: *mut c_void) -> i32,
    pub mesh_get_root_movement:
        unsafe extern "C" fn(mesh: *mut c_void, owner: *mut c_void, out: *mut f32) -> i32,
    pub mesh_adjust_root_movement:
        unsafe extern "C" fn(mesh: *mut c_void, owner: *mut c_void, adjust: *const f32) -> i32,
    pub actor_update_frequency: unsafe extern "C" fn(
        actor: *mut c_void,
        viewer: *mut c_void,
        view_dir: *mut f32,
        view_pos: *mut f32,
        out: *mut f32,
    ) -> i32,
    pub actor_net_priority: unsafe extern "C" fn(
        actor: *mut c_void,
        sent: *mut c_void,
        time: f32,
        lag: f32,
        out: *mut f32,
    ) -> i32,
    pub actor_no_variables_to_replicate:
        unsafe extern "C" fn(actor: *mut c_void, old_version: *mut c_void, out: *mut i32) -> i32,
    pub actor_check_recent_changes: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    pub actor_class_default_life_span:
        unsafe extern "C" fn(actor: *mut c_void, out: *mut f32) -> i32,
    pub actor_class_default_is_static:
        unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    pub level_actor_count: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    pub level_actor:
        unsafe extern "C" fn(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    pub level_info: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    pub level_fast_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        end: *const f32,
        start: *const f32,
        out: *mut i32,
    ) -> i32,
    pub level_single_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        end: *const f32,
        start: *const f32,
        trace: u32,
        extent: *const f32,
        out_time: *mut f32,
        out_normal: *mut f32,
        out: *mut i32,
    ) -> i32,
    pub level_model_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        out_location: *mut f32,
        start: *const f32,
        end: *const f32,
        node_flags: i32,
    ) -> i32,
    pub level_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        mv: *const f32,
        rotation: *const i32,
        hit: *mut c_void,
        out_time: *mut f32,
        out_normal: *mut f32,
        out: *mut i32,
    ) -> i32,
    pub level_far_move_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, location: *const f32) -> i32,
    pub level_destroy_actor: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void) -> i32,
    pub level_zone_time_since_render:
        unsafe extern "C" fn(level: *mut c_void, zone_number: i32, out: *mut f32) -> i32,
    pub level_max_tick_rate: unsafe extern "C" fn(level: *mut c_void, out: *mut f32) -> i32,
    pub level_cycles_clock: unsafe extern "C" fn(level: *mut c_void, which: i32) -> i32,
    pub level_cycles_unclock: unsafe extern "C" fn(level: *mut c_void, which: i32) -> i32,
    pub level_init_stats: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_init_runaway: unsafe extern "C" fn() -> i32,
    pub level_gseconds_per_cycle: unsafe extern "C" fn(out: *mut f32) -> i32,
    pub gmath_sin_tab: unsafe extern "C" fn(index: i32, out: *mut f32) -> i32,
    pub level_update_time: unsafe extern "C" fn(info: *mut c_void) -> i32,
    pub level_startup_phase_trace: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_hash_tick: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_engine_set_progress_empty: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_engine_client_travel: unsafe extern "C" fn(level: *mut c_void, url: i32) -> i32,
    pub level_log_running_net: unsafe extern "C" fn(level: *mut c_void, client_count: i32) -> i32,
    pub level_send_net_stats: unsafe extern "C" fn(level: *mut c_void, updated: i32) -> i32,
    pub level_frame_timing_enabled: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub level_frame_timing_now: unsafe extern "C" fn(out: *mut i64) -> i32,
    pub level_frame_timing_log: unsafe extern "C" fn(start: i64) -> i32,
    pub level_app_frand: unsafe extern "C" fn(out: *mut f32) -> i32,
    pub driver_tick_dispatch: unsafe extern "C" fn(driver: *mut c_void, delta: f32) -> i32,
    pub driver_tick_flush: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    pub driver_client_connection_count:
        unsafe extern "C" fn(driver: *mut c_void, out: *mut i32) -> i32,
    pub driver_client_connection:
        unsafe extern "C" fn(driver: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    pub conn_is_net_ready:
        unsafe extern "C" fn(conn: *mut c_void, saturate: i32, out: *mut i32) -> i32,
    pub conn_sent_temp_count: unsafe extern "C" fn(conn: *mut c_void, out: *mut i32) -> i32,
    pub conn_sent_temp_set_net_tag:
        unsafe extern "C" fn(conn: *mut c_void, index: i32, net_tag: i32) -> i32,
    pub conn_sent_temp_index:
        unsafe extern "C" fn(conn: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    pub conn_actor_channel:
        unsafe extern "C" fn(conn: *mut c_void, actor: *mut c_void, out: *mut *mut c_void) -> i32,
    pub conn_create_actor_channel:
        unsafe extern "C" fn(conn: *mut c_void, out: *mut *mut c_void) -> i32,
    pub conn_set_channel_actor:
        unsafe extern "C" fn(channel: *mut c_void, actor: *mut c_void) -> i32,
    pub conn_package_map_class_index:
        unsafe extern "C" fn(conn: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    pub conn_replicate_player_channels: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    pub channel_recent_count: unsafe extern "C" fn(channel: *mut c_void, out: *mut i32) -> i32,
    pub channel_recent_first:
        unsafe extern "C" fn(channel: *mut c_void, out: *mut *mut c_void) -> i32,
    pub channel_dirty_count: unsafe extern "C" fn(channel: *mut c_void, out: *mut i32) -> i32,
    pub channel_is_net_ready:
        unsafe extern "C" fn(channel: *mut c_void, saturate: i32, out: *mut i32) -> i32,
    pub channel_replicate_actor: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    pub channel_close: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    pub level_cleanup_destroyed: unsafe extern "C" fn(level: *mut c_void) -> i32,
    pub level_is_editor: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub gmath_cos_tab: unsafe extern "C" fn(index: i32, out: *mut f32) -> i32,
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    pub level_client_viewports_num: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    pub mem_mark_pop: unsafe extern "C" fn(buffer: *mut c_void) -> i32,
    pub level_gmem: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    pub level_info_pauser_empty: unsafe extern "C" fn(info: *mut c_void, out: *mut i32) -> i32,
    pub check_enabled: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub mem_push:
        unsafe extern "C" fn(mem: *mut c_void, size: u64, count: i32, out: *mut *mut c_void) -> i32,
    pub tick_actor_scoped: unsafe extern "C" fn(
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
        relation: *const c_char,
        root: *const c_char,
        out: *mut i32,
    ) -> i32,
    pub level_tick_scoped: unsafe extern "C" fn(
        level: *mut c_void,
        tick_type: i32,
        delta: f32,
        info: *mut c_void,
        frame_timing: i32,
        frame_start: i64,
    ) -> i32,
    pub server_tick_client_scoped:
        unsafe extern "C" fn(level: *mut c_void, connection: *mut c_void, out: *mut i32) -> i32,
    pub level_tick_net_client: unsafe extern "C" fn(level: *mut c_void, delta: f32) -> i32,
    pub level_tick_net_server: unsafe extern "C" fn(level: *mut c_void, delta: f32) -> i32,
    pub level_tick_demo_record: unsafe extern "C" fn(level: *mut c_void, delta: f32) -> i32,
    pub level_tick_demo_playback: unsafe extern "C" fn(level: *mut c_void, delta: f32) -> i32,
    pub level_server_tick_client: unsafe extern "C" fn(
        level: *mut c_void,
        connection: *mut c_void,
        delta: f32,
        out: *mut i32,
    ) -> i32,
    pub guard_scope: unsafe extern "C" fn(
        scope: i32,
        context: *mut c_void,
        continuation: unsafe extern "C" fn(*mut c_void) -> i32,
    ) -> i32,
    pub sort_priorities: unsafe extern "C" fn(actors: *mut *mut ActorPriority, count: i32) -> i32,
    pub conn_send_stats: unsafe extern "C" fn(
        level: *mut c_void,
        connection: *mut c_void,
        kind: i32,
        actor_count: i32,
        max_rate: f32,
    ) -> i32,
    pub channel_iterator_valid: unsafe extern "C" fn(iterator: *mut c_void, out: *mut i32) -> i32,
    pub channel_iterator_next: unsafe extern "C" fn(iterator: *mut c_void) -> i32,
    pub channel_iterator_value:
        unsafe extern "C" fn(iterator: *mut c_void, out: *mut *mut c_void) -> i32,
    pub root_movement_scoped: unsafe extern "C" fn(actor: *mut c_void) -> i32,
}

const _: () =
    assert!(std::mem::size_of::<Hp2LevTicServices>() == 102 * std::mem::size_of::<usize>());

/// Native stack-owned trace state. No engine object is borrowed across callbacks.
#[repr(C)]
pub struct Hp2TickDispatch {
    pub token: u64,
    pub skip_reason: *const c_char,
    pub event_tick_dispatched: i32,
}

impl Hp2LevTicServices {
    pub(crate) unsafe fn trace_event_tick(&self, token: u64) -> Result<(), i32> {
        let status = unsafe { (self.trace_event_tick)(token) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_is_a(&self, actor: *mut c_void, kind: i32) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_is_a)(actor, kind, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_event(
        &self,
        actor: *mut c_void,
        event: i32,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_event)(actor, event, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_process_state(
        &self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_process_state)(actor, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_perform_physics(
        &self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_perform_physics)(actor, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_move_smooth(
        &self,
        actor: *mut c_void,
        delta: *const f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_move_smooth)(actor, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_is_probing(
        &self,
        actor: *mut c_void,
        probe: i32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_is_probing)(actor, probe, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_creature_trace_tick(
        &self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_creature_trace_tick)(actor, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_latent_action(&self, actor: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_latent_action)(actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_show_self(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_show_self)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_check_enemy_visible(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_check_enemy_visible)(actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_event_player_calc_view(
        &self,
        viewer: *mut c_void,
        location: *mut f32,
        rotation: *mut i32,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.actor_event_player_calc_view)(viewer, location, rotation, out.as_mut_ptr())
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn player_read_input(
        &self,
        player: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.player_read_input)(player, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn pawn_zero_input_floats(&self, pawn: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.pawn_zero_input_floats)(pawn) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn pawn_begin_modern_tps(
        &self,
        pawn: *mut c_void,
        raw_base_x: *mut f32,
        raw_strafe: *mut f32,
        raw_camera_yaw: *mut f32,
        raw_camera_pitch: *mut f32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.pawn_begin_modern_tps)(
                pawn,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
                out.as_mut_ptr(),
            )
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn pawn_finish_modern_tps(
        &self,
        pawn: *mut c_void,
        active: i32,
        raw_base_x: f32,
        raw_strafe: f32,
        raw_camera_yaw: f32,
        raw_camera_pitch: f32,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe {
            (self.pawn_finish_modern_tps)(
                pawn,
                active,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
                delta,
            )
        };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn pawn_apply_modern_tps(&self, pawn: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.pawn_apply_modern_tps)(pawn) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_get_anim(
        &self,
        actor: *mut c_void,
        sequence: *const u8,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_get_anim)(actor, sequence, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn anim_notify_count(&self, seq: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.anim_notify_count)(seq, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn anim_notify_get(
        &self,
        seq: *mut c_void,
        index: i32,
        out_time: *mut f32,
        out_function: *mut u8,
    ) -> Result<(), i32> {
        let status = unsafe { (self.anim_notify_get)(seq, index, out_time, out_function) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_find_function(
        &self,
        actor: *mut c_void,
        name: *const u8,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_find_function)(actor, name, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_process_event(
        &self,
        actor: *mut c_void,
        function: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_process_event)(actor, function) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn mesh_get_root_movement(
        &self,
        mesh: *mut c_void,
        owner: *mut c_void,
        out: *mut f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.mesh_get_root_movement)(mesh, owner, out) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn mesh_adjust_root_movement(
        &self,
        mesh: *mut c_void,
        owner: *mut c_void,
        adjust: *const f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.mesh_adjust_root_movement)(mesh, owner, adjust) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn actor_update_frequency(
        &self,
        actor: *mut c_void,
        viewer: *mut c_void,
        view_dir: *mut f32,
        view_pos: *mut f32,
    ) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.actor_update_frequency)(actor, viewer, view_dir, view_pos, out.as_mut_ptr())
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_net_priority(
        &self,
        actor: *mut c_void,
        sent: *mut c_void,
        time: f32,
        lag: f32,
    ) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_net_priority)(actor, sent, time, lag, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_no_variables_to_replicate(
        &self,
        actor: *mut c_void,
        old_version: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.actor_no_variables_to_replicate)(actor, old_version, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_check_recent_changes(&self, actor: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_check_recent_changes)(actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_class_default_life_span(
        &self,
        actor: *mut c_void,
    ) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_class_default_life_span)(actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn actor_class_default_is_static(
        &self,
        actor: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.actor_class_default_is_static)(actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_actor_count(&self, level: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_actor_count)(level, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_actor(
        &self,
        level: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_actor)(level, index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn link_actor(&self, link: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*link.cast::<FActorLink>()).actor })
    }
    pub(crate) unsafe fn link_next(&self, link: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*link.cast::<FActorLink>()).next })
    }
    pub(crate) unsafe fn level_info(&self, level: *mut c_void) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_info)(level, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_fast_line_check(
        &self,
        level: *mut c_void,
        end: *const f32,
        start: *const f32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_fast_line_check)(level, end, start, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_single_line_check(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        end: *const f32,
        start: *const f32,
        trace: u32,
        extent: *const f32,
        out_time: *mut f32,
        out_normal: *mut f32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.level_single_line_check)(
                level,
                actor,
                end,
                start,
                trace,
                extent,
                out_time,
                out_normal,
                out.as_mut_ptr(),
            )
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_model_line_check(
        &self,
        level: *mut c_void,
        out_location: *mut f32,
        start: *const f32,
        end: *const f32,
        node_flags: i32,
    ) -> Result<(), i32> {
        let status =
            unsafe { (self.level_model_line_check)(level, out_location, start, end, node_flags) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_move_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        mv: *const f32,
        rotation: *const i32,
        hit: *mut c_void,
        out_time: *mut f32,
        out_normal: *mut f32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.level_move_actor)(
                level,
                actor,
                mv,
                rotation,
                hit,
                out_time,
                out_normal,
                out.as_mut_ptr(),
            )
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_far_move_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        location: *const f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_far_move_actor)(level, actor, location) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_destroy_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_destroy_actor)(level, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_zone_time_since_render(
        &self,
        level: *mut c_void,
        zone_number: i32,
    ) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.level_zone_time_since_render)(level, zone_number, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_max_tick_rate(&self, level: *mut c_void) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_max_tick_rate)(level, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_inc_num_pv(&self, level: *mut c_void) -> Result<(), i32> {
        unsafe { (*level.cast::<ULevel>()).num_pv += 1 };
        Ok(())
    }
    pub(crate) unsafe fn level_cycles_clock(
        &self,
        level: *mut c_void,
        which: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_cycles_clock)(level, which) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_cycles_unclock(
        &self,
        level: *mut c_void,
        which: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_cycles_unclock)(level, which) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_init_stats(&self, level: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.level_init_stats)(level) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_init_runaway(&self) -> Result<(), i32> {
        let status = unsafe { (self.level_init_runaway)() };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_gseconds_per_cycle(&self) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_gseconds_per_cycle)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn gmath_sin_tab(&self, index: i32) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.gmath_sin_tab)(index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_update_time(&self, info: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.level_update_time)(info) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_startup_phase_trace(&self, level: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.level_startup_phase_trace)(level) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_hash_tick(&self, level: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.level_hash_tick)(level) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_engine_set_progress_empty(
        &self,
        level: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_engine_set_progress_empty)(level) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_engine_client_travel(
        &self,
        level: *mut c_void,
        url: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_engine_client_travel)(level, url) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_log_running_net(
        &self,
        level: *mut c_void,
        client_count: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_log_running_net)(level, client_count) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_send_net_stats(
        &self,
        level: *mut c_void,
        updated: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_send_net_stats)(level, updated) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_frame_timing_enabled(&self) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_frame_timing_enabled)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_frame_timing_now(&self) -> Result<i64, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_frame_timing_now)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_frame_timing_log(&self, start: i64) -> Result<(), i32> {
        let status = unsafe { (self.level_frame_timing_log)(start) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_app_frand(&self) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_app_frand)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn driver_time(&self, driver: *mut c_void) -> Result<i64, i32> {
        Ok(i64::from_ne_bytes(unsafe {
            (*driver.cast::<UNetDriver>()).time
        }))
    }
    pub(crate) unsafe fn driver_spawn_priority_seconds(
        &self,
        driver: *mut c_void,
    ) -> Result<f32, i32> {
        Ok(unsafe { (*driver.cast::<UNetDriver>()).spawn_priority_seconds })
    }
    pub(crate) unsafe fn driver_relevant_timeout(&self, driver: *mut c_void) -> Result<f32, i32> {
        Ok(unsafe { (*driver.cast::<UNetDriver>()).relevant_timeout })
    }
    pub(crate) unsafe fn driver_tick_dispatch(
        &self,
        driver: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.driver_tick_dispatch)(driver, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn driver_tick_flush(&self, driver: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.driver_tick_flush)(driver) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn driver_server_connection(
        &self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*driver.cast::<UNetDriver>()).server_connection })
    }
    pub(crate) unsafe fn driver_client_connection_count(
        &self,
        driver: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.driver_client_connection_count)(driver, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn driver_client_connection(
        &self,
        driver: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.driver_client_connection)(driver, index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_state(&self, conn: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).state })
    }
    pub(crate) unsafe fn conn_actor(&self, conn: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).actor })
    }
    pub(crate) unsafe fn conn_driver(&self, conn: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).driver })
    }
    pub(crate) unsafe fn conn_is_net_ready(
        &self,
        conn: *mut c_void,
        saturate: i32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_is_net_ready)(conn, saturate, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_last_receive_time(&self, conn: *mut c_void) -> Result<i64, i32> {
        Ok(i64::from_ne_bytes(unsafe {
            (*conn.cast::<UNetConnection>()).last_receive_time
        }))
    }
    pub(crate) unsafe fn conn_set_last_rep_time(
        &self,
        conn: *mut c_void,
        time: i64,
    ) -> Result<(), i32> {
        unsafe { (*conn.cast::<UNetConnection>()).last_rep_time = time.to_ne_bytes() };
        Ok(())
    }
    pub(crate) unsafe fn conn_inc_tick_count(&self, conn: *mut c_void) -> Result<i32, i32> {
        let c = conn.cast::<UNetConnection>();
        unsafe { (*c).tick_count += 1 };
        Ok(unsafe { (*c).tick_count })
    }
    pub(crate) unsafe fn conn_tick_count(&self, conn: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).tick_count })
    }
    pub(crate) unsafe fn conn_sent_temp_count(&self, conn: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_sent_temp_count)(conn, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_sent_temp_set_net_tag(
        &self,
        conn: *mut c_void,
        index: i32,
        net_tag: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.conn_sent_temp_set_net_tag)(conn, index, net_tag) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn conn_sent_temp_index(
        &self,
        conn: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_sent_temp_index)(conn, actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_best_lag(&self, conn: *mut c_void) -> Result<f32, i32> {
        Ok(unsafe { (*conn.cast::<UNetConnection>()).best_lag })
    }
    pub(crate) unsafe fn conn_actor_channel(
        &self,
        conn: *mut c_void,
        actor: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_actor_channel)(conn, actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_create_actor_channel(
        &self,
        conn: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_create_actor_channel)(conn, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_set_channel_actor(
        &self,
        channel: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.conn_set_channel_actor)(channel, actor) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn conn_package_map_class_index(
        &self,
        conn: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.conn_package_map_class_index)(conn, actor, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_replicate_player_channels(
        &self,
        conn: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.conn_replicate_player_channels)(conn) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn channel_last_update_time(&self, channel: *mut c_void) -> Result<i64, i32> {
        Ok(i64::from_ne_bytes(unsafe {
            (*channel.cast::<UActorChannel>()).last_update_time
        }))
    }
    pub(crate) unsafe fn channel_relevant_time(&self, channel: *mut c_void) -> Result<i64, i32> {
        Ok(i64::from_ne_bytes(unsafe {
            (*channel.cast::<UActorChannel>()).relevant_time
        }))
    }
    pub(crate) unsafe fn channel_set_relevant_time(
        &self,
        channel: *mut c_void,
        time: i64,
    ) -> Result<(), i32> {
        unsafe { (*channel.cast::<UActorChannel>()).relevant_time = time.to_ne_bytes() };
        Ok(())
    }
    pub(crate) unsafe fn channel_recent_count(&self, channel: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_recent_count)(channel, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn channel_recent_first(
        &self,
        channel: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_recent_first)(channel, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn channel_dirty_count(&self, channel: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_dirty_count)(channel, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn channel_is_net_ready(
        &self,
        channel: *mut c_void,
        saturate: i32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_is_net_ready)(channel, saturate, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn channel_replicate_actor(&self, channel: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.channel_replicate_actor)(channel) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn channel_close(&self, channel: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.channel_close)(channel) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_cleanup_destroyed(&self, level: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.level_cleanup_destroyed)(level) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_is_editor(&self) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_is_editor)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn gmath_cos_tab(&self, index: i32) -> Result<f32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.gmath_cos_tab)(index, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn conn_last_rep_time(&self, conn: *mut c_void) -> Result<i64, i32> {
        Ok(i64::from_ne_bytes(unsafe {
            (*conn.cast::<UNetConnection>()).last_rep_time
        }))
    }
    pub(crate) unsafe fn check_failed(&self, expr: *const c_char, line: i32) -> Result<(), i32> {
        let status = unsafe { (self.check_failed)(expr, line) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn channel_closing(&self, channel: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*channel.cast::<UChannel>()).closing })
    }
    pub(crate) unsafe fn level_client_viewports_num(&self, level: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_client_viewports_num)(level, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn mem_mark_pop(&self, buffer: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.mem_mark_pop)(buffer) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_gmem(&self) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_gmem)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_info_pauser_empty(&self, info: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.level_info_pauser_empty)(info, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_info_players_only(&self, info: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*info.cast::<ALevelInfo>()).b_players_only() as i32 })
    }
    pub(crate) unsafe fn check_enabled(&self) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.check_enabled)(out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn mem_push(
        &self,
        mem: *mut c_void,
        size: u64,
        count: i32,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.mem_push)(mem, size, count, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn tick_actor_scoped(
        &self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
        relation: *const c_char,
        root: *const c_char,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe {
            (self.tick_actor_scoped)(actor, delta, tick_type, relation, root, out.as_mut_ptr())
        };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_tick_scoped(
        &self,
        level: *mut c_void,
        tick_type: i32,
        delta: f32,
        info: *mut c_void,
        frame_timing: i32,
        frame_start: i64,
    ) -> Result<(), i32> {
        let status = unsafe {
            (self.level_tick_scoped)(level, tick_type, delta, info, frame_timing, frame_start)
        };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn server_tick_client_scoped(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.server_tick_client_scoped)(level, connection, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn level_tick_net_client(
        &self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_tick_net_client)(level, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_tick_net_server(
        &self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_tick_net_server)(level, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_tick_demo_record(
        &self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_tick_demo_record)(level, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_tick_demo_playback(
        &self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.level_tick_demo_playback)(level, delta) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn level_server_tick_client(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
        delta: f32,
    ) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status =
            unsafe { (self.level_server_tick_client)(level, connection, delta, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn guard_scope(
        &self,
        scope: i32,
        context: *mut c_void,
        continuation: unsafe extern "C" fn(*mut c_void) -> i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.guard_scope)(scope, context, continuation) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    pub(crate) unsafe fn sort_priorities(
        &self,
        actors: *mut *mut ActorPriority,
        count: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.sort_priorities)(actors, count) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    pub(crate) unsafe fn root_movement_scoped(&self, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.root_movement_scoped)(actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    pub(crate) unsafe fn conn_user_flags(&self, connection: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (*connection.cast::<UNetConnection>()).user_flags })
    }
    pub(crate) unsafe fn conn_send_stats(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
        kind: i32,
        actor_count: i32,
        max_rate: f32,
    ) -> Result<(), i32> {
        let status =
            unsafe { (self.conn_send_stats)(level, connection, kind, actor_count, max_rate) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn channel_actor(&self, channel: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (*channel.cast::<UActorChannel>()).actor })
    }
    pub(crate) unsafe fn channel_iterator_valid(&self, iterator: *mut c_void) -> Result<i32, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_iterator_valid)(iterator, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
    pub(crate) unsafe fn channel_iterator_next(&self, iterator: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.channel_iterator_next)(iterator) };
        if status != 0 {
            return Err(status);
        }
        Ok(())
    }
    pub(crate) unsafe fn channel_iterator_value(
        &self,
        iterator: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { (self.channel_iterator_value)(iterator, out.as_mut_ptr()) };
        if status != 0 {
            return Err(status);
        }
        Ok(unsafe { out.assume_init() })
    }
}

/// Execute Rust policy inside its original native guard. Engine failures use Result;
/// an unexpected Rust panic is resumed only after the C callback has returned.
pub(crate) unsafe fn guarded<T, F: FnMut() -> Result<T, i32>>(
    svc: &Hp2LevTicServices,
    scope: i32,
    f: F,
) -> Result<T, i32> {
    struct Context<T, F> {
        f: F,
        value: std::mem::MaybeUninit<T>,
        panic: Option<Box<dyn std::any::Any + Send>>,
    }
    unsafe extern "C" fn invoke<T, F: FnMut() -> Result<T, i32>>(context: *mut c_void) -> i32 {
        let context = unsafe { &mut *context.cast::<Context<T, F>>() };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (context.f)())) {
            Ok(Ok(value)) => {
                context.value.write(value);
                0
            }
            Ok(Err(status)) => status,
            Err(panic) => {
                context.panic = Some(panic);
                i32::MIN
            }
        }
    }
    let mut context = Context {
        f,
        value: std::mem::MaybeUninit::uninit(),
        panic: None,
    };
    let result = unsafe {
        svc.guard_scope(
            scope,
            (&mut context as *mut Context<T, F>).cast(),
            invoke::<T, F>,
        )
    };
    if let Some(panic) = context.panic {
        std::panic::resume_unwind(panic);
    }
    result?;
    Ok(unsafe { context.value.assume_init() })
}

/// Native assertions report checked failure; they never unwind through Rust.
pub(crate) unsafe fn check(
    svc: &Hp2LevTicServices,
    ok: bool,
    expr: &std::ffi::CStr,
    line: i32,
) -> Result<(), i32> {
    if !ok && unsafe { svc.check_enabled()? } != 0 {
        unsafe { svc.check_failed(expr.as_ptr(), line)? };
    }
    Ok(())
}

/// `FTime` fixed-point scale (`Core.h:29`, `FIXTIME`).
const FIXTIME: f32 = 4_294_967_296.0;

/// `FTime(float)`: `FromSeconds(long double)`, truncating and saturating.
pub fn ftime_from_f32(seconds: f32) -> i64 {
    // f32 * 2^32 is exact in f64; `as i64` truncates toward zero and saturates.
    (seconds as f64 * 4_294_967_296.0) as i64
}

/// `FTime::GetFloat()`: `v / FIXTIME` in float.
pub fn ftime_get_float(v: i64) -> f32 {
    (v as f32) / FIXTIME
}

/// `FTime::operator+(float)`: `v + (TIMETYP)(f * FIXTIME)`.
pub fn ftime_add_f32(v: i64, f: f32) -> i64 {
    v.wrapping_add((f * FIXTIME) as i64)
}

/// `FTime::operator-(FTime)`: `(v - t.v) / FIXTIME` in float.
pub fn ftime_sub(a: i64, b: i64) -> f32 {
    (a.wrapping_sub(b) as f32) / FIXTIME
}

/// `FActorPriority` entry. `Compare` is `B->Priority - A->Priority`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ActorPriority {
    pub priority: i32,
    pub actor: *mut c_void,
    pub channel: *mut c_void,
}

/// `Compare(const FActorPriority* A, const FActorPriority* B)`.
#[inline]
pub fn compare_priority(a: &ActorPriority, b: &ActorPriority) -> i32 {
    b.priority.wrapping_sub(a.priority)
}

/// `Sort<T>(T* First, INT Num)` (`UnTemplate.h:1727`): a non-stable quicksort
/// with a bubble-sort tail for ranges of 8 or fewer.
pub fn sort_priority(v: &mut [ActorPriority]) {
    sort_by(v, compare_priority);
}

/// Sort the original `FActorPriority**` index array without copying its entries.
pub fn sort_priority_ptrs(v: &mut [*mut ActorPriority]) {
    sort_by(v, |a, b| unsafe { compare_priority(&**a, &**b) });
}

fn sort_by<T>(v: &mut [T], compare: impl Fn(&T, &T) -> i32) {
    let count = v.len() as isize;
    let ptr = v.as_mut_ptr();
    let _: Result<(), std::convert::Infallible> = hp2_core::sort::quicksort(
        count,
        |_| Ok(()),
        |i, j| unsafe { Ok(compare(&*ptr.add(i as usize), &*ptr.add(j as usize))) },
        |i, j| unsafe { std::ptr::swap(ptr.add(i as usize), ptr.add(j as usize)) },
    );
}

/// `AActor::IsOwnedBy` (`UnActor.h:44`): walk the Owner chain.
unsafe fn owned_by(this: *mut c_void, test: *mut c_void) -> bool {
    let mut arg = this;
    while !arg.is_null() {
        if arg == test {
            return true;
        }
        arg = unsafe { (*arg.cast::<AActor>()).owner };
    }
    false
}

/// `AActor::WorldSoundRadius` is shared with the public actor ABI wrapper.

/// `FActorPriority(ViewPos, ViewDir, InConnection, InActor)` (`UnLevTic.cpp:91-131`).
///
/// # Safety
/// `conn` and `actor` must be live engine objects.
pub(crate) unsafe fn make_priority_with<O: crate::lev_tic_net::LevelTickNetOps>(
    ops: &O,
    view_pos: FVector,
    view_dir: FVector,
    conn: *mut c_void,
    actor: *mut c_void,
) -> Result<ActorPriority, O::Error> {
    unsafe {
        ops.guard(12, |ops| unsafe {
            let a = actor.cast::<AActor>();
            let channel = ops.conn_actor_channel(conn, actor)?;
            let driver = ops.conn_driver(conn)?;
            let time = if channel.is_null() {
                ops.driver_spawn_priority_seconds(driver)?
            } else {
                ftime_sub(
                    ops.driver_time(driver)?,
                    ops.channel_last_update_time(channel)?,
                )
            };
            let mut dot_val: f32 = 0.0;
            if (*a).b_always_relevant() {
                dot_val = 0.0;
            } else if (*ops.conn_actor(conn)?.cast::<APawn>()).weapon == actor {
                dot_val = 1.0;
            } else if (*a).remote_role > 1 || (*a).physics == 0 {
                let dir = FVector {
                    x: (*a).location.x - view_pos.x,
                    y: (*a).location.y - view_pos.y,
                    z: (*a).location.z - view_pos.z,
                };
                dot_val = dot(view_dir, dir);
                if dot_val < 0.0 {
                    dot_val = -1.0;
                } else {
                    dot_val = dot(view_dir, safe_normal(dir));
                }
            }
            let sent = if !channel.is_null() && ops.channel_recent_count(channel)? != 0 {
                ops.channel_recent_first(channel)?
            } else {
                ptr::null_mut()
            };
            let net = ops.actor_net_priority(actor, sent, time, ops.conn_best_lag(conn)?)?;
            let mut priority = app_round(65536.0_f32 * (3.0_f32 + dot_val) * net);
            if (*a).b_net_optional() {
                priority = priority.wrapping_sub(3_000_000);
            }
            Ok(ActorPriority {
                priority,
                actor,
                channel,
            })
        })
    }
}

/// `ActorCanSee` (`UnLevTic.cpp:697-713`). The `0.3` literal is double, so the
/// comparison is done in f64.
///
/// # Safety
/// All pointers must be live engine objects (or null where C++ allows).
pub(crate) unsafe fn actor_can_see_with<O: crate::lev_tic_net::LevelTickNetOps>(
    ops: &O,
    actor: *mut c_void,
    real_viewer: *mut c_void,
    viewer: *mut c_void,
    src_location: FVector,
) -> Result<bool, O::Error> {
    unsafe {
        ops.guard(13, |ops| unsafe {
            let a = actor.cast::<AActor>();
            if (*a).b_always_relevant()
                || owned_by(actor, viewer)
                || owned_by(actor, real_viewer)
                || actor == viewer
                || actor == real_viewer
                || viewer == (*a).instigator
            {
                return Ok(true);
            }
            let v = viewer.cast::<AActor>();
            let d = FVector {
                x: (*a).location.x - (*v).location.x,
                y: (*a).location.y - (*v).location.y,
                z: (*a).location.z - (*v).location.z,
            };
            let wsr = crate::actor::world_sound_radius(&*a);
            if !(*a).ambient_sound.is_null()
                && f64::from(size_squared(d)) < 0.3_f64 * f64::from(wsr) * f64::from(wsr)
            {
                return Ok(true);
            }
            if !(*a).owner.is_null()
                && (*(*a).owner.cast::<AActor>()).b_is_pawn()
                && actor == (*(*a).owner.cast::<APawn>()).weapon
            {
                return Ok(actor_can_see_with(
                    ops,
                    (*a).owner,
                    real_viewer,
                    viewer,
                    src_location,
                )?);
            }
            if ((*a).b_hidden() || (*a).b_only_owner_see())
                && !(*a).b_block_players()
                && (*a).ambient_sound.is_null()
            {
                return Ok(false);
            }
            Ok(ops.level_fast_line_check(
                (*a).x_level,
                &(*a).location as *const FVector as *const f32,
                &src_location as *const FVector as *const f32,
            )? != 0)
        })
    }
}

/// `Info->AvgAITime` update (`UnLevTic.cpp:1064`, `ULevel::Tick`). The C++
/// `0.95f * AvgAITime + t` compiles to one `fmadd` (`dis.txt` 0x4c6c), so
/// `mul_add` fuses the same way. `t` is the separately rounded
/// `0.05f * 1000.f * GSecondsPerCycle * ActorTickCycles`.
unsafe fn level_update_avg_ai(gspc: f32, lv: *mut ULevel, info: *mut ALevelInfo) {
    unsafe {
        let t = 0.05f32 * 1000.0f32 * gspc * ((*lv).actor_tick_cycles as f32);
        (*info).avg_ai_time = (*info).avg_ai_time.mul_add(0.95f32, t);
    }
}

/// `Info->AIProfile[Clamp(appRound(10 * GSec * Cycles / Delta), 0, 7)] += 1`
/// (`UnLevTic.cpp:1065`). `appRound` truncates (`app_round`); `Clamp` is
/// `i32::clamp(0, 7)`. The C++ `int +=` wraps in practice; `wrapping_add` avoids a debug panic.
unsafe fn level_bump_ai_profile(gspc: f32, lv: *mut ULevel, info: *mut ALevelInfo, delta: f32) {
    unsafe {
        let x = app_round(10.0f32 * gspc * ((*lv).actor_tick_cycles as f32) / delta).clamp(0, 7);
        let slot = &mut (*info).ai_profile[x as usize];
        *slot = slot.wrapping_add(1);
    }
}

/// `FRotator::Vector()` = `(GMath.UnitCoords / *this).XAxis` (`UnMath.h:2658`).
/// Sin/cos come from the GMath tables, so no libm call.
/// `Info->AvgAITime` update (`UnLevTic.cpp:1064`, `ULevel::Tick`). The C++
/// `0.95f * AvgAITime + t` compiles to one `fmadd` (`dis.txt` 0x4c6c), so
/// `mul_add` fuses the same way. `t` is the separately rounded
/// `0.05f * 1000.f * GSecondsPerCycle * ActorTickCycles`.
/// `Info->AIProfile[Clamp(appRound(10 * GSec * Cycles / Delta), 0, 7)] += 1`
/// (`UnLevTic.cpp:1065`). `appRound` truncates (`app_round`); `Clamp` is
/// `i32::clamp(0, 7)`. The C++ `int +=` wraps in practice; `wrapping_add` avoids a debug panic.
/// `FRotator::Vector()` = `(GMath.UnitCoords / *this).XAxis` (`UnMath.h:2658`).
/// Sin/cos come from the GMath tables, so no libm call.
pub(crate) unsafe fn rotator_vector_with<O: crate::lev_tic_net::LevelTickNetOps>(
    ops: &O,
    rot: FRotator,
) -> Result<FVector, O::Error> {
    unsafe {
        let mut c = coords::unit_coords();
        let cr = ops.gmath_cos_tab(rot.roll)?;
        let sr = ops.gmath_sin_tab(rot.roll)?;
        coords::mul_coords(
            &mut c,
            &hp2_abi::FCoords {
                origin: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                x_axis: FVector {
                    x: 1.0,
                    y: -0.0,
                    z: 0.0,
                },
                y_axis: FVector {
                    x: -0.0,
                    y: cr,
                    z: sr,
                },
                z_axis: FVector {
                    x: 0.0,
                    y: -sr,
                    z: cr,
                },
            },
        );
        let cp = ops.gmath_cos_tab(rot.pitch)?;
        let sp = ops.gmath_sin_tab(rot.pitch)?;
        coords::mul_coords(
            &mut c,
            &hp2_abi::FCoords {
                origin: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                x_axis: FVector {
                    x: cp,
                    y: 0.0,
                    z: -sp,
                },
                y_axis: FVector {
                    x: 0.0,
                    y: 1.0,
                    z: -0.0,
                },
                z_axis: FVector {
                    x: sp,
                    y: 0.0,
                    z: cp,
                },
            },
        );
        let cy = ops.gmath_cos_tab(rot.yaw)?;
        let sy = ops.gmath_sin_tab(rot.yaw)?;
        coords::mul_coords(
            &mut c,
            &hp2_abi::FCoords {
                origin: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                x_axis: FVector {
                    x: cy,
                    y: -sy,
                    z: -0.0,
                },
                y_axis: FVector {
                    x: sy,
                    y: cy,
                    z: 0.0,
                },
                z_axis: FVector {
                    x: -0.0,
                    y: 0.0,
                    z: 1.0,
                },
            },
        );
        Ok(c.x_axis)
    }
}

/// `ULevel::Tick` (`UnLevTic.cpp:1043-1200`). `tick_type` is `ELevelTick`.
///
/// # Safety
/// `level` is a live `ULevel*` of the engine; `svc` is a valid services table.
pub unsafe fn level_tick(
    svc: &Hp2LevTicServices,
    level: *mut c_void,
    tick_type: i32,
    delta_seconds: f32,
    frame_start_storage: *mut i64,
) -> Result<(), i32> {
    unsafe {
        let frame_timing = svc.level_frame_timing_enabled()? != 0;
        let frame_start = if frame_timing {
            svc.level_frame_timing_now()?
        } else {
            0
        };
        // The native FTime stays alive until ULevel::Tick exits. Its sole field
        // has the same signed 64-bit representation as this checked time sample.
        frame_start_storage.write(frame_start);
        let info = svc.level_info(level)?;
        svc.level_init_stats(level)?;
        svc.level_tick_scoped(
            level,
            tick_type,
            delta_seconds,
            info,
            frame_timing as i32,
            frame_start,
        )?;
        Ok(())
    }
}

/// Runs with the two native FMemMark objects live; failures do not synthesize Pop.
pub(crate) trait LevelTickOps {
    type Error;

    unsafe fn guarded<T, F>(&mut self, scope: i32, f: F) -> Result<T, Self::Error>
    where
        F: FnOnce(&mut Self) -> Result<T, Self::Error>;
    unsafe fn init_runaway(&mut self) -> Result<(), Self::Error>;
    unsafe fn seconds_per_cycle(&mut self) -> Result<f32, Self::Error>;
    unsafe fn driver_tick(&mut self, driver: *mut c_void, delta: f32)
        -> Result<(), Self::Error>;
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn connection_open(&mut self, connection: *mut c_void)
        -> Result<bool, Self::Error>;
    unsafe fn tick_net_client(
        &mut self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn tick_demo_playback(
        &mut self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn hash_tick(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn update_time(&mut self, info: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn players_only(&mut self, info: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn pauser_empty(&mut self, info: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn actor_cycles_begin(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_cycles_end(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn startup_trace(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_count(&mut self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_at(
        &mut self,
        level: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn tick_actor(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn clear_newly_spawned(&mut self, level: *mut c_void);
    unsafe fn newly_spawned(&mut self, level: *mut c_void) -> *mut c_void;
    unsafe fn link_actor(&mut self, link: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn link_next(&mut self, link: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_ticked(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn actor_always_tick(&mut self, actor: *mut c_void) -> bool;
    unsafe fn player_pawn(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn pawn_player(&mut self, actor: *mut c_void) -> *mut c_void;
    unsafe fn player_read_input(
        &mut self,
        player: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn player_input_event(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn zero_input_floats(&mut self, actor: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn tick_net_server(
        &mut self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn driver_flush(&mut self, driver: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn tick_demo_record(
        &mut self,
        level: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn finish(
        &mut self,
        level: *mut c_void,
        mark: *mut c_void,
        engine_mark: *mut c_void,
    ) -> Result<(), Self::Error>;
    unsafe fn frame_timing_log(&mut self, frame_start: i64) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn level_tick_body_with<O: LevelTickOps>(
    ops: &mut O,
    level: *mut c_void,
    mut tick_type: i32,
    delta_seconds: f32,
    info: *mut c_void,
    frame_timing: i32,
    frame_start: i64,
    mark: *mut c_void,
    engine_mark: *mut c_void,
) -> Result<(), O::Error> {
    const KIND_APLAYERPAWN: i32 = 1;
    const LEVELTICK_TIME_ONLY: i32 = 0;
    const LEVELTICK_VIEWPORTS_ONLY: i32 = 1;
    unsafe {
        let lv = level.cast::<ULevel>();
        ops.init_runaway()?;
        (*lv).in_tick = 1;
        let gspc = ops.seconds_per_cycle()?;
        level_update_avg_ai(gspc, lv, info.cast::<ALevelInfo>());
        level_bump_ai_profile(gspc, lv, info.cast::<ALevelInfo>(), delta_seconds);

        let mut delta = delta_seconds;
        ops.guarded(0, |ops| {
            let net = (*lv).base.net_driver;
            if !net.is_null() {
                unsafe { ops.driver_tick(net, delta)? };
                if !unsafe { ops.driver_server_connection((*lv).base.net_driver)? }.is_null() {
                    unsafe { ops.tick_net_client(level, delta)? };
                }
            }
            Ok(())
        })?;
        ops.guarded(1, |ops| {
            let demo = (*lv).base.demo_rec_driver;
            if !demo.is_null() {
                unsafe { ops.driver_tick(demo, delta)? };
                if !unsafe {
                    ops.driver_server_connection((*lv).base.demo_rec_driver)?
                }
                .is_null()
                {
                    unsafe { ops.tick_demo_playback(level, delta)? };
                }
            }
            Ok(())
        })?;
        ops.guarded(2, |ops| {
            if !(*lv).hash.is_null() {
                unsafe { ops.hash_tick(level)? };
            }
            Ok(())
        })?;
        ops.guarded(3, |ops| {
            delta = if delta <= 0.10 { delta } else { 0.10 };
            let info_ref = info.cast::<ALevelInfo>();
            delta *= (*info_ref).time_dilation;
            let time = ftime_add_f32(i64::from_ne_bytes((*lv).time_seconds), delta);
            (*lv).time_seconds = time.to_ne_bytes();
            (*info_ref).time_seconds = ftime_get_float(time);
            unsafe { ops.update_time(info)? };
            tick_type = if unsafe { ops.players_only(info)? } {
                LEVELTICK_VIEWPORTS_ONLY
            } else {
                tick_type
            };
            Ok(())
        })?;

        ops.actor_cycles_begin(level)?;
        if tick_type != LEVELTICK_TIME_ONLY
            && ops.pauser_empty(info)?
            && {
                (*lv).base.net_driver.is_null()
                    || {
                        let conn = ops.driver_server_connection((*lv).base.net_driver)?;
                        conn.is_null() || ops.connection_open(conn)?
                    }
            }
        {
            ops.guarded(4, |ops| {
                unsafe { ops.clear_newly_spawned(level) };
                unsafe { ops.startup_trace(level)? };
                let mut updated = 0;
                let mut i = (*lv).i_first_dynamic_actor;
                while i < unsafe { ops.actor_count(level)? } {
                    let actor = unsafe { ops.actor_at(level, i)? };
                    if !actor.is_null() {
                        updated += unsafe { ops.tick_actor(actor, delta, tick_type)? };
                    }
                    i += 1;
                }
                while !(*lv).newly_spawned.is_null() && updated != 0 {
                    let mut link = unsafe { ops.newly_spawned(level) };
                    unsafe { ops.clear_newly_spawned(level) };
                    updated = 0;
                    while !link.is_null() {
                        let actor = unsafe { ops.link_actor(link)? };
                        if unsafe { ops.actor_ticked(actor)? } != ((*lv).ticked != 0) {
                            updated += unsafe { ops.tick_actor(actor, delta, tick_type)? };
                        }
                        link = unsafe { ops.link_next(link)? };
                    }
                }
                Ok(())
            })?;
        } else if !ops.pauser_empty(info)? {
            ops.guarded(5, |ops| {
                let mut i = (*lv).i_first_dynamic_actor;
                while i < unsafe { ops.actor_count(level)? } {
                    let actor = unsafe { ops.actor_at(level, i)? };
                    let pawn = if !actor.is_null() && unsafe { ops.player_pawn(actor)? } {
                        actor.cast::<APlayerPawn>()
                    } else {
                        ptr::null_mut()
                    };
                    let player = if pawn.is_null() {
                        ptr::null_mut()
                    } else {
                        unsafe { ops.pawn_player(actor) }
                    };
                    if !player.is_null() {
                        // Clear only mouse history before script callbacks: MenuTick can
                        // unpause or destroy the pawn during PlayerInput.
                        unsafe {
                            (*pawn).smooth_mouse_x = 0.0;
                            (*pawn).smooth_mouse_y = 0.0;
                            (*pawn).borrowed_mouse_x = 0.0;
                            (*pawn).borrowed_mouse_y = 0.0;
                            ops.player_read_input(player, delta)?;
                        }
                        unsafe { ops.player_input_event(actor, delta)? };
                        unsafe { ops.zero_input_floats(actor)? };
                    } else if !actor.is_null() && unsafe { ops.actor_always_tick(actor) } {
                        unsafe { ops.tick_actor(actor, delta, tick_type)? };
                    }
                    i += 1;
                }
                Ok(())
            })?;
        }
        ops.actor_cycles_end(level)?;

        ops.guarded(6, |ops| {
            let net = (*lv).base.net_driver;
            if !net.is_null() {
                if unsafe { ops.driver_server_connection((*lv).base.net_driver)? }.is_null() {
                    unsafe { ops.tick_net_server(level, delta)? };
                }
                unsafe { ops.driver_flush((*lv).base.net_driver)? };
            }
            Ok(())
        })?;
        ops.guarded(7, |ops| {
            let demo = (*lv).base.demo_rec_driver;
            if !demo.is_null() {
                if unsafe { ops.driver_server_connection((*lv).base.demo_rec_driver)? }.is_null()
                {
                    unsafe { ops.tick_demo_record(level, delta)? };
                }
                unsafe { ops.driver_flush((*lv).base.demo_rec_driver)? };
            }
            Ok(())
        })?;
        (*lv).ticked = ((*lv).ticked == 0) as i32;
        (*lv).in_tick = 0;
        ops.finish(level, mark, engine_mark)?;
        if frame_timing != 0 {
            ops.frame_timing_log(frame_start)?;
        }
        Ok(())
    }
}

struct NativeLevelTickOps<'a> {
    svc: &'a Hp2LevTicServices,
}

impl LevelTickOps for NativeLevelTickOps<'_> {
    type Error = i32;

    unsafe fn guarded<T, F>(&mut self, scope: i32, f: F) -> Result<T, i32>
    where
        F: FnOnce(&mut Self) -> Result<T, i32>,
    {
        let svc = self.svc;
        let mut f = Some(f);
        unsafe { guarded(svc, scope, || f.take().unwrap()(self)) }
    }
    unsafe fn init_runaway(&mut self) -> Result<(), i32> {
        unsafe { self.svc.level_init_runaway() }
    }
    unsafe fn seconds_per_cycle(&mut self) -> Result<f32, i32> {
        unsafe { self.svc.level_gseconds_per_cycle() }
    }
    unsafe fn driver_tick(&mut self, driver: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.driver_tick_dispatch(driver, delta) }
    }
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.driver_server_connection(driver) }
    }
    unsafe fn connection_open(&mut self, connection: *mut c_void) -> Result<bool, i32> {
        unsafe { self.svc.conn_state(connection).map(|state| state == 3) }
    }
    unsafe fn tick_net_client(&mut self, level: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.level_tick_net_client(level, delta) }
    }
    unsafe fn tick_demo_playback(&mut self, level: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.level_tick_demo_playback(level, delta) }
    }
    unsafe fn hash_tick(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_hash_tick(level) }
    }
    unsafe fn update_time(&mut self, info: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_update_time(info) }
    }
    unsafe fn players_only(&mut self, info: *mut c_void) -> Result<bool, i32> {
        unsafe {
            self.svc
                .level_info_players_only(info)
                .map(|value| value != 0)
        }
    }
    unsafe fn pauser_empty(&mut self, info: *mut c_void) -> Result<bool, i32> {
        unsafe {
            self.svc
                .level_info_pauser_empty(info)
                .map(|value| value != 0)
        }
    }
    unsafe fn actor_cycles_begin(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_cycles_clock(level, 0) }
    }
    unsafe fn actor_cycles_end(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_cycles_unclock(level, 0) }
    }
    unsafe fn startup_trace(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_startup_phase_trace(level) }
    }
    unsafe fn actor_count(&mut self, level: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.level_actor_count(level) }
    }
    unsafe fn actor_at(&mut self, level: *mut c_void, index: i32) -> Result<*mut c_void, i32> {
        unsafe { self.svc.level_actor(level, index) }
    }
    unsafe fn tick_actor(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
    ) -> Result<i32, i32> {
        unsafe {
            self.svc.tick_actor_scoped(
                actor,
                delta,
                tick_type,
                ptr::null(),
                c"dynamic".as_ptr(),
            )
        }
    }
    unsafe fn clear_newly_spawned(&mut self, level: *mut c_void) {
        unsafe {
            (*level.cast::<ULevel>()).newly_spawned = ptr::null_mut();
        }
    }
    unsafe fn newly_spawned(&mut self, level: *mut c_void) -> *mut c_void {
        unsafe { (*level.cast::<ULevel>()).newly_spawned }
    }
    unsafe fn link_actor(&mut self, link: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { self.svc.link_actor(link) }
    }
    unsafe fn link_next(&mut self, link: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { self.svc.link_next(link) }
    }
    unsafe fn actor_ticked(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_ticked() })
    }
    unsafe fn actor_always_tick(&mut self, actor: *mut c_void) -> bool {
        unsafe { (*actor.cast::<AActor>()).b_always_tick() }
    }
    unsafe fn player_pawn(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        unsafe { self.svc.actor_is_a(actor, 1).map(|value| value != 0) }
    }
    unsafe fn pawn_player(&mut self, actor: *mut c_void) -> *mut c_void {
        unsafe { (*actor.cast::<APlayerPawn>()).player }
    }
    unsafe fn player_read_input(&mut self, player: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.player_read_input(player, delta) }
    }
    unsafe fn player_input_event(&mut self, actor: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.actor_event(actor, 8, delta).map(|_| ()) }
    }
    unsafe fn zero_input_floats(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.pawn_zero_input_floats(actor) }
    }
    unsafe fn tick_net_server(&mut self, level: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.level_tick_net_server(level, delta) }
    }
    unsafe fn driver_flush(&mut self, driver: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.driver_tick_flush(driver) }
    }
    unsafe fn tick_demo_record(&mut self, level: *mut c_void, delta: f32) -> Result<(), i32> {
        unsafe { self.svc.level_tick_demo_record(level, delta) }
    }
    unsafe fn finish(
        &mut self,
        level: *mut c_void,
        mark: *mut c_void,
        engine_mark: *mut c_void,
    ) -> Result<(), i32> {
        unsafe {
            self.svc.mem_mark_pop(mark)?;
            self.svc.mem_mark_pop(engine_mark)?;
            self.svc.level_cleanup_destroyed(level)
        }
    }
    unsafe fn frame_timing_log(&mut self, frame_start: i64) -> Result<(), i32> {
        unsafe { self.svc.level_frame_timing_log(frame_start) }
    }
}

pub unsafe fn level_tick_body(
    svc: &Hp2LevTicServices,
    level: *mut c_void,
    tick_type: i32,
    delta_seconds: f32,
    info: *mut c_void,
    frame_timing: i32,
    frame_start: i64,
    mark: *mut c_void,
    engine_mark: *mut c_void,
) -> Result<(), i32> {
    unsafe {
        level_tick_body_with(
            &mut NativeLevelTickOps { svc },
            level,
            tick_type,
            delta_seconds,
            info,
            frame_timing,
            frame_start,
            mark,
            engine_mark,
        )
    }
}


/// Engine-owned state-frame values used by `AActor::ProcessState`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2ProcessStateSnapshot {
    pub state_node: *mut c_void,
    pub has_frame: i32,
    pub has_code: i32,
    pub role: i32,
    pub simulated: i32,
    pub pending_kill: i32,
    pub delete_me: i32,
    pub latent_action: i32,
}

/// Narrow VM/instrumentation leaves. Every callback catches C++ exceptions and
/// returns a status; failure stops dispatch before any further engine calls.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ProcessStateServices {
    pub context: *mut c_void,
    /// `read_gate` is one only for initial dispatch; VM snapshots need no role,
    /// pending-kill or state-flag queries.
    pub snapshot: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2ProcessStateSnapshot) -> i32,
    pub trace: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> i32,
    /// Enters the C++ guard and random-trace scope, then calls
    /// `process_state_steps` through its guarded FFI export.
    pub run_scoped: unsafe extern "C" fn(*mut c_void, f32) -> i32,
    pub poll_latent: unsafe extern "C" fn(*mut c_void, f32) -> i32,
    pub step: unsafe extern "C" fn(*mut c_void) -> i32,
    pub observe: unsafe extern "C" fn(*mut c_void, f32) -> i32,
}

/// Dispatch policy and outcome tracing from `AActor::ProcessState`.
///
/// # Safety
/// Services and their context must remain live for this call. Callbacks must
/// contain exceptions and `run_scoped` must execute `process_state_steps`.
pub unsafe fn process_state(svc: &Hp2ProcessStateServices, delta: f32) -> i32 {
    unsafe {
        let mut state = Hp2ProcessStateSnapshot::default();
        let mut status = (svc.snapshot)(svc.context, 1, &mut state);
        if status != 0 {
            return status;
        }
        let (outcome, dispatch) = if state.has_frame == 0 {
            (c"skip_no_frame", false)
        } else if state.has_code == 0 {
            (c"skip_no_code", false)
        } else if state.role < 4 && state.simulated == 0 {
            (c"skip_role", false)
        } else if state.pending_kill != 0 {
            (c"skip_pending_kill", false)
        } else {
            (c"dispatched", true)
        };
        status = (svc.trace)(svc.context, state.state_node, outcome.as_ptr());
        if status != 0 {
            return status;
        }
        if dispatch {
            let _script_scope = hp2_core::timing_globals::script_scope();
            status = (svc.run_scoped)(svc.context, delta);
            drop(_script_scope);
            if status != 0 {
                return status;
            }
        }
        // Every normal return, including every skip, reaches the observer.
        // An engine exception instead returns to the C++ rethrow site.
        (svc.observe)(svc.context, delta)
    }
}

/// Latent polling and VM loop policy, called inside the Rust-owned script timing
/// scope and native exception/random-trace scopes. `old_state` is the C++ guard's
/// diagnostic state identity.
///
/// # Safety
/// The actor must have a state frame, services must contain exceptions, and
/// `old_state` must be writable for this call without aliasing the engine frame.
pub unsafe fn process_state_steps(
    svc: &Hp2ProcessStateServices,
    delta: f32,
    old_state: *mut *mut c_void,
) -> i32 {
    unsafe {
        let mut state = Hp2ProcessStateSnapshot::default();
        let mut status = (svc.snapshot)(svc.context, 0, &mut state);
        if status != 0 {
            return status;
        }
        if state.latent_action != 0 {
            status = (svc.poll_latent)(svc.context, delta);
            if status != 0 {
                return status;
            }
            status = (svc.snapshot)(svc.context, 0, &mut state);
            if status != 0 {
                return status;
            }
        }
        let mut transitions = 0;
        while state.delete_me == 0 && state.has_code != 0 && state.latent_action == 0 {
            status = (svc.step)(svc.context);
            if status != 0 {
                return status;
            }
            status = (svc.snapshot)(svc.context, 0, &mut state);
            if status != 0 {
                return status;
            }
            if state.state_node != *old_state {
                *old_state = state.state_node;
                transitions += 1;
                if transitions > 4 {
                    break;
                }
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avg_ai_update_uses_fused_rounding() {
        let average = f32::from_bits(0x3f80_0001);
        let rounded_product = average * 0.95_f32;
        let gspc = -rounded_product / 50.0_f32;
        let cycles = 1_i32;
        let t = 0.05_f32 * 1000.0_f32 * gspc * cycles as f32;
        let fused = average.mul_add(0.95_f32, t);
        assert_ne!(fused.to_bits(), (average * 0.95_f32 + t).to_bits());

        let mut level = unsafe { std::mem::zeroed::<ULevel>() };
        let mut info = unsafe { std::mem::zeroed::<ALevelInfo>() };
        level.actor_tick_cycles = cycles;
        info.avg_ai_time = average;
        unsafe {
            level_update_avg_ai(gspc, &mut level, &mut info);
        }
        assert_eq!(info.avg_ai_time.to_bits(), fused.to_bits());
    }
}

/// `FRotator::Vector()` through the native service table.
///
/// # Safety
/// `svc` is a valid services table.
pub unsafe fn rotator_vector(svc: &Hp2LevTicServices, rot: FRotator) -> Result<FVector, i32> {
    unsafe { rotator_vector_with(&crate::lev_tic_net::NativeNetTickOps(svc), rot) }
}

/// `FActorPriority(...)` through the native service table.
///
/// # Safety
/// `svc` is valid; pointers are live engine objects.
pub unsafe fn make_priority(
    svc: &Hp2LevTicServices,
    view_pos: FVector,
    view_dir: FVector,
    conn: *mut c_void,
    actor: *mut c_void,
) -> Result<ActorPriority, i32> {
    unsafe { make_priority_with(&crate::lev_tic_net::NativeNetTickOps(svc), view_pos, view_dir, conn, actor) }
}

/// `ActorCanSee(...)` through the native service table.
///
/// # Safety
/// `svc` is valid; pointers are live engine objects.
pub unsafe fn actor_can_see(
    svc: &Hp2LevTicServices,
    actor: *mut c_void,
    real_viewer: *mut c_void,
    viewer: *mut c_void,
    src_location: FVector,
) -> Result<bool, i32> {
    unsafe { actor_can_see_with(&crate::lev_tic_net::NativeNetTickOps(svc), actor, real_viewer, viewer, src_location) }
}
