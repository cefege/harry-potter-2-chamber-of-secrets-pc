//! The complete CutSceneManager.DrawText view. Hold/SlideIn/SlideOut remain
//! script dispatchers; existing time-based border policy is not replaced here.
use std::{cell::RefCell, mem::size_of, ptr};

use hp2_abi::engine::generated::{TArrayRaw, UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use hp2_ui::{Color, Font, Node, Rect, Signal, Size, Store, TextAlign, Ui, col, label};

use super::{CanvasTarget, SUBTITLE_SHADOW, param_object, player_fonts, wrap};
use crate::{
    stock_graphics_menu::{Weak, is_a, live, object, prop, read_boolean},
    ui_console_views::{self as console, array_slot, declared, float, int, need, parameter, read_string, resolved, set_object},
    ui_host::{HostMetrics, find_class, render_commands},
    ui_vm::{self, V},
};

#[derive(Clone, PartialEq)]
struct Subtitle {
    width: f32,
    line_height: f32,
    font: Font,
    rows: Vec<(String, Color, f32, f32)>,
}
fn subtitle_view(model: &Subtitle) -> Node {
    col().children(model.rows.iter().map(|(text, color, x, y)| {
        label(text).font(model.font).color(*color).text_align(TextAlign::Center)
            .at(*x, *y).width(Size::Px(model.width)).height(Size::Px(model.line_height))
    }))
}
struct Retained {
    manager: Weak,
    model: Signal<Subtitle>,
    ui: Ui,
    metrics: HostMetrics,
}
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }

