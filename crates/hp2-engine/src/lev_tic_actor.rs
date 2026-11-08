//! Port of `AActor::Tick` (`HarryPotter2/Unreal/Engine/Src/UnLevTic.cpp`).
//!
//! Oracle: `/tmp/EngLevTic2/orig_UnLevTic.cpp` lines 137-661. The policy body is
//! shared by the native service adapter ([`actor_tick`]) and the canonical Core
//! host (`crate::actor_tick_owner`). Every engine side effect (virtual
//! Tick/event dispatch, physics, traces, animation callbacks, line checks,
//! process-state) goes through [`ActorTickOps`] in the original call order.
//! Plain mirror fields are read and written directly.

use std::ffi::c_void;
use std::ptr;

use crate::actor;
use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, APlayerPawn, AWeapon, AZoneInfo, ULevel,
};

use crate::lev_tic::{Hp2LevTicServices, Hp2TickDispatch};
use crate::math::app_round;
use crate::prim::dot;
use crate::vec::size;

// Enum values mirrored from `Engine/Inc/Hp2LevTicServices.h`.
pub(crate) const EVENT_TICK: i32 = 0;
pub(crate) const EVENT_TIMER: i32 = 1;
pub(crate) const EVENT_EXPIRED: i32 = 2;
pub(crate) const EVENT_ANIM_END: i32 = 3;
pub(crate) const EVENT_PAIN_TIMER: i32 = 4;
pub(crate) const EVENT_SPEECH_TIMER: i32 = 5;
pub(crate) const EVENT_UPDATE_TACTICS: i32 = 6;
pub(crate) const EVENT_UPDATE_EYE_HGT: i32 = 7;
pub(crate) const EVENT_PLAYER_INPUT: i32 = 8;
pub(crate) const EVENT_PLAYER_TICK: i32 = 9;

pub(crate) const KIND_APAWN: i32 = 0;
pub(crate) const KIND_APLAYERPAWN: i32 = 1;
pub(crate) const KIND_ACAMERA: i32 = 3;
pub(crate) const KIND_AWEAPON: i32 = 4;

pub(crate) const PROBE_TICK: i32 = 0;
pub(crate) const PROBE_ENEMY_NOT_VIS: i32 = 1;

// Engine constants (`Engine/Inc/*.h`).
pub(crate) const ROLE_SIMULATED_PROXY: u8 = 2;
pub(crate) const ROLE_AUTONOMOUS_PROXY: u8 = 3;
pub(crate) const ROLE_AUTHORITY: u8 = 4;
pub(crate) const PHYS_NONE: u8 = 0;
pub(crate) const PHYS_FALLING: u8 = 2;
pub(crate) const PHYS_ROTATING: u8 = 5;
pub(crate) const NM_STANDALONE: u8 = 0;
pub(crate) const SHOW_PLAYER_CTRL: i32 = 0x0000_0800;
pub(crate) const LEVELTICK_VIEWPORTS_ONLY: i32 = 1;
pub(crate) const LEVELTICK_ALL: i32 = 2;
pub(crate) const TRACE_VIS_BLOCKING: u32 = 0x06;
pub(crate) const EPOLL_FINISH_ANIM: i32 = 385;

/// Typed engine operations used by the shared `AActor::Tick` policy.
///
/// The native adapter forwards each call to `Hp2LevTicServices`; the canonical
/// Core host implements the same leaves over `CoreAccess` records.
pub(crate) trait ActorTickOps {
    type Error;

