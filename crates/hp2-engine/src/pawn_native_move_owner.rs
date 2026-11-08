//! Canonical providers for native pawn path/movement policy.
use std::{ffi::c_void, ptr};
use hp2_abi::{FVector, engine::generated::{AActor, APawn}};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::{pawn_native_move::{self, Hp2PawnNativeMoveServices}, route_owner::{output, status}};

unsafe extern "C" fn is_a(object: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        let name = match kind { 0 => "Pawn", 1 => "Inventory", 2 => "NavigationPoint", _ => return Err(RuntimeError::new("Invalid pawn move class selector")) };
        crate::actor_physics_owner::is_a_named(access, object, name).map(i32::from)
    })
}
unsafe extern "C" fn is_probing(object: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        let name = match kind { 0 => "SpecialHandling", 1 => "SeePlayer", _ => return Err(RuntimeError::new("Invalid pawn move probe selector")) };
        access.is_probing(object.cast(), access.intern_str(name, FindName::Find)?).map(i32::from)
    })
}
unsafe extern "C" fn alter_destination(pawn: *mut c_void) -> i32 {
    status(|access| unsafe { crate::actor_tick_owner::execute_event_params(access, pawn, "AlterDestination", ptr::null_mut(), 0) })
}
unsafe extern "C" fn see_player(pawn: *mut c_void, seer: *mut c_void) -> i32 {
    status(|access| unsafe {
        let mut seer = seer;
        crate::actor_tick_owner::execute_event_params(access, pawn, "SeePlayer", (&raw mut seer).cast(), size_of::<*mut c_void>())
    })
}
pub(crate) const SERVICES: Hp2PawnNativeMoveServices = Hp2PawnNativeMoveServices {
    is_a, is_probing, event_alter_destination: alter_destination, event_see_player: see_player,
};
fn with_path_services<T>(access: CoreAccess<'_>, body: impl FnOnce(&crate::route::Hp2RouteServices, &crate::pawn_reach::Hp2PawnReachServices, &crate::pawn_sense::Hp2PawnSenseServices) -> Result<T, i32>) -> RuntimeResult<T> {
    crate::route_owner::with_services(access, |route| {
        crate::pawn_reach_owner::with_services(access, |reach| {
            crate::actor_tick_owner::with_sense_services(access, |sense| {
                body(route, reach, sense).map_err(|status| RuntimeError::new(&format!("Canonical pawn path failed ({status})")))
            })
        })
    })
}
/// # Safety
/// Pawn and target belong to a live canonical world; null target is handled by the native.
pub unsafe fn actor_reachable(access: CoreAccess<'_>, pawn: *mut APawn, target: *mut AActor) -> RuntimeResult<u32> {
    with_path_services(access, |route, reach, _| unsafe { pawn_native_move::actor_reachable_with(&SERVICES, route, reach, pawn, target.cast()) })
}
/// # Safety
/// Pawn and its world remain live through synchronous route/event callbacks.
pub unsafe fn find_path_to(access: CoreAccess<'_>, pawn: *mut APawn, point: FVector, single: i32, clear: i32) -> RuntimeResult<*mut c_void> {
    with_path_services(access, |route, _, sense| unsafe { pawn_native_move::find_path_to_with(&SERVICES, route, sense, pawn, (&raw const point).cast(), single, clear) })
}
/// # Safety
/// Pawn and goal belong to the live canonical world.
pub unsafe fn find_path_toward(access: CoreAccess<'_>, pawn: *mut APawn, goal: *mut AActor, single: i32, clear: i32) -> RuntimeResult<*mut c_void> {
    with_path_services(access, |route, _, sense| unsafe { pawn_native_move::find_path_toward_with(&SERVICES, route, sense, pawn, goal.cast(), single, clear) })
}
/// # Safety
/// Pawn and its navigation graph remain live.
pub unsafe fn find_random_dest(access: CoreAccess<'_>, pawn: *mut APawn, clear: i32) -> RuntimeResult<*mut c_void> {
    with_path_services(access, |route, _, _| unsafe { pawn_native_move::find_random_dest_with(route, pawn, clear) })
}
/// # Safety
/// Pawn, start and writable result remain live through script callbacks.
pub unsafe fn find_path(access: CoreAccess<'_>, pawn: *mut APawn, start: *mut AActor, class: i32) -> RuntimeResult<*mut c_void> {
    crate::route_owner::with_services(access, |services| unsafe {
        let mut next = ptr::null_mut();
        crate::route::find_path(services, pawn, &raw mut next, start, class).map_err(|_| RuntimeError::new("Canonical FindPath failed"))?;
        Ok(next.cast())
    })
}

