//! Canonical Core host for the shared `AActor::Tick` policy.
//!
//! The complete source tick (parent/owner recursion, eligibility, pause,
//! stasis, animation, role dispatch, timers, lifespan, physics, pawn and root
//! movement) lives in [`crate::lev_tic_actor`]. This module implements its typed
//! [`crate::lev_tic_actor::ActorTickOps`] leaves against canonical Core records
//! and performs the virtual subclass dispatch that the C++ vtable supplied.

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, AParticleFX, AWind, FFrame, FMeshAnimSeq, FStateFrame, TArrayRaw,
    UAnimation, UBoolProperty, UClass, UField, UFunction, ULevel, UMesh, UNetDriver, UObject,
    UProperty, USkeletalMesh,
};
use hp2_abi::render_scene::FZoneProperties;
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;

use crate::lev_tic_actor::{
    ActorTickOps, EVENT_ANIM_END, EVENT_EXPIRED, EVENT_PAIN_TIMER, EVENT_PLAYER_INPUT,
    EVENT_PLAYER_TICK, EVENT_SPEECH_TIMER, EVENT_TICK, EVENT_TIMER, EVENT_UPDATE_EYE_HGT,
    EVENT_UPDATE_TACTICS, KIND_APAWN, KIND_APLAYERPAWN, KIND_ACAMERA, KIND_AWEAPON,
    PROBE_ENEMY_NOT_VIS, PROBE_TICK,
};

/// `STATE_Simulated` (`UnStack.h:45`).
const STATE_SIMULATED: u32 = 0x0000_0004;

/// Runs the actual virtual `AActor::Tick` for a canonical Actor.
///
/// Subclass overrides mirror the source vtable: ParticleFX updates before
/// `AActor::Tick`, while Wind ticks after its superclass.
///
/// # Safety
/// `actor` is a live canonical Actor and every object reached by the tick remains
/// live and owned by `access` for the synchronous call.
pub(crate) unsafe fn tick_core(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: f32,
    tick_type: i32,
    relation: *const c_char,
    root_admission: *const c_char,
) -> RuntimeResult<i32> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in Tick"));
    }
    unsafe { dispatch_virtual(access, actor, delta, tick_type, relation, root_admission) }
}


/// Dispatches one virtual `Tick` call, including overrides and their super call.
unsafe fn dispatch_virtual(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: f32,
    tick_type: i32,
    relation: *const c_char,
    root_admission: *const c_char,
) -> RuntimeResult<i32> {
    if unsafe { crate::actor_physics_owner::is_a_named(access, actor.cast(), "ParticleFX")? } {
        unsafe {
            crate::particle_fx_owner::tick(
                access,
                actor.cast::<AParticleFX>(),
                delta,
                tick_type,
            )?;
        }
        return unsafe { actor_policy(access, actor, delta, tick_type, relation, root_admission) };
    }
    if unsafe { crate::actor_physics_owner::is_a_named(access, actor.cast(), "Wind")? } {
        let super_tick = |wind_actor: *mut AActor, super_delta: f32, super_tick_type: i32| {
            unsafe {
                actor_policy(
                    access, wind_actor, super_delta, super_tick_type, relation, root_admission,
                )
            }
            .map(|value| value != 0)
        };
        unsafe {
            crate::wind_owner::tick(
                access,
                actor.cast::<AWind>(),
                delta,
                tick_type,
                super_tick,
            )?;
        }
        return Ok(1);
    }
    unsafe { actor_policy(access, actor, delta, tick_type, relation, root_admission) }
}


/// Runs the shared non-virtual `AActor::Tick` body and its source trace scope.
unsafe fn actor_policy(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: f32,
    tick_type: i32,
    relation: *const c_char,
    root_admission: *const c_char,
) -> RuntimeResult<i32> {
    let token = crate::live_trace_owner::global_tick_begin_dispatch(
        access, actor, relation, root_admission,
    );
    let mut ops = CoreActorTickOps {
        access,
        dispatch_token: token,
        event_tick_dispatched: false,
        skip_reason: b"skip_no_event\0",
    };
    let result = unsafe {
        crate::lev_tic_actor::actor_tick_with_ops(&mut ops, actor.cast(), delta, tick_type)
    };
    let outcome = if ops.event_tick_dispatched {
        c"dispatched".as_ptr()
    } else {
        ops.skip_reason.as_ptr().cast()
    };
    crate::live_trace_owner::global_tick_end_dispatch(access, token, outcome);
    result
}

struct CoreActorTickOps<'a> {
    access: CoreAccess<'a>,
    dispatch_token: u64,
    event_tick_dispatched: bool,
    skip_reason: &'static [u8],
}

