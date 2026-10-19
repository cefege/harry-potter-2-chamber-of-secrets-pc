//! Modern third-person input exports (`crates/hp2-engine/src/pawn_native_input.rs`).
//! `APlayerPawn::BeginModernThirdPersonInput`, `FinishModernThirdPersonInput`
//! and `ApplyModernThirdPersonMovement` in `Engine/Src/UnPawn.cpp` forward here.
//! Engine objects cross as `void*`; engine calls go through the
//! `Hp2PawnNativeInputServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::engine::generated::APlayerPawn;
use hp2_engine::pawn_native_input::{self, Hp2PawnNativeInputServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2PawnNativeInputServices`.
unsafe fn svc_of<'a>(
    p: *const Hp2PawnNativeInputServices,
) -> Option<&'a Hp2PawnNativeInputServices> {
    unsafe { p.as_ref() }
}

/// `APlayerPawn::BeginModernThirdPersonInput`. `raw` is the in-out array
/// `[RawBaseX, RawStrafe, RawCameraYaw, RawCameraPitch]`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_input_begin(
    svc: *const Hp2PawnNativeInputServices,
    pawn: *mut c_void,
    raw: *mut f32,
) -> i32 {
    guard("hp2_rs_engine_pawn_native_input_begin", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || raw.is_null()) {
            (Some(s), false) => {
                let raw = &mut *raw.cast::<[f32; 4]>();
                i32::from(pawn_native_input::begin_modern_third_person_input(
                    s,
                    pawn.cast::<APlayerPawn>(),
                    raw,
                ))
            }
            _ => 0,
        }
    })
}

/// `APlayerPawn::FinishModernThirdPersonInput`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_pawn_native_input_finish(
    svc: *const Hp2PawnNativeInputServices,
    pawn: *mut c_void,
    active: i32,
    raw_base_x: f32,
    raw_strafe: f32,
    raw_camera_yaw: f32,
    raw_camera_pitch: f32,
    delta_seconds: f32,
) {
    guard(
        "hp2_rs_engine_pawn_native_input_finish",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_input::finish_modern_third_person_input(
                    s,
                    pawn.cast::<APlayerPawn>(),
                    active != 0,
                    raw_base_x,
                    raw_strafe,
                    raw_camera_yaw,
                    raw_camera_pitch,
                    delta_seconds,
                );
            }
        },
    )
}

/// `APlayerPawn::ApplyModernThirdPersonMovement`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_pawn_native_input_apply(
    svc: *const Hp2PawnNativeInputServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_pawn_native_input_apply",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
                pawn_native_input::apply_modern_third_person_movement(
                    s,
                    pawn.cast::<APlayerPawn>(),
                );
            }
        },
    )
}