/// # Safety
/// Pawn/world and the caller's live Weight reference remain valid through script reentry.
pub unsafe fn find_best_inventory_path(access: CoreAccess<'_>, pawn: *mut APawn, weight: f32, predict: bool, weight_out: *mut f32) -> RuntimeResult<*mut c_void> {
    crate::route_owner::with_services(access, |route| {
        crate::actor_tick_owner::with_sense_services(access, |sense| unsafe {
            crate::pawn_native_files::find_best_inventory_path_with(
                pawn.cast(), weight, predict, weight_out,
                |weight, predict, path| crate::route::find_path_toward_best_inventory(route, pawn, path.cast(), 1, weight, i32::from(predict))
                    .map_err(|_| RuntimeError::new("Canonical inventory route search failed")),
                |path| access.is_probing(path.cast(), access.intern_str("SpecialHandling", FindName::Find)?),
                |path| crate::pawn_sense::handle_special(sense, pawn, path.cast())
                    .map_err(|_| RuntimeError::new("Canonical inventory SpecialHandling failed")),
            )
        })
    })
}

fn with_move_services(
    access: CoreAccess<'_>,
    body: impl FnOnce(&crate::pawn_sense::Hp2PawnSenseServices) -> Result<(), i32>,
) -> RuntimeResult<()> {
    crate::route_owner::with_services(access, |_| {
        crate::actor_tick_owner::with_sense_services(access, |sense| {
            body(sense).map_err(|status| RuntimeError::new(&format!("Canonical pawn movement failed ({status})")))
        })
    })
}
/// # Safety
/// Pawn/world/state-frame remain live through steering and script callbacks.
pub unsafe fn move_to(access: CoreAccess<'_>, pawn: *mut APawn, dest: FVector, speed: f32, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::move_to_with(sense, pawn, dest, speed, latent) })
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn poll_destination(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::poll_destination_with(sense, pawn) })
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn move_toward(access: CoreAccess<'_>, pawn: *mut APawn, goal: *mut AActor, speed: f32, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::move_toward_with(&SERVICES, sense, pawn, goal.cast(), speed, latent) })
}
/// # Safety
/// Pawn/target/world/state-frame remain live through script reentry.
pub unsafe fn poll_move_toward(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::poll_move_toward_with(&SERVICES, sense, pawn) })
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn strafe_to(access: CoreAccess<'_>, pawn: *mut APawn, dest: FVector, focus: FVector, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::strafe_to_with(sense, pawn, dest, focus, latent) })
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn strafe_facing(access: CoreAccess<'_>, pawn: *mut APawn, dest: FVector, goal: *mut AActor, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::strafe_facing_with(sense, pawn, dest, goal.cast(), latent) })
}
/// # Safety
/// Pawn/target/world/state-frame remain live through script reentry.
pub unsafe fn poll_strafe_facing(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::poll_strafe_facing_with(&SERVICES, sense, pawn) })
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn turn_toward(access: CoreAccess<'_>, pawn: *mut APawn, goal: *mut AActor, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::turn_toward_with(sense, pawn, goal.cast(), latent) })
}
/// # Safety
/// Pawn/target/world/state-frame remain live.
pub unsafe fn poll_turn_toward(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::poll_turn_toward_with(sense, pawn) })
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn turn_to(access: CoreAccess<'_>, pawn: *mut APawn, focus: FVector, latent: i32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::turn_to_with(sense, pawn, focus, latent) })
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn poll_turn_to(access: CoreAccess<'_>, pawn: *mut APawn) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::poll_turn_to_with(sense, pawn) })
}
/// # Safety
/// Actor and its instigator/listeners belong to the live canonical world.
pub unsafe fn make_noise(access: CoreAccess<'_>, actor: *mut AActor, loudness: f32) -> RuntimeResult<()> {
    with_move_services(access, |sense| unsafe { pawn_native_move::make_noise_with(sense, actor.cast(), loudness) })
}
