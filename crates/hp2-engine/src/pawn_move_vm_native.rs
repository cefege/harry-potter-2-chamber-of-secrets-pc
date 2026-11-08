//! Canonical VM bindings for source latent movement actions and their polls.
use hp2_abi::{FVector, engine::generated::{self, AActor, FFrame, UObject}};
use hp2_core::{canonical_vm_native::{finish_arguments, step_argument, VmNativeSpec}, core_access::{CoreAccess, RuntimeResult}};
use std::{ffi::c_void, ptr};
use crate::pawn_native_move_owner as owner;

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 13] = [
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "MoveTo", native_index: Some(500), handler: move_to },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollMoveTo", native_index: Some(501), handler: poll_destination },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "MoveToward", native_index: Some(502), handler: move_toward },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollMoveToward", native_index: Some(503), handler: poll_move_toward },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "StrafeTo", native_index: Some(504), handler: strafe_to },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollStrafeTo", native_index: Some(505), handler: poll_destination },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "StrafeFacing", native_index: Some(506), handler: strafe_facing },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollStrafeFacing", native_index: Some(507), handler: poll_strafe_facing },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "TurnTo", native_index: Some(508), handler: turn_to },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollTurnTo", native_index: Some(509), handler: poll_turn_to },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "TurnToward", native_index: Some(510), handler: turn_toward },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "PollTurnToward", native_index: Some(511), handler: poll_turn_toward },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "MakeNoise", native_index: Some(512), handler: make_noise },
];
unsafe fn move_to(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut dest = FVector { x: 0., y: 0., z: 0. }; let mut speed = 1_f32;
        step_argument(access, frame, (&raw mut dest).cast())?; step_argument(access, frame, (&raw mut speed).cast())?;
        finish_arguments(access, frame)?;
        owner::move_to(access, object.cast(), dest, speed, generated::AI_PollMoveTo)
    }
}
// Polls are invoked by ProcessState, not a script parameter list: no P_FINISH.
unsafe fn poll_destination(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { owner::poll_destination(access, object.cast()) }
}
unsafe fn move_toward(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut goal: *mut AActor = ptr::null_mut(); let mut speed = 1_f32;
        step_argument(access, frame, (&raw mut goal).cast())?; step_argument(access, frame, (&raw mut speed).cast())?;
        finish_arguments(access, frame)?;
        owner::move_toward(access, object.cast(), goal, speed, generated::AI_PollMoveToward)
    }
}
unsafe fn poll_move_toward(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { owner::poll_move_toward(access, object.cast()) }
}
unsafe fn strafe_to(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut dest = FVector { x: 0., y: 0., z: 0. }; let mut focus = dest;
        step_argument(access, frame, (&raw mut dest).cast())?; step_argument(access, frame, (&raw mut focus).cast())?;
        finish_arguments(access, frame)?;
        owner::strafe_to(access, object.cast(), dest, focus, generated::AI_PollStrafeTo)
    }
}
unsafe fn strafe_facing(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut dest = FVector { x: 0., y: 0., z: 0. }; let mut goal: *mut AActor = ptr::null_mut();
        step_argument(access, frame, (&raw mut dest).cast())?; step_argument(access, frame, (&raw mut goal).cast())?;
        finish_arguments(access, frame)?;
        owner::strafe_facing(access, object.cast(), dest, goal, generated::AI_PollStrafeFacing)
    }
}
unsafe fn poll_strafe_facing(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { owner::poll_strafe_facing(access, object.cast()) }
}
unsafe fn turn_to(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut focus = FVector { x: 0., y: 0., z: 0. }; step_argument(access, frame, (&raw mut focus).cast())?;
        finish_arguments(access, frame)?;
        owner::turn_to(access, object.cast(), focus, generated::AI_PollTurnTo)
    }
}
unsafe fn poll_turn_to(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { owner::poll_turn_to(access, object.cast()) }
}
unsafe fn turn_toward(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut goal: *mut AActor = ptr::null_mut(); step_argument(access, frame, (&raw mut goal).cast())?;
        finish_arguments(access, frame)?;
        owner::turn_toward(access, object.cast(), goal, generated::AI_PollTurnToward)
    }
}
unsafe fn poll_turn_toward(access: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { owner::poll_turn_toward(access, object.cast()) }
}
unsafe fn make_noise(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut loudness = 0_f32; step_argument(access, frame, (&raw mut loudness).cast())?;
        finish_arguments(access, frame)?;
        owner::make_noise(access, object.cast(), loudness)
    }
}
