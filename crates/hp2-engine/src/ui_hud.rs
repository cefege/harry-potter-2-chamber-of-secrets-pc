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
use hp2_ui::{Color, Font, Rect, Size, Store, TextAlign, Ui, col, label};

use crate::{
    stock_graphics_menu::{field, live, object, prop, read_boolean},
    ui_host::{find_texture, name_of, render_commands, DrawTarget, HostMetrics},
};

/// Stock border slide: 25 steps at the 30 fps the game was tuned for.
const SLIDE_SECONDS: f32 = 25. / 30.;
const SUBTITLE_SHADOW: Color = Color(0, 0, 0, 255);

fn enabled() -> bool {
    std::env::var_os("HP2_STOCK_HUD").is_none()
}

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
    unsafe fn segment(&self, a: CoreAccess<'_>, tex: *mut UObject, dest: Rect, src: Rect) -> RuntimeResult<()> {
        unsafe {
            self.set_color(a, [255, 255, 255, 255])?;
            self.tile(a, tex, dest, src)
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
            unsafe { target.segment(a, border, rect, Rect::new(0., 0., 1., 1.))? };
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

/// `StatusItem.DrawItem(canvas, nCurrX, nCurrY, fScaleFactor)`: the icon, then the count.
unsafe fn draw_status_item(a: CoreAccess<'_>, item: *mut UObject, function: *mut UFunction, params: *const u8, size: usize, canvas: *mut UObject, with_icon: bool) -> RuntimeResult<()> {
    let expected = usize::from(unsafe { (*function).parms_size });
    if params.is_null() || size < expected {
        return Err(RuntimeError::new("StatusItem frame"));
    }
    let at = |name: &str, kind: &str| unsafe { field(a, function.cast(), name, kind, 4, expected, true) };
    let x = unsafe { params.add(at("nCurrX", "Core.IntProperty")?).cast::<i32>().read_unaligned() } as f32;
    let y = unsafe { params.add(at("nCurrY", "Core.IntProperty")?).cast::<i32>().read_unaligned() } as f32;
    let scale = unsafe { params.add(at("fScaleFactor", "Core.FloatProperty")?).cast::<f32>().read_unaligned() };
    let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
    if with_icon {
        let icon = unsafe { object(a, item, "textureHudIcon")? };
        if live(a, icon) {
            let (w, h) = unsafe { (int_of(a, icon, "USize")? as f32, int_of(a, icon, "VSize")? as f32) };
            unsafe { target.tile(a, icon, Rect::new(x, y, w * scale, h * scale), Rect::new(0., 0., w, h))? };
        }
        if !unsafe { read_boolean(a, item, "bDisplayCount")? } {
            return Ok(());
        }
    }
    // The count, centred on the icon's count anchor, with a shadow for legibility.
    let mut text = unsafe { int_of(a, item, "nCount")? }.to_string();
    if unsafe { read_boolean(a, item, "bDisplayMaxCount")? } {
        text = format!("{text}/{}", unsafe { int_of(a, item, "nMaxCount")? });
    }
    let mut color = [0_u8; 4];
    unsafe { crate::ui_vm::invoke(a, item, "GetCountColor", &[], &[], (&raw mut color).cast())? };
    let mut font: *mut UObject = ptr::null_mut();
    unsafe { crate::ui_vm::invoke(a, item, "GetCountFont", &[("canvas", crate::ui_vm::V::Obj(canvas))], &[], (&raw mut font).cast())? };
    if !live(a, font) {
        return Err(RuntimeError::new("StatusItem count font"));
    }
    let target = CanvasTarget { canvas, tile_style: None, fonts: [font; 4] };
    let (tw, th) = unsafe { target.text_size(a, &text, Font::Slot(0))? };
    let cx = x + unsafe { int_of(a, item, "nCountMiddleX")? } as f32 * scale - tw / 2.;
    let cy = y + unsafe { int_of(a, item, "nCountMiddleY")? } as f32 * scale - th / 2.;
    // A light count gets a dark shadow and a dark count a light one.
    let luminance = u32::from(color[0]) * 3 + u32::from(color[1]) * 6 + u32::from(color[2]);
    let shadow = if luminance > 1280 { [0, 0, 0, 255] } else { [255, 255, 255, 160] };
    unsafe {
        target.text_at(a, cx + 1., cy + 1., &text, Font::Slot(0), shadow)?;
        target.text_at(a, cx, cy, &text, Font::Slot(0), [color[0], color[1], color[2], 255])?;
    }
    Ok(())
}

/// Frame-rate independent replacement of the state functions `SlideIn/SlideOut.SetCurrBorderHeight`.
unsafe fn slide(a: CoreAccess<'_>, manager: *mut UObject, canvas: *mut UObject, growing: bool) -> RuntimeResult<()> {
    // The script's own maximum (retail scales it with the resolution).
    let mut max = 0_f32;
    unsafe { crate::ui_vm::invoke(a, manager, "GetMaxBorderHeight", &[("Canvas", crate::ui_vm::V::Obj(canvas))], &[], (&raw mut max).cast())? };
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
            "DrawItem" | "DrawCount" => {
                let saved = unsafe { CanvasState::save(a, canvas)? };
                let drawn = unsafe { draw_status_item(a, receiver, function, params, size, canvas, name == "DrawItem") };
                unsafe { saved.restore(a)? };
                drawn
            }
            "SetCurrBorderHeight" => {
                let state = name_of(a, unsafe { object(a, function.cast(), "Outer") }.unwrap_or(ptr::null_mut()));
                unsafe { slide(a, receiver, canvas, state == "SlideIn") }
            }
            _ => Ok(()),
        }
    })();
    match result {
        Ok(()) => Ok(ScriptPreCallDecision::Consume),
        Err(error) => {
            // Fall back to the stock drawing for this call.
            warn(a, &error);
            Ok(ScriptPreCallDecision::Continue)
        }
    }
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
        ("HGame.StatusItem", "Core.Class", &["DrawItem", "DrawCount"][..]),
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
    if functions.is_empty() {
        return Ok(None);
    }
    Ok(Some(unsafe { a.install_script_pre_observer("ui-hud", &functions, callback)? }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrap_breaks_on_words_and_keeps_long_words_whole() {
        let mut width = |s: &str| s.len() as f32;
        assert_eq!(wrap("the quick brown fox", 10., &mut width), vec!["the quick", "brown fox"]);
        assert_eq!(wrap("antidisestablishment is long", 10., &mut width), vec!["antidisestablishment", "is long"]);
        assert_eq!(wrap("a\nb", 10., &mut width), vec!["a", "b"]);
        assert!(wrap("   ", 10., &mut width).is_empty());
    }
}
