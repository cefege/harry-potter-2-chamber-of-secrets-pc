//! Direct canonical handlers for Engine script natives.

use hp2_abi::engine::generated::{AActor, APlayerPawn, FFrame, TArrayRaw, UGameEngine, ULevel, UObject};
use hp2_core::{
    canonical_vm_native::{finish_arguments, step_argument, step_argument_out, VmNativeSpec},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    native_text::{code_units, tchar_units},
};
use std::{ffi::c_void, ptr};

/// Immutable canonical Engine native registrations installed before script metadata binding.
pub static ENGINE_VM_NATIVES: &[VmNativeSpec] = &[
    VmNativeSpec {
        class_name: "Engine.Pawn",
        function_name: "EAdjustJump",
        native_index: Some(523),
        handler: pawn_adjust_jump,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetBase",
        native_index: Some(298),
        handler: set_base,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetOwner",
        native_index: Some(272),
        handler: set_owner,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Move",
        native_index: Some(266),
        handler: move_actor,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Destroy",
        native_index: Some(279),
        handler: destroy_actor,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Spawn",
        native_index: Some(278),
        handler: spawn_actor,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetRotation",
        native_index: Some(299),
        handler: set_rotation,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "MoveSmooth",
        native_index: Some(3969),
        handler: move_smooth,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetLocation",
        native_index: Some(267),
        handler: set_location,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetCollision",
        native_index: Some(262),
        handler: set_collision,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetCollisionSize",
        native_index: Some(283),
        handler: set_collision_size,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetTimer",
        native_index: Some(280),
        handler: set_timer,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SaveGameExists",
        native_index: Some(3972),
        handler: save_game_exists,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "SetPhysics",
        native_index: Some(3970),
        handler: set_physics,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "FastTrace",
        native_index: Some(548),
        handler: fast_trace,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Trace",
        native_index: Some(277),
        handler: trace,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetWorldCollisionBox",
        native_index: Some(286),
        handler: get_world_collision_box,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetRenderExtent",
        native_index: Some(274),
        handler: get_render_extent,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "LinkSkelAnim",
        native_index: None,
        handler: link_skel_anim,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetSoundDuration",
        native_index: None,
        handler: get_sound_duration,
    },
    VmNativeSpec {
        class_name: "Engine.WarpZoneInfo",
        function_name: "Warp",
        native_index: Some(314),
        handler: warp,
    },
    VmNativeSpec {
        class_name: "Engine.WarpZoneInfo",
        function_name: "UnWarp",
        native_index: Some(315),
        handler: un_warp,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Multiply_ColorFloat",
        native_index: Some(552),
        handler: multiply_color_float,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Multiply_FloatColor",
        native_index: Some(550),
        handler: multiply_float_color,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Add_ColorColor",
        native_index: Some(551),
        handler: add_color_color,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "Subtract_ColorColor",
        native_index: Some(549),
        handler: subtract_color_color,
    },
    VmNativeSpec {
        class_name: "Engine.PlayerPawn",
        function_name: "GetDefaultURL",
        native_index: None,
        handler: get_default_url,
    },
    VmNativeSpec {
        class_name: "Engine.PlayerPawn",
        function_name: "UpdateURL",
        native_index: Some(546),
        handler: update_url,
    },
    VmNativeSpec {
        class_name: "Engine.PlayerPawn",
        function_name: "GetEntryLevel",
        native_index: None,
        handler: get_entry_level,
    },
    VmNativeSpec {
        class_name: "Engine.PlayerPawn",
        function_name: "ResetKeyboard",
        native_index: Some(544),
        handler: reset_keyboard,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetURLMap",
        native_index: Some(547),
        handler: get_url_map,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetMapName",
        native_index: Some(539),
        handler: get_map_name,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetNextInt",
        native_index: None,
        handler: get_next_int,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetNextIntDesc",
        native_index: None,
        handler: get_next_int_desc,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetCacheEntry",
        native_index: None,
        handler: get_cache_entry,
    },
    VmNativeSpec {
        class_name: "Engine.Actor",
        function_name: "GetNextSkin",
        native_index: Some(545),
        handler: get_next_skin,
    },
    VmNativeSpec {
        class_name: "Engine.GameInfo",
        function_name: "ParseKillMessage",
        native_index: None,
        handler: parse_kill_message,
    },
    VmNativeSpec { class_name: "Engine.ParticleFX", function_name: "NumParticles", native_index: Some(431), handler: particle_count },
    VmNativeSpec { class_name: "Engine.ParticleFX", function_name: "AddParticle", native_index: Some(432), handler: particle_add },
    VmNativeSpec { class_name: "Engine.ParticleFX", function_name: "GetParticleParams", native_index: Some(433), handler: particle_get_params },
    VmNativeSpec { class_name: "Engine.ParticleFX", function_name: "SetParticleParams", native_index: Some(434), handler: particle_set_params },
    VmNativeSpec { class_name: "Engine.ParticleFX", function_name: "RecomputeDeltas", native_index: Some(435), handler: particle_recompute },
    crate::canvas_vm_native::NATIVE_FUNCTIONS[0],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[1],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[2],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[3],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[4],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[5],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[6],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[7],
    crate::canvas_vm_native::NATIVE_FUNCTIONS[8],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[0],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[1],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[2],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[3],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[4],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[5],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[6],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[7],
    crate::actor_iterator_vm_native::NATIVE_FUNCTIONS[8],
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PickTarget", native_index: Some(531), handler: pawn_pick_target },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PickAnyTarget", native_index: Some(534), handler: pawn_pick_any_target },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "AddPawn", native_index: Some(529), handler: pawn_add },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "RemovePawn", native_index: Some(530), handler: pawn_remove },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "WaitForLanding", native_index: Some(527), handler: pawn_wait_for_landing },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollWaitForLanding", native_index: Some(528), handler: pawn_poll_wait_for_landing },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "StopWaiting", native_index: None, handler: pawn_stop_waiting },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindStairRotation", native_index: Some(524), handler: pawn_find_stair_rotation },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PickWallAdjust", native_index: Some(526), handler: pawn_pick_wall_adjust },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "LineOfSightTo", native_index: Some(514), handler: pawn_line_of_sight },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "CanSee", native_index: Some(533), handler: pawn_can_see },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "pointReachable", native_index: Some(521), handler: pawn_point_reachable },
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[0],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[1],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[2],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[3],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[4],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[5],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[6],
    crate::actor_audio_vm_native::NATIVE_FUNCTIONS[7],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[0],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[1],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[2],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[3],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[4],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[5],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[6],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[7],
    crate::pawn_path_vm_native::NATIVE_FUNCTIONS[8],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[0],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[1],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[2],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[3],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[4],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[5],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[6],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[7],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[8],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[9],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[10],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[11],
    crate::pawn_move_vm_native::NATIVE_FUNCTIONS[12],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[0],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[1],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[2],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[3],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[4],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[5],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[6],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[7],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[8],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[9],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[10],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[11],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[12],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[13],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[14],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[15],
    crate::actor_script_vm_native::NATIVE_FUNCTIONS[16],
    crate::console_vm_native::NATIVE_FUNCTIONS[0],
    crate::console_vm_native::NATIVE_FUNCTIONS[1],
    crate::console_vm_native::NATIVE_FUNCTIONS[2],
    crate::console_vm_native::NATIVE_FUNCTIONS[3],
    crate::console_vm_native::NATIVE_FUNCTIONS[4],
    crate::console_vm_native::NATIVE_FUNCTIONS[5],
    crate::console_vm_native::NATIVE_FUNCTIONS[6],
    crate::console_vm_native::NATIVE_FUNCTIONS[7],
    crate::console_vm_native::NATIVE_FUNCTIONS[8],
    crate::console_vm_native::NATIVE_FUNCTIONS[9],
    crate::console_vm_native::NATIVE_FUNCTIONS[10],
    crate::console_vm_native::NATIVE_FUNCTIONS[11],
    crate::console_vm_native::NATIVE_FUNCTIONS[12],
    crate::console_vm_native::NATIVE_FUNCTIONS[13],
    crate::console_vm_native::NATIVE_FUNCTIONS[14],
    crate::console_vm_native::NATIVE_FUNCTIONS[15],
    crate::console_vm_native::NATIVE_FUNCTIONS[16],
    crate::console_vm_native::NATIVE_FUNCTIONS[17],
    crate::console_vm_native::NATIVE_FUNCTIONS[18],
    crate::console_vm_native::NATIVE_FUNCTIONS[19],
    crate::console_vm_native::NATIVE_FUNCTIONS[20],
    crate::console_vm_native::NATIVE_FUNCTIONS[21],
    crate::console_vm_native::NATIVE_FUNCTIONS[22],
    crate::console_vm_native::NATIVE_FUNCTIONS[23],
    crate::console_vm_native::NATIVE_FUNCTIONS[24],
    crate::console_vm_native::NATIVE_FUNCTIONS[25],
    crate::console_vm_native::NATIVE_FUNCTIONS[26],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[0],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[1],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[2],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[3],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[4],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[5],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[6],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[7],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[8],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[9],
    crate::actor_file_vm_native::NATIVE_FUNCTIONS[10],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[0],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[1],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[2],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[3],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[4],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[5],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[6],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[7],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[8],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[9],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[10],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[11],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[12],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[13],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[14],
    crate::stat_log_vm_native::NATIVE_FUNCTIONS[15],
];

