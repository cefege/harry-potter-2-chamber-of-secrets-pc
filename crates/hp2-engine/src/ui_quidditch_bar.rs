//! Quidditch presentation, with script retaining progress, flashing and lifecycle policy.
use std::{cell::RefCell, ffi::c_void, ptr};

use hp2_abi::engine::generated::{UBoolProperty, UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};

use super::{CanvasState, CanvasTarget, f32_of, find_function, int_of, param_object};
use crate::{stock_graphics_menu::{Weak, field, is_a, live, object, prop, read_boolean}, ui_host::{DrawTarget, HostMetrics, find_class, render_commands}, ui_vm::{V, invoke_virtual}};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Art {
    texture: TextureRef,
    dest: Rect,
    source: Rect,
    color: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
struct Text {
    value: String,
    pos: (f32, f32),
    size: (f32, f32),
    color: [u8; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
struct View {
    art: Vec<Art>,
    text: Vec<Text>,
}

struct Retained {
    receiver: Weak,
    phase: u8,
    model: Signal<View>,
    ui: Ui,
    metrics: HostMetrics,
}

thread_local! {
    static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) };
}

fn view(model: &View) -> Node {
    let mut children = Vec::with_capacity(model.art.len() + model.text.len());
    for art in &model.art {
        children.push(image(art.texture.clone(), Fit::Region(art.source.x, art.source.y, art.source.w, art.source.h))
            .at(art.dest.x, art.dest.y).width(Size::Px(art.dest.w)).height(Size::Px(art.dest.h))
            .color(Color(art.color[0], art.color[1], art.color[2], art.color[3])).blend(BlendMode::Inherit));
    }
    for text in &model.text {
        children.push(label(&text.value).at(text.pos.0, text.pos.1)
            .width(Size::Px(text.size.0)).height(Size::Px(text.size.1)).font(Font::Slot(0))
            .color(Color(text.color[0], text.color[1], text.color[2], text.color[3])));
    }
    col().children(children)
}

/// These leaves inherit Style; unlike the common HUD text target they never force masked text.
struct Target {
    canvas: CanvasTarget,
    cr: bool,
}
impl DrawTarget for Target {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> {
        Err(RuntimeError::new("Unexpected sports fill"))
    }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        unsafe { self.canvas.texture_object(a, name) }
    }
    unsafe fn segment(&self, a: CoreAccess<'_>, texture: *mut UObject, dest: Rect, src: Rect, color: [u8; 4], blend: BlendMode) -> RuntimeResult<()> {
        unsafe { self.canvas.segment(a, texture, dest, src, color, blend) }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, font: Font, color: [u8; 4]) -> RuntimeResult<()> {
        // SAFETY: the complete route preflight validated the live Canvas and font.
        unsafe {
            self.canvas.set_font(a, font)?;
            self.canvas.set_color(a, color)?;
            self.canvas.set_pos(a, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.canvas.canvas.cast(), &units, self.cr, 0)
        }
    }
}

unsafe fn paint(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, phase: u8, model: View, cr: bool) -> RuntimeResult<()> {
    // SAFETY: callers admit all fields and resources before their first effect;
    // no retained RefCell borrow crosses a native or script call.
    unsafe {
        let font = object(a, canvas, "Font")?;
        let target = Target { canvas: CanvasTarget { canvas, tile_style: None, fonts: [font; 4] }, cr };
        let retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|v| v.receiver.live(a));
            views.iter().position(|v| v.receiver.p == receiver && v.phase == phase).map(|i| views.swap_remove(i))
        });
        let metrics = retained.as_ref().map_or_else(HostMetrics::default, |v| v.metrics.clone());
        for text in &model.text {
            metrics.learn(&text.value, Font::Slot(0), text.size);
        }
        let mut retained = retained.unwrap_or_else(|| {
            let store = Store::new();
            let current = store.signal(model.clone());
            let observed = current.clone();
            Retained { receiver: Weak::new(receiver), phase, model: current,
                ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics: metrics.clone() }
        });
        retained.model.set(model);
        retained.ui.resize(int_of(a, canvas, "SizeX")? as f32, int_of(a, canvas, "SizeY")? as f32);
        retained.ui.relayout();
        let result = render_commands(&target, a, &retained.ui.draw(), &metrics);
        if retained.receiver.live(a) {
            VIEWS.with(|views| views.borrow_mut().push(retained));
        }
        result
    }
}

