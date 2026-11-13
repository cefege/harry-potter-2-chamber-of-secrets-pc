//! Whole linked-group layout and counter views; transitions, gameplay queries and color policy stay script-owned.
use std::{cell::RefCell, ptr};
use hp2_abi::engine::generated::{UBoolProperty, UFunction, UObject};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Color, Font, Node, Rect, Signal, Size, Store, Ui, col, label};
use super::{CanvasTarget, find_function, int_of, param_object};
use crate::{stock_graphics_menu::{Weak, field, is_a, live, object, prop, read_boolean}, ui_host::{DrawTarget, HostMetrics, find_class, render_commands}, ui_vm::{V, invoke_virtual}};

const FAMILIES: [&str;10] = ["HGame.StatusGroupHealth","HGame.StatusGroupJellybeans","HGame.StatusGroupLocks","HGame.StatusGroupQGear","HGame.StatusGroupStars","HGame.StatusGroupWizardCards","HGame.StatusGroupHousePoints","HGame.StatusGroupPolyIngr","HGame.StatusGroupPotionIngr","HGame.StatusGroupPotions"];
unsafe fn resolved(a:CoreAccess<'_>, receiver:*mut UObject, name:&str)->RuntimeResult<*mut UFunction> {
    unsafe {
        if !live(a,receiver) { return Err(RuntimeError::new("Status receiver destroyed during callback")); }
        let f=a.find_object_function_by_name(receiver,a.intern_str(name,FindName::Find)?,false)?;
        if f.is_null() { Err(RuntimeError::new("Status callback unavailable")) } else { Ok(f) }
    }
}
unsafe fn arg(a:CoreAccess<'_>, f:*mut UFunction,name:&str,kind:&str,width:usize)->RuntimeResult<usize> {
    unsafe { field(a,f.cast(),name,kind,width,usize::from((*f).parms_size),true) }
}
unsafe fn bool_arg(a:CoreAccess<'_>, f:*mut UFunction,name:&str)->RuntimeResult<(usize,u32)> {
    unsafe {
        let offset=arg(a,f,name,"Core.BoolProperty",4)?;
        let p=a.find_field(f.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.BoolProperty")?).cast::<UBoolProperty>();
        if p.is_null() || (*p).bit_mask.count_ones()!=1 {return Err(RuntimeError::new("Invalid status boolean argument"));}
        Ok((offset,(*p).bit_mask))
    }
}
unsafe fn item_layout(a:CoreAccess<'_>, f:*mut UFunction,specified:bool)->RuntimeResult<[usize;5]> {
    unsafe {
        if (*f).parms_size as usize>128 {return Err(RuntimeError::new("Status drawing parameter bounds"));}
        Ok([arg(a,f,"Canvas","Core.ObjectProperty",size_of::<*mut UObject>())?,arg(a,f,"nCurrX","Core.IntProperty",4)?,arg(a,f,"nCurrY","Core.IntProperty",4)?,arg(a,f,"fScaleFactor","Core.FloatProperty",4)?,if specified {arg(a,f,"nLocalCount","Core.IntProperty",4)?} else {0}])
    }
}
unsafe fn item_frame(a:CoreAccess<'_>,f:*mut UFunction,canvas:*mut UObject,x:i32,y:i32,scale:f32,count:Option<i32>)->RuntimeResult<[u64;16]> {
    unsafe {
        let offsets=item_layout(a,f,count.is_some())?;let mut frame=[0u64;16];let base=frame.as_mut_ptr().cast::<u8>();
        base.add(offsets[0]).cast::<*mut UObject>().write_unaligned(canvas);base.add(offsets[1]).cast::<i32>().write_unaligned(x);base.add(offsets[2]).cast::<i32>().write_unaligned(y);base.add(offsets[3]).cast::<f32>().write_unaligned(scale);
        if let Some(count)=count {base.add(offsets[4]).cast::<i32>().write_unaligned(count);}
        Ok(frame)
    }
}
unsafe fn counter_preflight(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject)->RuntimeResult<()> {
    unsafe {
        if !live(a,item)||!live(a,canvas) {return Err(RuntimeError::new("Status counter unavailable"));}
        if !is_a(a,canvas,a.native_class("Engine.Canvas")?){return Err(RuntimeError::new("Status Canvas unsupported"));}
        let shadow=find_function(a,"Engine.Canvas","Core.Class","DrawShadowText")?;
        if shadow.is_null()||resolved(a,canvas,"DrawShadowText")?!=shadow{return Err(RuntimeError::new("Custom status Canvas shadow drawing"));}
        for name in ["nCount","nMaxCount","nCountMiddleX","nCountMiddleY"] {int_of(a,item,name)?;}
        read_boolean(a,item,"bDisplayMaxCount")?;
        arg(a,resolved(a,item,"GetCountFont")?,"Canvas","Core.ObjectProperty",size_of::<*mut UObject>())?;
        arg(a,resolved(a,item,"GetCountFont")?,"ReturnValue","Core.ObjectProperty",size_of::<*mut UObject>())?;
        bool_arg(a,resolved(a,item,"GetCountColor")?,"bShadow")?;
        arg(a,resolved(a,item,"GetCountColor")?,"ReturnValue","Core.StructProperty",4)?;
        object(a,canvas,"Font")?;prop(a,canvas,"DrawColor","Core.StructProperty",4)?;prop(a,canvas,"Style","Core.ByteProperty",1)?;
        for name in ["CurX","CurY","CurYL"] {super::f32_of(a,canvas,name)?;}
        int_of(a,canvas,"SizeX")?;int_of(a,canvas,"SizeY")?;
        Ok(())
    }
}
/// Admission is effect-free; callers execute unsupported child script exactly once.
pub(super) unsafe fn admits_specified_count(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject)->RuntimeResult<bool> {
    unsafe {
        if !live(a,item)||!live(a,canvas) {return Ok(false);}
        let expected=find_function(a,"HGame.StatusItem","Core.Class","DrawSpecifiedCount")?;
        if expected.is_null()||resolved(a,item,"DrawSpecifiedCount")?!=expected {return Ok(false);}
        item_layout(a,expected,true)?;counter_preflight(a,item,canvas)?;Ok(true)
    }
}
/// Admission for both ordinary inherited counters and the separately owned health view.
pub(super) unsafe fn admits_item(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject)->RuntimeResult<bool> {
    unsafe {
        if !live(a,item)||!live(a,canvas) {return Ok(false);}
        let actual=resolved(a,item,"DrawItem")?;
        let health=find_function(a,"HGame.StatusItemHealth","Core.Class","DrawItem")?;
        if !health.is_null()&&actual==health {return super::player_health::admits(a,item,canvas);}
        let base=find_function(a,"HGame.StatusItem","Core.Class","DrawItem")?;
        if base.is_null()||actual!=base {return Ok(false);}
        item_layout(a,base,false)?;
        let count=find_function(a,"HGame.StatusItem","Core.Class","DrawCount")?;
        if resolved(a,item,"DrawCount")?!=count||!admits_specified_count(a,item,canvas)? {return Ok(false);}
        let icon=find_function(a,"Engine.Canvas","Core.Class","DrawIcon")?;
        if icon.is_null()||resolved(a,canvas,"DrawIcon")?!=icon{return Ok(false);}
        let texture=object(a,item,"textureHudIcon")?;
        if !texture.is_null() {if !live(a,texture) {return Ok(false);}int_of(a,texture,"USize")?;int_of(a,texture,"VSize")?;}
        read_boolean(a,item,"bDisplayCount")?;Ok(true)
    }
}
#[derive(Clone,Debug,PartialEq)]
struct CountView { text:String,at:(f32,f32),size:(f32,f32),color:[u8;4],shadow:[u8;4] }
struct Retained { owner:Weak, model:Signal<CountView>,ui:Ui,metrics:HostMetrics }
thread_local! {static COUNTS:RefCell<Vec<Retained>>=const{RefCell::new(Vec::new())};}
fn count_view(model:&CountView)->Node {
    let color=|v:[u8;4]|Color(v[0],v[1],v[2],v[3]);
    col().children([
        label(&model.text).font(Font::Slot(0)).color(color(model.shadow)).at(model.at.0+1.,model.at.1+1.).width(Size::Px(model.size.0)).height(Size::Px(model.size.1)),
        label(&model.text).font(Font::Slot(0)).color(color(model.color)).at(model.at.0,model.at.1).width(Size::Px(model.size.0)).height(Size::Px(model.size.1)),
    ])
}
struct CountTarget(CanvasTarget);
impl DrawTarget for CountTarget {
    unsafe fn fill(&self,_:CoreAccess<'_>,_:Rect,_:[u8;4])->RuntimeResult<()> {Err(RuntimeError::new("Unexpected count fill"))}
    unsafe fn texture_object(&self,a:CoreAccess<'_>,name:&str)->RuntimeResult<*mut UObject> {unsafe{self.0.texture_object(a,name)}}
    unsafe fn segment(&self,a:CoreAccess<'_>,texture:*mut UObject,dest:Rect,source:Rect,color:[u8;4],blend:BlendMode)->RuntimeResult<()> {unsafe{self.0.segment(a,texture,dest,source,color,blend)}}
    unsafe fn text_at(&self,a:CoreAccess<'_>,x:f32,y:f32,text:&str,font:Font,color:[u8;4])->RuntimeResult<()> {
        // SAFETY: these are the two canonical DrawShadowText calls, including default CR=true.
        unsafe {
            self.0.set_font(a,font)?;self.0.set_color(a,color)?;self.0.set_pos(a,x,y)?;
            let units:Vec<u16>=text.encode_utf16().chain([0]).collect();
            crate::canvas_owner::draw_text(a,self.0.canvas.cast(),&units,true,0)
        }
    }
}
fn count_anchor(x:i32,y:i32,anchor:(i32,i32),scale:f32,text:(f32,f32),viewport:(i32,i32))->(f32,f32) {
    let mut x=x as f32+anchor.0 as f32*scale-text.0/2.;let mut y=y as f32+anchor.1 as f32*scale-text.1/2.;
    if x+text.0>viewport.0 as f32 {x=viewport.0 as f32-text.0-2.;}
    if y+text.1>viewport.1 as f32 {y=viewport.1 as f32-text.1-2.;}
    (x,y)
}
/// Paint a supplied count without temporarily mutating the gameplay-owned nCount.
pub(super) unsafe fn draw_specified_count(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject,x:i32,y:i32,scale:f32,count:i32)->RuntimeResult<()> {
    // SAFETY: callers preflight admission; every callback boundary revalidates receiver and resources.
    unsafe {
        counter_preflight(a,item,canvas)?;
        let saved_font=object(a,canvas,"Font")?;
        let mut text=count.to_string();if read_boolean(a,item,"bDisplayMaxCount")? {text=format!("{text}/{}",int_of(a,item,"nMaxCount")?);}
        let mut font=ptr::null_mut::<UObject>();invoke_virtual(a,item,"GetCountFont",&[("Canvas",V::Obj(canvas))],&[],(&raw mut font).cast())?;
        if !live(a,item)||!live(a,canvas)||!live(a,font) {return Err(RuntimeError::new("Status count font destroyed during callback"));}
        let target=CanvasTarget{canvas,tile_style:None,fonts:[font;4]};
        let text_size=target.text_size(a,&text,Font::Slot(0))?;
        let at=count_anchor(x,y,(int_of(a,item,"nCountMiddleX")?,int_of(a,item,"nCountMiddleY")?),scale,text_size,(int_of(a,canvas,"SizeX")?,int_of(a,canvas,"SizeY")?));
        // GetCountColor calls belong after measurement and position, not in an eager snapshot.
        target.set_pos(a,at.0,at.1)?;
        arg(a,resolved(a,item,"GetCountColor")?,"ReturnValue","Core.StructProperty",4)?;
        let mut color=[0u8;4];invoke_virtual(a,item,"GetCountColor",&[],&[],(&raw mut color).cast())?;
        if !live(a,item)||!live(a,canvas) {return Err(RuntimeError::new("Status counter destroyed during color callback"));}
        arg(a,resolved(a,item,"GetCountColor")?,"ReturnValue","Core.StructProperty",4)?;
        let mut script_shadow=[0u8;4];invoke_virtual(a,item,"GetCountColor",&[("bShadow",V::Bool(true))],&[],(&raw mut script_shadow).cast())?;
        if !live(a,item)||!live(a,canvas) {return Err(RuntimeError::new("Status counter resource destroyed during callback"));}
        let font=object(a,canvas,"Font")?;
        if !live(a,font) {return Err(RuntimeError::new("Status Canvas font destroyed during color callback"));}
        let target=CountTarget(CanvasTarget{canvas,tile_style:None,fonts:[font;4]});
        let saved_color=prop(a,canvas,"DrawColor","Core.StructProperty",4)?.cast::<[u8;4]>().read_unaligned();
        // Preserve the existing product's contrast shadow policy while retaining both original callbacks.
        let luminance=u32::from(color[0])*3+u32::from(color[1])*6+u32::from(color[2]);
        let shadow=if luminance>1280 {[0,0,0,255]} else {[255,255,255,160]};
        let at=(super::f32_of(a,canvas,"CurX")?,super::f32_of(a,canvas,"CurY")?);
        let model=CountView{text,at,size:text_size,color:[color[0],color[1],color[2],255],shadow};
        let mut retained=COUNTS.with(|views|{let mut views=views.borrow_mut();views.retain(|v|v.owner.live(a));views.iter().position(|v|v.owner.p==item).map(|i|views.swap_remove(i))}).unwrap_or_else(||{
            let store=Store::new();let signal=store.signal(model.clone());let observed=signal.clone();let metrics=HostMetrics::default();
            Retained{owner:Weak::new(item),model:signal,ui:Ui::new(store,move||count_view(&observed.get()),metrics.clone()),metrics}
        });
        retained.metrics.learn(&model.text,Font::Slot(0),text_size);retained.model.set(model);retained.ui.resize(int_of(a,canvas,"SizeX")? as f32,int_of(a,canvas,"SizeY")? as f32);retained.ui.relayout();
        let result=render_commands(&target,a,&retained.ui.draw(),&retained.metrics);
        if retained.owner.live(a) {COUNTS.with(|views|views.borrow_mut().push(retained));}result?;
        target.0.set_color(a,saved_color)?;
        prop(a,canvas,"Font","Core.ObjectProperty",size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(saved_font);Ok(())
    }
}
/// Direct composition entry: a consumed parent must not rely on nested observers.
pub(super) unsafe fn draw_item(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject,x:i32,y:i32,scale:f32)->RuntimeResult<()> {
    unsafe {
        let f=resolved(a,item,"DrawItem")?;
        if f==find_function(a,"HGame.StatusItemHealth","Core.Class","DrawItem")? {return super::player_health::draw(a,item,canvas,scale);}
        let target=CanvasTarget{canvas,tile_style:None,fonts:[ptr::null_mut();4]};target.set_pos(a,x as f32,y as f32)?;
        let texture=object(a,item,"textureHudIcon")?;
        if !texture.is_null() {
            if !live(a,texture) {return Err(RuntimeError::new("Status icon unavailable after callback"));}
            let w=int_of(a,texture,"USize")? as f32;let h=int_of(a,texture,"VSize")? as f32;
            target.tile(a,texture,hp2_ui::Rect::new(x as f32,y as f32,w*scale,h*scale),hp2_ui::Rect::new(0.,0.,w,h))?;
        }
        if read_boolean(a,item,"bDisplayCount")? {draw_specified_count(a,item,canvas,x,y,scale,int_of(a,item,"nCount")?)?;}
        Ok(())
    }
}
unsafe fn child(a:CoreAccess<'_>,item:*mut UObject,canvas:*mut UObject,x:i32,y:i32,scale:f32)->RuntimeResult<()> {
    unsafe {
        let f=resolved(a,item,"DrawItem")?;
        if admits_item(a,item,canvas)? {draw_item(a,item,canvas,x,y,scale)} else {
            let mut frame=item_frame(a,f,canvas,x,y,scale,None)?;a.execute_event(item,f,frame.as_mut_ptr().cast(),ptr::null_mut())
        }
    }
}
unsafe fn icon_size(a:CoreAccess<'_>,item:*mut UObject,horizontal:bool)->RuntimeResult<i32> {
    unsafe {
        let name=if horizontal{"GetHudIconUSize"}else{"GetHudIconVSize"};
        arg(a,resolved(a,item,name)?,"ReturnValue","Core.IntProperty",4)?;
        let mut size=0i32;invoke_virtual(a,item,name,&[],&[],(&raw mut size).cast())?;
        if !live(a,item){return Err(RuntimeError::new("Status icon receiver destroyed during dimension callback"));}
        Ok(size)
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum PositionMode { Final, FlyIn, FlyOut }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct LayoutRoute { family:usize, mode:PositionMode }
unsafe fn layout_route(a:CoreAccess<'_>,group:*mut UObject)->RuntimeResult<Option<LayoutRoute>> {
    unsafe {
        let mut family=None;
        for (i,name) in FAMILIES.into_iter().enumerate(){
            if let Some(class)=find_class(a,name)? {if is_a(a,group,class.cast()){family=Some(i);break;}}
        }
        let Some(family)=family else{return Ok(None);};
        let actual=resolved(a,group,"GetGroupCurrXY")?;
        let mut mode=None;
        for (owner,kind,value) in [
            ("HGame.StatusGroup","Core.Class",PositionMode::Final),
            ("HGame.StatusGroup.EffectIn","Core.State",PositionMode::FlyIn),
            ("HGame.StatusGroup.EffectOut","Core.State",PositionMode::FlyOut),
        ] {
            let expected=find_function(a,owner,kind,"GetGroupCurrXY")?;
            if !expected.is_null()&&actual==expected {mode=Some(value);break;}
        }
        let Some(mode)=mode else{return Ok(None);};
        for (owner,name) in [
            (FAMILIES[family],"GetGroupFinalXY_2"),
            (FAMILIES[family],"GetGroupFlyOriginXY"),
            (if family==0{"HGame.StatusGroup"}else{FAMILIES[family]},"GetGroupFinalXY"),
            ("HGame.StatusGroup","CalcFlyXY"),
            ("HGame.StatusGroup","GetScaleFactor"),
            ("HGame.StatusGroup","GetTimeRatio"),
        ] {
            let expected=find_function(a,owner,"Core.Class",name)?;
            if expected.is_null()||resolved(a,group,name)?!=expected{return Ok(None);}
        }
        Ok(Some(LayoutRoute{family,mode}))
    }
}
unsafe fn group_player(a:CoreAccess<'_>,group:*mut UObject)->RuntimeResult<*mut UObject> {
    unsafe {
        if !live(a,group){return Err(RuntimeError::new("Status group destroyed during layout query"));}
        let manager=object(a,group,"smParent")?;
        if !live(a,manager){return Err(RuntimeError::new("Status parent unavailable"));}
        let player=object(a,manager,"PlayerHarry")?;
        if !live(a,player){return Err(RuntimeError::new("Status player unavailable"));}
        Ok(player)
    }
}
unsafe fn objective_page(a:CoreAccess<'_>,group:*mut UObject)->RuntimeResult<*mut UObject> {
    unsafe {
        let player=object(a,group_player(a,group)?,"Player")?;
        if !live(a,player){return Err(RuntimeError::new("Status viewport unavailable"));}
        let console=object(a,player,"Console")?;
        if !live(a,console){return Err(RuntimeError::new("Status console unavailable"));}
        let book=object(a,console,"menuBook")?;
        if !live(a,book){return Err(RuntimeError::new("Status book unavailable"));}
        let page=object(a,book,"InGamePage")?;
        if !live(a,page){return Err(RuntimeError::new("Status objective page unavailable"));}
        Ok(page)
    }
}
unsafe fn cut_manager(a:CoreAccess<'_>,group:*mut UObject)->RuntimeResult<*mut UObject> {
    unsafe {
        let hud=object(a,group_player(a,group)?,"myHUD")?;
        if !live(a,hud){return Err(RuntimeError::new("Status HUD unavailable"));}
        let manager=object(a,hud,"managerCutScene")?;
        if !live(a,manager){return Err(RuntimeError::new("Status cutscene manager unavailable"));}
        Ok(manager)
    }
}
/// Final-coordinate arithmetic takes query results, never gameplay policy or mutable actor state.
fn final_coordinates(family:usize,size:(i32,i32),icon:(i32,i32),objective_top:Option<i32>,cut_border:i32)->(i32,i32) {
    let scale=size.0 as f32/640.;
    match family {
        0=>(0,0),
        1=>((scale*4.) as i32,(scale*70.) as i32),
        2=>((scale*4.) as i32,(scale*290.) as i32),
        3=>((scale*4.) as i32,(scale*140.) as i32),
        4=>((490.*scale) as i32,(scale*4.) as i32),
        5=>((scale*416.) as i32,(scale*4.) as i32),
        6=>(((size.0 as f32-scale*icon.0 as f32)-scale*4.) as i32,(scale*4.) as i32),
        7=>(((size.0 as f32-scale*icon.0 as f32)-scale*4.) as i32,(scale*166.) as i32),
        8|9=>{
            let x=if family==8{(scale*4.) as i32}else{((size.0 as f32-icon.0 as f32*scale)-scale*4.) as i32};
            let y=if let Some(top)=objective_top {
                ((top as f32-icon.1 as f32*scale)-5.*scale) as i32
            }else{((size.1 as f32-icon.1 as f32*scale)-cut_border as f32) as i32};
            (x,y)
        },
        _=>unreachable!("only admitted status layout families"),
    }
}
unsafe fn final_position(a:CoreAccess<'_>,group:*mut UObject,canvas:*mut UObject,family:usize,menu:bool,icon:(i32,i32))->RuntimeResult<(i32,i32)> {
    unsafe {
        if !live(a,canvas){return Err(RuntimeError::new("Status Canvas destroyed during layout query"));}
        // GetGroupFinalXY captures both dimensions as arguments before calling its concrete leaf.
        let size=(int_of(a,canvas,"SizeX")?,int_of(a,canvas,"SizeY")?);
        let mut top=None;let mut border=0;
        if family>=8 {
            let mut have_objective=0u32;
            if menu {invoke_virtual(a,group_player(a,group)?,"HaveObjectiveText",&[],&[],(&raw mut have_objective).cast())?;}
            if menu&&have_objective!=0 {
                let page=objective_page(a,group)?;
                let mut value=0i32;invoke_virtual(a,page,"GetObjectiveAreaTop",&[("nCanvasSizeX",V::I(size.0)),("nCanvasSizeY",V::I(size.1))],&[],(&raw mut value).cast())?;
                top=Some(value);
            }else{
                let manager=cut_manager(a,group)?;
                let mut value=0f32;invoke_virtual(a,manager,"GetMaxBorderHeightFromCanvasHeight",&[("nCanvasSizeY",V::I(size.1))],&[],(&raw mut value).cast())?;
                border=value as i32;
            }
            if !live(a,group)||!live(a,canvas){return Err(RuntimeError::new("Status receiver destroyed during layout query"));}
        }
        Ok(final_coordinates(family,size,icon,top,border))
    }
}
fn fly_origin(family:usize,final_xy:(i32,i32),height:i32,scale:f32,canvas_height:i32)->(i32,i32) {
    let y=match family {
        0=>height.wrapping_neg(),
        1|4=>(-(scale*height as f32)) as i32,
        3|8|9=>(canvas_height as f32+height as f32*scale) as i32,
        _=>(-(height as f32*scale)) as i32,
    };
    (final_xy.0,y)
}
fn fly_coordinates(final_xy:(i32,i32),origin:(i32,i32),incoming:bool,current:f32,total:f32)->(i32,i32) {
    let ratio=if total<current{1.}else{(total-current)/total};
    let axis=|end:i32,start:i32|{
        if end>start {
            let offset=(end.wrapping_sub(start) as f32*ratio) as i32;
            if incoming{start.wrapping_add(offset)}else{end.wrapping_sub(offset)}
        }else{
            let offset=(start.wrapping_sub(end) as f32*ratio) as i32;
            if incoming{start.wrapping_sub(offset)}else{end.wrapping_add(offset)}
        }
    };
    // Stock calculates Y before X; each integer distance wraps before f32 conversion.
    let y=axis(final_xy.1,origin.1);let x=axis(final_xy.0,origin.0);(x,y)
}
unsafe fn position(a:CoreAccess<'_>,group:*mut UObject,canvas:*mut UObject,menu:bool,width:i32,height:i32)->RuntimeResult<(i32,i32)> {
    unsafe {
        let route=layout_route(a,group)?.ok_or_else(||RuntimeError::new("Status layout became unsupported after callback"))?;
        let icon=(width,height);
        if route.mode==PositionMode::Final||prop(a,group,"CurrEffectType","Core.ByteProperty",1)?.read()!=1 {
            return final_position(a,group,canvas,route.family,menu,icon);
        }
        let incoming=route.mode==PositionMode::FlyIn;
        let current=super::f32_of(a,group,if incoming{"fCurrEffectInTime"}else{"fCurrEffectOutTime"})?;
        let total=super::f32_of(a,group,if incoming{"fTotalEffectInTime"}else{"fTotalEffectOutTime"})?;
        let final_xy=final_position(a,group,canvas,route.family,menu,icon)?;
        if layout_route(a,group)?!=Some(route){return Err(RuntimeError::new("Status layout changed during final-position query"));}
        let origin_scale=if route.family==0{0.}else{int_of(a,canvas,"SizeX")? as f32/640.};
        // Every concrete origin calls final-position again. Objective/cut queries must repeat,
        // even if the preceding callback returned the same visible geometry.
        let origin_final=final_position(a,group,canvas,route.family,menu,icon)?;
        let origin=fly_origin(route.family,origin_final,height,origin_scale,int_of(a,canvas,"SizeY")?);
        Ok(fly_coordinates(final_xy,origin,incoming,current,total))
    }
}
fn visibility(count:i32,display_zero:bool,increment_zero:bool)->(bool,bool) {let draw=count>0||(count==0&&display_zero);(draw,draw||increment_zero)}
fn advance(current:i32,scale:f32,dimension:i32,spacing:i32)->i32 {current.wrapping_add((scale*dimension.wrapping_add(spacing) as f32) as i32)}
unsafe fn preflight_group(a:CoreAccess<'_>,group:*mut UObject,canvas:*mut UObject,menu:bool)->RuntimeResult<bool> {
    unsafe {
        if !is_a(a,canvas,a.native_class("Engine.Canvas")?){return Ok(false);}
        let Some(route)=layout_route(a,group)? else{return Ok(false);};
        prop(a,group,"CurrEffectType","Core.ByteProperty",1)?;
        for name in ["fCurrEffectInTime","fTotalEffectInTime","fCurrEffectOutTime","fTotalEffectOutTime"] {super::f32_of(a,group,name)?;}
        if route.family>=8 {
            let player=group_player(a,group)?;
            if menu {
                let have=resolved(a,player,"HaveObjectiveText")?;arg(a,have,"ReturnValue","Core.BoolProperty",4)?;
                let page=objective_page(a,group)?;let query=resolved(a,page,"GetObjectiveAreaTop")?;
                for name in ["nCanvasSizeX","nCanvasSizeY","ReturnValue"]{arg(a,query,name,"Core.IntProperty",4)?;}
            }
            let manager=cut_manager(a,group)?;let query=resolved(a,manager,"GetMaxBorderHeightFromCanvasHeight")?;
            arg(a,query,"nCanvasSizeY","Core.IntProperty",4)?;arg(a,query,"ReturnValue","Core.FloatProperty",4)?;
        }
        for name in ["bCurrRenderInCutScene","bDisplayHorizontally","bDisplayJustFirstItem"] {read_boolean(a,group,name)?;}
        int_of(a,group,"nSpaceBetweenIcons")?;prop(a,canvas,"DrawColor","Core.StructProperty",4)?;
        let f=resolved(a,group,"GetGroupCurrXY")?;bool_arg(a,f,"bMenuMode")?;
        for name in ["nIconWidth","nIconHeight","nX","nY"] {arg(a,f,name,"Core.IntProperty",4)?;}
        arg(a,f,"Canvas","Core.ObjectProperty",size_of::<*mut UObject>())?;
        arg(a,resolved(a,group,"GetScaleFactor")?,"nCanvasSizeX","Core.IntProperty",4)?;
        bool_arg(a,resolved(a,group,"HandleMenuModeSwitching")?,"bMenuMode")?;arg(a,resolved(a,group,"GetDrawColor")?,"ReturnValue","Core.StructProperty",4)?;
        let mut item=object(a,group,"siList")?;let mut remaining=a.object_count();
        while !item.is_null() {
            if !live(a,item)||remaining<=0 {return Ok(false);}remaining-=1;
            for name in ["bMenuModeOnly","bDisplayWhenCountZero","bIncrementPosWhenCountZero"] {read_boolean(a,item,name)?;}
            int_of(a,item,"nCount")?;item_layout(a,resolved(a,item,"DrawItem")?,false)?;
            // Custom children need only the actual drawing signature: execute them once at their slot.
            let _=admits_item(a,item,canvas)?;resolved(a,item,"GetHudIconUSize")?;resolved(a,item,"GetHudIconVSize")?;item=object(a,item,"siNext")?;
        }
        Ok(true)
    }
}
unsafe fn render_group(a:CoreAccess<'_>,group:*mut UObject,canvas:*mut UObject,menu:bool,full_cut:bool)->RuntimeResult<()> {
    unsafe {
        let scale=int_of(a,canvas,"SizeX")? as f32/640.;
        if menu {return Ok(());}
        if !read_boolean(a,group,"bCurrRenderInCutScene")?&&full_cut {return Ok(());}
        invoke_virtual(a,group,"HandleMenuModeSwitching",&[("bMenuMode",V::Bool(menu))],&[],ptr::null_mut())?;
        if !live(a,group)||!live(a,canvas) {return Err(RuntimeError::new("Status group destroyed during menu callback"));}
        if layout_route(a,group)?.is_none(){return Err(RuntimeError::new("Status layout became unsupported during menu callback"));}
        let saved=prop(a,canvas,"DrawColor","Core.StructProperty",4)?.cast::<[u8;4]>().read_unaligned();
        let mut tint=[0u8;4];invoke_virtual(a,group,"GetDrawColor",&[],&[],(&raw mut tint).cast())?;
        if !live(a,group)||!live(a,canvas) {return Err(RuntimeError::new("Status group destroyed during color callback"));}
        let target=CanvasTarget{canvas,tile_style:None,fonts:[ptr::null_mut();4]};target.set_color(a,tint)?;
        // Read list and positioning policy only AFTER HandleMenuModeSwitching, which may change state/list.
        let mut item=object(a,group,"siList")?;let mut first=true;let(mut x,mut y)=(0,0);
        while !item.is_null() {
            if !live(a,item)||!live(a,group)||!live(a,canvas) {return Err(RuntimeError::new("Status list destroyed during callback"));}
            if menu||!read_boolean(a,item,"bMenuModeOnly")? {
                if first {let width=icon_size(a,item,true)?;let height=icon_size(a,item,false)?;(x,y)=position(a,group,canvas,menu,width,height)?;first=false;}
                if !live(a,item)||!live(a,group)||!live(a,canvas) {return Err(RuntimeError::new("Status item destroyed during position callback"));}
                let count=int_of(a,item,"nCount")?;
                let (draw,increment)=visibility(count,read_boolean(a,item,"bDisplayWhenCountZero")?,read_boolean(a,item,"bIncrementPosWhenCountZero")?);
                if draw {child(a,item,canvas,x,y,scale)?;}
                if !live(a,item)||!live(a,group)||!live(a,canvas) {return Err(RuntimeError::new("Status item destroyed during drawing callback"));}
                if increment {
                    let horizontal=read_boolean(a,group,"bDisplayHorizontally")?;let size=icon_size(a,item,horizontal)?;
                    if !live(a,group)||!live(a,canvas){return Err(RuntimeError::new("Status group destroyed during spacing callback"));}
                    let spacing=int_of(a,group,"nSpaceBetweenIcons")?;if horizontal{x=advance(x,scale,size,spacing);}else{y=advance(y,scale,size,spacing);}
                }
                if read_boolean(a,group,"bDisplayJustFirstItem")? {break;}
            }
            // Callback mutation is visible to traversal: never snapshot next links across DrawItem.
            item=object(a,item,"siNext")?;
        }
        target.set_color(a,saved)
    }
}
unsafe fn callback(a:CoreAccess<'_>,receiver:*mut UObject,f:*mut UFunction,params:*const u8,size:usize)->RuntimeResult<ScriptPreCallDecision> {
    if !live(a,receiver) {return Ok(ScriptPreCallDecision::Continue);}
    unsafe {
        let Some(canvas)=param_object(a,f,params,size,"Canvas") else{return Ok(ScriptPreCallDecision::Continue);};if !live(a,canvas){return Ok(ScriptPreCallDecision::Continue);}
        if resolved(a,receiver,"RenderHudItemManager")?!=f {return Ok(ScriptPreCallDecision::Continue);}
        let prepared=(||->RuntimeResult<_>{
            let(mo,mm)=bool_arg(a,f,"bMenuMode")?;let(co,cm)=bool_arg(a,f,"bFullCutMode")?;
            let menu=params.add(mo).cast::<u32>().read_unaligned()&mm!=0;
            if !preflight_group(a,receiver,canvas,menu)? {return Ok(None);}
            Ok(Some((menu,params.add(co).cast::<u32>().read_unaligned()&cm!=0)))
        })();
        let flags=match prepared{Ok(Some(flags))=>flags,Ok(None)=>return Ok(ScriptPreCallDecision::Continue),Err(e)=>{super::warn(a,&e);return Ok(ScriptPreCallDecision::Continue);}};
        render_group(a,receiver,canvas,flags.0,flags.1)?;Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a:CoreAccess<'_>)->RuntimeResult<Option<impl Drop+'_>> {
    // SAFETY: the guard owns canonical function observation; StatusManager and Idle wrappers stay script.
    unsafe {
        if !super::enabled(){return Ok(None);}let f=find_function(a,"HGame.StatusGroup","Core.Class","RenderHudItemManager")?;if f.is_null(){return Ok(None);}
        Ok(Some(a.install_script_pre_observer("ui-status-groups",&[f],callback)?))
    }
}
#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(visibility(0,false,false),(false,false));assert_eq!(visibility(0,false,true),(false,true));assert_eq!(visibility(0,true,false),(true,true));assert_eq!(visibility(-1,true,false),(false,false));
    assert_eq!(advance(10,1.25,31,4),53);assert_eq!(advance(i32::MAX,1.,1,0),i32::MIN);
    assert_eq!(count_anchor(790,592,(0,0),1.,(30.,20.),(800,600)),(768.,578.));
    assert_eq!(count_anchor(790,590,(0,0),1.,(30.,20.),(800,600)),(768.,580.));
    assert_eq!(count_anchor(-20,-30,(0,0),1.,(30.,20.),(800,600)),(-35.,-40.));
    let expected=[(0,0),(5,87),(5,362),(5,175),(612,5),(520,5),(745,5),(745,207),(5,500),(745,500)];
    for (family,expected) in expected.into_iter().enumerate(){assert_eq!(final_coordinates(family,(800,600),(40,20),None,75),expected,"layout family {family}");}
    assert_eq!(final_coordinates(8,(800,600),(40,20),Some(400),75),(5,368));
    assert_eq!(final_coordinates(9,(800,600),(40,20),Some(400),75),(745,368));
    assert_eq!(fly_origin(0,(5,7),20,1.25,600),(5,-20));
    assert_eq!(fly_origin(3,(5,7),20,1.25,600),(5,625));
    assert_eq!(fly_coordinates((100,100),(0,-100),true,0.25,1.),(75,50));
    assert_eq!(fly_coordinates((100,100),(0,-100),false,0.25,1.),(25,-50));
    let model=CountView{text:"0/10".into(),at:(4.,8.),size:(25.,12.),color:[206,200,190,255],shadow:[0,0,0,255]};
    let mut ui=Ui::new(Store::new(),move||count_view(&model),HostMetrics::default());ui.resize(800.,600.);ui.relayout();
    assert_eq!(ui.draw().iter().filter(|c|matches!(c,hp2_ui::DrawCmd::Text{..})).count(),2,"count and contrast shadow both paint");
    let store=Store::new();let signal=store.signal(CountView{text:"1".into(),at:(4.,8.),size:(25.,12.),color:[1,2,3,255],shadow:[0,0,0,255]});
    let observed=signal.clone();let mut ui=Ui::new(store,move||count_view(&observed.get()),HostMetrics::default());ui.resize(800.,600.);ui.relayout();
    let first=ui.draw();
    signal.set(CountView{text:"2".into(),at:(40.,80.),size:(25.,12.),color:[7,11,13,255],shadow:[0,0,0,255]});ui.relayout();
    let changed=ui.draw();
    assert_ne!(first,changed,"a retained view must observe a changed snapshot");
    assert!(changed.iter().any(|c|matches!(c,hp2_ui::DrawCmd::Text{text,rect,color:Color(7,11,13,255),..} if text=="2"&&rect.x==40.&&rect.y==80.)));
}