/// Applies `AActor.SetBase` to canonical actors and dispatches relation events through Core.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`; `result`
/// must satisfy the native's void result layout.
unsafe fn set_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut new_base = std::ptr::null_mut::<AActor>();
    unsafe {
        step_argument(access, frame, (&raw mut new_base).cast())?;
        finish_arguments(access, frame)?;
        crate::level_lifecycle_owner::set_base(access, object.cast(), new_base, true)
    }
}
/// Applies `AActor.SetOwner` to canonical actors and dispatches relation events through Core.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`; `result`
/// must satisfy the native's void result layout.
unsafe fn set_owner(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut new_owner = std::ptr::null_mut::<AActor>();
    unsafe {
        step_argument(access, frame, (&raw mut new_owner).cast())?;
        finish_arguments(access, frame)?;
        crate::level_lifecycle_owner::set_owner(access, object.cast(), new_owner)
    }
}
/// Applies `AActor.Move` through canonical Level movement and writes its UBOOL result.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn move_actor(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut delta = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    unsafe {
        step_argument(access, frame, (&raw mut delta).cast())?;
        finish_arguments(access, frame)?;
        let level = (*actor).x_level.cast();
        let rotation = std::ptr::addr_of!((*actor).rotation).read_unaligned();
        let mut hit: hp2_abi::engine::generated::FCheckResult = std::mem::zeroed();
        let moved = crate::level_move_owner::move_actor(
            access, level, actor, delta, rotation, &mut hit, false, false, false, false,
        )?;
        result.cast::<u32>().write_unaligned(u32::from(moved));
    }
    Ok(())
}
/// Runs canonical `AActor.Destroy` and writes its UBOOL return value.
///
/// # Safety
/// `object` and `frame` point to live canonical VM values owned by `access`; `result`
/// points to a writable UBOOL result slot.
unsafe fn destroy_actor(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        let actor = object.cast::<AActor>();
        let destroyed = crate::level_lifecycle_owner::destroy_actor(
            access,
            (*actor).x_level.cast(),
            actor,
            false,
        )?;
        result.cast::<u32>().write_unaligned(u32::from(destroyed));
    }
    Ok(())
}
/// Runs script-native spawn on canonical actors and writes the spawned object reference.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn spawn_actor(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut class = std::ptr::null_mut::<hp2_abi::engine::generated::UClass>();
    let mut owner = std::ptr::null_mut::<AActor>();
    let mut spawn_tag = hp2_abi::engine::generated::FName { index: 0 };
    let mut location = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
    unsafe {
        step_argument(access, frame, (&raw mut class).cast())?;
        step_argument(access, frame, (&raw mut owner).cast())?;
        step_argument(access, frame, (&raw mut spawn_tag).cast())?;
        location = std::ptr::addr_of!((*actor).location).read_unaligned();
        step_argument(access, frame, (&raw mut location).cast())?;
        rotation = std::ptr::addr_of!((*actor).rotation).read_unaligned();
        step_argument(access, frame, (&raw mut rotation).cast())?;
        finish_arguments(access, frame)?;
        crate::live_trace_owner::creature_class_resolution(access, class, actor);
        let spawned = crate::level_spawn_actor_owner::spawn_actor(
            access,
            (*actor).x_level.cast(),
            class,
            hp2_abi::engine::generated::FName { index: 0 },
            owner,
            (*actor).instigator.cast(),
            location,
            rotation,
            std::ptr::null_mut(),
            false,
            false,
        )?;
        if !spawned.is_null() {
            std::ptr::addr_of_mut!((*spawned).tag).write_unaligned(spawn_tag.index.to_ne_bytes());
        }
        result.cast::<*mut AActor>().write_unaligned(spawned);
    }
    Ok(())
}
/// Moves the canonical actor to the requested rotation through Level's movement policy.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn set_rotation(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
    unsafe {
        step_argument(access, frame, (&raw mut rotation).cast())?;
        finish_arguments(access, frame)?;
        let level = (*actor).x_level.cast();
        let mut hit: hp2_abi::engine::generated::FCheckResult = std::mem::zeroed();
        let moved = crate::level_move_owner::move_actor(
            access,
            level,
            actor,
            hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 },
            rotation,
            &mut hit,
            false,
            false,
            false,
            false,
        )?;
        result.cast::<u32>().write_unaligned(u32::from(moved));
    }
    Ok(())
}

