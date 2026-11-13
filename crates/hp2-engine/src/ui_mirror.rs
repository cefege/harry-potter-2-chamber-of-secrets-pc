//! Every stock menu page (and the message box) shown through `hp2-ui`.
//!
//! The stock page is still created and run by its script; its controls stay alive, in place and
//! keep their state, but their own `Paint` and pointer functions are swallowed by pre-call hooks
//! (the page's painting still sees their real geometry). Each paint reads them into a snapshot, the
//! toolkit draws that snapshot (same art, fonts and positions) and handles the pointer, and the
//! player's actions are replayed on the stock controls through their own script functions
//! (`Click`, `SetValue`, `SetSelectedIndex`, `Notify`), so every stock handler runs as before.
//! Page-specific artwork painted by the page's own `Paint` (book, cards, map) is left to the page.
//! Controls of unknown classes (text areas, 3D views) are not touched.
use std::{cell::RefCell, collections::HashMap, ffi::c_void, rc::Rc};

use hp2_abi::engine::generated::{UClass, UFunction, UObject};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{ButtonArt, Color, Fit, Font, ImageButton, Key, PointerButton, PopupMetrics, Rect, Signal, Size, Skin, SkinPart, SliderMetrics, Store, TextAlign, TextureRef, Ui, checkbox, col, image_button, label, number_input, select, slider};

use crate::{
    stock_graphics_menu::{boolean, children, field, live, object, prop, read_boolean, Weak},
    ui_host::{class_name_of, find_class, name_of, read_string, HostMetrics, Surface},
    ui_vm::{invoke, V},
};

/// Pages shown through the mirror. Sound/Video has its own hand-written view (`ui_pages`).
const PAGES: &[&str] = &["FEMainPage", "FEInGamePage", "FEOptionsPage", "FEInputPage", "FEFolioPage", "FEHousepointsPage", "FEQuidPage", "FEDuelPage", "FEMapPage", "FECreditsPage", "FELangGrid", "HPMessageBox"];

#[derive(Clone, Debug, PartialEq)]
enum Item {
    /// `at`: where the stock layout put the text inside the control.
    Label { text: String, font: u8, color: Color, at: (f32, f32) },
    Check { text: String, font: u8, color: Color, checked: bool },
    Slider { min: f32, max: f32, step: f32, value: f32, caption: String, font: u8, color: Color, at: (f32, f32) },
    Combo { caption: String, font: u8, color: Color, at: (f32, f32), value: String, items: Vec<String>, edit_w: f32, editable: bool, edit_color: Color },
    Button { art: Box<ImageButton>, font: u8, color: Color },
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    index: i32,
    rect: Rect,
    disabled: bool,
    item: Item,
}

enum Action {
    Click(i32),
    Toggle(i32, bool),
    Slide(i32, f32),
    Pick(i32, usize),
    Edit(i32, String),
}

struct Mirror {
    page: Weak,
    ui: Ui,
    entries: Signal<Vec<Entry>>,
    metrics: HostMetrics,
    queue: Rc<RefCell<Vec<Action>>>,
}

struct Performing;
impl Drop for Performing {
    fn drop(&mut self) {
        PERFORMING.with(|p| p.set(false));
    }
}

thread_local! {
    /// Set while the mirror itself calls stock control functions, which must then run normally.
    static PERFORMING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The classes of the mirrored pages, refreshed when the hooks are installed.
    static PAGE_CLASSES: RefCell<Vec<*mut UClass>> = const { RefCell::new(Vec::new()) };
    static MIRRORS: RefCell<Vec<Mirror>> = const { RefCell::new(Vec::new()) };
    static TEXTURES: Rc<RefCell<HashMap<String, (*mut UObject, i32)>>> = Rc::new(RefCell::new(HashMap::new()));
}

fn enabled() -> bool {
    std::env::var_os("HP2_STOCK_MENUS").is_none()
}

fn warn(a: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = a.intern_str("Warning", FindName::Add) {
        let _ = a.emit_log(category, &format!("Rust menu mirror: {error}").chars().map(u32::from).collect::<Vec<_>>());
    }
}

// ---- reading stock controls ---------------------------------------------------------------

