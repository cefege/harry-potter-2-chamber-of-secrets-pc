//! Candidate filtering for AActor script iterators. FFrame owns bytecode stepping.
use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::AActor;

use crate::actor;
use hp2_abi::engine::generated::FCheckResult;

const TOUCHING_SLOT_COUNT: i32 = 4;

#[repr(C)]
pub struct Hp2ActorIteratorServices {
    pub context: *mut c_void,
    pub actor_count:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    pub actor_at: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        index: i32,
        out: *mut *mut c_void,
    ) -> i32,
    pub default_actor_class:
        unsafe extern "C" fn(context: *mut c_void, out: *mut *mut c_void) -> i32,
    pub actor_is_a: unsafe extern "C" fn(
        context: *mut c_void,
        candidate: *mut c_void,
        base_class: *mut c_void,
        out: *mut i32,
    ) -> i32,
    pub actor_is_hidden:
        unsafe extern "C" fn(context: *mut c_void, candidate: *mut c_void, out: *mut i32) -> i32,
    pub actor_fast_line_check: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        candidate: *mut c_void,
        trace_location: *const c_void,
        out: *mut i32,
    ) -> i32,
    pub tag_is_none:
        unsafe extern "C" fn(context: *mut c_void, tag: *const c_void, out: *mut i32) -> i32,
    pub actor_tag_matches: unsafe extern "C" fn(
        context: *mut c_void,
        candidate: *mut c_void,
        tag: *const c_void,
        out: *mut i32,
    ) -> i32,
    pub actor_is_in_zone: unsafe extern "C" fn(
        context: *mut c_void,
        candidate: *mut c_void,
        zone: *mut c_void,
        out: *mut i32,
    ) -> i32,
}

unsafe fn resolve_class(
    base_class: *mut *mut c_void,
    services: &Hp2ActorIteratorServices,
) -> Result<*mut c_void, i32> {
    if base_class.is_null() {
        return Err(1);
    }
    let mut class = unsafe { *base_class };
    if class.is_null() {
        let status = unsafe { (services.default_actor_class)(services.context, &mut class) };
        if status != 0 {
            return Err(status);
        }
        unsafe { *base_class = class };
    }
    Ok(class)
}

unsafe fn next_matching(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    tag: Option<*const c_void>,
    require_owner: bool,
    require_based: bool,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if cursor.is_null() || out_actor.is_null() || out_index.is_null() {
        return Err(1);
    }
    unsafe {
        *out_actor = std::ptr::null_mut();
        *out_index = -1;
    }
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let tag = if let Some(tag) = tag {
        let mut is_none = 0;
        let status = unsafe { (services.tag_is_none)(services.context, tag, &mut is_none) };
        if status != 0 {
            return Err(status);
        }
        if is_none == 0 { Some(tag) } else { None }
    } else {
        None
    };

    let mut index = unsafe { *cursor };
    loop {
        let mut count = 0;
        let status = unsafe { (services.actor_count)(services.context, actor_ptr, &mut count) };
        if status != 0 {
            return Err(status);
        }
        if index >= count {
            break;
        }
        let candidate_index = index;
        let mut candidate = std::ptr::null_mut();
        let status = unsafe {
            (services.actor_at)(services.context, actor_ptr, candidate_index, &mut candidate)
        };
        if status != 0 {
            return Err(status);
        }
        index = index.wrapping_add(1);
        unsafe { *cursor = index };
        if candidate.is_null() {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class == 0 {
            continue;
        }
        if let Some(tag) = tag {
            let mut matches_tag = 0;
            let status = unsafe {
                (services.actor_tag_matches)(services.context, candidate, tag, &mut matches_tag)
            };
            if status != 0 {
                return Err(status);
            }
            if matches_tag == 0 {
                continue;
            }
        }
        if require_owner && unsafe { actor::is_owned_by(candidate.cast(), actor_ptr.cast()) } == 0 {
            continue;
        }
        if require_based && unsafe { (*candidate.cast::<AActor>()).base != actor_ptr } {
            continue;
        }
        unsafe {
            *out_actor = candidate;
            *out_index = candidate_index;
        }
        return Ok(());
    }
    Ok(())
}

/// Selects the next `AllActors` result and updates its actor-array cursor.
///
/// # Safety
/// Engine actors, cursor/result storage and service callbacks must remain valid.
pub unsafe fn all_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    tag: *const c_void,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    unsafe {
        next_matching(
            actor_ptr,
            base_class,
            Some(tag),
            false,
            false,
            cursor,
            out_actor,
            out_index,
            services,
        )
    }
}

