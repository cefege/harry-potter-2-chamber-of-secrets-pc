//! AActor exports (`crates/hp2-engine/src/actor.rs`). The `AActor` methods in
//! `Engine/Src/UnActor.cpp` forward here. Engine objects cross as `void*`;
//! engine calls go through the `Hp2ActorServices` table passed on every call.
//!
//! NULL policy: every C++ caller passes `&GHp2ActorServices` and `this`. An
//! export dereferences exactly what the original dereferences, so a NULL
//! `this` or `Other` faults where the C++ faults. No export skips the work or
//! returns a placeholder for a NULL `this`. NULL owner and base are valid
//! arguments and are handled by the Rust bodies.

use std::ffi::c_void;

use hp2_abi::engine::generated::{AActor, APlayerPawn, FBox};
use hp2_abi::{FCoords, FVector};
use hp2_engine::actor::{self, Hp2ActorServices};
use hp2_ffi_support::guard;

/// `AActor::PreNetReceive`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_pre_net_receive(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_pre_net_receive", (), move || unsafe {
        actor::pre_net_receive(&*svc, actor.cast::<AActor>());
    })
}

/// `AActor::PostNetReceive`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_post_net_receive(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_post_net_receive", (), move || unsafe {
        actor::post_net_receive(&*svc, actor.cast::<AActor>());
    })
}

/// `AActor::GetSkin`.
///
/// # Safety
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_skin(
    actor: *const c_void,
    index: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_actor_get_skin",
        std::ptr::null_mut(),
        move || unsafe { actor::get_skin(actor.cast::<AActor>(), index) },
    )
}

/// `AActor::GetNetPriority`.
///
/// # Safety
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_net_priority(
    actor: *const c_void,
    time: f32,
) -> f32 {
    guard(
        "hp2_rs_engine_actor_get_net_priority",
        0.0,
        move || unsafe { actor::get_net_priority(actor.cast::<AActor>(), time) },
    )
}

/// `AActor::SetCollision`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_set_collision(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    collide_actors: i32,
    block_actors: i32,
    block_players: i32,
) {
    guard("hp2_rs_engine_actor_set_collision", (), move || unsafe {
        actor::set_collision(
            &*svc,
            actor.cast::<AActor>(),
            collide_actors != 0,
            block_actors != 0,
            block_players != 0,
        );
    })
}

/// `AActor::SetCollisionSize`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_set_collision_size(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    radius: f32,
    height: f32,
) {
    guard(
        "hp2_rs_engine_actor_set_collision_size",
        (),
        move || unsafe {
            actor::set_collision_size(&*svc, actor.cast::<AActor>(), radius, height);
        },
    )
}

/// `AActor::SetOwner`. A NULL `owner` is valid.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `owner` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_set_owner(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    owner: *mut c_void,
) {
    guard("hp2_rs_engine_actor_set_owner", (), move || unsafe {
        actor::set_owner(&*svc, actor.cast::<AActor>(), owner);
    })
}

/// `AActor::SetBase`. A NULL `base` is valid.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `base` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_set_base(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    base: *mut c_void,
    notify_actor: i32,
) {
    guard("hp2_rs_engine_actor_set_base", (), move || unsafe {
        actor::set_base(
            &*svc,
            actor.cast::<AActor>(),
            base.cast::<AActor>(),
            notify_actor != 0,
        );
    })
}

/// `AActor::BeginTouch`. The original calls `Other->IsA` first, so a NULL `other` faults.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `other` must point to a live `AActor` (the C++ original faults on NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_begin_touch(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    other: *mut c_void,
) {
    guard("hp2_rs_engine_actor_begin_touch", (), move || unsafe {
        actor::begin_touch(&*svc, actor.cast::<AActor>(), other.cast::<AActor>());
    })
}

/// `AActor::EndTouch`. The original reads `Other->Touching`, so a NULL `other` faults.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `other` must point to a live `AActor` (the C++ original faults on NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_end_touch(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    other: *mut c_void,
    no_notify_self: i32,
) {
    guard("hp2_rs_engine_actor_end_touch", (), move || unsafe {
        actor::end_touch(
            &*svc,
            actor.cast::<AActor>(),
            other.cast::<AActor>(),
            no_notify_self,
        );
    })
}

/// `AActor::IsOverlapping` body. The C++ shell keeps `checkSlow(Other!=NULL)`.
/// A NULL `other` reaches `Other->IsBrush()` in the original, which faults. The
/// Rust side reads `other` in the same place, so it faults the same way.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `other` must point to a live `AActor` (the C++ original faults on NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_is_overlapping(
    svc: *const Hp2ActorServices,
    actor: *const c_void,
    other: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_actor_is_overlapping", 0, move || unsafe {
        actor::is_overlapping(&*svc, actor.cast::<AActor>(), other.cast::<AActor>())
    })
}

/// `AActor::GetVisibilityBox`. `out` points at the caller's `FBox` result.
///
/// # Safety
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `out` must be writable for one `FBox`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_visibility_box(
    actor: *const c_void,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_actor_get_visibility_box",
        (),
        move || unsafe {
            out.cast::<FBox>()
                .write_unaligned(actor::get_visibility_box(actor.cast::<AActor>()));
        },
    )
}