impl ActorTickOps for CoreActorTickOps<'_> {
    type Error = RuntimeError;

    unsafe fn event_tick(&mut self) -> Result<(), RuntimeError> {
        self.event_tick_dispatched = true;
        crate::live_trace_owner::global_tick_event_tick(self.access, self.dispatch_token);
        Ok(())
    }

    fn set_skip_reason(&mut self, reason: &'static [u8]) {
        self.skip_reason = reason;
    }


    unsafe fn check(
        &mut self,
        ok: bool,
        expr: &std::ffi::CStr,
        _line: i32,
    ) -> Result<(), RuntimeError> {
        if ok {
            Ok(())
        } else {
            Err(RuntimeError::new(&format!("Canonical Actor check failed: {}", expr.to_string_lossy())))
        }
    }

    unsafe fn actor_is_a(&mut self, actor: *mut c_void, kind: i32) -> Result<i32, RuntimeError> {
        let name = match kind {
            KIND_APAWN => "Pawn",
            KIND_APLAYERPAWN => "PlayerPawn",
            KIND_ACAMERA => "Camera",
            KIND_AWEAPON => "Weapon",
            2 => "ZoneInfo",
            _ => return Err(RuntimeError::new("Unknown canonical Actor kind")),
        };
        Ok(i32::from(unsafe {
            crate::actor_physics_owner::is_a_named(self.access, actor, name)?
        }))
    }

    unsafe fn cylinder_extent(&mut self, actor: *mut c_void) -> Result<FVector, RuntimeError> {
        unsafe { crate::collision_owner::actor_cylinder_extent(self.access, actor.cast()) }
    }

    unsafe fn actor_event(
        &mut self,
        actor: *mut c_void,
        event: i32,
        delta: f32,
    ) -> Result<(), RuntimeError> {
        let (name, has_delta) = match event {
            EVENT_TICK => ("Tick", true),
            EVENT_TIMER => ("Timer", false),
            EVENT_EXPIRED => ("Expired", false),
            EVENT_ANIM_END => ("AnimEnd", false),
            EVENT_PAIN_TIMER => ("PainTimer", false),
            EVENT_SPEECH_TIMER => ("SpeechTimer", false),
            EVENT_UPDATE_TACTICS => ("UpdateTactics", true),
            EVENT_UPDATE_EYE_HGT => ("UpdateEyeHeight", true),
            EVENT_PLAYER_INPUT => ("PlayerInput", true),
            EVENT_PLAYER_TICK => ("PlayerTick", true),
            _ => return Err(RuntimeError::new("Unknown canonical Actor event")),
        };
        unsafe { execute_event(self.access, actor, name, has_delta.then_some(delta)) }
    }

    unsafe fn actor_process_state(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), RuntimeError> {
        unsafe { process_state(self.access, actor.cast(), delta)?; }
        if let Some(observer) = self.access.owner_context::<crate::core_owner::EngineState>()
            .and_then(|state| state.actor_lifecycle_observer.get())
        {
            let actor = std::ptr::NonNull::new(actor.cast::<AActor>())
                .ok_or_else(|| RuntimeError::new("ProcessState observer received a null Actor"))?;
            observer(self.access, actor, crate::core_owner::ActorLifecycleEvent::ProcessState)?;
        }
        Ok(())
    }
    unsafe fn actor_perform_physics(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), RuntimeError> {
        unsafe { crate::actor_physics_owner::perform_physics(self.access, actor.cast(), delta) }
    }

    unsafe fn actor_move_smooth(
        &mut self,
        actor: *mut c_void,
        delta: *const f32,
    ) -> Result<(), RuntimeError> {
        if delta.is_null() {
            return Err(RuntimeError::new("Null MoveSmooth delta"));
        }
        let delta = FVector {
            x: unsafe { *delta },
            y: unsafe { *delta.add(1) },
            z: unsafe { *delta.add(2) },
        };
        unsafe { crate::actor_physics_owner::move_smooth(self.access, actor.cast(), delta).map(|_| ()) }
    }


    unsafe fn actor_is_probing(
        &mut self,
        actor: *mut c_void,
        probe: i32,
    ) -> Result<i32, RuntimeError> {
        let name = if probe == PROBE_ENEMY_NOT_VIS {
            "EnemyNotVisible"
        } else {
            "Tick"
        };
        let name = self.access.intern_str(name, FindName::Find)?;
        Ok(i32::from(unsafe { self.access.is_probing(actor.cast(), name)? }))
    }

    unsafe fn actor_creature_trace_tick(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> Result<(), RuntimeError> {
        crate::live_trace_owner::creature_tick_dispatch(self.access, actor.cast(), delta);
        Ok(())
    }

    unsafe fn actor_latent_action(&mut self, actor: *mut c_void) -> Result<i32, RuntimeError> {
        let frame = state_frame(actor)?;
        if frame.is_null() {
            return Err(RuntimeError::new("Canonical Actor has no state frame for LatentAction"));
        }
        Ok(unsafe { ptr::addr_of!((*frame).latent_action).read_unaligned() })
    }

    unsafe fn actor_show_self(&mut self, actor: *mut c_void) -> Result<(), RuntimeError> {
        unsafe { show_self(self.access, actor.cast()) }
    }

    unsafe fn actor_check_enemy_visible(
        &mut self,
        actor: *mut c_void,
    ) -> Result<(), RuntimeError> {
        with_tick_context(self.access, |_| unsafe {
            crate::pawn_sense::check_enemy_visible(&CANONICAL_SENSE_SERVICES, actor.cast())
                .map_err(|_| RuntimeError::new("Canonical CheckEnemyVisible failed"))
        })
    }

    unsafe fn player_read_input(
        &mut self,
        player: *mut c_void,
        delta: f32,
    ) -> Result<(), RuntimeError> {
        unsafe { crate::input_owner::player_read_input(self.access, player, delta) }
    }

    unsafe fn pawn_begin_modern_tps(
        &mut self,
        pawn: *mut c_void,
        raw_base_x: &mut f32,
        raw_strafe: &mut f32,
        raw_camera_yaw: &mut f32,
        raw_camera_pitch: &mut f32,
    ) -> Result<i32, RuntimeError> {
        with_tick_context(self.access, |_| unsafe {
            crate::pawn_native_input::begin_modern_third_person_input(
                &CANONICAL_INPUT_SERVICES,
                pawn.cast(),
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
            )
            .map(i32::from)
            .map_err(|_| RuntimeError::new("Canonical BeginModernThirdPersonInput failed"))
        })
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
    ) -> Result<(), RuntimeError> {
        with_tick_context(self.access, |_| unsafe {
            crate::pawn_native_input::finish_modern_third_person_input(
                &CANONICAL_INPUT_SERVICES,
                pawn.cast(),
                active != 0,
                raw_base_x,
                raw_strafe,
                raw_camera_yaw,
                raw_camera_pitch,
                delta,
            )
            .map_err(|_| RuntimeError::new("Canonical FinishModernThirdPersonInput failed"))
        })
    }

    unsafe fn pawn_apply_modern_tps(&mut self, pawn: *mut c_void) -> Result<(), RuntimeError> {
        with_tick_context(self.access, |_| unsafe {
            crate::pawn_native_input::apply_modern_third_person_movement(
                &CANONICAL_INPUT_SERVICES,
                pawn.cast(),
            )
            .map_err(|_| RuntimeError::new("Canonical ApplyModernThirdPersonMovement failed"))
        })
    }

    unsafe fn actor_get_anim(
        &mut self,
        actor: *mut c_void,
        sequence_name: *const u8,
    ) -> Result<*mut c_void, RuntimeError> {
        if sequence_name.is_null() {
            return Err(RuntimeError::new("Null animation sequence name"));
        }
        let sequence = unsafe { *sequence_name.cast::<[u8; 4]>() };
        unsafe { get_anim(self.access, actor.cast(), sequence) }
    }

    unsafe fn anim_notify_count(&mut self, seq: *mut c_void) -> Result<i32, RuntimeError> {
        let seq = seq.cast::<FMeshAnimSeq>();
        if seq.is_null() {
            return Ok(0);
        }
        let notifies = unsafe { ptr::addr_of!((*seq).notifys).read_unaligned() };
        if notifies.num < 0 || (notifies.num > 0 && notifies.data.is_null()) {
            return Err(RuntimeError::new("Invalid animation notify array"));
        }
        Ok(notifies.num)
    }

    unsafe fn anim_notify_get(
        &mut self,
        seq: *mut c_void,
        index: i32,
        time: &mut f32,
        function: &mut [u8; 4],
    ) -> Result<(), RuntimeError> {
        let seq = seq.cast::<FMeshAnimSeq>();
        if seq.is_null() || index < 0 {
            return Err(RuntimeError::new("Invalid animation notify index"));
        }
        let notifies = unsafe { ptr::addr_of!((*seq).notifys).read_unaligned() };
        if index >= notifies.num || notifies.data.is_null() {
            return Err(RuntimeError::new("Animation notify index is out of range"));
        }
        let notify = unsafe {
            notifies
                .data
                .cast::<hp2_abi::engine::generated::FMeshAnimNotify>()
                .add(index as usize)
                .read_unaligned()
        };
        *time = notify.time;
        *function = notify.function;
        Ok(())
    }

    unsafe fn actor_find_function(
        &mut self,
        actor: *mut c_void,
        name: *const u8,
    ) -> Result<*mut c_void, RuntimeError> {
        if name.is_null() {
            return Ok(ptr::null_mut());
        }
        let name = i32::from_ne_bytes(unsafe { *name.cast::<[u8; 4]>() });
        Ok(unsafe { self.access.find_object_function_by_name(actor.cast(), name, false)? }.cast())
    }

    unsafe fn actor_process_event(
        &mut self,
        actor: *mut c_void,
        function: *mut c_void,
    ) -> Result<(), RuntimeError> {
        unsafe {
            crate::actor_owner::process_event(
                self.access,
                actor.cast(),
                function.cast::<UFunction>(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
    }

    unsafe fn mesh_get_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        out: &mut [f32; 3],
    ) -> Result<(), RuntimeError> {
        if mesh.is_null() {
            *out = [0.0; 3];
            return Ok(());
        }
        if unsafe { crate::actor_physics_owner::is_a_named(self.access, mesh, "SkeletalMesh")? } {
            let movement = unsafe {
                crate::render_owner::skeletal_root_movement(
                    self.access,
                    mesh.cast::<USkeletalMesh>(),
                    actor.cast(),
                )?
            };
            *out = [movement.x, movement.y, movement.z];
        } else {
            // `UMesh::GetRootMovement` returns zero.
            *out = [0.0; 3];
        }
        Ok(())
    }

    unsafe fn mesh_adjust_root_movement(
        &mut self,
        mesh: *mut c_void,
        actor: *mut c_void,
        adjust: &[f32; 3],
    ) -> Result<(), RuntimeError> {
        if mesh.is_null() || adjust == &[0.0, 0.0, 0.0] {
            return Ok(());
        }
        if unsafe { crate::actor_physics_owner::is_a_named(self.access, mesh, "SkeletalMesh")? } {
            unsafe {
                crate::render_owner::skeletal_adjust_root_movement(
                    self.access,
                    mesh.cast::<USkeletalMesh>(),
                    actor.cast(),
                    FVector {
                        x: adjust[0],
                        y: adjust[1],
                        z: adjust[2],
                    },
                )?;
            }
        }
        Ok(())
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
    ) -> Result<(), RuntimeError> {
        if level.is_null() || end.is_null() || start.is_null() || extent.is_null() {
            return Err(RuntimeError::new("Invalid canonical SingleLineCheck arguments"));
        }
        let mut hit = crate::levact::check_result(1.0);
        unsafe {
            crate::level_trace_owner::single_line_check(
                self.access,
                level.cast::<ULevel>(),
                &mut hit,
                actor.cast(),
                read_vector(end),
                read_vector(start),
                trace,
                read_vector(extent),
                0,
            )?;
        }
        *hit_time = hit.time;
        if hit.time != 1.0 {
            hit_normal[0] = hit.normal.x;
            hit_normal[1] = hit.normal.y;
            hit_normal[2] = hit.normal.z;
        }
        Ok(())
    }

    unsafe fn level_far_move_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
        location: *const f32,
    ) -> Result<(), RuntimeError> {
        if level.is_null() || location.is_null() {
            return Err(RuntimeError::new("Invalid canonical FarMoveActor arguments"));
        }
        unsafe {
            crate::level_move_owner::far_move_actor(
                self.access,
                level.cast::<ULevel>(),
                actor.cast(),
                read_vector(location),
                false,
                false,
            )?;
        }
        Ok(())
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
    ) -> Result<(), RuntimeError> {
        if level.is_null() || movement.is_null() || rotation.is_null() || hit.is_null() {
            return Err(RuntimeError::new("Invalid canonical MoveActor arguments"));
        }
        let rotation = hp2_abi::engine::generated::FRotator {
            pitch: unsafe { *rotation },
            yaw: unsafe { *rotation.add(1) },
            roll: unsafe { *rotation.add(2) },
        };
        let hit = hit.cast::<hp2_abi::engine::generated::FCheckResult>();
        unsafe {
            crate::level_move_owner::move_actor(
                self.access,
                level.cast::<ULevel>(),
                actor.cast(),
                read_vector(movement),
                rotation,
                hit,
                false,
                false,
                false,
                false,
            )?;
            *hit_time = (*hit).time;
            if (*hit).time < 1.0 {
                hit_normal[0] = (*hit).normal.x;
                hit_normal[1] = (*hit).normal.y;
                hit_normal[2] = (*hit).normal.z;
            }
        }
        Ok(())
    }

    unsafe fn level_destroy_actor(
        &mut self,
        level: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), RuntimeError> {
        unsafe {
            crate::level_lifecycle_owner::destroy_actor(
                self.access,
                level.cast::<ULevel>(),
                actor.cast(),
                false,
            )?;
        }
        Ok(())
    }

    unsafe fn level_zone_time_since_render(
        &mut self,
        level: *mut c_void,
        zone_number: i32,
    ) -> Result<f32, RuntimeError> {
        let level = level.cast::<ULevel>();
        if level.is_null() {
            return Err(RuntimeError::new("Canonical zone query has no Level"));
        }
        let model = unsafe { ptr::addr_of!((*level).model).read_unaligned() }
            .cast::<hp2_abi::engine::generated::UModel>();
        if model.is_null() || zone_number < 0 || zone_number >= 64 {
            return Err(RuntimeError::new("Actor stasis has invalid zone geometry"));
        }
        let zones = unsafe { ptr::addr_of!((*model).zones).cast::<FZoneProperties>() };
        let last_render = unsafe {
            zones
                .cast::<u8>()
                .add(
                    zone_number as usize * std::mem::size_of::<FZoneProperties>()
                        + std::mem::offset_of!(FZoneProperties, last_render_time),
                )
                .cast::<f32>()
                .read_unaligned()
        };
        let time = i64::from_ne_bytes(unsafe {
            ptr::addr_of!((*level).time_seconds).read_unaligned()
        });
        Ok(hp2_core::time::subtract(
            time,
            hp2_core::time::from_float(last_render),
        ))
    }

    unsafe fn level_is_editor(&mut self) -> Result<i32, RuntimeError> {
        Ok(i32::from(self.access.process_mode().is_editor))
    }

    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, RuntimeError> {
        if driver.is_null() {
            return Ok(ptr::null_mut());
        }
        Ok(unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).server_connection).read_unaligned()
        })
    }

    unsafe fn root_movement_scoped(
        &mut self,
        actor: *mut c_void,
    ) -> Result<(), RuntimeError> {
        let mut hit = crate::levact::check_result(1.0);
        unsafe {
            crate::lev_tic_actor::root_movement_body_with_ops(
                self,
                actor,
                (&mut hit as *mut hp2_abi::engine::generated::FCheckResult).cast(),
            )
        }
    }

    unsafe fn tick_actor_scoped(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
        relation: *const c_char,
        root_admission: *const c_char,
    ) -> Result<i32, RuntimeError> {
        unsafe {
            dispatch_virtual(
                self.access,
                actor.cast(),
                delta,
                tick_type,
                relation,
                root_admission,
            )
        }
    }
}

