//! The complete duelling selector as a toolkit view of script-owned state.
use std::cell::RefCell;
use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{
    ScriptPreCallDecision,
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};

use super::{CanvasState, CanvasTarget, find_function, int_of, param_object, player_fonts};
use crate::{
    stock_graphics_menu::{Weak, is_a, live, object, prop},
    ui_host::{DrawTarget, HostMetrics, find_class, render_commands},
};

/// Actual retail HudItemManager.GetScaleFactor divides the Canvas width by 640.
const BASE_WIDTH: f32 = 640.;
/// Actual retail RenderHudItemManager starts at 175 (the prototype uses 200).
const START_Y: f32 = 175.;
const HOTKEYS: [&str; 3] = ["1", "2", "3"];
const TEXTURES: [(&str, &str); 3] = [
    ("textureSpellRictusempra", "textureSpellRictusempraSel"),
    ("textureSpellMimblewimble", "textureSpellMimblewimbleSel"),
    ("textureSpellExpelliarmus", "textureSpellExpelliarmusSel"),
];

#[derive(Clone, Debug, PartialEq)]
struct Icon {
    texture: Option<TextureRef>,
    size: (i32, i32),
    /// Spacing uses the normal texture, even when the selected art differs.
    normal_height: i32,
}

#[derive(Clone, Debug, PartialEq)]
struct Selector {
    size: (i32, i32),
    icons: [Icon; 3],
    text_sizes: [(f32, f32); 3],
    tint: [u8; 4],
}

struct SelectorUi {
    receiver: Weak,
    model: Signal<Selector>,
    ui: Ui,
    metrics: HostMetrics,
}

thread_local! {
    static VIEWS: RefCell<Vec<SelectorUi>> = const { RefCell::new(Vec::new()) };
}

fn hotkey_font(width: i32) -> Font {
    Font::Slot(if width <= 512 {
        3
    } else if width <= 640 {
        2
    } else {
        1
    })
}

fn view(model: &Selector) -> Node {
    let scale = model.size.0 as f32 / BASE_WIDTH;
    let x = (2. * scale) as i32;
    let mut y = (START_Y * scale) as i32;
    let mut children = Vec::with_capacity(6);
    for (i, icon) in model.icons.iter().enumerate() {
        if let Some(texture) = &icon.texture {
            children.push(
                image(texture.clone(), Fit::Stretch)
                    .color(Color(model.tint[0], model.tint[1], model.tint[2], model.tint[3]))
                    .at(x as f32, y as f32)
                    .width(Size::Px(icon.size.0 as f32 * scale))
                    .height(Size::Px(icon.size.1 as f32 * scale)),
            );
        }
        let (w, h) = model.text_sizes[i];
        // Retail converts the scaled anchor to int BEFORE subtracting half the
        // text size, then converts the subtraction result to int once more.
        let dx = ((20. * scale) as i32 as f32 - w / 2.) as i32;
        let dy = ((50. * scale) as i32 as f32 - h / 2.) as i32;
        children.push(
            label(HOTKEYS[i])
                .font(hotkey_font(model.size.0))
                .color(Color(206, 200, 190, model.tint[3]))
                .at(x.wrapping_add(dx) as f32, y.wrapping_add(dy) as f32)
                .width(Size::Px(w))
                .height(Size::Px(h)),
        );
        y = y.wrapping_add((icon.normal_height.wrapping_add(4) as f32 * scale) as i32);
    }
    col().children(children)
}

