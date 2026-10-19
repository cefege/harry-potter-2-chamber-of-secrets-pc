//! Pawn AI exports (`crates/hp2-engine/src/pawn_native_ai.rs`). The
//! `AActor::execPlayerCanSeeMe`/`TestCanSeeMe`, `APawn::execPickTarget`,
//! `execPickAnyTarget`, `execAddPawn`, `execRemovePawn`, `execStopWaiting`,
//! `execWaitForLanding`, `execPollWaitForLanding` and `execFindStairRotation`
//! thunks in `Engine/Src/UnPawn.cpp` forward here. Engine objects cross as `void*`;
//! engine calls go through the `Hp2PawnNativeAiServices` table passed on every call.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, APlayerPawn, FCheckResult};
use hp2_engine::pawn_native_ai::{self, Hp2PawnNativeAiServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnNativeAiServices`.
unsafe fn svc_of<'a>(p: *const Hp2PawnNativeAiServices) -> Option<&'a Hp2PawnNativeAiServices> {
    unsafe { p.as_ref() }
}

/// `AActor::execPlayerCanSeeMe`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_player_can_see_me(
    svc: *const Hp2PawnNativeAiServices,
    actor: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_ai_player_can_see_me",
        0,
        move || unsafe {
            match (svc_of(svc), actor.is_null()) {
                (Some(s), false) => {
                    i32::from(pawn_native_ai::player_can_see_me(s, actor.cast::<AActor>()))
                }
                _ => 0,
            }
        },
    )
}

/// `AActor::TestCanSeeMe(Viewer)`. `viewer` may be null. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_test_can_see_me(
    svc: *const Hp2PawnNativeAiServices,
    actor: *mut c_void,
    viewer: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_ai_test_can_see_me",
        0,
        move || unsafe {
            match (svc_of(svc), actor.is_null()) {
                (Some(s), false) => i32::from(pawn_native_ai::test_can_see_me(
                    s,
                    actor.cast::<AActor>(),
                    viewer.cast::<APlayerPawn>(),
                )),
                _ => 0,
            }
        },
    )
}

/// `APawn::execPickTarget`. `best_aim`/`best_dist` are in-out; `fire_dir` and
/// `proj_start` are `FVector`. Returns the picked `APawn*` or null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_pick_target(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
    best_aim: *mut f32,
    best_dist: *mut f32,
    fire_dir: *const c_void,
    proj_start: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_ai_pick_target",
        null_mut::<c_void>(),
        move || unsafe {
            let missing = pawn.is_null()
                || best_aim.is_null()
                || best_dist.is_null()
                || fire_dir.is_null()
                || proj_start.is_null();
            match (svc_of(svc), missing) {
                (Some(s), false) => pawn_native_ai::pick_target(
                    s,
                    pawn.cast::<APawn>(),
                    best_aim,
                    best_dist,
                    *fire_dir.cast::<FVector>(),
                    *proj_start.cast::<FVector>(),
                )
                .cast::<c_void>(),
                _ => null_mut::<c_void>(),
            }
        },
    )
}

/// `APawn::execPickAnyTarget`. Same arguments as `pick_target`; returns the picked
/// `AActor*` or null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_pick_any_target(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
    best_aim: *mut f32,
    best_dist: *mut f32,
    fire_dir: *const c_void,
    proj_start: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_ai_pick_any_target",
        null_mut::<c_void>(),
        move || unsafe {
            let missing = pawn.is_null()
                || best_aim.is_null()
                || best_dist.is_null()
                || fire_dir.is_null()
                || proj_start.is_null();
            match (svc_of(svc), missing) {
                (Some(s), false) => pawn_native_ai::pick_any_target(
                    s,
                    pawn.cast::<APawn>(),
                    best_aim,
                    best_dist,
                    *fire_dir.cast::<FVector>(),
                    *proj_start.cast::<FVector>(),
                )
                .cast::<c_void>(),
                _ => null_mut::<c_void>(),
            }
        },
    )
}

/// `APawn::execAddPawn`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_add_pawn(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_ai_add_pawn",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_ai::add_pawn(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execRemovePawn`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_remove_pawn(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_ai_remove_pawn",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_ai::remove_pawn(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execStopWaiting`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_stop_waiting(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_ai_stop_waiting",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_ai::stop_waiting(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execWaitForLanding`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_wait_for_landing(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_ai_wait_for_landing",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_ai::wait_for_landing(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execPollWaitForLanding`. `result` is the thunk's `Result`, read only when the actor is falling.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_poll_wait_for_landing(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
    result: *const c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_ai_poll_wait_for_landing",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_ai::poll_wait_for_landing(
                    s,
                    pawn.cast::<APawn>(),
                    result.cast::<f32>(),
                );
            }
        },
    )
}

/// `APawn::execFindStairRotation`. `hit` is the thunk's `FCheckResult Hit(1.f)`.
/// Returns the stair pitch as `INT`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_ai_find_stair_rotation(
    svc: *const Hp2PawnNativeAiServices,
    pawn: *mut c_void,
    delta_time: f32,
    hit: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_native_ai_find_stair_rotation",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || hit.is_null()) {
                (Some(s), false) => pawn_native_ai::find_stair_rotation(
                    s,
                    pawn.cast::<APawn>(),
                    delta_time,
                    hit.cast::<FCheckResult>(),
                ),
                _ => 0,
            }
        },
    )
}
