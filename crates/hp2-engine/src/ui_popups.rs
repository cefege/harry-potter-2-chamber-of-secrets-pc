//! Spell-page and warning presentation; popup dispatch and lifetime remain script-owned.
use std::{cell::RefCell, ptr};
use hp2_abi::engine::generated::{TArrayRaw, UFunction, UObject, UProperty};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Fit, Font, Node, Rect, Signal, Size, Store, TextureRef, Ui, col, image, label};
use super::{CanvasTarget, f32_of, find_function, int_of, param_object, player_fonts, set_f32};
use crate::{stock_graphics_menu::{Weak, boolean, is_a, live, object, prop, read_boolean}, ui_host::{DrawTarget, HostMetrics, render_commands}};

#[derive(Clone, Debug, PartialEq)]
struct Piece { texture: TextureRef, dest: Rect, source: Rect }
#[derive(Clone, Debug, PartialEq)]
struct Caption { text: String, at: (f32, f32), size: (f32, f32), font: Font }
#[derive(Clone, Debug, PartialEq)]
struct Popup { size: (i32, i32), pieces: Vec<Piece>, caption: Option<Caption>, color: [u8; 4] }
struct Retained { owner: Weak, model: Signal<Popup>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }

fn view(model: &Popup) -> Node {
    let color = Color(model.color[0], model.color[1], model.color[2], model.color[3]);
    let mut children = Vec::with_capacity(model.pieces.len() + usize::from(model.caption.is_some()));
    for p in &model.pieces {
        children.push(image(p.texture.clone(), Fit::Region(p.source.x, p.source.y, p.source.w, p.source.h))
            .color(color).at(p.dest.x, p.dest.y).width(Size::Px(p.dest.w)).height(Size::Px(p.dest.h)));
    }
    if let Some(c) = &model.caption {
        children.push(label(&c.text).font(c.font).color(color).at(c.at.0, c.at.1).width(Size::Px(c.size.0)).height(Size::Px(c.size.1)));
    }
    col().children(children)
}
struct Target(CanvasTarget);
impl DrawTarget for Target {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected popup fill")) }
    unsafe fn texture_object(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> { unsafe { self.0.texture_object(a, name) } }
    unsafe fn segment(&self, a: CoreAccess<'_>, texture: *mut UObject, dest: Rect, src: Rect, color: [u8; 4], blend: BlendMode) -> RuntimeResult<()> {
        // SAFETY: admitted live Canvas and resources are used synchronously.
        unsafe { self.0.segment(a, texture, dest, src, color, blend) }
    }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, font: Font, color: [u8; 4]) -> RuntimeResult<()> {
        // SAFETY: use the actual Canvas text routine, preserving incoming Style and cursor effects.
        unsafe {
            self.0.set_font(a, font)?;
            self.0.set_color(a, color)?;
            self.0.set_pos(a, x, y)?;
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a, self.0.canvas.cast(), &units, false, 0)
        }
    }
}
unsafe fn paint(a: CoreAccess<'_>, receiver: *mut UObject, target: &Target, model: Popup, metrics: HostMetrics) -> RuntimeResult<()> {
    // SAFETY: no retained-state borrow crosses rendering or script/native calls.
    unsafe {
        let mut retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|v| v.owner.live(a));
            views.iter().position(|v| v.owner.p == receiver).map(|i| views.swap_remove(i))
        }).unwrap_or_else(|| {
            let store = Store::new(); let signal = store.signal(model.clone()); let observed = signal.clone();
            Retained { owner: Weak::new(receiver), model: signal, ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics: metrics.clone() }
        });
        // Font/provider-dependent sizes are relearned on every invocation.
        if let Some(c) = &model.caption { retained.metrics.learn(&c.text, c.font, c.size); }
        retained.ui.resize(model.size.0 as f32, model.size.1 as f32);
        retained.model.set(model); retained.ui.relayout();
        let result = render_commands(target, a, &retained.ui.draw(), &retained.metrics);
        if retained.owner.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}