fn read_vector(pointer: *const f32) -> FVector {
    unsafe {
        FVector {
            x: *pointer,
            y: *pointer.add(1),
            z: *pointer.add(2),
        }
    }
}

fn core_rotator(rotation: hp2_abi::engine::generated::FRotator) -> hp2_core::math::FRotator {
    hp2_core::math::FRotator {
        pitch: rotation.pitch,
        yaw: rotation.yaw,
        roll: rotation.roll,
    }
}

fn abi_rotator(rotation: hp2_core::math::FRotator) -> hp2_abi::engine::generated::FRotator {
    hp2_abi::engine::generated::FRotator {
        pitch: rotation.pitch,
        yaw: rotation.yaw,
        roll: rotation.roll,
    }
}

/// Returns the live state frame of an object (Actor's frame is on UObject).
unsafe fn state_frame(object: *mut c_void) -> RuntimeResult<*mut FStateFrame> {
    if object.is_null() {
        return Err(RuntimeError::new("Null object in state-frame lookup"));
    }
    Ok(unsafe { ptr::addr_of!((*object.cast::<UObject>()).state_frame).read_unaligned() }
        .cast::<FStateFrame>())
}

/// Executes one named event on a canonical Actor with an optional float parameter.
unsafe fn execute_event(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    event: &str,
    delta: Option<f32>,
) -> RuntimeResult<()> {
    let mut parameter = delta.unwrap_or(0.0);
    if delta.is_some() {
        unsafe {
            execute_event_params(
                access,
                actor,
                event,
                (&mut parameter as *mut f32).cast(),
                std::mem::size_of::<f32>(),
            )
        }
    } else {
        unsafe { execute_event_params(access, actor, event, ptr::null_mut(), 0) }
    }
}

