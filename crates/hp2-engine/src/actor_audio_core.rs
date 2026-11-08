//! Canonical Core implementation of AActor sound propagation and commands.

use crate::{
    actor_audio::{self, Hp2ActorAudioInteger as Integer, Hp2ActorAudioObject as Object, Hp2ActorAudioServices, Hp2ActorAudioSound, Hp2ActorAudioVector},
    audio_owner,
};
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, APlayerPawn, FFrame, TArrayRaw, UClient,
    UEngine, UField, UFunction, ULevel, UStruct, UViewport,
};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, ptr};

struct Calls<'a> {
    access: CoreAccess<'a>,
    error: Option<RuntimeError>,
}

fn failed(calls: &mut Calls<'_>, result: RuntimeResult<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            calls.error.get_or_insert(error);
            1
        }
    }
}

fn invalid(calls: &mut Calls<'_>, message: &'static str) -> i32 {
    failed(calls, Err(RuntimeError::new(message)))
}

unsafe fn get_calls<'a>(opaque: *mut c_void) -> Option<&'a mut Calls<'a>> {
    unsafe { opaque.cast::<Calls<'a>>().as_mut() }
}

unsafe fn is_a(access: CoreAccess<'_>, object: *mut c_void, class: &str) -> RuntimeResult<bool> {
    if object.is_null() {
        return Ok(false);
    }
    let class = access.native_class(class)?;
    Ok(unsafe { hp2_core::object_model::is_a(object.cast(), class.cast()) })
}

unsafe fn field_function(
    access: CoreAccess<'_>,
    object: *mut hp2_abi::engine::generated::UObject,
    name: &str,
) -> RuntimeResult<*mut UFunction> {
    if object.is_null() {
        return Err(RuntimeError::new("Null actor audio event receiver"));
    }
    let wanted = access.intern_str(name, hp2_core::name_owner::FindName::Find)?;
    let mut class = unsafe { (*object).class.cast::<UStruct>() };
    while !class.is_null() {
        let mut field = unsafe { (*class).children.cast::<UField>() };
        while !field.is_null() {
            if i32::from_ne_bytes(unsafe { (*field.cast::<hp2_abi::engine::generated::UObject>()).name }) == wanted {
                return Ok(field.cast());
            }
            field = unsafe { (*field).next.cast() };
        }
        class = unsafe { (*class.cast::<UField>()).super_field.cast() };
    }
    Err(RuntimeError::new("Actor audio event function was not found").with_context(name))
}

unsafe extern "C" fn object_field(
    context: *mut c_void,
    subject: *mut c_void,
    selector: Object,
    out: *mut *mut c_void,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if subject.is_null() || out.is_null() {
        return invalid(calls, "Null actor audio object field");
    }
    let actor = subject.cast::<AActor>();
    let result: RuntimeResult<*mut c_void> = unsafe { (|| {
        match selector {
            Object::AudioLevel => Ok((*actor).x_level),
            Object::AudioDemoDriver => Ok((*subject.cast::<ULevel>()).base.demo_rec_driver),
            Object::AudioServerConnection => Ok((*subject.cast::<hp2_abi::engine::generated::UNetDriver>()).server_connection),
            Object::AudioClient => {
                let engine = (*subject.cast::<ULevel>()).base.engine.cast::<UEngine>();
                Ok(if engine.is_null() { ptr::null_mut() } else { (*engine).client })
            }
            Object::AudioViewTarget => Ok((*subject.cast::<APlayerPawn>()).view_target),
            Object::AudioInstigator => Ok((*actor).instigator),
            Object::AudioOwner => Ok((*actor).owner),
            Object::AudioPlayer => {
                if is_a(calls.access, subject, "Engine.PlayerPawn")? {
                    Ok((*subject.cast::<APlayerPawn>()).player)
                } else {
                    Ok(ptr::null_mut())
                }
            }
            Object::AudioPawnList => {
                let level = (*actor).level.cast::<ALevelInfo>();
                Ok(if level.is_null() { ptr::null_mut() } else { (*level).pawn_list })
            }
            Object::AudioNextPawn => Ok((*subject.cast::<APawn>()).next_pawn),
            Object::AudioSubsystem => {
                let engine = (*subject.cast::<ULevel>()).base.engine.cast::<UEngine>();
                Ok(if engine.is_null() { ptr::null_mut() } else { (*engine).audio })
            }
        }
    })() };
    match result {
        Ok(value) => { unsafe { out.write(value) }; 0 }
        Err(error) => failed(calls, Err(error)),
    }
}