    /// Records that the probing Tick event was dispatched (native trace only).
    unsafe fn event_tick(&mut self) -> Result<(), Self::Error>;
    fn set_skip_reason(&mut self, reason: &'static [u8]);
    unsafe fn check(
        &mut self,
        ok: bool,
        expr: &std::ffi::CStr,
        line: i32,
    ) -> Result<(), Self::Error>;

    unsafe fn actor_is_a(&mut self, actor: *mut c_void, kind: i32) -> Result<i32, Self::Error>;
    unsafe fn cylinder_extent(&mut self, actor: *mut c_void) -> Result<FVector, Self::Error>;
    unsafe fn actor_event(
        &mut self,
        actor: *mut c_void,
        event: i32,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn actor_process_state(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn actor_perform_physics(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn actor_move_smooth(
        &mut self,
        actor: *mut c_void,
        delta: *const f32,
    ) -> Result<(), Self::Error>;
    unsafe fn actor_is_probing(
        &mut self,
        actor: *mut c_void,
        probe: i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn actor_creature_trace_tick(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn actor_latent_action(&mut self, actor: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_show_self(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_check_enemy_visible(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;

    unsafe fn player_read_input(
        &mut self,
        player: *mut c_void,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn pawn_begin_modern_tps(
        &mut self,
        pawn: *mut c_void,
        raw_base_x: &mut f32,
        raw_strafe: &mut f32,
        raw_camera_yaw: &mut f32,
        raw_camera_pitch: &mut f32,
    ) -> Result<i32, Self::Error>;
    unsafe fn pawn_finish_modern_tps(
        &mut self,
        pawn: *mut c_void,
        active: i32,
        raw_base_x: f32,
        raw_strafe: f32,
        raw_camera_yaw: f32,
        raw_camera_pitch: f32,
        delta: f32,
    ) -> Result<(), Self::Error>;
    unsafe fn pawn_apply_modern_tps(&mut self, pawn: *mut c_void) -> Result<(), Self::Error>;

    unsafe fn actor_get_anim(
        &mut self,
        actor: *mut c_void,
        sequence_name: *const u8,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn anim_notify_count(&mut self, seq: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn anim_notify_get(
        &mut self,
        seq: *mut c_void,
        index: i32,
        time: &mut f32,
        function: &mut [u8; 4],
    ) -> Result<(), Self::Error>;
    unsafe fn actor_find_function(
        &mut self,
        actor: *mut c_void,
        name: *const u8,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_process_event(
        &mut self,
        actor: *mut c_void,
        function: *mut c_void,
    ) -> Result<(), Self::Error>;

    unsafe fn mesh_get_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        out: &mut [f32; 3],
    ) -> Result<(), Self::Error>;
    unsafe fn mesh_adjust_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        adjust: &[f32; 3],
    ) -> Result<(), Self::Error>;

    #[allow(clippy::too_many_arguments)]
    unsafe fn level_single_line_check(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        end: *const f32,
        start: *const f32,
        trace: u32,
        extent: *const f32,
        hit_time: &mut f32,
        hit_normal: &mut [f32; 3],
    ) -> Result<(), Self::Error>;
    unsafe fn level_far_move_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        location: *const f32,
    ) -> Result<(), Self::Error>;
    #[allow(clippy::too_many_arguments)]
    unsafe fn level_move_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        movement: *const f32,
        rotation: *const i32,
        hit: *mut c_void,
        hit_time: &mut f32,
        hit_normal: &mut [f32; 3],
    ) -> Result<(), Self::Error>;
    unsafe fn level_destroy_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), Self::Error>;
    unsafe fn level_zone_time_since_render(
        &mut self,
        level: *mut c_void,
        zone_number: i32,
    ) -> Result<f32, Self::Error>;
    unsafe fn level_is_editor(&mut self) -> Result<i32, Self::Error>;
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn root_movement_scoped(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    #[allow(clippy::too_many_arguments)]
    unsafe fn tick_actor_scoped(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
        relation: *const std::ffi::c_char,
        root: *const std::ffi::c_char,
    ) -> Result<i32, Self::Error>;
}

/// Native adapter over the checked `Hp2LevTicServices` table.
struct NativeActorTickOps<'a> {
    svc: &'a Hp2LevTicServices,
    dispatch: *mut Hp2TickDispatch,
}

impl ActorTickOps for NativeActorTickOps<'_> {
    type Error = i32;

    unsafe fn event_tick(&mut self) -> Result<(), i32> {
        unsafe {
            (*self.dispatch).event_tick_dispatched = 1;
            if (*self.dispatch).token != u64::MAX {
                self.svc.trace_event_tick((*self.dispatch).token)?;
            }
        }
        Ok(())
    }
    fn set_skip_reason(&mut self, reason: &'static [u8]) {
        unsafe { (*self.dispatch).skip_reason = reason.as_ptr().cast() };
    }
    unsafe fn check(&mut self, ok: bool, expr: &std::ffi::CStr, line: i32) -> Result<(), i32> {
        crate::lev_tic::check(self.svc, ok, expr, line)
    }
    unsafe fn actor_is_a(&mut self, actor: *mut c_void, kind: i32) -> Result<i32, i32> {
        unsafe { self.svc.actor_is_a(actor, kind) }
    }
    unsafe fn cylinder_extent(&mut self, actor: *mut c_void) -> Result<FVector, i32> {
        let mut extent = FVector { x: 0.0, y: 0.0, z: 0.0 };
        unsafe {
            actor::get_cylinder_extent(
                &actor::published_services()?,
                actor.cast::<AActor>(),
                &mut extent,
            )?;
        }
        Ok(extent)
    }
    unsafe fn actor_event(
        &mut self,
        actor: *mut c_void,
        event: i32,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_event(actor, event, delta) }
    }
    unsafe fn actor_process_state(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_process_state(actor, delta) }
    }
    unsafe fn actor_perform_physics(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_perform_physics(actor, delta) }
    }
    unsafe fn actor_move_smooth(
        &mut self,
        actor: *mut c_void,
        delta: *const f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_move_smooth(actor, delta) }
    }
    unsafe fn actor_is_probing(
        &mut self,
        actor: *mut c_void,
        probe: i32,
    ) -> Result<i32, i32> {
        unsafe { self.svc.actor_is_probing(actor, probe) }
    }
    unsafe fn actor_creature_trace_tick(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_creature_trace_tick(actor, delta) }
    }
    unsafe fn actor_latent_action(&mut self, actor: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.actor_latent_action(actor) }
    }
    unsafe fn actor_show_self(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.actor_show_self(actor) }
    }
    unsafe fn actor_check_enemy_visible(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.actor_check_enemy_visible(actor) }
    }
    unsafe fn player_read_input(
        &mut self,
        player: *mut c_void,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.player_read_input(player, delta) }
    }
    unsafe fn pawn_begin_modern_tps(
        &mut self,
        pawn: *mut c_void,
        raw_base_x: &mut f32,
        raw_strafe: &mut f32,
        raw_camera_yaw: &mut f32,
        raw_camera_pitch: &mut f32,
    ) -> Result<i32, i32> {
        unsafe {
            self.svc.pawn_begin_modern_tps(
                pawn,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
            )
        }
    }
    unsafe fn pawn_finish_modern_tps(
        &mut self,
        pawn: *mut c_void,
        active: i32,
        raw_base_x: f32,
        raw_strafe: f32,
        raw_camera_yaw: f32,
        raw_camera_pitch: f32,
        delta: f32,
    ) -> Result<(), i32> {
        unsafe {
            self.svc.pawn_finish_modern_tps(
                pawn,
                active,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
                delta,
            )
        }
    }
    unsafe fn pawn_apply_modern_tps(&mut self, pawn: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.pawn_apply_modern_tps(pawn) }
    }
    unsafe fn actor_get_anim(
        &mut self,
        actor: *mut c_void,
        sequence_name: *const u8,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.actor_get_anim(actor, sequence_name) }
    }
    unsafe fn anim_notify_count(&mut self, seq: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.anim_notify_count(seq) }
    }
    unsafe fn anim_notify_get(
        &mut self,
        seq: *mut c_void,
        index: i32,
        time: &mut f32,
        function: &mut [u8; 4],
    ) -> Result<(), i32> {
        unsafe { self.svc.anim_notify_get(seq, index, time, function.as_mut_ptr()) }
    }
    unsafe fn actor_find_function(
        &mut self,
        actor: *mut c_void,
        name: *const u8,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.actor_find_function(actor, name) }
    }
    unsafe fn actor_process_event(
        &mut self,
        actor: *mut c_void,
        function: *mut c_void,
    ) -> Result<(), i32> {
        unsafe { self.svc.actor_process_event(actor, function) }
    }
    unsafe fn mesh_get_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        out: &mut [f32; 3],
    ) -> Result<(), i32> {
        unsafe { self.svc.mesh_get_root_movement(mesh, actor, out.as_mut_ptr()) }
    }
    unsafe fn mesh_adjust_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        adjust: &[f32; 3],
    ) -> Result<(), i32> {
        unsafe { self.svc.mesh_adjust_root_movement(mesh, actor, adjust.as_ptr()) }
    }
    unsafe fn level_single_line_check(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        end: *const f32,
        start: *const f32,
        trace: u32,
        extent: *const f32,
        hit_time: &mut f32,
        hit_normal: &mut [f32; 3],
    ) -> Result<(), i32> {
        unsafe {
            self.svc.level_single_line_check(
                level,
                actor,
                end,
                start,
                trace,
                extent,
                hit_time,
                hit_normal.as_mut_ptr(),
            ).map(|_| ())
        }
    }
    unsafe fn level_far_move_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        location: *const f32,
    ) -> Result<(), i32> {
        unsafe { self.svc.level_far_move_actor(level, actor, location) }
    }
    unsafe fn level_move_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        movement: *const f32,
        rotation: *const i32,
        hit: *mut c_void,
        hit_time: &mut f32,
        hit_normal: &mut [f32; 3],
    ) -> Result<(), i32> {
        unsafe {
            self.svc.level_move_actor(
                level,
                actor,
                movement,
                rotation,
                hit,
                hit_time,
                hit_normal.as_mut_ptr(),
            ).map(|_| ())
        }
    }
    unsafe fn level_destroy_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        unsafe { self.svc.level_destroy_actor(level, actor) }
    }
    unsafe fn level_zone_time_since_render(
        &mut self,
        level: *mut c_void,
        zone_number: i32,
    ) -> Result<f32, i32> {
        unsafe { self.svc.level_zone_time_since_render(level, zone_number) }
    }
    unsafe fn level_is_editor(&mut self) -> Result<i32, i32> {
        unsafe { self.svc.level_is_editor() }
    }
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.driver_server_connection(driver) }
    }
    unsafe fn root_movement_scoped(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.root_movement_scoped(actor) }
    }
    unsafe fn tick_actor_scoped(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
        relation: *const std::ffi::c_char,
        root: *const std::ffi::c_char,
    ) -> Result<i32, i32> {
        unsafe { self.svc.tick_actor_scoped(actor, delta, tick_type, relation, root) }
    }
}

