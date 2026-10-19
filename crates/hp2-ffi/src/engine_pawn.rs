//! Pawn move-primitive exports (`crates/hp2-engine/src/pawn.rs`). The C++
//! `APawn::walkMove`, `flyMove` and `swimMove` in `Engine/Src/UnPawn.cpp`
//! forward here. Engine objects cross as opaque `void*`; the engine-side calls
//! go through the `Hp2PawnServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, FCheckResult};
use hp2_engine::pawn::{self, Hp2PawnServices};
use hp2_ffi_support::guard;

/// Borrows the pawn services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnServices`.
unsafe fn svc_of<'a>(p: *const Hp2PawnServices) -> Option<&'a Hp2PawnServices> {
    unsafe { p.as_ref() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_walk_move(
    svc: *const Hp2PawnServices,
    pawn: *mut c_void,
    delta: *const c_void,
    hit: *mut c_void,
    goal: *mut c_void,
    threshold: f32,
    adjust: i32,
) -> i32 {
    guard("hp2_rs_engine_pawn_walk_move", 0, move || unsafe {
        match (
            svc_of(svc),
            pawn.is_null() || delta.is_null() || hit.is_null(),
        ) {
            (Some(s), false) => pawn::walk_move(
                s,
                pawn.cast::<APawn>(),
                *delta.cast::<FVector>(),
                &mut *hit.cast::<FCheckResult>(),
                goal.cast::<AActor>(),
                threshold,
                adjust,
            ),
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_fly_move(
    svc: *const Hp2PawnServices,
    pawn: *mut c_void,
    delta: *const c_void,
    goal: *mut c_void,
    threshold: f32,
    adjust: i32,
) -> i32 {
    guard("hp2_rs_engine_pawn_fly_move", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || delta.is_null()) {
            (Some(s), false) => pawn::fly_move(
                s,
                pawn.cast::<APawn>(),
                *delta.cast::<FVector>(),
                goal.cast::<AActor>(),
                threshold,
                adjust,
            ),
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_pawn_swim_move(
    svc: *const Hp2PawnServices,
    pawn: *mut c_void,
    delta: *const c_void,
    goal: *mut c_void,
    threshold: f32,
    adjust: i32,
) -> i32 {
    guard("hp2_rs_engine_pawn_swim_move", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || delta.is_null()) {
            (Some(s), false) => pawn::swim_move(
                s,
                pawn.cast::<APawn>(),
                *delta.cast::<FVector>(),
                goal.cast::<AActor>(),
                threshold,
                adjust,
            ),
            _ => 0,
        }
    })
}
