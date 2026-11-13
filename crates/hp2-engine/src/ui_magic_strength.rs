//! Complete DisplayStrength presentation; script still owns strength and visibility.
use std::{cell::RefCell, ptr};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{Color, Fit, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image};

use super::{CanvasTarget, f32_of, find_function, int_of, param_object};
use crate::{stock_graphics_menu::{Weak, field, live, object, prop}, ui_host::{HostMetrics, render_commands}, ui_vm::{V, invoke_virtual}};

#[derive(Clone, Debug, PartialEq)]
struct Meter {
    viewport: (i32, i32),
    textures: [TextureRef; 3],
    texture_sizes: [(f32, f32); 3],
    strength: f32,
    scale: f32,
    color: [u8; 4],
}

fn geometry(model: &Meter) -> [(Rect, Rect); 3] {
    let s = model.scale;
    let x = 25. * s;
    let y = 25. * s;
    let empty = 120. - model.strength;
    let (fw, fh) = model.texture_sizes[0];
    let ew = model.texture_sizes[1].0;
    let (sw, sh) = model.texture_sizes[2];
    [
        (Rect::new(x, y, fw * s, fh * s), Rect::new(0., 0., fw, fh)),
        (Rect::new(x, y, ew * s, empty * s), Rect::new(0., 0., ew, empty)),
        (Rect::new(x - ((128. - 36.) / 2.) * s, ((25. + empty) - 66.) * s, sw * s, sh * s), Rect::new(0., 0., sw, sh)),
    ]
}

fn view(model: &Meter) -> Node {
    col().children(geometry(model).into_iter().zip(&model.textures).map(|((dest, src), texture)| {
        image(texture.clone(), Fit::Region(src.x, src.y, src.w, src.h))
            .color(Color(model.color[0], model.color[1], model.color[2], model.color[3]))
            .at(dest.x, dest.y).width(Size::Px(dest.w)).height(Size::Px(dest.h))
    }))
}

struct MeterUi { receiver: Weak, model: Signal<Meter>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<MeterUi>> = const { RefCell::new(Vec::new()) }; }

unsafe fn snapshot(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, scale: f32) -> RuntimeResult<Meter> {
    // SAFETY: reflected accessors validate layouts; textures are checked before dereference.
    unsafe {
        if !live(a, receiver) || !live(a, canvas) { return Err(RuntimeError::new("Magic meter receiver/Canvas expired")); }
        let mut textures = std::array::from_fn(|_| TextureRef(String::new()));
        let mut sizes = [(0., 0.); 3];
        for (i, name) in ["textureBarFull", "textureBarEmpty", "textureSlider"].into_iter().enumerate() {
            let texture = object(a, receiver, name)?;
            if !live(a, texture) { return Err(RuntimeError::new("Magic meter texture unavailable")); }
            textures[i] = TextureRef(format!("#{}", (*texture).index));
            sizes[i] = (int_of(a, texture, "USize")? as f32, int_of(a, texture, "VSize")? as f32);
        }
        Ok(Meter {
            viewport: (int_of(a, canvas, "SizeX")?, int_of(a, canvas, "SizeY")?),
            textures,
            texture_sizes: sizes,
            strength: f32_of(a, receiver, "fRemainingStrength")?, scale,
            color: prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned(),
        })
    }
}

unsafe fn admits(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    // SAFETY: no script calls or writes occur during admission.
    unsafe {
        let expected = find_function(a, "HGame.HudItemManager", "Core.Class", "GetScaleFactor")?;
        let resolved = a.find_object_function_by_name(receiver, a.intern_str("GetScaleFactor", FindName::Find)?, false)?;
        if expected.is_null() || expected != resolved { return Ok(false); }
        for name in ["textureBarFull", "textureBarEmpty", "textureSlider"] {
            let texture = object(a, receiver, name)?;
            if !live(a, texture) { return Ok(false); }
            int_of(a, texture, "USize")?; int_of(a, texture, "VSize")?;
        }
        int_of(a, canvas, "SizeX")?; int_of(a, canvas, "SizeY")?;
        f32_of(a, receiver, "fRemainingStrength")?;
        prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?;
        field(a, resolved.cast(), "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>(), usize::from((*resolved).parms_size), true)?;
        for name in ["CurX", "CurY", "CurYL", "OrgX", "OrgY", "ClipX", "ClipY"] { f32_of(a, canvas, name)?; }
        prop(a, canvas, "Style", "Core.ByteProperty", 1)?;
        Ok(true)
    }
}

unsafe fn draw(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: the admitted canonical scale callback executes before all presentation reads.
    unsafe {
        let mut scale = 0.;
        invoke_virtual(a, receiver, "GetScaleFactor", &[("Canvas", V::Obj(canvas))], &[], (&raw mut scale).cast())?;
        scale *= crate::display_options::hud_scale(a);
        let model = snapshot(a, receiver, canvas, scale)?;
        let (w, h) = model.viewport;
        let mut retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|view| view.receiver.live(a));
            views.iter().position(|view| view.receiver.p == receiver).map(|i| views.swap_remove(i))
        }).unwrap_or_else(|| {
            let store = Store::new(); let current = store.signal(model.clone()); let observed = current.clone();
            let metrics = HostMetrics::default();
            MeterUi { receiver: Weak::new(receiver), model: current, ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics }
        });
        retained.model.set(model); retained.ui.resize(w as f32, h as f32); retained.ui.relayout();
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        // Source makes no Font/DrawColor/Style/cursor restoration here.
        let result = render_commands(&target, a, &retained.ui.draw(), &retained.metrics);
        if retained.receiver.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: observer parameters belong to the live canonical invocation.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !live(a, canvas) { return Ok(ScriptPreCallDecision::Continue); }
        match admits(a, receiver, canvas) {
            Ok(true) => {}, Ok(false) => return Ok(ScriptPreCallDecision::Continue),
            Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
        }
        draw(a, receiver, canvas)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: the guard retains the selected canonical function metadata.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.MagicStrengthManager.DisplayStrength", "Core.State", "RenderHudItemManager")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-magic-strength", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    let model = Meter { viewport: (800, 600), textures: ["#1", "#2", "#3"].map(|v| TextureRef(v.to_owned())), texture_sizes: [(128., 128.), (128., 128.), (128., 66.)], strength: 150., scale: 1.25, color: [17, 33, 65, 73] };
    let store = Store::new(); let current = store.signal(model); let observed = current.clone();
    let mut ui = Ui::new(store, move || view(&observed.get()), HostMetrics::default()); ui.resize(800., 600.);
    let commands = ui.draw();
    assert!(matches!(&commands[1], DrawCmd::Image { rect, fit: Fit::Region(_, _, _, height), color, .. }
        if rect.h == -37.5 && *height == -30. && *color == Color(17, 33, 65, 73)));
    assert!(matches!(&commands[2], DrawCmd::Image { rect, .. } if rect.x == -26.25 && rect.y == -88.75));
    current.update(|model| model.strength = -10.);
    let commands = ui.draw();
    assert!(matches!(&commands[1], DrawCmd::Image { rect, fit: Fit::Region(_, _, _, height), .. }
        if rect.h == 162.5 && *height == 130.));
    assert!(matches!(&commands[2], DrawCmd::Image { rect, .. } if rect.y == 111.25));
}
