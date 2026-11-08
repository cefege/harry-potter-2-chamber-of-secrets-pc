//! Canonical world/sight queries for the shared native pawn AI policy.

use std::{ffi::c_void, ptr};
use hp2_abi::{FVector, engine::generated::{AActor, AGameInfo, APawn, APlayerReplicationInfo, ULevel}};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::pawn_native_ai::{self, TargetOps};

struct Targets<'a>(CoreAccess<'a>);
impl TargetOps for Targets<'_> {
    type Error = RuntimeError;
    unsafe fn pawn_list(&self, level: *mut c_void) -> RuntimeResult<*mut APawn> {
        unsafe { read_pawn_list(level).map(|head| head.cast()) }
    }
    unsafe fn team_game(&self, level: *mut c_void) -> RuntimeResult<bool> {
        let info = unsafe { crate::package_map_level_owner::level_info(level.cast::<ULevel>())? };
        let game = unsafe { ptr::addr_of!((*info).game).read_unaligned().cast::<AGameInfo>() };
        if game.is_null() { return Err(RuntimeError::new("Target selection LevelInfo has no GameInfo")); }
        Ok(unsafe { (*game).b_team_game() })
    }
    unsafe fn team(&self, info: *mut c_void) -> RuntimeResult<i32> {
        if info.is_null() { return Err(RuntimeError::new("Null target replication info")); }
        Ok(i32::from(unsafe { (*info.cast::<APlayerReplicationInfo>()).team }))
    }
    unsafe fn health(&self, pawn: *mut APawn) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*pawn).health).read_unaligned() })
    }
    unsafe fn projectile_target(&self, actor: *mut AActor) -> RuntimeResult<bool> {
        Ok(unsafe { (*actor).b_proj_target() })
    }
    unsafe fn is_pawn(&self, actor: *mut AActor) -> RuntimeResult<bool> {
        let class = self.0.native_class("Engine.Pawn")?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn line_of_sight(&self, pawn: *mut APawn, target: *mut AActor) -> RuntimeResult<bool> {
        unsafe { crate::actor_tick_owner::line_of_sight_to(self.0, pawn, target, 0) }
    }
    unsafe fn actor_count(&self, level: *mut c_void) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level.cast()) }
    }
    unsafe fn actor_at(&self, level: *mut c_void, index: i32) -> RuntimeResult<*mut AActor> {
        let level = level.cast::<ULevel>();
        let count = unsafe { crate::level_owner::actor_count(level)? };
        if index < 0 || index >= count { return Err(RuntimeError::new("Target selection actor index is outside Level")); }
        Ok(unsafe { crate::level_owner::actor_at(level, index) })
    }
}

/// Runs source PickTarget over the canonical live pawn list, team metadata and sight queries.
///
/// # Safety
/// Pawn, its world and reached actors remain live; out pointers hold writable floats.
pub unsafe fn pick_target(
    access: CoreAccess<'_>, pawn: *mut APawn, best_aim: *mut f32, best_dist: *mut f32,
    fire_dir: FVector, start: FVector,
) -> RuntimeResult<*mut APawn> {
    unsafe { pawn_native_ai::pick_target_with(&Targets(access), pawn, best_aim, best_dist, fire_dir, start) }
}

/// Runs source PickAnyTarget over the canonical live actor array and real sight queries.
///
/// # Safety
/// Same live world/output contract as `pick_target`.
pub unsafe fn pick_any_target(
    access: CoreAccess<'_>, pawn: *mut APawn, best_aim: *mut f32, best_dist: *mut f32,
    fire_dir: FVector, start: FVector,
) -> RuntimeResult<*mut AActor> {
    unsafe { pawn_native_ai::pick_any_target_with(&Targets(access), pawn, best_aim, best_dist, fire_dir, start) }
}

unsafe fn read_pawn_list(level: *mut c_void) -> RuntimeResult<*mut c_void> {
    let info = unsafe { crate::package_map_level_owner::level_info(level.cast())? };
    Ok(unsafe { ptr::addr_of!((*info).pawn_list).read_unaligned() })
}
unsafe fn write_pawn_list(level: *mut c_void, head: *mut c_void) -> RuntimeResult<()> {
    let info = unsafe { crate::package_map_level_owner::level_info(level.cast())? };
    unsafe { ptr::addr_of_mut!((*info).pawn_list).write_unaligned(head); }
    Ok(())
}