/// Executes one named event with caller-owned reflected parameter storage.
pub(crate) unsafe fn execute_event_params(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    event: &str,
    params: *mut c_void,
    parameter_size: usize,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Ok(());
    }
    let name = access.intern_str(event, FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(actor.cast(), name, false)? };
    if function.is_null() {
        return Ok(());
    }
    if usize::from(unsafe { (*function).parms_size }) > parameter_size
        || (unsafe { (*function).parms_size } != 0 && params.is_null())
    {
        return Err(RuntimeError::new("Canonical Actor event parameter layout is too small"));
    }
    unsafe {
        crate::actor_owner::process_event(
            access,
            actor.cast(),
            function,
            params,
            ptr::null_mut(),
        )
    }
}

/// Runs `AActor::ProcessState` with canonical VM state stepping and emits the
/// source's pre-dispatch state snapshot before VM reentry.
unsafe fn process_state(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    delta: f32,
) -> RuntimeResult<()> {
    let frame = unsafe { state_frame(actor.cast())? };
    if frame.is_null() {
        crate::live_trace_owner::global_tick_process_state(
            access, actor, ptr::null_mut(), c"skip_no_frame".as_ptr(),
        );
        return Ok(());
    }
    let state_node = unsafe { ptr::addr_of!((*frame).state_node).read_unaligned() };
    let has_code = !unsafe { ptr::addr_of!((*frame).code).read_unaligned() }.is_null();
    if !has_code {
        crate::live_trace_owner::global_tick_process_state(
            access, actor, state_node.cast(), c"skip_no_code".as_ptr(),
        );
        return Ok(());
    }
    let role = i32::from(unsafe { ptr::addr_of!((*actor).role).read_unaligned() });
    let simulated = !state_node.is_null()
        && role < 4
        && unsafe {
            ptr::addr_of!((*state_node.cast::<hp2_abi::engine::generated::UState>()).state_flags)
                .read_unaligned()
        } & STATE_SIMULATED
            != 0;
    let pending_kill = unsafe { crate::actor::is_pending_kill(actor) };
    let dispatch = !(role < 4 && !simulated) && !pending_kill;
    let outcome = if role < 4 && !simulated {
        c"skip_role".as_ptr()
    } else if pending_kill {
        c"skip_pending_kill".as_ptr()
    } else {
        c"dispatched".as_ptr()
    };
    crate::live_trace_owner::global_tick_process_state(access, actor, state_node.cast(), outcome);
    if !dispatch {
        return Ok(());
    }

    let _script_scope = hp2_core::timing_globals::script_scope();
    let mut state = snapshot(frame, actor);
    if state.latent_action != 0 {
        poll_latent(access, actor, frame, delta)?;
        state = snapshot(frame, actor);
    }
    let mut transitions = 0;
    let mut old_state = state.state_node;
    while !state.delete_me && state.has_code && state.latent_action == 0 {
        // ProcessStateStep supplies MAX_CONST_SIZE bytes even for discarded
        // statement results; assignment natives write their expression result.
        let mut scratch = std::mem::MaybeUninit::<[u128; std::mem::size_of::<hp2_core::ansi::Tchar>()]>::uninit();
        unsafe { access.execute_state_frame_step(frame.cast::<FFrame>(), scratch.as_mut_ptr().cast())?; }
        state = snapshot(frame, actor);
        if state.state_node != old_state {
            old_state = state.state_node;
            transitions += 1;
            if transitions > 4 {
                break;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ProcessSnapshot {
    state_node: *mut c_void,
    has_code: bool,
    latent_action: i32,
    delete_me: bool,
}

unsafe fn snapshot(frame: *mut FStateFrame, actor: *mut AActor) -> ProcessSnapshot {
    ProcessSnapshot {
        state_node: unsafe { ptr::addr_of!((*frame).state_node).read_unaligned() },
        has_code: !unsafe { ptr::addr_of!((*frame).code).read_unaligned() }.is_null(),
        latent_action: unsafe { ptr::addr_of!((*frame).latent_action).read_unaligned() },
        delete_me: unsafe { (*actor).b_delete_me() },
    }
}

unsafe fn poll_latent(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    frame: *mut FStateFrame,
    delta: f32,
) -> RuntimeResult<()> {
    let latent = unsafe { ptr::addr_of!((*frame).latent_action).read_unaligned() };
    let mut step = delta;
    unsafe {
        access.execute_vm_native_index(
            actor.cast(),
            latent as u32,
            frame.cast::<FFrame>(),
            (&mut step as *mut f32).cast(),
        )
    }
}

/// Canonical `APawn::ShowSelf` policy.
unsafe fn show_self(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    if pawn.is_null() {
        return Ok(());
    }
    with_tick_context(access, |_| unsafe {
        let level = (*pawn.cast::<AActor>()).level.cast::<ALevelInfo>();
        if level.is_null() {
            return Err(RuntimeError::new("Canonical ShowSelf has no LevelInfo"));
        }
        let mut other = (*level).pawn_list.cast::<APawn>();
        while !other.is_null() {
            let raw_other = other.cast::<c_void>();
            if other != pawn
                && (*other).sight_counter < 0.0
                && probing(access, raw_other, "SeePlayer")?
                && crate::pawn_sense::line_of_sight_to(
                    &CANONICAL_SENSE_SERVICES,
                    other,
                    pawn.cast(),
                    1,
                )
                .map_err(|_| RuntimeError::new("Canonical ShowSelf line-of-sight failed"))?
                    != 0
            {
                let mut seer = pawn.cast::<c_void>();
                execute_event_params(
                    access,
                    raw_other,
                    "SeePlayer",
                    (&mut seer as *mut *mut c_void).cast(),
                    std::mem::size_of::<*mut c_void>(),
                )?;
            }
            other = (*other).next_pawn.cast();
        }
        Ok(())
    })
}

unsafe fn probing(access: CoreAccess<'_>, object: *mut c_void, name: &str) -> RuntimeResult<bool> {
    let name = access.intern_str(name, FindName::Find)?;
    unsafe { access.is_probing(object.cast::<UObject>(), name) }
}

unsafe fn get_anim(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    sequence: [u8; 4],
) -> RuntimeResult<*mut c_void> {
    if actor.is_null() {
        return Ok(ptr::null_mut());
    }
    let mesh = unsafe { ptr::addr_of!((*actor).mesh).read_unaligned() };
    if mesh.is_null() {
        return Ok(ptr::null_mut());
    }
    let owner = if unsafe {
        crate::actor_physics_owner::is_a_named(access, mesh, "SkeletalMesh")?
    } {
        let animation = unsafe { ptr::addr_of!((*actor).skel_anim).read_unaligned() };
        if !animation.is_null() {
            animation
        } else {
            unsafe {
                ptr::addr_of!((*mesh.cast::<USkeletalMesh>()).default_animation).read_unaligned()
            }
        }
    } else {
        mesh
    };
    if owner.is_null() {
        return Ok(ptr::null_mut());
    }
    let sequences: TArrayRaw = if unsafe {
        crate::actor_physics_owner::is_a_named(access, owner, "Animation")?
    } {
        unsafe { ptr::addr_of!((*owner.cast::<UAnimation>()).anim_seqs).read_unaligned() }
    } else {
        unsafe { ptr::addr_of!((*owner.cast::<UMesh>()).anim_seqs).read_unaligned() }
    };
    if sequences.num <= 0 || sequences.data.is_null() {
        return Ok(ptr::null_mut());
    }
    let count = sequences.num as usize;
    for index in 0..count {
        let seq = unsafe {
            sequences
                .data
                .cast::<FMeshAnimSeq>()
                .add(index)
                .read_unaligned()
        };
        if seq.name == sequence {
            return Ok(unsafe { sequences.data.cast::<FMeshAnimSeq>().add(index).cast() });
        }
    }
    Ok(ptr::null_mut())
}

// ---------------------------------------------------------------------------
// Canonical call tables for pawn sense and modern third-person input.
// ---------------------------------------------------------------------------

struct TickCallContext {
    access: CoreAccess<'static>,
    error: RefCell<Option<RuntimeError>>,
}

thread_local! {
    static TICK_CALL_CONTEXT: Cell<*mut TickCallContext> = const { Cell::new(ptr::null_mut()) };
}

struct TickCallScope {
    previous: *mut TickCallContext,
}

impl TickCallScope {
    fn enter(context: &mut TickCallContext) -> Self {
        let previous = TICK_CALL_CONTEXT.with(|slot| {
            let previous = slot.get();
            slot.set(context);
            previous
        });
        Self { previous }
    }
}

impl Drop for TickCallScope {
    fn drop(&mut self) {
        TICK_CALL_CONTEXT.with(|slot| slot.set(self.previous));
    }
}

fn tick_context() -> Option<*mut TickCallContext> {
    TICK_CALL_CONTEXT.with(|slot| {
        let context = slot.get();
        (!context.is_null()).then_some(context)
    })
}

fn with_tick_context<T>(
    access: CoreAccess<'_>,
    body: impl FnOnce(*mut TickCallContext) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    let mut context = TickCallContext {
        // SAFETY: the context lives on this stack frame for the whole call and
        // is popped before `access` returns; wrappers never retain it.
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
        error: RefCell::new(None),
    };
    let _scope = TickCallScope::enter(&mut context);
    match body(&mut context) {
        Ok(value) => Ok(value),
        Err(error) => {
            if context.error.borrow().is_none() {
                *context.error.borrow_mut() = Some(error);
            }
            Err(context.error.into_inner().unwrap())
        }
    }
}

pub(crate) fn with_sense_services<T>(
    access: CoreAccess<'_>,
    body: impl FnOnce(&'static crate::pawn_sense::Hp2PawnSenseServices) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    with_tick_context(access, |_| body(&CANONICAL_SENSE_SERVICES))
}

fn tick_error(message: &'static str) -> i32 {
    match tick_context() {
        Some(context) => {
            let context = unsafe { &*context };
            if context.error.borrow().is_none() {
                *context.error.borrow_mut() = Some(RuntimeError::new(message));
            }
            1
        }
        None => 1,
    }
}

fn tick_access() -> Option<CoreAccess<'static>> {
    tick_context().map(|context| unsafe { (*context).access })
}

macro_rules! tick_status {
    ($body:expr) => {
        match $body {
            Ok(()) => 0,
            Err(_) => tick_error("Canonical pawn/input route failed"),
        }
    };
}

unsafe extern "C" fn sense_fast_line_check(
    level: *mut c_void,
    a: *const c_void,
    b: *const c_void,
    out: *mut i32,
) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if level.is_null() || a.is_null() || b.is_null() {
        return tick_error("Canonical FastLineCheck has null arguments");
    }
    let result = (|| -> RuntimeResult<i32> {
        let level = level.cast::<ULevel>();
        let model = unsafe { ptr::addr_of!((*level).model).read_unaligned() }
            .cast::<hp2_abi::engine::generated::UModel>();
        if model.is_null() {
            return Err(RuntimeError::new("Canonical FastLineCheck has no Model"));
        }
        let end = unsafe { *a.cast::<FVector>() };
        let start = unsafe { *b.cast::<FVector>() };
        Ok(i32::from(unsafe {
            crate::collision_owner::model_fast_line_check(model, end, start)?
        }))
    })();
    match result {
        Ok(value) => {
            if !out.is_null() { unsafe { *out = value; } }
            0
        }
        Err(_) => tick_error("Canonical FastLineCheck failed"),
    }
}

#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn sense_single_line_check(
    level: *mut c_void,
    source: *mut c_void,
    hit: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    trace: u32,
    extent: *const c_void,
) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if level.is_null() || hit.is_null() || end.is_null() || start.is_null() || extent.is_null() {
        return tick_error("Canonical sense SingleLineCheck has null arguments");
    }
    let result = unsafe {
        crate::level_trace_owner::single_line_check(
            access,
            level.cast::<ULevel>(),
            hit.cast(),
            source.cast(),
            *end.cast::<FVector>(),
            *start.cast::<FVector>(),
            trace,
            *extent.cast::<FVector>(),
            0,
        )
    };
    tick_status!(result.map(|_| ()))
}

unsafe extern "C" fn sense_app_frand(out: *mut f32) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if out.is_null() { return 1; }
    unsafe { *out = access.rand_range_float(0.0, 1.0); }
    0
}

