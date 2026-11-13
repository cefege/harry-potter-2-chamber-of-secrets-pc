//! The round label only; lesson activation, empty-round and LevelOver dispatch remain script-owned.
use hp2_abi::engine::generated::{TArrayRaw, UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};

use super::{find_function, int_of, param_object, player_fonts, quidditch_bar as paint};
use crate::stock_graphics_menu::{live, object, prop};

fn centered(width: i32, height: i32, text: (f32, f32)) -> (f32, f32) {
    ((width / 2) as f32 - text.0 / 2., (height / 2) as f32 - text.1 / 2.)
}

unsafe fn round_text(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<String> {
    // SAFETY: this is a reflected engine-owned FString, used synchronously without script reentry.
    unsafe {
        let array = prop(a, receiver, "strCurrRound", "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned();
        if array.num < 0 || array.max < array.num || (array.num > 0 && array.data.is_null()) {
            return Err(RuntimeError::new("Lesson round string unavailable"));
        }
        if array.num == 0 { return Ok(String::new()); }
        let units = std::slice::from_raw_parts(array.data.cast::<u32>(), array.num as usize);
        if units.last() != Some(&0) { return Err(RuntimeError::new("Unterminated lesson round string")); }
        units[..units.len() - 1].iter().copied().map(char::from_u32).collect::<Option<String>>()
            .ok_or_else(|| RuntimeError::new("Invalid lesson round text"))
    }
}

unsafe fn prepared(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<Option<(String, *mut UObject)>> {
    // SAFETY: preflight has no script, native draw, measurement or assignment effects.
    unsafe {
        if !paint::class_route(a, receiver, "HGame.SpellLessonTrigger")? { return Ok(None); }
        paint::validate_canvas(a, canvas)?;
        let fonts = player_fonts(a, object(a, receiver, "PlayerHarry")?)?;
        if !live(a, fonts[0]) { return Ok(None); }
        Ok(Some((round_text(a, receiver)?, fonts[0])))
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    let Some(canvas) = (unsafe { param_object(a, function, params, size, "Canvas") }) else { return Ok(ScriptPreCallDecision::Continue) };
    let (value, font) = match unsafe { prepared(a, receiver, canvas) } {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Ok(ScriptPreCallDecision::Continue),
        Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
    };
    // SAFETY: only this canonical drawing leaf is consumed. Canvas retains native cursor/CurYL effects.
    unsafe {
        let saved_font = object(a, canvas, "Font")?;
        let saved_color = paint::color(a, canvas)?;
        paint::set_font(a, canvas, font)?;
        paint::set_color(a, canvas, [255, 255, 0, saved_color[3]])?;
        let measured = paint::measure(a, canvas, &value)?;
        let pos = centered(int_of(a, canvas, "SizeX")?, int_of(a, canvas, "SizeY")?, measured);
        paint::text(a, receiver, canvas, 20, value, pos, measured, false)?;
        paint::set_font(a, canvas, saved_font)?;
        paint::set_color(a, canvas, saved_color)?;
    }
    Ok(ScriptPreCallDecision::Consume)
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: Core retains the canonical drawing leaf's metadata for the observer lifetime.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.SpellLessonTrigger", "Core.Class", "DrawRoundText")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-spell-lesson", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(centered(801, 601, (13., 17.)), (393.5, 291.5));
    assert_eq!(centered(1280, 720, (120., 30.)), (580., 345.));
    assert_eq!(centered(640, 480, (0., 0.)), (320., 240.));
}