/// Runs the canonical actor MoveSmooth policy and writes its UBOOL result.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn move_smooth(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut delta = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    unsafe {
        step_argument(access, frame, (&raw mut delta).cast())?;
        finish_arguments(access, frame)?;
        let moved = crate::actor_physics_owner::move_smooth(access, object.cast(), delta)?;
        result.cast::<u32>().write_unaligned(u32::from(moved));
    }
    Ok(())
}
/// Applies `AActor.SetLocation` with FarMoveActor's placement and touch notifications.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn set_location(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut location = unsafe { std::ptr::addr_of!((*actor).location).read_unaligned() };
    unsafe {
        step_argument(access, frame, (&raw mut location).cast())?;
        finish_arguments(access, frame)?;
        let moved = crate::level_move_owner::far_move_actor(
            access, (*actor).x_level.cast(), actor, location, false, false,
        )?;
        result.cast::<u32>().write_unaligned(u32::from(moved));
    }
    Ok(())
}
struct CoreActorCollisionOps<'a>(CoreAccess<'a>);

impl crate::actor::ActorCollisionOps for CoreActorCollisionOps<'_> {
    type Error = hp2_core::core_access::RuntimeError;

    unsafe fn has_hash(&mut self, level: *mut c_void) -> RuntimeResult<bool> {
        Ok(!unsafe { (*level.cast::<hp2_abi::engine::generated::ULevel>()).hash }.is_null())
    }
    unsafe fn hash_remove(
        &mut self,
        level: *mut c_void,
        actor: *mut AActor,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_remove(self.0, level.cast(), actor, false) }
    }
    unsafe fn hash_add(&mut self, level: *mut c_void, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_add(self.0, level.cast(), actor) }
    }
}

/// Applies `AActor.SetCollision` directly to canonical actors.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`.
unsafe fn set_collision(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut collide_actors = unsafe { u32::from((*actor).b_collide_actors()) };
    let mut block_actors = unsafe { u32::from((*actor).b_block_actors()) };
    let mut block_players = unsafe { u32::from((*actor).b_block_players()) };
    unsafe {
        step_argument(access, frame, (&raw mut collide_actors).cast())?;
        step_argument(access, frame, (&raw mut block_actors).cast())?;
        step_argument(access, frame, (&raw mut block_players).cast())?;
        finish_arguments(access, frame)?;
        let mut ops = CoreActorCollisionOps(access);
        crate::actor::set_collision_with_ops(
            &mut ops,
            actor,
            collide_actors != 0,
            block_actors != 0,
            block_players != 0,
        )
    }
}