unsafe extern "C" fn sense_set_physics(actor: *mut c_void, physics: u8) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    tick_status!(unsafe {
        crate::actor_physics_owner::set_physics(access, actor.cast(), physics, ptr::null_mut())
    })
}

unsafe extern "C" fn sense_is_probing(obj: *mut c_void, name: u32, out: *mut i32) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    let result = (|| -> RuntimeResult<i32> {
        let name = match name {
            crate::pawn_sense::NAME_HEAR_NOISE => "HearNoise",
            crate::pawn_sense::NAME_SPECIAL_HANDLING => "SpecialHandling",
            _ => return Err(RuntimeError::new("Invalid canonical sense probe selector")),
        };
        Ok(i32::from(unsafe { access.is_probing(obj.cast(), access.intern_str(name, FindName::Find)?)? }))
    })();
    match result {
        Ok(value) => { if !out.is_null() { unsafe { *out = value; } } 0 }
        Err(_) => tick_error("Canonical IsProbing failed"),
    }
}

unsafe extern "C" fn sense_event_enemy_not_visible(pawn: *mut c_void) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    tick_status!(unsafe { execute_event(access, pawn, "EnemyNotVisible", None) })
}

unsafe extern "C" fn sense_event_hear_noise(pawn: *mut c_void, loudness: f32, noise: *mut c_void) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    let mut params = HearNoiseParams { loudness, noise };
    tick_status!(unsafe {
        execute_event_params(
            access,
            pawn,
            "HearNoise",
            (&mut params as *mut HearNoiseParams).cast(),
            std::mem::size_of::<HearNoiseParams>(),
        )
    })
}