/// Preserve the existing deliberate wrapping, fallback fonts and shadow policy,
/// rather than silently reverting subtitles to the retail unwrapped formula.
unsafe fn subtitle(a: CoreAccess<'_>, manager: *mut UObject, hud: *mut UObject, c: *mut UObject, text: &str, top: f32, height: f32, tint: [u8; 4]) -> RuntimeResult<()> {
    if text.trim().is_empty() || height <= 0. { return Ok(()); }
    unsafe {
        let saved_font = object(a, c, "Font")?;
        let saved_color = console::color(a, c)?;
        let saved_style = prop(a, c, "Style", "Core.ByteProperty", 1)?.read();
        let result = (|| -> RuntimeResult<()> {
            let fonts = player_fonts(a, object(a, hud, "Owner")?)?;
            let target = CanvasTarget { canvas: c, tile_style: None, fonts };
            let width = int(a, c, "SizeX")? as f32;
            let margin = (width * 0.04).max(16.);
            let mut chosen = None;
            for font in [Font::Slot(1), Font::Slot(2), Font::Slot(3)] {
                let mut error = None;
                let mut measure = |value: &str| match target.text_size(a, value, font) {
                    Ok(size) => size.0,
                    Err(e) => { error = Some(e); 0. }
                };
                let lines = wrap(text, width.max(1.) - 2. * margin, &mut measure);
                if let Some(error) = error { return Err(error); }
                let line_height = target.text_size(a, "Ag", font)?.1;
                let fits = line_height * lines.len() as f32 <= height - 2.;
                chosen = Some((font, lines, line_height));
                if fits { break; }
            }
            let Some((font, lines, line_height)) = chosen else { return Ok(()); };
            let block_height = line_height * lines.len() as f32;
            let y0 = if height <= 4. * line_height && block_height <= height { top + (height - block_height) / 2. } else { top };
            let mut rows = Vec::with_capacity(lines.len() * 2);
            for (index, line) in lines.into_iter().enumerate() {
                let y = y0 + line_height * index as f32;
                rows.push((line.clone(), SUBTITLE_SHADOW, 1.5, y + 1.5));
                rows.push((line, Color(tint[0], tint[1], tint[2], 255), 0., y));
            }
            let model = Subtitle { width: width.max(1.), line_height, font, rows };
            let retained = VIEWS.with(|views| {
                let mut views = views.borrow_mut(); views.retain(|v| v.manager.live(a));
                views.iter().position(|v| v.manager.p == manager).map(|i| views.swap_remove(i))
            });
            let metrics = retained.as_ref().map_or_else(HostMetrics::default, |v| v.metrics.clone());
            metrics.clear();
            // Always remeasure: fonts/providers and viewport can change without
            // changing the text. No callback or measurement is cached away.
            for (text, _, _, _) in &model.rows {
                if !metrics.knows(text, font) { metrics.learn(text, font, target.text_size(a, text, font)?); }
            }
            let mut retained = retained.unwrap_or_else(|| {
                let store = Store::new(); let current = store.signal(model.clone()); let observed = current.clone();
                Retained { manager: Weak::new(manager), model: current, ui: Ui::new(store, move || subtitle_view(&observed.get()), metrics.clone()), metrics: metrics.clone() }
            });
            retained.model.set(model);
            retained.ui.resize(width, int(a, c, "SizeY")? as f32);
            retained.ui.relayout();
            let result = render_commands(&target, a, &retained.ui.draw(), &metrics);
            if retained.manager.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
            result
        })();
        // Retail saves these three fields, not CurX/CurY/CurYL. The toolkit's
        // bitmap-native cursor and line-height effects remain observable.
        let restored = (|| -> RuntimeResult<()> {
            set_object(a, c, "Font", saved_font)?;
            prop(a, c, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(saved_color);
            prop(a, c, "Style", "Core.ByteProperty", 1)?.write(saved_style);
            Ok(())
        })();
        result?; restored
    }
}
unsafe fn harry_hud(a: CoreAccess<'_>, manager: *mut UObject) -> RuntimeResult<*mut UObject> {
    unsafe {
        need(a, manager)?;
        let level = object(a, manager, "Level")?; need(a, level)?;
        let harry = object(a, level, "PlayerHarryActor")?; need(a, harry)?;
        let hud = object(a, harry, "myHUD")?; need(a, hud)?;
        let Some(class) = find_class(a, "HGame.HPHud")? else { return Err(RuntimeError::new("Cutscene HUD class unavailable")); };
        if !is_a(a, hud, class) { return Err(RuntimeError::new("Cutscene HUD cast unavailable")); }
        Ok(hud)
    }
}
unsafe fn max_height(a: CoreAccess<'_>, manager: *mut UObject, c: *mut UObject) -> RuntimeResult<i32> {
    unsafe {
        let mut height = 0f32;
        ui_vm::invoke_virtual(a, manager, "GetMaxBorderHeight", &[("Canvas", V::Obj(c))], &[], (&raw mut height).cast())?;
        need(a, manager)?; need(a, c)?;
        Ok(height as i32)
    }
}
unsafe fn subtitle_layout(a: CoreAccess<'_>, f: *mut UFunction) -> RuntimeResult<()> {
    unsafe {
        parameter(a, f, "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())?;
        parameter(a, f, "strText", "Core.StrProperty", size_of::<TArrayRaw>())?;
        for n in ["nXPos", "nYPos", "nHeight"] { parameter(a, f, n, "Core.IntProperty", 4)?; }
        parameter(a, f, "colorText", "Core.StructProperty", 4)?;
        Ok(())
    }
}
/// This is one typed existing callback's parameter frame, not a callback
/// language or an instruction driver. Custom DrawCutStyleText executes once.
unsafe fn custom_subtitle(a: CoreAccess<'_>, hud: *mut UObject, c: *mut UObject, f: *mut UFunction, text: &str, y: i32, height: i32, tint: [u8; 4]) -> RuntimeResult<()> {
    unsafe {
        subtitle_layout(a, f)?;
        let mut frame = vec![0u64; usize::from((*f).parms_size).div_ceil(8).max(1)];
        let base = frame.as_mut_ptr().cast::<u8>();
        let string_offset = parameter(a, f, "strText", "Core.StrProperty", size_of::<TArrayRaw>())?;
        let mut value: TArrayRaw = std::mem::zeroed();
        a.assign_fstring(&mut value, &text.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?;
        base.add(string_offset).cast::<TArrayRaw>().write_unaligned(value);
        base.add(parameter(a, f, "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())?).cast::<*mut UObject>().write_unaligned(c);
        for (n, v) in [("nXPos", 0), ("nYPos", y), ("nHeight", height)] { base.add(parameter(a, f, n, "Core.IntProperty", 4)?).cast::<i32>().write_unaligned(v); }
        base.add(parameter(a, f, "colorText", "Core.StructProperty", 4)?).cast::<[u8; 4]>().write_unaligned(tint);
        let result = a.execute_event(hud, f, base.cast(), ptr::null_mut());
        let mut final_value = base.add(string_offset).cast::<TArrayRaw>().read_unaligned();
        let cleaned = a.destroy_owned_fstring(&mut final_value);
        result?; cleaned?; need(a, c)
    }
}
unsafe fn draw_subtitle(a: CoreAccess<'_>, manager: *mut UObject, hud: *mut UObject, c: *mut UObject, f: *mut UFunction, text: &str, y: i32, height: i32, tint: [u8; 4], canonical: *mut UFunction) -> RuntimeResult<()> {
    unsafe {
        need(a, hud)?;
        if !canonical.is_null() && f == canonical { subtitle(a, manager, hud, c, text, y as f32, height as f32, tint) }
        else { custom_subtitle(a, hud, c, f, text, y, height, tint) }
    }
}
fn icon_index(counter: i32, drawn: bool) -> i32 {
    let value = if drawn { counter / 10 } else { counter };
    (value as f32 % 3.) as i32
}
unsafe fn icon(a: CoreAccess<'_>, manager: *mut UObject, index: i32) -> RuntimeResult<*mut UObject> {
    unsafe { Ok(array_slot(a, manager, "textureFFIcons", "Core.ObjectProperty", size_of::<*mut UObject>(), 3, index)?.cast::<*mut UObject>().read_unaligned()) }
}
unsafe fn load_icons(a: CoreAccess<'_>, manager: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        if !icon(a, manager, 0)?.is_null() { return Ok(()); }
        let f = declared(a, "Core.Object.DynamicLoadObject")?;
        let Some(texture_class) = find_class(a, "Engine.Texture")? else { return Err(RuntimeError::new("Fast-forward texture class unavailable")); };
        for (i, path) in ["HP2_Menu.Icons.ff1", "HP2_Menu.Icons.ff2", "HP2_Menu.Icons.ff3"].into_iter().enumerate() {
            let mut texture: *mut UObject = ptr::null_mut();
            ui_vm::invoke_resolved(a, manager, f, &[("ObjectName", V::S(path)), ("ObjectClass", V::Obj(texture_class.cast()))], &[], (&raw mut texture).cast())?;
            need(a, manager)?;
            // Preserve the Texture cast, including a failed/null load.
            if !texture.is_null() && !is_a(a, texture, texture_class) { texture = ptr::null_mut(); }
            array_slot(a, manager, "textureFFIcons", "Core.ObjectProperty", size_of::<*mut UObject>(), 3, i as i32)?.cast::<*mut UObject>().write_unaligned(texture);
        }
        Ok(())
    }
}
unsafe fn draw(a: CoreAccess<'_>, manager: *mut UObject, c: *mut UObject, canonical: *mut UFunction) -> RuntimeResult<()> {
    unsafe {
        // Argument snapshots are deliberately separated by callbacks. In
        // particular, comment/FF state is not hoisted before the first subtitle.
        let hud = harry_hud(a, manager)?;
        // Virtual dispatch is selected before the original argument getters.
        let painter = resolved(a, hud, "DrawCutStyleText")?;
        let first = read_string(a, manager, "strText")?;
        let y = ((int(a, c, "SizeY")? as f32 - float(a, manager, "fCurrBorderHeight")?) + 1.) as i32;
        let height = max_height(a, manager, c)?;
        let tint = prop(a, manager, "colorCutTextBlue", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned();
        draw_subtitle(a, manager, hud, c, painter, &first, y, height, tint, canonical)?;
        need(a, manager)?; need(a, c)?;
        if !read_string(a, manager, "strCutCommentText")?.is_empty() {
            let hud = harry_hud(a, manager)?;
            let painter = resolved(a, hud, "DrawCutStyleText")?;
            let comment = read_string(a, manager, "strCutCommentText")?;
            let height = max_height(a, manager, c)?;
            let tint = prop(a, manager, "colorCutTextBlue", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned();
            draw_subtitle(a, manager, hud, c, painter, &comment, 5, height, tint, canonical)?;
            need(a, manager)?; need(a, c)?;
        }
        if read_boolean(a, manager, "bShowFF")? {
            load_icons(a, manager)?;
            let counter = int(a, manager, "curFFIcon")?;
            let centering = icon(a, manager, icon_index(counter, false))?;
            need(a, centering)?;
            let (w, h) = (int(a, centering, "USize")?, int(a, centering, "VSize")?);
            let x = (int(a, c, "SizeX")? / 2) as f32 - (w / 2) as f32;
            let y = (int(a, c, "SizeY")? / 2) as f32 - (h / 2) as f32;
            console::set_pos(a, c, x, y)?;
            let texture = icon(a, manager, icon_index(int(a, manager, "curFFIcon")?, true))?;
            // DrawIcon(None) returns before the tile native; it still follows
            // SetPos, and the icon counter still advances.
            if !texture.is_null() {
                need(a, texture)?;
                let w = int(a, texture, "USize")? as f32;
                let h = int(a, texture, "VSize")? as f32;
                console::tile(a, manager, c, texture, Rect::new(x, y, w, h), Rect::new(0., 0., w, h), false)?;
            }
            prop(a, manager, "curFFIcon", "Core.IntProperty", 4)?.cast::<i32>().write_unaligned(int(a, manager, "curFFIcon")?.wrapping_add(1));
        }
        Ok(())
    }
}
unsafe fn preflight(a: CoreAccess<'_>, manager: *mut UObject, c: *mut UObject) -> RuntimeResult<*mut UFunction> {
    unsafe {
        need(a, manager)?; need(a, c)?;
        for n in ["CurX", "CurY", "CurYL", "OrgX", "OrgY", "ClipX", "ClipY"] { float(a, c, n)?; }
        int(a, c, "SizeX")?; int(a, c, "SizeY")?; object(a, c, "Font")?; console::color(a, c)?;
        prop(a, c, "Style", "Core.ByteProperty", 1)?;
        for n in ["strText", "strCutCommentText"] { read_string(a, manager, n)?; }
        float(a, manager, "fCurrBorderHeight")?;
        prop(a, manager, "colorCutTextBlue", "Core.StructProperty", 4)?;
        read_boolean(a, manager, "bShowFF")?; int(a, manager, "curFFIcon")?;
        for i in 0..3 {
            let texture = icon(a, manager, i)?;
            if !texture.is_null() { need(a, texture)?; int(a, texture, "USize")?; int(a, texture, "VSize")?; }
        }
        let max = resolved(a, manager, "GetMaxBorderHeight")?;
        parameter(a, max, "Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())?;
        parameter(a, max, "ReturnValue", "Core.FloatProperty", 4)?;
        let hud = harry_hud(a, manager)?;
        let canonical = declared(a, "HGame.HPHud.DrawCutStyleText")?;
        let child = resolved(a, hud, "DrawCutStyleText")?; subtitle_layout(a, child)?;
        if child == canonical && !canonical.is_null() {
            let fonts = player_fonts(a, object(a, hud, "Owner")?)?;
            if fonts[1..].iter().any(|&font| !live(a, font)) { return Err(RuntimeError::new("Cutscene fonts unavailable")); }
        }
        let load = declared(a, "Core.Object.DynamicLoadObject")?;
        parameter(a, load, "ObjectName", "Core.StrProperty", size_of::<TArrayRaw>())?;
        parameter(a, load, "ObjectClass", "Core.ObjectProperty", size_of::<*mut UObject>())?;
        if find_class(a, "Engine.Texture")?.is_none() { return Err(RuntimeError::new("Cutscene texture class unavailable")); }
        if read_boolean(a, manager, "bShowFF")? {
            // Invalid source indices remain a whole-stock route before effects.
            let n = int(a, manager, "curFFIcon")?;
            let centering = icon(a, manager, icon_index(n, false))?;
            icon(a, manager, icon_index(n, true))?;
            if !icon(a, manager, 0)?.is_null() && centering.is_null() {
                return Err(RuntimeError::new("Fast-forward centering texture is null"));
            }
        }
        Ok(canonical)
    }
}
unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, _function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if std::env::var_os("HP2_STOCK_HUD").is_some() || !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    let Some(c) = (unsafe { param_object(a, _function, params, size, "Canvas") }) else { return Ok(ScriptPreCallDecision::Continue); };
    let Ok(canonical) = (unsafe { preflight(a, receiver, c) }) else { return Ok(ScriptPreCallDecision::Continue); };
    unsafe { draw(a, receiver, c, canonical)?; }
    Ok(ScriptPreCallDecision::Consume)
}
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if std::env::var_os("HP2_STOCK_HUD").is_some() { return Ok(None); }
    let f = unsafe { declared(a, "HGame.CutSceneManager.DrawText")? };
    if f.is_null() { return Ok(None); }
    Ok(Some(unsafe { a.install_script_pre_observer("ui-cutscene-text", &[f], callback)? }))
}

#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!((icon_index(11, false), icon_index(11, true)), (2, 1));
    assert_eq!((icon_index(29, false), icon_index(29, true)), (2, 2));
    assert_eq!((icon_index(30, false), icon_index(30, true)), (0, 0));
    // Odd texture dimensions use integer halves before float conversion.
    assert_eq!((801 / 2) as f32 - (65 / 2) as f32, 368.);
    let model = Subtitle { width: 800., line_height: 18., font: Font::Slot(1), rows: vec![("caption".to_owned(), SUBTITLE_SHADOW, 1.5, 41.5), ("caption".to_owned(), Color(1, 2, 3, 255), 0., 40.)] };
    let metrics = HostMetrics::default(); metrics.learn("caption", Font::Slot(1), (70., 18.));
    let mut ui = Ui::new(Store::new(), move || subtitle_view(&model), metrics);
    ui.resize(800., 600.);
    let commands = ui.draw();
    assert_eq!(commands.iter().filter(|c| matches!(c, hp2_ui::DrawCmd::Text { .. })).count(), 2);
}
