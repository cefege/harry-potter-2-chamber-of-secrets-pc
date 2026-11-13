//! The in-game HUD drawn with `hp2-ui`, starting with the cutscene letterbox and subtitles.
//!
//! The HUD scripts keep their state machines (when a cutscene starts, what is said, when it
//! ends); only their drawing is taken over by script pre-call hooks:
//! * `CutSceneManager.DrawBorder`: the letterbox;
//! * `HPHud.DrawCutStyleText`: every cutscene subtitle and folio card description, with real word
//!   wrapping, centred lines and a drop shadow (the stock code guessed line counts with a fixed
//!   "+90" fudge factor and could overflow the border);
//! * `CutSceneManager.SlideIn/SlideOut.SetCurrBorderHeight`: the border used to slide 1/25 of its
//!   height per *rendered frame* (0.17 s at 144 fps, 0.8 s at 30 fps); it now slides at the
//!   30-fps speed whatever the frame rate.
//!
//! * `StatusItem.DrawItem` / `DrawCount`: every status counter (beans, potions, keys, cards...)
//!   that does not draw itself; counts get a contrasting shadow.
//!
//! `HP2_STOCK_HUD=1` restores the stock drawing.
use std::{cell::RefCell, collections::HashMap, ffi::c_void, ptr};

use hp2_abi::engine::generated::{UFunction, UObject};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName, ScriptPreCallDecision};
use hp2_ui::{BlendMode, Color, DrawCmd, Fit, Font, Rect, Size, Store, TextAlign, TextureRef, Ui, col, label};

#[path = "ui_spell_selector.rs"]
mod spell_selector;

#[path = "ui_magic_strength.rs"]
mod magic_strength;
#[path = "ui_countdown.rs"]
mod countdown;

#[path = "ui_player_health.rs"]
mod player_health;
#[path = "ui_enemy_health.rs"]
mod enemy_health;
#[path = "ui_status_groups.rs"]
mod status_groups;
#[path = "ui_vendor.rs"]
mod vendor;
#[path = "ui_pickups.rs"]
mod pickups;
#[path = "ui_popups.rs"]
mod popups;
#[path = "ui_quidditch_bar.rs"]
mod quidditch_bar;
#[path = "ui_quidditch_score.rs"]
mod quidditch_score;
#[path = "ui_challenge_score.rs"]
mod challenge_score;
#[path = "ui_spell_lesson.rs"]
mod spell_lesson;
#[path = "ui_cutscene.rs"]
mod cutscene;
#[path = "ui_muggle_meter.rs"]
mod muggle_meter;
#[path = "ui_extra_hud.rs"]
mod extra_hud;
#[path = "ui_ceremony.rs"]
mod ceremony;
#[path = "ui_weapon_overlay.rs"]
mod weapon_overlay;

use crate::{
    stock_graphics_menu::{field, is_a, live, object, prop, read_boolean},
    ui_host::{find_texture, name_of, render_commands, DrawTarget, HostMetrics},
};

/// Stock border slide: 25 steps at the 30 fps the game was tuned for.
const SLIDE_SECONDS: f32 = 25. / 30.;
const SUBTITLE_SHADOW: Color = Color(0, 0, 0, 255);

fn enabled() -> bool {
    std::env::var_os("HP2_STOCK_HUD").is_none()
}


struct HudGuards<A, B> { _pre: A, _components: B }
impl<A, B> Drop for HudGuards<A, B> { fn drop(&mut self) {} }

fn warn(a: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = a.intern_str("Warning", FindName::Add) {
        let _ = a.emit_log(category, &format!("Rust HUD: {error}").chars().map(u32::from).collect::<Vec<_>>());
    }
}

