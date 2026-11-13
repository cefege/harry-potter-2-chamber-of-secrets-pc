//! Retail hoops caption and extra wand icon, distinct from SpellSelector presentation.
use std::{cell::RefCell, ptr};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};

use super::{CanvasTarget, find_function, int_of, param_object, quidditch_bar as shared};
use crate::{stock_graphics_menu::{Weak, field, is_a, live, object}, ui_host::{DrawTarget, HostMetrics, render_commands}, ui_vm::invoke_virtual};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Hoops, Spell }

#[derive(Clone, Debug, PartialEq)]
enum View {
    Hoops { value: String, pos: (f32, f32), size: (f32, f32), color: [u8; 4] },
    Spell { texture: TextureRef, rect: Rect, size: (f32, f32), color: [u8; 4] },
}

fn hoops_pos(height: i32, number: i32) -> (f32, f32) {
    // Oy is an integer local; both integer operations wrap before conversion to float.
    (if number < 10 { 102. } else { 93. }, height.wrapping_sub(156).wrapping_add(100) as f32)
}

fn spell_pos(height: i32) -> (f32, f32) {
    // Retail subtracts 64 as an integer, converts, then subtracts float(5).
    (5., height.wrapping_sub(64) as f32 - 5.)
}

fn view(model: &View) -> Node {
    let node = match model {
        View::Hoops { value, pos, size, color } => label(value).at(pos.0, pos.1)
            .width(Size::Px(size.0)).height(Size::Px(size.1)).font(Font::Slot(0))
            .color(Color(color[0], color[1], color[2], color[3])),
        View::Spell { texture, rect, size, color } => image(texture.clone(), Fit::Region(0., 0., size.0, size.1))
            .at(rect.x, rect.y).width(Size::Px(rect.w)).height(Size::Px(rect.h))
            .color(Color(color[0], color[1], color[2], color[3])).blend(BlendMode::Inherit),
    };
    col().child(node)
}

struct Retained { receiver: Weak, kind: Kind, model: Signal<View>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }

struct Target { canvas: CanvasTarget }
impl DrawTarget for Target {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> {
        Err(RuntimeError::new("Unexpected extra HUD fill"))
    }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        // SAFETY: texture lookup is synchronous and canonical CanvasTarget validates identity.
        unsafe { self.canvas.texture_object(a, name) }
    }
    unsafe fn segment(&self, a: CoreAccess<'_>, texture: *mut UObject, dest: Rect, src: Rect, color: [u8; 4], blend: BlendMode) -> RuntimeResult<()> {
        // SAFETY: admitted resources stay live throughout this callback-free presentation.
        unsafe { self.canvas.segment(a, texture, dest, src, color, blend) }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, _: Font, _: [u8; 4]) -> RuntimeResult<()> {
        // SAFETY: DrawHoops inherits the current Font/Style/DrawColor, including bitmap cursor behavior.
        unsafe {
            self.canvas.set_pos(a, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.canvas.canvas.cast(), &units, false, 0)
        }
    }
}

unsafe fn paint(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, kind: Kind, model: View) -> RuntimeResult<()> {
    // SAFETY: no RefCell borrow crosses Canvas calls; every invocation refreshes live measurements/resources.
    unsafe {
        let retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|v| v.receiver.live(a));
            views.iter().position(|v| v.receiver.p == receiver && v.kind == kind).map(|i| views.swap_remove(i))
        });
        let metrics = retained.as_ref().map_or_else(HostMetrics::default, |v| v.metrics.clone());
        if let View::Hoops { value, size, .. } = &model { metrics.learn(value, Font::Slot(0), *size); }
        let mut retained = retained.unwrap_or_else(|| {
            let store = Store::new();
            let current = store.signal(model.clone());
            let observed = current.clone();
            Retained { receiver: Weak::new(receiver), kind, model: current,
                ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics: metrics.clone() }
        });
        retained.model.set(model);
        retained.ui.resize(int_of(a, canvas, "SizeX")? as f32, int_of(a, canvas, "SizeY")? as f32);
        retained.ui.relayout();
        let target = Target { canvas: CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] } };
        // Neither retail entry saves/restores Canvas fields.
        let result = render_commands(&target, a, &retained.ui.draw(), &retained.metrics);
        if retained.receiver.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}

unsafe fn int_param(a: CoreAccess<'_>, function: *mut UFunction, params: *const u8, size: usize, name: &str) -> RuntimeResult<i32> {
    // SAFETY: the caller owns live metadata/params; field checks width, argument flag and bounds.
    unsafe {
        let expected = usize::from((*function).parms_size);
        if params.is_null() || size < expected { return Err(RuntimeError::new("Extra HUD parameters unavailable")); }
        let offset = field(a, function.cast(), name, "Core.IntProperty", 4, expected, true)?;
        Ok(params.add(offset).cast::<i32>().read_unaligned())
    }
}

