//! Finite, source-level console presentation routes. Input, history, message expiry,
//! timedemo calculation and UWindow lifecycle remain script-owned.
use std::{cell::RefCell, mem::size_of, ptr};

use hp2_abi::engine::generated::{TArrayRaw, UClass, UFunction, UObject, UProperty, UStruct, UStructProperty};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};

use crate::{
    stock_graphics_menu::{Weak, boolean, field, is_a, live, object, prop, read_boolean},
    ui_host::{DrawTarget, HostMetrics, find_class, find_texture, render_commands},
    ui_vm::{self, V},
};

pub(crate) unsafe fn need(a: CoreAccess<'_>, o: *mut UObject) -> RuntimeResult<()> {
    if live(a, o) { Ok(()) } else { Err(RuntimeError::new("Console presentation object is gone")) }
}
pub(crate) unsafe fn float(a: CoreAccess<'_>, o: *mut UObject, n: &str) -> RuntimeResult<f32> {
    unsafe { need(a, o)?; Ok(prop(a, o, n, "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned()) }
}
pub(crate) unsafe fn int(a: CoreAccess<'_>, o: *mut UObject, n: &str) -> RuntimeResult<i32> {
    unsafe { need(a, o)?; Ok(prop(a, o, n, "Core.IntProperty", 4)?.cast::<i32>().read_unaligned()) }
}
pub(crate) unsafe fn set_float(a: CoreAccess<'_>, o: *mut UObject, n: &str, v: f32) -> RuntimeResult<()> {
    unsafe { need(a, o)?; prop(a, o, n, "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(v); } Ok(())
}
pub(crate) unsafe fn set_object(a: CoreAccess<'_>, o: *mut UObject, n: &str, v: *mut UObject) -> RuntimeResult<()> {
    unsafe { need(a, o)?; prop(a, o, n, "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(v); } Ok(())
}
pub(crate) unsafe fn set_pos(a: CoreAccess<'_>, c: *mut UObject, x: f32, y: f32) -> RuntimeResult<()> {
    unsafe { set_float(a, c, "CurX", x)?; set_float(a, c, "CurY", y) }
}
unsafe fn origin(a: CoreAccess<'_>, c: *mut UObject, x: f32, y: f32) -> RuntimeResult<()> {
    unsafe { set_float(a, c, "OrgX", x)?; set_float(a, c, "OrgY", y) }
}
pub(crate) unsafe fn color(a: CoreAccess<'_>, c: *mut UObject) -> RuntimeResult<[u8; 4]> {
    unsafe { need(a, c)?; Ok(prop(a, c, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned()) }
}
unsafe fn white_rgb(a: CoreAccess<'_>, c: *mut UObject) -> RuntimeResult<()> {
    unsafe { let mut v = color(a, c)?; v[..3].fill(255); prop(a, c, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(v); } Ok(())
}
unsafe fn style(a: CoreAccess<'_>, c: *mut UObject, v: u8) -> RuntimeResult<()> {
    unsafe { prop(a, c, "Style", "Core.ByteProperty", 1)?.write(v); } Ok(())
}

/// Fixed arrays are reflected independently of scalar `prop`: no assumed offsets
/// or serialized package pointer widths enter the live object layout.
pub(crate) unsafe fn array_slot(a: CoreAccess<'_>, o: *mut UObject, n: &str, kind: &str, width: usize, count: usize, index: i32) -> RuntimeResult<*mut u8> {
    unsafe {
        need(a, o)?;
        let class = (*o).class.cast::<UClass>();
        let p = a.find_field(class.cast(), a.intern_str(n, FindName::Find)?, a.native_class(kind)?).cast::<UProperty>();
        if p.is_null() { return Err(RuntimeError::new("Console array property missing")); }
        let offset = usize::try_from((*p).offset).map_err(|_| RuntimeError::new("Console array offset"))?;
        let size = usize::try_from((*class).properties_size).map_err(|_| RuntimeError::new("Console class size"))?;
        if usize::try_from((*p).array_dim).ok() != Some(count)
            || usize::try_from((*p).element_size).ok() != Some(width)
            || offset.checked_add(width.checked_mul(count).ok_or_else(|| RuntimeError::new("Console array size"))?).is_none_or(|end| end > size)
            || index < 0 || index as usize >= count {
            return Err(RuntimeError::new("Console array layout/index"));
        }
        Ok(o.cast::<u8>().add(offset + index as usize * width))
    }
}
unsafe fn member(a: CoreAccess<'_>, o: *mut UObject, n: &str, m: &str, kind: &str, width: usize) -> RuntimeResult<*mut u8> {
    unsafe {
        need(a, o)?;
        let p = a.find_field((*o).class.cast(), a.intern_str(n, FindName::Find)?, a.native_class("Core.StructProperty")?).cast::<UStructProperty>();
        if p.is_null() || (*p).r#struct.is_null() { return Err(RuntimeError::new("Console struct missing")); }
        let structure = (*p).r#struct.cast::<UStruct>();
        let size = usize::try_from((*structure).properties_size).map_err(|_| RuntimeError::new("Console struct size"))?;
        let element_size = usize::try_from((*p.cast::<UProperty>()).element_size).map_err(|_| RuntimeError::new("Console struct element size"))?;
        if element_size < size { return Err(RuntimeError::new("Console struct extent")); }
        let base = prop(a, o, n, "Core.StructProperty", element_size)?;
        Ok(base.add(field(a, structure, m, kind, width, size, false)?))
    }
}
pub(crate) unsafe fn declared(a: CoreAccess<'_>, path: &str) -> RuntimeResult<*mut UFunction> {
    unsafe { Ok(a.find_object_by_name(a.native_class("Core.Function")?, &path.chars().map(u32::from).collect::<Vec<_>>())?.unwrap_or(ptr::null_mut()).cast()) }
}
pub(crate) unsafe fn resolved(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    unsafe {
        need(a, o)?;
        let f = a.find_object_function_by_name(o, a.intern_str(name, FindName::Find)?, false)?;
        if f.is_null() { Err(RuntimeError::new("Console callback missing")) } else { Ok(f) }
    }
}
pub(crate) unsafe fn parameter(a: CoreAccess<'_>, f: *mut UFunction, name: &str, kind: &str, width: usize) -> RuntimeResult<usize> {
    unsafe {
        if f.is_null() { return Err(RuntimeError::new("Console callback missing")); }
        field(a, f.cast(), name, kind, width, usize::from((*f).parms_size), true)
    }
}
unsafe fn canvas_parameter(a: CoreAccess<'_>, f: *mut UFunction) -> RuntimeResult<&'static str> {
    for name in ["C", "Canvas"] {
        if unsafe { parameter(a, f, name, "Core.ObjectProperty", size_of::<*mut UObject>()) }.is_ok() { return Ok(name); }
    }
    Err(RuntimeError::new("Console Canvas parameter"))
}
unsafe fn call_canvas(a: CoreAccess<'_>, o: *mut UObject, f: *mut UFunction, c: *mut UObject) -> RuntimeResult<()> {
    unsafe { let n = canvas_parameter(a, f)?; ui_vm::invoke_resolved(a, o, f, &[(n, V::Obj(c))], &[], ptr::null_mut())?; need(a, c)?; need(a, o) }
}
unsafe fn call_empty(a: CoreAccess<'_>, o: *mut UObject, f: *mut UFunction) -> RuntimeResult<()> {
    unsafe { ui_vm::invoke_resolved(a, o, f, &[], &[], ptr::null_mut())?; need(a, o) }
}
pub(crate) unsafe fn string_value(_a: CoreAccess<'_>, raw: &TArrayRaw) -> RuntimeResult<String> {
    if raw.num < 0 || raw.num > raw.max || (raw.num != 0 && raw.data.is_null()) { return Err(RuntimeError::new("Console FString layout")); }
    if raw.num == 0 { return Ok(String::new()); }
    Ok(unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) }.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect())
}
pub(crate) unsafe fn read_string(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> RuntimeResult<String> {
    unsafe {
        need(a, o)?;
        let raw = prop(a, o, name, "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned();
        string_value(a, &raw)
    }
}
unsafe fn returned_string(a: CoreAccess<'_>, o: *mut UObject, f: *mut UFunction, args: &[(&str, V<'_>)]) -> RuntimeResult<String> {
    unsafe {
        parameter(a, f, "ReturnValue", "Core.StrProperty", size_of::<TArrayRaw>())?;
        let mut raw: TArrayRaw = std::mem::zeroed();
        let outcome = ui_vm::invoke_resolved(a, o, f, args, &[], (&raw mut raw).cast());
        let text = if outcome.is_ok() { string_value(a, &raw) } else { Err(outcome.err().unwrap()) };
        let cleanup = a.destroy_owned_fstring(&mut raw);
        let text = text?; cleanup?; need(a, o)?; Ok(text)
    }
}
pub(crate) unsafe fn measure(a: CoreAccess<'_>, c: *mut UObject, text: &str, unwrapped: bool) -> RuntimeResult<(f32, f32)> {
    let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
    let size = if unwrapped {
        unsafe { crate::canvas_owner::native_text_size(a, c.cast(), &units)? }.unwrap_or((0, 0))
    } else { unsafe { crate::canvas_owner::native_str_len(a, c.cast(), &units)? } };
    Ok((size.0 as f32, size.1 as f32))
}

#[derive(Clone, PartialEq)]
enum Picture {
    Text { text: String, rect: Rect, color: [u8; 4] },
    Tile { texture: TextureRef, rect: Rect, uv: Rect, color: [u8; 4] },
}
fn view(picture: &Picture) -> Node {
    let node = match picture {
        Picture::Text { text, rect, color } => label(text).font(Font::Normal).color(Color(color[0], color[1], color[2], color[3])).at(rect.x, rect.y).width(Size::Px(rect.w)).height(Size::Px(rect.h)),
        Picture::Tile { texture, rect, uv, color } => image(texture.clone(), Fit::Region(uv.x, uv.y, uv.w, uv.h)).color(Color(color[0], color[1], color[2], color[3])).blend(BlendMode::Inherit).at(rect.x, rect.y).width(Size::Px(rect.w)).height(Size::Px(rect.h)),
    };
    // Absolute children preserve signed/zero geometry and are deliberately not clipped.
    col().children([node])
}
struct Retained {
    receiver: Weak,
    model: Signal<Picture>,
    ui: Ui,
    metrics: HostMetrics,
}
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }
struct Target { canvas: *mut UObject, cr: bool, clipped: bool }
impl DrawTarget for Target {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected console fill")) }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        let index = name.strip_prefix('#').and_then(|n| n.parse::<i32>().ok()).ok_or_else(|| RuntimeError::new("Console texture identity"))?;
        let o = a.object_at(index).cast::<UObject>();
        unsafe { need(a, o)?; } Ok(o)
    }
    unsafe fn segment(&self, a: CoreAccess<'_>, texture: *mut UObject, r: Rect, uv: Rect, tint: [u8; 4], _: BlendMode) -> RuntimeResult<()> {
        unsafe {
            prop(a, self.canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(tint);
            set_pos(a, self.canvas, r.x, r.y)?;
            if self.clipped { crate::canvas_owner::native_draw_tile_clipped(a, self.canvas.cast(), texture.cast(), r.w, r.h, uv.x, uv.y, uv.w, uv.h) }
            else { crate::canvas_owner::native_draw_tile(a, self.canvas.cast(), texture.cast(), r.w, r.h, uv.x, uv.y, uv.w, uv.h) }
        }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, _: Font, tint: [u8; 4]) -> RuntimeResult<()> {
        unsafe {
            prop(a, self.canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(tint);
            set_pos(a, self.canvas, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.canvas.cast(), &units, self.cr, 0)
        }
    }
}
unsafe fn paint(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, picture: Picture, cr: bool, clipped: bool) -> RuntimeResult<()> {
    unsafe {
        need(a, receiver)?; need(a, canvas)?;
        let retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut(); views.retain(|v| v.receiver.live(a));
            views.iter().position(|v| v.receiver.p == receiver).map(|i| views.swap_remove(i))
        });
        let metrics = retained.as_ref().map_or_else(HostMetrics::default, |v| v.metrics.clone());
        // Every phase is measured by the original native, if the source measured.
        // No cached provider/font/viewport metrics suppress a script/native call.
        metrics.clear();
        if let Picture::Text { text, rect, .. } = &picture { metrics.learn(text, Font::Normal, (rect.w, rect.h)); }
        let mut retained = retained.unwrap_or_else(|| {
            let store = Store::new(); let model = store.signal(picture.clone()); let observed = model.clone();
            Retained { receiver: Weak::new(receiver), model, ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics: metrics.clone() }
        });
        retained.model.set(picture);
        retained.ui.resize(int(a, canvas, "SizeX")? as f32, int(a, canvas, "SizeY")? as f32);
        retained.ui.relayout();
        let result = render_commands(&Target { canvas, cr, clipped }, a, &retained.ui.draw(), &metrics);
        if retained.receiver.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}
pub(crate) unsafe fn text(a: CoreAccess<'_>, receiver: *mut UObject, c: *mut UObject, value: String, x: f32, y: f32, size: (f32, f32), cr: bool) -> RuntimeResult<()> {
    unsafe {
        // The toolkit omits empty labels; the original native still applies CR
        // and cursor effects to an empty string.
        if value.is_empty() { set_pos(a, c, x, y)?; return crate::canvas_owner::draw_text(a, c.cast(), &[0], cr, 0); }
        paint(a, receiver, c, Picture::Text { text: value, rect: Rect::new(x, y, size.0, size.1), color: color(a, c)? }, cr, false)
    }
}
pub(crate) unsafe fn tile(a: CoreAccess<'_>, receiver: *mut UObject, c: *mut UObject, texture: *mut UObject, rect: Rect, uv: Rect, clipped: bool) -> RuntimeResult<()> {
    unsafe {
        if texture.is_null() {
            set_pos(a, c, rect.x, rect.y)?;
            // Keep canonical null-texture logging/cursor behavior, not a proxy.
            return if clipped { crate::canvas_owner::native_draw_tile_clipped(a, c.cast(), texture.cast(), rect.w, rect.h, uv.x, uv.y, uv.w, uv.h) }
                else { crate::canvas_owner::native_draw_tile(a, c.cast(), texture.cast(), rect.w, rect.h, uv.x, uv.y, uv.w, uv.h) };
        }
        need(a, texture)?;
        paint(a, receiver, c, Picture::Tile { texture: TextureRef(format!("#{}", (*texture).index)), rect, uv, color: color(a, c)? }, false, clipped)
    }
}

#[derive(Clone, Copy)]
enum Route { Engine, Harry, UWindow }
struct Functions {
    engine: *mut UFunction, harry: *mut UFunction, uwindow: *mut UFunction,
    action: *mut UFunction, base_action: *mut UFunction,
    console: *mut UFunction, single: *mut UFunction, timedemo: *mut UFunction,
    print: *mut UFunction, upper: *mut UFunction, scale: *mut UFunction, locked: *mut UFunction,
    format: *mut UFunction, entry: *mut UFunction,
}
impl Functions {
    unsafe fn load(a: CoreAccess<'_>) -> RuntimeResult<Self> {
        unsafe { Ok(Self {
            engine: declared(a, "Engine.Console.PostRender")?, harry: declared(a, "HGame.HPConsole.PostRender")?, uwindow: declared(a, "UWindow.WindowConsole.UWindow.PostRender")?,
            action: declared(a, "Engine.Console.DrawLevelAction")?, base_action: declared(a, "HGame.baseConsole.DrawLevelAction")?,
            console: declared(a, "Engine.Console.DrawConsoleView")?, single: declared(a, "Engine.Console.DrawSingleView")?, timedemo: declared(a, "Engine.Console.TimeDemoRender")?,
            print: declared(a, "Engine.Console.PrintActionMessage")?, upper: declared(a, "HGame.baseConsole.PrintActionMessageInUpperLeft")?, scale: declared(a, "HGame.baseConsole.ScaleAndDraw")?, locked: declared(a, "HGame.HPConsole.DrawE3DemoLockout")?,
            format: declared(a, "Engine.Console.FormatFloat")?, entry: declared(a, "Engine.PlayerPawn.GetEntryLevel")?,
        }) }
    }
}
unsafe fn player(a: CoreAccess<'_>, console: *mut UObject) -> RuntimeResult<*mut UObject> {
    unsafe { need(a, console)?; let viewport = object(a, console, "Viewport")?; need(a, viewport)?; let p = object(a, viewport, "Actor")?; need(a, p)?; Ok(p) }
}
unsafe fn level(a: CoreAccess<'_>, console: *mut UObject) -> RuntimeResult<*mut UObject> {
    unsafe { let l = object(a, player(a, console)?, "Level")?; need(a, l)?; Ok(l) }
}
unsafe fn array_text(a: CoreAccess<'_>, console: *mut UObject, index: i32) -> RuntimeResult<String> {
    unsafe { string_value(a, &array_slot(a, console, "MsgText", "Core.StrProperty", size_of::<TArrayRaw>(), 64, index)?.cast::<TArrayRaw>().read_unaligned()) }
}
unsafe fn message(a: CoreAccess<'_>, console: *mut UObject, index: i32, scrollback: bool) -> RuntimeResult<String> {
    unsafe {
        let value = array_text(a, console, index)?;
        let kind = array_slot(a, console, "MsgType", "Core.NameProperty", 4, 64, index)?.cast::<i32>().read_unaligned();
        if kind != a.intern_str("Say", FindName::Find)? && (!scrollback || kind != a.intern_str("TeamSay", FindName::Find)?) { return Ok(value); }
        let sender = array_slot(a, console, "MsgPlayer", "Core.ObjectProperty", size_of::<*mut UObject>(), 64, index)?.cast::<*mut UObject>().read_unaligned();
        let who = if scrollback {
            need(a, sender)?; read_string(a, sender, "PlayerName")?
        } else if sender.is_null() { "None".to_owned() } else {
            need(a, sender)?;
            a.object_path_text(sender)?.into_iter().take_while(|&c| c != 0).map(char::from_u32)
                .collect::<Option<String>>().ok_or_else(|| RuntimeError::new("Invalid console sender Unicode"))?
        };
        Ok(format!("{who}: {value}"))
    }
}
unsafe fn console_view(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        let mut y = int(a, console, "ConsoleLines")?.wrapping_sub(1);
        let prompt_index = ((int(a, console, "TopLine")?.wrapping_add(1).wrapping_add(64) as f32) % 64.) as i32;
        let prompt = format!("(> {}", read_string(a, console, "TypedStr")?);
        let slot = array_slot(a, console, "MsgText", "Core.StrProperty", size_of::<TArrayRaw>(), 64, prompt_index)?.cast::<TArrayRaw>();
        a.assign_fstring(&mut *slot, &prompt.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?;
        let mut i = int(a, console, "Scrollback")?;
        while i < int(a, console, "NumLines")?.wrapping_add(1) {
            let line = ((int(a, console, "TopLine")?.wrapping_add(128).wrapping_sub(i.wrapping_sub(1)) as f32) % 64.) as i32;
            set_object(a, c, "Font", object(a, c, "MedFont")?)?;
            let measured = message(a, console, line, true)?;
            let (w, mut h) = measure(a, c, &measured, false)?;
            if h == 0. { h = 5.; }
            y = y.wrapping_sub(h as i32);
            if y as f32 + h < 0. { break; }
            set_pos(a, c, 4., y as f32)?;
            set_object(a, c, "Font", object(a, c, "MedFont")?)?;
            let value = message(a, console, line, true)?;
            text(a, console, c, value, 4., y as f32, (w, h), false)?;
            i = i.wrapping_add(1);
        }
        Ok(())
    }
}
unsafe fn single_view(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        origin(a, c, 0., 0.)?;
        let hud = object(a, player(a, console)?, "myHUD")?;
        if !hud.is_null() {
            let f = resolved(a, hud, "DisplayMessages")?;
            let mut handled = 0u32;
            ui_vm::invoke_resolved(a, hud, f, &[(canvas_parameter(a, f)?, V::Obj(c))], &[], (&raw mut handled).cast())?;
            need(a, console)?; need(a, c)?;
            if handled != 0 { return Ok(()); }
        }
        if !read_boolean(a, player(a, console)?, "bShowMenu")? && read_boolean(a, console, "bTyping")? {
            let prompt = format!("(> {}_", read_string(a, console, "TypedStr")?);
            set_object(a, c, "Font", object(a, c, "MedFont")?)?;
            let size = measure(a, c, &prompt, false)?;
            let y = ((float(a, console, "FrameY")? - int(a, console, "ConsoleLines")? as f32) - size.1) - 1.;
            text(a, console, c, prompt, 2., y, size, false)?;
        }
        if int(a, console, "TextLines")? > 0 && (!read_boolean(a, player(a, console)?, "bShowMenu")? || read_boolean(a, player(a, console)?, "bShowScores")?) {
            let mut messages: [String; 4] = std::array::from_fn(|_| String::new());
            let mut j = int(a, console, "TopLine")?;
            let mut i = 0;
            while i < 4 && j >= 0 {
                if !array_text(a, console, j)?.is_empty() {
                    let tick = array_slot(a, console, "MsgTick", "Core.FloatProperty", 4, 64, j)?.cast::<f32>().read_unaligned();
                    if tick > 0. && tick > float(a, console, "MsgTickTime")? { messages[i] = message(a, console, j, false)?; i += 1; }
                }
                j = j.wrapping_sub(1);
            }
            set_object(a, c, "Font", object(a, c, "MedFont")?)?;
            let mut row = 0i32;
            let mut extra = 0i32;
            for value in messages.into_iter().rev().filter(|s| !s.is_empty()) {
                let y = (2. + row.wrapping_mul(10) as f32) + extra.wrapping_mul(10) as f32;
                set_pos(a, c, 4., y)?;
                let size = measure(a, c, &value, false)?;
                text(a, console, c, value, 4., y, size, false)?;
                if size.1 == 18. { extra = extra.wrapping_add(1); }
                row = row.wrapping_add(1);
            }
        }
        Ok(())
    }
}

unsafe fn print_message(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, f: *mut UFunction, functions: &Functions, value: &str, upper: bool) -> RuntimeResult<()> {
    unsafe {
        let canonical = if upper { functions.upper } else { functions.print };
        if f != canonical || canonical.is_null() {
            ui_vm::invoke_resolved(a, console, f, &[(canvas_parameter(a, f)?, V::Obj(c)), ("BigMessage", V::S(value))], &[], ptr::null_mut())?;
            need(a, c)?; return need(a, console);
        }
        boolean(a, c, "bCenter", false)?;
        let size = measure(a, c, value, false)?;
        let x = float(a, console, "FrameX")? / if upper { 4. } else { 2. } - size.0 / 2.;
        let y = float(a, console, "FrameY")? / if upper { 4.5 } else { 2. } - size.1 / 2.;
        text(a, console, c, value.to_owned(), x, y, size, false)
    }
}
unsafe fn scale_and_draw(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions, x: f32, y: f32, texture: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        let f = resolved(a, console, "ScaleAndDraw")?;
        if f != functions.scale || functions.scale.is_null() {
            ui_vm::invoke_resolved(a, console, f, &[("C", V::Obj(c)), ("X", V::F(x)), ("Y", V::F(y)), ("Tex", V::Obj(texture))], &[], ptr::null_mut())?;
            need(a, c)?; return need(a, console);
        }
        if texture.is_null() { return Ok(()); }
        need(a, texture)?;
        // The retail helper overwrites both computed viewport scales with 1.
        let w = int(a, texture, "USize")? as f32;
        let h = int(a, texture, "VSize")? as f32;
        let root = object(a, console, "Root")?; need(a, root)?;
        let gui = float(a, object(a, root, "Root")?, "GUIScale")?;
        let get = |n| -> RuntimeResult<f32> { Ok(member(a, root, "ClippingRegion", n, "Core.IntProperty", 4)?.cast::<i32>().read_unaligned() as f32) };
        let (rx, ry, rw, rh) = (get("X")?, get("Y")?, get("W")?, get("H")?);
        let (ox, oy, cx, cy) = (float(a, c, "OrgX")?, float(a, c, "OrgY")?, float(a, c, "ClipX")?, float(a, c, "ClipY")?);
        origin(a, c, ox + rx * gui, oy + ry * gui)?;
        set_float(a, c, "ClipX", rw * gui)?; set_float(a, c, "ClipY", rh * gui)?;
        let drawn = tile(a, console, c, texture, Rect::new((x - rx) * gui, (y - ry) * gui, w * gui, h * gui), Rect::new(0., 0., w, h), true);
        let restored = (|| -> RuntimeResult<()> { set_float(a, c, "ClipX", cx)?; set_float(a, c, "ClipY", cy)?; origin(a, c, ox, oy) })();
        drawn?; restored
    }
}
unsafe fn localize(a: CoreAccess<'_>, console: *mut UObject, key: &str) -> RuntimeResult<String> {
    unsafe { returned_string(a, console, resolved(a, console, "Localize")?, &[("SectionName", V::S("all")), ("KeyName", V::S(key)), ("PackageName", V::S("HPMenu"))]) }
}
unsafe fn action(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        let f = resolved(a, console, "DrawLevelAction")?;
        if f != functions.action && f != functions.base_action { return call_canvas(a, console, f, c); }
        let base = f == functions.base_action && !functions.base_action.is_null();
        let pauser = read_string(a, level(a, console)?, "Pauser")?;
        let act = prop(a, level(a, console)?, "LevelAction", "Core.ByteProperty", 1)?.read();
        if !pauser.is_empty() && act == 0 {
            set_object(a, c, "Font", object(a, c, "MedFont")?)?;
            let value = read_string(a, console, "PausedMessage")?;
            return print_message(a, console, c, resolved(a, console, "PrintActionMessage")?, functions, &value, false);
        }
        if act == 0 || (!base && read_boolean(a, player(a, console)?, "bShowMenu")?) { return Ok(()); }
        if base && (act == 1 || read_boolean(a, player(a, console)?, "bShowLoadingScreen")?) {
            let value = localize(a, console, "Options_0058")?;
            need(a, c)?;
            boolean(a, player(a, console)?, "bShowLoadingScreen", true)?;
            style(a, c, 1)?;
            let size = measure(a, c, &value, true)?;
            let font = if size.0 > int(a, c, "SizeX")?.wrapping_sub(32) as f32 { "LocalMedFont" } else { "LocalBigFont" };
            set_object(a, c, "Font", object(a, console, font)?)?;
            for (n, x, y) in [("p1", 0., 0.), ("p2", 256., 0.), ("p3", 512., 0.), ("p4", 0., 256.), ("p5", 256., 256.), ("p6", 512., 256.)] {
                let texture = member(a, console, "LoadingBackground", n, "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().read_unaligned();
                scale_and_draw(a, console, c, functions, x, y, texture)?;
            }
            return print_message(a, console, c, resolved(a, console, "PrintActionMessageInUpperLeft")?, functions, &value, true);
        }
        let value = match act {
            1 => read_string(a, console, "LoadingMessage")?,
            2 if base => localize(a, console, "Options_0057")?,
            2 => read_string(a, console, "SavingMessage")?,
            3 => read_string(a, console, "ConnectingMessage")?,
            4 => read_string(a, console, "PrecachingMessage")?,
            _ => String::new(),
        };
        if !value.is_empty() {
            style(a, c, 1)?;
            let font = if base {
                let size = measure(a, c, &value, true)?;
                object(a, console, if size.0 > int(a, c, "SizeX")?.wrapping_sub(32) as f32 { "LocalMedFont" } else { "LocalBigFont" })?
            } else { object(a, c, "LargeFont")? };
            set_object(a, c, "Font", font)?;
            print_message(a, console, c, resolved(a, console, "PrintActionMessage")?, functions, &value, false)?;
        }
        Ok(())
    }
}
unsafe fn timedemo(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        if !read_boolean(a, console, "bTimeDemo")? { return Ok(()); }
        call_empty(a, console, resolved(a, console, "TimeDemoCalc")?)?;
        need(a, c)?;
        let f = resolved(a, console, "TimeDemoRender")?;
        if f != functions.timedemo || functions.timedemo.is_null() { return call_canvas(a, console, f, c); }
        set_object(a, c, "Font", object(a, console, "TimeDemoFont")?)?;
        white_rgb(a, c)?;
        let entry = crate::engine_vm_native::entry_level_info(player(a, console)?)?;
        need(a, entry)?; need(a, console)?; need(a, c)?;
        let average = int(a, console, "FrameCount")? as f32 / ((float(a, entry, "TimeSeconds")? - float(a, console, "StartTime")?) - float(a, console, "ExtraTime")?);
        let avg = read_string(a, console, "AvgText")?;
        let avg = format!("{avg} {}", returned_string(a, console, functions.format, &[("F", V::F(average))])?);
        let last = read_string(a, console, "LastSecText")?;
        let last = format!("{last} {}", returned_string(a, console, functions.format, &[("F", V::F(float(a, console, "LastSecFPS")?))])?);
        let size = measure(a, c, &avg, true)?;
        text(a, console, c, avg, float(a, c, "ClipX")? - size.0, 0.3f32 * float(a, c, "ClipY")?, size, true)?;
        let size = measure(a, c, &last, true)?;
        text(a, console, c, last, float(a, c, "ClipX")? - size.0, 0.3f32 * float(a, c, "ClipY")? + size.1, size, true)
    }
}
unsafe fn pattern(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, texture: *mut UObject, r: Rect) -> RuntimeResult<()> {
    unsafe { tile(a, console, c, texture, r, Rect::new(r.x - float(a, c, "OrgX")?, r.y - float(a, c, "OrgY")?, r.w, r.h), false) }
}
unsafe fn engine(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        if read_boolean(a, console, "bNoDrawWorld")? {
            set_pos(a, c, 0., 0.)?;
            pattern(a, console, c, find_texture(a, "Engine.Border")?, Rect::new(0., 0., float(a, c, "ClipX")?, float(a, c, "ClipY")?))?;
        }
        timedemo(a, console, c, functions)?;
        action(a, console, c, functions)?;
        if int(a, console, "ConsoleLines")? > 0 {
            origin(a, c, 0., int(a, console, "ConsoleLines")? as f32 - float(a, console, "FrameY")? * 0.6f32)?;
            tile(a, console, c, object(a, console, "ConBackground")?, Rect::new(0., 0., float(a, console, "FrameX")?, float(a, console, "FrameY")? * 0.6f32), Rect::new(0., 0., float(a, console, "FrameX")?, float(a, console, "FrameY")?), false)?;
        }
        let lines = int(a, console, "BorderLines")?;
        let pixels = int(a, console, "BorderPixels")?;
        if lines > 0 || pixels > 0 {
            let start = lines.wrapping_add(int(a, console, "ConsoleLines")?);
            let end = (float(a, console, "FrameY")? - lines as f32) as i32;
            if lines > 0 {
                origin(a, c, 0., 0.)?;
                pattern(a, console, c, object(a, console, "Border")?, Rect::new(0., 0., float(a, console, "FrameX")?, lines as f32))?;
                pattern(a, console, c, object(a, console, "Border")?, Rect::new(0., end as f32, float(a, console, "FrameX")?, lines as f32))?;
            }
            if pixels > 0 {
                origin(a, c, 0., 0.)?;
                pattern(a, console, c, object(a, console, "Border")?, Rect::new(0., start as f32, pixels as f32, end.wrapping_sub(start) as f32))?;
                pattern(a, console, c, object(a, console, "Border")?, Rect::new(float(a, console, "FrameX")? - pixels as f32, start as f32, pixels as f32, end.wrapping_sub(start) as f32))?;
            }
        }
        origin(a, c, 0., 0.)?;
        let name = if int(a, console, "ConsoleLines")? > 0 { "DrawConsoleView" } else { "DrawSingleView" };
        let f = resolved(a, console, name)?;
        if f == functions.console && !f.is_null() { console_view(a, console, c) }
        else if f == functions.single && !f.is_null() { single_view(a, console, c) }
        else { call_canvas(a, console, f, c) }
    }
}
unsafe fn lockout(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        let f = resolved(a, console, "DrawE3DemoLockout")?;
        if f != functions.locked || functions.locked.is_null() { return call_canvas(a, console, f, c); }
        let saved = object(a, c, "Font")?;
        let root = object(a, console, "Root")?;
        let font = array_slot(a, root, "Fonts", "Core.ObjectProperty", size_of::<*mut UObject>(), 10, 0)?.cast::<*mut UObject>().read_unaligned();
        set_object(a, c, "Font", font)?;
        let drawn = text(a, console, c, "Locked".to_owned(), 10., 460., (0., 0.), true);
        let restored = set_object(a, c, "Font", saved);
        drawn?; restored
    }
}
unsafe fn harry(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        // Retail explicitly names Engine.Console, not nearest-super PostRender.
        engine(a, console, c, functions)?;
        if !object(a, console, "Root")?.is_null() {
            let book = object(a, console, "menuBook")?;
            need(a, book)?;
            let open = read_boolean(a, book, "bIsOpen")?;
            if open || read_boolean(a, console, "bShowCutConsole")? || read_boolean(a, console, "bVendorBar")? {
                call_canvas(a, console, resolved(a, console, "RenderUWindow")?, c)?;
            }
        }
        if read_boolean(a, console, "bShowPos")? {
            white_rgb(a, c)?;
            let x = int(a, c, "SizeX")?.wrapping_sub(200) as f32;
            let y = int(a, c, "SizeY")?.wrapping_sub(40) as f32;
            set_pos(a, c, x, y)?;
            let read = || -> RuntimeResult<[f32; 3]> { Ok(prop(a, player(a, console)?, "Location", "Core.StructProperty", 12)?.cast::<[f32; 3]>().read_unaligned()) };
            let px = read()?[0] as i32; let py = read()?[1] as i32; let pz = read()?[2] as i32;
            text(a, console, c, format!("Player @ {px},{py},{pz}"), x, y, (0., 0.), true)?;
        }
        let p = player(a, console)?;
        let Some(class) = find_class(a, "HGame.harry")? else { return Ok(()); };
        if is_a(a, p, class) && read_boolean(a, p, "bE3DemoLockout")? { lockout(a, console, c, functions)?; }
        Ok(())
    }
}

/// Only metadata/property reads occur here. Custom leaf functions are admitted
/// as real script callbacks, with their parameter capabilities checked in advance.
unsafe fn preflight(a: CoreAccess<'_>, console: *mut UObject, c: *mut UObject, route: Route, f: *mut UFunction, functions: &Functions) -> RuntimeResult<()> {
    unsafe {
        need(a, console)?; need(a, c)?; canvas_parameter(a, f)?;
        for n in ["CurX", "CurY", "CurYL", "OrgX", "OrgY", "ClipX", "ClipY"] { float(a, c, n)?; }
        for n in ["SizeX", "SizeY"] { int(a, c, n)?; }
        color(a, c)?; prop(a, c, "Style", "Core.ByteProperty", 1)?; object(a, c, "Font")?;
        read_boolean(a, console, "bTimeDemo")?;
        let calc = resolved(a, console, "TimeDemoCalc")?;
        if (*calc).num_parms != 0 { return Err(RuntimeError::new("TimeDemoCalc parameters")); }
        let render = resolved(a, console, "TimeDemoRender")?; canvas_parameter(a, render)?;
        if render == functions.timedemo {
            object(a, console, "TimeDemoFont")?; int(a, console, "FrameCount")?;
            for n in ["StartTime", "ExtraTime", "LastSecFPS"] { float(a, console, n)?; }
            for n in ["AvgText", "LastSecText"] { read_string(a, console, n)?; }
            parameter(a, functions.format, "F", "Core.FloatProperty", 4)?;
            parameter(a, functions.format, "ReturnValue", "Core.StrProperty", size_of::<TArrayRaw>())?;
            parameter(a, functions.entry, "ReturnValue", "Core.ObjectProperty", size_of::<*mut UObject>())?;
        }
        if matches!(route, Route::UWindow | Route::Harry) {
            let root = object(a, console, "Root")?;
            if !root.is_null() { need(a, root)?; read_boolean(a, root, "bUWindowActive")?; }
            canvas_parameter(a, resolved(a, console, "RenderUWindow")?)?;
        }
        if matches!(route, Route::UWindow) { return Ok(()); }
        let p = player(a, console)?; let l = level(a, console)?;
        for n in ["bShowMenu", "bShowScores"] { read_boolean(a, p, n)?; }
        for n in ["bNoDrawWorld", "bTyping"] { read_boolean(a, console, n)?; }
        for n in ["FrameX", "FrameY", "MsgTickTime"] { float(a, console, n)?; }
        for n in ["ConsoleLines", "BorderLines", "BorderPixels", "TopLine", "Scrollback", "NumLines", "TextLines"] { int(a, console, n)?; }
        for n in ["Border", "ConBackground"] {
            let t = object(a, console, n)?;
            if !t.is_null() { need(a, t)?; int(a, t, "USize")?; int(a, t, "VSize")?; }
        }
        if read_boolean(a, console, "bNoDrawWorld")? { find_texture(a, "Engine.Border")?; }
        for (n, kind, width) in [("MsgText", "Core.StrProperty", size_of::<TArrayRaw>()), ("MsgType", "Core.NameProperty", 4), ("MsgPlayer", "Core.ObjectProperty", size_of::<*mut UObject>()), ("MsgTick", "Core.FloatProperty", 4)] { array_slot(a, console, n, kind, width, 64, 0)?; }
        read_string(a, console, "TypedStr")?;
        object(a, c, "MedFont")?; object(a, c, "LargeFont")?; read_boolean(a, c, "bCenter")?;
        for n in ["DrawConsoleView", "DrawSingleView"] { canvas_parameter(a, resolved(a, console, n)?)?; }
        if int(a, console, "ConsoleLines")? > 0 && resolved(a, console, "DrawConsoleView")? == functions.console {
            let start = int(a, console, "Scrollback")?;
            let end = int(a, console, "NumLines")?.wrapping_add(1);
            // Outside the source-maintained ring domain, preserve the stock
            // bounds diagnostics instead of emulating VM error reporting.
            if start < 0 || end > 64 {
                return Err(RuntimeError::new("Console ring diagnostic route remains stock"));
            }
            for i in start..end {
                let index = ((int(a, console, "TopLine")?.wrapping_add(128).wrapping_sub(i.wrapping_sub(1)) as f32) % 64.) as i32;
                let kind = array_slot(a, console, "MsgType", "Core.NameProperty", 4, 64, index)?.cast::<i32>().read_unaligned();
                if kind == a.intern_str("Say", FindName::Find)? || kind == a.intern_str("TeamSay", FindName::Find)? {
                    let sender = array_slot(a, console, "MsgPlayer", "Core.ObjectProperty", size_of::<*mut UObject>(), 64, index)?.cast::<*mut UObject>().read_unaligned();
                    need(a, sender)?; read_string(a, sender, "PlayerName")?;
                }
            }
        }
        let hud = object(a, p, "myHUD")?;
        if !hud.is_null() {
            let display = resolved(a, hud, "DisplayMessages")?;
            canvas_parameter(a, display)?;
            parameter(a, display, "ReturnValue", "Core.BoolProperty", 4)?;
        }
        let act = resolved(a, console, "DrawLevelAction")?; canvas_parameter(a, act)?;
        if act == functions.action || act == functions.base_action {
            read_string(a, l, "Pauser")?; prop(a, l, "LevelAction", "Core.ByteProperty", 1)?;
            for n in ["PausedMessage", "LoadingMessage", "SavingMessage", "ConnectingMessage", "PrecachingMessage"] { read_string(a, console, n)?; }
            let print = resolved(a, console, "PrintActionMessage")?;
            canvas_parameter(a, print)?; parameter(a, print, "BigMessage", "Core.StrProperty", size_of::<TArrayRaw>())?;
        }
        if act == functions.base_action && !act.is_null() {
            read_boolean(a, p, "bShowLoadingScreen")?;
            object(a, console, "LocalBigFont")?; object(a, console, "LocalMedFont")?;
            let loc = resolved(a, console, "Localize")?;
            for n in ["SectionName", "KeyName", "PackageName", "ReturnValue"] { parameter(a, loc, n, "Core.StrProperty", size_of::<TArrayRaw>())?; }
            let upper = resolved(a, console, "PrintActionMessageInUpperLeft")?;
            canvas_parameter(a, upper)?; parameter(a, upper, "BigMessage", "Core.StrProperty", size_of::<TArrayRaw>())?;
            let scale = resolved(a, console, "ScaleAndDraw")?;
            parameter(a, scale, "C", "Core.ObjectProperty", size_of::<*mut UObject>())?;
            for n in ["X", "Y"] { parameter(a, scale, n, "Core.FloatProperty", 4)?; }
            parameter(a, scale, "Tex", "Core.ObjectProperty", size_of::<*mut UObject>())?;
            for n in ["p1", "p2", "p3", "p4", "p5", "p6"] {
                let t = member(a, console, "LoadingBackground", n, "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().read_unaligned();
                if !t.is_null() { need(a, t)?; int(a, t, "USize")?; int(a, t, "VSize")?; }
            }
            if scale == functions.scale {
                let root = object(a, console, "Root")?;
                if !root.is_null() {
                    let rr = object(a, root, "Root")?; float(a, rr, "GUIScale")?;
                    for n in ["X", "Y", "W", "H"] { member(a, root, "ClippingRegion", n, "Core.IntProperty", 4)?; }
                } else if prop(a, l, "LevelAction", "Core.ByteProperty", 1)?.read() != 0
                    && (prop(a, l, "LevelAction", "Core.ByteProperty", 1)?.read() == 1 || read_boolean(a, p, "bShowLoadingScreen")?) {
                    return Err(RuntimeError::new("Loading view has no Root; retain stock null route"));
                }
            }
        }
        if matches!(route, Route::Harry) {
            for n in ["bShowPos", "bShowCutConsole", "bVendorBar"] { read_boolean(a, console, n)?; }
            let book = object(a, console, "menuBook")?;
            if !object(a, console, "Root")?.is_null() { need(a, book)?; read_boolean(a, book, "bIsOpen")?; }
            prop(a, p, "Location", "Core.StructProperty", 12)?;
            let locked = resolved(a, console, "DrawE3DemoLockout")?; canvas_parameter(a, locked)?;
            if let Some(class) = find_class(a, "HGame.harry")? {
                if is_a(a, p, class) && read_boolean(a, p, "bE3DemoLockout")? && locked == functions.locked {
                    array_slot(a, object(a, console, "Root")?, "Fonts", "Core.ObjectProperty", size_of::<*mut UObject>(), 10, 0)?;
                }
            }
        }
        Ok(())
    }
}

/// Called only at console_owner's existing outer event boundary. `false` is
/// exclusively pre-effect stock admission; errors after admission never replay.
pub(crate) unsafe fn post_render(a: CoreAccess<'_>, console: *mut UObject, canvas: *mut UObject, function: *mut UFunction) -> RuntimeResult<bool> {
    if std::env::var_os("HP2_STOCK_HUD").is_some() || !live(a, console) || !live(a, canvas) || function.is_null() { return Ok(false); }
    let admitted = (|| -> RuntimeResult<Option<(Route, Functions)>> {
        let functions = unsafe { Functions::load(a)? };
        let route = if function == functions.engine { Route::Engine } else if function == functions.harry { Route::Harry } else if function == functions.uwindow { Route::UWindow } else { return Ok(None); };
        unsafe { preflight(a, console, canvas, route, function, &functions)?; }
        Ok(Some((route, functions)))
    })();
    let Ok(Some((route, functions))) = admitted else { return Ok(false); };
    unsafe {
        match route {
            Route::Engine => engine(a, console, canvas, &functions)?,
            Route::Harry => harry(a, console, canvas, &functions)?,
            Route::UWindow => {
                timedemo(a, console, canvas, &functions)?;
                let root = object(a, console, "Root")?;
                if !root.is_null() { need(a, root)?; boolean(a, root, "bUWindowActive", true)?; }
                call_canvas(a, console, resolved(a, console, "RenderUWindow")?, canvas)?;
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
pub(super) fn assert_view() {
    // Data-level contract: absolute geometry remains signed; source colors are
    // passed explicitly, not an opaque global text/image style policy.
    let model = Picture::Tile { texture: TextureRef("fixture".to_owned()), rect: Rect::new(-4., 8., -16., 0.), uv: Rect::new(2., 3., 4., 5.), color: [6, 7, 8, 9] };
    let store = Store::new();
    let mut ui = Ui::new(store, move || view(&model), HostMetrics::default());
    ui.resize(800., 600.);
    assert!(ui.draw().iter().any(|cmd| matches!(cmd, hp2_ui::DrawCmd::Image { rect, color: Color(6, 7, 8, 9), blend: BlendMode::Inherit, .. } if *rect == Rect::new(-4., 8., -16., 0.))));
}