unsafe extern "C" fn sense_event_touch(actor: *mut c_void, other: *mut c_void) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    let mut other = other;
    tick_status!(unsafe {
        execute_event_params(
            access,
            actor,
            "Touch",
            (&mut other as *mut *mut c_void).cast(),
            std::mem::size_of::<*mut c_void>(),
        )
    })
}

unsafe extern "C" fn sense_safe_normal(v: *const c_void, out: *mut c_void) -> i32 {
    if v.is_null() || out.is_null() { return 1; }
    unsafe { out.cast::<FVector>().write(crate::prim::safe_normal(*v.cast::<FVector>())); }
    0
}

unsafe extern "C" fn sense_vector_rotation(v: *const c_void, out: *mut c_void) -> i32 {
    if v.is_null() || out.is_null() { return 1; }
    let rotation = hp2_core::math::vector_rotation(*v.cast::<FVector>());
    unsafe {
        out.cast::<hp2_abi::engine::generated::FRotator>()
            .write(abi_rotator(rotation));
    }
    0
}

unsafe extern "C" fn sense_rotation_vector(v: *const c_void, out: *mut c_void) -> i32 {
    if v.is_null() || out.is_null() { return 1; }
    unsafe {
        out.cast::<FVector>().write(hp2_core::math::rotator_vector(
            core_rotator(*v.cast::<hp2_abi::engine::generated::FRotator>()),
            hp2_core::math::trig_table(),
        ));
    }
    0
}

unsafe extern "C" fn sense_is_a(obj: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    let name = match kind {
        1 => "Pawn",
        2 => "PlayerPawn",
        3 => "Inventory",
        _ => return 1,
    };
    match unsafe { crate::actor_physics_owner::is_a_named(access, obj, name) } {
        Ok(value) => { if !out.is_null() { unsafe { *out = i32::from(value); } } 0 }
        Err(_) => tick_error("Canonical IsA failed"),
    }
}

unsafe extern "C" fn sense_is_a_class_of(obj: *mut c_void, other: *mut c_void, out: *mut i32) -> i32 {
    if obj.is_null() || other.is_null() { return 1; }
    let class = unsafe { (*other.cast::<UObject>()).class };
    let value = unsafe { hp2_core::object_model::is_a(obj.cast(), class.cast()) };
    if !out.is_null() { unsafe { *out = i32::from(value); } }
    0
}

unsafe extern "C" fn sense_level_pawn_head(level: *mut c_void, out: *mut *mut c_void) -> i32 {
    if level.is_null() { return 1; }
    match unsafe { crate::package_map_level_owner::level_info(level.cast::<ULevel>()) } {
        Ok(info) => {
            let value = unsafe { ptr::addr_of!((*info).pawn_list).read_unaligned() };
            if !out.is_null() { unsafe { *out = value; } }
            0
        }
        Err(_) => tick_error("Canonical level pawn list lookup failed"),
    }
}

unsafe extern "C" fn sense_level_team_game(level: *mut c_void, out: *mut i32) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if level.is_null() { return 1; }
    let result = (|| -> RuntimeResult<i32> {
        let info = unsafe { crate::package_map_level_owner::level_info(level.cast::<ULevel>())? };
        let game = unsafe { ptr::addr_of!((*info).game).read_unaligned() }.cast::<UObject>();
        if game.is_null() {
            return Err(RuntimeError::new("Canonical level TeamGame has no GameInfo"));
        }
        let class = unsafe { (*game).class };
        let name = access.intern_str("bTeamGame", FindName::Find)?;
        let bool_class = access.native_class("BoolProperty")?;
        let property = unsafe { access.find_field(class.cast(), name, bool_class) }
            .cast::<UBoolProperty>();
        if property.is_null() {
            return Err(RuntimeError::new("Canonical GameInfo is missing bTeamGame"));
        }
        let offset = unsafe { ptr::addr_of!((*property.cast::<UProperty>()).offset).read_unaligned() };
        let mask = unsafe { ptr::addr_of!((*property).bit_mask).read_unaligned() };
        let storage = unsafe {
            game.cast::<u8>()
                .offset(offset as isize)
                .cast::<u32>()
                .read_unaligned()
        };
        Ok(i32::from(storage & mask != 0))
    })();
    match result {
        Ok(value) => { if !out.is_null() { unsafe { *out = value; } } 0 }
        Err(_) => tick_error("Canonical level TeamGame lookup failed"),
    }
}

unsafe extern "C" fn sense_level_time(level: *mut c_void, out: *mut f32) -> i32 {
    if level.is_null() || out.is_null() { return 1; }
    let raw = unsafe { ptr::addr_of!((*level.cast::<ULevel>()).time_seconds).read_unaligned() };
    unsafe { *out = hp2_core::time::get_float(i64::from_ne_bytes(raw)); }
    0
}