/// Applies `AActor.SetCollisionSize` and writes its true UBOOL result.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn set_collision_size(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut radius = unsafe { std::ptr::addr_of!((*actor).collision_radius).read_unaligned() };
    let mut height = unsafe { std::ptr::addr_of!((*actor).collision_height).read_unaligned() };
    let mut width = unsafe { std::ptr::addr_of!((*actor).collision_width).read_unaligned() };
    unsafe {
        step_argument(access, frame, (&raw mut radius).cast())?;
        step_argument(access, frame, (&raw mut height).cast())?;
        step_argument(access, frame, (&raw mut width).cast())?;
        finish_arguments(access, frame)?;
        std::ptr::addr_of_mut!((*actor).collision_width).write_unaligned(width);
        let mut ops = CoreActorCollisionOps(access);
        crate::actor::set_collision_size_with_ops(&mut ops, actor, radius, height)?;
        result.cast::<u32>().write_unaligned(1);
    }
    Ok(())
}
/// Applies `AActor.SetTimer` through its field-only actor policy.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`.
unsafe fn set_timer(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut timer_rate = 0.0_f32;
    let mut looping = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut timer_rate).cast())?;
        step_argument(access, frame, (&raw mut looping).cast())?;
        finish_arguments(access, frame)?;
        crate::actor::exec_set_timer(object.cast(), timer_rate, looping as i32);
    }
    Ok(())
}
/// Queries the canonical save-slot path for `Save9.usa`.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn save_game_exists(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        let size = crate::file_channel_owner::save_slot_file_size(access, 9)?;
        result
            .cast::<u32>()
            .write_unaligned(u32::from(crate::actor::save_game_exists(size)));
    }
    Ok(())
}
struct CoreSetPhysicsOps<'a>(CoreAccess<'a>);

impl crate::physic::dispatch::SetPhysicsOps for CoreSetPhysicsOps<'_> {
    type Error = hp2_core::core_access::RuntimeError;

    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::set_base(self.0, actor, new_base, true) }
    }
    unsafe fn find_base(&mut self, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::actor_physics_owner::find_base(self.0, actor) }
    }
}

/// Applies `AActor.SetPhysics` through the shared Core/native physics policy.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`.
unsafe fn set_physics(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut new_physics = 0_u8;
    unsafe {
        step_argument(access, frame, (&raw mut new_physics).cast())?;
        finish_arguments(access, frame)?;
        let mut ops = CoreSetPhysicsOps(access);
        crate::physic::dispatch::set_physics_with_ops(
            &mut ops,
            object.cast(),
            new_physics,
            std::ptr::null_mut(),
        )
    }
}
/// Performs `AActor.Trace` with canonical Level tracing and live reference arguments.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn trace(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut hit_location = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut hit_normal = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut trace_end = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut trace_start = unsafe { std::ptr::addr_of!((*actor).location).read_unaligned() };
    let mut trace_actors = unsafe { u32::from((*actor).b_collide_actors()) };
    let mut trace_extent = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    unsafe {
        let hit_location_ptr = step_argument_out(access, frame, (&raw mut hit_location).cast())?;
        let hit_normal_ptr = step_argument_out(access, frame, (&raw mut hit_normal).cast())?;
        step_argument(access, frame, (&raw mut trace_end).cast())?;
        step_argument(access, frame, (&raw mut trace_start).cast())?;
        step_argument(access, frame, (&raw mut trace_actors).cast())?;
        step_argument(access, frame, (&raw mut trace_extent).cast())?;
        finish_arguments(access, frame)?;
        let mut hit: hp2_abi::engine::generated::FCheckResult = std::mem::zeroed();
        hit.time = 1.0;
        let trace_flags = if trace_actors != 0 {
            crate::levact::TRACE_ALL_COLLIDING
        } else {
            crate::levact::TRACE_LEVEL | crate::levact::TRACE_MOVERS
        };
        crate::level_trace_owner::single_line_check(
            access,
            (*actor).x_level.cast(),
            &mut hit,
            actor,
            trace_end,
            trace_start,
            trace_flags,
            trace_extent,
            0,
        )?;
        hit_location_ptr.cast::<hp2_abi::FVector>().write_unaligned(hit.location);

        hit_normal_ptr.cast::<hp2_abi::FVector>().write_unaligned(hit.normal);
        result.cast::<*mut c_void>().write_unaligned(hit.actor);
    }
    Ok(())
}
/// Computes canonical world collision bounds with the requested visual primitive selection.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn get_world_collision_box(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut visual = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut visual).cast())?;
        finish_arguments(access, frame)?;
        let bounds = crate::collision_owner::actor_collision_box_visual(
            access,
            object.cast(),
            visual != 0,
        )?;
        result.cast::<hp2_abi::engine::generated::FBox>().write_unaligned(bounds);
    }
    Ok(())
}

/// Performs `AActor.FastTrace` against the actor's canonical level model.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn fast_trace(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let mut trace_end = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut trace_start = unsafe { std::ptr::addr_of!((*actor).location).read_unaligned() };
    unsafe {
        step_argument(access, frame, (&raw mut trace_end).cast())?;
        step_argument(access, frame, (&raw mut trace_start).cast())?;
        finish_arguments(access, frame)?;
        let level = (*actor).x_level.cast::<hp2_abi::engine::generated::ULevel>();
        let model = (*level).model.cast::<hp2_abi::engine::generated::UModel>();
        let clear = crate::collision_owner::model_fast_line_check(model, trace_end, trace_start)?;
        result.cast::<u32>().write_unaligned(u32::from(clear));
    }
    Ok(())
}
/// Returns local visual primitive bounds as a canonical render extent.
///
/// # Safety
/// `object`, `frame`, and `result` point to live canonical VM values owned by `access`.
unsafe fn get_render_extent(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        let extent = crate::collision_owner::actor_render_extent(access, object.cast())?;
        result.cast::<hp2_abi::FVector>().write_unaligned(extent);
    }
    Ok(())
}