unsafe fn f32_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> f32 {
    unsafe { prop(a, o, name, "Core.FloatProperty", 4) }.map(|p| unsafe { p.cast::<f32>().read_unaligned() }).unwrap_or(0.)
}
unsafe fn i32_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> i32 {
    unsafe { prop(a, o, name, "Core.IntProperty", 4) }.map(|p| unsafe { p.cast::<i32>().read_unaligned() }).unwrap_or(0)
}
unsafe fn u8_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> u8 {
    unsafe { prop(a, o, name, "Core.ByteProperty", 1) }.map(|p| unsafe { p.read() }).unwrap_or(0)
}
unsafe fn bool_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> bool {
    unsafe { read_boolean(a, o, name) }.unwrap_or(false)
}
unsafe fn color_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> Color {
    unsafe { prop(a, o, name, "Core.StructProperty", 4) }.map(|p| unsafe { p.cast::<[u8; 4]>().read_unaligned() }).map(|c| Color(c[0], c[1], c[2], 255)).unwrap_or(Color::WHITE)
}
unsafe fn string_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> String {
    unsafe { read_string(a, o, name) }.unwrap_or_default()
}
unsafe fn object_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> *mut UObject {
    unsafe { object(a, o, name) }.unwrap_or(std::ptr::null_mut())
}
unsafe fn region_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> [i32; 4] {
    unsafe { prop(a, o, name, "Core.StructProperty", 16) }.map(|p| unsafe { p.cast::<[i32; 4]>().read_unaligned() }).unwrap_or_default()
}

fn align_of(value: u8) -> TextAlign {
    match value {
        1 => TextAlign::Right,
        2 => TextAlign::Center,
        _ => TextAlign::Left,
    }
}

unsafe fn is(a: CoreAccess<'_>, o: *mut UObject, class: &str) -> bool {
    a.native_class(class).is_ok_and(|c| unsafe { hp2_core::object_model::is_a(o.cast(), c.cast()) })
}

/// The art a stock `UWindowButton` draws for one state.
unsafe fn button_art(a: CoreAccess<'_>, button: *mut UObject, texture: &str, region: &str, w: f32, h: f32) -> Option<ButtonArt> {
    let tex = unsafe { object_of(a, button, texture) };
    if tex.is_null() {
        return None;
    }
    let offset = unsafe { (f32_of(a, button, "ImageX"), f32_of(a, button, "ImageY")) };
    let texture = TextureRef(format!("#{}", unsafe { (*tex).index }));
    if unsafe { bool_of(a, button, "bUseRegion") } {
        let r = unsafe { region_of(a, button, region) };
        let scale = unsafe { f32_of(a, button, "RegionScale") };
        let scale = if scale > 0. { scale } else { 1. };
        return Some(ButtonArt { texture, fit: Fit::Region(r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32), offset, size: Some((r[2] as f32 * scale, r[3] as f32 * scale)) });
    }
    if unsafe { bool_of(a, button, "bStretched") } {
        return Some(ButtonArt { texture, fit: Fit::Stretch, offset, size: Some((w, h)) });
    }
    Some(ButtonArt { texture, fit: Fit::Native, offset, size: None })
}

