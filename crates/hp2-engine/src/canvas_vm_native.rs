//! Canonical VM bindings for native Canvas operations.

use hp2_abi::engine::generated::{
    AActor, FFrame, FRotator, FSceneNode, TArrayRaw, UCanvas, UObject, URender,
    UTexture, UViewport,
};
use hp2_abi::FVector;
use hp2_core::{
    canonical_vm_native::{finish_arguments, log_frame, step_argument, step_argument_out, VmNativeSpec},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    NativeTileScope,
};
use std::{ffi::c_void, ptr};
/// Immutable native UCanvas registrations installed by the parent Core registry.
pub(crate) static NATIVE_FUNCTIONS: &[VmNativeSpec] = &[
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "StrLen", native_index: Some(464), handler: str_len },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawText", native_index: Some(465), handler: draw_text },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawTile", native_index: Some(466), handler: draw_tile },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawActor", native_index: Some(467), handler: draw_actor },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawTileClipped", native_index: Some(468), handler: draw_tile_clipped },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawTextClipped", native_index: Some(469), handler: draw_text_clipped },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "TextSize", native_index: Some(470), handler: text_size },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawClippedActor", native_index: Some(471), handler: draw_clipped_actor },
    VmNativeSpec { class_name: "Engine.Canvas", function_name: "DrawPortal", native_index: Some(480), handler: draw_portal },
];

unsafe fn canvas_text_arg(access: CoreAccess<'_>, frame: *mut FFrame, text: &mut TArrayRaw, maximum: usize) -> RuntimeResult<Vec<u16>> {
    unsafe { step_argument(access, frame, (text as *mut TArrayRaw).cast())?; }
    let source = unsafe { hp2_core::native_text::fstring_units(text as *const TArrayRaw) };
    let length = source.len().min(maximum);
    let mut units = Vec::with_capacity(length * 2 + 1);
    for &unit in source.iter().take(length) {
        let scalar = char::from_u32(unit as u32).unwrap_or(char::REPLACEMENT_CHARACTER);
        let mut encoded = [0; 2];
        units.extend_from_slice(scalar.encode_utf16(&mut encoded));
    }
    units.push(0);
    Ok(units)
}
unsafe fn step_optional<T: Copy>(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    value: &mut T,
) -> RuntimeResult<()> {
    let code = unsafe { ptr::addr_of!((*frame).code).read_unaligned() };
    if !code.is_null() && unsafe { code.read() } as i32 != hp2_abi::engine::generated::EX_EndFunctionParms {
        unsafe { step_argument(access, frame, (value as *mut T).cast())?; }
    }
    Ok(())
}

unsafe fn compute_canvas_frame_size(access: CoreAccess<'_>, frame: *mut FSceneNode) -> RuntimeResult<()> {
    unsafe {
        crate::camera::compute_render_size_with(frame, |viewport, frame| {
            let device = ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast();
            crate::render_device_owner::set_scene_node(access, device, frame)
                .map_err(|_| -1)
        })
        .map_err(|_| RuntimeError::new("Canvas scene frame sizing failed"))
    }
}

unsafe fn compute_canvas_frame_coords(
    access: CoreAccess<'_>,
    frame: *mut FSceneNode,
    location: FVector,
    rotation: FRotator,
) -> RuntimeResult<()> {
    unsafe {
        crate::camera::compute_render_coords_with(frame, location, rotation, |frame| {
            compute_canvas_frame_size(access, frame).map_err(|_| -1)
        })
        .map_err(|_| RuntimeError::new("Canvas scene frame coordinate computation failed"))
    }
}
unsafe fn str_len(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let mut text = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut xl = 0.0_f32;
        let mut yl = 0.0_f32;
        let units = canvas_text_arg(access, frame, &mut text, 4095)?;
        let out_x = step_argument_out(access, frame, (&raw mut xl).cast())?;
        let out_y = step_argument_out(access, frame, (&raw mut yl).cast())?;
        finish_arguments(access, frame)?;
        let (width, height) = crate::canvas_owner::native_str_len(access, object.cast(), &units)?;
        out_x.cast::<f32>().write_unaligned(width as f32);
        out_y.cast::<f32>().write_unaligned(height as f32);
        Ok(())
    })() };
    let released = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(released)
}

