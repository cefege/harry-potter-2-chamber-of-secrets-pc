//! Direct canonical VM handlers for Actor sound and music natives.

use hp2_abi::engine::generated::{AActor, FFrame, TArrayRaw, UObject};
use hp2_core::{
    canonical_vm_native::{finish_arguments, step_argument, VmNativeSpec},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
};
use std::ffi::c_void;

/// Immutable canonical Actor audio registrations for installation by the Engine native registry.
pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 8] = [
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PlaySound", native_index: Some(264), handler: play_sound },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PlayOwnedSound", native_index: None, handler: play_owned_sound },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "DemoPlaySound", native_index: None, handler: demo_play_sound },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PlayMusic", native_index: None, handler: play_music },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "StopMusic", native_index: None, handler: stop_music },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "StopAllMusic", native_index: None, handler: stop_all_music },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "ModifySound", native_index: Some(567), handler: modify_sound },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "StopSound", native_index: Some(568), handler: stop_sound },
];

unsafe fn play(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, kind: u32) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null actor audio receiver")); }
    // EngineClasses.h: TransientSoundVolume/Radius/Pitch occupy AActor's
    // opaque 652..664 ABI region, as in the legacy script handler.
    let defaults = unsafe { object.cast::<u8>().add(652).cast::<f32>() };
    let mut args = crate::actor_audio::Hp2ActorAudioSound {
        sound: std::ptr::null_mut(), slot: 1,
        volume: unsafe { defaults.read_unaligned() }, no_override: 0,
        radius: unsafe { defaults.add(1).read_unaligned() },
        pitch: unsafe { defaults.add(2).read_unaligned() } / 64.0, disable_3d: 0, looping: 0,
    };
    unsafe {
        step_argument(access, frame, (&raw mut args.sound).cast())?;
        step_argument(access, frame, (&raw mut args.slot).cast())?;
        step_argument(access, frame, (&raw mut args.volume).cast())?;
        step_argument(access, frame, (&raw mut args.no_override).cast())?;
        step_argument(access, frame, (&raw mut args.radius).cast())?;
        step_argument(access, frame, (&raw mut args.pitch).cast())?;
        step_argument(access, frame, (&raw mut args.disable_3d).cast())?;
        step_argument(access, frame, (&raw mut args.looping).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::play_sound(access, object.cast::<AActor>(), frame, kind, &args)
    }
}

unsafe fn play_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { play(access, object, frame, 1) }
}
unsafe fn play_owned_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { play(access, object, frame, 2) }
}
unsafe fn demo_play_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { play(access, object, frame, 0) }
}

unsafe fn play_music(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut fade = 0.0_f32;
    let mut song = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        step_argument(access, frame, (&raw mut song).cast())?;
        step_argument(access, frame, (&raw mut fade).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::audio_command(access, object.cast(), 0, (&raw mut song).cast(), 0, 0, 0, fade, result.cast())
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&mut song) };
    operation.and(release)
}
unsafe fn stop_music(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    let mut fade = 0.0_f32;
    let mut handle = 0_i32;
    unsafe {
        step_argument(access, frame, (&raw mut handle).cast())?;
        step_argument(access, frame, (&raw mut fade).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::audio_command(access, object.cast(), 1, std::ptr::null_mut(), 0, 0, handle, fade, std::ptr::null_mut())
    }
}
unsafe fn stop_all_music(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    let mut fade = 0.0_f32;
    unsafe {
        step_argument(access, frame, (&raw mut fade).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::audio_command(access, object.cast(), 2, std::ptr::null_mut(), 0, 0, 0, fade, std::ptr::null_mut())
    }
}
unsafe fn modify_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut slot = 1_u8;
    let mut sound = std::ptr::null_mut::<c_void>();
    let mut value = 0.0_f32;
    let mut parameter = 0_u8;
    unsafe {
        step_argument(access, frame, (&raw mut parameter).cast())?;
        step_argument(access, frame, (&raw mut value).cast())?;
        step_argument(access, frame, (&raw mut sound).cast())?;
        step_argument(access, frame, (&raw mut slot).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::audio_command(access, object.cast(), 3, sound, slot, parameter, 0, value, result.cast())
    }
}
unsafe fn stop_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    let mut fade = 0.0_f32;
    let mut slot = 1_u8;
    let mut sound = std::ptr::null_mut::<c_void>();
    unsafe {
        step_argument(access, frame, (&raw mut sound).cast())?;
        step_argument(access, frame, (&raw mut slot).cast())?;
        step_argument(access, frame, (&raw mut fade).cast())?;
        finish_arguments(access, frame)?;
        crate::actor_audio_core::audio_command(access, object.cast(), 4, sound, slot, 0, 0, fade, std::ptr::null_mut())
    }
}