pub(super) unsafe fn color(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<[u8; 4]> {
    unsafe { Ok(prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned()) }
}
pub(super) unsafe fn set_color(a: CoreAccess<'_>, canvas: *mut UObject, value: [u8; 4]) -> RuntimeResult<()> {
    unsafe { prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(value) };
    Ok(())
}
pub(super) unsafe fn set_font(a: CoreAccess<'_>, canvas: *mut UObject, font: *mut UObject) -> RuntimeResult<()> {
    unsafe { prop(a, canvas, "Font", "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(font) };
    Ok(())
}
pub(super) unsafe fn set_pos(a: CoreAccess<'_>, canvas: *mut UObject, x: f32, y: f32) -> RuntimeResult<()> {
    unsafe { CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] }.set_pos(a, x, y) }
}

pub(super) unsafe fn validate_canvas(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<()> {
    if !live(a, canvas) { return Err(RuntimeError::new("Sports Canvas unavailable")); }
    unsafe {
        if !is_a(a, canvas, a.native_class("Engine.Canvas")?) { return Err(RuntimeError::new("Unsupported sports Canvas")); }
        CanvasState::save(a, canvas)?;
        for name in ["CurYL", "OrgX", "OrgY", "ClipX", "ClipY", "SpaceX", "SpaceY"] { f32_of(a, canvas, name)?; }
        int_of(a, canvas, "SizeX")?;
        int_of(a, canvas, "SizeY")?;
    }
    Ok(())
}

pub(super) unsafe fn resolve(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    if !live(a, receiver) { return Err(RuntimeError::new("Sports receiver unavailable")); }
    unsafe { a.find_object_function_by_name(receiver, a.intern_str(name, FindName::Find)?, false) }
}

pub(super) unsafe fn canonical(a: CoreAccess<'_>, receiver: *mut UObject, name: &str, owners: &[(&str, &str)]) -> RuntimeResult<bool> {
    let actual = unsafe { resolve(a, receiver, name)? };
    if actual.is_null() { return Ok(false); }
    for &(owner, kind) in owners {
        let expected = unsafe { find_function(a, owner, kind, name)? };
        if !expected.is_null() && expected == actual { return Ok(true); }
    }
    Ok(false)
}

pub(super) unsafe fn class_route(a: CoreAccess<'_>, receiver: *mut UObject, path: &str) -> RuntimeResult<bool> {
    let Some(class) = (unsafe { find_class(a, path)? }) else { return Ok(false) };
    Ok(live(a, receiver) && unsafe { is_a(a, receiver, class.cast()) })
}

pub(super) unsafe fn validate_args(a: CoreAccess<'_>, function: *mut UFunction, args: &[(&str, &str, usize)]) -> RuntimeResult<()> {
    if function.is_null() { return Err(RuntimeError::new("Sports callback unavailable")); }
    for &(name, kind, width) in args {
        unsafe { field(a, function.cast(), name, kind, width, usize::from((*function).parms_size), true)?; }
    }
    Ok(())
}

pub(super) unsafe fn menu_param(a: CoreAccess<'_>, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<bool> {
    if params.is_null() || size < unsafe { usize::from((*function).parms_size) } { return Err(RuntimeError::new("Sports parameters unavailable")); }
    unsafe {
        let offset = field(a, function.cast(), "bMenuMode", "Core.BoolProperty", 4, usize::from((*function).parms_size), true)?;
        let property = a.find_field(function.cast(), a.intern_str("bMenuMode", FindName::Find)?, a.native_class("Core.BoolProperty")?).cast::<UBoolProperty>();
        if property.is_null() || (*property).bit_mask.count_ones() != 1 { return Err(RuntimeError::new("Invalid sports menu mask")); }
        Ok(params.add(offset).cast::<u32>().read_unaligned() & (*property).bit_mask != 0)
    }
}

pub(super) unsafe fn validate_scale(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<bool> {
    unsafe {
        if !canonical(a, receiver, "GetScaleFactor", &[("HGame.HudItemManager", "Core.Class")])? { return Ok(false); }
        validate_args(a, resolve(a, receiver, "GetScaleFactor")?, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>()), ("ReturnValue", "Core.FloatProperty", 4)])?;
    }
    Ok(true)
}
pub(super) unsafe fn scale(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<f32> {
    let mut value = 0.;
    unsafe { invoke_virtual(a, receiver, "GetScaleFactor", &[("Canvas", V::Obj(canvas))], &[], (&raw mut value).cast::<c_void>())?; }
    Ok(value)
}

pub(super) unsafe fn texture(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    let value = unsafe { object(a, receiver, name)? };
    unsafe { validate_texture(a, value)?; }
    Ok(value)
}
pub(super) unsafe fn validate_texture(a: CoreAccess<'_>, texture: *mut UObject) -> RuntimeResult<()> {
    if texture.is_null() { return Ok(()); }
    if !live(a, texture) { return Err(RuntimeError::new("Sports texture unavailable")); }
    unsafe {
        if !is_a(a, texture, a.native_class("Engine.Texture")?) { return Err(RuntimeError::new("Unsupported sports texture")); }
        int_of(a, texture, "USize")?;
        int_of(a, texture, "VSize")?;
    }
    Ok(())
}

pub(super) unsafe fn icon(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, phase: u8, texture: *mut UObject, x: f32, y: f32, scale: f32) -> RuntimeResult<()> {
    unsafe {
        set_pos(a, canvas, x, y)?;
        if texture.is_null() { return Ok(()); }
        validate_texture(a, texture)?;
        let w = int_of(a, texture, "USize")? as f32;
        let h = int_of(a, texture, "VSize")? as f32;
        let art = Art { texture: TextureRef(format!("#{}", (*texture).index)), dest: Rect::new(x, y, w * scale, h * scale), source: Rect::new(0., 0., w, h), color: color(a, canvas)? };
        paint(a, receiver, canvas, phase, View { art: vec![art], text: Vec::new() }, false)
    }
}

pub(super) unsafe fn measure(a: CoreAccess<'_>, canvas: *mut UObject, text: &str) -> RuntimeResult<(f32, f32)> {
    let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
    unsafe { Ok(crate::canvas_owner::native_text_size(a, canvas.cast(), &units)?.map_or((0., 0.), |(w, h)| (w as f32, h as f32))) }
}

pub(super) unsafe fn text(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, phase: u8, value: String, pos: (f32, f32), size: (f32, f32), cr: bool) -> RuntimeResult<()> {
    unsafe {
        if value.is_empty() {
            // The host intentionally skips empty labels; Canvas still owns their cursor effects.
            set_pos(a, canvas, pos.0, pos.1)?;
            return crate::canvas_owner::draw_text(a, canvas.cast(), &[0], cr, 0);
        }
        let caption = Text { value, pos, size, color: color(a, canvas)? };
        paint(a, receiver, canvas, phase, View { art: Vec::new(), text: vec![caption] }, cr)
    }
}

fn empty_phase(current: f32, total: f32) -> bool {
    current < total / 2.
}

fn bar_geometry(width: i32, height: i32, scale: f32, percent: i32) -> (Rect, f32) {
    let x = width as f32 - scale * 132.;
    let y = height as f32 - scale * 80.;
    let ratio = (percent as f32 / 100.).clamp(0., 1.);
    (Rect::new(x + 4. * scale, y + 52. * scale, (ratio * 117.) * scale, 0.), ratio * 117.)
}

unsafe fn admit(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    unsafe {
        if !class_route(a, receiver, "HGame.QuidditchBar")? || !validate_scale(a, receiver)?
            || !canonical(a, receiver, "GetBarDrawColor", &[("HGame.QuidditchBar", "Core.Class")])? { return Ok(false); }
        validate_canvas(a, canvas)?;
        validate_args(a, resolve(a, receiver, "GetBarDrawColor")?, &[("ReturnValue", "Core.StructProperty", 4)])?;
        for name in ["textureBarEmpty", "textureBarFull", "textureBarWhite"] { texture(a, receiver, name)?; }
        if texture(a, receiver, "textureBarWhite")?.is_null() { return Ok(false); }
        read_boolean(a, receiver, "bFlashing")?;
        f32_of(a, receiver, "fFlashCurrSeconds")?;
        f32_of(a, receiver, "fFlashTotalSeconds")?;
        f32_of(a, receiver, "fFadeRedTotalSecs")?;
        f32_of(a, receiver, "fFadeRedCurrSecs")?;
        int_of(a, receiver, "nPercentFull")?;
    }
    Ok(true)
}

unsafe fn draw(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: admission checks every required route/layout/resource before scale (the first callback).
    unsafe {
        let saved_color = color(a, canvas)?;
        let scale = scale(a, receiver, canvas)?;
        validate_canvas(a, canvas)?;
        let width = int_of(a, canvas, "SizeX")?;
        let height = int_of(a, canvas, "SizeY")?;
        let x = width as f32 - scale * 132.;
        let y = height as f32 - scale * 80.;
        set_pos(a, canvas, x, y)?;
        let empty = !read_boolean(a, receiver, "bFlashing")?
            || empty_phase(f32_of(a, receiver, "fFlashCurrSeconds")?, f32_of(a, receiver, "fFlashTotalSeconds")?);
        icon(a, receiver, canvas, 0, texture(a, receiver, if empty { "textureBarEmpty" } else { "textureBarFull" })?, x, y, scale)?;
        let (mut dest, segment_width) = bar_geometry(width, height, scale, int_of(a, receiver, "nPercentFull")?);
        let mut tint = [0u8; 4];
        // This callback remains between the background and fill, even during the full flash phase.
        invoke_virtual(a, receiver, "GetBarDrawColor", &[], &[], tint.as_mut_ptr().cast())?;
        validate_canvas(a, canvas)?;
        set_color(a, canvas, tint)?;
        set_pos(a, canvas, dest.x, dest.y)?;
        if empty {
            let white = texture(a, receiver, "textureBarWhite")?;
            if white.is_null() { return Err(RuntimeError::new("Quidditch fill texture unavailable")); }
            let h = int_of(a, white, "VSize")? as f32;
            dest.h = h * scale;
            paint(a, receiver, canvas, 1, View { art: vec![Art { texture: TextureRef(format!("#{}", (*white).index)), dest, source: Rect::new(0., 0., segment_width, h), color: tint }], text: Vec::new() }, false)?;
        }
        set_color(a, canvas, saved_color)
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    let Some(canvas) = (unsafe { param_object(a, function, params, size, "Canvas") }) else { return Ok(ScriptPreCallDecision::Continue) };
    match unsafe { admit(a, receiver, canvas) } {
        Ok(true) => {},
        Ok(false) => return Ok(ScriptPreCallDecision::Continue),
        Err(error) => { super::warn(a, &error); return Ok(ScriptPreCallDecision::Continue); }
    }
    unsafe { draw(a, receiver, canvas)?; }
    Ok(ScriptPreCallDecision::Consume)
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: Core retains canonical function metadata for the observer's lifetime.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.QuidditchBar.DisplayQBar", "Core.State", "RenderHudItemManager")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-quidditch-bar", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert!(empty_phase(0.49, 1.));
    assert!(!empty_phase(0.5, 1.), "the exact half boundary paints the full flash image");
    assert!(!empty_phase(f32::NAN, 1.), "retail's less-than comparison keeps its NaN semantics");
    let (rect, source_width) = bar_geometry(800, 600, 1.25, 50);
    assert_eq!(rect, Rect::new(640., 565., 73.125, 0.));
    assert_eq!(source_width, 58.5);
    assert_eq!(bar_geometry(640, 480, 1., -1).1, 0.);
    assert_eq!(bar_geometry(640, 480, 1., 101).1, 117.);
    let model = View { art: vec![Art { texture: TextureRef("bar".into()), dest: Rect::new(1., 2., 3., 4.), source: Rect::new(5., 6., 7., 8.), color: [1, 2, 3, 4] }], text: Vec::new() };
    let mut ui = Ui::new(Store::new(), move || view(&model), HostMetrics::default());
    ui.resize(800., 600.);
    assert!(ui.draw().iter().any(|cmd| matches!(cmd, hp2_ui::DrawCmd::Image { fit: Fit::Region(5., 6., 7., 8.), color: Color(1, 2, 3, 4), blend: BlendMode::Inherit, .. })));
}
