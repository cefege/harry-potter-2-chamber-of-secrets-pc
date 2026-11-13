//! Challenge score and result composition; score updates, awards and state transitions stay script-owned.
use std::{ffi::c_void, ptr};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};

use super::{find_function, int_of, param_object, player_fonts, quidditch_bar as paint, status_groups};
use crate::{stock_graphics_menu::{field, live, object}, ui_vm::{V, invoke_resolved, invoke_virtual}};

const SCORE_STATES: &[(&str, &str)] = &[
    ("HGame.ChallengeScoreManager.ChallengeInProgress", "Core.State"),
    ("HGame.ChallengeScoreManager.Tally", "Core.State"),
    ("HGame.ChallengeScoreManager.PostTallyHold", "Core.State"),
];

unsafe fn validate_xy(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<()> {
    // SAFETY: reflected fields are bounds/type checked; the fixed stack block is admitted before effects.
    unsafe {
        let function = paint::resolve(a, receiver, name)?;
        if function.is_null() || usize::from((*function).parms_size) > 32 {
            return Err(RuntimeError::new("Unsupported challenge coordinate parameters"));
        }
        let (x, y) = if name == "GetScorePosition" { ("nIconX", "nIconY") } else { ("nMidX", "nMidY") };
        paint::validate_args(a, function, &[(x, "Core.IntProperty", 4), (y, "Core.IntProperty", 4)])?;
        if name == "GetScorePosition" {
            paint::validate_args(a, function, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())])?;
        }
    }
    Ok(())
}

unsafe fn xy(a: CoreAccess<'_>, receiver: *mut UObject, name: &str, canvas: *mut UObject) -> RuntimeResult<(i32, i32)> {
    // SAFETY: the admitted, typed coordinate callback receives an aligned stack parameter block.
    unsafe {
        validate_xy(a, receiver, name)?;
        let function = paint::resolve(a, receiver, name)?;
        let size = usize::from((*function).parms_size);
        let mut params = [0_u64; 4];
        let base = params.as_mut_ptr().cast::<u8>();
        let (x, y) = if name == "GetScorePosition" { ("nIconX", "nIconY") } else { ("nMidX", "nMidY") };
        let x = field(a, function.cast(), x, "Core.IntProperty", 4, size, true)?;
        let y = field(a, function.cast(), y, "Core.IntProperty", 4, size, true)?;
        if name == "GetScorePosition" {
            let offset = field(a, function.cast(), "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>(), size, true)?;
            base.add(offset).cast::<*mut UObject>().write_unaligned(canvas);
        }
        a.execute_event(receiver, function, base.cast(), ptr::null_mut())?;
        Ok((base.add(x).cast::<i32>().read_unaligned(), base.add(y).cast::<i32>().read_unaligned()))
    }
}

fn anchor(icon: (i32, i32), midpoint: (i32, i32), scale: f32, text: (f32, f32)) -> (f32, f32) {
    (icon.0 as f32 + midpoint.0 as f32 * scale - text.0 / 2., icon.1 as f32 + midpoint.1 as f32 * scale - text.1 / 2.)
}

unsafe fn admit_score(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    // SAFETY: complete route admission does not invoke callbacks, measurements or assignments.
    unsafe {
        if !paint::class_route(a, receiver, "HGame.ChallengeScoreManager")? || !paint::validate_scale(a, receiver)? { return Ok(false); }
        paint::validate_canvas(a, canvas)?;
        for name in ["GetScorePosition", "GetCurrScoreTextXY", "GetHighScoreTextXY", "GetScoreTexture"] {
            if !paint::canonical(a, receiver, name, SCORE_STATES)? { return Ok(false); }
        }
        for name in ["GetInProgressScorePosition", "GetTallyScorePosition", "GetScoreFont"] {
            if !paint::canonical(a, receiver, name, &[("HGame.ChallengeScoreManager", "Core.Class")])? { return Ok(false); }
        }
        for name in ["GetScorePosition", "GetCurrScoreTextXY", "GetHighScoreTextXY"] { validate_xy(a, receiver, name)?; }
        paint::validate_args(a, paint::resolve(a, receiver, "GetScoreTexture")?, &[("ReturnValue", "Core.ObjectProperty", size_of::<*mut UObject>())])?;
        paint::validate_args(a, paint::resolve(a, receiver, "GetScoreFont")?, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>()), ("ReturnValue", "Core.ObjectProperty", size_of::<*mut UObject>())])?;
        for name in ["textureScoreIcon", "textureTallyScoreIcon"] { paint::texture(a, receiver, name)?; }
        int_of(a, receiver, "nCurrScore")?;
        int_of(a, receiver, "nHighScore")?;
        let fonts = player_fonts(a, object(a, receiver, "PlayerHarry")?)?;
        let width = int_of(a, canvas, "SizeX")?;
        let slot = if width <= 512 { 2 } else if width <= 640 { 1 } else { 0 };
        if !live(a, fonts[slot]) { return Ok(false); }
    }
    Ok(true)
}