/// `Max(A, B)` (`UnTemplate.h:111`).
fn ue_max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `if (Pawn) PlayerPawn = Cast<APlayerPawn>(this)`. Returns `this` or null.
unsafe fn player_pawn_of<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    pawn: *mut c_void,
) -> Result<*mut c_void, O::Error> {
    unsafe {
        Ok(
            if !pawn.is_null() && ops.actor_is_a(actor, KIND_APLAYERPAWN)? != 0 {
                actor
            } else {
                ptr::null_mut()
            },
        )
    }
}

/// Copies the view target's view state onto the player pawn (oracle 403-447, 553-575).
unsafe fn copy_view_target_state<O: ActorTickOps>(
    ops: &mut O,
    player_pawn: *mut APlayerPawn,
) -> Result<(), O::Error> {
    unsafe {
        let view_target = (*player_pawn).view_target;
        if ops.actor_is_a(view_target, KIND_APAWN)? == 0 {
            return Ok(());
        }
        let target = view_target.cast::<APawn>();
        (*player_pawn).target_view_rotation = (*target).view_rotation;
        (*player_pawn).target_eye_height = (*target).eye_height;
        let weapon = (*target).weapon;
        if !weapon.is_null() {
            (*player_pawn).target_weapon_view_offset =
                (*weapon.cast::<AWeapon>()).base.player_view_offset;
        }
        Ok(())
    }
}

