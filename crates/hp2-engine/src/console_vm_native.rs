//! Canonical console, platform, and miscellaneous script-native entry points.
use hp2_abi::{FVector, engine::generated::{FFrame, TArrayRaw, UGesture, UObject}};
use hp2_core::{array_storage, canonical_vm_native::{finish_arguments, step_argument, VmNativeSpec}, core_access::{CoreAccess, RuntimeError, RuntimeResult}, vm_string};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 27] = [
    VmNativeSpec { class_name: "Engine.Wind", function_name: "GetWind", native_index: Some(425), handler: get_wind },
    VmNativeSpec { class_name: "Engine.Gesture", function_name: "CompareGesture", native_index: Some(426), handler: compare_gesture },
    VmNativeSpec { class_name: "Engine.Gesture", function_name: "CompareGesturePoint", native_index: Some(427), handler: compare_gesture_point },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "ConsoleCommand", native_index: None, handler: actor_console_command },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "Error", native_index: Some(233), handler: actor_error },
    VmNativeSpec { class_name: "Engine.LevelInfo", function_name: "GetLocalURL", native_index: None, handler: local_url },
    VmNativeSpec { class_name: "Engine.LevelInfo", function_name: "GetAddressURL", native_index: None, handler: address_url },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "ClientTravel", native_index: None, handler: client_travel },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "GetPlayerNetworkAddress", native_index: None, handler: network_address },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "CopyToClipboard", native_index: None, handler: copy_clipboard },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "PasteFromClipboard", native_index: None, handler: paste_clipboard },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "ClientHearSound", native_index: None, handler: client_hear_sound },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "GetCurrentKeyState", native_index: Some(330), handler: current_key_state },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "AutonomousPhysics", native_index: Some(3971), handler: autonomous_physics },
    VmNativeSpec { class_name: "Engine.Console", function_name: "ConsoleCommand", native_index: None, handler: console_command },
    VmNativeSpec { class_name: "Engine.Console", function_name: "CreateNativeFont", native_index: None, handler: create_native_font },
    VmNativeSpec { class_name: "Engine.Console", function_name: "SaveTimeDemo", native_index: None, handler: save_time_demo },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "ConsoleCommand", native_index: None, handler: player_console_command },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "CheckValidSkinPackage", native_index: None, handler: check_valid_skin_package },
    VmNativeSpec { class_name: "Engine.PlayerPawn", function_name: "ScreenToWorld", native_index: Some(542), handler: screen_to_world },
    crate::scripted_texture_vm_native::NATIVE_FUNCTIONS[0],
    crate::scripted_texture_vm_native::NATIVE_FUNCTIONS[1],
    crate::scripted_texture_vm_native::NATIVE_FUNCTIONS[2],
    crate::scripted_texture_vm_native::NATIVE_FUNCTIONS[3],
    crate::scripted_texture_vm_native::NATIVE_FUNCTIONS[4],
    VmNativeSpec { class_name: "Engine.Actor", function_name: "MoveCacheEntry", native_index: None, handler: move_cache_entry },
    crate::trace_texture_vm_native::NATIVE_FUNCTIONS[0],
];

unsafe fn get_wind(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut location = FVector { x: 0., y: 0., z: 0. };
        step_argument(access, frame, (&raw mut location).cast())?;
        finish_arguments(access, frame)?;
        let value = crate::wind_owner::get_wind(access, object.cast(), &location, |level, end, start| {
            crate::collision_owner::model_fast_line_check((*level.as_ptr()).model.cast(), *end, *start)
        })?;
        result.cast::<FVector>().write(value);
        Ok(())
    }
}

unsafe fn vectors<'a>(array: TArrayRaw) -> &'a [FVector] {
    if array.data.is_null() || array.num <= 0 { &[] }
    else { unsafe { std::slice::from_raw_parts(array.data.cast(), array.num as usize) } }
}

unsafe fn compare_gesture_point(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut point = FVector { x: 0., y: 0., z: 0. };
        let mut accuracy = 0_f32;
        step_argument(access, frame, (&raw mut point).cast())?;
        step_argument(access, frame, (&raw mut accuracy).cast())?;
        finish_arguments(access, frame)?;
        let points = vectors(ptr::addr_of!((*object.cast::<UGesture>()).points).read_unaligned());
        result.cast::<f32>().write(crate::gesture::compare_gesture_point(points, point));
        Ok(())
    }
}

