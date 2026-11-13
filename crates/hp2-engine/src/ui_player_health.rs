//! Retail health bolts, admitting only the inventoried base/Hold/Fade color callbacks.
use std::{cell::RefCell, ffi::c_void, ptr};

use hp2_abi::engine::generated::{FStateFrame, UField, UFunction, UObject, UState, UStructProperty};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Fit, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image};

use super::{CanvasTarget, find_function, int_of, param_object};
use crate::{stock_graphics_menu::{Weak, field, is_a, live, object, prop, read_boolean}, ui_host::{HostMetrics, find_class, render_commands}, ui_vm::invoke_virtual};

/// An image phase; signed rectangles must not pass through flex sizing or clipping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Picture {
    pub texture: i32,
    pub dest: Rect,
    pub source: Rect,
    pub color: [u8; 4],
    pub blend: BlendMode,
}

fn view(picture: &Picture) -> Node {
    let Rect { x, y, w, h } = picture.dest;
    let source = picture.source;
    let [r, g, b, a] = picture.color;
    col().child(image(TextureRef(format!("#{}", picture.texture)), Fit::Region(source.x, source.y, source.w, source.h))
        .color(Color(r, g, b, a)).blend(picture.blend)
        .at(x, y).width(Size::Px(w)).height(Size::Px(h)))
}

struct Retained {
    receiver: Weak,
    canvas: Weak,
    model: Signal<Picture>,
    ui: Ui,
    metrics: HostMetrics,
}
thread_local! {
    static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) };
}

/// Paints one ordered image phase using the retained whole-component UI.
/// # Safety
/// Receiver, Canvas and picture texture must be live canonical objects.
pub(super) unsafe fn paint_picture(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, picture: Picture) -> RuntimeResult<()> {
    // SAFETY: callers validate all objects before this synchronous native paint.
    unsafe {
        let mut retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|view| view.receiver.live(a) && view.canvas.live(a) && !read_boolean(a, view.receiver.p, "bDeleteMe").unwrap_or(true));
            views.iter().position(|view| view.receiver.p == receiver && view.canvas.p == canvas)
                .map(|index| views.swap_remove(index))
        }).unwrap_or_else(|| {
            let store = Store::new();
            let model = store.signal(picture);
            let observed = model.clone();
            let metrics = HostMetrics::default();
            Retained { receiver: Weak::new(receiver), canvas: Weak::new(canvas), model,
                ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics }
        });
        retained.model.set(picture);
        retained.ui.resize(int_of(a, canvas, "SizeX")? as f32, int_of(a, canvas, "SizeY")? as f32);
        retained.ui.relayout();
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        // No VIEWS borrow survives the native calls (including synchronous reentry).
        let result = render_commands(&target, a, &retained.ui.draw(), &retained.metrics);
        if retained.receiver.live(a) && retained.canvas.live(a) && !read_boolean(a, retained.receiver.p, "bDeleteMe").unwrap_or(true) {
            VIEWS.with(|views| views.borrow_mut().push(retained));
        }
        result
    }
}

/// Checks the reflected parameter layout before an original script callback.
/// # Safety
/// Function must be indexed live metadata supplied by Core.
pub(super) unsafe fn signature(a: CoreAccess<'_>, function: *mut UFunction, args: &[(&str, &str, usize)], result: Option<(&str, usize)>) -> RuntimeResult<bool> {
    if !live(a, function.cast()) { return Ok(false); }
    // SAFETY: Core owns live function/property metadata throughout this check.
    unsafe {
        let size = usize::from((*function).parms_size);
        if usize::from((*function).num_parms) != args.len() + usize::from(result.is_some()) { return Ok(false); }
        for &(name, kind, width) in args {
            field(a, function.cast(), name, kind, width, size, true)?;
        }
        if let Some((kind, width)) = result {
            let offset = field(a, function.cast(), "ReturnValue", kind, width, size, true)?;
            if usize::from((*function).return_value_offset) != offset { return Ok(false); }
            if kind == "Core.StructProperty" {
                let property = a.find_field(function.cast(), a.intern_str("ReturnValue", FindName::Find)?, a.native_class(kind)?).cast::<UStructProperty>();
                let structure = (*property).r#struct;
                if structure.is_null() || !a.object_path_text(structure.cast())?.into_iter().eq("Core.Object.Color".chars().map(u32::from)) { return Ok(false); }
            }
        } else if (*function).return_value_offset != u16::MAX { return Ok(false); }
        Ok(true)
    }
}