unsafe extern "C" fn sense_teams_differ(a: *mut c_void, b: *mut c_void, out: *mut i32) -> i32 {
    if a.is_null() || b.is_null() { return 1; }
    let a = unsafe {
        ptr::addr_of!((*a.cast::<hp2_abi::engine::generated::APlayerReplicationInfo>()).team)
            .read_unaligned()
    };
    let b = unsafe {
        ptr::addr_of!((*b.cast::<hp2_abi::engine::generated::APlayerReplicationInfo>()).team)
            .read_unaligned()
    };
    if !out.is_null() { unsafe { *out = i32::from(a != b); } }
    0
}

unsafe extern "C" fn sense_zone_default_gravity_z(zone: *mut c_void, out: *mut f32) -> i32 {
    if zone.is_null() || out.is_null() { return 1; }
    let Some(access) = tick_access() else { return 1; };
    let class = unsafe { ptr::addr_of!((*zone.cast::<UObject>()).class).read_unaligned() };
    match unsafe { access.default_object(class.cast()) } {
        Ok(default) => {
            let gravity = unsafe { ptr::addr_of!((*default.cast::<hp2_abi::engine::generated::AZoneInfo>()).zone_gravity).read_unaligned() };
            unsafe { out.write_unaligned(gravity.z); }
            0
        }
        Err(error) => {
            if let Some(context) = tick_context() { unsafe { (*context).error.borrow_mut().get_or_insert(error); } }
            1
        }
    }
}

unsafe extern "C" fn sense_actor_reachable(
    pawn: *mut c_void,
    target: *mut c_void,
    know_visible: i32,
    out: *mut i32,
) -> i32 {
    let Some(access) = tick_access() else { return 1; };
    match unsafe { crate::pawn_reach_owner::actor_reachable(access, pawn.cast(), target.cast(), know_visible) } {
        Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
        Err(error) => {
            if let Some(context) = tick_context() {
                let mut first = unsafe { &*context }.error.borrow_mut();
                if first.is_none() { *first = Some(error); }
            }
            1
        }
    }
}

unsafe extern "C" fn sense_find_path_toward(
    pawn: *mut c_void,
    goal: *mut c_void,
    single: i32,
    best: *mut *mut c_void,
    clear: i32,
    out: *mut i32,
) -> i32 {
    let Some(access) = tick_access() else { return 1; };
    match crate::route_owner::with_services(access, |services| unsafe {
        crate::route::find_path_toward(services, pawn.cast(), goal.cast(), single, best.cast(), clear)
            .map_err(|_| RuntimeError::new("Canonical special route search failed"))
    }) {
        Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
        Err(error) => {
            if let Some(context) = tick_context() { unsafe { (*context).error.borrow_mut().get_or_insert(error); } }
            1
        }
    }
}

unsafe extern "C" fn sense_event_special_handling(
    actor: *mut c_void,
    pawn: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    let Some(access) = tick_access() else { return 1; };
    match unsafe { crate::route_owner::pawn_event(access, actor.cast(), "SpecialHandling", "Other", pawn) } {
        Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
        Err(error) => {
            if let Some(context) = tick_context() { unsafe { (*context).error.borrow_mut().get_or_insert(error); } }
            1
        }
    }
}

unsafe extern "C" fn sense_can_hear(
    pawn: *mut c_void,
    noise_loc: *const c_void,
    loudness: f32,
    other: *mut c_void,
    out: *mut i32,
) -> i32 {
    if pawn.is_null() || noise_loc.is_null() || out.is_null() {
        return 1;
    }
    match unsafe {
        crate::pawn_sense::can_hear(
            &CANONICAL_SENSE_SERVICES,
            pawn.cast(),
            *noise_loc.cast::<FVector>(),
            loudness,
            other.cast(),
        )
    } {
        Ok(value) => { unsafe { *out = value; } 0 }
        Err(_) => tick_error("Canonical CanHear failed"),
    }
}

/// Canonical APawn::LineOfSightTo with a scoped, reentrant Core query context.
///
/// # Safety
/// Pawn, target, level/model and any reached actors are live canonical records.
pub(crate) unsafe fn line_of_sight_to(
    access: CoreAccess<'_>, pawn: *mut APawn, target: *mut AActor, show_self: i32,
) -> RuntimeResult<bool> {
    with_tick_context(access, |_| unsafe {
        crate::pawn_sense::line_of_sight_to(&CANONICAL_SENSE_SERVICES, pawn, target, show_self)
            .map(|visible| visible != 0)
            .map_err(|_| RuntimeError::new("Canonical LineOfSightTo failed"))
    })
}

/// Canonical wall adjustment, including real shape extents and falling transitions.
///
/// # Safety
/// Pawn/world, collision shapes and reached actors remain live through synchronous queries.
pub(crate) unsafe fn pick_wall_adjust(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<i32> {
    with_tick_context(access, |context| unsafe {
        let mut hit = crate::levact::check_result(1.);
        crate::pawn_sense::pick_wall_adjust_with(
            &CANONICAL_SENSE_SERVICES, pawn, &raw mut hit,
            |actor| match crate::collision_owner::actor_cylinder_extent(access, actor) {
                Ok(extent) => Ok(extent),
                Err(error) => {
                    let mut first = (*context).error.borrow_mut();
                    if first.is_none() { *first = Some(error); }
                    Err(1)
                }
            },
        ).map_err(|_| RuntimeError::new("Canonical PickWallAdjust failed"))
    })
}

unsafe extern "C" fn sense_line_of_sight_to(
    pawn: *mut c_void,
    other: *mut c_void,
    show_self: i32,
    out: *mut u32,
) -> i32 {
    match unsafe {
        crate::pawn_sense::line_of_sight_to(
            &CANONICAL_SENSE_SERVICES,
            pawn.cast(),
            other.cast(),
            show_self,
        )
    } {
        Ok(value) => { if !out.is_null() { unsafe { *out = value; } } 0 }
        Err(_) => tick_error("Canonical LineOfSightTo failed"),
    }
}

#[repr(C, packed(4))]
struct HearNoiseParams {
    loudness: f32,
    noise: *mut c_void,
}

static CANONICAL_SENSE_SERVICES: crate::pawn_sense::Hp2PawnSenseServices =
    crate::pawn_sense::Hp2PawnSenseServices {
        fast_line_check: sense_fast_line_check,
        single_line_check: sense_single_line_check,
        app_frand: sense_app_frand,
        set_physics: sense_set_physics,
        is_probing: sense_is_probing,
        event_hear_noise: sense_event_hear_noise,
        event_enemy_not_visible: sense_event_enemy_not_visible,
        event_touch: sense_event_touch,
        safe_normal: sense_safe_normal,
        vector_rotation: sense_vector_rotation,
        rotation_vector: sense_rotation_vector,
        is_a: sense_is_a,
        is_a_class_of: sense_is_a_class_of,
        level_pawn_head: sense_level_pawn_head,
        level_team_game: sense_level_team_game,
        level_time: sense_level_time,
        teams_differ: sense_teams_differ,
        zone_default_gravity_z: sense_zone_default_gravity_z,
        actor_reachable: sense_actor_reachable,
        find_path_toward: sense_find_path_toward,
        event_special_handling: sense_event_special_handling,
        can_hear: sense_can_hear,
        line_of_sight_to: sense_line_of_sight_to,
    };

unsafe fn input_find_property(
    klass: *mut c_void,
    name: *const c_char,
    out: *mut *mut c_void,
    class_name: &str,
) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if klass.is_null() || name.is_null() {
        return tick_error("Canonical property lookup has null arguments");
    }
    let result = (|| -> RuntimeResult<*mut c_void> {
        let text = unsafe { CStr::from_ptr(name) }.to_string_lossy();
        let name = access.intern_str(&text, FindName::Find)?;
        let field_class = access.native_class(class_name)?;
        let field = unsafe {
            access.find_field(
                klass.cast::<hp2_abi::engine::generated::UStruct>(),
                name,
                field_class,
            )
        };
        Ok(field.cast())
    })();
    match result {
        Ok(field) => { if !out.is_null() { unsafe { *out = field; } } 0 }
        Err(_) => tick_error("Canonical property lookup failed"),
    }
}