unsafe fn color(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<[u8; 4]> {
    unsafe { Ok(prop(a, canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned()) }
}
unsafe fn string(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<String> {
    // SAFETY: reflected FString owns the validated scalar buffer throughout this synchronous read.
    unsafe {
        let s = prop(a, receiver, name, "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned();
        if s.num < 0 || s.max < s.num || (s.num > 0 && s.data.is_null()) { return Err(RuntimeError::new("Invalid popup string")); }
        if s.num == 0 { return Ok(String::new()); }
        let units = std::slice::from_raw_parts(s.data.cast::<u32>(), s.num as usize);
        if units.last() != Some(&0) { return Err(RuntimeError::new("Unterminated popup string")); }
        units[..units.len()-1].iter().copied().map(char::from_u32).collect::<Option<_>>().ok_or_else(|| RuntimeError::new("Invalid popup Unicode"))
    }
}
unsafe fn page_pieces(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<[*mut UObject; 4]> {
    // SAFETY: validate the complete fixed array before accessing any element.
    unsafe {
        let p = a.find_field((*receiver).class.cast(), a.intern_str("pagePieces", FindName::Find)?, a.native_class("Core.ObjectProperty")?).cast::<UProperty>();
        if p.is_null() || (*p).array_dim != 4 || (*p).element_size as usize != size_of::<*mut UObject>() || (*p).offset < 0 {
            return Err(RuntimeError::new("Invalid spell-page pieces"));
        }
        let size = (*(*receiver).class.cast::<hp2_abi::engine::generated::UClass>()).properties_size;
        let offset = (*p).offset as usize;
        if offset.checked_add(4 * size_of::<*mut UObject>()).is_none_or(|end| end > size as usize) { return Err(RuntimeError::new("Spell-page array bounds")); }
        Ok(receiver.cast::<u8>().add(offset).cast::<[*mut UObject; 4]>().read_unaligned())
    }
}
fn page_rects(size: (i32, i32), offset: i32, width: i32) -> [(f32, f32); 4] {
    // Retail does integer division before conversion, with fixed 256-pixel piece anchors.
    let x = (size.0 / 2) as f32 - (width / 2) as f32;
    let y = size.1.wrapping_sub(offset) as f32;
    [(x,y), (x+256.,y), (x,y+256.), (x+256.,y+256.)]
}
unsafe fn page_preflight(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<i32> {
    unsafe {
        let pieces = page_pieces(a, receiver)?;
        if !live(a, pieces[0]) || !live(a, pieces[1]) || pieces.iter().any(|p| !p.is_null() && !live(a,*p)) {
            return Err(RuntimeError::new("Spell-page resource unavailable"));
        }
        int_of(a,canvas,"SizeX")?; int_of(a,canvas,"SizeY")?; int_of(a,receiver,"yOffset")?;
        for texture in pieces.into_iter().filter(|p| !p.is_null()) {
            int_of(a,texture,"USize")?; int_of(a,texture,"VSize")?;
        }
        Ok(int_of(a,pieces[0],"USize")?.wrapping_add(int_of(a,pieces[1],"USize")?))
    }
}
unsafe fn draw_page(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, width: i32) -> RuntimeResult<()> {
    unsafe {
        let target = Target(CanvasTarget { canvas, tile_style:None, fonts:[ptr::null_mut();4] });
        for i in 0..4 {
            if !live(a,receiver) || !live(a,canvas) {
                return Err(RuntimeError::new("Spell page destroyed during artwork callback"));
            }
            if i >= 2 && page_pieces(a,receiver)?[i].is_null() { continue; }
            // Width is a source local; viewport, offset and the next texture are not.
            let size = (int_of(a,canvas,"SizeX")?,int_of(a,canvas,"SizeY")?);
            let at = page_rects(size,int_of(a,receiver,"yOffset")?,width)[i];
            target.0.set_pos(a,at.0,at.1)?;
            let texture = page_pieces(a,receiver)?[i];
            if texture.is_null() { continue; }
            if !live(a,texture) { return Err(RuntimeError::new("Spell-page artwork destroyed")); }
            let w = int_of(a,texture,"USize")? as f32;
            let h = int_of(a,texture,"VSize")? as f32;
            let piece = Piece { texture:TextureRef(format!("#{}",(*texture).index)),dest:Rect::new(at.0,at.1,w,h),source:Rect::new(0.,0.,w,h) };
            paint(a,receiver,&target,Popup { size,pieces:vec![piece],caption:None,color:color(a,canvas)? },HostMetrics::default())?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Flash { show: bool, time: f32, draw: bool, reset: bool }
fn flash(show: bool, time: f32) -> Flash {
    // Both retail branches assign TRUE. Do not turn this unusual policy into a toggle.
    let reset = time > if show { 1. } else { 0.5 };
    Flash { show: if reset { true } else { show }, time: if reset { 0. } else { time }, draw: show, reset }
}
struct Warning { model: Popup, fonts: [*mut UObject;4], background: *mut UObject, saved_font: *mut UObject, transition: Flash }
unsafe fn warning_snapshot(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject) -> RuntimeResult<Warning> {
    unsafe {
        let show = read_boolean(a,receiver,"bShow")?; let time = f32_of(a,receiver,"fFlashTime")?;
        let saved_font = object(a,canvas,"Font")?;
        let mut model = Popup { size:(int_of(a,canvas,"SizeX")?,int_of(a,canvas,"SizeY")?), pieces:Vec::new(), caption:None, color:color(a,canvas)? };
        let mut fonts = [ptr::null_mut();4]; let mut background = ptr::null_mut();
        if show {
            let harry=object(a,receiver,"PlayerHarry")?;
            if !live(a,harry){return Err(RuntimeError::new("Warning player unavailable"));}
            let player=object(a,harry,"Player")?;
            if !live(a,player){return Err(RuntimeError::new("Warning viewport unavailable"));}
            let console=object(a,player,"Console")?;
            if !live(a,console){return Err(RuntimeError::new("Warning console unavailable"));}
            fonts = player_fonts(a,harry)?;
            if fonts[..3].iter().any(|f| !live(a,*f)) { return Err(RuntimeError::new("Warning font unavailable")); }
            background = super::find_texture(a,"HGame.Icons.leftPanel")?;
            if !live(a,background) { return Err(RuntimeError::new("Warning panel unavailable")); }
            prop(a,background,"Alpha","Core.FloatProperty",4)?;
            read_boolean(a,background,"bTransparent")?;
            model.caption = Some(Caption { text:string(a,receiver,"DisplayText")?,at:(0.,16.),size:(0.,0.),font:Font::Slot(0) });
        }
        Ok(Warning { model, fonts, background, saved_font, transition:flash(show,time) })
    }
}
unsafe fn draw_warning(a: CoreAccess<'_>, receiver: *mut UObject, canvas: *mut UObject, mut warning: Warning) -> RuntimeResult<()> {
    // SAFETY: complete branch preflight preceded these inseparable presentation assignments.
    unsafe {
        if warning.transition.reset {
            boolean(a,receiver,"bShow",warning.transition.show)?;
            set_f32(a,receiver,"fFlashTime",warning.transition.time)?;
        }
        if warning.transition.draw {
            set_f32(a,warning.background,"Alpha",0.5)?; boolean(a,warning.background,"bTransparent",true)?;
            let target = Target(CanvasTarget { canvas, tile_style:None, fonts:warning.fonts });
            let caption = warning.model.caption.as_mut().expect("visible warning has caption");
            for slot in 0..3 {
                caption.font = Font::Slot(slot); caption.size = target.0.text_size(a,&caption.text,caption.font)?;
                if caption.size.0 <= warning.model.size.0.wrapping_sub(32) as f32 { break; }
            }
            let x = (warning.model.size.0 / 2) as f32 - caption.size.0 / 2.; caption.at = (x,16.);
            warning.model.pieces.push(Piece { texture:TextureRef(format!("#{}",(*warning.background).index)),dest:Rect::new(x-8.,8.,caption.size.0+16.,caption.size.1+16.),source:Rect::new(0.,0.,1.,1.) });
            let metrics = HostMetrics::default(); metrics.learn(&caption.text,caption.font,caption.size);
            paint(a,receiver,&target,warning.model,metrics)?;
        }
        prop(a,canvas,"Font","Core.ObjectProperty",size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(warning.saved_font);
        Ok(())
    }
}
unsafe fn callback(a: CoreAccess<'_>, receiver:*mut UObject, function:*mut UFunction, params:*const u8, size:usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !live(a,receiver) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: Core supplies live invocation metadata; unsupported routes remain script-owned before effects.
    unsafe {
        let Some(canvas) = param_object(a,function,params,size,"Canvas") else { return Ok(ScriptPreCallDecision::Continue); };
        if !live(a,canvas) { return Ok(ScriptPreCallDecision::Continue); }
        let draw = a.find_object_function_by_name(receiver,a.intern_str("Draw",FindName::Find)?,false)?;
        if draw != function { return Ok(ScriptPreCallDecision::Continue); }
        let page = find_function(a,"HGame.baseSpellPage","Core.Class","Draw")?;
        let warning = find_function(a,"HGame.baseWarning","Core.Class","Draw")?;
        let canvas_ready=(||->RuntimeResult<()>{
            if !is_a(a,canvas,a.native_class("Engine.Canvas")?){return Err(RuntimeError::new("Popup Canvas unsupported"));}
            for name in ["CurX","CurY","CurYL"]{f32_of(a,canvas,name)?;}
            prop(a,canvas,"Style","Core.ByteProperty",1)?;
            object(a,canvas,"Font")?;color(a,canvas)?;Ok(())
        })();
        if let Err(error)=canvas_ready {super::warn(a,&error);return Ok(ScriptPreCallDecision::Continue);}
        if function == page {
            let prepared = page_preflight(a,receiver,canvas);
            let width = match prepared { Ok(v)=>v, Err(e)=>{ super::warn(a,&e); return Ok(ScriptPreCallDecision::Continue); } };
            draw_page(a,receiver,canvas,width)?;
        } else if function == warning {
            let prepared = warning_snapshot(a,receiver,canvas);
            let prepared = match prepared { Ok(v)=>v, Err(e)=>{ super::warn(a,&e); return Ok(ScriptPreCallDecision::Continue); } };
            draw_warning(a,receiver,canvas,prepared)?;
        } else { return Ok(ScriptPreCallDecision::Continue); }
        Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a:CoreAccess<'_>) -> RuntimeResult<Option<impl Drop+'_>> {
    // SAFETY: the guard retains canonical metadata and is installed only by ui_hud.
    unsafe {
        if !super::enabled() { return Ok(None); }
        let mut functions = Vec::with_capacity(2);
        for class in ["HGame.baseSpellPage","HGame.baseWarning"] { let f=find_function(a,class,"Core.Class","Draw")?; if !f.is_null() { functions.push(f); } }
        if functions.is_empty() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-popups",&functions,callback)?))
    }
}
#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(page_rects((801,601),301,511),[(145.,300.),(401.,300.),(145.,556.),(401.,556.)]);
    assert_eq!(flash(false,0.5),Flash{show:false,time:0.5,draw:false,reset:false});
    assert_eq!(flash(false,0.5001),Flash{show:true,time:0.,draw:false,reset:true});
    assert_eq!(flash(true,1.0001),Flash{show:true,time:0.,draw:true,reset:true});
    assert_eq!(flash(true,1.),Flash{show:true,time:1.,draw:true,reset:false});
    let transition=flash(false,0.5001);
    assert!(!transition.draw,"the visibility assignment must not paint in its own branch");
    assert!(flash(transition.show,transition.time).draw,"the next invocation observes the assignment");
    let model=Popup{size:(800,600),pieces:vec![Piece{texture:TextureRef("panel".into()),dest:Rect::new(20.,8.,100.,30.),source:Rect::new(0.,0.,1.,1.)}],caption:None,color:[3,7,11,13]};
    let mut ui=Ui::new(Store::new(),move||view(&model),HostMetrics::default());ui.resize(800.,600.);ui.relayout();
    assert!(ui.draw().iter().any(|cmd|matches!(cmd,hp2_ui::DrawCmd::Image{color:Color(3,7,11,13),..})),"panel preserves incoming tint");
}