/// Shared timer block (oracle 415-429 / 519-533): advances `TimerCounter`, and on
/// expiry normalizes it, stops one-shot timers, and fires `eventTimer`.
unsafe fn tick_timer<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        if (*a).timer_rate > 0.0 {
            (*a).timer_counter += delta;
            if (*a).timer_counter >= (*a).timer_rate {
                let ticks = app_round((*a).timer_counter / (*a).timer_rate);
                // TimerCounter -= TimerRate * TimerTicksPassed (fmsub).
                (*a).timer_counter = (-(*a).timer_rate).mul_add(ticks as f32, (*a).timer_counter);
                if ticks != 0 && !(*a).b_timer_loop() {
                    (*a).timer_rate = 0.0;
                }
                ops.actor_event(actor, EVENT_TIMER, 0.0)?;
            }
        }
        Ok(())
    }
}

/// Editor-mode tween (oracle 227-248).
unsafe fn animate_editor_tween<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    animator: *mut c_void,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        if (*a).tween_rate > 0.0 {
            (*a).tween_alpha = (*a).tween_rate.mul_add(delta, (*a).tween_alpha);
            if (*a).tween_alpha >= 1.0 {
                (*a).tween_alpha = 0.0;
                (*a).tween_rate = 0.0;
                if (*a).anim_rate == 0.0 {
                    (*a).set_b_anim_finished(true);
                    if !(*animator.cast::<AActor>()).b_simulated_pawn() {
                        ops.actor_event(animator, EVENT_ANIM_END, 0.0)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Non-editor animation loop (oracle 249-369).
unsafe fn animate_loop<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    animator: *mut c_void,
    simulated_pawn: bool,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        let an = animator.cast::<AActor>();
        let mut seconds = delta;
        let mut iterations = 0_i32;
        loop {
            if !(actor::is_animating(&*a) && seconds > 0.0) {
                break;
            }
            iterations += 1;
            if iterations > 4 {
                break;
            }

            if (*a).tween_rate > 0.0 {
                (*a).tween_alpha = (*a).tween_rate.mul_add(seconds, (*a).tween_alpha);
                if (*a).tween_alpha >= 1.0 {
                    (*a).tween_alpha = 1.0;
                    (*a).tween_rate = 0.0;
                    if (*a).anim_rate == 0.0 {
                        (*a).set_b_anim_finished(true);
                        if !(*an).b_simulated_pawn() {
                            ops.actor_event(animator, EVENT_ANIM_END, 0.0)?;
                        }
                    }
                }
            }

            let old_anim_frame = (*a).anim_frame;
            if (*a).anim_rate != 0.0 {
                if (*a).anim_rate >= 0.0 {
                    (*a).anim_frame = (*a).anim_rate.mul_add(seconds, (*a).anim_frame);
                } else {
                    let speed = size((*an).velocity) * -(*a).anim_rate;
                    (*a).anim_frame =
                        ue_max((*a).anim_min_rate, speed).mul_add(seconds, (*a).anim_frame);
                }

                if (*a).b_anim_notify() && !(*a).mesh.is_null() {
                    let seq = ops.actor_get_anim(actor, (*a).anim_sequence.as_ptr())?;
                    if !seq.is_null() {
                        let mut best_elapsed = 100000.0_f32;
                        let mut best: Option<(f32, [u8; 4])> = None;
                        let count = ops.anim_notify_count(seq)?;
                        for i in 0..count {
                            let mut time = 0.0_f32;
                            let mut function = [0_u8; 4];
                            ops.anim_notify_get(seq, i, &mut time, &mut function)?;
                            if old_anim_frame < time && (*a).anim_frame >= time {
                                let elapsed = time - old_anim_frame;
                                if best.is_none() || elapsed < best_elapsed {
                                    best_elapsed = elapsed;
                                    best = Some((time, function));
                                }
                            }
                        }

                        if let Some((best_time, best_function)) = best {
                            seconds = seconds * ((*a).anim_frame - best_time)
                                / ((*a).anim_frame - old_anim_frame);
                            (*a).anim_frame = best_time;
                            let function =
                                ops.actor_find_function(animator, best_function.as_ptr())?;
                            if !function.is_null() {
                                ops.actor_process_event(animator, function)?;
                            }
                            continue;
                        }
                    }
                }

                if (*a).anim_frame < (*a).anim_last {
                    break;
                } else if (*a).b_anim_loop() {
                    if (*a).anim_frame < 1.0 {
                        seconds = 0.0;
                    } else {
                        seconds =
                            seconds * ((*a).anim_frame - 1.0) / ((*a).anim_frame - old_anim_frame);
                        (*a).anim_frame = 0.0;
                    }
                    if old_anim_frame < (*a).anim_last {
                        if ops.actor_latent_action(actor)? == EPOLL_FINISH_ANIM {
                            (*a).set_b_anim_finished(true);
                        }
                        if !simulated_pawn {
                            ops.actor_event(actor, EVENT_ANIM_END, 0.0)?;
                        }
                    }
                } else {
                    seconds = seconds * ((*a).anim_frame - (*a).anim_last)
                        / ((*a).anim_frame - old_anim_frame);
                    (*a).anim_frame = (*a).anim_last;
                    (*a).set_b_anim_finished(true);
                    (*a).anim_rate = 0.0;
                    if !(*an).b_simulated_pawn() {
                        ops.actor_event(animator, EVENT_ANIM_END, 0.0)?;
                    }
                    if (*a).remote_role < ROLE_SIMULATED_PROXY
                        && ops.actor_is_a(actor, KIND_AWEAPON)? == 0
                    {
                        (*a).sim_anim.x = 10000.0 * (*a).anim_frame;
                        (*a).sim_anim.y = 5000.0 * (*a).anim_rate;
                        if (*a).sim_anim.y > 32767.0 {
                            (*a).sim_anim.y = 32767.0;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// `bSimulatedPawn` movement (oracle 372-402): ground check, gravity, smooth move,
/// then the probing tick. Returns nothing; the caller always continues.
unsafe fn simulated_pawn_move<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    pawn: *mut c_void,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        let p = pawn.cast::<APawn>();
        if (*p).b_is_player()
            && !(*p).b_can_fly()
            && !(*(*a).region.zone.cast::<AZoneInfo>()).b_water_zone()
        {
            let loc = (*a).location;
            let start = &(*a).location as *const FVector as *const f32;
            let end = [loc.x, loc.y, loc.z - 6.0];
            let extent_v = ops.cylinder_extent(actor)?;
            let extent = [extent_v.x, extent_v.y, extent_v.z];
            let mut hit_time = 1.0_f32;
            let mut hit_normal = [0.0_f32; 3];
            ops.level_single_line_check(
                (*a).x_level,
                actor,
                end.as_ptr(),
                start,
                TRACE_VIS_BLOCKING,
                extent.as_ptr(),
                &mut hit_time,
                &mut hit_normal,
            )?;
            if hit_time == 1.0 || hit_normal[2] < 0.7 {
                let zone = (*a).region.zone.cast::<AZoneInfo>();
                if (*a).velocity.z == 0.0 {
                    (*a).velocity.z = -120.0;
                }
                let gravity = (*zone).zone_gravity;
                (*a).velocity.x += 0.5 * gravity.x * delta;
                (*a).velocity.y += 0.5 * gravity.y * delta;
                (*a).velocity.z += 0.5 * gravity.z * delta;
            }
        }

        let v = (*a).velocity;
        let smooth = [v.x * delta, v.y * delta, v.z * delta];
        ops.actor_move_smooth(actor, smooth.as_ptr())?;

        if ops.actor_is_probing(actor, PROBE_TICK)? != 0 {
            ops.actor_creature_trace_tick(actor, delta)?;
            ops.event_tick()?;
            ops.actor_event(actor, EVENT_TICK, delta)?;
        } else {
            ops.set_skip_reason(b"skip_not_probing\0");
        }
        Ok(())
    }
}

/// `Role >= ROLE_SimulatedProxy` player/non-player tick (oracle 448-544), followed
/// by process-state, timers, life span and physics. Returns `false` when the
/// C++ returned 1 early.
unsafe fn simulated_proxy_update<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    pawn: *mut c_void,
    tick_type: i32,
    delta: f32,
) -> Result<bool, O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        let player_pawn = player_pawn_of(ops, actor, pawn)?;
        if player_pawn.is_null() || (*player_pawn.cast::<APlayerPawn>()).player.is_null() {
            if tick_type == LEVELTICK_VIEWPORTS_ONLY {
                ops.set_skip_reason(b"skip_viewports_only\0");
                return Ok(false);
            }
            if ops.actor_is_probing(actor, PROBE_TICK)? != 0 {
                ops.actor_creature_trace_tick(actor, delta)?;
                ops.event_tick()?;
                ops.actor_event(actor, EVENT_TICK, delta)?;
            } else {
                ops.set_skip_reason(b"skip_not_probing\0");
            }
        } else {
            let pp = player_pawn.cast::<APlayerPawn>();
            if ops.actor_is_a(actor, KIND_ACAMERA)? != 0
                && ((*pp).show_flags & SHOW_PLAYER_CTRL) == 0
            {
                ops.set_skip_reason(b"skip_camera_input_disabled\0");
                return Ok(false);
            }
            ops.set_skip_reason(b"skip_player_tick\0");

            ops.player_read_input((*pp).player, delta)?;
            let mut raw_base_x = 0.0_f32;
            let mut raw_strafe = 0.0_f32;
            let mut raw_camera_yaw = 0.0_f32;
            let mut raw_camera_pitch = 0.0_f32;
            let modern = ops.pawn_begin_modern_tps(
                actor,
                &mut raw_base_x,
                &mut raw_strafe,
                &mut raw_camera_yaw,
                &mut raw_camera_pitch,
            )?;
            ops.actor_event(actor, EVENT_PLAYER_INPUT, delta)?;
            ops.pawn_finish_modern_tps(
                actor,
                modern,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
                delta,
            )?;
            ops.actor_event(actor, EVENT_PLAYER_TICK, delta)?;
            ops.pawn_apply_modern_tps(actor)?;
            ops.player_read_input((*pp).player, -1.0)?;

            let demo_driver = (*(*a).x_level.cast::<ULevel>()).base.demo_rec_driver;
            if !demo_driver.is_null()
                && ops.driver_server_connection(demo_driver)?.is_null()
            {
                (*pp).demo_view_pitch = (*pp).base.view_rotation.pitch;
                (*pp).demo_view_yaw = (*pp).base.view_rotation.yaw;
            }
        }

        ops.actor_process_state(actor, delta)?;
        tick_timer(ops, actor, delta)?;

        if (*a).life_span != 0.0 {
            (*a).life_span -= delta;
            if (*a).life_span <= 0.0001 {
                ops.actor_event(actor, EVENT_EXPIRED, 0.0)?;
                ops.level_destroy_actor((*a).x_level, actor)?;
                return Ok(false);
            }
        }

        if (*a).physics != PHYS_NONE && (*a).role != ROLE_AUTONOMOUS_PROXY {
            ops.actor_perform_physics(actor, delta)?;
        }
        Ok(true)
    }
}

/// Pawn block (oracle 580-637): eye height / view rotation sync, weapon placement,
/// sight, pain / speech timers and tactics.
unsafe fn pawn_update<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    pawn: *mut c_void,
    tick_type: i32,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        if pawn.is_null() {
            return Ok(());
        }
        let a = actor.cast::<AActor>();
        let p = pawn.cast::<APawn>();

        if (*p).b_is_player() && (*a).role >= ROLE_AUTONOMOUS_PROXY {
            if (*p).b_view_target() {
                ops.actor_event(actor, EVENT_UPDATE_EYE_HGT, delta)?;
            } else {
                (*p).view_rotation = (*a).rotation;
            }
        }

        if !(*p).weapon.is_null() {
            ops.level_far_move_actor(
                (*a).x_level,
                (*p).weapon,
                &(*a).location as *const FVector as *const f32,
            )?;
        }

        if (*a).role == ROLE_AUTHORITY && tick_type == LEVELTICK_ALL {
            if (*p).sight_counter < 0.0 {
                (*p).sight_counter += 0.2;
            }
            (*p).sight_counter -= delta;
            if (*p).b_is_player() && !(*p).base.b_hidden() {
                ops.actor_show_self(actor)?;
            }
            if (*p).sight_counter < 0.0
                && ops.actor_is_probing(actor, PROBE_ENEMY_NOT_VIS)? != 0
            {
                ops.actor_check_enemy_visible(actor)?;
                (*p).sight_counter = 0.1;
            }

            if (*p).pain_time > 0.0 {
                (*p).pain_time -= delta;
                if (*p).pain_time < 0.001 {
                    (*p).pain_time = 0.0;
                    ops.actor_event(actor, EVENT_PAIN_TIMER, 0.0)?;
                }
            }

            if (*p).speech_time > 0.0 {
                (*p).speech_time -= delta;
                if f64::from((*p).speech_time) < 0.001_f64 {
                    (*p).speech_time = 0.0;
                    ops.actor_event(actor, EVENT_SPEECH_TIMER, 0.0)?;
                }
            }

            if (*p).b_advanced_tactics() {
                ops.actor_event(actor, EVENT_UPDATE_TACTICS, delta)?;
            }
        }
        Ok(())
    }
}

/// Root motion (oracle 640-657): `Move` is applied with a slide retry on hit.
unsafe fn root_movement<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();
        if !(*a).b_anim_move() || (*a).role != ROLE_AUTHORITY || (*a).mesh.is_null() {
            return Ok(());
        }
        ops.root_movement_scoped(actor)
    }
}