/// Selects the next `ChildActors` result and updates its actor-array cursor.
///
/// # Safety
/// Engine actors, cursor/result storage and service callbacks must remain valid.
pub unsafe fn child_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    unsafe {
        next_matching(
            actor_ptr, base_class, None, true, false, cursor, out_actor, out_index, services,
        )
    }
}

/// Selects the next `BasedActors` result and updates its actor-array cursor.
///
/// # Safety
/// Engine actors, cursor/result storage and service callbacks must remain valid.
pub unsafe fn based_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    unsafe {
        next_matching(
            actor_ptr, base_class, None, false, true, cursor, out_actor, out_index, services,
        )
    }
}

/// Selects the next non-null `Touching` slot matching `BaseClass`.
///
/// # Safety
/// Engine actor, cursor/result storage and service callbacks must remain valid.
pub unsafe fn touching_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if actor_ptr.is_null() || cursor.is_null() || out_actor.is_null() {
        return Err(1);
    }
    unsafe { *out_actor = std::ptr::null_mut() };
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let actor = actor_ptr.cast::<AActor>();
    while unsafe { *cursor } < TOUCHING_SLOT_COUNT {
        let index = unsafe { *cursor } as usize;
        unsafe { *cursor = *cursor + 1 };
        let candidate = unsafe {
            std::ptr::addr_of!((*actor).touching)
                .cast::<*mut c_void>()
                .add(index)
                .read_unaligned()
        };
        if candidate.is_null() {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class != 0 {
            unsafe { *out_actor = candidate };
            break;
        }
    }
    Ok(())
}

unsafe fn actor_location(actor: *mut c_void) -> FVector {
    unsafe { std::ptr::addr_of!((*actor.cast::<AActor>()).location).read_unaligned() }
}

fn distance_squared(a: FVector, b: FVector) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

unsafe fn inside_actor_radius(
    candidate: *mut c_void,
    center: FVector,
    radius: f32,
    include_collision_radius: bool,
) -> bool {
    let location = unsafe { actor_location(candidate) };
    let collision_radius = if include_collision_radius {
        unsafe {
            std::ptr::addr_of!((*candidate.cast::<AActor>()).collision_radius).read_unaligned()
        }
    } else {
        0.0
    };
    let expanded_radius = radius + collision_radius;
    distance_squared(location, center) < expanded_radius * expanded_radius
}

/// Selects the next actor within the radius expanded by its collision radius.
///
/// # Safety
/// Engine actors, cursor/results, trace location and service callbacks must remain valid.
pub unsafe fn radius_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    radius: f32,
    trace_location: *const FVector,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if actor_ptr.is_null()
        || trace_location.is_null()
        || cursor.is_null()
        || out_actor.is_null()
        || out_index.is_null()
    {
        return Err(1);
    }
    unsafe {
        *out_actor = std::ptr::null_mut();
        *out_index = -1;
    }
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let center = unsafe { trace_location.read_unaligned() };
    let mut index = unsafe { *cursor };
    loop {
        let mut count = 0;
        let status = unsafe { (services.actor_count)(services.context, actor_ptr, &mut count) };
        if status != 0 {
            return Err(status);
        }
        if index >= count {
            break;
        }
        let candidate_index = index;
        let mut candidate = std::ptr::null_mut();
        let status = unsafe {
            (services.actor_at)(services.context, actor_ptr, candidate_index, &mut candidate)
        };
        if status != 0 {
            return Err(status);
        }
        index = index.wrapping_add(1);
        unsafe { *cursor = index };
        if candidate.is_null() {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class == 0 || !unsafe { inside_actor_radius(candidate, center, radius, true) } {
            continue;
        }
        unsafe {
            *out_actor = candidate;
            *out_index = candidate_index;
        }
        return Ok(());
    }
    Ok(())
}

