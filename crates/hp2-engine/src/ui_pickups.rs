//! Flying pickups retain the canonical live-actor Canvas raster at their script composition slot.
use std::ptr;
use hp2_abi::engine::generated::{FSceneNode, UCanvas, UFunction, UObject, UViewport};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeResult}, name_owner::FindName};
use super::{find_function, param_object};
use crate::{stock_graphics_menu::{field, is_a, live}, ui_host::find_class};

unsafe fn callback(a:CoreAccess<'_>,receiver:*mut UObject,function:*mut UFunction,params:*const u8,size:usize)->RuntimeResult<ScriptPreCallDecision> {
    if !live(a,receiver) {return Ok(ScriptPreCallDecision::Continue);}
    // SAFETY: admission checks the actual route, reflected arguments and live native capabilities
    // before the DrawActor call can change visibility, clear Z, or submit actor geometry.
    unsafe {
        let Some(class)=find_class(a,"HGame.HProp")? else{return Ok(ScriptPreCallDecision::Continue);};
        if !is_a(a,receiver,class.cast()) {return Ok(ScriptPreCallDecision::Continue);}
        let actual=a.find_object_function_by_name(receiver,a.intern_str("RenderHud",FindName::Find)?,false)?;
        if actual!=function {return Ok(ScriptPreCallDecision::Continue);}
        let Some(canvas)=param_object(a,function,params,size,"Canvas") else{return Ok(ScriptPreCallDecision::Continue);};
        if !live(a,canvas){return Ok(ScriptPreCallDecision::Continue);}
        if !is_a(a,canvas,a.native_class("Engine.Canvas")?) {return Ok(ScriptPreCallDecision::Continue);}
        let draw=find_function(a,"Engine.Canvas","Core.Class","DrawActor")?;
        if draw.is_null() {return Ok(ScriptPreCallDecision::Continue);}
        if a.find_object_function_by_name(canvas,a.intern_str("DrawActor",FindName::Find)?,false)?!=draw {return Ok(ScriptPreCallDecision::Continue);}
        let prepared=(||->RuntimeResult<bool>{
            let bytes=usize::from((*draw).parms_size);
            field(a,draw.cast(),"A","Core.ObjectProperty",size_of::<*mut UObject>(),bytes,true)?;
            for name in ["WireFrame","ClearZ"] {
                field(a,draw.cast(),name,"Core.BoolProperty",4,bytes,true)?;
                let p=a.find_field(draw.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.BoolProperty")?).cast::<hp2_abi::engine::generated::UBoolProperty>();
                if p.is_null()||(*p).bit_mask.count_ones()!=1 {return Ok(false);}
            }
            let c=canvas.cast::<UCanvas>();let viewport=ptr::addr_of!((*c).viewport).read_unaligned().cast::<UViewport>();
            let scene=ptr::addr_of!((*c).frame).read_unaligned().cast::<FSceneNode>();
            if ptr::addr_of!((*c).render).read_unaligned().is_null()||scene.is_null()||!live(a,viewport.cast()) {return Ok(false);}
            let actor=ptr::addr_of!((*viewport).actor).read_unaligned();
            Ok(live(a,actor.cast())&&!ptr::addr_of!((*viewport).ren_dev).read_unaligned().is_null())
        })();
        match prepared {Ok(true)=>{},Ok(false)=>return Ok(ScriptPreCallDecision::Continue),Err(e)=>{super::warn(a,&e);return Ok(ScriptPreCallDecision::Continue);}}
        // Native DrawActor owns its exact temporary hidden/render-mode transitions, Z clear and
        // failure behavior. Do not restore visibility on errors or substitute a texture proxy.
        crate::canvas_owner::draw_actor(a,canvas.cast(),receiver.cast(),false,true)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a:CoreAccess<'_>)->RuntimeResult<Option<impl Drop+'_>> {
    // SAFETY: ui_hud retains this observer guard for the canonical HProp drawing leaf only.
    unsafe {
        if !super::enabled(){return Ok(None);}let f=find_function(a,"HGame.HProp","Core.Class","RenderHud")?;
        if f.is_null(){return Ok(None);}Ok(Some(a.install_script_pre_observer("ui-pickups",&[f],callback)?))
    }
}
