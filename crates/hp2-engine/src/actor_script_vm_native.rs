//! Actor latent actions and animation queries through the sole canonical VM registry.
use hp2_abi::engine::generated::{self, FFrame, FName, UClass, UObject};
use hp2_core::{canonical_vm_native::{finish_arguments, step_argument, VmNativeSpec}, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use std::{ffi::c_void, ptr};
use crate::{actor, actor_anim, actor_anim_owner as owner};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 17] = [
    VmNativeSpec { class_name: "Engine.Actor", function_name: "Sleep", native_index: Some(256), handler: sleep },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "FinishAnim", native_index: Some(261), handler: finish_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "FinishInterpolation", native_index: Some(301), handler: finish_interpolation },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PollSleep", native_index: Some(generated::EPOLL_Sleep as u16), handler: poll_sleep },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PollFinishAnim", native_index: Some(generated::EPOLL_FinishAnim as u16), handler: poll_finish_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PollFinishInterpolation", native_index: Some(generated::EPOLL_FinishInterpolation as u16), handler: poll_finish_interpolation },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "CreateAnimChannel", native_index: Some(265), handler: create_channel },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PlayAnim", native_index: Some(259), handler: play_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "LoopAnim", native_index: Some(260), handler: loop_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "TweenAnim", native_index: Some(294), handler: tween_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "IsAnimating", native_index: Some(282), handler: is_animating },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "GetAnimGroup", native_index: Some(293), handler: get_anim_group },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "HasAnim", native_index: Some(263), handler: has_anim },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "BoneNumber", native_index: Some(268), handler: bone_number },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "BoneName", native_index: Some(269), handler: bone_name },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "BonePos", native_index: Some(257), handler: bone_pos },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "BoneRot", native_index: Some(328), handler: bone_rot },
];
fn latent_status(status: i32) -> RuntimeResult<()> {
    if status == 0 { Ok(()) } else { Err(RuntimeError::new("Actor latent action has no state frame")) }
}
unsafe fn sleep(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut seconds = 0_f32; step_argument(access, frame, (&raw mut seconds).cast())?; finish_arguments(access, frame)?;
        latent_status(actor::exec_sleep(object.cast(), seconds, generated::EPOLL_Sleep, &owner::LATENT_SERVICES))
    }
}
unsafe fn finish_anim(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut bone = FName { index: 0 }; step_argument(access, frame, (&raw mut bone).cast())?; finish_arguments(access, frame)?;
        owner::finish_anim(access, object.cast(), bone, generated::EPOLL_FinishAnim)
    }
}
unsafe fn finish_interpolation(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; latent_status(actor::exec_finish_interpolation(object.cast(), generated::EPOLL_FinishInterpolation, &owner::LATENT_SERVICES)) }
}
// Latent polls receive elapsed seconds via Result and never step script code.
unsafe fn poll_sleep(_: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { latent_status(actor::poll_sleep(object.cast(), result.cast::<f32>().read_unaligned(), &owner::LATENT_SERVICES)) }
}
unsafe fn poll_finish_anim(_: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { latent_status(actor::poll_finish_anim(object.cast(), &owner::LATENT_SERVICES)) }
}
unsafe fn poll_finish_interpolation(_: CoreAccess<'_>, object: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { latent_status(actor::poll_finish_interpolation(object.cast(), &owner::LATENT_SERVICES)) }
}
unsafe fn create_channel(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut class: *mut UClass = ptr::null_mut(); let mut kind = 0_i32; let mut bone = FName { index: 0 }; let mut transient = 0_u32; let mut protected = 0_u32;
        step_argument(access, frame, (&raw mut class).cast())?; step_argument(access, frame, (&raw mut kind).cast())?;
        step_argument(access, frame, (&raw mut bone).cast())?; step_argument(access, frame, (&raw mut transient).cast())?; step_argument(access, frame, (&raw mut protected).cast())?;
        finish_arguments(access, frame)?;
        let channel = owner::create_channel(access, object.cast(), class, kind, bone, transient as i32, protected as i32)?;
        result.cast::<*mut generated::AActor>().write_unaligned(channel); Ok(())
    }
}
unsafe fn play_anim(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut sequence = FName { index: 0 }; let mut rate = 1_f32; let mut tween = -1_f32; let mut kind = 0_i32; let mut bone = FName { index: 0 };
        step_argument(access, frame, (&raw mut sequence).cast())?; step_argument(access, frame, (&raw mut rate).cast())?; step_argument(access, frame, (&raw mut tween).cast())?;
        step_argument(access, frame, (&raw mut kind).cast())?; step_argument(access, frame, (&raw mut bone).cast())?; finish_arguments(access, frame)?;
        owner::play(access, object.cast(), sequence, 0, rate, tween, 0., kind, bone).map(|_| ())
    }
}
unsafe fn loop_anim(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut sequence = FName { index: 0 }; let mut rate = 1_f32; let mut tween = -1_f32; let mut min_rate = 0_f32; let mut kind = 0_i32; let mut bone = FName { index: 0 };
        step_argument(access, frame, (&raw mut sequence).cast())?; step_argument(access, frame, (&raw mut rate).cast())?; step_argument(access, frame, (&raw mut tween).cast())?;
        step_argument(access, frame, (&raw mut min_rate).cast())?; step_argument(access, frame, (&raw mut kind).cast())?; step_argument(access, frame, (&raw mut bone).cast())?; finish_arguments(access, frame)?;
        owner::play(access, object.cast(), sequence, 1, rate, tween, min_rate, kind, bone).map(|_| ())
    }
}
unsafe fn tween_anim(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut sequence = FName { index: 0 }; let mut tween = 0_f32;
        step_argument(access, frame, (&raw mut sequence).cast())?; step_argument(access, frame, (&raw mut tween).cast())?; finish_arguments(access, frame)?;
        owner::play(access, object.cast(), sequence, 0, 0., tween, 0., 0, FName { index: 0 }).map(|_| ())
    }
}
unsafe fn is_animating(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut bone = FName { index: 0 }; step_argument(access, frame, (&raw mut bone).cast())?; finish_arguments(access, frame)?;
        result.cast::<u32>().write_unaligned(owner::is_animating(access, object.cast(), bone)? as u32); Ok(())
    }
}
unsafe fn get_anim_group(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut name = FName { index: 0 }; step_argument(access, frame, (&raw mut name).cast())?; finish_arguments(access, frame)?;
        owner::get_anim_group(access, object.cast(), name, frame, result)
    }
}
unsafe fn has_anim(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut name = FName { index: 0 }; step_argument(access, frame, (&raw mut name).cast())?; finish_arguments(access, frame)?;
        owner::has_anim(access, object.cast(), name, frame, result)
    }
}
unsafe fn bone_number(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut name = FName { index: 0 }; step_argument(access, frame, (&raw mut name).cast())?; finish_arguments(access, frame)?;
        result.cast::<i32>().write_unaligned(owner::bone_number(access, object.cast(), name)?); Ok(())
    }
}
unsafe fn bone_name(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut number = 0_i32; step_argument(access, frame, (&raw mut number).cast())?; finish_arguments(access, frame)?;
        result.cast::<FName>().write_unaligned(owner::bone_name(access, object.cast(), number)?); Ok(())
    }
}
unsafe fn bone_pos(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { bone_transform(access, object, frame, result, actor_anim::BONE_TRANSFORM_POSITION) }
}
unsafe fn bone_rot(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { bone_transform(access, object, frame, result, actor_anim::BONE_TRANSFORM_ROTATION) }
}
unsafe fn bone_transform(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void, kind: i32) -> RuntimeResult<()> {
    unsafe {
        let mut name = FName { index: 0 }; step_argument(access, frame, (&raw mut name).cast())?; finish_arguments(access, frame)?;
        owner::bone_transform(access, object.cast(), name, kind, result)
    }
}