/// Selects the next visible, non-hidden actor, with zero radius disabling the range test.
///
/// # Safety
/// Engine actors, cursor/results, trace location and service callbacks must remain valid.
pub unsafe fn visible_actors_next(
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    radius: f32,
    trace_location: *const FVector,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    out_index: *mut i32,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if actor_ptr.is_null()
        || trace_location.is_null()
        || cursor.is_null()
        || out_actor.is_null()
        || out_index.is_null()
    {
        return Err(1);
    }
    unsafe {
        *out_actor = std::ptr::null_mut();
        *out_index = -1;
    }
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let center = unsafe { trace_location.read_unaligned() };
    let mut index = unsafe { *cursor };
    loop {
        let mut count = 0;
        let status = unsafe { (services.actor_count)(services.context, actor_ptr, &mut count) };
        if status != 0 {
            return Err(status);
        }
        if index >= count {
            break;
        }
        let candidate_index = index;
        let mut candidate = std::ptr::null_mut();
        let status = unsafe {
            (services.actor_at)(services.context, actor_ptr, candidate_index, &mut candidate)
        };
        if status != 0 {
            return Err(status);
        }
        index = index.wrapping_add(1);
        unsafe { *cursor = index };
        if candidate.is_null() {
            continue;
        }
        let mut is_hidden = 0;
        let status =
            unsafe { (services.actor_is_hidden)(services.context, candidate, &mut is_hidden) };
        if status != 0 {
            return Err(status);
        }
        if is_hidden != 0 {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class == 0
            || (radius != 0.0 && !unsafe { inside_actor_radius(candidate, center, radius, false) })
        {
            continue;
        }
        let mut is_visible = 0;
        let status = unsafe {
            (services.actor_fast_line_check)(
                services.context,
                actor_ptr,
                candidate,
                trace_location.cast(),
                &mut is_visible,
            )
        };
        if status != 0 {
            return Err(status);
        }
        if is_visible == 0 {
            continue;
        }
        unsafe {
            *out_actor = candidate;
            *out_index = candidate_index;
        }
        return Ok(());
    }
    Ok(())
}

/// Emits one hit from the `MultiLineCheck` chain and advances its cursor.
///
/// # Safety
/// The hit chain and output pointers must remain valid for the call.
pub unsafe fn trace_actors_next(
    hit: *mut *mut c_void,
    out_actor: *mut *mut c_void,
    out_location: *mut FVector,
    out_normal: *mut FVector,
) -> Result<(), i32> {
    if hit.is_null() || out_actor.is_null() || out_location.is_null() || out_normal.is_null() {
        return Err(1);
    }
    let current = unsafe { *hit };
    if current.is_null() {
        unsafe { *out_actor = std::ptr::null_mut() };
        return Ok(());
    }
    let result = current.cast::<FCheckResult>();
    let next = unsafe { std::ptr::addr_of!((*result).next).read_unaligned() };
    let actor = unsafe { std::ptr::addr_of!((*result).actor).read_unaligned() };
    let location = unsafe { std::ptr::addr_of!((*result).location).read_unaligned() };
    let normal = unsafe { std::ptr::addr_of!((*result).normal).read_unaligned() };
    unsafe {
        *hit = next;
        *out_actor = actor;
        std::ptr::write(out_location, location);
        std::ptr::write(out_normal, normal);
    }
    Ok(())
}

/// Selects the next visible actor from an engine-owned `ActorRadiusCheck` hit chain.
///
/// # Safety
/// The hit chain, actor, trace location and service callbacks must remain valid.
pub unsafe fn visible_colliding_actors_next(
    link: *mut *mut c_void,
    actor_ptr: *mut c_void,
    base_class: *mut *mut c_void,
    ignore_hidden: i32,
    trace_location: *const FVector,
    out_actor: *mut *mut c_void,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if link.is_null() || actor_ptr.is_null() || trace_location.is_null() || out_actor.is_null() {
        return Err(1);
    }
    unsafe { *out_actor = std::ptr::null_mut() };
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let trace_location = trace_location.cast::<c_void>();
    loop {
        let current = unsafe { *link };
        if current.is_null() {
            return Ok(());
        }
        let result = current.cast::<FCheckResult>();
        let candidate = unsafe { std::ptr::addr_of!((*result).actor).read_unaligned() };
        let next = unsafe { std::ptr::addr_of!((*result).next).read_unaligned() };
        unsafe { *link = next };
        if candidate.is_null() {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class == 0 {
            continue;
        }
        if ignore_hidden != 0 {
            let mut is_hidden = 0;
            let status =
                unsafe { (services.actor_is_hidden)(services.context, candidate, &mut is_hidden) };
            if status != 0 {
                return Err(status);
            }
            if is_hidden != 0 {
                continue;
            }
        }
        let mut is_visible = 0;
        let status = unsafe {
            (services.actor_fast_line_check)(
                services.context,
                actor_ptr,
                candidate,
                trace_location,
                &mut is_visible,
            )
        };
        if status != 0 {
            return Err(status);
        }
        if is_visible != 0 {
            unsafe { *out_actor = candidate };
            return Ok(());
        }
    }
}

/// Selects the next actor whose class matches and whose engine zone leaf accepts it.
///
/// # Safety
/// Actor arrays, zone objects, cursors and service callbacks must remain valid.
pub unsafe fn zone_actors_next(
    zone: *mut c_void,
    base_class: *mut *mut c_void,
    cursor: *mut i32,
    out_actor: *mut *mut c_void,
    services: &Hp2ActorIteratorServices,
) -> Result<(), i32> {
    if zone.is_null() || cursor.is_null() || out_actor.is_null() {
        return Err(1);
    }
    unsafe { *out_actor = std::ptr::null_mut() };
    let base_class = unsafe { resolve_class(base_class, services) }?;
    let mut index = unsafe { *cursor };
    loop {
        let mut count = 0;
        let status = unsafe { (services.actor_count)(services.context, zone, &mut count) };
        if status != 0 {
            return Err(status);
        }
        if index >= count {
            break;
        }
        let mut candidate = std::ptr::null_mut();
        let status = unsafe { (services.actor_at)(services.context, zone, index, &mut candidate) };
        if status != 0 {
            return Err(status);
        }
        index = index.wrapping_add(1);
        unsafe { *cursor = index };
        if candidate.is_null() {
            continue;
        }
        let mut matches_class = 0;
        let status = unsafe {
            (services.actor_is_a)(services.context, candidate, base_class, &mut matches_class)
        };
        if status != 0 {
            return Err(status);
        }
        if matches_class == 0 {
            continue;
        }
        let mut is_in_zone = 0;
        let status = unsafe {
            (services.actor_is_in_zone)(services.context, candidate, zone, &mut is_in_zone)
        };
        if status != 0 {
            return Err(status);
        }
        if is_in_zone != 0 {
            unsafe { *out_actor = candidate };
            return Ok(());
        }
    }
    Ok(())
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's `GActorIteratorServices` table.
-----------------------------------------------------------------------------*/

static mut ITERATOR_SERVICES: Option<Hp2ActorIteratorServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorIteratorServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ActorIteratorServices) {
    unsafe { (&raw mut ITERATOR_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ActorIteratorServices, i32> {
    unsafe { (&raw const ITERATOR_SERVICES).read().ok_or(1) }
}
