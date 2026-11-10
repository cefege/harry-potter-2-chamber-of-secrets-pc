//! The Sound/Video page written with `hp2-ui`. The stock page's controls stay alive but hidden:
//! they remain the source of truth and keep doing the real work (every setting still flows
//! through the stock `Notify` handlers and the `stock_*` modules); this module is the view and the
//! input handling on top of them.
use std::{cell::RefCell, collections::HashMap, ffi::c_void, rc::Rc};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{
    Align, Color, Edges, Font, Justify, Key, PointerButton, PopupMetrics, Signal, Size, Skin, SkinPart, SliderMetrics, Store, Style, Stylesheet, TextAlign, Ui,
    checkbox, col, label, row, select, slider,
};

use crate::{
    stock_graphics_menu::{boolean, children, field, function, live, object, prop, read_boolean, set_text, text, Weak},
    ui_host::{class_name_of, find_class, name_of, read_string, HostMetrics, Surface},
    ui_vm::{invoke, V},
};

const PAGE_CLASS: &str = "FESoundVideoPage";
const GREEN: Color = Color(40, 180, 40, 255);
const GREY: Color = Color(200, 200, 200, 255);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ctl {
    Music,
    Sound,
    Brightness,
    ObjectDetail,
    Scale,
    Resolution,
    Textures,
    Display,
    ShowFps,
    Advanced(usize),
}

enum Command {
    Slider(Ctl, f32),
    PickIndex(Ctl, usize),
    PickText(Ctl, String),
    Check(Ctl, bool),
}

struct AdvancedRow {
    caption: &'static str,
    options: Vec<String>,
    current: Signal<String>,
}

/// The view's state: signals mirrored from the stock controls plus the pending commands.
struct Data {
    music: Signal<f32>,
    sound: Signal<f32>,
    brightness: Signal<f32>,
    object_detail: Signal<f32>,
    scale: Signal<f32>,
    resolution: Signal<String>,
    resolution_options: Signal<Vec<String>>,
    textures: Signal<String>,
    texture_options: Signal<Vec<String>>,
    display: Signal<String>,
    display_options: Vec<String>,
    show_fps: Signal<bool>,
    advanced: Vec<AdvancedRow>,
    queue: Rc<RefCell<Vec<Command>>>,
}

impl Data {
    fn new(store: &Store) -> Rc<Self> {
        let advanced = crate::stock_graphics_menu::advanced_rows()
            .into_iter()
            .map(|(caption, options)| AdvancedRow { caption, options: options.into_iter().map(str::to_owned).collect(), current: store.signal(String::new()) })
            .collect();
        Rc::new(Self {
            music: store.signal(0.),
            sound: store.signal(0.),
            brightness: store.signal(0.),
            object_detail: store.signal(0.),
            scale: store.signal(0.),
            resolution: store.signal(String::new()),
            resolution_options: store.signal(Vec::new()),
            textures: store.signal(String::new()),
            texture_options: store.signal(Vec::new()),
            display: store.signal(String::new()),
            display_options: display_choices(),
            show_fps: store.signal(false),
            advanced,
            queue: Rc::new(RefCell::new(Vec::new())),
        })
    }
}

fn stylesheet() -> Stylesheet {
    Stylesheet::new()
        .with("header", Style { color: Color::WHITE, font: Font::Bold, ..Style::default() })
        .with("caption", Style { color: GREEN, font: Font::Bold, ..Style::default() })
        .with("small-caption", Style { color: GREEN, font: Font::Normal, ..Style::default() })
        .with("hint", Style { color: Color::WHITE, font: Font::Normal, ..Style::default() })
}

