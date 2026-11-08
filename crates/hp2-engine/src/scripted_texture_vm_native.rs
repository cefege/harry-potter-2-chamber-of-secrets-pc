//! Canonical VM argument stepping for the source-shared ScriptedTexture natives.

use hp2_abi::engine::generated::{FColor, FFrame, TArrayRaw, UFont, UObject, UTexture};
use hp2_core::{
    canonical_vm_native::{finish_arguments, step_argument, step_argument_out, VmNativeSpec},
    core_access::{CoreAccess, RuntimeResult},
    native_text::fstring_units,
};
use std::{ffi::c_void, ptr};

pub(crate) const NATIVE_FUNCTIONS: [VmNativeSpec; 5] = [
    VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "DrawText", native_index: Some(472), handler: draw_text },
    VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "DrawTile", native_index: Some(473), handler: draw_tile },
    VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "DrawColoredText", native_index: Some(474), handler: draw_colored_text },
    VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "ReplaceTexture", native_index: Some(475), handler: replace_texture },
    VmNativeSpec { class_name: "Engine.ScriptedTexture", function_name: "TextSize", native_index: Some(476), handler: text_size },
];

unsafe fn draw_text(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut x = 0.0_f32;
        let mut y = 0.0_f32;
        let mut font = ptr::null_mut::<UFont>();
        step_argument(access, frame, (&raw mut x).cast())?;
        step_argument(access, frame, (&raw mut y).cast())?;
        step_argument(access, frame, (&raw mut text).cast())?;
        step_argument(access, frame, (&raw mut font).cast())?;
        finish_arguments(access, frame)?;
        let units = unsafe { fstring_units(&text) };
        crate::scripted_texture_owner::draw_text(
            access, object, frame, x, y, units.as_ptr().cast_mut().cast(), units.len(), font,
        )
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(release)
}

unsafe fn draw_colored_text(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut x = 0.0_f32;
        let mut y = 0.0_f32;
        let mut font = ptr::null_mut::<UFont>();
        let mut color = FColor { r: 0, g: 0, b: 0, a: 0 };
        step_argument(access, frame, (&raw mut x).cast())?;
        step_argument(access, frame, (&raw mut y).cast())?;
        step_argument(access, frame, (&raw mut text).cast())?;
        step_argument(access, frame, (&raw mut font).cast())?;
        step_argument(access, frame, (&raw mut color).cast())?;
        finish_arguments(access, frame)?;
        let units = unsafe { fstring_units(&text) };
        let color = u32::from_ne_bytes([color.r, color.g, color.b, color.a]);
        crate::scripted_texture_owner::draw_colored_text(
            access, object, frame, x, y, units.as_ptr().cast_mut().cast(), units.len(), font, color,
        )
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(release)
}

unsafe fn draw_tile(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut rect = [0.0_f32; 8];
    let mut texture = ptr::null_mut::<UTexture>();
    let mut masked = 0_u32;
    unsafe {
        for value in &mut rect { step_argument(access, frame, (&raw mut *value).cast())?; }
        step_argument(access, frame, (&raw mut texture).cast())?;
        step_argument(access, frame, (&raw mut masked).cast())?;
        finish_arguments(access, frame)?;
    }
    unsafe {
        crate::scripted_texture_owner::draw_tile(
            access, object, frame,
            crate::scr_tex::TileRect {
                x: rect[0], y: rect[1], xl: rect[2], yl: rect[3],
                u: rect[4], v: rect[5], ul: rect[6], vl: rect[7],
            }, texture, masked != 0,
        )
    }
}

unsafe fn replace_texture(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut texture = ptr::null_mut::<UTexture>();
    unsafe { step_argument(access, frame, (&raw mut texture).cast())?; finish_arguments(access, frame)?; }
    unsafe { crate::scripted_texture_owner::replace_texture(access, object, frame, texture) }
}

unsafe fn text_size(
    access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _result: *mut c_void,
) -> RuntimeResult<()> {
    let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let operation = unsafe { (|| {
        let mut x = 0.0_f32;
        let mut y = 0.0_f32;
        let mut font = ptr::null_mut::<UFont>();
        step_argument(access, frame, (&raw mut text).cast())?;
        let out_x = step_argument_out(access, frame, (&raw mut x).cast())?;
        let out_y = step_argument_out(access, frame, (&raw mut y).cast())?;
        step_argument(access, frame, (&raw mut font).cast())?;
        finish_arguments(access, frame)?;
        let units = unsafe { fstring_units(&text) };
        if let Some((width, height)) = unsafe {
            crate::scripted_texture_owner::text_size(
                access, object, frame, units.as_ptr().cast_mut().cast(), units.len(), font,
            )?
        } {
            unsafe { out_x.cast::<f32>().write_unaligned(width); }
            unsafe { out_y.cast::<f32>().write_unaligned(height); }
        }
        Ok(())
    })() };
    let release = unsafe { access.release_vm_fstring_argument(&mut text) };
    operation.and(release)
}
