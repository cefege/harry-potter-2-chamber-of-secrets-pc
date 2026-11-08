//! Canonical level collision placement and Actor zone transitions.

use crate::{level_owner, level_trace_owner, levact};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, ALevelInfo, APawn, APlayerReplicationInfo, FCheckResult, FPointRegion, ULevel, UModel, UObject};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, ptr};

/// Finds a collision-free placement using the shared native FindSpot policy.
///
/// # Safety
/// `level` and `location` must remain live through collision tracing.
pub unsafe fn find_spot(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    extent: FVector,
    location: *mut FVector,
    check_actors: bool,
    assume_fit: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || location.is_null() {
        return Err(RuntimeError::new("FindSpot received a null Level or location"));
    }
    let level_info = unsafe { first_actor(level)? };
    let mut hit = unsafe { blank_hit() };
    // SAFETY: the caller owns the live Level and writable location for the operation.
    unsafe {
        levact::find_spot_policy(
            extent,
            location,
            assume_fit,
            &mut hit,
            |at, result| unsafe {
                level_trace_owner::single_point_check(
                    access, level, result, at, extent, 0, level_info.cast(), check_actors,
                )
            },
            |adjusted, destination, trace_len, result| unsafe {
                let end = destination;
                let start = *adjusted;
                let empty_extent = FVector { x: 0.0, y: 0.0, z: 0.0 };
                let _ = level_trace_owner::single_line_check(
                    access, level, result, ptr::null_mut(), end, start,
                    crate::levact::TRACE_LEVEL | crate::levact::TRACE_MOVERS, empty_extent, 0,
                )?;
                if (*result).time < 1.0 {
                    *adjusted = crate::vec::add(
                        *adjusted,
                        crate::vec::scale(
                            crate::vec::scale((*result).normal, 1.05 - (*result).time),
                            trace_len,
                        ),
                    );
                }
                Ok(())
            },
        )
    }
}