unsafe fn draw_score(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: callback order follows the retail presentation leaf, including each text callback after measurement.
    unsafe {
        let scale = paint::scale(a, receiver, canvas)?;
        let icon = xy(a, receiver, "GetScorePosition", canvas)?;
        paint::validate_canvas(a, canvas)?;
        paint::set_pos(a, canvas, icon.0 as f32, icon.1 as f32)?;
        let mut texture = ptr::null_mut::<UObject>();
        invoke_virtual(a, receiver, "GetScoreTexture", &[], &[], (&raw mut texture).cast::<c_void>())?;
        paint::validate_canvas(a, canvas)?;
        paint::validate_texture(a, texture)?;
        // GetScoreTexture can alter Canvas position. DrawIcon starts at the live
        // cursor set before that callback, not an eagerly cached replacement.
        let x = super::f32_of(a, canvas, "CurX")?;
        let y = super::f32_of(a, canvas, "CurY")?;
        paint::icon(a, receiver, canvas, 30, texture, x, y, scale)?;
        let saved_color = paint::color(a, canvas)?;
        let saved_font = object(a, canvas, "Font")?;
        let current = int_of(a, receiver, "nCurrScore")?.to_string();
        let previous = int_of(a, receiver, "nHighScore")?.to_string();
        let mut black = saved_color;
        black[..3].fill(0);
        paint::set_color(a, canvas, black)?;
        let mut font = ptr::null_mut::<UObject>();
        invoke_virtual(a, receiver, "GetScoreFont", &[("Canvas", V::Obj(canvas))], &[], (&raw mut font).cast::<c_void>())?;
        paint::validate_canvas(a, canvas)?;
        if !live(a, font) { return Err(RuntimeError::new("Challenge score font unavailable")); }
        paint::set_font(a, canvas, font)?;
        let measured = paint::measure(a, canvas, &current)?;
        let midpoint = xy(a, receiver, "GetCurrScoreTextXY", canvas)?;
        paint::validate_canvas(a, canvas)?;
        paint::text(a, receiver, canvas, 31, current, anchor(icon, midpoint, scale, measured), measured, false)?;
        let measured = paint::measure(a, canvas, &previous)?;
        let midpoint = xy(a, receiver, "GetHighScoreTextXY", canvas)?;
        paint::validate_canvas(a, canvas)?;
        paint::text(a, receiver, canvas, 32, previous, anchor(icon, midpoint, scale, measured), measured, false)?;
        paint::set_color(a, canvas, saved_color)?;
        paint::set_font(a, canvas, saved_font)
    }
}

unsafe fn child(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    if !live(a, receiver) { return Err(RuntimeError::new("Challenge receiver unavailable")); }
    let item = unsafe { object(a, receiver, name)? };
    if !live(a, item) { return Err(RuntimeError::new("Challenge result item unavailable")); }
    Ok(item)
}

