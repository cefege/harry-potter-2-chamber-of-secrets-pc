//! The complete retail vendor bar; transaction, purchase and input wrappers stay in script.
use std::{cell::RefCell,ptr};
use hp2_abi::engine::generated::{TArrayRaw,UFunction,UObject};
use hp2_core::{ScriptPreCallDecision,core_access::{CoreAccess,RuntimeError,RuntimeResult},name_owner::FindName};
use hp2_ui::{BlendMode,Color,Fit,Font,Node,Rect,Signal,Size,Store,TextureRef,Ui,col,image,label};
use super::{CanvasTarget,find_function,int_of,param_object,player_fonts};
use crate::{stock_graphics_menu::{Weak,field,is_a,live,object,prop},ui_host::{DrawTarget,HostMetrics,find_class,render_commands},ui_vm::{V,invoke_virtual}};

#[derive(Clone,Debug,PartialEq)]
struct Icon { texture:TextureRef,rect:Rect,source:Rect }
#[derive(Clone,Debug,PartialEq)]
struct Text { text:String,at:(f32,f32),size:(f32,f32),color:[u8;4],shadow:[u8;4] }
#[derive(Clone,Debug,PartialEq)]
struct Vendor { size:(i32,i32),icons:Vec<Icon>,text:Option<Text>,tint:[u8;4] }
struct Retained { owner:Weak,model:Signal<Vendor>,ui:Ui,metrics:HostMetrics }
thread_local! {static VIEWS:RefCell<Vec<Retained>>=const{RefCell::new(Vec::new())};}
fn view(model:&Vendor)->Node {
    let rgba=|c:[u8;4]|Color(c[0],c[1],c[2],c[3]);let mut children=Vec::with_capacity(model.icons.len()+2);
    for icon in &model.icons {children.push(image(icon.texture.clone(),Fit::Region(icon.source.x,icon.source.y,icon.source.w,icon.source.h)).color(rgba(model.tint)).at(icon.rect.x,icon.rect.y).width(Size::Px(icon.rect.w)).height(Size::Px(icon.rect.h)));}
    if let Some(text)=&model.text {
        children.push(label(&text.text).font(Font::Slot(0)).color(rgba(text.shadow)).at(text.at.0+1.,text.at.1+1.).width(Size::Px(text.size.0)).height(Size::Px(text.size.1)));
        children.push(label(&text.text).font(Font::Slot(0)).color(rgba(text.color)).at(text.at.0,text.at.1).width(Size::Px(text.size.0)).height(Size::Px(text.size.1)));
    }
    col().children(children)
}
struct Target(CanvasTarget);
impl DrawTarget for Target {
    unsafe fn fill(&self,_:CoreAccess<'_>,_:Rect,_:[u8;4])->RuntimeResult<()> {Err(RuntimeError::new("Unexpected vendor fill"))}
    unsafe fn texture_object(&self,a:CoreAccess<'_>,name:&str)->RuntimeResult<*mut UObject> {unsafe{self.0.texture_object(a,name)}}
    unsafe fn segment(&self,a:CoreAccess<'_>,texture:*mut UObject,dest:Rect,source:Rect,color:[u8;4],blend:BlendMode)->RuntimeResult<()> {unsafe{self.0.segment(a,texture,dest,source,color,blend)}}
    unsafe fn text_at(&self,a:CoreAccess<'_>,x:f32,y:f32,text:&str,font:Font,color:[u8;4])->RuntimeResult<()> {
        // SAFETY: canonical DrawShadowText defaults CR to true and inherits Style. Its two draws
        // reset only X/Y between shadow and foreground, leaving CurYL/native text effects intact.
        unsafe {self.0.set_font(a,font)?;self.0.set_color(a,color)?;self.0.set_pos(a,x,y)?;
            let units:Vec<u16>=text.encode_utf16().chain([0]).collect();crate::canvas_owner::draw_text(a,self.0.canvas.cast(),&units,true,0)}
    }
}
unsafe fn paint(a:CoreAccess<'_>,receiver:*mut UObject,target:&Target,model:Vendor)->RuntimeResult<()> {
    unsafe {
        let mut retained=VIEWS.with(|views|{let mut views=views.borrow_mut();views.retain(|v|v.owner.live(a));views.iter().position(|v|v.owner.p==receiver).map(|i|views.swap_remove(i))}).unwrap_or_else(||{
            let store=Store::new();let signal=store.signal(model.clone());let observed=signal.clone();let metrics=HostMetrics::default();
            Retained{owner:Weak::new(receiver),model:signal,ui:Ui::new(store,move||view(&observed.get()),metrics.clone()),metrics}
        });
        if let Some(text)=&model.text {retained.metrics.learn(&text.text,Font::Slot(0),text.size);}
        retained.ui.resize(model.size.0 as f32,model.size.1 as f32);retained.model.set(model);retained.ui.relayout();
        let result=render_commands(target,a,&retained.ui.draw(),&retained.metrics);
        if retained.owner.live(a){VIEWS.with(|views|views.borrow_mut().push(retained));}result
    }
}
unsafe fn resolved(a:CoreAccess<'_>,receiver:*mut UObject,name:&str)->RuntimeResult<*mut UFunction> {
    unsafe {
        if !live(a,receiver){return Err(RuntimeError::new("Vendor callback receiver destroyed"));}
        let f=a.find_object_function_by_name(receiver,a.intern_str(name,FindName::Find)?,false)?;
        if f.is_null(){Err(RuntimeError::new("Vendor callback unavailable"))}else{Ok(f)}
    }
}
unsafe fn string(a:CoreAccess<'_>,receiver:*mut UObject,name:&str)->RuntimeResult<String> {
    unsafe {
        let raw=prop(a,receiver,name,"Core.StrProperty",size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned();
        if raw.num<0||raw.max<raw.num||(raw.num>0&&raw.data.is_null()){return Err(RuntimeError::new("Invalid vendor string"));}
        if raw.num==0{return Ok(String::new());}let units=std::slice::from_raw_parts(raw.data.cast::<u32>(),raw.num as usize);
        if units.last()!=Some(&0){return Err(RuntimeError::new("Unterminated vendor string"));}
        units[..units.len()-1].iter().copied().map(char::from_u32).collect::<Option<_>>().ok_or_else(||RuntimeError::new("Invalid vendor Unicode"))
    }
}
unsafe fn tint(a:CoreAccess<'_>,canvas:*mut UObject)->RuntimeResult<[u8;4]> {unsafe{Ok(prop(a,canvas,"DrawColor","Core.StructProperty",4)?.cast::<[u8;4]>().read_unaligned())}}
unsafe fn manager(a:CoreAccess<'_>,receiver:*mut UObject)->RuntimeResult<*mut UObject> {
    unsafe {let level=object(a,receiver,"Level")?;if !live(a,level){return Err(RuntimeError::new("Vendor level unavailable"));}
        let harry=object(a,level,"PlayerHarryActor")?;if !live(a,harry){return Err(RuntimeError::new("Vendor player unavailable"));}
        let manager=object(a,harry,"managerStatus")?;if !live(a,manager){return Err(RuntimeError::new("Vendor status manager unavailable"));}Ok(manager)}
}
unsafe fn lookup_layout(a:CoreAccess<'_>,manager:*mut UObject)->RuntimeResult<(*mut UFunction,[usize;2])> {
    unsafe {
        let f=resolved(a,manager,"GetStatusItem")?;let size=usize::from((*f).parms_size);if size>128{return Err(RuntimeError::new("Vendor lookup parameter bounds"));}
        field(a,f.cast(),"ReturnValue","Core.ObjectProperty",size_of::<*mut UObject>(),size,true)?;
        Ok((f,[field(a,f.cast(),"classGroup","Core.ClassProperty",size_of::<*mut UObject>(),size,true)?,field(a,f.cast(),"classItem","Core.ClassProperty",size_of::<*mut UObject>(),size,true)?]))
    }
}
unsafe fn preflight(a:CoreAccess<'_>,receiver:*mut UObject,canvas:*mut UObject)->RuntimeResult<()> {
    unsafe {
        prop(a,receiver,"VendorCanvas","Core.ObjectProperty",size_of::<*mut UObject>())?;object(a,canvas,"Font")?;tint(a,canvas)?;
        if !is_a(a,canvas,a.native_class("Engine.Canvas")?){return Err(RuntimeError::new("Vendor Canvas unsupported"));}
        for name in ["SizeX","SizeY"]{int_of(a,canvas,name)?;}
        for name in ["CurX","CurY","CurYL"]{super::f32_of(a,canvas,name)?;}
        prop(a,canvas,"Style","Core.ByteProperty",1)?;
        for name in ["textureVendorBarLeft","textureVendorBarRight","textureItemToSell","textureVendorButtonNormal","textureVendorButtonOver"] {
            let texture=object(a,receiver,name)?;if !texture.is_null(){if !live(a,texture){return Err(RuntimeError::new("Vendor artwork unavailable"));}int_of(a,texture,"USize")?;int_of(a,texture,"VSize")?;}
        }
        string(a,receiver,"strYes")?;string(a,receiver,"strNo")?;int_of(a,receiver,"nCurrPrice")?;
        for name in ["IsMouseOverVendorYes","IsMouseOverVendorNo"]{resolved(a,receiver,name)?;}
        for name in ["GetVendorBarX","GetVendorBarY"]{let f=resolved(a,receiver,name)?;field(a,f.cast(),"Canvas","Core.ObjectProperty",size_of::<*mut UObject>(),usize::from((*f).parms_size),true)?;}
        resolved(a,canvas,"GetHudScaleFactor")?;
        let expected=find_function(a,"Engine.Canvas","Core.Class","DrawShadowText")?;
        if expected.is_null()||resolved(a,canvas,"DrawShadowText")?!=expected{return Err(RuntimeError::new("Custom vendor Canvas shadow drawing"));}
        let icon=find_function(a,"Engine.Canvas","Core.Class","DrawIcon")?;
        if icon.is_null()||resolved(a,canvas,"DrawIcon")?!=icon{return Err(RuntimeError::new("Custom vendor Canvas icon drawing"));}
        lookup_layout(a,manager(a,receiver)?)?;
        for class in ["HGame.StatusGroupStars","HGame.StatusItemStars"]{if find_class(a,class)?.is_none(){return Err(RuntimeError::new("Vendor price class unavailable"));}}
        let level=object(a,receiver,"Level")?;
        if !live(a,level){return Err(RuntimeError::new("Vendor level unavailable"));}
        let harry=object(a,level,"PlayerHarryActor")?;
        if !live(a,harry){return Err(RuntimeError::new("Vendor player unavailable"));}
        let player=object(a,harry,"Player")?;
        if !live(a,player){return Err(RuntimeError::new("Vendor viewport unavailable"));}
        let console=object(a,player,"Console")?;
        if !live(a,console){return Err(RuntimeError::new("Vendor console unavailable"));}
        let fonts=player_fonts(a,harry)?;
        if fonts[..3].iter().any(|font|!live(a,*font)){return Err(RuntimeError::new("Vendor font unavailable"));}Ok(())
    }
}
fn icon_positions(x:f32,y:f32,scale:f32)->[(f32,f32);5] {[(x,y),(x+256.*scale,y),(x+155.*scale,y+14.*scale),(x+246.*scale,y+20.*scale),(x+246.*scale,y+54.*scale)]}
fn text_position(x:f32,y:f32,scale:f32,index:usize,size:(f32,f32))->(f32,f32) {
    match index {0=>((x+246.*scale)+46.*scale-size.0/2.,(y+20.*scale)+12.*scale-size.1/2.),1=>((x+246.*scale)+46.*scale-size.0/2.,(y+54.*scale)+12.*scale-size.1/2.),_ =>(x+76.*scale-size.0/2.,y+74.*scale-size.1/2.)}
}
unsafe fn draw(a:CoreAccess<'_>,receiver:*mut UObject,canvas:*mut UObject)->RuntimeResult<()> {
    // SAFETY: admission completed before the first presentation assignment. Errors from here
    // propagate; replaying stock would duplicate lookup/callback effects and previously drawn phases.
    unsafe {
        prop(a,receiver,"VendorCanvas","Core.ObjectProperty",size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(canvas);
        let saved_font=object(a,canvas,"Font")?;
        let mut scale=0f32;invoke_virtual(a,canvas,"GetHudScaleFactor",&[],&[],(&raw mut scale).cast())?;scale*=crate::display_options::hud_scale(a);
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during scale callback"));}
        let mut yes=0u32;invoke_virtual(a,receiver,"IsMouseOverVendorYes",&[],&[],(&raw mut yes).cast())?;
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during Yes hover callback"));}
        let mut no=0u32;if yes==0 {invoke_virtual(a,receiver,"IsMouseOverVendorNo",&[],&[],(&raw mut no).cast())?;}
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during hover callback"));}
        // These two are locals chosen before position callbacks; retain their identities across them.
        let normal=object(a,receiver,"textureVendorButtonNormal")?;let over=object(a,receiver,"textureVendorButtonOver")?;
        let yes_texture=if yes!=0{over}else{normal};let no_texture=if yes==0&&no!=0{over}else{normal};
        let mut x=0f32;invoke_virtual(a,receiver,"GetVendorBarX",&[("Canvas",V::Obj(canvas))],&[],(&raw mut x).cast())?;
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during X callback"));}
        let mut y=0f32;invoke_virtual(a,receiver,"GetVendorBarY",&[("Canvas",V::Obj(canvas))],&[],(&raw mut y).cast())?;
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during position callback"));}
        let viewport=(int_of(a,canvas,"SizeX")?,int_of(a,canvas,"SizeY")?);
        let positions=icon_positions(x,y,scale);
        let target=Target(CanvasTarget{canvas,tile_style:None,fonts:[ptr::null_mut();4]});
        for i in 0..5 {
            if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during artwork callback"));}
            // Only button identities are locals. Artwork fields are read after each prior draw.
            target.0.set_pos(a,positions[i].0,positions[i].1)?;
            let texture=match i {
                0=>object(a,receiver,"textureVendorBarLeft")?,
                1=>object(a,receiver,"textureVendorBarRight")?,
                2=>object(a,receiver,"textureItemToSell")?,
                3=>yes_texture,
                _=>no_texture,
            };
            if texture.is_null(){continue;}if !live(a,texture){return Err(RuntimeError::new("Vendor artwork destroyed during callback"));}
            let w=int_of(a,texture,"USize")? as f32;let h=int_of(a,texture,"VSize")? as f32;
            let icon=Icon{texture:TextureRef(format!("#{}",(*texture).index)),rect:Rect::new(positions[i].0,positions[i].1,w*scale,h*scale),source:Rect::new(0.,0.,w,h)};
            paint(a,receiver,&target,Vendor{size:viewport,icons:vec![icon],text:None,tint:tint(a,canvas)?})?;
        }
        // Inventory lookup is intentionally AFTER all five images, including its creation effects.
        let manager=manager(a,receiver)?;let (lookup,slots)=lookup_layout(a,manager)?;let mut params=[0u64;16];let base=params.as_mut_ptr().cast::<u8>();
        let group=find_class(a,"HGame.StatusGroupStars")?.ok_or_else(||RuntimeError::new("Vendor group class destroyed"))?;
        let item_class=find_class(a,"HGame.StatusItemStars")?.ok_or_else(||RuntimeError::new("Vendor item class destroyed"))?;
        base.add(slots[0]).cast::<*mut UObject>().write_unaligned(group.cast());base.add(slots[1]).cast::<*mut UObject>().write_unaligned(item_class.cast());
        let mut item=ptr::null_mut::<UObject>();a.execute_event(manager,lookup,base.cast(),(&raw mut item).cast())?;
        if !live(a,item)||!live(a,canvas){return Err(RuntimeError::new("Vendor price item unavailable after lookup"));}
        let font_function=resolved(a,item,"GetCountFont")?;
        field(a,font_function.cast(),"Canvas","Core.ObjectProperty",size_of::<*mut UObject>(),usize::from((*font_function).parms_size),true)?;
        field(a,font_function.cast(),"ReturnValue","Core.ObjectProperty",size_of::<*mut UObject>(),usize::from((*font_function).parms_size),true)?;
        let mut font=ptr::null_mut::<UObject>();invoke_virtual(a,item,"GetCountFont",&[("Canvas",V::Obj(canvas))],&[],(&raw mut font).cast())?;
        if !live(a,item)||!live(a,canvas)||!live(a,font){return Err(RuntimeError::new("Vendor price font unavailable after callback"));}
        prop(a,canvas,"Font","Core.ObjectProperty",size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(font);
        let color_function=resolved(a,item,"GetCountColor")?;
        field(a,color_function.cast(),"ReturnValue","Core.StructProperty",4,usize::from((*color_function).parms_size),true)?;
        let mut foreground=[0u8;4];invoke_virtual(a,item,"GetCountColor",&[],&[],(&raw mut foreground).cast())?;
        if !live(a,item)||!live(a,canvas){return Err(RuntimeError::new("Vendor price item destroyed during foreground callback"));}
        let color_function=resolved(a,item,"GetCountColor")?;
        field(a,color_function.cast(),"ReturnValue","Core.StructProperty",4,usize::from((*color_function).parms_size),true)?;
        let mut shadow=[0u8;4];invoke_virtual(a,item,"GetCountColor",&[("bShadow",V::Bool(true))],&[],(&raw mut shadow).cast())?;
        if !live(a,receiver)||!live(a,canvas){return Err(RuntimeError::new("Vendor destroyed during color callback"));}
        // Color callbacks may replace the Canvas font; retail text uses the current one.
        let font=object(a,canvas,"Font")?;if !live(a,font){return Err(RuntimeError::new("Vendor Canvas font unavailable"));}
        let target=Target(CanvasTarget{canvas,tile_style:None,fonts:[font;4]});
        for index in 0..3 {
            let text=match index {0=>string(a,receiver,"strYes")?,1=>string(a,receiver,"strNo")?,_=>int_of(a,receiver,"nCurrPrice")?.to_string()};
            let size=target.0.text_size(a,&text,Font::Slot(0))?;let at=text_position(x,y,scale,index,size);let saved_color=tint(a,canvas)?;
            paint(a,receiver,&target,Vendor{size:viewport,icons:Vec::new(),text:Some(Text{text,at,size,color:foreground,shadow}),tint:saved_color})?;
            target.0.set_color(a,saved_color)?;
        }
        prop(a,canvas,"Font","Core.ObjectProperty",size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(saved_font);Ok(())
    }
}
unsafe fn callback(a:CoreAccess<'_>,receiver:*mut UObject,function:*mut UFunction,params:*const u8,size:usize)->RuntimeResult<ScriptPreCallDecision> {
    if !live(a,receiver){return Ok(ScriptPreCallDecision::Continue);}
    unsafe {
        let Some(class)=find_class(a,"HGame.VendorManager")? else{return Ok(ScriptPreCallDecision::Continue);};if !is_a(a,receiver,class.cast())||resolved(a,receiver,"DrawVendorBar")?!=function{return Ok(ScriptPreCallDecision::Continue);}
        let Some(canvas)=param_object(a,function,params,size,"Canvas") else{return Ok(ScriptPreCallDecision::Continue);};if !live(a,canvas){return Ok(ScriptPreCallDecision::Continue);}
        if let Err(e)=preflight(a,receiver,canvas){super::warn(a,&e);return Ok(ScriptPreCallDecision::Continue);}
        draw(a,receiver,canvas)?;Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a:CoreAccess<'_>)->RuntimeResult<Option<impl Drop+'_>> {
    unsafe {if !super::enabled(){return Ok(None);}let f=find_function(a,"HGame.VendorManager","Core.Class","DrawVendorBar")?;if f.is_null(){return Ok(None);}Ok(Some(a.install_script_pre_observer("ui-vendor",&[f],callback)?))}
}
#[cfg(test)]
pub(super) fn assert_view(){
    assert_eq!(icon_positions(10.,20.,1.25),[(10.,20.),(330.,20.),(203.75,37.5),(317.5,45.),(317.5,87.5)]);
    assert_eq!(text_position(10.,20.,1.25,0,(40.,10.)),(355.,55.));
    assert_eq!(text_position(10.,20.,1.25,2,(40.,10.)),(85.,107.5));
    let model=Vendor{size:(800,600),icons:Vec::new(),text:Some(Text{text:"Yes".into(),at:(4.,8.),size:(24.,12.),color:[1,2,3,4],shadow:[5,6,7,8]}),tint:[255;4]};
    let mut ui=Ui::new(Store::new(),move||view(&model),HostMetrics::default());ui.resize(800.,600.);ui.relayout();assert_eq!(ui.draw().iter().filter(|c|matches!(c,hp2_ui::DrawCmd::Text{..})).count(),2);
}