/// Applies `AWarpZoneInfo.Warp` using canonical out-argument lvalues and its direct Rust owner.
///
/// # Safety
/// `object` and `frame` point to live canonical VM values owned by `access`.
unsafe fn warp(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut location = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut velocity = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
    unsafe {
        let location = step_argument_out(access, frame, (&raw mut location).cast())?;
        let velocity = step_argument_out(access, frame, (&raw mut velocity).cast())?;
        let rotation = step_argument_out(access, frame, (&raw mut rotation).cast())?;
        finish_arguments(access, frame)?;
        crate::warp_owner::transform(
            access,
            object.cast(),
            location.cast(),
            velocity.cast(),
            rotation.cast(),
            true,
        )
    }
}

/// Applies `AWarpZoneInfo.UnWarp` using the inverse coordinate transform and Core out arguments.
///
/// # Safety
/// `object` and `frame` point to live canonical VM values owned by `access`.
unsafe fn un_warp(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut location = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut velocity = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
    unsafe {
        let location = step_argument_out(access, frame, (&raw mut location).cast())?;
        let velocity = step_argument_out(access, frame, (&raw mut velocity).cast())?;
        let rotation = step_argument_out(access, frame, (&raw mut rotation).cast())?;
        finish_arguments(access, frame)?;
        crate::warp_owner::transform(
            access,
            object.cast(),
            location.cast(),
            velocity.cast(),
            rotation.cast(),
            false,
        )
    }
}

/// Reads the canonical Sound duration; a null sound returns zero as in the source native.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn get_sound_duration(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut sound = std::ptr::null_mut::<UObject>();
    unsafe {
        step_argument(access, frame, (&raw mut sound).cast())?;
        finish_arguments(access, frame)?;
        if sound.is_null() {
            result.cast::<f32>().write_unaligned(0.0);
        } else {
            let class = access.native_class("Engine.Sound")?;
            if !hp2_core::object_model::is_a(sound.cast(), class.cast()) {
                return Err(hp2_core::core_access::RuntimeError::new(
                    "AActor.GetSoundDuration received a non-Sound object",
                ));
            }
            let duration = crate::sound_owner::get_duration(access, sound.cast())?;
            result.cast::<f32>().write_unaligned(duration);
        }
    }
    Ok(())
}

/// Links or clears the actor's skeletal animation resource.
///
/// # Safety
/// `object` and `frame` must point to live canonical VM values owned by `access`.
unsafe fn link_skel_anim(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut animation = std::ptr::null_mut::<UObject>();
    unsafe {
        step_argument(access, frame, (&raw mut animation).cast())?;
        finish_arguments(access, frame)?;
        if crate::actor_anim::link_skeletal_animation(object.cast(), animation.cast()) != 0 {
            return Err(hp2_core::core_access::RuntimeError::new(
                "AActor.LinkSkelAnim received a null Actor",
            ));
        }
    }
    Ok(())
}
/// Implements the native FColor-times-float operator through the shared color kernel.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn multiply_color_float(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut color = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    let mut factor = 0.0_f32;
    unsafe {
        step_argument(access, frame, (&raw mut color).cast())?;
        step_argument(access, frame, (&raw mut factor).cast())?;
        finish_arguments(access, frame)?;
        result.cast::<hp2_abi::FColor>()
            .write_unaligned(crate::actor_color::multiply_color_float(color, factor));
    }
    Ok(())
}

/// Implements the native float-times-FColor operator through the shared color kernel.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn multiply_float_color(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut factor = 0.0_f32;
    let mut color = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    unsafe {
        step_argument(access, frame, (&raw mut factor).cast())?;
        step_argument(access, frame, (&raw mut color).cast())?;
        finish_arguments(access, frame)?;
        result.cast::<hp2_abi::FColor>()
            .write_unaligned(crate::actor_color::multiply_float_color(factor, color));
    }
    Ok(())
}

/// Implements native FColor addition with the shared color kernel.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn add_color_color(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut left = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    let mut right = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    unsafe {
        step_argument(access, frame, (&raw mut left).cast())?;
        step_argument(access, frame, (&raw mut right).cast())?;
        finish_arguments(access, frame)?;
        result.cast::<hp2_abi::FColor>()
            .write_unaligned(crate::actor_color::add_color_color(left, right));
    }
    Ok(())
}

/// Implements native FColor subtraction with the shared color kernel.
///
/// # Safety
/// `frame` and `result` point to live canonical VM values owned by `access`.
unsafe fn subtract_color_color(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut left = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    let mut right = hp2_abi::FColor { r: 0, g: 0, b: 0, a: 0 };
    unsafe {
        step_argument(access, frame, (&raw mut left).cast())?;
        step_argument(access, frame, (&raw mut right).cast())?;
        finish_arguments(access, frame)?;
        result.cast::<hp2_abi::FColor>()
            .write_unaligned(crate::actor_color::subtract_color_color(left, right));
    }
    Ok(())
}

/// Substitutes the three GameInfo kill-message tokens into a canonical FString result.
///
/// # Safety
/// The frame and result must be live canonical VM storage owned by `access`.
unsafe fn parse_kill_message(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut killer_name = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut victim_name = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut weapon_name = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut kill_message = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };

    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut killer_name).cast())?;
            step_argument(access, frame, (&raw mut victim_name).cast())?;
            step_argument(access, frame, (&raw mut weapon_name).cast())?;
            step_argument(access, frame, (&raw mut kill_message).cast())?;
            finish_arguments(access, frame)?;
            let message = crate::game_info::parse_kill_message(
                hp2_core::native_text::fstring_units(&killer_name),
                hp2_core::native_text::fstring_units(&victim_name),
                hp2_core::native_text::fstring_units(&weapon_name),
                hp2_core::native_text::fstring_units(&kill_message),
            );
            access.write_vm_fstring_result(result.cast(), &message)
        })()
    };

    let release_killer = unsafe { access.release_vm_fstring_argument(&mut killer_name) };
    let release_victim = unsafe { access.release_vm_fstring_argument(&mut victim_name) };
    let release_weapon = unsafe { access.release_vm_fstring_argument(&mut weapon_name) };
    let release_message = unsafe { access.release_vm_fstring_argument(&mut kill_message) };
    operation
        .and(release_killer)
        .and(release_victim)
        .and(release_weapon)
        .and(release_message)
}

