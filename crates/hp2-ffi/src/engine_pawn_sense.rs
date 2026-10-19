//! Pawn sense exports (`crates/hp2-engine/src/pawn_sense.rs`). The C++
//! `APawn::setMoveTimer`, `moveToward`, `rotateToward`, `LineOfSightTo`,
//! `CanHear`, `AActor::CheckNoiseHearing`, `CheckEnemyVisible`,
//! `PickWallAdjust` and `APawn::GetNetPriority` in `Engine/Src/UnPawn.cpp`
//! forward here. Engine objects and vectors cross as `void*`; engine calls go
//! through the `Hp2PawnSenseServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn};
use hp2_engine::pawn_sense::{self, Hp2PawnSenseServices};
use hp2_ffi_support::guard;

/// Borrows the sense services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnSenseServices`.
unsafe fn svc_of<'a>(p: *const Hp2PawnSenseServices) -> Option<&'a Hp2PawnSenseServices> {
    unsafe { p.as_ref() }
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_set_move_timer(pawn: *mut c_void, move_size: f32) {
    guard(
        "hp2_rs_engine_pawn_sense_set_move_timer",
        (),
        move || unsafe {
            if !pawn.is_null() {
                pawn_sense::set_move_timer(pawn.cast::<APawn>(), move_size);
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_move_toward(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
    dest: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_pawn_sense_move_toward", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || dest.is_null()) {
            (Some(s), false) => {
                pawn_sense::move_toward(s, pawn.cast::<APawn>(), *dest.cast::<FVector>())
            }
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_rotate_toward(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
    focal: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_sense_rotate_toward",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null() || focal.is_null()) {
                (Some(s), false) => {
                    pawn_sense::rotate_toward(s, pawn.cast::<APawn>(), *focal.cast::<FVector>())
                }
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_line_of_sight_to(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
    other: *mut c_void,
    show_self: i32,
) -> u32 {
    guard(
        "hp2_rs_engine_pawn_sense_line_of_sight_to",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null()) {
                (Some(s), false) => pawn_sense::line_of_sight_to(
                    s,
                    pawn.cast::<APawn>(),
                    other.cast::<AActor>(),
                    show_self,
                ),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_can_hear(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
    noise_loc: *const c_void,
    loudness: f32,
    other: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_pawn_sense_can_hear", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || noise_loc.is_null()) {
            (Some(s), false) => pawn_sense::can_hear(
                s,
                pawn.cast::<APawn>(),
                *noise_loc.cast::<FVector>(),
                loudness,
                other.cast::<APawn>(),
            ),
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_check_noise_hearing(
    svc: *const Hp2PawnSenseServices,
    this: *mut c_void,
    loudness: f32,
) {
    guard(
        "hp2_rs_engine_pawn_sense_check_noise_hearing",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), this.is_null()) {
                pawn_sense::check_noise_hearing(s, this.cast::<AActor>(), loudness);
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_check_enemy_visible(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_sense_check_enemy_visible",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_sense::check_enemy_visible(s, pawn.cast::<APawn>());
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_pick_wall_adjust(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_pawn_sense_pick_wall_adjust",
        0,
        move || unsafe {
            match (svc_of(svc), pawn.is_null()) {
                (Some(s), false) => pawn_sense::pick_wall_adjust(s, pawn.cast::<APawn>()),
                _ => 0,
            }
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_get_net_priority(
    pawn: *mut c_void,
    recent: *mut c_void,
    time: f32,
    lag: f32,
) -> f32 {
    guard(
        "hp2_rs_engine_pawn_sense_get_net_priority",
        0.0,
        move || unsafe {
            if pawn.is_null() {
                return 0.0;
            }
            pawn_sense::get_net_priority(pawn.cast::<APawn>(), recent.cast::<AActor>(), time, lag)
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_sense_handle_special(
    svc: *const Hp2PawnSenseServices,
    pawn: *mut c_void,
    best_path: *mut *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_sense_handle_special",
        (),
        move || unsafe {
            if let (Some(s), false, false) = (svc_of(svc), pawn.is_null(), best_path.is_null()) {
                pawn_sense::handle_special(
                    s,
                    pawn.cast::<APawn>(),
                    best_path.cast::<*mut AActor>(),
                );
            }
        },
    )
}
