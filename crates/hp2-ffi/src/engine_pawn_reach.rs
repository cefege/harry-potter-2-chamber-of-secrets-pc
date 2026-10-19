//! Pawn reachability exports (`crates/hp2-engine/src/pawn_reach.rs`). The C++
//! `APawn::actorReachable`, `pointReachable`, `Reachable`, `flyReachable`,
//! `swimReachable`, `walkReachable`, `jumpReachable`, `jumpLanding`,
//! `FindJumpUp`, `SuggestJumpVelocity` and `FindBestJump` in
//! `Engine/Src/UnPawn.cpp` forward here. Engine objects and vectors cross as
//! `void*`; engine calls go through the `Hp2PawnReachServices` table.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn};
use hp2_engine::pawn_reach::{self, Hp2PawnReachServices};
use hp2_ffi_support::guard;

/// Borrows the reach services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnReachServices`.
unsafe fn svc_of<'a>(p: *const Hp2PawnReachServices) -> Option<&'a Hp2PawnReachServices> {
    unsafe { p.as_ref() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_actor_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    other: *mut c_void,
    know_visible: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_actor_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null()) {
                (Some(s), false) => pawn_reach::actor_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    other.cast::<AActor>(),
                    know_visible,
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_point_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    a_point: *const c_void,
    know_visible: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_point_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || a_point.is_null()) {
                (Some(s), false) => pawn_reach::point_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    *a_point.cast::<FVector>(),
                    know_visible,
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    a_point: *const c_void,
    threshold: f32,
    goal: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_pawn_reach_reachable", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || a_point.is_null()) {
            (Some(s), false) => pawn_reach::reachable(
                s,
                pawn.cast::<APawn>(),
                *a_point.cast::<FVector>(),
                threshold,
                goal.cast::<AActor>(),
            ),
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_walk_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    threshold: f32,
    reach_flags: i32,
    goal: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_walk_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || dest.is_null()) {
                (Some(s), false) => pawn_reach::walk_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    threshold,
                    reach_flags,
                    goal.cast::<AActor>(),
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_fly_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    threshold: f32,
    reach_flags: i32,
    goal: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_fly_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || dest.is_null()) {
                (Some(s), false) => pawn_reach::fly_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    threshold,
                    reach_flags,
                    goal.cast::<AActor>(),
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_swim_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    threshold: f32,
    reach_flags: i32,
    goal: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_swim_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || dest.is_null()) {
                (Some(s), false) => pawn_reach::swim_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    threshold,
                    reach_flags,
                    goal.cast::<AActor>(),
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_jump_reachable(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    threshold: f32,
    reach_flags: i32,
    goal: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_jump_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || dest.is_null()) {
                (Some(s), false) => pawn_reach::jump_reachable(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    threshold,
                    reach_flags,
                    goal.cast::<AActor>(),
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_jump_landing(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    test_vel: *const c_void,
    landing: *mut c_void,
    move_pawn: i32,
) {
    guard(
        "hp2_rs_engine_pawn_reach_jump_landing",
        (),
        move || unsafe {
            if let (Some(s), false) = (
                svc_of(svc),
                pawn.is_null() || test_vel.is_null() || landing.is_null(),
            ) {
                pawn_reach::jump_landing(
                    s,
                    pawn.cast::<APawn>(),
                    *test_vel.cast::<FVector>(),
                    &mut *landing.cast::<FVector>(),
                    move_pawn,
                );
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_find_jump_up(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    vel: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_pawn_reach_find_jump_up", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || vel.is_null()) {
            (Some(s), false) => {
                pawn_reach::find_jump_up(s, pawn.cast::<APawn>(), *vel.cast::<FVector>())
            }
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_suggest_jump_velocity(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    vel: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_reach_suggest_jump_velocity",
        (),
        move || unsafe {
            if let (Some(s), false) = (
                svc_of(svc),
                pawn.is_null() || dest.is_null() || vel.is_null(),
            ) {
                pawn_reach::suggest_jump_velocity(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    &mut *vel.cast::<FVector>(),
                );
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_reach_find_best_jump(
    svc: *const Hp2PawnReachServices,
    pawn: *mut c_void,
    dest: *const c_void,
    vel: *const c_void,
    landing: *mut c_void,
    move_pawn: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_reach_find_best_jump",
        0,
        move || unsafe {
            match (
                svc_of(svc),
                pawn.is_null() || dest.is_null() || vel.is_null() || landing.is_null(),
            ) {
                (Some(s), false) => pawn_reach::find_best_jump(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    *vel.cast::<FVector>(),
                    &mut *landing.cast::<FVector>(),
                    move_pawn,
                ),
                _ => 0,
            }
        },
    )
}
