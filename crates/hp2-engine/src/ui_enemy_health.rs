//! Enemy frame/query/fill/End, restricted to inventoried canonical callbacks.
use std::ptr;

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use hp2_ui::{BlendMode, Rect};

use super::{CanvasTarget, find_function, int_of, param_object};
use super::player_health::{Picture, canvas_ready, color, icon, paint_picture, signature, texture_size, virtual_function};
use crate::{stock_graphics_menu::{Weak, is_a, live, object, read_boolean}, ui_host::find_class, ui_vm::{V, invoke_virtual}};

fn clamp_health(health: f32) -> f32 {
    // Retail FClamp's ordered comparisons map NaN to the upper bound, unlike
    // Rust's clamp; do not replace this with a normalized progress helper.
    if health < 0. { 0. } else if health < 1. { health } else { 1. }
}
fn frame_origin(scale: f32, height: i32) -> (f32, f32) {
    (4. * scale, height as f32 - scale * 110.)
}
fn fill_rect(x: f32, y: f32, scale: f32, health: f32, height: f32) -> (Rect, Rect) {
    let width = health * 116.;
    (Rect::new(x + 5. * scale, y + 83. * scale, width * scale, height * scale), Rect::new(0., 0., width, height))
}

unsafe fn supported_health(a: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<bool> {
    // SAFETY: only the decoded pure getters are supported. A custom function
    // with the same signature must stay stock before the frame is painted.
    unsafe {
        if function.is_null() { return Ok(false); }
        for owner in [
            "HGame.HChar", "HGame.baseBoss", "HGame.BossRailMove",
            "HGame.Basilisk", "HGame.Duellist", "HGame.QuidditchPlayer",
            "HGame.Aragog", "HGame.Peeves",
        ] {
            if function == find_function(a, owner, "Core.Class", "GetHealth")? {
                return signature(a, function, &[], Some(("Core.FloatProperty", 4)));
            }
        }
        Ok(false)
    }
}

unsafe fn supported_end(a: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<bool> {
    // SAFETY: exact identity retains the inventoried unregister/state effects.
    unsafe {
        Ok(!function.is_null()
            && function == find_function(a, "HGame.EnemyHealthManager", "Core.Class", "End")?
            && signature(a, function, &[], None)?)
    }
}

unsafe fn preflight(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    if !live(a, manager) || !live(a, canvas) { return Ok(false); }
    // SAFETY: all reflection reads follow live-object and field-layout checks.
    unsafe {
        let Some(class) = find_class(a, "HGame.EnemyHealthManager")? else { return Ok(false); };
        if !is_a(a, manager, class) || read_boolean(a, manager, "bDeleteMe")? || !canvas_ready(a, canvas)? { return Ok(false); }
        let function = virtual_function(a, manager, "RenderHudItemManager")?;
        let canonical = find_function(a, "HGame.EnemyHealthManager.DisplayEnemyHealth", "Core.State", "RenderHudItemManager")?;
        if canonical.is_null() || function != canonical || !signature(a, function,
            &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>()), ("bMenuMode", "Core.BoolProperty", 4), ("bFullCutMode", "Core.BoolProperty", 4), ("bHalfCutMode", "Core.BoolProperty", 4)], None)? { return Ok(false); }
        let scale = virtual_function(a, manager, "GetScaleFactor")?;
        if scale.is_null() || scale != find_function(a, "HGame.HudItemManager", "Core.Class", "GetScaleFactor")?
            || !signature(a, scale, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())], Some(("Core.FloatProperty", 4)))? { return Ok(false); }
        let end = virtual_function(a, manager, "End")?;
        if !supported_end(a, end)? { return Ok(false); }
        let enemy = object(a, manager, "Enemy")?;
        // A null enemy is a stock CONTEXT path (Accessed None -> zero result),
        // not a new Rust special case. Admit only before effects when nonnull.
        if !live(a, enemy) { return Ok(false); }
        let health = virtual_function(a, enemy, "GetHealth")?;
        if !supported_health(a, health)? { return Ok(false); }
        texture_size(a, object(a, manager, "textureBarFull")?)?;
        texture_size(a, object(a, manager, "textureBarEmpty")?)?;
        Ok(true)
    }
}