unsafe fn draw_text(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let mut text = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let units = canvas_text_arg(access, frame, &mut text, usize::MAX)?;
        let mut cr = 1_u32;
        let mut num_chars = 0_i32;
        let mut cursor = 0_u32;
        step_optional(access, frame, &mut cr)?;
        step_optional(access, frame, &mut num_chars)?;
        step_optional(access, frame, &mut cursor)?;
        finish_arguments(access, frame)?;
        // Cursor is part of the source ABI even though this native ignores it.
        crate::canvas_owner::draw_text(access, object.cast(), &units, cr != 0, num_chars)
    })() };
    let released = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(released)
}

unsafe fn draw_tile(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    unsafe { tile(access, object, frame, false, None) }
}

unsafe fn draw_tile_clipped(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    unsafe { tile(access, object, frame, true, None) }
}

/// Runs the ordinary DrawTile owner under a current Core lexical handoff.
///
/// # Safety
/// `access`, `object`, `frame`, and `scope` must identify the same live, current
/// selected native invocation. The scope must authorize this runner's seven
/// ordered, synchronously writable destinations and its one finish operation.
/// Neither the scope nor a destination may escape or operate a suspended frame.
pub(crate) unsafe fn health_draw_tile_selected(
    access: CoreAccess<'_>,
    object: *mut UObject,
    frame: *mut FFrame,
    _result: *mut c_void,
    scope: &NativeTileScope<'_>,
) -> RuntimeResult<()> {
    // SAFETY: the caller supplies the matching live invocation and scope.
    unsafe { tile(access, object, frame, false, Some(scope)) }
}

unsafe fn tile_texture_input(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    destination: *mut *mut UTexture,
    selected: Option<&NativeTileScope<'_>>,
) -> RuntimeResult<()> {
    // SAFETY: the runner supplies its current, writable texture destination.
    unsafe {
        match selected {
            Some(scope) => scope.step_texture(destination),
            None => step_argument(access, frame, destination.cast()),
        }
    }
}

unsafe fn tile_float_input(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    destination: *mut f32,
    selected: Option<&NativeTileScope<'_>>,
) -> RuntimeResult<()> {
    // SAFETY: the runner supplies its current, writable float destination.
    unsafe {
        match selected {
            Some(scope) => scope.step_float(destination),
            None => step_argument(access, frame, destination.cast()),
        }
    }
}

unsafe fn tile_finish(
    access: CoreAccess<'_>,
    frame: *mut FFrame,
    selected: Option<&NativeTileScope<'_>>,
) -> RuntimeResult<()> {
    // SAFETY: the runner calls finish once on the same current invocation.
    unsafe {
        match selected {
            Some(scope) => scope.finish_arguments(),
            None => finish_arguments(access, frame),
        }
    }
}

unsafe fn tile(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, clipped: bool, selected: Option<&NativeTileScope<'_>>) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let mut texture = std::ptr::null_mut::<UTexture>();
    let mut xl = 0.0_f32;
    let mut yl = 0.0_f32;
    let mut u = 0.0_f32;
    let mut v = 0.0_f32;
    let mut ul = 0.0_f32;
    let mut vl = 0.0_f32;
    unsafe {
        tile_texture_input(access, frame, &raw mut texture, selected)?;
        tile_float_input(access, frame, &raw mut xl, selected)?;
        tile_float_input(access, frame, &raw mut yl, selected)?;
        tile_float_input(access, frame, &raw mut u, selected)?;
        tile_float_input(access, frame, &raw mut v, selected)?;
        tile_float_input(access, frame, &raw mut ul, selected)?;
        tile_float_input(access, frame, &raw mut vl, selected)?;
        tile_finish(access, frame, selected)?;
        if clipped {
            crate::canvas_owner::native_draw_tile_clipped(access, object.cast(), texture, xl, yl, u, v, ul, vl)
        } else {
            crate::canvas_owner::native_draw_tile(access, object.cast(), texture, xl, yl, u, v, ul, vl)
        }
    }
}