/// Publishes a live canonical Pawn in the actual LevelInfo pawn list.
///
/// # Safety
/// Pawn and its level/list nodes remain live through this mutation.
pub unsafe fn add_pawn(pawn: *mut APawn) -> RuntimeResult<()> {
    unsafe { pawn_native_ai::add_pawn_with(pawn, |level| read_pawn_list(level), |level, head| write_pawn_list(level, head)) }
}

/// Unlinks a canonical Pawn with source head/interior and retained NextPawn behavior.
///
/// # Safety
/// Same live list contract as `add_pawn`.
pub unsafe fn remove_pawn(pawn: *mut APawn) -> RuntimeResult<()> {
    unsafe { pawn_native_ai::remove_pawn_with(pawn, |level| read_pawn_list(level), |level, head| write_pawn_list(level, head)) }
}

unsafe fn frame(actor: *mut c_void) -> RuntimeResult<*mut hp2_abi::engine::generated::FStateFrame> {
    let frame: *mut hp2_abi::engine::generated::FStateFrame = unsafe {
        ptr::addr_of!((*actor.cast::<hp2_abi::engine::generated::UObject>()).state_frame)
            .read_unaligned().cast()
    };
    if frame.is_null() { return Err(RuntimeError::new("Pawn latent operation requires InitExecution")); }
    Ok(frame)
}
unsafe fn set_latent(actor: *mut c_void, action: i32) -> RuntimeResult<()> {
    let frame = unsafe { frame(actor)? };
    unsafe { ptr::addr_of_mut!((*frame).latent_action).write_unaligned(action); }
    Ok(())
}

/// Cancels only the source Sleep timer, without changing another latent action.
///
/// # Safety
/// Pawn and its initialized execution frame are live.
pub unsafe fn stop_waiting(pawn: *mut APawn) -> RuntimeResult<()> {
    unsafe { pawn_native_ai::stop_waiting_with(pawn, || {
        let frame = frame(pawn.cast())?;
        Ok(ptr::addr_of!((*frame).latent_action).read_unaligned())
    }) }
}

/// Arms the native landing timer and, only while falling, its real state-frame poll.
///
/// # Safety
/// Pawn is live and has an execution frame when falling.
pub unsafe fn wait_for_landing(pawn: *mut APawn) -> RuntimeResult<()> {
    unsafe { pawn_native_ai::wait_for_landing_with(pawn, |actor, action| set_latent(actor, action)) }
}

/// Polls landing through the actual latent frame and reflected LongFall event.
///
/// # Safety
/// Pawn/world/frame are live; delta is readable on the Falling branch.
pub unsafe fn poll_wait_for_landing(
    access: CoreAccess<'_>, pawn: *mut APawn, delta: *const f32,
) -> RuntimeResult<()> {
    unsafe { pawn_native_ai::poll_wait_for_landing_with(
        pawn, delta, |actor, action| set_latent(actor, action), |pawn| {
            let name = access.intern_str("LongFall", hp2_core::name_owner::FindName::Find)?;
            let function = access.find_object_function_by_name(pawn.cast(), name, false)?;
            if function.is_null() { return Err(RuntimeError::new("Pawn LongFall event metadata is unavailable")); }
            access.execute_event(pawn.cast(), function, ptr::null_mut(), ptr::null_mut())
        },
    ) }
}

/// Computes source stair pitch with native trig and genuine canonical visibility traces.
///
/// # Safety
/// Pawn and its world/model remain live on trace branches.
pub unsafe fn find_stair_rotation(
    access: CoreAccess<'_>, pawn: *mut APawn, delta_time: f32,
) -> RuntimeResult<i32> {
    let mut hit: hp2_abi::engine::generated::FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.;
    hit.item = -1;
    unsafe { pawn_native_ai::find_stair_rotation_with(
        pawn, delta_time, &raw mut hit,
        |rotation| Ok(hp2_core::math::rotator_vector(
            hp2_core::math::FRotator { pitch: rotation.pitch, yaw: rotation.yaw, roll: rotation.roll },
            hp2_core::math::trig_table(),
        )),
        |pawn, hit, end, start, extent| {
            let level = ptr::addr_of!((*pawn).base.x_level).read_unaligned().cast();
            crate::level_trace_owner::single_line_check(
                access, level, hit, pawn.cast(), end, start, 0x06, extent, 0,
            ).map(|_| ())
        },
    ) }
}