/// Draw the complete state-resolved enemy view without consuming its lifecycle.
/// # Safety
/// The route must have passed pre-effect admission; Core survives all reentry.
unsafe fn draw(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: callbacks use checked parameter/result layouts and are followed by
    // identity/resource revalidation. There is never a stock replay after effects.
    unsafe {
        let receiver = Weak::new(manager);
        let output = Weak::new(canvas);
        let mut scale = 0_f32;
        invoke_virtual(a, manager, "GetScaleFactor", &[("Canvas", V::Obj(canvas))], &[], (&raw mut scale).cast())?;
        if !receiver.live(a) || !output.live(a) || read_boolean(a, manager, "bDeleteMe")? { return Err(RuntimeError::new("Enemy scale callback destroyed its view")); }
        let (x, y) = frame_origin(scale, int_of(a, canvas, "SizeY")?);
        icon(a, manager, canvas, object(a, manager, "textureBarFull")?, x, y, scale)?;

        // Query the actual current Enemy only AFTER its frame has been painted.
        let enemy = object(a, manager, "Enemy")?;
        if !live(a, enemy) { return Err(RuntimeError::new("Enemy unavailable after frame drawing")); }
        let getter = virtual_function(a, enemy, "GetHealth")?;
        if !supported_health(a, getter)? { return Err(RuntimeError::new("Enemy health callback left the supported route")); }
        let mut health = 0_f32;
        invoke_virtual(a, enemy, "GetHealth", &[], &[], (&raw mut health).cast())?;
        if !receiver.live(a) || !output.live(a) || read_boolean(a, manager, "bDeleteMe")? { return Err(RuntimeError::new("Enemy health callback destroyed its view")); }
        let health = clamp_health(health);
        // The empty texture is intentionally looked up again after the getter.
        // It is not the inverse region: retail draws health * 116 pixels.
        let texture = object(a, manager, "textureBarEmpty")?;
        let (_, height) = texture_size(a, texture)?;
        let (dest, source) = fill_rect(x, y, scale, health, height);
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        target.set_pos(a, dest.x, dest.y)?;
        paint_picture(a, manager, canvas, Picture { texture: (*texture).index, dest, source, color: color(a, canvas)?, blend: BlendMode::Inherit })?;
        if health <= 0. {
            let end = virtual_function(a, manager, "End")?;
            if !supported_end(a, end)? { return Err(RuntimeError::new("Enemy End callback left the supported route")); }
            // End owns HUD unregister, bRegisteredWithHud and GotoState. It is
            // called even though a zero-width native tile has just been drawn.
            invoke_virtual(a, manager, "End", &[], &[], ptr::null_mut())?;
        }
        // Retail saves/restores no Canvas fields here, including on errors.
        Ok(())
    }
}

unsafe fn callback(a: CoreAccess<'_>, manager: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    // SAFETY: Core supplies function metadata and the readable parameter frame.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !preflight(a, manager, canvas).unwrap_or(false) { return Ok(ScriptPreCallDecision::Continue); }
        draw(a, manager, canvas)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}

/// # Safety
/// Core must outlive the registration guard.
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: only the declared drawing leaf is observed, not its state wrapper,
    // Start/End, GetHealth, Tick, registration or any gameplay mutation.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.EnemyHealthManager.DisplayEnemyHealth", "Core.State", "RenderHudItemManager")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-enemy-health", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(clamp_health(-1.), 0.);
    assert_eq!(clamp_health(0.25), 0.25);
    assert_eq!(clamp_health(2.), 1.);
    assert_eq!(clamp_health(f32::NAN), 1.);
    let (x, y) = frame_origin(1.25, 600);
    assert_eq!((x, y), (5., 462.5));
    let (dest, source) = fill_rect(x, y, 1.25, 0.5, 128.);
    assert_eq!(dest, Rect::new(11.25, 566.25, 72.5, 160.));
    assert_eq!(source, Rect::new(0., 0., 58., 128.));
    let (zero, source) = fill_rect(4., 490., 1., 0., 128.);
    assert_eq!(zero, Rect::new(9., 573., 0., 128.));
    assert_eq!(source.w, 0., "zero-health fill still precedes End");
    let (negative, _) = fill_rect(-4., -110., -1., 1., 128.);
    assert_eq!(negative.w, -116., "native signed geometry is preserved");
}