/// Reads one stock control. `None` for classes the mirror does not handle.
unsafe fn read_item(a: CoreAccess<'_>, o: *mut UObject, w: f32, h: f32) -> Option<Item> {
    let font = || unsafe { i32_of(a, o, "Font") }.clamp(0, 9) as u8;
    let color = || unsafe { color_of(a, o, "TextColor") };
    let text = || unsafe { string_of(a, o, "Text") };
    let at = || unsafe { (f32_of(a, o, "TextX"), f32_of(a, o, "TextY")) };
    unsafe {
        if is(a, o, "UWindow.UWindowLabelControl") {
            return Some(Item::Label { text: text(), font: font(), color: color(), at: at() });
        }
        if is(a, o, "UWindow.UWindowCheckbox") {
            return Some(Item::Check { text: text(), font: font(), color: color(), checked: bool_of(a, o, "bChecked") });
        }
        if is(a, o, "UWindow.UWindowHSliderControl") {
            return Some(Item::Slider { min: f32_of(a, o, "MinValue"), max: f32_of(a, o, "MaxValue"), step: i32_of(a, o, "Step") as f32, value: f32_of(a, o, "Value"), caption: text(), font: font(), color: color(), at: at() });
        }
        if is(a, o, "UWindow.UWindowComboControl") {
            let edit = object_of(a, o, "EditBox");
            let list = object_of(a, o, "List");
            let mut items = Vec::new();
            if live(a, list) {
                let mut item = object_of(a, object_of(a, list, "Items"), "Next");
                while live(a, item) && items.len() < 64 {
                    items.push(string_of(a, item, "Value"));
                    item = object_of(a, item, "Next");
                }
            }
            let value = if live(a, edit) { string_of(a, edit, "Value") } else { String::new() };
            let edit_color = if live(a, edit) { color_of(a, edit, "TextColor") } else { Color::WHITE };
            return Some(Item::Combo { caption: text(), font: font(), color: color(), at: at(), value, items, edit_w: f32_of(a, o, "EditBoxWidth"), editable: bool_of(a, o, "bCanEdit"), edit_color });
        }
        if is(a, o, "UWindow.UWindowButton") {
            let raised = class_name_of(a, o) == "HPMenuRaisedButton";
            let over_color = bool_of(a, o, "bColorOver").then(|| color_of(a, o, "OverColor"));
            let original_text = text();
            let pad_text = hp2_platform::game_controller::active()
                .then(|| hp2_platform::game_controller::button_labels(&original_text))
                .flatten();
            let text_pos = pad_text.is_none().then(|| (f32_of(a, o, "TextX"), f32_of(a, o, "TextY")));
            let art = ImageButton {
                up: button_art(a, o, "UpTexture", "UpRegion", w, h),
                over: button_art(a, o, "OverTexture", "OverRegion", w, h),
                down: button_art(a, o, "DownTexture", "DownRegion", w, h),
                disabled: button_art(a, o, "DisabledTexture", "DisabledRegion", w, h),
                text: pad_text.unwrap_or(original_text),
                over_color,
                shadow: bool_of(a, o, "bShadowText"),
                text_align: align_of(u8_of(a, o, "Align")),
                text_inset: if raised { i32_of(a, o, "textOffsetX") as f32 } else { 0. },
                // The stock layout (its own BeforePaint or the page) already placed the caption.
                text_pos,
            };
            return Some(Item::Button { art: Box::new(art), font: font(), color: color() });
        }
    }
    None
}