mod path_services {
    use super::*;
    use hp2_abi::engine::generated::{FReachSpec, FRotator, UClient, UEngine, UViewport};
    use crate::{pawn_native_ai::Hp2PawnNativeAiServices, route_owner::{output, status}};
    unsafe extern "C" fn rotation(raw: *const c_void, out: *mut c_void) -> i32 {
        output(out.cast::<FVector>(), |_| {
            let r = unsafe { raw.cast::<FRotator>().read_unaligned() };
            Ok(hp2_core::math::rotator_vector(hp2_core::math::FRotator { pitch: r.pitch, yaw: r.yaw, roll: r.roll }, hp2_core::math::trig_table()))
        })
    }
    unsafe extern "C" fn latent(actor: *mut c_void, out: *mut i32) -> i32 { output(out, |_| unsafe { frame(actor).map(|frame| (*frame).latent_action) }) }
    unsafe extern "C" fn set_action(actor: *mut c_void, action: i32) -> i32 { status(|_| unsafe { set_latent(actor, action) }) }
    unsafe extern "C" fn net_mode(level: *mut c_void, out: *mut i32) -> i32 {
        output(out, |_| unsafe { crate::package_map_level_owner::level_info(level.cast()).map(|info| i32::from((*info).net_mode)) })
    }
    unsafe extern "C" fn pawn_list(level: *mut c_void, out: *mut *mut c_void) -> i32 { output(out, |_| unsafe { read_pawn_list(level) }) }
    unsafe extern "C" fn set_pawn_list(level: *mut c_void, head: *mut c_void) -> i32 { status(|_| unsafe { write_pawn_list(level, head) }) }
    unsafe extern "C" fn team_game(level: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).team_game(level).map(i32::from) }) }
    unsafe extern "C" fn actor_count(level: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).actor_count(level) }) }
    unsafe extern "C" fn actor_at(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 { output(out, |access| unsafe { Targets(access).actor_at(level, index).map(|actor| actor.cast()) }) }
    unsafe fn viewports(level: *mut c_void) -> RuntimeResult<hp2_abi::engine::generated::TArrayRaw> {
        let engine = unsafe { (*level.cast::<ULevel>()).base.engine.cast::<UEngine>() };
        if engine.is_null() { return Err(RuntimeError::new("Visibility Level has no Engine")); }
        let client = unsafe { (*engine).client.cast::<UClient>() };
        if client.is_null() { return Ok(hp2_abi::engine::generated::TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
        let array = unsafe { (*client).viewports };
        if array.num < 0 || (array.num > 0 && array.data.is_null()) { return Err(RuntimeError::new("Invalid visibility viewport array")); }
        Ok(array)
    }
    unsafe extern "C" fn viewport_count(level: *mut c_void, out: *mut i32) -> i32 { output(out, |_| unsafe { viewports(level).map(|array| array.num) }) }
    unsafe extern "C" fn viewport_actor(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 {
        output(out, |_| unsafe {
            let array = viewports(level)?;
            if index < 0 || index >= array.num { return Err(RuntimeError::new("Visibility viewport index outside Client")); }
            let viewport = array.data.cast::<*mut UViewport>().add(index as usize).read_unaligned();
            Ok(if viewport.is_null() { ptr::null_mut() } else { (*viewport).actor })
        })
    }
    unsafe extern "C" fn net_temporary(actor: *mut c_void, out: *mut i32) -> i32 { output(out, |_| Ok(i32::from(unsafe { (*actor.cast::<AActor>()).b_net_temporary() }))) }
    unsafe extern "C" fn proj_target(actor: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).projectile_target(actor.cast()).map(i32::from) }) }
    unsafe extern "C" fn is_pawn(actor: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).is_pawn(actor.cast()).map(i32::from) }) }
    unsafe extern "C" fn health(pawn: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).health(pawn.cast()) }) }
    unsafe extern "C" fn behind_view(pawn: *mut c_void, out: *mut i32) -> i32 { output(out, |_| Ok(i32::from(unsafe { (*pawn.cast::<APawn>()).b_behind_view() }))) }
    unsafe extern "C" fn team(info: *mut c_void, out: *mut i32) -> i32 { output(out, |access| unsafe { Targets(access).team(info) }) }
    unsafe extern "C" fn sight(pawn: *mut c_void, other: *mut c_void, show: i32, out: *mut i32) -> i32 {
        output(out, |access| unsafe { crate::actor_tick_owner::line_of_sight_to(access, pawn.cast(), other.cast(), show).map(i32::from) })
    }
    unsafe extern "C" fn trace(level: *mut c_void, source: *mut c_void, hit: *mut c_void, end: *const c_void, start: *const c_void, flags: u32, extent: *const c_void) -> i32 {
        status(|access| unsafe { crate::level_trace_owner::single_line_check(access, level.cast(), hit.cast(), source.cast(), end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned(), flags, extent.cast::<FVector>().read_unaligned(), 0).map(|_| ()) })
    }
    unsafe extern "C" fn long_fall(pawn: *mut c_void) -> i32 { status(|access| unsafe { crate::actor_tick_owner::execute_event_params(access, pawn, "LongFall", ptr::null_mut(), 0) }) }
    unsafe extern "C" fn test_visibility(actor: *mut c_void, viewer: *mut c_void, out: *mut i32) -> i32 {
        output(out, |_| unsafe { pawn_native_ai::test_can_see_me(&SERVICES, actor.cast(), viewer.cast()).map(i32::from).map_err(|_| RuntimeError::new("Canonical TestCanSeeMe failed")) })
    }
    unsafe extern "C" fn spec_count(level: *mut c_void, out: *mut i32) -> i32 {
        output(out, |_| {
            let count = unsafe { (*level.cast::<ULevel>()).reach_specs.num };
            if count < 0 { return Err(RuntimeError::new("Negative ReachSpecs length")); }
            Ok(count)
        })
    }
    unsafe extern "C" fn spec_at(level: *mut c_void, index: i32, start: *mut *mut c_void, end: *mut *mut c_void, flags: *mut i32, distance: *mut i32) -> i32 {
        status(|_| unsafe {
            let array = (*level.cast::<ULevel>()).reach_specs;
            if index < 0 || index >= array.num || array.data.is_null() { return Err(RuntimeError::new("ReachSpec index outside Level")); }
            let spec = array.data.cast::<FReachSpec>().add(index as usize).read_unaligned();
            start.write_unaligned(spec.start); end.write_unaligned(spec.end);
            flags.write_unaligned(spec.reach_flags); distance.write_unaligned(spec.distance);
            Ok(())
        })
    }
    static SERVICES: Hp2PawnNativeAiServices = Hp2PawnNativeAiServices {
        rotation_vector: rotation, state_latent_action: latent, set_state_latent_action: set_action,
        level_net_mode: net_mode, level_pawn_list: pawn_list, set_level_pawn_list: set_pawn_list,
        level_team_game: team_game, level_actor_count: actor_count, level_actor: actor_at,
        viewport_count, viewport_actor, actor_net_temporary: net_temporary, actor_proj_target: proj_target,
        actor_is_pawn: is_pawn, pawn_health: health, pawn_behind_view: behind_view, replication_team: team,
        line_of_sight_to: sight, single_line_check: trace, event_long_fall: long_fall,
        test_can_see_me: test_visibility, reach_spec_count: spec_count, reach_spec_at: spec_at,
    };
    pub(super) unsafe fn visibility(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<bool> {
        crate::route_owner::with_services(access, |_| unsafe {
            pawn_native_ai::player_can_see_me(&SERVICES, actor).map_err(|_| RuntimeError::new("Canonical PlayerCanSeeMe failed"))
        })
    }
    pub(super) unsafe fn describe(access: CoreAccess<'_>, level: *mut c_void, index: i32) -> RuntimeResult<(*mut c_void, *mut c_void, i32, i32)> {
        crate::route_owner::with_services(access, |_| unsafe {
            pawn_native_ai::describe_spec(&SERVICES, level, index).map_err(|_| RuntimeError::new("Canonical describeSpec failed"))
        })
    }
}

/// # Safety
/// Actor and its level/player list remain live through visibility queries.
pub unsafe fn player_can_see_me(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<bool> {
    unsafe { path_services::visibility(access, actor) }
}
/// # Safety
/// Level and its reachspec array remain live.
pub unsafe fn describe_spec(access: CoreAccess<'_>, level: *mut c_void, index: i32) -> RuntimeResult<(*mut c_void, *mut c_void, i32, i32)> {
    unsafe { path_services::describe(access, level, index) }
}