unsafe fn f32_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> RuntimeResult<f32> {
    Ok(unsafe { prop(a, o, name, "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned() })
}
unsafe fn set_f32(a: CoreAccess<'_>, o: *mut UObject, name: &str, value: f32) -> RuntimeResult<()> {
    unsafe { prop(a, o, name, "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(value) };
    Ok(())
}
unsafe fn int_of(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> RuntimeResult<i32> {
    Ok(unsafe { prop(a, o, name, "Core.IntProperty", 4)?.cast::<i32>().read_unaligned() })
}

/// A HUD canvas: draws in pixels with the canvas' own natives.
struct CanvasTarget {
    canvas: *mut UObject,
    /// Render style for tiles; `None` keeps the canvas' current style (as `DrawIcon` does).
    tile_style: Option<u8>,
    /// Font slots: 0 big, 1 medium, 2 small, 3 tiny (the console's localized fonts).
    fonts: [*mut UObject; 4],
}

impl CanvasTarget {
    unsafe fn set_pos(&self, a: CoreAccess<'_>, x: f32, y: f32) -> RuntimeResult<()> {
        unsafe {
            set_f32(a, self.canvas, "CurX", x)?;
            set_f32(a, self.canvas, "CurY", y)?;
        }
        Ok(())
    }
    unsafe fn set_color(&self, a: CoreAccess<'_>, color: [u8; 4]) -> RuntimeResult<()> {
        unsafe { prop(a, self.canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(color) };
        Ok(())
    }
    unsafe fn set_font(&self, a: CoreAccess<'_>, font: Font) -> RuntimeResult<()> {
        let slot = match font {
            Font::Slot(n) => usize::from(n).min(3),
            Font::Bold | Font::Heading => 0,
            Font::Normal => 1,
        };
        unsafe { prop(a, self.canvas, "Font", "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(self.fonts[slot]) };
        Ok(())
    }
    unsafe fn text_size(&self, a: CoreAccess<'_>, text: &str, font: Font) -> RuntimeResult<(f32, f32)> {
        unsafe { self.set_font(a, font)? };
        // Canvas natives read their arguments from script bytecode, so call their Rust bodies directly.
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        let size = unsafe { crate::canvas_owner::native_text_size(a, self.canvas.cast(), &units)? };
        Ok(size.map_or((0., 0.), |(w, h)| (w as f32, h as f32)))
    }
    /// `ERenderStyle`: 1 normal, 2 masked (what the stock HUD text uses).
    unsafe fn set_style(&self, a: CoreAccess<'_>, style: u8) -> RuntimeResult<()> {
        unsafe { prop(a, self.canvas, "Style", "Core.ByteProperty", 1)?.write(style) };
        Ok(())
    }
    unsafe fn tile(&self, a: CoreAccess<'_>, tex: *mut UObject, dest: Rect, src: Rect) -> RuntimeResult<()> {
        unsafe {
            if let Some(style) = self.tile_style {
                self.set_style(a, style)?;
            }
            self.set_pos(a, dest.x, dest.y)?;
            crate::canvas_owner::native_draw_tile(a, self.canvas.cast(), tex.cast(), dest.w, dest.h, src.x, src.y, src.w, src.h)?;
        }
        Ok(())
    }
}

impl DrawTarget for CanvasTarget {
    unsafe fn fill(&self, a: CoreAccess<'_>, rect: Rect, color: [u8; 4]) -> RuntimeResult<()> {
        let white = unsafe { find_texture(a, "UWindow.WhiteTexture")? };
        unsafe {
            self.set_color(a, color)?;
            self.tile(a, white, rect, Rect::new(0., 0., 1., 1.))
        }
    }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        if let Some(index) = name.strip_prefix('#').and_then(|i| i.parse::<i32>().ok()) {
            let object = a.object_at(index).cast::<UObject>();
            return if object.is_null() { Err(RuntimeError::new("HUD texture is gone")) } else { Ok(object) };
        }
        unsafe { find_texture(a, name) }
    }
    unsafe fn segment(&self, a: CoreAccess<'_>, tex: *mut UObject, dest: Rect, src: Rect, color: [u8; 4], blend: BlendMode) -> RuntimeResult<()> {
        unsafe {
            self.set_color(a, color)?;
            if let Some(style) = crate::ui_host::canvas_blend(blend) {
                self.set_style(a, style)?;
                self.set_pos(a, dest.x, dest.y)?;
                crate::canvas_owner::native_draw_tile(a, self.canvas.cast(), tex.cast(), dest.w, dest.h, src.x, src.y, src.w, src.h)
            } else {
                self.tile(a, tex, dest, src)
            }
        }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, font: Font, color: [u8; 4]) -> RuntimeResult<()> {
        unsafe {
            self.set_font(a, font)?;
            self.set_color(a, color)?;
            self.set_style(a, 2)?;
            self.set_pos(a, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.canvas.cast(), &units, false, 0)?;
        }
        Ok(())
    }
}

/// Saves the canvas state the stock HUD code expects to find unchanged, and restores it.
struct CanvasState {
    canvas: *mut UObject,
    font: *mut UObject,
    color: [u8; 4],
    style: u8,
    pos: (f32, f32),
}

impl CanvasState {
    unsafe fn save(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<Self> {
        unsafe {
            Ok(Self {
                canvas,
                font: object(a, canvas, "Font")?,
                color: prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned(),
                style: prop(a, canvas, "Style", "Core.ByteProperty", 1)?.read(),
                pos: (f32_of(a, canvas, "CurX")?, f32_of(a, canvas, "CurY")?),
            })
        }
    }
    unsafe fn restore(&self, a: CoreAccess<'_>) -> RuntimeResult<()> {
        unsafe {
            prop(a, self.canvas, "Font", "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(self.font);
            prop(a, self.canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(self.color);
            prop(a, self.canvas, "Style", "Core.ByteProperty", 1)?.write(self.style);
            set_f32(a, self.canvas, "CurX", self.pos.0)?;
            set_f32(a, self.canvas, "CurY", self.pos.1)?;
        }
        Ok(())
    }
}

/// Splits `text` into lines no wider than `width`. Words wider than a line stay on their own line.
fn wrap(text: &str, width: f32, measure: &mut dyn FnMut(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && measure(&candidate) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

thread_local! {
    static METRICS: HostMetrics = HostMetrics::default();
    /// Last game time each manager's border slide was advanced at.
    static SLIDE_CLOCK: RefCell<HashMap<i32, f32>> = RefCell::new(HashMap::new());
}

unsafe fn player_fonts(a: CoreAccess<'_>, pawn: *mut UObject) -> RuntimeResult<[*mut UObject; 4]> {
    if !live(a, pawn) {
        return Err(RuntimeError::new("HUD owner unavailable"));
    }
    let player = unsafe { object(a, pawn, "Player")? };
    let console = unsafe { object(a, player, "Console")? };
    if !live(a, console) {
        return Err(RuntimeError::new("HUD console unavailable"));
    }
    let font = |name: &str| unsafe { object(a, console, name) };
    Ok([font("LocalBigFont")?, font("LocalMedFont")?, font("LocalSmallFont")?, font("LocalTinyFont")?])
}

unsafe fn param_object(a: CoreAccess<'_>, f: *mut UFunction, params: *const u8, size: usize, name: &str) -> Option<*mut UObject> {
    let expected = usize::from(unsafe { (*f).parms_size });
    if params.is_null() || size < expected {
        return None;
    }
    let offset = unsafe { field(a, f.cast(), name, "Core.ObjectProperty", size_of::<*mut c_void>(), expected, true) }.ok()?;
    Some(unsafe { params.add(offset).cast::<*mut UObject>().read_unaligned() })
}

/// The letterbox borders.
unsafe fn draw_border(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject) -> RuntimeResult<()> {
    let (w, h) = unsafe { (int_of(a, canvas, "SizeX")? as f32, int_of(a, canvas, "SizeY")? as f32) };
    let height = unsafe { f32_of(a, manager, "fCurrBorderHeight")? };
    let level = unsafe { object(a, manager, "Level")? };
    let harry = unsafe { object(a, level, "PlayerHarryActor")? };
    let captured = live(a, harry) && unsafe { read_boolean(a, harry, "bIsCaptured") }.unwrap_or(false);
    let border = unsafe { object(a, manager, "textureBorder")? };
    let target = CanvasTarget { canvas, tile_style: Some(1), fonts: [ptr::null_mut(); 4] };
    let mut rects = vec![Rect::new(0., h - height, w, height)];
    if captured {
        rects.push(Rect::new(0., 0., w, height));
    }
    for rect in rects {
        if rect.h <= 0. {
            continue;
        }
        if live(a, border) {
            unsafe { target.segment(a, border, rect, Rect::new(0., 0., 1., 1.), [255; 4], BlendMode::Inherit)? };
        } else {
            unsafe { target.fill(a, rect, [0, 0, 0, 255])? };
        }
    }
    Ok(())
}

/// Draws `text` wrapped and centred inside the band `x0..w - x0`, `top..top + height`, with a
/// drop shadow. The largest of `fonts` whose lines fit the band is used. Short blocks are centred
/// vertically (subtitles); taller ones start at the top (folio card descriptions).
unsafe fn draw_wrapped(a: CoreAccess<'_>, target: &CanvasTarget, text: &str, x0: f32, top: f32, height: f32, color: Color, fonts: &[Font]) -> RuntimeResult<()> {
    let (w, h) = unsafe { (int_of(a, target.canvas, "SizeX")? as f32, int_of(a, target.canvas, "SizeY")? as f32) };
    if text.trim().is_empty() || height <= 0. {
        return Ok(());
    }
    // Like the stock routine, lines are centred in `0..w - x0`.
    let band = (w - x0).max(1.);
    let margin = (w * 0.04).max(16.);
    let mut chosen = None;
    for &font in fonts {
        let mut error = None;
        let mut measure = |s: &str| match unsafe { target.text_size(a, s, font) } {
            Ok((tw, _)) => tw,
            Err(e) => {
                error = Some(e);
                0.
            }
        };
        let lines = wrap(text, band - 2. * margin, &mut measure);
        if let Some(e) = error {
            return Err(e);
        }
        let line_h = unsafe { target.text_size(a, "Ag", font)? }.1;
        let fits = line_h * lines.len() as f32 <= height - 2.;
        chosen = Some((font, lines, line_h));
        if fits {
            break;
        }
    }
    let Some((font, lines, line_h)) = chosen else { return Ok(()) };
    let block_h = line_h * lines.len() as f32;
    // Subtitle-sized bands are centred vertically; larger areas start at the top, as before.
    let y0 = if height <= 4. * line_h && block_h <= height { top + (height - block_h) / 2. } else { top };
    // Plain data, so the view can be rebuilt (the Ui builds once at creation and again on resize).
    let mut rows: Vec<(String, Color, f32, f32)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let y = y0 + line_h * i as f32;
        rows.push((line.clone(), SUBTITLE_SHADOW, 1.5, y + 1.5));
        rows.push((line.clone(), color, 0., y));
    }
    let width = band;
    let metrics = METRICS.with(Clone::clone);
    let view = move || {
        col().children(rows.iter().map(|(text, color, x, y)| label(text).font(font).color(*color).text_align(TextAlign::Center).at(*x, *y).width(Size::Px(width)).height(Size::Px(line_h))))
    };
    let mut ui = Ui::new(Store::new(), view, metrics.clone());
    ui.resize(w, h);
    for (text, font) in ui.text_requests() {
        if !metrics.knows(&text, font) {
            metrics.learn(&text, font, unsafe { target.text_size(a, &text, font)? });
        }
    }
    ui.relayout();
    let commands = ui.draw();
    unsafe { render_commands(target, a, &commands, &metrics) }
}

/// `HPHud.DrawCutStyleText(Canvas, strText, nXPos, nYPos, nHeight, colorText, optional fontText)`.
unsafe fn draw_cut_style_text(a: CoreAccess<'_>, hud: *mut UObject, function: *mut UFunction, params: *const u8, size: usize, canvas: *mut UObject) -> RuntimeResult<()> {
    let expected = usize::from(unsafe { (*function).parms_size });
    if params.is_null() || size < expected {
        return Err(RuntimeError::new("DrawCutStyleText frame"));
    }
    let at = |name: &str, kind: &str, width: usize| unsafe { field(a, function.cast(), name, kind, width, expected, true) };
    let text = unsafe { params.add(at("strText", "Core.StrProperty", size_of::<hp2_abi::engine::generated::TArrayRaw>())?).cast::<hp2_abi::engine::generated::TArrayRaw>().read_unaligned() };
    let text: String = if text.num > 0 && !text.data.is_null() && text.num < 8192 {
        unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) }.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect()
    } else {
        String::new()
    };
    let x = unsafe { params.add(at("nXPos", "Core.IntProperty", 4)?).cast::<i32>().read_unaligned() } as f32;
    let y = unsafe { params.add(at("nYPos", "Core.IntProperty", 4)?).cast::<i32>().read_unaligned() } as f32;
    let height = unsafe { params.add(at("nHeight", "Core.IntProperty", 4)?).cast::<i32>().read_unaligned() } as f32;
    let color = unsafe { params.add(at("colorText", "Core.StructProperty", 4)?).cast::<[u8; 4]>().read_unaligned() };
    let given = unsafe { params.add(at("fontText", "Core.ObjectProperty", size_of::<*mut UObject>())?).cast::<*mut UObject>().read_unaligned() };
    let owner = unsafe { object(a, hud, "Owner")? };
    let mut target = CanvasTarget { canvas, tile_style: Some(1), fonts: unsafe { player_fonts(a, owner)? } };
    // A caller-chosen font is tried first; the stock fallbacks follow (medium, small, tiny).
    let mut fonts = vec![Font::Slot(1), Font::Slot(2), Font::Slot(3)];
    if live(a, given) {
        target.fonts[0] = given;
        fonts.insert(0, Font::Slot(0));
    }
    unsafe { draw_wrapped(a, &target, &text, x, y, height, Color(color[0], color[1], color[2], 255), &fonts) }
}

/// Typed counter composition; unsupported routes decline before callbacks or painting.
unsafe fn status_callback(a: CoreAccess<'_>, item: *mut UObject, function: *mut UFunction, params: *const u8, size: usize, canvas: *mut UObject, name: &str) -> RuntimeResult<ScriptPreCallDecision> {
    let with_icon = name == "DrawItem";
    let prepared = (|| -> RuntimeResult<Option<(i32, i32, f32, i32)>> {
        let expected = usize::from(unsafe { (*function).parms_size });
        if params.is_null() || size < expected { return Ok(None); }
        let at = |name: &str, kind: &str| unsafe { field(a, function.cast(), name, kind, 4, expected, true) };
        let x = unsafe { params.add(at("nCurrX", "Core.IntProperty")?).cast::<i32>().read_unaligned() };
        let y = unsafe { params.add(at("nCurrY", "Core.IntProperty")?).cast::<i32>().read_unaligned() };
        let scale = unsafe { params.add(at("fScaleFactor", "Core.FloatProperty")?).cast::<f32>().read_unaligned() };
        let supported = if with_icon { unsafe { status_groups::admits_item(a, item, canvas)? } }
            else { unsafe { status_groups::admits_specified_count(a, item, canvas)? } };
        if !supported { return Ok(None); }
        let count = if name == "DrawSpecifiedCount" {
            unsafe { params.add(at("nLocalCount", "Core.IntProperty")?).cast::<i32>().read_unaligned() }
        } else if with_icon { 0 } else { unsafe { int_of(a, item, "nCount")? } };
        Ok(Some((x, y, scale, count)))
    })();
    let (x, y, scale, count) = match prepared {
        Ok(Some(prepared)) => prepared,
        Ok(None) | Err(_) => return Ok(ScriptPreCallDecision::Continue),
    };
    if with_icon { unsafe { status_groups::draw_item(a, item, canvas, x, y, scale)? }; }
    else { unsafe { status_groups::draw_specified_count(a, item, canvas, x, y, scale, count)? }; }
    Ok(ScriptPreCallDecision::Consume)
}

/// Frame-rate independent replacement of the state functions `SlideIn/SlideOut.SetCurrBorderHeight`.
unsafe fn slide(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject, growing: bool) -> RuntimeResult<()> {
    // The script's own maximum (retail scales it with the resolution).
    let mut max = 0_f32;
    unsafe { crate::ui_vm::invoke_virtual(a, manager, "GetMaxBorderHeight", &[("Canvas", crate::ui_vm::V::Obj(canvas))], &[], (&raw mut max).cast())? };
    if !max.is_finite() || max <= 0. {
        max = unsafe { int_of(a, canvas, "SizeY")? } as f32 / 8.;
    }
    let level = unsafe { object(a, manager, "Level")? };
    let now = unsafe { f32_of(a, level, "TimeSeconds")? };
    let index = unsafe { (*manager).index } * if growing { 1 } else { -1 };
    let dt = SLIDE_CLOCK.with(|clock| {
        let mut clock = clock.borrow_mut();
        let previous = clock.insert(index, now);
        clock.remove(&-index);
        previous.map_or(1. / 30., |p| (now - p).clamp(0., 0.25))
    });
    let step = max * dt / SLIDE_SECONDS;
    let current = unsafe { f32_of(a, manager, "fCurrBorderHeight")? };
    let next = if growing { (current + step).min(max) } else { (current - step).max(0.) };
    unsafe { set_f32(a, manager, "fCurrBorderHeight", next) }
}

/// Retail countdown placement. Keep the script's f32 order and int conversion.
fn timer_origin(size_x: i32, size_y: i32, scale: f32) -> (f32, f32) {
    let x = (size_x as f32 - 8.0 * scale - 205.0 * scale) as i32;
    let y = (size_y as f32 - 8.0 * scale - 58.0 * scale) as i32;
    (x as f32, y as f32)
}

fn timer_segment(remaining: f32, duration: f32) -> f32 {
    (remaining / duration) * 118.0
}




unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if receiver.is_null() || !live(a, receiver) {
        return Ok(ScriptPreCallDecision::Continue);
    }
    let name = name_of(a, function.cast());
    let Some(canvas) = (unsafe { param_object(a, function, params, size, "canvas").or_else(|| param_object(a, function, params, size, "Canvas")) }) else {
        return Ok(ScriptPreCallDecision::Continue);
    };
    if !live(a, canvas) {
        return Ok(ScriptPreCallDecision::Continue);
    }
    if matches!(name.as_str(), "DrawItem" | "DrawCount" | "DrawSpecifiedCount") {
        return unsafe { status_callback(a, receiver, function, params, size, canvas, &name) };
    }
    // Read-only admission ends before callbacks, measurements or painting.
    let admitted = (|| -> RuntimeResult<()> {
        unsafe {
            CanvasState::save(a, canvas)?;
            for field_name in ["SizeX", "SizeY"] { int_of(a, canvas, field_name)?; }
            for field_name in ["CurYL", "OrgX", "OrgY", "ClipX", "ClipY"] {
                f32_of(a, canvas, field_name)?;
            }
            match name.as_str() {
                "DrawBorder" => {
                    f32_of(a, receiver, "fCurrBorderHeight")?;
                    let level = object(a, receiver, "Level")?;
                    if !live(a, level) { return Err(RuntimeError::new("HUD level unavailable")); }
                    object(a, level, "PlayerHarryActor")?;
                    let border = object(a, receiver, "textureBorder")?;
                    if !live(a, border) { find_texture(a, "UWindow.WhiteTexture")?; }
                }
                "DrawCutStyleText" => {
                    for (field_name, kind, width) in [
                        ("strText", "Core.StrProperty", size_of::<hp2_abi::engine::generated::TArrayRaw>()),
                        ("nXPos", "Core.IntProperty", 4),
                        ("nYPos", "Core.IntProperty", 4),
                        ("nHeight", "Core.IntProperty", 4),
                        ("colorText", "Core.StructProperty", 4),
                        ("fontText", "Core.ObjectProperty", size_of::<*mut UObject>()),
                    ] { field(a, function.cast(), field_name, kind, width, size, true)?; }
                    let fonts = player_fonts(a, object(a, receiver, "Owner")?)?;
                    if fonts[1..].iter().any(|&font| !live(a, font)) {
                        return Err(RuntimeError::new("HUD fonts unavailable"));
                    }
                }
                "SetCurrBorderHeight" => {
                    f32_of(a, receiver, "fCurrBorderHeight")?;
                    let level = object(a, receiver, "Level")?;
                    if !live(a, level) { return Err(RuntimeError::new("HUD level unavailable")); }
                    f32_of(a, level, "TimeSeconds")?;
                    if a.find_object_function_by_name(receiver, a.intern_str("GetMaxBorderHeight", FindName::Find)?, false)?.is_null() {
                        return Err(RuntimeError::new("HUD border callback unavailable"));
                    }
                }
                _ => return Err(RuntimeError::new("Unsupported HUD drawing route")),
            }
        }
        Ok(())
    })();
    if let Err(error) = admitted {
        warn(a, &error);
        return Ok(ScriptPreCallDecision::Continue);
    }
    let result = (|| -> RuntimeResult<()> {
        match name.as_str() {
            "DrawBorder" | "DrawCutStyleText" => {
                let saved = unsafe { CanvasState::save(a, canvas)? };
                let drawn = if name == "DrawBorder" { unsafe { draw_border(a, receiver, canvas) } } else { unsafe { draw_cut_style_text(a, receiver, function, params, size, canvas) } };
                unsafe { saved.restore(a)? };
                if name == "DrawBorder" {
                    // The stock border leaves the canvas in normal style (it drew tiles); retail's
                    // subtitle text that follows relies on that.
                    unsafe { prop(a, canvas, "Style", "Core.ByteProperty", 1)?.write(1) };
                }
                drawn
            }
            "SetCurrBorderHeight" => {
                let state = name_of(a, unsafe { object(a, function.cast(), "Outer") }.unwrap_or(ptr::null_mut()));
                unsafe { slide(a, receiver, canvas, state == "SlideIn") }
            }
            _ => Ok(()),
        }
    })();
    result?;
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn find_function(a: CoreAccess<'_>, owner_path: &str, owner_class: &str, name: &str) -> RuntimeResult<*mut UFunction> {
    let Some(owner) = (unsafe { a.find_object_by_name(a.native_class(owner_class)?, &owner_path.chars().map(u32::from).collect::<Vec<_>>())? }) else {
        return Ok(ptr::null_mut());
    };
    Ok(unsafe { a.find_field(owner.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>())
}

/// Take over the cutscene letterbox and subtitles for one draw.
pub(crate) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !enabled() {
        return Ok(None);
    }
    let mut functions = Vec::new();
    for (owner, class, names) in [
        ("HGame.CutSceneManager", "Core.Class", &["DrawBorder"][..]),
        ("HGame.HPHud", "Core.Class", &["DrawCutStyleText"][..]),
        ("HGame.StatusItem", "Core.Class", &["DrawItem", "DrawCount", "DrawSpecifiedCount"][..]),
        ("HGame.CutSceneManager.SlideIn", "Core.State", &["SetCurrBorderHeight"][..]),
        ("HGame.CutSceneManager.SlideOut", "Core.State", &["SetCurrBorderHeight"][..]),
    ] {
        for name in names {
            let f = unsafe { find_function(a, owner, class, name)? };
            // Only functions this state or class declares itself (not an inherited empty stub).
            if !f.is_null() && unsafe { object(a, f.cast(), "Outer") }.ok().is_some_and(|o| name_of(a, o) == owner.rsplit('.').next().unwrap_or_default()) && !functions.contains(&f) {
                functions.push(f);
            }
        }
    }
    let pre = if functions.is_empty() { None } else {
        Some(unsafe { a.install_script_pre_observer("ui-hud", &functions, callback)? })
    };
    let selector = unsafe { spell_selector::install(a)? };
    let magic = unsafe { magic_strength::install(a)? };
    let timer = unsafe { countdown::install(a)? };
    let player_health = unsafe { player_health::install(a)? };
    let enemy_health = unsafe { enemy_health::install(a)? };
    let status = unsafe { status_groups::install(a)? };
    let vendor = unsafe { vendor::install(a)? };
    let pickups = unsafe { pickups::install(a)? };
    let popups = unsafe { popups::install(a)? };
    let quidditch_bar = unsafe { quidditch_bar::install(a)? };
    let quidditch_score = unsafe { quidditch_score::install(a)? };
    let challenge = unsafe { challenge_score::install(a)? };
    let lesson = unsafe { spell_lesson::install(a)? };
    let cutscene = unsafe { cutscene::install(a)? };
    let muggle = unsafe { muggle_meter::install(a)? };
    let extra = unsafe { extra_hud::install(a)? };
    let ceremony = unsafe { ceremony::install(a)? };
    let weapon = unsafe { weapon_overlay::install(a)? };
    if pre.is_none() && selector.is_none() && magic.is_none() && timer.is_none()
        && player_health.is_none() && enemy_health.is_none() && status.is_none()
        && vendor.is_none() && pickups.is_none() && popups.is_none()
        && quidditch_bar.is_none() && quidditch_score.is_none() && challenge.is_none()
        && lesson.is_none() && cutscene.is_none() && muggle.is_none() && extra.is_none()
        && ceremony.is_none() && weapon.is_none() { return Ok(None); }
    Ok(Some(HudGuards { _pre: pre, _components: (
        (selector, magic, timer), (player_health, enemy_health),
        (status, vendor, pickups, popups), (quidditch_bar, quidditch_score, challenge, lesson),
        (cutscene, muggle, extra, ceremony, weapon),
    ) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_meter_retains_signed_geometry_and_tint() {
        magic_strength::assert_view();
    }

    #[test]
    fn countdown_rebuilds_bar_and_elapsed_views_without_clamping() {
        countdown::assert_view();
    }

    #[test]
    fn spell_selector_whole_view_preserves_retail_geometry() {
        spell_selector::assert_view();
    }


    #[test]
    fn player_health_preserves_bolt_count_boundaries_and_signed_regions() {
        player_health::assert_view();
    }

    #[test]
    fn enemy_health_preserves_source_clamp_and_frame_fill_geometry() {
        enemy_health::assert_view();
    }

    #[test]
    fn status_groups_preserve_layout_visibility_and_count_composition() {
        status_groups::assert_view();
    }

    #[test]
    fn vendor_preserves_whole_bar_geometry() {
        vendor::assert_view();
    }

    #[test]
    fn popups_preserve_pages_and_warning_branches() {
        popups::assert_view();
    }

    #[test]
    fn quidditch_bar_preserves_signed_progress_and_flash_geometry() {
        quidditch_bar::assert_view();
    }

    #[test]
    fn quidditch_score_preserves_opponent_and_tally_views() {
        quidditch_score::assert_view();
    }

    #[test]
    fn challenge_score_preserves_points_and_star_views() {
        challenge_score::assert_view();
    }

    #[test]
    fn spell_lesson_preserves_integer_half_centering() {
        spell_lesson::assert_view();
    }

    #[test]
    fn cutscene_preserves_comment_and_fast_forward_view() {
        cutscene::assert_view();
    }

    #[test]
    fn accessibility_options_and_palette_contracts() {
        crate::ui_accessibility::assert_contracts();
    }

    #[test]
    fn console_preserves_source_text_and_image_geometry() {
        crate::ui_console_views::assert_view();
    }

    #[test]
    fn ceremony_preserves_integer_centered_scripted_texture_score() {
        ceremony::assert_view();
    }

    #[test]
    fn weapon_overlay_geometry_clock_edges() {
        weapon_overlay::assert_edges();
    }

    #[test]
    fn empty_caption_preserves_actual_canvas_position_and_line_height() {
        use hp2_abi::engine::generated::UCanvas;
        let root = std::env::var_os("HP2_ARTIFACT_DIR").map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("hp2-ui-{}",std::process::id())))
            .join("empty-caption");
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/Default.ini"),"").unwrap();
        std::fs::write(root.join("System/DefUser.ini"),"").unwrap();
        let owner = crate::EngineOwner::new(hp2_core::core_access::CoreOptions {
            data_root:root.clone(),system_ini:root.join("System/Default.ini"),user_ini:root.join("System/DefUser.ini"),
            command_line:Vec::new(),output:|_,_,_|Ok(()),log_file:Some(std::fs::File::create(root.join("runtime.log")).unwrap()),
        }).unwrap();
        let a = owner.access();
        unsafe {
            let canvas = a.construct_object(a.native_class("Engine.Canvas").unwrap(),a.transient_package(),0,0,ptr::null_mut()).unwrap().cast::<UCanvas>();
            let font = a.construct_object(a.native_class("Engine.Font").unwrap(),a.transient_package(),0,0,ptr::null_mut()).unwrap();
            (*canvas).cur_x = 100.; (*canvas).cur_y = 101.; (*canvas).cur_yl = 7.;
            let target = CanvasTarget { canvas:canvas.cast(),tile_style:None,fonts:[font;4] };
            let metrics = HostMetrics::default(); metrics.learn("",Font::Slot(0),(0.,0.));
            let mut ui = Ui::new(Store::new(),|| col().children([label("").font(Font::Slot(0)).at(20.,30.).width(Size::Px(0.)).height(Size::Px(0.))]),metrics.clone());
            ui.resize(100.,100.); ui.relayout();
            render_commands(&target,a,&ui.draw(),&metrics).unwrap();
            assert_eq!(((*canvas).cur_x,(*canvas).cur_y,(*canvas).cur_yl),(20.,30.,7.));
            assert_eq!(ptr::addr_of!((*canvas).font).read_unaligned(),font.cast());
        }
    }

    #[test]
    fn muggle_meter_preserves_signed_detection_geometry() {
        muggle_meter::assert_view();
    }

    #[test]
    fn extra_hud_preserves_hoops_and_distinct_spell_icon() {
        extra_hud::assert_view();
    }


    #[test]
    fn retail_timer_origin_truncates_after_scaled_subtractions() {
        assert_eq!(timer_origin(1280, 720, 2.0), (854.0, 588.0));
        assert_eq!(timer_origin(640, 480, 0.75), (480.0, 430.0));
    }


    #[test]
    fn timer_segment_does_not_clamp_or_hide_zero_division() {
        assert_eq!(timer_segment(30.0, 60.0), 59.0);
        assert_eq!(timer_segment(90.0, 60.0), 177.0);
        assert!(timer_segment(1.0, 0.0).is_infinite());
        assert!(timer_segment(0.0, 0.0).is_nan());
    }

    #[test]
    fn wrap_breaks_on_words_and_keeps_long_words_whole() {
        let mut width = |s: &str| s.len() as f32;
        assert_eq!(wrap("the quick brown fox", 10., &mut width), vec!["the quick", "brown fox"]);
        assert_eq!(wrap("antidisestablishment is long", 10., &mut width), vec!["antidisestablishment", "is long"]);
        assert_eq!(wrap("a\nb", 10., &mut width), vec!["a", "b"]);
        assert!(wrap("   ", 10., &mut width).is_empty());
    }
}