/// The page's known controls, in paint order.
unsafe fn snapshot(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<Vec<Entry>> {
    // Controls run their BeforePaint as usual, so their text positions are current.
    let mut out = Vec::new();
    for (child, _) in unsafe { children(a, page) } {
        let (x, y, w, mut h) = unsafe { (f32_of(a, child, "WinLeft"), f32_of(a, child, "WinTop"), f32_of(a, child, "WinWidth"), f32_of(a, child, "WinHeight")) };
        let Some(item) = (unsafe { read_item(a, child, w, h) }) else { continue };
        if !unsafe { bool_of(a, child, "bWindowVisible") } {
            continue;
        }
        if matches!(item, Item::Button { .. }) && class_name_of(a, child) == "HPMenuRaisedButton" {
            h = 24.;
        }
        let disabled = unsafe { bool_of(a, child, "bDisabled") };
        out.push(Entry { index: unsafe { (*child).index }, rect: Rect::new(x, y, w, h), disabled, item });
    }
    Ok(out)
}

// ---- the view -------------------------------------------------------------------------------

fn skin() -> Skin {
    let named = |n: &str| format!("HGame.Icons.{n}");
    Skin::new()
        .with_native(SkinPart::BoxIdle, &named("FEComboIdleTexture"))
        .with_native(SkinPart::BoxHover, &named("FEComboOverTexture"))
        .with_native(SkinPart::SliderTrack, &named("FESliderTexture"))
        .with_native(SkinPart::SliderTrackHover, &named("FEOverSliderTexture"))
        .with_native(SkinPart::SliderThumb, &named("FESliderKnobTexture"))
        .with_native(SkinPart::CheckOff, &named("FEOptionTickUncheckedTex"))
        .with_native(SkinPart::CheckOffHover, &named("FEOptionTickUncheckedOverTex"))
        .with_native(SkinPart::CheckOn, &named("FEOptionTickCheckedTex"))
        .with_native(SkinPart::CheckOnHover, &named("FEOptionTickCheckedOverTex"))
        .with_region(SkinPart::ListBackground, &named("FEComboListLarge"), (0., 0., 147., 89.))
        .with(SkinPart::ListRowSelected, &named("FEComboListBox"))
        .with_slider(SliderMetrics { track_dx: -1., thumb_start: 9., travel: 99., thumb_width: 10. })
        .with_check_size(12.)
        .with_box(1., 11.)
        .with_popup(PopupMetrics { row_height: 20.5, top: 3., bottom: 4., inset: 2., text_dx: 11. })
}

fn view(entries: &[Entry], queue: &Rc<RefCell<Vec<Action>>>) -> hp2_ui::Node {
    let mut root = col();
    for e in entries {
        let r = e.rect;
        let key = format!("c{}", e.index);
        let index = e.index;
        let push = |action: Action| {
            let queue = queue.clone();
            move || queue.borrow_mut().push(action_clone(&action))
        };
        let node = match &e.item {
            Item::Label { text, font, color, at } => label(text).font(Font::Slot(*font)).color(*color).at(r.x + at.0, r.y + at.1),
            Item::Check { text, font, color, checked } => {
                let q = queue.clone();
                checkbox(text, *checked, move |on| q.borrow_mut().push(Action::Toggle(index, on))).font(Font::Slot(*font)).color(*color).at(r.x, r.y).width(Size::Px(r.w)).height(Size::Px(r.h.max(16.)))
            }
            Item::Slider { min, max, step, value, caption, font, color, at } => {
                let q = queue.clone();
                let x0 = r.x + r.w - 133.;
                let mut group = col().at(0., 0.);
                if !caption.is_empty() {
                    group = group.child(label(caption).font(Font::Slot(*font)).color(*color).at(r.x + at.0, r.y + at.1));
                }
                root = root.child(group);
                slider(*min, *max, step.max(0.), *value, move |v| q.borrow_mut().push(Action::Slide(index, v))).at(x0, r.y).width(Size::Px(133.)).height(Size::Px(28.))
            }
            Item::Combo { caption, font, color, at, value, items, edit_w, editable, edit_color } => {
                let box_x = r.x + r.w - edit_w;
                if !caption.is_empty() {
                    root = root.child(label(caption).font(Font::Slot(*font)).color(*color).at(r.x + at.0, r.y + at.1));
                }
                if *editable {
                    let q = queue.clone();
                    let current: f32 = value.trim().parse().unwrap_or(0.);
                    number_input(current, f32::MIN, f32::MAX, value.split_once('.').map_or(0, |(_, f)| f.len()).max(1), move |v| q.borrow_mut().push(Action::Edit(index, trim_number(v)))).font(Font::Slot(*font)).color(*edit_color).at(box_x, r.y).width(Size::Px(*edit_w)).height(Size::Px(24.))
                } else {
                    let q = queue.clone();
                    let mut shown = items.clone();
                    let selected = shown.iter().position(|i| i == value).unwrap_or_else(|| {
                        shown.push(value.clone());
                        shown.len() - 1
                    });
                    let count = items.len();
                    select(&shown, selected, move |i| {
                        if i < count {
                            q.borrow_mut().push(Action::Pick(index, i));
                        }
                    })
                    .font(Font::Slot(*font))
                    .color(*edit_color)
                    .at(box_x, r.y)
                    .width(Size::Px(*edit_w))
                    .height(Size::Px(24.))
                }
            }
            Item::Button { art, font, color } => {
                image_button((**art).clone())
                    .font(Font::Slot(*font))
                    .color(*color)
                    .on_click(push(Action::Click(index)))
                    .at(r.x, r.y)
                    .width(Size::Px(r.w))
                    .height(Size::Px(r.h))
            }
        };
        root = root.child(node.key(&key).disabled(e.disabled));
    }
    root
}

fn action_clone(action: &Action) -> Action {
    match action {
        Action::Click(i) => Action::Click(*i),
        Action::Toggle(i, on) => Action::Toggle(*i, *on),
        Action::Slide(i, v) => Action::Slide(*i, *v),
        Action::Pick(i, n) => Action::Pick(*i, *n),
        Action::Edit(i, t) => Action::Edit(*i, t.clone()),
    }
}

fn trim_number(value: f32) -> String {
    let text = format!("{value:.2}");
    let trimmed = text.trim_end_matches('0');
    if trimmed.ends_with('.') { format!("{trimmed}0") } else { trimmed.to_owned() }
}

// ---- hooks --------------------------------------------------------------------------------------

fn is_mirrored(a: CoreAccess<'_>, receiver: *mut UObject) -> bool {
    !receiver.is_null() && live(a, receiver) && PAGES.contains(&class_name_of(a, receiver).as_str())
}

unsafe fn param(a: CoreAccess<'_>, f: *mut UFunction, params: *const u8, size: usize, name: &str, kind: &str, width: usize) -> Option<*const u8> {
    let expected = usize::from(unsafe { (*f).parms_size });
    if params.is_null() || size < expected {
        return None;
    }
    let offset = unsafe { field(a, f.cast(), name, kind, width, expected, true) }.ok()?;
    Some(unsafe { params.add(offset) })
}

unsafe fn create(page: *mut UObject) -> Mirror {
    let store = Store::new();
    let entries = store.signal(Vec::<Entry>::new());
    let queue = Rc::new(RefCell::new(Vec::new()));
    let metrics = HostMetrics::default();
    let (view_entries, view_queue) = (entries.clone(), queue.clone());
    let mut ui = Ui::new(store, move || view(&view_entries.get(), &view_queue), metrics.clone()).with_skin(skin());
    ui.resize(640., 480.);
    Mirror { page: unsafe { Weak::new(page) }, ui, entries, metrics, queue }
}

unsafe fn paint_callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if std::env::var_os("HP2_UI_DEBUG").is_some() && !receiver.is_null() {
        thread_local! { static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
        let class = class_name_of(a, receiver);
        let kids: Vec<String> = unsafe { children(a, receiver) }.into_iter().map(|(_, c)| c).collect();
        let line = format!("HP2_UI_PAINT {class} {} children={kids:?}", name_of(a, function.cast()));
        if SEEN.with(|s| { let mut s = s.borrow_mut(); let new = !s.contains(&line); if new { s.push(line.clone()); } new }) {
            if let Ok(category) = a.intern_str("Log", FindName::Add) { let _ = a.emit_log(category, &line.chars().map(u32::from).collect::<Vec<_>>()); }
        }
    }
    if !is_mirrored(a, receiver) || name_of(a, function.cast()) != "Paint" {
        return Ok(());
    }
    let result = (|| -> RuntimeResult<()> {
        // `Paint(Canvas C, ...)` or `Paint(Canvas canvas, ...)` depending on the page.
        let canvas = unsafe { param(a, function, params, size, "C", "Core.ObjectProperty", size_of::<*mut c_void>()).or_else(|| param(a, function, params, size, "Canvas", "Core.ObjectProperty", size_of::<*mut c_void>())) };
        let Some(canvas) = canvas.map(|p| unsafe { p.cast::<*mut UObject>().read_unaligned() }) else { return Ok(()) };
        let surface = unsafe { Surface::new(a, receiver, canvas, TEXTURES.with(Rc::clone))? };
        let snapshot = unsafe { snapshot(a, receiver)? };
        MIRRORS.with(|mirrors| -> RuntimeResult<()> {
            let Ok(mut mirrors) = mirrors.try_borrow_mut() else { return Ok(()) };
            mirrors.retain(|m| m.page.live(a));
            if !mirrors.iter().any(|m| m.page.p == receiver) {
                if std::env::var_os("HP2_UI_DEBUG").is_some() {
                    for (child, class) in unsafe { children(a, receiver) } {
                        let line = unsafe { format!("HP2_UI_CHILD {} {class} vis={} rect=({},{},{},{}) text={:?} textxy=({},{}) align={} font={} up={} over={} down={} dis={} stretched={} region={} img=({},{}) bdisabled={}",
                            class_name_of(a, receiver), bool_of(a, child, "bWindowVisible"), f32_of(a, child, "WinLeft"), f32_of(a, child, "WinTop"), f32_of(a, child, "WinWidth"), f32_of(a, child, "WinHeight"),
                            string_of(a, child, "Text"), f32_of(a, child, "TextX"), f32_of(a, child, "TextY"), u8_of(a, child, "Align"), i32_of(a, child, "Font"),
                            !object_of(a, child, "UpTexture").is_null(), !object_of(a, child, "OverTexture").is_null(), !object_of(a, child, "DownTexture").is_null(), !object_of(a, child, "DisabledTexture").is_null(),
                            bool_of(a, child, "bStretched"), bool_of(a, child, "bUseRegion"), f32_of(a, child, "ImageX"), f32_of(a, child, "ImageY"), bool_of(a, child, "bDisabled")) };
                        if let Ok(category) = a.intern_str("Log", FindName::Add) { let _ = a.emit_log(category, &line.chars().map(u32::from).collect::<Vec<_>>()); }
                    }
                }
                mirrors.push(unsafe { create(receiver) });
            }
            let Some(m) = mirrors.iter_mut().find(|m| m.page.p == receiver) else { return Ok(()) };
            if m.queue.borrow().is_empty() {
                m.entries.set(snapshot);
            }
            m.ui.update();
            let mut learned = false;
            for (text, font) in m.ui.text_requests() {
                if !m.metrics.knows(&text, font) {
                    let measured = unsafe { surface.text_size(a, &text, font)? };
                    m.metrics.learn(&text, font, measured);
                    learned = true;
                }
            }
            if learned {
                m.ui.relayout();
            }
            let commands = m.ui.draw();
            unsafe { surface.render(a, &commands, &m.metrics) }
        })
    })();
    if let Err(error) = &result {
        warn(a, error);
    }
    Ok(())
}

fn with_mirror(a: CoreAccess<'_>, receiver: *mut UObject, change: impl FnOnce(&mut Mirror)) {
    MIRRORS.with(|mirrors| {
        if let Ok(mut mirrors) = mirrors.try_borrow_mut()
            && let Some(m) = mirrors.iter_mut().find(|m| m.page.p == receiver && m.page.live(a))
        {
            change(m);
        }
    });
}

unsafe fn input_callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if !is_mirrored(a, receiver) {
        return Ok(());
    }
    let name = name_of(a, function.cast());
    let name_id = a.intern_str(&name, FindName::Find)?;
    if unsafe { a.find_object_function_by_name(receiver, name_id, false)? } != function {
        return Ok(());
    }
    let float = |n: &str| unsafe { param(a, function, params, size, n, "Core.FloatProperty", 4) }.map(|p| unsafe { p.cast::<f32>().read_unaligned() });
    let int = |n: &str| unsafe { param(a, function, params, size, n, "Core.IntProperty", 4) }.map(|p| unsafe { p.cast::<i32>().read_unaligned() });
    match (name.as_str(), float("X"), float("Y")) {
        ("MouseMove", Some(x), Some(y)) => with_mirror(a, receiver, |m| m.ui.pointer_move(x, y)),
        ("LMouseDown", Some(x), Some(y)) => with_mirror(a, receiver, |m| m.ui.pointer_down(x, y, PointerButton::Left)),
        ("LMouseUp", Some(x), Some(y)) => with_mirror(a, receiver, |m| m.ui.pointer_up(x, y, PointerButton::Left)),
        ("MouseLeave", _, _) => with_mirror(a, receiver, |m| m.ui.pointer_move(-1000., -1000.)),
        // Keys go to the page's own handlers (for example key rebinding) unless a box is being typed in.
        ("KeyDown", _, _) => with_mirror(a, receiver, |m| {
            if m.ui.is_editing() {
                let key = match int("Key") {
                    Some(8) => Some(Key::Backspace),
                    Some(13) => Some(Key::Enter),
                    Some(27) => Some(Key::Escape),
                    Some(46) => Some(Key::Delete),
                    _ => None,
                };
                if let Some(key) = key {
                    m.ui.key(key);
                }
            }
        }),
        ("KeyType", _, _) => with_mirror(a, receiver, |m| {
            if m.ui.is_editing()
                && let Some(ch) = int("Key").and_then(|k| u32::try_from(k).ok()).and_then(char::from_u32)
            {
                m.ui.text(&ch.to_string());
            }
        }),
        _ => {}
    }
    Ok(())
}