unsafe fn validate_child(a: CoreAccess<'_>, item: *mut UObject, canvas: *mut UObject, specified: bool) -> RuntimeResult<()> {
    unsafe {
        for name in ["GetHudIconUSize", "GetHudIconVSize"] {
            paint::validate_args(a, paint::resolve(a, item, name)?, &[("ReturnValue", "Core.IntProperty", 4)])?;
        }
        let draw = paint::resolve(a, item, if specified { "DrawSpecifiedCount" } else { "DrawItem" })?;
        paint::validate_args(a, draw, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>()), ("nCurrX", "Core.IntProperty", 4), ("nCurrY", "Core.IntProperty", 4), ("fScaleFactor", "Core.FloatProperty", 4)])?;
        if specified {
            paint::validate_args(a, draw, &[("nLocalCount", "Core.IntProperty", 4)])?;
            status_groups::admits_specified_count(a, item, canvas)?;
        } else {
            status_groups::admits_item(a, item, canvas)?;
        }
    }
    Ok(())
}

unsafe fn admit_points(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    unsafe {
        if !paint::class_route(a, receiver, "HGame.ChallengeScoreManager")? || !paint::validate_scale(a, receiver)? { return Ok(false); }
        paint::validate_canvas(a, canvas)?;
        for (name, specified) in [("siGryffPts", true), ("siJellybeans", false), ("siStars", false)] {
            let item = child(a, receiver, name)?;
            validate_child(a, item, canvas, specified)?;
        }
        paint::texture(a, child(a, receiver, "siGryffPts")?, "textureHudIcon")?;
        int_of(a, receiver, "nAwardGryffPoints")?;
    }
    Ok(true)
}

unsafe fn dimension(a: CoreAccess<'_>, receiver: *mut UObject, item: &str, name: &str) -> RuntimeResult<i32> {
    let mut result = 0_i32;
    unsafe {
        let item = child(a, receiver, item)?;
        paint::validate_args(a, paint::resolve(a, item, name)?, &[("ReturnValue", "Core.IntProperty", 4)])?;
        invoke_virtual(a, item, name, &[], &[], (&raw mut result).cast())?;
    }
    Ok(result)
}

unsafe fn draw_item(a: CoreAccess<'_>, receiver: *mut UObject, name: &str, canvas: *mut UObject, x: i32, y: i32, scale: f32, count: Option<i32>) -> RuntimeResult<()> {
    // SAFETY: re-admit the current child after previous callbacks. Unsupported
    // custom children run their resolved script exactly once, never a nested hook.
    unsafe {
        paint::validate_canvas(a, canvas)?;
        let item = child(a, receiver, name)?;
        validate_child(a, item, canvas, count.is_some())?;
        if let Some(count) = count {
            if status_groups::admits_specified_count(a, item, canvas)? {
                return status_groups::draw_specified_count(a, item, canvas, x, y, scale, count);
            }
            let function = paint::resolve(a, item, "DrawSpecifiedCount")?;
            invoke_resolved(a, item, function, &[("Canvas", V::Obj(canvas)), ("nCurrX", V::I(x)), ("nCurrY", V::I(y)), ("fScaleFactor", V::F(scale)), ("nLocalCount", V::I(count))], &[], ptr::null_mut())?;
        } else if status_groups::admits_item(a, item, canvas)? {
            status_groups::draw_item(a, item, canvas, x, y, scale)?;
        } else {
            let function = paint::resolve(a, item, "DrawItem")?;
            invoke_resolved(a, item, function, &[("Canvas", V::Obj(canvas)), ("nCurrX", V::I(x)), ("nCurrY", V::I(y)), ("fScaleFactor", V::F(scale))], &[], ptr::null_mut())?;
        }
    }
    Ok(())
}