/// Do not swallow custom drawing or scale callbacks when replacing the base view.
unsafe fn base_route(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<bool> {
    // SAFETY: the caller checked receiver liveness; class and function objects are
    // canonical metadata owned by Core throughout this synchronous callback.
    unsafe {
        let Some(class) = find_class(a, "HGame.SpellSelector")? else {
            return Ok(false);
        };
        if !is_a(a, receiver, class.cast()) {
            return Ok(false);
        }
        for (owner, name) in [
            ("HGame.HudItemManager", "GetScaleFactor"),
            ("HGame.SpellSelector", "DrawSpellIcon"),
            ("HGame.SpellSelector", "DrawHotKeyText"),
        ] {
            let expected = find_function(a, owner, "Core.Class", name)?;
            let resolved = a.find_object_function_by_name(
                receiver,
                a.intern_str(name, FindName::Find)?,
                false,
            )?;
            if expected.is_null() || resolved != expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

unsafe fn snapshot(
    a: CoreAccess<'_>,
    receiver: *mut UObject,
    canvas: *mut UObject,
) -> RuntimeResult<Selector> {
    // SAFETY: receiver and Canvas are live; reflected accessors validate each
    // property's type, size and bounds before reading it. Texture liveness is
    // established before accessing its metadata.
    unsafe {
        let selection = prop(a, receiver, "CurrSelection", "Core.ByteProperty", 1)?.read();
        let mut icons = Vec::with_capacity(3);
        for (i, (normal_name, selected_name)) in TEXTURES.into_iter().enumerate() {
            let normal = object(a, receiver, normal_name)?;
            let selected = object(a, receiver, selected_name)?;
            if !live(a, normal) {
                return Err(RuntimeError::new("Spell selector spacing texture unavailable"));
            }
            let texture = if usize::from(selection) == i {
                selected
            } else {
                normal
            };
            if !texture.is_null() && !live(a, texture) {
                return Err(RuntimeError::new("Spell selector icon unavailable"));
            }
            icons.push(Icon {
                texture: (!texture.is_null()).then(|| TextureRef(format!("#{}", (*texture).index))),
                size: if texture.is_null() { (0, 0) } else {
                    (int_of(a, texture, "USize")?, int_of(a, texture, "VSize")?)
                },
                normal_height: int_of(a, normal, "VSize")?,
            });
        }
        Ok(Selector {
            size: (int_of(a, canvas, "SizeX")?, int_of(a, canvas, "SizeY")?),
            icons: icons.try_into().expect("three spell texture pairs"),
            text_sizes: [(0., 0.); 3],
            tint: prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?
                .cast::<[u8; 4]>()
                .read_unaligned(),
        })
    }
}

/// Unlike the subtitle target, the selector preserves the incoming tile tint
/// and does not force masked text style. Native Canvas draws own cursor/CurYL
/// and style side effects; only Font and DrawColor are restored, as in stock.
struct SelectorTarget {
    canvas: CanvasTarget,
}

impl DrawTarget for SelectorTarget {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> {
        Err(RuntimeError::new("Unexpected selector fill"))
    }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        // SAFETY: this delegates the existing live Canvas texture bridge.
        unsafe { self.canvas.texture_object(a, name) }
    }
    unsafe fn segment(
        &self,
        a: CoreAccess<'_>,
        texture: *mut UObject,
        dest: Rect,
        src: Rect,
        color: [u8; 4],
        blend: BlendMode,
    ) -> RuntimeResult<()> {
        // SAFETY: draw-list resources were snapshotted from live script fields;
        // rendering is synchronous and does not call script or retain pointers.
        unsafe {
            self.canvas.segment(a, texture, dest, src, color, blend)
        }
    }
    unsafe fn text_at(
        &self,
        a: CoreAccess<'_>,
        x: f32,
        y: f32,
        text: &str,
        font: Font,
        color: [u8; 4],
    ) -> RuntimeResult<()> {
        // SAFETY: the Canvas and font remain live for this synchronous draw. Use
        // canonical bitmap-compatible text, including its cursor updates.
        unsafe {
            self.canvas.set_font(a, font)?;
            self.canvas.set_color(a, color)?;
            self.canvas.set_pos(a, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.canvas.canvas.cast(), &units, false, 0)
        }
    }
}

unsafe fn paint(
    a: CoreAccess<'_>,
    receiver: *mut UObject,
    canvas: *mut UObject,
    fonts: [*mut UObject; 4],
    mut model: Selector,
) -> RuntimeResult<()> {
    // SAFETY: callback preflight established Canvas liveness and reflected
    // properties. No pointer escapes this synchronous paint.
    unsafe {
        let saved = CanvasState::save(a, canvas)?;
        let target = SelectorTarget {
            canvas: CanvasTarget {
                canvas,
                tile_style: None,
                fonts,
            },
        };
        let result = (|| -> RuntimeResult<()> {
            // Remove the retained view while painting: no RefCell borrow can
            // cross a synchronous Canvas/script boundary.
            let mut retained = VIEWS.with(|views| {
                let mut views = views.borrow_mut();
                views.retain(|view| view.receiver.live(a));
                views.iter().position(|view| view.receiver.p == receiver)
                    .map(|index| views.swap_remove(index))
            });
            let metrics = retained.as_ref().map_or_else(HostMetrics::default, |view| view.metrics.clone());
            let font = hotkey_font(model.size.0);
            for (i, text) in HOTKEYS.into_iter().enumerate() {
                // Remeasure every invocation, including provider/font changes.
                model.text_sizes[i] = target.canvas.text_size(a, text, font)?;
                metrics.learn(text, font, model.text_sizes[i]);
            }
            let (w, h) = model.size;
            let mut retained = retained.take().unwrap_or_else(|| {
                let store = Store::new();
                let current = store.signal(model.clone());
                let observed = current.clone();
                SelectorUi {
                    receiver: Weak::new(receiver),
                    model: current,
                    ui: Ui::new(store, move || view(&observed.get()), metrics.clone()),
                    metrics: metrics.clone(),
                }
            });
            retained.model.set(model);
            retained.ui.resize(w as f32, h as f32);
            retained.ui.relayout();
            let result = render_commands(&target, a, &retained.ui.draw(), &metrics);
            if retained.receiver.live(a) {
                VIEWS.with(|views| views.borrow_mut().push(retained));
            }
            result
        })();
        // Restore only the stock selector's saved properties, NOT its cursor or
        // style. Even on error, never replay stock over a partially drawn view.
        let restored = (|| -> RuntimeResult<()> {
            prop(
                a,
                canvas,
                "Font",
                "Core.ObjectProperty",
                size_of::<*mut UObject>(),
            )?
            .cast::<*mut UObject>()
            .write_unaligned(saved.font);
            target.canvas.set_color(a, saved.color)
        })();
        result?;
        restored
    }
}

unsafe fn callback(
    a: CoreAccess<'_>,
    receiver: *mut UObject,
    function: *mut UFunction,
    params: *const u8,
    size: usize,
) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a, receiver) {
        return Ok(ScriptPreCallDecision::Continue);
    }
    // SAFETY: observer metadata and parameters are supplied by Core for this
    // live invocation. Only the admitted base route is suppressed.
    unsafe {
        if !base_route(a, receiver)? {
            return Ok(ScriptPreCallDecision::Continue);
        }
        let Some(canvas) = param_object(a, function, params, size, "Canvas") else {
            return Ok(ScriptPreCallDecision::Continue);
        };
        if !live(a, canvas) {
            return Ok(ScriptPreCallDecision::Continue);
        }
        // Fallback is allowed only before measurements or rendering can have
        // effects. Missing resources during startup remain stock-owned.
        let prepared = (|| -> RuntimeResult<_> {
            let model = snapshot(a, receiver, canvas)?;
            let level = object(a, receiver, "Level")?;
            if !live(a, level) {
                return Err(RuntimeError::new("Spell selector level unavailable"));
            }
            let fonts = player_fonts(a, object(a, level, "PlayerHarryActor")?)?;
            let slot = if model.size.0 <= 512 {
                3
            } else if model.size.0 <= 640 {
                2
            } else {
                1
            };
            if !live(a, fonts[slot]) {
                return Err(RuntimeError::new("Spell selector font unavailable"));
            }
            Ok((model, fonts))
        })();
        let (model, fonts) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                super::warn(a, &error);
                return Ok(ScriptPreCallDecision::Continue);
            }
        };
        paint(a, receiver, canvas, fonts, model)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}

pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    // SAFETY: canonical function metadata is retained by the observer guard.
    unsafe {
        if !super::enabled() {
            return Ok(None);
        }
        let draw = find_function(
            a,
            "HGame.SpellSelector",
            "Core.Class",
            "RenderHudItemManager",
        )?;
        if draw.is_null() {
            return Ok(None);
        }
        Ok(Some(a.install_script_pre_observer(
            "ui-spell-selector",
            &[draw],
            callback,
        )?))
    }
}

#[cfg(test)]
pub(super) fn assert_view() {
    use hp2_ui::DrawCmd;
    let model = Selector {
        size: (800, 600),
        icons: std::array::from_fn(|i| Icon {
            texture: Some(TextureRef(format!("#{i}"))),
            size: (64, if i == 0 { 80 } else { 64 }),
            normal_height: 64,
        }),
        text_sizes: [(9., 13.); 3],
        tint: [17, 33, 65, 73],
    };
    let metrics = HostMetrics::default();
    for width in [512, 513, 640, 641, 800, 1280] {
        for text in HOTKEYS {
            metrics.learn(text, hotkey_font(width), (9., 13.));
        }
    }
    let store = Store::new();
    let current = store.signal(model.clone());
    let observed = current.clone();
    let mut ui = Ui::new(store, move || view(&observed.get()), metrics);
    ui.resize(800., 600.);
    let commands = ui.draw();
    assert_eq!(
        commands.len(),
        6,
        "three whole icons and their three labels"
    );
    let ys = [218., 303., 388.];
    for i in 0..3 {
        assert!(
            matches!(&commands[2*i], DrawCmd::Image { rect, texture, fit: Fit::Stretch, color, blend: BlendMode::Inherit }
            if rect.x == 2. && rect.y == ys[i] && rect.h == if i == 0 {100.} else {80.}
                && texture.0 == format!("#{i}") && *color == Color(17, 33, 65, 73)),
            "icon {i}: {:?}",
            commands[2 * i]
        );
        assert!(
            matches!(&commands[2*i+1], DrawCmd::Text { rect, text, color, .. }
            if rect.x == 22. && rect.y == ys[i] + 55. && text == HOTKEYS[i]
                && *color == Color(206, 200, 190, 73)),
            "label {i}: {:?}",
            commands[2 * i + 1]
        );
    }
    for (width, slot) in [(512, 3), (513, 2), (640, 2), (641, 1), (1280, 1)] {
        let mut resized = model.clone();
        resized.size = (width, 720);
        current.set(resized);
        ui.resize(width as f32, 720.);
        let commands = ui.draw();
        let scale = width as f32 / BASE_WIDTH;
        let x = (2. * scale) as i32;
        let y = (START_Y * scale) as i32;
        let dx = ((20. * scale) as i32 as f32 - 4.5) as i32;
        let dy = ((50. * scale) as i32 as f32 - 6.5) as i32;
        assert!(
            matches!(&commands[1], DrawCmd::Text { rect, font, text, .. }
                if *font == Font::Slot(slot) && text == "1"
                    && rect.x == x.wrapping_add(dx) as f32
                    && rect.y == y.wrapping_add(dy) as f32),
            "font threshold and integer centering at width {width}: {:?}", commands[1],
        );
    }
    let mut absent = model;
    absent.icons[1].texture = None;
    current.set(absent);
    ui.resize(800., 600.);
    let commands = ui.draw();
    assert!(!commands.iter().any(|cmd| matches!(cmd,
        DrawCmd::Image { texture, .. } if texture.0 == "#1")));
    assert!(commands.iter().any(|cmd| matches!(cmd,
        DrawCmd::Text { text, rect, .. } if text == "2" && rect.y == 358.)));
    assert!(commands.iter().any(|cmd| matches!(cmd,
        DrawCmd::Image { texture, rect, .. } if texture.0 == "#2" && rect.y == 388.)));
}