unsafe extern "C" fn integer_field(
    context: *mut c_void,
    subject: *mut c_void,
    selector: Integer,
    out: *mut i32,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if subject.is_null() || out.is_null() {
        return invalid(calls, "Null actor audio integer field");
    }
    let result: RuntimeResult<i32> = unsafe { (|| {
        match selector {
            Integer::AudioNetMode => {
                let level = subject.cast::<ULevel>();
                if level.is_null() {
                    Err(RuntimeError::new("Null actor audio level"))
                } else {
                    let actors = (*level).base.actors;
                    if actors.array_num <= 0 || actors.data.is_null() {
                        Err(RuntimeError::new("Actor audio level has no LevelInfo"))
                    } else {
                        let info = *actors.data.cast::<*mut AActor>();
                        if info.is_null() { Err(RuntimeError::new("Actor audio LevelInfo is null")) }
                        else { Ok(i32::from((*info.cast::<ALevelInfo>()).net_mode)) }
                    }
                }
            }
            Integer::AudioActorIndex => Ok((*subject.cast::<AActor>()).index),
            Integer::AudioIsPlayerPawn => Ok(i32::from(is_a(calls.access, subject, "Engine.PlayerPawn")?)),
            Integer::AudioIsViewport => Ok(i32::from(is_a(calls.access, subject, "Engine.Viewport")?)),
            Integer::AudioIsPlayer => Ok(i32::from((*subject.cast::<APawn>()).b_is_player())),
            Integer::AudioViewportCount => {
                let array = (*subject.cast::<UClient>()).viewports;
                if array.num < 0 || (array.num > 0 && array.data.is_null()) {
                    Err(RuntimeError::new("Invalid client viewport array"))
                } else { Ok(array.num) }
            }
            Integer::AudioCallerSimulated => {
                let node = (*subject.cast::<FFrame>()).node.cast::<UFunction>();
                Ok(if node.is_null() { 0 } else { i32::from((*node).function_flags & 0x0000_0100 != 0) })
            }
        }
    })() };
    match result {
        Ok(value) => { unsafe { out.write(value) }; 0 }
        Err(error) => failed(calls, Err(error)),
    }
}

unsafe extern "C" fn location(
    context: *mut c_void,
    subject: *mut c_void,
    out: *mut Hp2ActorAudioVector,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if subject.is_null() || out.is_null() { return invalid(calls, "Null actor audio location"); }
    let p = unsafe { (*subject.cast::<AActor>()).location };
    unsafe { out.write(Hp2ActorAudioVector { x: p.x, y: p.y, z: p.z }) };
    0
}

unsafe extern "C" fn viewport_actor(
    context: *mut c_void,
    client: *mut c_void,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if client.is_null() || out.is_null() { return invalid(calls, "Null actor audio viewport query"); }
    let array = unsafe { (*client.cast::<UClient>()).viewports };
    if index < 0 || index >= array.num || array.data.is_null() {
        return invalid(calls, "Invalid actor audio viewport index");
    }
    let viewport = unsafe { *array.data.cast::<*mut UViewport>().add(index as usize) };
    unsafe { out.write(if viewport.is_null() { ptr::null_mut() } else { (*viewport).actor }) };
    0
}

unsafe extern "C" fn line_check(
    context: *mut c_void,
    level: *mut c_void,
    start: *const Hp2ActorAudioVector,
    end: *const Hp2ActorAudioVector,
    out: *mut i32,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if level.is_null() || start.is_null() || end.is_null() || out.is_null() {
        return invalid(calls, "Invalid actor audio line check");
    }
    let start = unsafe { *start };
    let end = unsafe { *end };
    let result = unsafe {
        crate::collision_owner::model_fast_line_check(
            (*level.cast::<ULevel>()).model.cast(),
            FVector { x: start.x, y: start.y, z: start.z },
            FVector { x: end.x, y: end.y, z: end.z },
        )
    };
    match result {
        Ok(clear) => { unsafe { out.write(i32::from(clear)) }; 0 }
        Err(error) => failed(calls, Err(error)),
    }
}