/// Both collision calls share the original stack FCheckResult, including fields
/// not initialized by its constructor. Rust reads only fields the policy uses.
pub(crate) unsafe fn root_movement_body_with_ops<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    hit: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();

        let mut hit_time = 1.0_f32;
        let mut hit_normal = [0.0_f32; 3];
        let mut mv = [0.0_f32; 3];
        ops.mesh_get_root_movement((*a).mesh, actor, &mut mv)?;
        if mv[0] == 0.0 && mv[1] == 0.0 && mv[2] == 0.0 {
            return Ok(());
        }

        ops.level_move_actor(
            (*a).x_level,
            actor,
            mv.as_ptr(),
            ptr::addr_of!((*a).rotation).cast(),
            hit,
            &mut hit_time,
            &mut hit_normal,
        )?;

        let scale = 1.0 - hit_time;
        mv = [mv[0] * scale, mv[1] * scale, mv[2] * scale];

        if hit_time < 1.0 {
            let move_vec = FVector {
                x: mv[0],
                y: mv[1],
                z: mv[2],
            };
            let normal = FVector {
                x: hit_normal[0],
                y: hit_normal[1],
                z: hit_normal[2],
            };
            let along = dot(move_vec, normal);
            let move2 = [
                mv[0] - hit_normal[0] * along,
                mv[1] - hit_normal[1] * along,
                mv[2] - hit_normal[2] * along,
            ];

            ops.level_move_actor(
                (*a).x_level,
                actor,
                move2.as_ptr(),
                ptr::addr_of!((*a).rotation).cast(),
                hit,
                &mut hit_time,
                &mut hit_normal,
            )?;

            mv = [
                mv[0] - move2[0] * hit_time,
                mv[1] - move2[1] * hit_time,
                mv[2] - move2[2] * hit_time,
            ];
        }

        ops.mesh_adjust_root_movement((*a).mesh, actor, &mv)?;
        Ok(())
    }
}