/// Applies ULevel::SetActorZone with Core-owned model geometry and VM events.
///
/// # Safety
/// `level` and `actor` are live canonical objects and remain live through VM reentry.
pub(crate) unsafe fn set_actor_zone(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    actor: *mut AActor,
    test: bool,
    force_refresh: bool,
) -> RuntimeResult<()> {
    if level.is_null() || actor.is_null() {
        return Err(RuntimeError::new("SetActorZone received a null Level or Actor"));
    }
    if unsafe { (*actor).b_delete_me() } {
        return Ok(());
    }
    let actor_count = unsafe { level_owner::actor_count(level)? };
    let info = if actor_count == 0 {
        ptr::null_mut()
    } else {
        unsafe { level_owner::actor_at(level, 0).cast() }
    };
    if actor.cast::<c_void>() == info {
        let fallback = fallback_region(info);
        unsafe { ptr::addr_of_mut!((*actor).region).write_unaligned(fallback); }
        return Ok(());
    }
    let pawn_class = access.native_class("Engine.Pawn")?;
    let is_pawn = unsafe { hp2_core::object_model::is_a(actor.cast(), pawn_class.cast()) };
    let pawn = if is_pawn { actor.cast::<APawn>() } else { ptr::null_mut() };
    let fallback = fallback_region(info);
    if force_refresh {
        unsafe {
            ptr::addr_of_mut!((*actor).region).write_unaligned(fallback);
            if !pawn.is_null() {
                ptr::addr_of_mut!((*pawn).foot_region).write_unaligned(fallback);
                ptr::addr_of_mut!((*pawn).head_region).write_unaligned(fallback);
            }
        }
    }
    let zone = if actor_count != 0 { info } else { actor.cast() };
    let model = unsafe { (*level).model.cast::<UModel>() };
    if model.is_null() { return Err(RuntimeError::new("SetActorZone Level has no model")); }
    let location = unsafe { ptr::addr_of!((*actor).location).read_unaligned() };
    let new_region = unsafe {
        crate::collision_owner::CollisionOwner::model_point_region(model, zone, location)?
    };
    let old_region = unsafe { ptr::addr_of!((*actor).region).read_unaligned() };
    unsafe {
        levact::zone_transition_policy(
            actor.cast(),
            old_region,
            new_region,
            test,
            |zone, actor| unsafe { dispatch_actor_event(access, zone.cast(), "ActorLeaving", actor.cast()) },
            |actor, zone| unsafe { dispatch_zone_event(access, actor.cast(), "ZoneChange", zone) },
            |region| unsafe { ptr::addr_of_mut!((*actor).region).write_unaligned(region) },
            |zone, actor| unsafe { dispatch_actor_event(access, zone.cast(), "ActorEntered", actor.cast()) },
        )?;
    }
    if !pawn.is_null() {
        let height = unsafe { ptr::addr_of!((*actor).collision_height).read_unaligned() };
        let foot = FVector { x: location.x, y: location.y, z: location.z - height };
        let new_foot = unsafe { crate::collision_owner::CollisionOwner::model_point_region(model, info, foot)? };
        let old_foot = unsafe { ptr::addr_of!((*pawn).foot_region).read_unaligned() };
        levact::update_region_policy(
            old_foot,
            new_foot,
            test,
            |zone| unsafe { dispatch_zone_event(access, pawn.cast(), "FootZoneChange", zone) },
            |region| unsafe { ptr::addr_of_mut!((*pawn).foot_region).write_unaligned(region) },
        )?;
        let eye_height = unsafe { ptr::addr_of!((*pawn).eye_height).read_unaligned() };
        let head = FVector { x: location.x, y: location.y, z: location.z + eye_height };
        let new_head = unsafe { crate::collision_owner::CollisionOwner::model_point_region(model, info, head)? };
        let old_head = unsafe { ptr::addr_of!((*pawn).head_region).read_unaligned() };
        levact::update_region_policy(
            old_head,
            new_head,
            test,
            |zone| unsafe { dispatch_zone_event(access, pawn.cast(), "HeadZoneChange", zone) },
            |region| unsafe { ptr::addr_of_mut!((*pawn).head_region).write_unaligned(region) },
        )?;
        let pri = unsafe { ptr::addr_of!((*pawn).player_replication_info).read_unaligned() };
        if !pri.is_null() && !info.is_null() {
            let net_mode = unsafe { ptr::addr_of!((*info.cast::<ALevelInfo>()).net_mode).read_unaligned() };
            if net_mode != 3 {
                unsafe {
                    ptr::addr_of_mut!((*pri.cast::<APlayerReplicationInfo>()).player_zone)
                        .write_unaligned(new_region.zone);
                }
            }
        }
    }
    Ok(())
}

fn blank_hit() -> FCheckResult {
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.0;
    hit
}

fn fallback_region(zone: *mut c_void) -> FPointRegion {
    let mut region: FPointRegion = unsafe { std::mem::zeroed() };
    region.zone = zone;
    region.i_leaf = -1;
    region
}

unsafe fn first_actor(level: *mut ULevel) -> RuntimeResult<*mut c_void> {
    let count = unsafe { level_owner::actor_count(level)? };
    if count == 0 {
        Ok(ptr::null_mut())
    } else {
        Ok(unsafe { level_owner::actor_at(level, 0).cast() })
    }
}

unsafe fn dispatch_actor_event(
    access: CoreAccess<'_>, receiver: *mut UObject, name: &str, mut actor: *mut AActor,
) -> RuntimeResult<()> {
    unsafe { dispatch_event(access, receiver, name, ptr::from_mut(&mut actor).cast()) }
}

unsafe fn dispatch_zone_event(
    access: CoreAccess<'_>, receiver: *mut UObject, name: &str, mut zone: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { dispatch_event(access, receiver, name, ptr::from_mut(&mut zone).cast()) }
}

unsafe fn dispatch_event(
    access: CoreAccess<'_>, receiver: *mut UObject, name: &str, parameters: *mut c_void,
) -> RuntimeResult<()> {
    if receiver.is_null() { return Ok(()); }
    let name = access.intern_str(name, FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(receiver, name, false)? };
    if !function.is_null() {
        unsafe { crate::actor_owner::process_event(access, receiver.cast(), function, parameters, ptr::null_mut())?; }
    }
    Ok(())
}