/// Resolves the next map through the shared Engine search-path policy.
///
/// # Safety
/// The frame and result must be live canonical VM storage owned by `access`.
unsafe fn get_map_name(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut prefix = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut map = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut dir = 0_i32;
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut prefix).cast())?;
            step_argument(access, frame, (&raw mut map).cast())?;
            step_argument(access, frame, (&raw mut dir).cast())?;
            finish_arguments(access, frame)?;
            access.write_vm_fstring_result(result.cast(), &[])?;
            let value = crate::pawn_native_files_owner::get_map_name(
                access,
                code_units(hp2_core::native_text::fstring_units(&prefix)),
                code_units(hp2_core::native_text::fstring_units(&map)),
                dir,
            )?;
            access.write_vm_fstring_result(result.cast(), tchar_units(&value))
        })()
    };
    let release_prefix = unsafe { access.release_vm_fstring_argument(&mut prefix) };
    let release_map = unsafe { access.release_vm_fstring_argument(&mut map) };
    operation.and(release_prefix).and(release_map)
}

/// Copies the live GameEngine URL map into the VM result FString.
///
/// # Safety
/// The actor, frame, and result must be live canonical VM values owned by `access`.
unsafe fn get_url_map(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; }
    let actor = object.cast::<AActor>();
    if actor.is_null() {
        return Err(RuntimeError::new("GetURLMap received a null canonical Actor"));
    }
    let level = unsafe { (*actor).x_level.cast::<ULevel>() };
    if level.is_null() {
        return Err(RuntimeError::new("GetURLMap Actor has no canonical Level"));
    }
    let engine = unsafe { (*level).base.engine.cast::<UGameEngine>() };
    if engine.is_null() {
        return Err(RuntimeError::new("GetURLMap Level has no canonical GameEngine"));
    }
    let map = unsafe { hp2_core::native_text::fstring_units(std::ptr::addr_of!((*engine).last_url.map)) };
    access.write_vm_fstring_result(result.cast(), map)
}

/// Returns the saved DefaultPlayer URL option through the canonical URL owner.
///
/// # Safety
/// The frame and result must be live canonical VM storage owned by `access`.
unsafe fn get_default_url(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut option = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut option).cast())?;
            finish_arguments(access, frame)?;
            let value = crate::pawn_native_files_owner::get_default_url(
                access,
                code_units(hp2_core::native_text::fstring_units(&option)),
            )?;
            access.write_vm_fstring_result(result.cast(), tchar_units(&value))
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&mut option) };
    operation.and(release)
}

/// Runs PlayerPawn.ResetKeyboard against the actual local input class.
///
/// # Safety
/// `object` and `frame` are live canonical PlayerPawn/VM records.
unsafe fn reset_keyboard(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_files_owner::reset_keyboard(access, object.cast())
    }
}

/// Returns `UGameEngine.GEntry`'s level-info actor without C++ service dispatch.
///
/// # Safety
/// The PlayerPawn, frame, and result must be live canonical VM values owned by `access`.
unsafe fn get_entry_level(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; }
    let pawn = object.cast::<APlayerPawn>();
    if pawn.is_null() {
        return Err(RuntimeError::new("GetEntryLevel received a null canonical PlayerPawn"));
    }
    let level = unsafe { (*pawn).base.base.x_level.cast::<ULevel>() };
    if level.is_null() {
        return Err(RuntimeError::new("GetEntryLevel PlayerPawn has no canonical Level"));
    }
    let engine = unsafe { (*level).base.engine.cast::<UGameEngine>() };
    if engine.is_null() {
        return Err(RuntimeError::new("GetEntryLevel Level has no canonical GameEngine"));
    }
    let entry = unsafe { (*engine).g_entry.cast::<ULevel>() };
    if entry.is_null() {
        return Err(RuntimeError::new("GetEntryLevel GameEngine has no Entry Level"));
    }
    let info = unsafe { crate::package_map_level_owner::level_info(entry)? };
    result.cast::<*mut c_void>().write_unaligned(info.cast());
    Ok(())
}

/// Adds and optionally persists one URL option through the canonical URL policy.
///
/// # Safety
/// The PlayerPawn, frame, and result must be live canonical VM values owned by `access`.
unsafe fn update_url(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut option = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut value = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut save = 0_i32;
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut option).cast())?;
            step_argument(access, frame, (&raw mut value).cast())?;
            step_argument(access, frame, (&raw mut save).cast())?;
            finish_arguments(access, frame)?;
            crate::pawn_native_files_owner::update_url(
                access,
                object.cast(),
                code_units(hp2_core::native_text::fstring_units(&option)),
                code_units(hp2_core::native_text::fstring_units(&value)),
                save != 0,
            )
        })()
    };
    let release_option = unsafe { access.release_vm_fstring_argument(&mut option) };
    let release_value = unsafe { access.release_vm_fstring_argument(&mut value) };
    operation.and(release_option).and(release_value)
}
/// Reads the selected canonical Cache.ini row into its two out FString arguments.
///
/// # Safety
/// The frame, output FString slots, and result must be live canonical VM storage.
unsafe fn get_cache_entry(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut number = 0_i32;
    let mut guid = std::ptr::null_mut::<c_void>();
    let mut filename = std::ptr::null_mut::<c_void>();
    unsafe {
        step_argument(access, frame, (&raw mut number).cast())?;
        let guid_out = step_argument_out(access, frame, (&raw mut guid).cast())?;
        let filename_out = step_argument_out(access, frame, (&raw mut filename).cast())?;
        finish_arguments(access, frame)?;
        access.write_vm_fstring_result(guid_out.cast(), &[])?;
        access.write_vm_fstring_result(filename_out.cast(), &[])?;
        let entry = crate::pawn_native_files_owner::get_cache_entry(access, number)?;
        if let Some((guid, filename)) = entry {
            access.write_vm_fstring_result(guid_out.cast(), tchar_units(&guid))?;
            access.write_vm_fstring_result(filename_out.cast(), tchar_units(&filename))?;
            result.cast::<i32>().write_unaligned(1);
        } else {
            result.cast::<i32>().write_unaligned(0);
        }
    }
    Ok(())
}