fn event_params(_access: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<Vec<u128>> {
    if function.is_null() { return Err(RuntimeError::new("Null actor audio event function")); }
    let size = unsafe { ptr::addr_of!((*function).parms_size).read_unaligned() };
    if size < 0 { return Err(RuntimeError::new("Invalid actor audio event parameter size")); }
    let words = (size as usize + std::mem::size_of::<u128>() - 1) / std::mem::size_of::<u128>();
    Ok(vec![0; words])
}

fn event_property(access: CoreAccess<'_>, function: *mut UFunction, name: &str, expected_class: &str) -> RuntimeResult<(*mut hp2_abi::engine::generated::UProperty, usize)> {
    let property_class = access.native_class("Core.Property")?;
    let wanted = access.intern_str(name, hp2_core::name_owner::FindName::Find)?;
    let expected = access.native_class(expected_class)?;
    let mut field = unsafe { (*function.cast::<UStruct>()).children.cast::<UField>() };
    while !field.is_null() {
        let value = field.cast::<hp2_abi::engine::generated::UObject>();
        let actual_name = i32::from_ne_bytes(unsafe { ptr::addr_of!((*value).name).read_unaligned() });
        let actual_class = unsafe { ptr::addr_of!((*value).class).read_unaligned() };
        if actual_name == wanted && unsafe { hp2_core::object_model::is_a(value.cast(), property_class.cast()) } {
            if !unsafe { hp2_core::object_model::is_child_of(actual_class.cast(), expected.cast()) } {
                return Err(RuntimeError::new("Unexpected actor audio event parameter type").with_context(name));
            }
            let property = field.cast::<hp2_abi::engine::generated::UProperty>();
            let offset = unsafe { ptr::addr_of!((*property).offset).read_unaligned() };
            if offset < 0 { return Err(RuntimeError::new("Negative actor audio event parameter offset").with_context(name)); }
            return Ok((property, offset as usize));
        }
        field = unsafe { ptr::addr_of!((*field).next).read_unaligned().cast() };
    }
    Err(RuntimeError::new("Actor audio event parameter was not found").with_context(name))
}

fn write_event_value<T: Copy>(access: CoreAccess<'_>, function: *mut UFunction, params: &mut [u128], name: &str, class: &str, value: T) -> RuntimeResult<()> {
    let (property, offset) = event_property(access, function, name, class)?;
    let element_size = unsafe { ptr::addr_of!((*property).element_size).read_unaligned() };
    let size = std::mem::size_of::<T>();
    let parm_size = unsafe { ptr::addr_of!((*function).parms_size).read_unaligned() };
    if element_size < 0 || (element_size as usize) < size || parm_size < 0 || offset.checked_add(size).is_none_or(|end| end > parm_size as usize) {
        return Err(RuntimeError::new("Actor audio event parameter does not fit reflected storage").with_context(name));
    }
    let base = params.as_mut_ptr().cast::<u8>();
    unsafe { ptr::copy_nonoverlapping((&value as *const T).cast::<u8>(), base.add(offset), size); }
    Ok(())
}

fn write_event_bool(access: CoreAccess<'_>, function: *mut UFunction, params: &mut [u128], name: &str, value: bool) -> RuntimeResult<()> {
    let (property, offset) = event_property(access, function, name, "Core.BoolProperty")?;
    let parm_size = unsafe { ptr::addr_of!((*function).parms_size).read_unaligned() };
    if parm_size < 0 || offset.checked_add(std::mem::size_of::<u32>()).is_none_or(|end| end > parm_size as usize) {
        return Err(RuntimeError::new("Actor audio event boolean exceeds reflected storage").with_context(name));
    }
    let mask = unsafe { ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask).read_unaligned() };
    if mask == 0 { return Err(RuntimeError::new("Actor audio event boolean has an empty reflected mask").with_context(name)); }
    let base = params.as_mut_ptr().cast::<u8>();
    let storage = unsafe { base.add(offset).cast::<u32>() };
    let old = unsafe { storage.read_unaligned() };
    unsafe { storage.write_unaligned(if value { old | mask } else { old & !mask }); }
    Ok(())
}