fn skin() -> Skin {
    const ICONS: &str = "HGame.Icons.";
    let named = |n: &str| format!("{ICONS}{n}");
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

// Optional extensions exist only in builds with their features.
#[cfg(feature = "sdl")]
fn display_choices() -> Vec<String> { crate::stock_display_control::choice_labels().into_iter().map(str::to_owned).collect() }
#[cfg(not(feature = "sdl"))]
fn display_choices() -> Vec<String> { Vec::new() }
#[cfg(feature = "sdl")]
fn display_combo(page: *mut UObject) -> Option<*mut UObject> { crate::stock_display_control::combo_of(page) }
#[cfg(not(feature = "sdl"))]
fn display_combo(_: *mut UObject) -> Option<*mut UObject> { None }
#[cfg(feature = "renderer-wgpu")]
fn scale_slider(page: *mut UObject) -> Option<*mut UObject> { crate::stock_render_scale_control::slider_of(page) }
#[cfg(not(feature = "renderer-wgpu"))]
fn scale_slider(_: *mut UObject) -> Option<*mut UObject> { None }

// ---- the view ----------------------------------------------------------------------------

const COLUMN_W: f32 = 280.;
const ART_W: f32 = 133.;

fn key_of(ctl: Ctl) -> String {
    format!("{ctl:?}")
}

fn slider_node(d: &Rc<Data>, ctl: Ctl, min: f32, max: f32, signal: &Signal<f32>) -> hp2_ui::Node {
    let (queue, signal_for_change) = (d.queue.clone(), signal.clone());
    slider(min, max, 1., signal.get(), move |v| {
        signal_for_change.set(v);
        queue.borrow_mut().push(Command::Slider(ctl, v));
    })
    .key(&key_of(ctl))
}

fn select_node(d: &Rc<Data>, ctl: Ctl, current: &Signal<String>, options: &[String], by_index: bool) -> hp2_ui::Node {
    let now = current.get();
    let mut shown: Vec<String> = options.to_vec();
    // A value the stock page reports that is not one of the choices (for example "Custom") is shown, not hidden.
    let selected = shown.iter().position(|o| *o == now).unwrap_or_else(|| {
        shown.push(now.clone());
        shown.len() - 1
    });
    let (queue, signal, choices) = (d.queue.clone(), current.clone(), options.to_vec());
    select(&shown, selected, move |i| {
        let Some(choice) = choices.get(i) else { return };
        signal.set(choice.clone());
        queue.borrow_mut().push(if by_index { Command::PickIndex(ctl, i) } else { Command::PickText(ctl, choice.clone()) });
    })
    .key(&key_of(ctl))
    .color(GREY)
    .font(Font::Bold)
}

/// Caption on the left and a value box on the right, the box right-aligned in the column.
fn combo_row(caption: &str, select_node: hp2_ui::Node, height: f32) -> hp2_ui::Node {
    row().width(Size::Px(COLUMN_W)).height(Size::Px(height)).justify(Justify::End).gap(20.).child(label(caption).class("caption").height(Size::Px(24.))).child(select_node)
}

/// Caption on the left, slider right-aligned with Low/High under it.
fn slider_row(caption: &str, slider_node: hp2_ui::Node, height: f32) -> hp2_ui::Node {
    let hints = row().width(Size::Px(ART_W)).justify(Justify::SpaceBetween).child(label("Low").class("hint")).child(label("High").class("hint"));
    row()
        .width(Size::Px(COLUMN_W))
        .height(Size::Px(height))
        .justify(Justify::SpaceBetween)
        .child(label(caption).class("caption").height(Size::Px(24.)).padding(Edges { top: 3., ..Edges::default() }))
        .child(col().width(Size::Px(ART_W)).gap(4.).child(slider_node).child(hints))
}

/// A volume block: the live caption above the slider.
fn volume_block(caption: String, slider_node: hp2_ui::Node) -> hp2_ui::Node {
    let hints = row().width(Size::Px(ART_W)).justify(Justify::SpaceBetween).child(label("Low").class("hint")).child(label("High").class("hint"));
    row().width(Size::Px(COLUMN_W)).height(Size::Px(72.)).justify(Justify::End).child(
        col().width(Size::Px(ART_W)).gap(0.).child(label(&caption).class("caption").height(Size::Px(20.))).child(slider_node).child(col().height(Size::Px(4.))).child(hints),
    )
}

fn view(d: &Rc<Data>) -> hp2_ui::Node {
    let resolution = select_node(d, Ctl::Resolution, &d.resolution, &d.resolution_options.get(), true);
    let textures = select_node(d, Ctl::Textures, &d.textures, &d.texture_options.get(), true);
    let video = col()
        .width(Size::Px(COLUMN_W))
        .child(label("Video").class("header").height(Size::Px(26.)))
        .child(combo_row("Logical res.", resolution, 28.))
        .child(combo_row("Textures", textures, 30.))
        .child(slider_row("Object Detail", slider_node(d, Ctl::ObjectDetail, 0., 4., &d.object_detail), 46.))
        .child(slider_row("Brightness", slider_node(d, Ctl::Brightness, 2., 10., &d.brightness), 46.))
        .child({
            let percent = scale_percent(d.scale.get());
            let scale = slider_node(d, Ctl::Scale, 0., 11., &d.scale);
            row()
                .width(Size::Px(COLUMN_W))
                .height(Size::Px(30.))
                .justify(Justify::SpaceBetween)
                .child(label(&format!("3D Scale: {percent}%")).class("small-caption").height(Size::Px(24.)))
                .child(col().width(Size::Px(ART_W)).child(scale))
        });
    let display = select_node(d, Ctl::Display, &d.display, &d.display_options, false);
    let fps = {
        let (queue, signal) = (d.queue.clone(), d.show_fps.clone());
        checkbox("Show FPS", d.show_fps.get(), move |on| {
            signal.set(on);
            queue.borrow_mut().push(Command::Check(Ctl::ShowFps, on));
        })
        .key(&key_of(Ctl::ShowFps))
        .class("caption")
    };
    let audio = col()
        .width(Size::Px(COLUMN_W))
        .child(label("Audio").class("header").height(Size::Px(26.)))
        .child(volume_block(format!("Music Volume - {}", d.music.get() as i32), slider_node(d, Ctl::Music, 0., 100., &d.music)))
        .child(volume_block(format!("Sound Volume - {}", d.sound.get() as i32), slider_node(d, Ctl::Sound, 0., 100., &d.sound)))
        .child(combo_row("Display", display, 32.))
        .child(row().width(Size::Px(COLUMN_W)).height(Size::Px(26.)).justify(Justify::End).child(col().width(Size::Px(ART_W)).child(fps)));

    // Advanced: four on the left, the rest on the right.
    let (left, right): (Vec<_>, Vec<_>) = d.advanced.iter().enumerate().partition(|(i, _)| *i < 4);
    let advanced_column = |rows: Vec<(usize, &AdvancedRow)>| {
        col().width(Size::Px(COLUMN_W)).children(rows.into_iter().map(|(i, r)| combo_row(r.caption, select_node(d, Ctl::Advanced(i), &r.current, &r.options, false), 28.)))
    };

    col().padding(Edges { top: 44., left: 30., ..Edges::default() }).child(row().gap(16.).height(Size::Px(238.)).align(Align::Start).child(video).child(audio)).child(
        col().child(label("Advanced").class("header").height(Size::Px(26.))).child(row().gap(16.).child(advanced_column(left)).child(advanced_column(right))),
    )
}

/// The stock 12-step render-scale slider as a percent.
fn scale_percent(step: f32) -> i32 {
    const SCALES: [i32; 12] = [50, 55, 60, 65, 67, 70, 75, 80, 85, 90, 95, 100];
    SCALES.get(step.round().max(0.) as usize).copied().unwrap_or(100)
}

// ---- page state -------------------------------------------------------------------------------

struct PageUi {
    page: Weak,
    ui: Ui,
    data: Rc<Data>,
    metrics: HostMetrics,
    hidden: bool,
}

thread_local! {
    static PAGES: RefCell<Vec<PageUi>> = const { RefCell::new(Vec::new()) };
    static TEXTURES: Rc<RefCell<HashMap<String, (*mut UObject, i32)>>> = Rc::new(RefCell::new(HashMap::new()));
}

fn warn(a: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = a.intern_str("Warning", FindName::Add) {
        let _ = a.emit_log(category, &format!("Rust menu: {error}").chars().map(u32::from).collect::<Vec<_>>());
    }
}

fn is_page(a: CoreAccess<'_>, receiver: *mut UObject) -> bool {
    !receiver.is_null() && live(a, receiver) && class_name_of(a, receiver) == PAGE_CLASS
}

/// The stock controls the view mirrors and drives.
struct Handles {
    music: *mut UObject,
    sound: *mut UObject,
    brightness: *mut UObject,
    object_detail: *mut UObject,
    scale: Option<*mut UObject>,
    resolution: *mut UObject,
    textures: *mut UObject,
    display: Option<*mut UObject>,
    fps: Option<*mut UObject>,
    advanced: Vec<*mut UObject>,
}

impl Handles {
    unsafe fn resolve(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<Self> {
        let named = |name: &str| -> RuntimeResult<*mut UObject> {
            let o = unsafe { object(a, page, name)? };
            if live(a, o) { Ok(o) } else { Err(RuntimeError::new(&format!("Sound/Video page lacks {name}"))) }
        };
        Ok(Self {
            music: named("MusicVolumeSlider")?,
            sound: named("SoundVolumeSlider")?,
            brightness: named("BrightnessSlider")?,
            object_detail: named("ObjectDetailSlider")?,
            scale: scale_slider(page),
            resolution: named("ResolutionCombo")?,
            textures: named("TextureDetailCombo")?,
            display: display_combo(page),
            fps: crate::stock_fps_menu::checkbox_of(a, page),
            advanced: crate::stock_graphics_menu::advanced_combos(page),
        })
    }
}

unsafe fn slider_value(a: CoreAccess<'_>, slider: *mut UObject) -> RuntimeResult<f32> {
    Ok(unsafe { prop(a, slider, "Value", "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned() })
}

/// The choices of a stock combo (`List.Items` is a sentinel followed by the items).
unsafe fn combo_items(a: CoreAccess<'_>, combo: *mut UObject) -> RuntimeResult<Vec<String>> {
    let list = unsafe { object(a, combo, "List")? };
    let mut item = unsafe { object(a, object(a, list, "Items")?, "Next")? };
    let mut out = Vec::new();
    while live(a, item) && out.len() < 64 {
        out.push(unsafe { read_string(a, item, "Value")? });
        item = unsafe { object(a, item, "Next")? };
    }
    Ok(out)
}

/// Copies the stock controls' current state into the view's signals.
unsafe fn refresh(a: CoreAccess<'_>, data: &Data, h: &Handles) -> RuntimeResult<()> {
    unsafe {
        data.music.set(slider_value(a, h.music)?);
        data.sound.set(slider_value(a, h.sound)?);
        data.brightness.set(slider_value(a, h.brightness)?);
        data.object_detail.set(slider_value(a, h.object_detail)?);
        if let Some(scale) = h.scale {
            data.scale.set(slider_value(a, scale)?);
        }
        data.resolution_options.set(combo_items(a, h.resolution)?);
        data.resolution.set(text(a, h.resolution)?);
        data.texture_options.set(combo_items(a, h.textures)?);
        data.textures.set(text(a, h.textures)?);
        if let Some(display) = h.display {
            data.display.set(text(a, display)?);
        }
        if let Some(fps) = h.fps {
            data.show_fps.set(read_boolean(a, fps, "bChecked")?);
        }
        for (row, combo) in data.advanced.iter().zip(&h.advanced) {
            row.current.set(text(a, *combo)?);
        }
    }
    Ok(())
}

unsafe fn param(a: CoreAccess<'_>, f: *mut UFunction, params: *const u8, size: usize, name: &str, kind: &str, width: usize) -> Option<*const u8> {
    let expected = usize::from(unsafe { (*f).parms_size });
    if params.is_null() || size < expected {
        return None;
    }
    let offset = unsafe { field(a, f.cast(), name, kind, width, expected, true) }.ok()?;
    Some(unsafe { params.add(offset) })
}

unsafe fn float_param(a: CoreAccess<'_>, f: *mut UFunction, params: *const u8, size: usize, name: &str) -> Option<f32> {
    unsafe { param(a, f, params, size, name, "Core.FloatProperty", 4) }.map(|p| unsafe { p.cast::<f32>().read_unaligned() })
}

unsafe fn int_param(a: CoreAccess<'_>, f: *mut UFunction, params: *const u8, size: usize, name: &str) -> Option<i32> {
    unsafe { param(a, f, params, size, name, "Core.IntProperty", 4) }.map(|p| unsafe { p.cast::<i32>().read_unaligned() })
}

/// Builds the page's UI the first time it is painted (the stock controls exist by then).
unsafe fn create(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<PageUi> {
    let store = Store::new();
    let data = Data::new(&store);
    let metrics = HostMetrics::default();
    let view_data = data.clone();
    let mut ui = Ui::new(store, move || view(&view_data), metrics.clone()).with_stylesheet(stylesheet()).with_skin(skin());
    ui.resize(640., 480.);
    let handles = unsafe { Handles::resolve(a, page)? };
    unsafe { refresh(a, &data, &handles)? };
    Ok(PageUi { page: unsafe { Weak::new(page) }, ui, data, metrics, hidden: false })
}

/// Removes every stock control except the Back button from the page's window list.
unsafe fn hide_stock_controls(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<()> {
    let back = unsafe { object(a, page, "BackPageButton")? };
    for (child, _) in unsafe { children(a, page) } {
        if child != back {
            unsafe { invoke(a, child, "HideWindow", &[], &[], std::ptr::null_mut())? };
        }
    }
    Ok(())
}

unsafe fn paint_callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if !is_page(a, receiver) || name_of(a, function.cast()) != "Paint" {
        return Ok(());
    }
    let result = (|| -> RuntimeResult<()> {
        let canvas_slot = unsafe { param(a, function, params, size, "C", "Core.ObjectProperty", size_of::<*mut c_void>()) };
        let Some(canvas) = canvas_slot.map(|p| unsafe { p.cast::<*mut UObject>().read_unaligned() }) else { return Ok(()) };
        let surface = unsafe { Surface::new(a, receiver, canvas, TEXTURES.with(Rc::clone))? };
        PAGES.with(|pages| -> RuntimeResult<()> {
            let Ok(mut pages) = pages.try_borrow_mut() else { return Ok(()) };
            pages.retain(|p| p.page.live(a));
            if !pages.iter().any(|p| p.page.p == receiver) {
                pages.push(unsafe { create(a, receiver)? });
            }
            let Some(entry) = pages.iter_mut().find(|p| p.page.p == receiver) else { return Ok(()) };
            if !entry.hidden {
                unsafe { hide_stock_controls(a, receiver)? };
                entry.hidden = true;
            }
            // While a command is waiting for the next tick, the stock controls are stale: do not overwrite the view.
            if entry.data.queue.borrow().is_empty() {
                let handles = unsafe { Handles::resolve(a, receiver)? };
                unsafe { refresh(a, &entry.data, &handles)? };
            }
            entry.ui.update();
            let mut learned = false;
            for (text_value, font) in entry.ui.text_requests() {
                if !entry.metrics.knows(&text_value, font) {
                    let size = unsafe { surface.text_size(a, &text_value, font)? };
                    entry.metrics.learn(&text_value, font, size);
                    learned = true;
                }
            }
            if learned {
                entry.ui.relayout();
            }
            let commands = entry.ui.draw();
            unsafe { surface.render(a, &commands, &entry.metrics) }
        })
    })();
    if let Err(error) = &result {
        warn(a, error);
    }
    Ok(())
}

fn with_page(a: CoreAccess<'_>, receiver: *mut UObject, change: impl FnOnce(&mut PageUi)) {
    PAGES.with(|pages| {
        if let Ok(mut pages) = pages.try_borrow_mut()
            && let Some(entry) = pages.iter_mut().find(|p| p.page.p == receiver && p.page.live(a))
        {
            change(entry);
        }
    });
}

fn map_key(code: i32) -> Option<Key> {
    Some(match code {
        8 => Key::Backspace,
        9 => Key::Tab,
        13 => Key::Enter,
        27 => Key::Escape,
        32 => Key::Space,
        37 => Key::Left,
        38 => Key::Up,
        39 => Key::Right,
        40 => Key::Down,
        46 => Key::Delete,
        _ => return None,
    })
}

unsafe fn input_callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if !is_page(a, receiver) {
        return Ok(());
    }
    // An explicit `Super.X()` call reaches the observer too; only the receiver's own dispatch counts.
    let name = name_of(a, function.cast());
    let name_id = a.intern_str(&name, FindName::Find)?;
    if unsafe { a.find_object_function_by_name(receiver, name_id, false)? } != function {
        return Ok(());
    }
    let (x, y) = unsafe { (float_param(a, function, params, size, "X"), float_param(a, function, params, size, "Y")) };
    if debug() && name != "MouseMove" {
        log(a, &format!("HP2_UI_INPUT {name} {x:?} {y:?}"));
    }
    match (name.as_str(), x, y) {
        ("MouseMove", Some(x), Some(y)) => with_page(a, receiver, |p| p.ui.pointer_move(x, y)),
        ("LMouseDown", Some(x), Some(y)) => with_page(a, receiver, |p| p.ui.pointer_down(x, y, PointerButton::Left)),
        ("LMouseUp", Some(x), Some(y)) => with_page(a, receiver, |p| p.ui.pointer_up(x, y, PointerButton::Left)),
        ("MouseLeave", _, _) => with_page(a, receiver, |p| p.ui.pointer_move(-1000., -1000.)),
        ("KeyDown", _, _) => {
            if let Some(key) = unsafe { int_param(a, function, params, size, "Key") }.and_then(map_key) {
                with_page(a, receiver, |p| p.ui.key(key));
            }
        }
        ("KeyType", _, _) => {
            if let Some(ch) = unsafe { int_param(a, function, params, size, "Key") }.and_then(|k| u32::try_from(k).ok()).and_then(char::from_u32) {
                with_page(a, receiver, |p| p.ui.text(&ch.to_string()));
            }
        }
        _ => {}
    }
    Ok(())
}

unsafe fn page_functions(a: CoreAccess<'_>, names: &[&str]) -> RuntimeResult<Option<Vec<*mut UFunction>>> {
    // `HP2_STOCK_MENUS=1` keeps the stock UnrealScript page (for comparison and as a fallback).
    if std::env::var_os("HP2_STOCK_MENUS").is_some() {
        return Ok(None);
    }
    let Some(page) = (unsafe { find_class(a, &format!("HGame.{PAGE_CLASS}"))? }) else { return Ok(None) };
    let mut out = Vec::new();
    for name in names {
        let f = unsafe { a.find_field(page.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>();
        if f.is_null() {
            return Ok(None);
        }
        out.push(f);
    }
    Ok(Some(out))
}

/// Observe page painting for one draw.
pub(crate) unsafe fn install_paint(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(functions) = (unsafe { page_functions(a, &["Paint"])? }) else { return Ok(None) };
    Ok(Some(unsafe { a.install_script_observer("ui-pages-paint", &functions, paint_callback)? }))
}

/// Observe pointer and keyboard input for one tick.
pub(crate) unsafe fn install_input(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(functions) = (unsafe { page_functions(a, &["MouseMove", "LMouseDown", "LMouseUp", "MouseLeave", "KeyDown", "KeyType"])? }) else { return Ok(None) };
    Ok(Some(unsafe { a.install_script_observer("ui-pages-input", &functions, input_callback)? }))
}

/// Raises `DE_Change` on a stock control. The control's own script forwards it to the page's
/// `Notify`, exactly as a click would, so script-call hooks and observers see it.
unsafe fn notify(a: CoreAccess<'_>, _page: *mut UObject, control: *mut UObject) -> RuntimeResult<()> {
    const DE_CHANGE: u8 = 1;
    unsafe { invoke(a, control, "Notify", &[("E", V::U8(DE_CHANGE))], &[], std::ptr::null_mut())? };
    Ok(())
}

fn debug() -> bool {
    std::env::var_os("HP2_UI_DEBUG").is_some()
}

fn log(a: CoreAccess<'_>, line: &str) {
    if let Ok(category) = a.intern_str("Log", FindName::Add) {
        let _ = a.emit_log(category, &line.chars().map(u32::from).collect::<Vec<_>>());
    }
}

unsafe fn execute(a: CoreAccess<'_>, page: *mut UObject, h: &Handles, command: Command) -> RuntimeResult<()> {
    if debug() {
        let what = match &command {
            Command::Slider(c, v) => format!("slider {c:?} {v}"),
            Command::PickIndex(c, i) => format!("index {c:?} {i}"),
            Command::PickText(c, t) => format!("text {c:?} {t}"),
            Command::Check(c, on) => format!("check {c:?} {on}"),
        };
        log(a, &format!("HP2_UI_EXEC {what} active={}", crate::stock_menu_extensions::active()));
    }
    match command {
        Command::Slider(ctl, value) => {
            let slider = match ctl {
                Ctl::Music => Some(h.music),
                Ctl::Sound => Some(h.sound),
                Ctl::Brightness => Some(h.brightness),
                Ctl::ObjectDetail => Some(h.object_detail),
                Ctl::Scale => h.scale,
                _ => None,
            };
            if let Some(slider) = slider {
                unsafe { invoke(a, slider, "SetValue", &[("NewValue", V::F(value))], &[], std::ptr::null_mut())? };
            }
        }
        Command::PickIndex(ctl, index) => {
            let combo = if ctl == Ctl::Resolution { h.resolution } else { h.textures };
            unsafe { invoke(a, combo, "SetSelectedIndex", &[("Index", V::I(i32::try_from(index).unwrap_or(0)))], &[], std::ptr::null_mut())? };
        }
        Command::PickText(ctl, choice) => {
            let combo = match ctl {
                Ctl::Display => h.display,
                Ctl::Advanced(i) => h.advanced.get(i).copied(),
                _ => None,
            };
            if let Some(combo) = combo {
                unsafe { set_text(a, combo, &choice)? };
                unsafe { notify(a, page, combo)? };
            }
        }
        Command::Check(_, on) => {
            if let Some(fps) = h.fps {
                unsafe { boolean(a, fps, "bChecked", on)? };
                unsafe { notify(a, page, fps)? };
            }
        }
    }
    Ok(())
}

/// Runs the commands the view queued. Called inside the engine tick, where the stock hook that
/// drives the `stock_*` observers is active, so the same `Notify` path as a stock click runs.
pub(crate) unsafe fn tick(a: CoreAccess<'_>) -> RuntimeResult<()> {
    let work: Vec<(*mut UObject, Vec<Command>)> = PAGES.with(|pages| {
        let Ok(mut pages) = pages.try_borrow_mut() else { return Vec::new() };
        pages.retain(|p| p.page.live(a));
        pages.iter().map(|p| (p.page.p, std::mem::take(&mut *p.data.queue.borrow_mut()))).filter(|(_, c)| !c.is_empty()).collect()
    });
    for (page, commands) in work {
        let handles = unsafe { Handles::resolve(a, page)? };
        for command in commands {
            unsafe { execute(a, page, &handles, command)? };
        }
    }
    Ok(())
}