/// Returns the next registry object name using Core-owned registry policy.
///
/// # Safety
/// The frame and FString result must be live canonical VM storage owned by `access`.
unsafe fn get_next_int(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let mut class_name = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut number = 0_i32;
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut class_name).cast())?;
            step_argument(access, frame, (&raw mut number).cast())?;
            finish_arguments(access, frame)?;
            let value = crate::pawn_native_files_owner::get_next_int(
                access,
                code_units(hp2_core::native_text::fstring_units(&class_name)),
                number,
            )?;
            access.write_vm_fstring_result(result.cast(), tchar_units(&value))
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&mut class_name) };
    operation.and(release)
}

/// Returns registry object and description strings through canonical out FString slots.
///
/// # Safety
/// The frame and out FString slots must be live canonical VM storage owned by `access`.
unsafe fn get_next_int_desc(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut class_name = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut number = 0_i32;
    let mut entry = std::ptr::null_mut::<c_void>();
    let mut description = std::ptr::null_mut::<c_void>();
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut class_name).cast())?;
            step_argument(access, frame, (&raw mut number).cast())?;
            let entry_out = step_argument_out(access, frame, (&raw mut entry).cast())?;
            let description_out = step_argument_out(access, frame, (&raw mut description).cast())?;
            finish_arguments(access, frame)?;
            let (entry, description) = crate::pawn_native_files_owner::get_next_int_desc(
                access,
                code_units(hp2_core::native_text::fstring_units(&class_name)),
                number,
            )?;
            access.write_vm_fstring_result(entry_out.cast(), tchar_units(&entry))?;
            access.write_vm_fstring_result(description_out.cast(), tchar_units(&description))
        })()
    };
    let release = unsafe { access.release_vm_fstring_argument(&mut class_name) };
    operation.and(release)
}

/// Selects a texture registry entry using the shared Core/native skin policy.
///
/// # Safety
/// The frame and out FString slots must be live canonical VM storage owned by `access`.
unsafe fn get_next_skin(
    access: CoreAccess<'_>,
    _object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut prefix = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut current = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut direction = 0_i32;
    let mut skin = std::ptr::null_mut::<c_void>();
    let mut description = std::ptr::null_mut::<c_void>();
    let operation = unsafe {
        (|| {
            step_argument(access, frame, (&raw mut prefix).cast())?;
            step_argument(access, frame, (&raw mut current).cast())?;
            step_argument(access, frame, (&raw mut direction).cast())?;
            let skin_out = step_argument_out(access, frame, (&raw mut skin).cast())?;
            let description_out = step_argument_out(access, frame, (&raw mut description).cast())?;
            finish_arguments(access, frame)?;
            let (skin, description) = crate::pawn_native_files_owner::get_next_skin(
                access,
                code_units(hp2_core::native_text::fstring_units(&prefix)),
                code_units(hp2_core::native_text::fstring_units(&current)),
                direction,
            )?;
            access.write_vm_fstring_result(skin_out.cast(), tchar_units(&skin))?;
            access.write_vm_fstring_result(description_out.cast(), tchar_units(&description))
        })()
    };
    let release_prefix = unsafe { access.release_vm_fstring_argument(&mut prefix) };
    let release_current = unsafe { access.release_vm_fstring_argument(&mut current) };
    operation.and(release_prefix).and(release_current)
}

unsafe fn particle_count(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        result.cast::<i32>().write_unaligned(crate::particle_fx_owner::num_particles(access, object.cast())?);
    }
    Ok(())
}

unsafe fn particle_add(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    let mut id = 0i32;
    let mut location = crate::vec::zero();
    unsafe {
        step_argument(access, frame, ptr::addr_of_mut!(id).cast())?;
        step_argument(access, frame, ptr::addr_of_mut!(location).cast())?;
        finish_arguments(access, frame)?;
        crate::particle_fx_owner::add_particle(access, object.cast(), id, &location)
    }
}

unsafe fn particle_parameters(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void, write: bool) -> RuntimeResult<()> {
    let mut id = 0i32;
    // All fields are numeric scalars or plain vectors/colors; zero is valid.
    let mut temporary: crate::particle_fx::FParticleParams = unsafe { std::mem::zeroed() };
    unsafe {
        step_argument(access, frame, ptr::addr_of_mut!(id).cast())?;
        let address = step_argument_out(access, frame, ptr::addr_of_mut!(temporary).cast())?;
        let params = if address.is_null() { ptr::addr_of_mut!(temporary) } else { address.cast() };
        finish_arguments(access, frame)?;
        let found = if write {
            crate::particle_fx_owner::set_particle_params(access, object.cast(), id, params)?
        } else {
            crate::particle_fx_owner::get_particle_params(access, object.cast(), id, params)?
        };
        result.cast::<u8>().write_unaligned(u8::from(found));
    }
    Ok(())
}