unsafe fn draw_text_clipped(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let mut text = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let units = canvas_text_arg(access, frame, &mut text, usize::MAX)?;
        let mut check_hotkey = 0_u32;
        step_optional(access, frame, &mut check_hotkey)?;
        finish_arguments(access, frame)?;
        crate::canvas_owner::draw_text_clipped(access, object.cast(), &units, check_hotkey != 0)
    })() };
    let released = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(released)
}

unsafe fn text_size(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
    let mut text = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let units = canvas_text_arg(access, frame, &mut text, usize::MAX)?;
        let mut xl = 0.0_f32;
        let mut yl = 0.0_f32;
        let out_x = step_argument_out(access, frame, (&raw mut xl).cast())?;
        let out_y = step_argument_out(access, frame, (&raw mut yl).cast())?;
        finish_arguments(access, frame)?;
        if let Some((width, height)) = crate::canvas_owner::native_text_size(access, object.cast(), &units)? {
            out_x.cast::<f32>().write_unaligned(width as f32);
            out_y.cast::<f32>().write_unaligned(height as f32);
        }
        Ok(())
    })() };
    let released = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(released)
}

unsafe fn draw_actor(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    let mut actor = ptr::null_mut::<AActor>();
    let mut wireframe = 0_u32;
    let mut clear_z = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut actor).cast())?;
        step_argument(access, frame, (&raw mut wireframe).cast())?;
        step_optional(access, frame, &mut clear_z)?;
        finish_arguments(access, frame)?;
        let canvas = object.cast::<UCanvas>();
        if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
        if actor.is_null() {
            let message = (*b"DrawActor: Missing Actor").map(u32::from);
            log_frame(access, frame, &message)?;
            return Ok(());
        }
        crate::canvas_owner::draw_actor(access, canvas, actor, wireframe != 0, clear_z != 0)?;
        Ok(())
    }
}

unsafe fn draw_clipped_actor(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    let mut actor = ptr::null_mut::<AActor>();
    let mut wireframe = 0_u32;
    let mut x = 0_i32;
    let mut y = 0_i32;
    let mut xb = 0_i32;
    let mut yb = 0_i32;
    let mut clear_z = 0_u32;
    unsafe {
        step_argument(access, frame, (&raw mut actor).cast())?;
        step_argument(access, frame, (&raw mut wireframe).cast())?;
        step_argument(access, frame, (&raw mut x).cast())?;
        step_argument(access, frame, (&raw mut y).cast())?;
        step_argument(access, frame, (&raw mut xb).cast())?;
        step_argument(access, frame, (&raw mut yb).cast())?;
        step_optional(access, frame, &mut clear_z)?;
        finish_arguments(access, frame)?;
        let canvas = object.cast::<UCanvas>();
        if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
        if actor.is_null() {
            let message = (*b"DrawClippedActor: Missing Actor").map(u32::from);
            log_frame(access, frame, &message)?;
            return Ok(());
        }
        let render = ptr::addr_of!((*canvas).render).read_unaligned().cast::<URender>();
        let scene = ptr::addr_of!((*canvas).frame).read_unaligned().cast::<FSceneNode>();
        let viewport = ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>();
        if render.is_null() || scene.is_null() || viewport.is_null() {
            return Err(RuntimeError::new("Invalid canonical Canvas DrawClippedActor input"));
        }

        let view_actor = ptr::addr_of!((*viewport).actor).read_unaligned().cast::<hp2_abi::engine::generated::APlayerPawn>();
        if view_actor.is_null() { return Err(RuntimeError::new("Canonical Canvas has no viewport actor")); }
        let old_x = ptr::addr_of!((*scene).x).read_unaligned();
        let old_y = ptr::addr_of!((*scene).y).read_unaligned();
        let old_xb = ptr::addr_of!((*scene).xb).read_unaligned();
        let old_yb = ptr::addr_of!((*scene).yb).read_unaligned();
        ptr::addr_of_mut!((*scene).x).write_unaligned(x);
        ptr::addr_of_mut!((*scene).y).write_unaligned(y);
        ptr::addr_of_mut!((*scene).xb).write_unaligned(xb);
        ptr::addr_of_mut!((*scene).yb).write_unaligned(yb);

        compute_canvas_frame_coords(
            access,
            scene,
            FVector { x: 0.0, y: 0.0, z: 0.0 },
            FRotator { pitch: 0, yaw: 0, roll: 0 },
        )?;
        let old_render_map = ptr::addr_of!((*view_actor).rend_map).read_unaligned();
        if wireframe != 0 {
            ptr::addr_of_mut!((*view_actor).rend_map).write_unaligned(hp2_abi::engine::generated::ENGINE_REN_WIRE);
        }
        (*actor).set_b_hidden(false);
        if clear_z != 0 {
            let device = ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast();
            crate::render_device_owner::clear_z(access, device)?;
        }
        crate::render_owner::draw_actor(access, render, scene, actor)?;
        (*actor).set_b_hidden(true);
        ptr::addr_of_mut!((*view_actor).rend_map).write_unaligned(old_render_map);
        ptr::addr_of_mut!((*scene).x).write_unaligned(old_x);
        ptr::addr_of_mut!((*scene).y).write_unaligned(old_y);
        ptr::addr_of_mut!((*scene).xb).write_unaligned(old_xb);
        ptr::addr_of_mut!((*scene).yb).write_unaligned(old_yb);
        compute_canvas_frame_size(access, scene)?;
    }
    Ok(())
}

