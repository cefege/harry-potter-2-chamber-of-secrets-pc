//! Canonical script path/search natives; policy stays in the shared route/pawn modules.
use hp2_abi::{FVector, engine::generated::{AActor, APawn, FFrame, UObject}};
use hp2_core::{canonical_vm_native::{finish_arguments, step_argument, step_argument_out, VmNativeSpec}, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 9] = [
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "actorReachable", native_index: Some(520), handler: actor_reachable },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindPathTo", native_index: Some(518), handler: find_path_to },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindPathToward", native_index: Some(517), handler: find_path_toward },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindRandomDest", native_index: Some(525), handler: find_random_dest },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindPath", native_index: Some(553), handler: find_path },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "ClearPaths", native_index: Some(522), handler: clear_paths },
    VmNativeSpec { class_name: "Engine.Pawn", function_name: "FindBestInventoryPath", native_index: Some(540), handler: inventory_path },
    VmNativeSpec { class_name: "Engine.NavigationPoint", function_name: "describeSpec", native_index: Some(519), handler: describe_spec },
    VmNativeSpec { class_name: "Engine.Actor", function_name: "PlayerCanSeeMe", native_index: Some(532), handler: player_can_see_me },
];
unsafe fn timed<T>(pawn: *mut UObject, body: impl FnOnce() -> RuntimeResult<T>) -> RuntimeResult<T> {
    unsafe { crate::pawn_native_move::clock_find_path_begin(pawn.cast()) }.map_err(|_| RuntimeError::new("Canonical path clock failed"))?;
    let value = body()?;
    unsafe { crate::pawn_native_move::clock_find_path_end(pawn.cast()) }.map_err(|_| RuntimeError::new("Canonical path unclock failed"))?;
    Ok(value)
}
unsafe fn actor_reachable(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut target: *mut AActor = ptr::null_mut();
        step_argument(access, frame, (&raw mut target).cast())?;
        finish_arguments(access, frame)?;
        let value = if target.is_null() { 0 } else { timed(object, || crate::pawn_native_move_owner::actor_reachable(access, object.cast(), target))? };
        result.cast::<u32>().write_unaligned(value);
        Ok(())
    }
}
unsafe fn find_path_to(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut point = FVector { x: 0., y: 0., z: 0. }; let mut single = 0_i32; let mut clear = 1_i32;
        step_argument(access, frame, (&raw mut point).cast())?;
        step_argument(access, frame, (&raw mut single).cast())?;
        step_argument(access, frame, (&raw mut clear).cast())?;
        finish_arguments(access, frame)?;
        let value = timed(object, || crate::pawn_native_move_owner::find_path_to(access, object.cast(), point, single, clear))?;
        result.cast::<*mut c_void>().write_unaligned(value); Ok(())
    }
}
unsafe fn find_path_toward(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut goal: *mut AActor = ptr::null_mut(); let mut single = 0_i32; let mut clear = 1_i32;
        step_argument(access, frame, (&raw mut goal).cast())?;
        step_argument(access, frame, (&raw mut single).cast())?;
        step_argument(access, frame, (&raw mut clear).cast())?;
        finish_arguments(access, frame)?;
        let value = if goal.is_null() { ptr::null_mut() } else { timed(object, || crate::pawn_native_move_owner::find_path_toward(access, object.cast(), goal, single, clear))? };
        result.cast::<*mut c_void>().write_unaligned(value); Ok(())
    }
}
unsafe fn find_random_dest(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut clear = 1_i32; step_argument(access, frame, (&raw mut clear).cast())?; finish_arguments(access, frame)?;
        let value = timed(object, || crate::pawn_native_move_owner::find_random_dest(access, object.cast(), clear))?;
        result.cast::<*mut c_void>().write_unaligned(value); Ok(())
    }
}
unsafe fn find_path(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut start: *mut AActor = ptr::null_mut(); let mut class = 0_i32;
        step_argument(access, frame, (&raw mut start).cast())?; step_argument(access, frame, (&raw mut class).cast())?;
        finish_arguments(access, frame)?;
        let value = crate::pawn_native_move_owner::find_path(access, object.cast(), start, class)?;
        result.cast::<*mut c_void>().write_unaligned(value); Ok(())
    }
}
unsafe fn clear_paths(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe { finish_arguments(access, frame)?; timed(object, || crate::route_owner::clear_paths(access, object.cast())) }
}
unsafe fn inventory_path(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut weight = 0_f32; let mut predict = 0_i32;
        let weight_out = step_argument_out(access, frame, (&raw mut weight).cast())?.cast::<f32>();
        step_argument(access, frame, (&raw mut predict).cast())?; finish_arguments(access, frame)?;
        let initial = weight_out.read_unaligned();
        let value = timed(object, || crate::pawn_native_move_owner::find_best_inventory_path(access, object.cast::<APawn>(), initial, predict != 0, weight_out))?;
        result.cast::<*mut c_void>().write_unaligned(value); Ok(())
    }
}
unsafe fn describe_spec(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let mut index = 0_i32; let mut start: *mut c_void = ptr::null_mut(); let mut end: *mut c_void = ptr::null_mut(); let mut flags = 0_i32; let mut distance = 0_i32;
        step_argument(access, frame, (&raw mut index).cast())?;
        let start_out = step_argument_out(access, frame, (&raw mut start).cast())?.cast::<*mut c_void>();
        let end_out = step_argument_out(access, frame, (&raw mut end).cast())?.cast::<*mut c_void>();
        let flags_out = step_argument_out(access, frame, (&raw mut flags).cast())?.cast::<i32>();
        let distance_out = step_argument_out(access, frame, (&raw mut distance).cast())?.cast::<i32>();
        finish_arguments(access, frame)?;
        let values = crate::pawn_native_ai_owner::describe_spec(access, (*object.cast::<AActor>()).x_level, index)?;
        start_out.write_unaligned(values.0); end_out.write_unaligned(values.1); flags_out.write_unaligned(values.2); distance_out.write_unaligned(values.3); Ok(())
    }
}
unsafe fn player_can_see_me(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        finish_arguments(access, frame)?;
        let value = crate::pawn_native_ai_owner::player_can_see_me(access, object.cast())?;
        result.cast::<i32>().write_unaligned(i32::from(value)); Ok(())
    }
}