unsafe extern "C" fn hear(
    context: *mut c_void,
    hearer: *mut c_void,
    actor: *mut c_void,
    id: i32,
    sound: *mut c_void,
    location: *const Hp2ActorAudioVector,
    parameters: *const Hp2ActorAudioVector,
    disable_3d: i32,
    looping: i32,
) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if hearer.is_null() || actor.is_null() || location.is_null() || parameters.is_null() {
        return invalid(calls, "Null actor audio event argument");
    }
    let location = unsafe { *location };
    let parameters = unsafe { *parameters };
    let result = (|| {
        let function = unsafe { field_function(calls.access, hearer.cast(), "ClientHearSound")? };
        let mut params = event_params(calls.access, function)?;
        write_event_value(calls.access, function, &mut params, "Actor", "Core.ObjectProperty", actor)?;
        write_event_value(calls.access, function, &mut params, "Id", "Core.IntProperty", id)?;
        write_event_value(calls.access, function, &mut params, "S", "Core.ObjectProperty", sound)?;
        write_event_value(calls.access, function, &mut params, "SoundLocation", "Core.StructProperty", FVector { x: location.x, y: location.y, z: location.z })?;
        write_event_value(calls.access, function, &mut params, "Parameters", "Core.StructProperty", FVector { x: parameters.x, y: parameters.y, z: parameters.z })?;
        write_event_bool(calls.access, function, &mut params, "Disable3D", disable_3d != 0)?;
        write_event_bool(calls.access, function, &mut params, "Loop", looping != 0)?;
        unsafe { crate::actor_owner::process_event(calls.access, hearer.cast::<AActor>(), function, params.as_mut_ptr().cast(), ptr::null_mut()) }
    })();
    failed(calls, result)
}

unsafe extern "C" fn demo(context: *mut c_void, actor: *mut c_void, args: *const Hp2ActorAudioSound) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if actor.is_null() || args.is_null() { return invalid(calls, "Null demo sound argument"); }
    let a = unsafe { *args };
    let result = (|| {
        let function = unsafe { field_function(calls.access, actor.cast(), "DemoPlaySound")? };
        let mut params = event_params(calls.access, function)?;
        write_event_value(calls.access, function, &mut params, "Sound", "Core.ObjectProperty", a.sound)?;
        write_event_value(calls.access, function, &mut params, "Slot", "Core.ByteProperty", a.slot)?;
        write_event_value(calls.access, function, &mut params, "Volume", "Core.FloatProperty", a.volume)?;
        write_event_bool(calls.access, function, &mut params, "bNoOverride", a.no_override != 0)?;
        write_event_value(calls.access, function, &mut params, "Radius", "Core.FloatProperty", a.radius)?;
        write_event_value(calls.access, function, &mut params, "Pitch", "Core.FloatProperty", a.pitch)?;
        write_event_bool(calls.access, function, &mut params, "Disable3D", a.disable_3d != 0)?;
        write_event_bool(calls.access, function, &mut params, "Loop", a.looping != 0)?;
        unsafe { crate::actor_owner::process_event(calls.access, actor.cast::<AActor>(), function, params.as_mut_ptr().cast(), ptr::null_mut()) }
    })();
    failed(calls, result)
}

unsafe extern "C" fn play_music(context: *mut c_void, audio: *mut c_void, song: *mut c_void, fade: f32, out: *mut i32) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if audio.is_null() || song.is_null() || out.is_null() { return invalid(calls, "Invalid actor PlayMusic argument"); }
    let song = unsafe { &*song.cast::<TArrayRaw>() };
    if song.num < 0 || (song.num > 0 && song.data.is_null()) { return invalid(calls, "Invalid actor PlayMusic string"); }
    let text = if song.num == 0 {
        String::new()
    } else {
        let units = unsafe { std::slice::from_raw_parts(song.data.cast::<hp2_core::ansi::Tchar>(), song.num as usize) };
        let units = hp2_core::native_text::code_units(units);
        match units.iter().copied().take_while(|unit| *unit != 0).map(char::from_u32).collect::<Option<String>>() {
            Some(text) => text,
            None => return invalid(calls, "Actor PlayMusic string contains an invalid code point"),
        }
    };
    match audio_owner::play_music(calls.access, audio.cast(), &text, fade) {
        Ok(value) => { unsafe { out.write(value) }; 0 }
        Err(error) => failed(calls, Err(error)),
    }
}