unsafe fn particle_get_params(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { particle_parameters(access, object, frame, result, false) }
}

unsafe fn particle_set_params(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { particle_parameters(access, object, frame, result, true) }
}

unsafe fn particle_recompute(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut id = 0i32;
    unsafe {
        step_argument(access, frame, ptr::addr_of_mut!(id).cast())?;
        finish_arguments(access, frame)?;
        result.cast::<u8>().write_unaligned(u8::from(crate::particle_fx_owner::recompute_deltas(access, object.cast(), id)?));
    }
    Ok(())
}

unsafe fn pawn_pick_target(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { pawn_pick(access, object, frame, result, false) }
}

unsafe fn pawn_pick_any_target(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { pawn_pick(access, object, frame, result, true) }
}

unsafe fn pawn_pick(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void,
    any: bool,
) -> RuntimeResult<()> {
    if object.is_null() || result.is_null() {
        return Err(RuntimeError::new("Canonical target selection requires a Pawn and result storage"));
    }
    let mut aim = 0_f32;
    let mut distance = 0_f32;
    let mut direction = hp2_abi::FVector { x: 0., y: 0., z: 0. };
    let mut start = direction;
    unsafe {
        let aim = hp2_core::canonical_vm_native::step_argument_out(access, frame, (&raw mut aim).cast())?.cast::<f32>();
        let distance = hp2_core::canonical_vm_native::step_argument_out(access, frame, (&raw mut distance).cast())?.cast::<f32>();
        step_argument(access, frame, (&raw mut direction).cast())?;
        step_argument(access, frame, (&raw mut start).cast())?;
        finish_arguments(access, frame)?;
        let selected = if any {
            crate::pawn_native_ai_owner::pick_any_target(access, object.cast(), aim, distance, direction, start)?
        } else {
            crate::pawn_native_ai_owner::pick_target(access, object.cast(), aim, distance, direction, start)?.cast()
        };
        result.cast::<*mut c_void>().write_unaligned(selected.cast());
    }
    Ok(())
}

unsafe fn pawn_add(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_ai_owner::add_pawn(object.cast())
    }
}
unsafe fn pawn_remove(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_ai_owner::remove_pawn(object.cast())
    }
}

unsafe fn pawn_wait_for_landing(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_ai_owner::wait_for_landing(object.cast())
    }
}
unsafe fn pawn_poll_wait_for_landing(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, delta: *mut c_void) -> RuntimeResult<()> {
    unsafe { crate::pawn_native_ai_owner::poll_wait_for_landing(access, object.cast(), delta.cast()) }
}
unsafe fn pawn_stop_waiting(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_ai_owner::stop_waiting(object.cast())
    }
}

unsafe fn pawn_find_stair_rotation(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut delta = 0_f32;
    unsafe {
        step_argument(access, frame, (&raw mut delta).cast())?;
        finish_arguments(access, frame)?;
        let rotation = crate::pawn_native_ai_owner::find_stair_rotation(access, object.cast(), delta)?;
        result.cast::<i32>().write_unaligned(rotation);
    }
    Ok(())
}

unsafe fn pawn_pick_wall_adjust(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        crate::pawn_native_move::clock_find_path_begin(object.cast())
            .map_err(|_| RuntimeError::new("Canonical PickWallAdjust clock failed"))?;
        let adjusted = crate::actor_tick_owner::pick_wall_adjust(access, object.cast())?;
        result.cast::<u32>().write_unaligned(adjusted as u32);
        crate::pawn_native_move::clock_find_path_end(object.cast())
            .map_err(|_| RuntimeError::new("Canonical PickWallAdjust unclock failed"))
    }
}
unsafe fn pawn_line_of_sight(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { pawn_sight(access, object, frame, result, 0) }
}
unsafe fn pawn_can_see(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { pawn_sight(access, object, frame, result, 1) }
}
unsafe fn pawn_sight(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void, show_self: i32) -> RuntimeResult<()> {
    let mut other = ptr::null_mut::<AActor>();
    unsafe {
        step_argument(access, frame, (&raw mut other).cast())?;
        finish_arguments(access, frame)?;
        let visible = crate::actor_tick_owner::line_of_sight_to(access, object.cast(), other, show_self)?;
        result.cast::<u32>().write_unaligned(u32::from(visible));
    }
    Ok(())
}

unsafe fn pawn_point_reachable(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut point = hp2_abi::FVector { x: 0., y: 0., z: 0. };
    unsafe {
        step_argument(access, frame, (&raw mut point).cast())?;
        finish_arguments(access, frame)?;
        crate::pawn_native_move::clock_find_path_begin(object.cast())
            .map_err(|_| RuntimeError::new("Canonical pointReachable clock failed"))?;
        let reachable = crate::pawn_reach_owner::point_reachable(access, object.cast(), point, 0)?;
        result.cast::<i32>().write_unaligned(reachable);
        crate::pawn_native_move::clock_find_path_end(object.cast())
            .map_err(|_| RuntimeError::new("Canonical pointReachable unclock failed"))
    }
}

unsafe fn pawn_adjust_jump(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let pawn = object.cast::<hp2_abi::engine::generated::APawn>();
        let mut velocity = (*pawn).base.velocity;
        crate::pawn_reach::suggest_jump_velocity(pawn, (*pawn).destination, &raw mut velocity)
            .map_err(|status| RuntimeError::new(&format!("Canonical EAdjustJump failed ({status})")))?;
        // The source computes the jump before P_FINISH, and never writes Pawn.Velocity.
        finish_arguments(access, frame)?;
        result.cast::<hp2_abi::FVector>().write_unaligned(velocity);
    }
    Ok(())
}