unsafe fn draw_portal(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void) -> RuntimeResult<()> {
    let mut x = 0_i32;
    let mut y = 0_i32;
    let mut width = 0_i32;
    let mut height = 0_i32;
    let mut camera_actor = ptr::null_mut::<AActor>();
    let mut camera_location = FVector { x: 0.0, y: 0.0, z: 0.0 };
    let mut camera_rotation = FRotator { pitch: 0, yaw: 0, roll: 0 };
    let mut fov = 90_i32;
    let mut clear_z = 1_u32;
    unsafe {
        step_argument(access, frame, (&raw mut x).cast())?;
        step_argument(access, frame, (&raw mut y).cast())?;
        step_argument(access, frame, (&raw mut width).cast())?;
        step_argument(access, frame, (&raw mut height).cast())?;
        step_argument(access, frame, (&raw mut camera_actor).cast())?;
        step_argument(access, frame, (&raw mut camera_location).cast())?;
        step_argument(access, frame, (&raw mut camera_rotation).cast())?;
        step_optional(access, frame, &mut fov)?;
        step_optional(access, frame, &mut clear_z)?;
        finish_arguments(access, frame)?;
        let canvas = object.cast::<UCanvas>();
        if canvas.is_null() { return Err(RuntimeError::new("Null canonical UCanvas")); }
        let viewport = ptr::addr_of!((*canvas).viewport).read_unaligned().cast::<UViewport>();
        let render = ptr::addr_of!((*canvas).render).read_unaligned().cast::<URender>();
        if viewport.is_null() || render.is_null() { return Err(RuntimeError::new("Invalid canonical Canvas DrawPortal input")); }
        let view_actor = ptr::addr_of!((*viewport).actor).read_unaligned().cast::<hp2_abi::engine::generated::APlayerPawn>();
        if view_actor.is_null() { return Err(RuntimeError::new("Canonical Canvas DrawPortal has no viewport actor")); }
        let saved_fov = ptr::addr_of!((*view_actor).base.fov_angle).read_unaligned();
        ptr::addr_of_mut!((*view_actor).base.fov_angle).write_unaligned(fov as f32);
        let portal_frame = crate::render_owner::create_master_frame(access, render, viewport, camera_location, camera_rotation)?;
        ptr::addr_of_mut!((*portal_frame).xb).write_unaligned(x);
        ptr::addr_of_mut!((*portal_frame).yb).write_unaligned(y);
        ptr::addr_of_mut!((*portal_frame).x).write_unaligned(width);
        ptr::addr_of_mut!((*portal_frame).y).write_unaligned(height);
        compute_canvas_frame_size(access, portal_frame)?;
        if clear_z != 0 {
            crate::render_device_owner::global_clear_z(access)?;
        }
        let _ = camera_actor;
        crate::render_owner::draw_world(access, render, portal_frame)?;
        crate::render_owner::finish_master_frame(access, render, portal_frame)?;
        ptr::addr_of_mut!((*view_actor).base.fov_angle).write_unaligned(saved_fov);
    }
    Ok(())
}

#[cfg(test)]
#[path = "canvas_tile_vm_tests.rs"]
mod tile_vm_tests;