unsafe fn draw_points(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: the full composition was admitted before the first callback. Each
    // child/resource is reread at its original use, not cached across script calls.
    unsafe {
        let scale = paint::scale(a, receiver, canvas)?;
        paint::validate_canvas(a, canvas)?;
        let half_width = int_of(a, canvas, "SizeX")? / 2;
        let icon_half_width = dimension(a, receiver, "siGryffPts", "GetHudIconUSize")? / 2;
        let px = (half_width as f32 - icon_half_width as f32 * scale) as i32;
        let py = (2_f32 * scale) as i32;
        paint::validate_canvas(a, canvas)?;
        paint::set_pos(a, canvas, px as f32, py as f32)?;
        paint::icon(a, receiver, canvas, 33, paint::texture(a, child(a, receiver, "siGryffPts")?, "textureHudIcon")?, px as f32, py as f32, scale)?;
        let award = int_of(a, receiver, "nAwardGryffPoints")?;
        draw_item(a, receiver, "siGryffPts", canvas, px, py, scale, Some(award))?;
        let bean_width = dimension(a, receiver, "siJellybeans", "GetHudIconUSize")?.wrapping_add(30);
        let bx = (px as f32 - bean_width as f32 * scale) as i32;
        let points_half_height = dimension(a, receiver, "siGryffPts", "GetHudIconVSize")? / 2;
        let bean_half_height = dimension(a, receiver, "siJellybeans", "GetHudIconVSize")? / 2;
        let by = (py as f32 + points_half_height as f32 * scale - bean_half_height as f32 * scale) as i32;
        // Retail sets the not-yet-assigned star locals (both zero) here.
        paint::set_pos(a, canvas, 0., 0.)?;
        draw_item(a, receiver, "siJellybeans", canvas, bx, by, scale, None)?;
        let points_width = dimension(a, receiver, "siGryffPts", "GetHudIconUSize")?.wrapping_add(30);
        let sx = (px as f32 + points_width as f32 * scale) as i32;
        let points_half_height = dimension(a, receiver, "siGryffPts", "GetHudIconVSize")? / 2;
        let star_half_height = dimension(a, receiver, "siStars", "GetHudIconVSize")? / 2;
        let sy = (py as f32 + points_half_height as f32 * scale - star_half_height as f32 * scale) as i32;
        paint::set_pos(a, canvas, sx as f32, sy as f32)?;
        draw_item(a, receiver, "siStars", canvas, sx, sy, scale, None)
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    let Some(canvas) = (unsafe { param_object(a, function, params, size, "Canvas") }) else { return Ok(ScriptPreCallDecision::Continue) };
    let points = function == unsafe { find_function(a, "HGame.ChallengeScoreManager.PostTallyHoldPoints", "Core.State", "RenderHudItemManager")? };
    let menu = match unsafe { paint::menu_param(a, function, params, size) } {
        Ok(menu) => menu,
        Err(_) => return Ok(ScriptPreCallDecision::Continue),
    };
    if menu { return Ok(ScriptPreCallDecision::Consume); }
    let admission = unsafe { if points { admit_points(a, receiver, canvas) } else { admit_score(a, receiver, canvas) } };
    match admission {
        Ok(true) => {},
        Ok(false) => return Ok(ScriptPreCallDecision::Continue),
        Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
    }
    unsafe { if points { draw_points(a, receiver, canvas)?; } else { draw_score(a, receiver, canvas)?; } }
    Ok(ScriptPreCallDecision::Consume)
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: only the drawing leaves are observed; all award/tally state wrappers remain script.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let score = find_function(a, "HGame.ChallengeScoreManager", "Core.Class", "DrawScore")?;
        let points = find_function(a, "HGame.ChallengeScoreManager.PostTallyHoldPoints", "Core.State", "RenderHudItemManager")?;
        let functions: Vec<_> = [score, points].into_iter().filter(|f| !f.is_null()).collect();
        if functions.is_empty() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-challenge-score", &functions, callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(anchor((255, 3), (32, 65), 641_f32 / 640., (13., 17.)), (280.55, 59.601562));
    assert_eq!(anchor((-20, 3), (-10, 0), 1.25, (15., 17.)), (-40., -5.5));
}
