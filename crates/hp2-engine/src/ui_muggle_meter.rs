//! Retail Muggle meter presentation; detection, eye selection and lifecycle remain script-owned.
use std::{cell::RefCell, ptr};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use hp2_ui::{BlendMode, Color, Fit, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image};

use super::{CanvasTarget, f32_of, find_function, int_of, param_object, quidditch_bar as shared};
use crate::{stock_graphics_menu::{Weak, live}, ui_host::{HostMetrics, render_commands}};

#[derive(Clone, Debug, PartialEq)]
struct Meter {
    viewport: (i32, i32),
    textures: [TextureRef; 3],
    sizes: [(f32, f32); 3],
    amount: f32,
    scale: f32,
    color: [u8; 4],
}

fn geometry(model: &Meter) -> [(Rect, Rect); 3] {
    let s = model.scale;
    let x = 25. * s;
    let y = 25. * s;
    let bottom = y + 128. * s;
    let full_height = model.amount + 14.;
    let [(ew, eh), (fw, fh), (iw, ih)] = model.sizes;
    let full = if full_height >= 128. {
        (Rect::new(x, y, fw * s, fh * s), Rect::new(0., 0., fw, fh))
    } else {
        (Rect::new(x, bottom - full_height * s, fw * s, full_height * s),
         Rect::new(0., 128. - full_height, fw, full_height))
    };
    [
        (Rect::new(x, y, ew * s, eh * s), Rect::new(0., 0., ew, eh)),
        full,
        (Rect::new(x - ((64. - 31.) / 2.) * s, (bottom - model.amount * s) - 45. * s, iw * s, ih * s),
         Rect::new(0., 0., iw, ih)),
    ]
}

fn view(model: &Meter) -> Node {
    col().children(geometry(model).into_iter().zip(&model.textures).map(|((dest, src), texture)| {
        image(texture.clone(), Fit::Region(src.x, src.y, src.w, src.h))
            .at(dest.x, dest.y).width(Size::Px(dest.w)).height(Size::Px(dest.h))
            .color(Color(model.color[0], model.color[1], model.color[2], model.color[3]))
            .blend(BlendMode::Inherit)
    }))
}

struct Retained { receiver: Weak, model: Signal<Meter>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }

unsafe fn snapshot(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, scale: f32) -> RuntimeResult<Meter> {
    // SAFETY: live objects and reflected layouts are checked before every read.
    unsafe {
        if !live(a, receiver) || !live(a, canvas) { return Err(RuntimeError::new("Muggle meter receiver/Canvas expired")); }
        let mut textures = std::array::from_fn(|_| TextureRef(String::new()));
        let mut sizes = [(0., 0.); 3];
        for (i, name) in ["textureBarEmpty", "textureBarFull", "textureCurrEye"].into_iter().enumerate() {
            let texture = shared::texture(a, receiver, name)?;
            // Null/error paths stay in the original script before any scale callback or drawing.
            if texture.is_null() { return Err(RuntimeError::new("Muggle meter texture unavailable")); }
            textures[i] = TextureRef(format!("#{}", (*texture).index));
            sizes[i] = (int_of(a, texture, "USize")? as f32, int_of(a, texture, "VSize")? as f32);
        }
        Ok(Meter { viewport: (int_of(a, canvas, "SizeX")?, int_of(a, canvas, "SizeY")?), textures, sizes,
            amount: f32_of(a, receiver, "fBarFullAmount")?, scale, color: shared::color(a, canvas)? })
    }
}

unsafe fn admit(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    // SAFETY: admission performs only metadata/field reads, never a callback or Canvas write.
    unsafe {
        if !shared::class_route(a, receiver, "HGame.MuggleMeterManager")? || !shared::validate_scale(a, receiver)? { return Ok(false); }
        shared::validate_canvas(a, canvas)?;
        for name in ["textureBarEmpty", "textureBarFull", "textureCurrEye"] {
            if shared::texture(a, receiver, name)?.is_null() { return Ok(false); }
        }
        f32_of(a, receiver, "fBarFullAmount")?;
        Ok(true)
    }
}

unsafe fn draw(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: only the inventoried, state-first canonical scale callback is admitted. No cache borrow crosses it.
    unsafe {
        let scale = shared::scale(a, receiver, canvas)?;
        let model = snapshot(a, receiver, canvas, scale)?;
        let (w, h) = model.viewport;
        let retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|v| v.receiver.live(a));
            views.iter().position(|v| v.receiver.p == receiver).map(|i| views.swap_remove(i))
        });
        let mut retained = retained.unwrap_or_else(|| {
            let store = Store::new();
            let current = store.signal(model.clone());
            let observed = current.clone();
            let metrics = HostMetrics::default();
            Retained { receiver: Weak::new(receiver), model: current,
                ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics }
        });
        retained.model.set(model);
        retained.ui.resize(w as f32, h as f32);
        retained.ui.relayout();
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        // Retail saves/restores nothing: every tile keeps its native cursor/CurYL effects.
        let result = render_commands(&target, a, &retained.ui.draw(), &retained.metrics);
        if retained.receiver.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: Core supplies live function metadata and the invocation's parameter storage.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !live(a, canvas) { return Ok(ScriptPreCallDecision::Continue); }
        match admit(a, receiver, canvas) {
            Ok(true) => {},
            Ok(false) => return Ok(ScriptPreCallDecision::Continue),
            Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
        }
        draw(a, receiver, canvas)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: the observer guard retains the canonical DetectMuggles drawing entry.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.MuggleMeterManager.DetectMuggles", "Core.State", "RenderHudItemManager")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-muggle-meter", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    let model = Meter { viewport: (800, 600), textures: ["#1", "#2", "#3"].map(|s| TextureRef(s.to_owned())),
        sizes: [(32., 128.), (31., 127.), (64., 64.)], amount: -20., scale: 1.25, color: [17, 33, 65, 73] };
    let store = Store::new();
    let current = store.signal(model);
    let observed = current.clone();
    let mut ui = Ui::new(store, move || view(&observed.get()), HostMetrics::default());
    ui.resize(800., 600.);
    let commands = ui.draw();
    assert_eq!(commands.len(), 3, "The meter composes empty bar, full region and eye");
    assert!(matches!(&commands[1], DrawCmd::Image { rect, fit: Fit::Region(_, v, w, h), color, blend: BlendMode::Inherit, .. }
        if rect.y == 198.75 && rect.h == -7.5 && *v == 134. && *w == 31. && *h == -6. && *color == Color(17, 33, 65, 73)));
    assert!(matches!(&commands[2], DrawCmd::Image { rect, .. } if rect.x == 10.625 && rect.y == 160.));
    current.update(|m| m.amount = 114.);
    let commands = ui.draw();
    assert!(matches!(&commands[1], DrawCmd::Image { rect, fit: Fit::Region(_, _, _, h), .. }
        if rect.y == 31.25 && rect.h == 158.75 && *h == 127.), "Inclusive full branch uses actual texture height, not hard-coded 128");
    current.update(|m| m.amount = 200.);
    assert!(matches!(&ui.draw()[2], DrawCmd::Image { rect, .. } if rect.y == -115.), "Eye position is not clamped");
}