/// `ACarcass::UpdateFrequency`. The viewer and view vectors are unused.
///
/// # Safety
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_carcass_update_frequency(actor: *const c_void) -> f32 {
    guard(
        "hp2_rs_engine_actor_carcass_update_frequency",
        0.0,
        move || unsafe { actor::carcass_update_frequency(actor.cast::<AActor>()) },
    )
}

/// `AActor::GetHitActor`.
///
/// # Safety
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_hit_actor(actor: *mut c_void) -> *mut c_void {
    guard(
        "hp2_rs_engine_actor_get_hit_actor",
        std::ptr::null_mut(),
        move || unsafe { actor::get_hit_actor(actor.cast::<AActor>()) },
    )
}

/// `AActor::PostEditChange` after `Super`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_post_edit_change(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_post_edit_change", (), move || unsafe {
        actor::actor_post_edit_change(&*svc, actor.cast::<AActor>());
    })
}

/// `AZoneInfo::PostEditChange` after `Super`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_zone_info_post_edit_change(
    svc: *const Hp2ActorServices,
) {
    guard(
        "hp2_rs_engine_actor_zone_info_post_edit_change",
        (),
        move || unsafe {
            actor::zone_info_post_edit_change(&*svc);
        },
    )
}

/// `AActor::GetPrimitive`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_primitive(
    svc: *const Hp2ActorServices,
    actor: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_actor_get_primitive",
        std::ptr::null_mut(),
        move || unsafe { actor::get_primitive(&*svc, actor.cast::<AActor>()) },
    )
}

/// `AActor::Spawned`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_spawned(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_spawned", (), move || unsafe {
        actor::spawned(&*svc, actor.cast::<AActor>());
    })
}

/// `AActor::ProcessEvent` (the guard/unguard shell stays in C++).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `func`, `parms` and `result` are opaque engine pointers passed through unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_process_event(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    func: *mut c_void,
    parms: *mut c_void,
    result: *mut c_void,
) {
    guard("hp2_rs_engine_actor_process_event", (), move || unsafe {
        actor::process_event(&*svc, actor.cast::<AActor>(), func, parms, result);
    })
}

/// `AActor::Destroy` (the guard/unguard shell stays in C++).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_destroy(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_destroy", (), move || unsafe {
        actor::destroy(&*svc, actor.cast::<AActor>());
    })
}

/// `AActor::PostLoad` (the guard/unguard shell stays in C++).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_post_load(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_engine_actor_post_load", (), move || unsafe {
        actor::post_load(&*svc, actor.cast::<AActor>());
    })
}

/// `APlayerPawn::SetPlayer` (the guard/unguard shell stays in C++; the `check` runs in Rust).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `pawn` must point to a live `APlayerPawn` for the call duration. `player` is opaque.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_set_player(
    svc: *const Hp2ActorServices,
    pawn: *mut c_void,
    player: *mut c_void,
) {
    guard("hp2_rs_engine_actor_set_player", (), move || unsafe {
        actor::set_player(&*svc, pawn.cast::<APlayerPawn>(), player);
    })
}

/// `AActor::GetCylinderExtent`, result written to `out` (`FVector*`).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `out` must be writable for one `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_cylinder_extent(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_actor_get_cylinder_extent",
        (),
        move || unsafe {
            actor::get_cylinder_extent(&*svc, actor.cast::<AActor>(), out.cast::<FVector>());
        },
    )
}

/// `AActor::GetRenderBoundingBox`, result written to `out` (`FCoords*`).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `out` must be writable for one `FCoords`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_get_render_bounding_box(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    exact: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_actor_get_render_bounding_box",
        (),
        move || unsafe {
            actor::get_render_bounding_box(
                &*svc,
                actor.cast::<AActor>(),
                exact,
                out.cast::<FCoords>(),
            );
        },
    )
}

/// `AActor::SerializePersistence` (guard/unguard stay in C++).
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `ar` is an opaque engine archive pointer passed through unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_serialize_persistence(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    ar: *mut c_void,
) {
    guard(
        "hp2_rs_engine_actor_serialize_persistence",
        (),
        move || unsafe {
            actor::serialize_persistence(&*svc, actor.cast::<AActor>(), ar);
        },
    )
}

/// `AActor::execTraceTexture` core (P_GET macros and `P_FINISH` stay in C++). Returns the `UTexture*`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorServices` table.
/// `actor` must point to a live `AActor`-layout object for the call duration.
/// `trace_end` and `trace_start` must point to readable `FVector`s; `flags` must be writable for one `i32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_trace_texture(
    svc: *const Hp2ActorServices,
    actor: *mut c_void,
    trace_end: *const c_void,
    trace_start: *const c_void,
    flags: *mut i32,
    trace_decals: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_actor_trace_texture",
        std::ptr::null_mut(),
        move || unsafe {
            actor::trace_texture(
                &*svc,
                actor.cast::<AActor>(),
                trace_end.cast::<FVector>(),
                trace_start.cast::<FVector>(),
                flags,
                trace_decals != 0,
            )
        },
    )
}
