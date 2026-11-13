//! Countdown, Bean and Gold presentation. Duration, lifecycle and sounds remain script.
use std::{cell::RefCell, ptr};
use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};
use super::{CanvasTarget, f32_of, find_function, int_of, param_object, set_f32, timer_origin, timer_segment};
use crate::{stock_graphics_menu::{Weak, field, live, object, prop, read_boolean}, ui_host::{DrawTarget, HostMetrics, render_commands}, ui_vm::{V, invoke_resolved, invoke_virtual}};

#[derive(Clone, Debug, PartialEq)]
struct Art { texture: TextureRef, destination: Rect, source: Rect, color: [u8; 4] }
#[derive(Clone, Debug, PartialEq)]
enum TimerPhase {
    Background(Art),
    Fill(Art),
    Numeric { text: String, position: (f32, f32), extent: (f32, f32), color: [u8; 4] },
}
fn rgba(c: [u8; 4]) -> Color { Color(c[0], c[1], c[2], c[3]) }
fn view(phase: &TimerPhase) -> Node {
    col().child(match phase {
        TimerPhase::Background(art) | TimerPhase::Fill(art) => {
            let s = art.source; let d = art.destination;
            image(art.texture.clone(), Fit::Region(s.x, s.y, s.w, s.h)).color(rgba(art.color))
                .at(d.x, d.y).width(Size::Px(d.w)).height(Size::Px(d.h))
        }
        TimerPhase::Numeric { text, position, extent, color } => label(text).font(Font::Slot(0)).color(rgba(*color))
            .at(position.0, position.1).width(Size::Px(extent.0)).height(Size::Px(extent.1)),
    })
}
struct TimerUi { receiver: Weak, model: Signal<TimerPhase>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<TimerUi>> = const { RefCell::new(Vec::new()) }; }
struct TimerTarget(CanvasTarget);
impl DrawTarget for TimerTarget {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected timer fill command")) }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        // SAFETY: delegate canonical synchronous texture resolution.
        unsafe { self.0.texture_object(a, name) }
    }
    unsafe fn segment(&self, a: CoreAccess<'_>, tex: *mut UObject, dest: Rect, src: Rect, color: [u8; 4], blend: BlendMode) -> RuntimeResult<()> {
        // SAFETY: resources are live and no script callback runs during this phase.
        unsafe { self.0.segment(a, tex, dest, src, color, blend) }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, font: Font, color: [u8; 4]) -> RuntimeResult<()> {
        // SAFETY: original SetPos preceded the second duration callback. Preserve
        // its possibly changed CurYL, font and style, rather than issuing SetPos again.
        unsafe {
            self.0.set_font(a, font)?; self.0.set_color(a, color)?;
            set_f32(a, self.0.canvas, "CurX", x)?; set_f32(a, self.0.canvas, "CurY", y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.0.canvas.cast(), &units, false, 0)
        }
    }
}
unsafe fn canvas_color(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<[u8; 4]> {
    // SAFETY: accessor checks property type/layout and live object bounds.
    Ok(unsafe { prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned() })
}
unsafe fn current_font(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<*mut UObject> {
    // SAFETY: object accessor checks layout before reading the reflected pointer.
    unsafe { object(a, canvas, "Font") }
}
unsafe fn texture(a: CoreAccess<'_>, manager: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    // SAFETY: property access validates the receiver before reading; dimensions require live texture.
    unsafe {
        let texture = object(a, manager, name)?;
        if !live(a, texture) { return Err(RuntimeError::new("Timer graphics unavailable")); }
        int_of(a, texture, "USize")?; int_of(a, texture, "VSize")?;
        Ok(texture)
    }
}
unsafe fn virtual_function(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    // SAFETY: receiver has been checked by admission or after a callback.
    let function = unsafe { a.find_object_function_by_name(receiver, a.intern_str(name, FindName::Find)?, false)? };
    if function.is_null() { return Err(RuntimeError::new("Timer callback unavailable")); }
    Ok(function)
}
unsafe fn canvas_parameter(a: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<()> {
    // SAFETY: function is canonical metadata; field validates parameter bounds/type.
    unsafe { field(a, function.cast(), "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>(), usize::from((*function).parms_size), true)?; }
    Ok(())
}
unsafe fn admits(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject, tuning_only: bool) -> RuntimeResult<bool> {
    // SAFETY: all operations in this routine are read-only, before effects.
    unsafe {
        if !live(a, manager) || !live(a, canvas) { return Ok(false); }
        let duration = virtual_function(a, manager, "GetTimerDuration")?;
        let base = find_function(a, "HGame.CountdownTimerManager", "Core.Class", "GetTimerDuration")?;
        let bean = find_function(a, "HGame.BeanRoomTimerManager", "Core.Class", "GetTimerDuration")?;
        if duration != base && duration != bean { return Ok(false); }
        if (*duration).parms_size != 4 { return Ok(false); }
        let tuning = virtual_function(a, manager, "DrawTuningModeData")?; canvas_parameter(a, tuning)?;
        read_boolean(a, manager, "bShowNumericTime")?;
        f32_of(a, manager, "fCountdownTime")?;
        int_of(a, canvas, "SizeX")?; int_of(a, canvas, "SizeY")?; canvas_color(a, canvas)?;
        for name in ["CurX", "CurY", "CurYL", "OrgX", "OrgY", "ClipX", "ClipY"] { f32_of(a, canvas, name)?; }
        prop(a, canvas, "Style", "Core.ByteProperty", 1)?;
        // DrawText's null-font diagnostics stay stock during unsupported startup resources.
        if read_boolean(a, manager, "bShowNumericTime")? && !live(a, current_font(a, canvas)?) { return Ok(false); }
        if !tuning_only {
            let scale = virtual_function(a, canvas, "GetHudScaleFactor")?;
            let expected = find_function(a, "Engine.Canvas", "Core.Class", "GetHudScaleFactor")?;
            if scale != expected || expected.is_null() { return Ok(false); }
            texture(a, manager, "textureTimerEmpty")?; texture(a, manager, "textureFullBar")?;
            f32_of(a, manager, "fFULL_OFFSET_X")?; f32_of(a, manager, "fFULL_OFFSET_Y")?;
        }
        Ok(true)
    }
}
unsafe fn paint(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject, phase: TimerPhase) -> RuntimeResult<()> {
    // SAFETY: no borrow of retained state is held across native/script execution.
    unsafe {
        if !live(a, manager) || !live(a, canvas) { return Err(RuntimeError::new("Timer receiver/Canvas expired")); }
        let w = int_of(a, canvas, "SizeX")?; let h = int_of(a, canvas, "SizeY")?;
        let mut retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut(); views.retain(|view| view.receiver.live(a));
            views.iter().position(|view| view.receiver.p == manager).map(|i| views.swap_remove(i))
        }).unwrap_or_else(|| {
            let store = Store::new(); let current = store.signal(phase.clone()); let observed = current.clone(); let metrics = HostMetrics::default();
            TimerUi { receiver: Weak::new(manager), model: current, ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics }
        });
        if let TimerPhase::Numeric { text, extent, .. } = &phase { retained.metrics.learn(text, Font::Slot(0), *extent); }
        retained.model.set(phase); retained.ui.resize(w as f32, h as f32); retained.ui.relayout();
        let font = current_font(a, canvas)?;
        let target = TimerTarget(CanvasTarget { canvas, tile_style: None, fonts: [font; 4] });
        let result = render_commands(&target, a, &retained.ui.draw(), &retained.metrics);
        if retained.receiver.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}
unsafe fn draw_numeric(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: this is the source order, including SetPos BEFORE the duration callback.
    unsafe {
        if !read_boolean(a, manager, "bShowNumericTime")? { return Ok(()); }
        let target = CanvasTarget { canvas, tile_style: None, fonts: [current_font(a, canvas)?; 4] };
        target.set_pos(a, int_of(a, canvas, "SizeX")?.wrapping_sub(75) as f32, int_of(a, canvas, "SizeY")?.wrapping_sub(60) as f32)?;
        let mut duration = 0.;
        invoke_virtual(a, manager, "GetTimerDuration", &[], &[], (&raw mut duration).cast())?;
        if !live(a, manager) || !live(a, canvas) { return Err(RuntimeError::new("Timer expired during duration callback")); }
        let text = ((duration - f32_of(a, manager, "fCountdownTime")?) as i32).to_string();
        let font = current_font(a, canvas)?;
        if !live(a, font) { return Err(RuntimeError::new("Timer numeric font unavailable")); }
        let target = CanvasTarget { canvas, tile_style: None, fonts: [font; 4] };
        let extent = target.text_size(a, &text, Font::Slot(0))?;
        let phase = TimerPhase::Numeric { text, position: (f32_of(a, canvas, "CurX")?, f32_of(a, canvas, "CurY")?), extent, color: canvas_color(a, canvas)? };
        paint(a, manager, canvas, phase)
    }
}
unsafe fn draw(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: admitted callbacks execute at their source points, with resources re-read afterward.
    unsafe {
        let mut scale = 0.; invoke_virtual(a, canvas, "GetHudScaleFactor", &[], &[], (&raw mut scale).cast())?;
        let (x, y) = timer_origin(int_of(a, canvas, "SizeX")?, int_of(a, canvas, "SizeY")?, scale);
        let empty = texture(a, manager, "textureTimerEmpty")?;
        let (w, h) = (int_of(a, empty, "USize")? as f32, int_of(a, empty, "VSize")? as f32);
        paint(a, manager, canvas, TimerPhase::Background(Art { texture: TextureRef(format!("#{}", (*empty).index)), destination: Rect::new(x, y, w * scale, h * scale), source: Rect::new(0., 0., w, h), color: canvas_color(a, canvas)? }))?;
        // The numerator is evaluated BEFORE the callback; numeric mode reverses that order.
        let remaining = f32_of(a, manager, "fCountdownTime")?;
        let mut duration = 0.; invoke_virtual(a, manager, "GetTimerDuration", &[], &[], (&raw mut duration).cast())?;
        if !live(a, manager) || !live(a, canvas) { return Err(RuntimeError::new("Timer expired during duration callback")); }
        let width = timer_segment(remaining, duration);
        let x = x + f32_of(a, manager, "fFULL_OFFSET_X")? * scale;
        let y = y + f32_of(a, manager, "fFULL_OFFSET_Y")? * scale;
        let full = texture(a, manager, "textureFullBar")?;
        let dest_h = int_of(a, full, "VSize")? as f32 * scale;
        let source_h = int_of(a, full, "VSize")? as f32;
        paint(a, manager, canvas, TimerPhase::Fill(Art { texture: TextureRef(format!("#{}", (*full).index)), destination: Rect::new(x, y, width * scale, dest_h), source: Rect::new(0., 0., width, source_h), color: canvas_color(a, canvas)? }))?;
        let tuning = virtual_function(a, manager, "DrawTuningModeData")?;
        let canonical = find_function(a, "HGame.CountdownTimerManager", "Core.Class", "DrawTuningModeData")?;
        if tuning == canonical { draw_numeric(a, manager, canvas)?; }
        else { invoke_resolved(a, manager, tuning, &[("Canvas", V::Obj(canvas))], &[], ptr::null_mut())?; }
        Ok(())
    }
}
unsafe fn callback(a: CoreAccess<'_>, manager: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a, manager) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: observer parameters are canonical and only read during admission.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        let numeric = crate::ui_host::name_of(a, function.cast()) == "DrawTuningModeData";
        match admits(a, manager, canvas, numeric) {
            Ok(true) => {}, Ok(false) => return Ok(ScriptPreCallDecision::Continue),
            Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
        }
        if numeric { draw_numeric(a, manager, canvas)?; } else { draw(a, manager, canvas)?; }
        Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: metadata remains owned by the observer guard.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let mut functions = Vec::new();
        for name in ["DrawCountdown", "DrawTuningModeData"] {
            let f = find_function(a, "HGame.CountdownTimerManager", "Core.Class", name)?;
            if !f.is_null() { functions.push(f); }
        }
        if functions.is_empty() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-countdown", &functions, callback)?))
    }
}
#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    let art = Art { texture: TextureRef("#7".to_owned()), destination: Rect::new(854., 588., -59., 32.), source: Rect::new(0., 0., -29.5, 16.), color: [17, 33, 65, 73] };
    let store = Store::new(); let current = store.signal(TimerPhase::Fill(art)); let observed = current.clone(); let metrics = HostMetrics::default();
    metrics.learn("-7", Font::Slot(0), (12., 16.));
    let mut ui = Ui::new(store, move || view(&observed.get()), metrics); ui.resize(1280., 720.);
    assert!(matches!(&ui.draw()[0], DrawCmd::Image { rect, fit: Fit::Region(_, _, width, _), color, .. }
        if rect.w == -59. && *width == -29.5 && *color == Color(17, 33, 65, 73)));
    current.set(TimerPhase::Numeric { text: "-7".to_owned(), position: (1205., 660.), extent: (12., 16.), color: [17, 33, 65, 73] });
    let commands = ui.draw();
    assert!(matches!(&commands[0], DrawCmd::Text { rect, text, color, .. }
        if text == "-7" && *rect == Rect::new(1205., 660., 12., 16.) && *color == Color(17, 33, 65, 73)));
    assert!(!commands.iter().any(|cmd| matches!(cmd, DrawCmd::Image { .. })));
}