unsafe fn compare_gesture(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut mouse = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut accuracy = 0_f32;
        step_argument(access, frame, (&raw mut mouse).cast())?;
        step_argument(access, frame, (&raw mut accuracy).cast())?;
        finish_arguments(access, frame)?;
        let points = vectors(ptr::addr_of!((*object.cast::<UGesture>()).points).read_unaligned());
        result.cast::<f32>().write(crate::gesture::compare_gesture(points, vectors(mouse), accuracy));
        Ok(())
    })() };
    // FVector is POD; unlike FString this array has no element destructors.
    let release = unsafe { array_storage::release(vm_string::slots(&raw mut mouse), array_storage::system_allocator()) }
        .map_err(|_| RuntimeError::new("Unable to release Core VM gesture vector argument"));
    operation.and(release)
}

unsafe fn string_argument(
    access: CoreAccess<'_>, frame: *mut FFrame,
    operation: impl FnOnce(*const c_void) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    let mut value = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let result = unsafe { (|| {
        step_argument(access, frame, (&raw mut value).cast())?;
        finish_arguments(access, frame)?;
        operation(vm_string::view(&raw mut value).data)
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut value) };
    result.and(release)
}
unsafe fn actor_console_command(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { string_argument(access, frame, |text| crate::script_console_owner::console_command(access, object.cast(), text, result)) }
}
unsafe fn actor_error(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { string_argument(access, frame, |text| crate::script_console_owner::error(access, object.cast(), frame, text)) }
}
unsafe fn copy_clipboard(access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { string_argument(access, frame, |text| crate::script_console_owner::clipboard_copy(access, text)) }
}
unsafe fn paste_clipboard(access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; crate::script_console_owner::clipboard_paste(access, result) }
}
unsafe fn local_url(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; crate::script_console_owner::level_url(access, object.cast(), result, false) }
}
unsafe fn address_url(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; crate::script_console_owner::level_url(access, object.cast(), result, true) }
}
unsafe fn network_address(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; crate::script_console_owner::network_address(access, object.cast(), result) }
}
unsafe fn client_travel(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    let mut url = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let result = unsafe { (|| {
        let mut travel = 0_u8; let mut items = 0_i32;
        step_argument(access, frame, (&raw mut url).cast())?;
        step_argument(access, frame, (&raw mut travel).cast())?;
        step_argument(access, frame, (&raw mut items).cast())?;
        finish_arguments(access, frame)?;
        crate::script_console_owner::client_travel(access, object.cast(), vm_string::view(&raw mut url).data, items, travel)
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut url) };
    result.and(release)
}
unsafe fn client_hear_sound(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut actor: *mut c_void = ptr::null_mut(); let mut sound: *mut c_void = ptr::null_mut(); let mut id = 0_i32;
        let mut location = FVector { x: 0., y: 0., z: 0. }; let mut parameters = location;
        let mut disable = 0_i32; let mut looping = 0_i32;
        step_argument(access, frame, (&raw mut actor).cast())?; step_argument(access, frame, (&raw mut id).cast())?;
        step_argument(access, frame, (&raw mut sound).cast())?; step_argument(access, frame, (&raw mut location).cast())?;
        step_argument(access, frame, (&raw mut parameters).cast())?; step_argument(access, frame, (&raw mut disable).cast())?;
        step_argument(access, frame, (&raw mut looping).cast())?; finish_arguments(access, frame)?;
        crate::script_console_owner::client_hear_sound(access, object.cast(), actor, id, sound, location, parameters, disable, looping)
    }
}
unsafe fn current_key_state(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut key = 0_u8; step_argument(access, frame, (&raw mut key).cast())?; finish_arguments(access, frame)?;
        let value = crate::input::current_key_state_with(key, || -> RuntimeResult<*mut hp2_abi::engine::generated::UViewport> {
            let pawn = access.native_class("Engine.PlayerPawn")?;
            if !hp2_core::object_model::is_a(object.cast(), pawn.cast()) { return Ok(ptr::null_mut()); }
            let player = (*object.cast::<hp2_abi::engine::generated::APlayerPawn>()).player;
            let viewport = access.native_class("Engine.Viewport")?;
            Ok(if hp2_core::object_model::is_a(player, viewport.cast()) { player.cast() } else { ptr::null_mut() })
        })?;
        result.cast::<u32>().write(value as u32); Ok(())
    }
}
unsafe fn autonomous_physics(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut delta = 0_f32; step_argument(access, frame, (&raw mut delta).cast())?; finish_arguments(access, frame)?;
        crate::physic::exec_autonomous_physics_with(object.cast(), delta, |actor, delta| crate::actor_physics_owner::perform_physics(access, actor, delta))
    }
}