unsafe extern "C" fn input_find_bool_property(
    klass: *mut c_void,
    name: *const c_char,
    out: *mut *mut c_void,
) -> i32 {
    input_find_property(klass, name, out, "BoolProperty")
}

unsafe extern "C" fn input_find_object_property(
    klass: *mut c_void,
    name: *const c_char,
    out: *mut *mut c_void,
) -> i32 {
    input_find_property(klass, name, out, "ObjectProperty")
}

unsafe extern "C" fn input_property_offset(property: *mut c_void, out: *mut i32) -> i32 {
    if property.is_null() || out.is_null() { return 1; }
    let offset = unsafe { ptr::addr_of!((*property.cast::<UProperty>()).offset).read_unaligned() };
    unsafe { *out = offset; }
    0
}

unsafe extern "C" fn input_bool_property_mask(property: *mut c_void, out: *mut u32) -> i32 {
    if property.is_null() || out.is_null() { return 1; }
    let mask = unsafe { ptr::addr_of!((*property.cast::<UBoolProperty>()).bit_mask).read_unaligned() };
    unsafe { *out = mask; }
    0
}

unsafe extern "C" fn input_object_class(object: *mut c_void, out: *mut *mut c_void) -> i32 {
    if object.is_null() || out.is_null() { return 1; }
    unsafe { *out = (*object.cast::<UObject>()).class; }
    0
}

unsafe extern "C" fn input_class_super(class: *mut c_void, out: *mut *mut c_void) -> i32 {
    if class.is_null() || out.is_null() { return 1; }
    unsafe { *out = (*class.cast::<UClass>()).super_field; }
    0
}

unsafe extern "C" fn input_is_a(object: *mut c_void, class: *mut c_void, out: *mut i32) -> i32 {
    if object.is_null() || class.is_null() || out.is_null() { return 1; }
    unsafe { *out = i32::from(hp2_core::object_model::is_a(object.cast(), class.cast())); }
    0
}

unsafe extern "C" fn input_class_path_equals(
    class: *mut c_void,
    path: *const c_char,
    out: *mut i32,
) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if class.is_null() || path.is_null() {
        return tick_error("Canonical class-path comparison has null arguments");
    }
    let result = (|| -> RuntimeResult<bool> {
        let text = unsafe { CStr::from_ptr(path) }.to_string_lossy();
        let full = unsafe { access.object_full_name(class.cast())? };
        let path_start = full
            .iter()
            .position(|unit| *unit == ' ' as u32)
            .map_or(0, |index| index + 1);
        let path_text: String = full[path_start..]
            .iter()
            .filter_map(|unit| char::from_u32(*unit))
            .collect();
        Ok(path_text.eq_ignore_ascii_case(&text))
    })();
    match result {
        Ok(value) => { if !out.is_null() { unsafe { *out = i32::from(value); } } 0 }
        Err(_) => tick_error("Canonical class-path comparison failed"),
    }
}

unsafe extern "C" fn input_state_node(object: *mut c_void, out: *mut *mut c_void) -> i32 {
    if object.is_null() || out.is_null() { return 1; }
    let frame = unsafe { state_frame(object) };
    let node = match frame {
        Ok(frame) if !frame.is_null() => {
            unsafe { ptr::addr_of!((*frame).state_node).read_unaligned() }
        }
        _ => ptr::null_mut(),
    };
    unsafe { *out = node; }
    0
}

unsafe extern "C" fn input_state_super(state: *mut c_void, out: *mut *mut c_void) -> i32 {
    if state.is_null() || out.is_null() { return 1; }
    unsafe { *out = (*state.cast::<UField>()).super_field; }
    0
}

unsafe extern "C" fn input_state_name_equals(
    state: *mut c_void,
    name: *const c_char,
    out: *mut i32,
) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if state.is_null() || name.is_null() { return 1; }
    let text = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let value = match access.intern_str(&text, FindName::Find) {
        Ok(name) => (unsafe { (*state.cast::<UObject>()).name } == name.to_ne_bytes()),
        Err(_) => false,
    };
    if !out.is_null() { unsafe { *out = i32::from(value); } }
    0
}

unsafe extern "C" fn input_log_missing_property(name: *const c_char) -> i32 {
    let Some(access) = tick_access() else { return 1 };
    if name.is_null() { return 1; }
    let result = (|| -> RuntimeResult<()> {
        let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
        let category = access.intern_str("Warning", FindName::Add)?;
        let mut message: Vec<u32> = "Modern third-person controls disabled: HGame.Harry is missing required property "
            .chars()
            .map(u32::from)
            .collect();
        message.extend(name.chars().map(u32::from));
        access.emit_log(category, &message)
    })();
    tick_status!(result)
}

unsafe extern "C" fn input_sin_tab(angle: i32) -> f32 {
    hp2_core::math::sin_tab_inline(angle, hp2_core::math::trig_table())
}

unsafe extern "C" fn input_cos_tab(angle: i32) -> f32 {
    hp2_core::math::cos_tab_inline(angle, hp2_core::math::trig_table())
}

static CANONICAL_INPUT_SERVICES: crate::pawn_native_input::Hp2PawnNativeInputServices =
    crate::pawn_native_input::Hp2PawnNativeInputServices {
        find_bool_property: input_find_bool_property,
        find_object_property: input_find_object_property,
        property_offset: input_property_offset,
        bool_property_mask: input_bool_property_mask,
        object_class: input_object_class,
        class_super: input_class_super,
        is_a: input_is_a,
        class_path_equals: input_class_path_equals,
        state_node: input_state_node,
        state_super: input_state_super,
        state_name_equals: input_state_name_equals,
        log_missing_property: input_log_missing_property,
        sin_tab: input_sin_tab,
        cos_tab: input_cos_tab,
    };
