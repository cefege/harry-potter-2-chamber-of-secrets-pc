//! Move natives (`crates/hp2-engine/src/pawn_native_move.rs`).
//! `APawn::execactorReachable`, `execFindPathTo`, `execFindPathToward`,
//! `execFindRandomDest`, `AActor::execMakeNoise`, the MoveTo, MoveToward,
//! StrafeTo, StrafeFacing, TurnToward and TurnTo latent actions with their
//! Poll continuations, and `APawn::ShowSelf` in `Engine/Src/UnPawn.cpp` forward here.
//! Engine objects and vectors cross as `void*`; engine calls go through the
//! `Hp2PawnNativeMoveServices` table passed on every call.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::APawn;
use hp2_engine::pawn_native_move::{self, Hp2PawnNativeMoveServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnNativeMoveServices`.
unsafe fn svc_of<'a>(p: *const Hp2PawnNativeMoveServices) -> Option<&'a Hp2PawnNativeMoveServices> {
    unsafe { p.as_ref() }
}

/// `APawn::execactorReachable` body after the `!actor` check. Returns `DWORD`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_actor_reachable(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    actor: *mut c_void,
) -> u32 {
    guard(
        "hp2_rs_engine_pawn_native_move_actor_reachable",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || actor.is_null()) {
                (Some(s), false) => {
                    pawn_native_move::actor_reachable(s, pawn.cast::<APawn>(), actor)
                }
                _ => 0,
            }
        },
    )
}

/// `APawn::execFindPathTo` body between the clock calls. Returns `bestPath`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_find_path_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    point: *const c_void,
    single: i32,
    clear: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_move_find_path_to",
        ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || point.is_null()) {
                (Some(s), false) => {
                    pawn_native_move::find_path_to(s, pawn.cast::<APawn>(), point, single, clear)
                }
                _ => ptr::null_mut(),
            }
        },
    )
}

/// `APawn::execFindPathToward` body after the `!goal` check. Returns `bestPath`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_find_path_toward(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    goal: *mut c_void,
    single: i32,
    clear: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_move_find_path_toward",
        ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || goal.is_null()) {
                (Some(s), false) => {
                    pawn_native_move::find_path_toward(s, pawn.cast::<APawn>(), goal, single, clear)
                }
                _ => ptr::null_mut(),
            }
        },
    )
}

/// `APawn::execFindRandomDest` body between the clock calls. Returns `bestPath`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_find_random_dest(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    clear: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_pawn_native_move_find_random_dest",
        ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), pawn.is_null()) {
                (Some(s), false) => {
                    pawn_native_move::find_random_dest(s, pawn.cast::<APawn>(), clear)
                }
                _ => ptr::null_mut(),
            }
        },
    )
}

/// `AActor::execMakeNoise` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_make_noise(
    svc: *const Hp2PawnNativeMoveServices,
    actor: *mut c_void,
    loudness: f32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_make_noise",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), actor.is_null()) {
                pawn_native_move::make_noise(s, actor, loudness);
            }
        },
    )
}

/// `APawn::execMoveTo` body after `P_GET_*`/`P_FINISH`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_move_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    dest: *const c_void,
    speed: f32,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_move_to",
        (),
        move || unsafe {
            if let (Some(s), false, false) = (svc_of(svc), pawn.is_null(), dest.is_null()) {
                pawn_native_move::move_to(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    speed,
                    latent,
                );
            }
        },
    )
}

/// `APawn::execPollMoveTo` and `APawn::execPollStrafeTo` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_move_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_move_to",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_destination(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execPollStrafeTo` body. Same body as `execPollMoveTo`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_strafe_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_strafe_to",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_destination(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execMoveToward` body after `P_GET_*`/`P_FINISH`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_move_toward(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    goal: *mut c_void,
    speed: f32,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_move_toward",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::move_toward(s, pawn.cast::<APawn>(), goal, speed, latent);
            }
        },
    )
}

/// `APawn::execPollMoveToward` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_move_toward(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_move_toward",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_move_toward(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execStrafeTo` body after `P_GET_VECTOR` x2.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_strafe_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    dest: *const c_void,
    focal: *const c_void,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_strafe_to",
        (),
        move || unsafe {
            if let (Some(s), false, false, false) =
                (svc_of(svc), pawn.is_null(), dest.is_null(), focal.is_null())
            {
                pawn_native_move::strafe_to(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    *focal.cast::<FVector>(),
                    latent,
                );
            }
        },
    )
}

/// `APawn::execStrafeFacing` body after `P_FINISH`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_strafe_facing(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    dest: *const c_void,
    goal: *mut c_void,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_strafe_facing",
        (),
        move || unsafe {
            if let (Some(s), false, false) = (svc_of(svc), pawn.is_null(), dest.is_null()) {
                pawn_native_move::strafe_facing(
                    s,
                    pawn.cast::<APawn>(),
                    *dest.cast::<FVector>(),
                    goal,
                    latent,
                );
            }
        },
    )
}

/// `APawn::execPollStrafeFacing` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_strafe_facing(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_strafe_facing",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_strafe_facing(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execTurnToward` body after `P_FINISH`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_turn_toward(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    goal: *mut c_void,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_turn_toward",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::turn_toward(s, pawn.cast::<APawn>(), goal, latent);
            }
        },
    )
}

/// `APawn::execPollTurnToward` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_turn_toward(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_turn_toward",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_turn_toward(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::execTurnTo` body after `P_GET_VECTOR`/`P_FINISH`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_turn_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
    focal: *const c_void,
    latent: i32,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_turn_to",
        (),
        move || unsafe {
            if let (Some(s), false, false) = (svc_of(svc), pawn.is_null(), focal.is_null()) {
                pawn_native_move::turn_to(
                    s,
                    pawn.cast::<APawn>(),
                    *focal.cast::<FVector>(),
                    latent,
                );
            }
        },
    )
}

/// `APawn::execPollTurnTo` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_poll_turn_to(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_poll_turn_to",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::poll_turn_to(s, pawn.cast::<APawn>());
            }
        },
    )
}

/// `APawn::ShowSelf` body between the clock calls.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_move_show_self(
    svc: *const Hp2PawnNativeMoveServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_move_show_self",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_move::show_self(s, pawn.cast::<APawn>());
            }
        },
    )
}