/// Adapts the native stack-owned root-motion hit to the source-shared kernel.
///
/// # Safety
/// `actor`, `hit`, and every checked service operand remain live through reentry.
pub unsafe fn root_movement_body(
    svc: &Hp2LevTicServices, actor: *mut c_void, hit: *mut c_void,
) -> Result<(), i32> {
    // Root motion itself emits no Tick event; the enclosing native tick owns
    // its dispatch token. This adapter only supplies the movement/mesh leaves.
    let mut dispatch = Hp2TickDispatch {
        token: u64::MAX, skip_reason: ptr::null(), event_tick_dispatched: 0,
    };
    let mut ops = NativeActorTickOps { svc, dispatch: &mut dispatch };
    unsafe { root_movement_body_with_ops(&mut ops, actor, hit) }
}

/// `AActor::Tick` (oracle 137-661). Returns UBOOL as i32; 1 = ticked or skipped.
///
/// # Safety
/// `actor` is a live `AActor*`; every backend-reached actor remains live.
pub(crate) unsafe fn actor_tick_with_ops<O: ActorTickOps>(
    ops: &mut O,
    actor: *mut c_void,
    delta_seconds: f32,
    tick_type: i32,
) -> Result<i32, O::Error> {
    unsafe {
        let a = actor.cast::<AActor>();

        // Parent / owner ticks first, each as a recursive trace admission (oracle 143-163).
        let tick_parent = (*a).tick_parent;
        if !tick_parent.is_null() {
            ops.tick_actor_scoped(
                tick_parent,
                delta_seconds,
                tick_type,
                c"tick_parent".as_ptr(),
                c"recursive".as_ptr(),
            )?;
        }
        let tick_parent2 = (*a).tick_parent2;
        if !tick_parent2.is_null() {
            ops.tick_actor_scoped(
                tick_parent2,
                delta_seconds,
                tick_type,
                c"tick_parent2".as_ptr(),
                c"recursive".as_ptr(),
            )?;
        }
        let owner = (*a).owner;
        if !owner.is_null()
            && (*owner.cast::<AActor>()).tick_parent != actor
            && (*owner.cast::<AActor>()).tick_parent2 != actor
        {
            ops.tick_actor_scoped(
                owner,
                delta_seconds,
                tick_type,
                c"owner".as_ptr(),
                c"recursive".as_ptr(),
            )?;
        }

        let level = (*a).x_level;
        let lv = level.cast::<ULevel>();

        if (*a).b_ticked() as i32 == (*lv).ticked {
            ops.set_skip_reason(b"skip_already_ticked\0");
            return Ok(1);
        }

        if (*lv).b_in_special_pause_mode != 0 {
            let pp = if ops.actor_is_a(actor, KIND_APLAYERPAWN)? != 0 {
                actor
            } else {
                ptr::null_mut()
            };
            if !pp.is_null() && !(*pp.cast::<APlayerPawn>()).player.is_null() {
                ops.player_read_input((*pp.cast::<APlayerPawn>()).player, delta_seconds)?;
                ops.actor_event(actor, EVENT_PLAYER_INPUT, delta_seconds)?;
                ops.player_read_input((*pp.cast::<APlayerPawn>()).player, -1.0)?;
            }
            if !(*a).b_can_move_in_special_pause() {
                ops.set_skip_reason(b"skip_special_pause\0");
                return Ok(1);
            }
            (*a).set_b_in_special_pause(true);
        } else {
            (*a).set_b_in_special_pause(false);
        }

        if (*a).b_stasis()
            && ((*a).b_force_stasis() || (*a).physics == PHYS_NONE || (*a).physics == PHYS_ROTATING)
            && ops.level_zone_time_since_render(level, i32::from((*a).region.zone_number))? > 5.0
            && (*(*a).level.cast::<ALevelInfo>()).net_mode == NM_STANDALONE
        {
            ops.set_skip_reason(b"skip_stasis\0");
            return Ok(1);
        }

        // UBOOL bitfield: only the low bit of Ticked survives the assignment.
        (*a).set_b_ticked((*lv).ticked & 1 != 0);

        let pawn = if (*a).b_is_pawn() && ops.actor_is_a(actor, KIND_APAWN)? != 0 {
            actor
        } else {
            ptr::null_mut()
        };
        let simulated_pawn = !pawn.is_null() && (*a).role == ROLE_SIMULATED_PROXY;

        let animator = if (*a).anim_bone != 0 && (*a).b_anim_transient() {
            (*a).owner
        } else {
            actor
        };
        ops.check(!animator.is_null(), c"Animator", 219)?;

        if ops.level_is_editor()? != 0 {
            animate_editor_tween(ops, actor, animator, delta_seconds)?;
        } else {
            animate_loop(ops, actor, animator, simulated_pawn, delta_seconds)?;
        }

        if simulated_pawn {
            simulated_pawn_move(ops, actor, pawn, delta_seconds)?;
        } else if (*a).remote_role == ROLE_AUTONOMOUS_PROXY {
            ops.set_skip_reason(b"skip_autonomous_proxy\0");
            if (*a).role == ROLE_AUTHORITY {
                let pp = player_pawn_of(ops, actor, pawn)?;
                if !pp.is_null() && !(*pp.cast::<APlayerPawn>()).view_target.is_null() {
                    copy_view_target_state(ops, pp.cast::<APlayerPawn>())?;
                }
                tick_timer(ops, actor, delta_seconds)?;
            }
        } else if (*a).role >= ROLE_SIMULATED_PROXY {
            if !simulated_proxy_update(ops, actor, pawn, tick_type, delta_seconds)? {
                return Ok(1);
            }
        } else {
            ops.set_skip_reason(b"skip_role_below_simulated\0");
            if (*a).physics == PHYS_FALLING {
                ops.actor_perform_physics(actor, delta_seconds)?;
            }
        }

        let demo_driver = (*(*a).x_level.cast::<ULevel>()).base.demo_rec_driver;
        if !demo_driver.is_null()
            && !ops.driver_server_connection(demo_driver)?.is_null()
            && (*a).role == ROLE_AUTHORITY
        {
            let pp = player_pawn_of(ops, actor, pawn)?;
            if !pp.is_null()
                && !(*pp.cast::<APlayerPawn>()).view_target.is_null()
                && !(*pp.cast::<APlayerPawn>()).base.b_behind_view()
            {
                copy_view_target_state(ops, pp.cast::<APlayerPawn>())?;
            }
        }

        pawn_update(ops, actor, pawn, tick_type, delta_seconds)?;
        root_movement(ops, actor)?;

        Ok(1)
    }
}

/// Native `AActor::Tick` entry point. Returns UBOOL as i32; 1 = ticked or skipped.
///
/// # Safety
/// `actor` is a live `AActor*` of the level in `svc`; `svc` is a valid services table.
pub unsafe fn actor_tick(
    svc: &Hp2LevTicServices,
    actor: *mut c_void,
    delta_seconds: f32,
    tick_type: i32,
    dispatch: *mut Hp2TickDispatch,
) -> Result<i32, i32> {
    let mut ops = NativeActorTickOps { svc, dispatch };
    unsafe { actor_tick_with_ops(&mut ops, actor, delta_seconds, tick_type) }
}