unsafe fn console_command(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { string_argument(access, frame, |text| {
        let value = crate::console_owner::exec_command(access, object.cast(), text.cast())?;
        result.cast::<i32>().write(i32::from(value)); Ok(())
    }) }
}
unsafe fn create_native_font(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    let mut name = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut height = 0_i32;
        step_argument(access, frame, (&raw mut name).cast())?; step_argument(access, frame, (&raw mut height).cast())?;
        finish_arguments(access, frame)?;
        let font = crate::console::create_native_font_with(object.cast(), vm_string::view(&raw mut name).data, height,
            |viewport, name, height| crate::platform_owner::create_native_font(access, viewport.cast(), name.cast(), height))?;
        result.cast::<*mut c_void>().write(font); Ok(())
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut name) };
    operation.and(release)
}
unsafe fn save_time_demo(access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    let mut value = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        step_argument(access, frame, (&raw mut value).cast())?; finish_arguments(access, frame)?;
        crate::console::save_time_demo_with((&raw const value).cast(), |string, _filename| {
            let text = hp2_core::native_text::fstring_units(string.cast());
            let mut filename = access.data_root().join("System");
            filename.push(crate::console::FPS_FILENAME);
            // Source execSaveTimeDemo discards appSaveStringToFile's success boolean.
            hp2_core::file_persistence::save_text_to_path(&filename, text);
            Ok(())
        })
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&raw mut value) };
    operation.and(release)
}

unsafe fn player_console_command(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { string_argument(access, frame, |text| {
        // This override initializes Result before either command target runs.
        access.write_vm_fstring_result(result.cast(), &[])?;
        crate::script_console_owner::player_console_command(access, object.cast(), text, result)
    }) }
}

unsafe fn two_strings(
    access: CoreAccess<'_>, frame: *mut FFrame,
    operation: impl FnOnce(&[u32], &[u32]) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    let mut first = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let mut second = first;
    let result = unsafe { (|| {
        step_argument(access, frame, (&raw mut first).cast())?;
        step_argument(access, frame, (&raw mut second).cast())?;
        finish_arguments(access, frame)?;
        operation(hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&raw const first)),
            hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&raw const second)))
    })() };
    let release_first = unsafe { access.release_vm_fstring_argument(&raw mut first) };
    let release_second = unsafe { access.release_vm_fstring_argument(&raw mut second) };
    result.and(release_first).and(release_second)
}
unsafe fn check_valid_skin_package(access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe { two_strings(access, frame, |package, mesh| {
        let value = crate::pawn_native_files_owner::check_valid_skin_package(access, package, mesh)?;
        result.cast::<i32>().write(i32::from(value)); Ok(())
    }) }
}

unsafe fn screen_to_world(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut screen = FVector { x: 0., y: 0., z: 0. };
        step_argument(access, frame, (&raw mut screen).cast())?; finish_arguments(access, frame)?;
        crate::pawn_native_files_owner::screen_to_world(access, object.cast(), screen, result.cast())
    }
}

unsafe fn move_cache_entry(
    access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { two_strings(access, frame, |guid, filename| {
        result.cast::<i32>().write(0);
        let moved = crate::pawn_native_files_owner::move_cache_entry(access, guid, filename)?;
        result.cast::<i32>().write(i32::from(moved));
        Ok(())
    }) }
}