unsafe fn class_function(a: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<*mut UFunction> {
    Ok(unsafe { a.find_field(class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>())
}

/// Observe the mirrored pages' painting for one draw.
pub(crate) unsafe fn install_paint(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !enabled() {
        return Ok(None);
    }
    let mut functions = Vec::new();
    for page in PAGES {
        if let Some(class) = unsafe { find_class(a, &format!("HGame.{page}"))? } {
            let paint = unsafe { class_function(a, class, "Paint")? };
            if !paint.is_null() && !functions.contains(&paint) {
                functions.push(paint);
            }
        }
    }
    if functions.is_empty() {
        return Ok(None);
    }
    Ok(Some(unsafe { a.install_script_observer("ui-mirror-paint", &functions, paint_callback)? }))
}

/// Observe pointer and key input for one tick.
pub(crate) unsafe fn install_input(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !enabled() {
        return Ok(None);
    }
    let mut functions = Vec::new();
    for page in PAGES {
        let Some(class) = (unsafe { find_class(a, &format!("HGame.{page}"))? }) else { continue };
        for name in ["MouseMove", "LMouseDown", "LMouseUp", "MouseLeave", "KeyDown", "KeyType"] {
            let f = unsafe { class_function(a, class, name)? };
            if !f.is_null() && !functions.contains(&f) {
                functions.push(f);
            }
        }
    }
    if functions.is_empty() {
        return Ok(None);
    }
    Ok(Some(unsafe { a.install_script_observer("ui-mirror-input", &functions, input_callback)? }))
}

unsafe fn perform(a: CoreAccess<'_>, action: Action) -> RuntimeResult<()> {
    PERFORMING.with(|p| p.set(true));
    let _done = Performing;
    const DE_CHANGE: u8 = 1;
    let target = |index: i32| -> RuntimeResult<*mut UObject> {
        let o = a.object_at(index).cast::<UObject>();
        if o.is_null() { Err(RuntimeError::new("Mirrored control is gone")) } else { Ok(o) }
    };
    let none = std::ptr::null_mut();
    unsafe {
        match action {
            Action::Click(i) => { invoke(a, target(i)?, "Click", &[("X", V::F(0.)), ("Y", V::F(0.))], &[], none)?; }
            Action::Toggle(i, on) => {
                let o = target(i)?;
                boolean(a, o, "bChecked", on)?;
                invoke(a, o, "Notify", &[("E", V::U8(DE_CHANGE))], &[], none)?;
            }
            Action::Slide(i, v) => { invoke(a, target(i)?, "SetValue", &[("NewValue", V::F(v))], &[], none)?; }
            Action::Pick(i, n) => { invoke(a, target(i)?, "SetSelectedIndex", &[("Index", V::I(i32::try_from(n).unwrap_or(0)))], &[], none)?; }
            Action::Edit(i, text) => { invoke(a, target(i)?, "SetValue", &[("NewValue", V::S(&text))], &[], none)?; }
        }
    }
    Ok(())
}

/// Replays queued actions on the stock controls; call once per engine tick.
pub(crate) unsafe fn tick(a: CoreAccess<'_>) -> RuntimeResult<()> {
    let work: Vec<Action> = MIRRORS.with(|mirrors| {
        let Ok(mut mirrors) = mirrors.try_borrow_mut() else { return Vec::new() };
        mirrors.retain(|m| m.page.live(a));
        mirrors.iter().flat_map(|m| std::mem::take(&mut *m.queue.borrow_mut())).collect()
    });
    for action in work {
        if let Err(error) = unsafe { perform(a, action) } {
            warn(a, &error);
        }
    }
    Ok(())
}

// ---- swallowing the stock controls' own painting and pointer handling ---------------------------

/// Stock control classes whose painting and pointer input the mirror takes over, with their
/// sub-windows (a combo's edit box and button).
const CONTROL_CLASSES: &[&str] = &[
    "UWindow.UWindowLabelControl",
    "UWindow.UWindowCheckbox",
    "HGame.HPMenuOptionCheckBox",
    "UWindow.UWindowHSliderControl",
    "HGame.HPMenuOptionHSlider",
    "UWindow.UWindowComboControl",
    "HGame.HPMenuOptionCombo",
    "UWindow.UWindowEditBox",
    "HGame.HPMenuOptionEditBox",
    "UWindow.UWindowComboButton",
    "HGame.HPMenuOptionComboButton",
    "UWindow.UWindowButton",
    "HGame.HPMenuRaisedButton",
];
/// `MouseEnter`/`MouseLeave` are not taken over: the stock controls keep their own tooltips and
/// hover sounds, which follow the real pointer exactly as before.
const POINTER_FUNCTIONS: &[&str] = &["MouseMove", "LMouseDown", "LMouseUp", "Click", "DoubleClick", "RMouseDown", "RMouseUp"];

/// The mirrored page that contains `window`, and `window`'s offset inside that page.
unsafe fn owning_page(a: CoreAccess<'_>, window: *mut UObject) -> Option<(*mut UObject, f32, f32)> {
    let (mut x, mut y) = (0., 0.);
    let mut current = window;
    for _ in 0..16 {
        let parent = unsafe { object_of(a, current, "ParentWindow") };
        if parent.is_null() {
            return None;
        }
        unsafe {
            x += f32_of(a, current, "WinLeft");
            y += f32_of(a, current, "WinTop");
        }
        let class = unsafe { (*parent).class.cast::<UClass>() };
        if PAGE_CLASSES.with(|c| c.borrow().contains(&class)) {
            return Some((parent, x, y));
        }
        current = parent;
    }
    None
}

unsafe fn control_callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    use hp2_core::ScriptPreCallDecision::{Consume, Continue};
    if receiver.is_null() || PERFORMING.with(std::cell::Cell::get) {
        return Ok(Continue);
    }
    let Some((page, dx, dy)) = (unsafe { owning_page(a, receiver) }) else { return Ok(Continue) };
    let name = name_of(a, function.cast());
    if name == "Paint" {
        return Ok(Consume);
    }
    let float = |n: &str| unsafe { param(a, function, params, size, n, "Core.FloatProperty", 4) }.map(|p| unsafe { p.cast::<f32>().read_unaligned() });
    match (name.as_str(), float("X"), float("Y")) {
        ("MouseMove", Some(x), Some(y)) => with_mirror(a, page, |m| m.ui.pointer_move(x + dx, y + dy)),
        ("LMouseDown", Some(x), Some(y)) => with_mirror(a, page, |m| m.ui.pointer_down(x + dx, y + dy, PointerButton::Left)),
        ("LMouseUp", Some(x), Some(y)) => with_mirror(a, page, |m| m.ui.pointer_up(x + dx, y + dy, PointerButton::Left)),
        _ => {}
    }
    Ok(Consume)
}

unsafe fn control_functions(a: CoreAccess<'_>, names: &[&str]) -> RuntimeResult<Vec<*mut UFunction>> {
    let mut functions = Vec::new();
    for class in CONTROL_CLASSES {
        let Some(class) = (unsafe { find_class(a, class)? }) else { continue };
        for name in names {
            let f = unsafe { class_function(a, class, name)? };
            if !f.is_null() && unsafe { (*f).return_value_offset } == u16::MAX && !functions.contains(&f) {
                functions.push(f);
            }
        }
    }
    Ok(functions)
}

unsafe fn refresh_page_classes(a: CoreAccess<'_>) -> RuntimeResult<()> {
    let mut classes = Vec::new();
    for page in PAGES {
        if let Some(class) = unsafe { find_class(a, &format!("HGame.{page}"))? } {
            classes.push(class);
        }
    }
    PAGE_CLASSES.with(|c| *c.borrow_mut() = classes);
    Ok(())
}

/// Swallow the mirrored controls' own painting for one draw.
pub(crate) unsafe fn install_control_paint(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !enabled() {
        return Ok(None);
    }
    unsafe { refresh_page_classes(a)? };
    let functions = unsafe { control_functions(a, &["Paint"])? };
    if functions.is_empty() {
        return Ok(None);
    }
    Ok(Some(unsafe { a.install_script_pre_observer("ui-mirror-control-paint", &functions, control_callback)? }))
}

/// Swallow the mirrored controls' own pointer handling for one tick and route it to the page's UI.
pub(crate) unsafe fn install_control_input(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !enabled() {
        return Ok(None);
    }
    unsafe { refresh_page_classes(a)? };
    let functions = unsafe { control_functions(a, POINTER_FUNCTIONS)? };
    if functions.is_empty() {
        return Ok(None);
    }
    Ok(Some(unsafe { a.install_script_pre_observer("ui-mirror-control-input", &functions, control_callback)? }))
}