unsafe extern "C" fn stop_music(context: *mut c_void, audio: *mut c_void, handle: i32, fade: f32) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    match audio_owner::stop_music(calls.access, audio.cast(), handle, fade) { Ok(_) => 0, Err(error) => failed(calls, Err(error)) }
}

unsafe extern "C" fn stop_all_music(context: *mut c_void, audio: *mut c_void, fade: f32) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    match audio_owner::stop_all_music(calls.access, audio.cast(), fade) { Ok(()) => 0, Err(error) => failed(calls, Err(error)) }
}

unsafe extern "C" fn modify_sound(context: *mut c_void, audio: *mut c_void, actor: *mut c_void, id: i32, sound: *mut c_void, parameter: u8, value: f32, out: *mut i32) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    if out.is_null() { return invalid(calls, "Null ModifySound result"); }
    match audio_owner::modify_sound(calls.access, audio.cast(), actor as usize, id, sound as usize, parameter, value) {
        Ok(value) => { unsafe { out.write(i32::from(value)) }; 0 }
        Err(error) => failed(calls, Err(error)),
    }
}

unsafe extern "C" fn stop_sound(context: *mut c_void, audio: *mut c_void, actor: *mut c_void, id: i32, sound: *mut c_void, fade: f32) -> i32 {
    let Some(calls) = (unsafe { get_calls(context) }) else { return 1; };
    match audio_owner::stop_sound_by_id(calls.access, audio.cast(), actor as usize, id, sound as usize, fade) {
        Ok(_) => 0,
        Err(error) => failed(calls, Err(error)),
    }
}

fn services(calls: &mut Calls<'_>) -> Hp2ActorAudioServices {
    Hp2ActorAudioServices {
        context: (calls as *mut Calls<'_>).cast(), object: object_field, integer: integer_field,
        location, viewport_actor, line_check, hear, demo, play_music, stop_music,
        stop_all_music, modify_sound, stop_sound,
    }
}

fn policy_result<T>(calls: Calls<'_>, policy: Result<T, i32>) -> RuntimeResult<T> {
    match policy {
        Ok(value) => Ok(value),
        Err(_) => Err(calls.error.unwrap_or_else(|| RuntimeError::new("Actor audio operation failed"))),
    }
}

/// Executes canonical AActor PlaySound, PlayOwnedSound, or DemoPlaySound propagation.
///
/// # Safety
/// `actor`, `caller`, and `args.sound` are live canonical records during synchronous callbacks.
pub(crate) unsafe fn play_sound(
    access: CoreAccess<'_>, actor: *mut AActor, caller: *mut FFrame, kind: u32,
    args: &Hp2ActorAudioSound,
) -> RuntimeResult<()> {
    let mut calls = Calls { access, error: None };
    let services = services(&mut calls);
    let policy = unsafe { actor_audio::play_sound(actor.cast(), caller.cast(), kind, args, &services) };
    policy_result(calls, policy)
}

/// Executes an AActor music/sound command through canonical Core audio records.
///
/// # Safety
/// `actor`, command object, and output storage (when required) remain live during this call.
pub(crate) unsafe fn audio_command(
    access: CoreAccess<'_>, actor: *mut AActor, command: u32, object: *mut c_void,
    slot: u8, parameter: u8, handle: i32, value: f32, out: *mut i32,
) -> RuntimeResult<()> {
    let mut calls = Calls { access, error: None };
    let services = services(&mut calls);
    let policy = unsafe { actor_audio::audio_command(actor.cast(), command, object, slot, parameter, handle, value, out, &services) };
    policy_result(calls, policy)
}
