//! Score and house-point leaves; tally, awards, sounds and hold states remain script-owned.
use std::ptr;

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};

use super::{find_function, int_of, param_object, player_fonts, quidditch_bar as paint};
use crate::stock_graphics_menu::{live, object};

fn font_slot(width: i32) -> usize {
    if width <= 512 { 2 } else if width <= 640 { 1 } else { 0 }
}

fn score_anchor(x: i32, y: i32, scale: f32, midpoint: (i32, i32), text: (f32, f32)) -> (f32, f32) {
    (x as f32 + midpoint.0 as f32 * scale - text.0 / 2., y as f32 + midpoint.1 as f32 * scale - text.1 / 2.)
}

unsafe fn admit(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, tally: bool) -> RuntimeResult<bool> {
    // SAFETY: admission reads metadata and fields only, before any script callback or Canvas effect.
    unsafe {
        if !paint::class_route(a, receiver, "HGame.QuidScoreManager")? || !paint::validate_scale(a, receiver)? { return Ok(false); }
        paint::validate_canvas(a, canvas)?;
        if tally {
            paint::texture(a, receiver, "textureTallyPointsIcon")?;
            let points = object(a, receiver, "siGryffHousePoints")?;
            if !live(a, points) { return Ok(false); }
            int_of(a, points, "nCount")?;
        } else {
            paint::texture(a, receiver, "textureGryff")?;
            paint::texture(a, receiver, "textureOpponent")?;
            int_of(a, receiver, "nGryffindorScore")?;
            int_of(a, receiver, "nOpponentScore")?;
            if !paint::canonical(a, canvas, "DrawShadowText", &[("Engine.Canvas", "Core.Class")])? { return Ok(false); }
            let fonts = player_fonts(a, object(a, receiver, "PlayerHarry")?)?;
            if !live(a, fonts[font_slot(int_of(a, canvas, "SizeX")?)]) { return Ok(false); }
        }
    }
    Ok(true)
}

unsafe fn draw_score(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: the complete canonical leaf has passed admission. Later failures propagate without replay.
    unsafe {
        let saved_font = object(a, canvas, "Font")?;
        let scale = paint::scale(a, receiver, canvas)?;
        paint::validate_canvas(a, canvas)?;
        let gx = (520_f32 * scale) as i32;
        let gy = (4_f32 * scale) as i32;
        paint::set_pos(a, canvas, gx as f32, gy as f32)?;
        paint::icon(a, receiver, canvas, 10, paint::texture(a, receiver, "textureGryff")?, gx as f32, gy as f32, scale)?;
        let ox = (578_f32 * scale) as i32;
        let oy = (4_f32 * scale) as i32;
        paint::set_pos(a, canvas, ox as f32, oy as f32)?;
        paint::icon(a, receiver, canvas, 11, paint::texture(a, receiver, "textureOpponent")?, ox as f32, oy as f32, scale)?;
        let gryff = int_of(a, receiver, "nGryffindorScore")?.to_string();
        let opponent = int_of(a, receiver, "nOpponentScore")?.to_string();
        let fonts = player_fonts(a, object(a, receiver, "PlayerHarry")?)?;
        let font = fonts[font_slot(int_of(a, canvas, "SizeX")?)];
        if !live(a, font) { return Err(RuntimeError::new("Quidditch score font unavailable")); }
        paint::set_font(a, canvas, font)?;
        let size = paint::measure(a, canvas, &gryff)?;
        let pos = score_anchor(gx, gy, scale, (45, 50), size);
        paint::set_pos(a, canvas, pos.0, pos.1)?;
        let saved_color = paint::color(a, canvas)?;
        // Retail DrawShadowText uses two DrawText calls with omitted CR (true),
        // then restores DrawColor, not its cursor/CurYL or the incoming Style.
        paint::set_color(a, canvas, [0, 0, 0, 0])?;
        paint::text(a, receiver, canvas, 12, gryff.clone(), (pos.0 + 1., pos.1 + 1.), size, true)?;
        paint::set_color(a, canvas, [206, 200, 190, 0])?;
        paint::text(a, receiver, canvas, 13, gryff, pos, size, true)?;
        paint::set_color(a, canvas, saved_color)?;
        let size = paint::measure(a, canvas, &opponent)?;
        let pos = score_anchor(ox, oy, scale, (45, 50), size);
        // The opponent intentionally has neither a shadow nor the off-white tint.
        paint::text(a, receiver, canvas, 14, opponent, pos, size, false)?;
        paint::set_font(a, canvas, saved_font)
    }
}

unsafe fn draw_tally(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: the leaf's resources, parameter and field layouts passed preflight.
    unsafe {
        let saved_color = paint::color(a, canvas)?;
        let saved_font = object(a, canvas, "Font")?;
        let scale = paint::scale(a, receiver, canvas)?;
        paint::validate_canvas(a, canvas)?;
        let x = ((int_of(a, canvas, "SizeX")? / 2) as f32 - (128 / 2) as f32 * scale) as i32;
        let y = (20_f32 * scale) as i32;
        paint::set_pos(a, canvas, x as f32, y as f32)?;
        paint::icon(a, receiver, canvas, 15, paint::texture(a, receiver, "textureTallyPointsIcon")?, x as f32, y as f32, scale)?;
        let points = object(a, receiver, "siGryffHousePoints")?;
        if !live(a, points) { return Err(RuntimeError::new("Quidditch tally points unavailable")); }
        let value = int_of(a, points, "nCount")?.to_string();
        let mut black = paint::color(a, canvas)?;
        black[..3].fill(0);
        paint::set_color(a, canvas, black)?;
        let size = paint::measure(a, canvas, &value)?;
        let pos = score_anchor(x, y, scale, (65, 87), size);
        paint::text(a, receiver, canvas, 16, value, pos, size, false)?;
        paint::set_color(a, canvas, saved_color)?;
        paint::set_font(a, canvas, saved_font)
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    let Some(canvas) = (unsafe { param_object(a, function, params, size, "Canvas") }) else { return Ok(ScriptPreCallDecision::Continue) };
    let tally = function == unsafe { find_function(a, "HGame.QuidScoreManager", "Core.Class", "DrawTallyHousepoints")? };
    match unsafe { admit(a, receiver, canvas, tally) } {
        Ok(true) => {},
        Ok(false) => return Ok(ScriptPreCallDecision::Continue),
        Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
    }
    unsafe { if tally { draw_tally(a, receiver, canvas)?; } else { draw_score(a, receiver, canvas)?; } }
    Ok(ScriptPreCallDecision::Consume)
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: the observer retains canonical metadata; wrapper functions are not consumed.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let score = find_function(a, "HGame.QuidScoreManager", "Core.Class", "DrawQuidScore")?;
        let tally = find_function(a, "HGame.QuidScoreManager", "Core.Class", "DrawTallyHousepoints")?;
        let functions: Vec<_> = [score, tally].into_iter().filter(|f| *f != ptr::null_mut()).collect();
        if functions.is_empty() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-quidditch-score", &functions, callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!([512, 513, 640, 641].map(font_slot), [2, 1, 1, 0]);
    assert_eq!(score_anchor(650, 5, 1.25, (45, 50), (13., 17.)), (699.75, 59.));
    assert_eq!(((641 / 2) as f32 - 64. * (641_f32 / 640.)) as i32, 255,
        "tally divides the odd viewport width as an integer before centering");
}