unsafe fn spell_weapon(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<Option<*mut UObject>> {
    // SAFETY: failed casts/null contexts stay stock-owned, retaining their original diagnostics.
    unsafe {
        let owner = object(a, receiver, "Owner")?;
        if !is_a(a, owner, a.native_class("Engine.PlayerPawn")?) { return Ok(None); }
        let weapon = object(a, owner, "Weapon")?;
        if !shared::class_route(a, weapon, "HGame.baseWand")? { return Ok(None); }
        if !shared::canonical(a, weapon, "GetSpellIcon", &[("HGame.baseWand", "Core.Class")])? { return Ok(None); }
        shared::validate_args(a, shared::resolve(a, weapon, "GetSpellIcon")?, &[("ReturnValue", "Core.ObjectProperty", size_of::<*mut UObject>())])?;
        Ok(Some(weapon))
    }
}

unsafe fn draw_spell(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, weapon: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: only the actual state-first canonical getter is admitted, not arbitrary ABI-compatible overrides.
    unsafe {
        let mut texture: *mut UObject = ptr::null_mut();
        invoke_virtual(a, weapon, "GetSpellIcon", &[], &[], (&raw mut texture).cast())?;
        if !live(a, receiver) || !live(a, canvas) { return Err(RuntimeError::new("Extra HUD receiver/Canvas expired")); }
        if texture.is_null() { return Ok(()); }
        shared::validate_texture(a, texture)?;
        let size = (int_of(a, texture, "USize")? as f32, int_of(a, texture, "VSize")? as f32);
        let (x, y) = spell_pos(int_of(a, canvas, "SizeY")?);
        paint(a, receiver, canvas, Kind::Spell, View::Spell { texture: TextureRef(format!("#{}", (*texture).index)),
            rect: Rect::new(x, y, size.0, size.1), size, color: shared::color(a, canvas)? })
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: Core supplies this invocation's metadata and parameter storage. Preflight is read-only.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !live(a, canvas) { return Ok(ScriptPreCallDecision::Continue); }
        let prepared = (|| -> RuntimeResult<Option<(Kind, Option<*mut UObject>, i32, i32)>> {
            if !shared::class_route(a, receiver, "HGame.HPHud")? { return Ok(None); }
            shared::validate_canvas(a, canvas)?;
            if function == find_function(a, "HGame.HPHud", "Core.Class", "DrawHoops")? {
                let number = int_param(a, function, params, size, "iNumber")?;
                let maximum = int_param(a, function, params, size, "iMaxNumber")?;
                if !live(a, object(a, canvas, "Font")?) { return Ok(None); }
                Ok(Some((Kind::Hoops, None, number, maximum)))
            } else {
                Ok(spell_weapon(a, receiver)?.map(|weapon| (Kind::Spell, Some(weapon), 0, 0)))
            }
        })();
        let (kind, weapon, number, maximum) = match prepared {
            Ok(Some(value)) => value,
            Ok(None) => return Ok(ScriptPreCallDecision::Continue),
            Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
        };
        match kind {
            Kind::Hoops => {
                let value = format!("{number}/{maximum}");
                let pos = hoops_pos(int_of(a, canvas, "SizeY")?, number);
                let measured = shared::measure(a, canvas, &value)?;
                paint(a, receiver, canvas, kind, View::Hoops { value, pos, size: measured, color: shared::color(a, canvas)? })?;
            }
            Kind::Spell => draw_spell(a, receiver, canvas, weapon.ok_or_else(|| RuntimeError::new("Extra HUD weapon unavailable"))?)?,
        }
        Ok(ScriptPreCallDecision::Consume)
    }
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: observer registration holds the declared HPHud functions, not SpellSelector's same-named helper.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let mut functions = Vec::with_capacity(2);
        for name in ["DrawHoops", "DrawSpellIcon"] {
            let f = find_function(a, "HGame.HPHud", "Core.Class", name)?;
            if !f.is_null() { functions.push(f); }
        }
        if functions.is_empty() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-extra-hud", &functions, callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    assert_eq!(hoops_pos(600, 9), (102., 544.));
    assert_eq!(hoops_pos(600, 10), (93., 544.));
    assert_eq!(hoops_pos(40, -2), (102., -16.));
    assert_eq!(hoops_pos(i32::MIN, 10).1, i32::MIN.wrapping_sub(156).wrapping_add(100) as f32);
    assert_eq!(spell_pos(600), (5., 531.));
    assert_eq!(spell_pos(i32::MIN).1, i32::MIN.wrapping_sub(64) as f32 - 5.);
    let metrics = HostMetrics::default();
    metrics.learn("-2/17", Font::Slot(0), (41., 13.));
    let store = Store::new();
    let current = store.signal(View::Hoops { value: "-2/17".to_owned(), pos: hoops_pos(40, -2), size: (41., 13.), color: [17, 33, 65, 73] });
    let observed = current.clone();
    let mut ui = Ui::new(store, move || view(&observed.get()), metrics);
    ui.resize(800., 600.);
    assert!(matches!(&ui.draw()[0], DrawCmd::Text { text, rect, color, .. }
        if text == "-2/17" && rect.x == 102. && rect.y == -16. && *color == Color(17, 33, 65, 73)));
    current.set(View::Spell { texture: TextureRef("#123".to_owned()), rect: Rect::new(5., 531., 71., 93.), size: (71., 93.), color: [17, 33, 65, 73] });
    let commands = ui.draw();
    assert_eq!(commands.len(), 1, "Extra icon contains no selector labels or fixed-size proxy");
    assert!(matches!(&commands[0], DrawCmd::Image { rect, fit: Fit::Region(_, _, w, h), color, blend: BlendMode::Inherit, .. }
        if rect.x == 5. && rect.y == 531. && rect.w == 71. && rect.h == 93. && *w == 71. && *h == 93. && *color == Color(17, 33, 65, 73)));
}