/// Resolves the current state override, never a class-only approximation.
/// # Safety
/// Receiver must be a live canonical object.
pub(super) unsafe fn virtual_function(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    // SAFETY: caller established receiver liveness; Core owns its metadata.
    unsafe { a.find_object_function_by_name(receiver, a.intern_str(name, FindName::Find)?, false) }
}

/// Checks the Canvas fields used by the image bridge, without changing them.
/// # Safety
/// Canvas must be a live indexed object.
pub(super) unsafe fn canvas_ready(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<bool> {
    // SAFETY: reflected accessors check each field's kind and allocation bounds.
    unsafe {
        let Some(class) = find_class(a, "Engine.Canvas")? else { return Ok(false); };
        if !is_a(a, canvas, class) { return Ok(false); }
        for name in ["SizeX", "SizeY"] { int_of(a, canvas, name)?; }
        for name in ["CurX", "CurY", "CurYL", "SpaceX", "SpaceY", "OrgX", "OrgY", "ClipX", "ClipY"] {
            prop(a, canvas, name, "Core.FloatProperty", 4)?;
        }
        prop(a, canvas, "Style", "Core.ByteProperty", 1)?;
        prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?;
        Ok(true)
    }
}

/// Reads dimensions only after checking the live texture and its actual type.
/// # Safety
/// The owner of texture must have been revalidated since the last callback.
pub(super) unsafe fn texture_size(a: CoreAccess<'_>, texture: *mut UObject) -> RuntimeResult<(f32, f32)> {
    // SAFETY: type/liveness checks precede texture/property dereferences.
    unsafe {
        let Some(class) = find_class(a, "Engine.Texture")? else { return Err(RuntimeError::new("Missing Texture class")); };
        if !is_a(a, texture, class) { return Err(RuntimeError::new("HUD texture unavailable")); }
        Ok((int_of(a, texture, "USize")? as f32, int_of(a, texture, "VSize")? as f32))
    }
}

/// # Safety
/// Canvas must be live and its reflected DrawColor validated.
pub(super) unsafe fn color(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<[u8; 4]> {
    // SAFETY: the checked reflected Color slot is four readable bytes.
    unsafe { Ok(prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned()) }
}

/// # Safety
/// All three objects must remain live throughout this synchronous native phase.
pub(super) unsafe fn icon(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, texture: *mut UObject, x: f32, y: f32, scale: f32) -> RuntimeResult<()> {
    // SAFETY: the canonical SetPos assignment happens even for a null DrawIcon.
    unsafe {
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        target.set_pos(a, x, y)?;
        if texture.is_null() { return Ok(()); }
        let (w, h) = texture_size(a, texture)?;
        paint_picture(a, receiver, canvas, Picture { texture: (*texture).index, dest: Rect::new(x, y, w * scale, h * scale), source: Rect::new(0., 0., w, h), color: color(a, canvas)?, blend: BlendMode::Inherit })
    }
}

fn icon_count(potential: i32, units: i32) -> i32 {
    let count = if units == 0 { 0 } else { potential.wrapping_div(units) };
    // Retail converts both integers to float before its fmod native.
    let remainder = ((potential as f32 as f64) % (units as f32 as f64)) as f32;
    if remainder > 0. { count.wrapping_add(1) } else { count }
}
fn affected(count: i32, units: i32, index: i32) -> bool {
    let ratio = count as f32 / units as f32;
    ratio >= index as f32 && ratio <= index.wrapping_add(1) as f32
}
fn fill_rect(x: i32, y: i32, scale: f32, ratio: f32, width: i32, height: i32) -> (Rect, Rect) {
    let segment = ratio * height.wrapping_sub(12) as f32;
    let start = height.wrapping_sub(11) as f32 - segment;
    let segment = segment + 11.;
    (Rect::new(x as f32, y as f32 + start * scale, width as f32 * scale, segment * scale), Rect::new(0., start, width as f32, segment))
}

// Pure ancestry query shared with the color-policy regression fixture. Metadata
// access stays in normal_display; no native is dispatched through execute_event.
fn state_contains<S: Copy>(mut current: Option<S>, requested: i32, name: impl Fn(S) -> i32, parent: impl Fn(S) -> Option<S>) -> bool {
    while let Some(state) = current {
        if name(state) == requested { return true; }
        current = parent(state);
    }
    false
}

unsafe fn normal_display(a: CoreAccess<'_>, item: *mut UObject, requested: i32) -> RuntimeResult<bool> {
    // SAFETY: admission established a live actor. Core owns its state frame and
    // metadata; this is Core's IsInState current/superstate query, not dispatch
    // of an indexed native through the script-event boundary.
    unsafe {
        let frame = ptr::addr_of!((*item).state_frame).read().cast::<FStateFrame>();
        if frame.is_null() { return Ok(false); }
        let current = ptr::addr_of!((*frame).state_node).read().cast::<UState>();
        if current.is_null() { return Ok(false); }
        let state_class = a.native_class("State")?;
        Ok(state_contains(Some(current), requested,
            |state| i32::from_ne_bytes(ptr::addr_of!((*state.cast::<UObject>()).name).read()),
            |state| {
                let parent = ptr::addr_of!((*state.cast::<UField>()).super_field).read();
                if parent.is_null() || !hp2_core::object_model::is_a(parent, state_class.cast()) { None } else { Some(parent.cast()) }
            }))
    }
}

unsafe fn supported_color(a: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<bool> {
    // SAFETY: canonical metadata identity, not a compatible signature alone,
    // bounds callbacks to the decoded pure getters with no resource writes.
    unsafe {
        if function.is_null() { return Ok(false); }
        for (owner, kind) in [
            ("HGame.StatusItemHealth", "Core.Class"),
            ("HGame.StatusItemHealth.HoldChange", "Core.State"),
            ("HGame.StatusItemHealth.FadeChangeOut", "Core.State"),
        ] {
            if function == find_function(a, owner, kind, "GetHealthDrawColor")? {
                return signature(a, function, &[], Some(("Core.StructProperty", 4)));
            }
        }
        Ok(false)
    }
}

unsafe fn preflight(a: CoreAccess<'_>, item: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    if !live(a, item) || !live(a, canvas) { return Ok(false); }
    // SAFETY: only live indexed objects and checked reflection are read.
    unsafe {
        let Some(class) = find_class(a, "HGame.StatusItemHealth")? else { return Ok(false); };
        if !is_a(a, item, class) || read_boolean(a, item, "bDeleteMe")? || !canvas_ready(a, canvas)? { return Ok(false); }
        let expected = find_function(a, "HGame.StatusItemHealth", "Core.Class", "DrawItem")?;
        let actual = virtual_function(a, item, "DrawItem")?;
        if expected.is_null() || actual != expected || !signature(a, actual, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>()), ("nCurrX", "Core.IntProperty", 4), ("nCurrY", "Core.IntProperty", 4), ("fScaleFactor", "Core.FloatProperty", 4)], None)? { return Ok(false); }
        let getter = virtual_function(a, item, "GetHealthDrawColor")?;
        if !supported_color(a, getter)? { return Ok(false); }
        a.native_class("State")?;
        for name in ["nCount", "nCurrCountPotential", "nUnitsPerIcon", "nActualIconW"] { int_of(a, item, name)?; }
        for name in ["textureHudIcon", "textureHealthBlack", "textureHealthBorder", "textureHealthOrangeInside"] {
            texture_size(a, object(a, item, name)?)?;
        }
        Ok(true)
    }
}

/// Pre-effect admission for direct composition by the status-group view.
/// # Safety
/// Inputs must be indexed Core objects or null; this function has no effects.
pub(super) unsafe fn admits(a: CoreAccess<'_>, item: *mut UObject, canvas: *mut UObject) -> RuntimeResult<bool> {
    // Unsupported or unavailable routes remain stock, before any drawing.
    match unsafe { preflight(a, item, canvas) } { Ok(admitted) => Ok(admitted), Err(_) => Ok(false) }
}

/// Draws the entire retail horizontal row; caller coordinates are deliberately ignored.
/// # Safety
/// The caller must admit this route before effects and keep Core alive through reentry.
pub(super) unsafe fn draw(a: CoreAccess<'_>, item: *mut UObject, canvas: *mut UObject, scale: f32) -> RuntimeResult<()> {
    // SAFETY: admission checks reflected fields; weak identities are revalidated
    // after every original callback before any further property access.
    unsafe {
        // Admission excludes custom callbacks before the first paint. The
        // native state query below still reflects the current state each bolt.
        let normal_state = a.intern_str("NormalDisplay", FindName::Find)?;
        let receiver = Weak::new(item);
        let output = Weak::new(canvas);
        let saved_color = color(a, canvas)?;
        let saved_style = prop(a, canvas, "Style", "Core.ByteProperty", 1)?.read();
        let count = icon_count(int_of(a, item, "nCurrCountPotential")?, int_of(a, item, "nUnitsPerIcon")?);
        let mut remaining = int_of(a, item, "nCount")?;
        let mut x = (5. * scale) as i32;
        let y = (4. * scale) as i32;
        let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
        let color_blind = crate::ui_accessibility::color_blind_hud(a);
        for index in 0..count {
            let mut overlay = None;
            let units = int_of(a, item, "nUnitsPerIcon")?;
            let ratio = if remaining >= units { 1. } else { remaining as f32 / units as f32 };
            remaining = remaining.wrapping_sub(int_of(a, item, "nUnitsPerIcon")?);
            if remaining < 0 { remaining = 0; }
            let alpha = color(a, canvas)?[3];
            target.set_color(a, [255, 255, 255, alpha])?;
            target.set_style(a, 3)?;
            icon(a, item, canvas, object(a, item, "textureHealthBlack")?, x as f32, y as f32, scale)?;
            target.set_style(a, saved_style)?;
            icon(a, item, canvas, object(a, item, "textureHealthBorder")?, x as f32, y as f32, scale)?;
            if !normal_display(a, item, normal_state)? && affected(int_of(a, item, "nCount")?, int_of(a, item, "nUnitsPerIcon")?, index) {
                let getter = virtual_function(a, item, "GetHealthDrawColor")?;
                if !supported_color(a, getter)? { return Err(RuntimeError::new("Health color callback left the supported route")); }
                let mut tint = [0_u8; 4];
                invoke_virtual(a, item, "GetHealthDrawColor", &[], &[], tint.as_mut_ptr().cast::<c_void>())?;
                if !receiver.live(a) || !output.live(a) || read_boolean(a, item, "bDeleteMe")? { return Err(RuntimeError::new("Health callback destroyed its view")); }
                if color_blind {
                    // Blue/orange instead of the retail red/green change colours.
                    let palette = crate::ui_accessibility::health_palette(tint);
                    tint = palette.fill;
                    overlay = palette.overlay;
                }
                target.set_color(a, tint)?;
            }
            if !receiver.live(a) || !output.live(a) || read_boolean(a, item, "bDeleteMe")? { return Err(RuntimeError::new("Health view destroyed during drawing")); }
            if ratio > 0. {
                let texture = object(a, item, "textureHudIcon")?;
                texture_size(a, texture)?;
                let width = int_of(a, texture, "USize")?;
                let height = int_of(a, texture, "VSize")?;
                let (dest, source) = fill_rect(x, y, scale, ratio, width, height);
                let fill = object(a, item, "textureHealthOrangeInside")?;
                texture_size(a, fill)?;
                paint_picture(a, item, canvas, Picture { texture: (*fill).index, dest, source, color: color(a, canvas)?, blend: BlendMode::Inherit })?;
                if let Some(add) = overlay {
                    // The fill texture is gold, which a tint cannot turn blue: add coloured
                    // copies of the (light) HUD icon bolt over the same region of the fill.
                    // Translucent drawing multiplies texel by colour, so a few passes are
                    // needed for the added colour to carry over the fill.
                    if texture_size(a, texture)? == texture_size(a, fill)? {
                        for _ in 0..crate::ui_accessibility::HEALTH_OVERLAY_PASSES {
                            paint_picture(a, item, canvas, Picture { texture: (*texture).index, dest, source, color: add, blend: BlendMode::Translucent })?;
                        }
                    }
                }
            }
            x = x.wrapping_add((int_of(a, item, "nActualIconW")? as f32 * scale) as i32);
        }
        // Only the normal-return DrawColor restore exists in retail. In
        // particular do not undo partial drawing or callback effects on error.
        target.set_color(a, saved_color)
    }
}

unsafe fn callback(a: CoreAccess<'_>, item: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    // SAFETY: Core supplied this invocation and its parameter storage.
    unsafe {
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !admits(a, item, canvas)? { return Ok(ScriptPreCallDecision::Continue); }
        let offset = field(a, function.cast(), "fScaleFactor", "Core.FloatProperty", 4, size, true)?;
        let scale = params.add(offset).cast::<f32>().read_unaligned() * crate::display_options::hud_scale(a);
        draw(a, item, canvas, scale)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}

/// # Safety
/// Core must outlive the returned registration guard.
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: the observer guard keeps its canonical metadata registered.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let function = find_function(a, "HGame.StatusItemHealth", "Core.Class", "DrawItem")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-player-health", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    // Observe the rendering decision: normal (including inherited normal)
    // never requests a changed-fill tint; Hold/Fade do, at both inclusive
    // boundaries even when the second bolt's actual fill is empty.
    const NORMAL: i32 = 1;
    const HOLD: i32 = 2;
    const FADE: i32 = 3;
    let states = [(NORMAL, None), (HOLD, None), (FADE, None), (4, Some(0))];
    for (state, expected_callbacks) in [(0, 0), (1, 2), (2, 2), (3, 0)] {
        let callbacks = (0..2).filter(|&index| {
            !state_contains(Some(state), NORMAL, |index| states[index].0, |index| states[index].1)
                && affected(100, 100, index)
        }).count();
        assert_eq!(callbacks, expected_callbacks, "normal/hold/fade fill-color policy for state {state}");
    }
    assert_eq!(icon_count(600, 100), 6);
    assert_eq!(icon_count(601, 100), 7);
    assert_eq!(icon_count(100, 0), 0);
    assert!(affected(100, 100, 0) && affected(100, 100, 1));
    assert!(!affected(101, 100, 0));
    let (dest, source) = fill_rect(6, 5, 1.25, 0.5, 64, 64);
    assert_eq!(source, Rect::new(0., 27., 64., 37.));
    assert_eq!(dest, Rect::new(6., 38.75, 80., 46.25));
    let picture = Picture { texture: 17, dest: Rect::new(-3., 4., -80., 0.), source, color: [7, 8, 9, 10], blend: BlendMode::Translucent };
    let store = Store::new();
    let mut ui = Ui::new(store, move || view(&picture), HostMetrics::default());
    ui.resize(800., 600.);
    ui.relayout();
    let commands = ui.draw();
    assert_eq!(commands.len(), 1, "signed/zero health geometry is not clipped away");
    assert!(matches!(&commands[0], DrawCmd::Image { rect, fit: Fit::Region(0., 27., 64., 37.), color: Color(7,8,9,10), blend: BlendMode::Translucent, .. } if *rect == picture.dest));
}
