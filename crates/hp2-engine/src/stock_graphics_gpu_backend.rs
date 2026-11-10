//! Engine-thread adapter for the advanced graphics GPU transaction.
use hp2_core::core_access::CoreAccess;
use crate::{settings_config, settings_policy::SettingsPatch, stock_graphics_gpu::{Backend, Value, VSync}};

pub(crate) struct Live<'a> {
    access: CoreAccess<'a>,
    #[cfg(feature = "renderer-wgpu")]
    original: Option<crate::render_device_owner::LiveGraphicsSnapshot>,
}
impl<'a> Live<'a> {
    pub(crate) fn new(access: CoreAccess<'a>) -> Self { Self {access, #[cfg(feature = "renderer-wgpu")] original: None} }
    /// Show the effective value, with a visible warning when a saved request
    /// fell back. Do not use these labels as transaction or rollback values.
    pub(crate) fn display(&mut self, kind: Value) -> Result<String, String> {
        #[cfg(feature = "renderer-wgpu")]
        {
            let s = unsafe { crate::render_device_owner::global_live_graphics_snapshot(self.access) }.map_err(|e| format!("{e}"))?;
            match kind {
                Value::Msaa(_) => Ok(msaa_label(s.requested_msaa, s.msaa, s.msaa_observed, s.msaa_unavailable, s.startup_msaa_fallback)),
                Value::Anisotropy(_) => Ok(preference_label(anisotropy_label(s.anisotropy), s.requested_anisotropy, s.anisotropy, s.startup_anisotropy_fallback)),
                Value::VSync(_) => {
                    let actual=s.vsync_mode.ok_or("Surface present mode is unknown")?;
                    Ok(preference_label(vsync_label(actual), s.requested_vsync, actual, s.startup_vsync_fallback))
                }
            }
        }
        #[cfg(not(feature = "renderer-wgpu"))]
        { let _ = kind; Err("WGPU renderer is unavailable".into()) }
    }
    pub(crate) fn msaa_display(&mut self) -> Result<String, String> { self.display(Value::Msaa(0)) }
    pub(crate) fn requested(&mut self, kind: Value) -> Result<Value, String> {
        #[cfg(feature = "renderer-wgpu")]
        {
            let s=unsafe {crate::render_device_owner::global_live_graphics_snapshot(self.access)}.map_err(|e|format!("{e}"))?;
            match kind {
                Value::Msaa(_)=>Ok(Value::Msaa(u8::try_from(s.requested_msaa).map_err(|_|"Invalid requested MSAA")?)),
                Value::Anisotropy(_)=>Ok(Value::Anisotropy(u8::try_from(s.requested_anisotropy).map_err(|_|"Invalid requested anisotropy")?)),
                Value::VSync(_)=>match s.requested_vsync {0=>Ok(Value::VSync(VSync::Off)),1=>Ok(Value::VSync(VSync::On)),2=>Ok(Value::VSync(VSync::Adaptive)),_=>Err("Invalid requested VSync".into())},
            }
        }
        #[cfg(not(feature = "renderer-wgpu"))]
        {let _=kind;Err("WGPU renderer is unavailable".into())}
    }
}
fn label(effective:&str, fallback:bool)->String {if fallback {format!("{effective} (fallback)")} else {effective.to_owned()}}
fn preference_label<T:PartialEq>(text:&str, requested:T, effective:T, fallback:bool)->String {label(text,fallback || requested!=effective)}
fn anisotropy_label(value:u16)->&'static str {match value {0=>"Off",4=>"4x",8=>"8x",16=>"16x",_=>"Custom"}}
fn vsync_label(value:u8)->&'static str {match value {0=>"Off",1=>"On",2=>"Adaptive",_=>"Custom"}}
fn msaa_label(requested:i32, configured:i32, observed:bool, unavailable:bool, fallback:bool)->String {
    // A zero-sample configuration is already known to be off. A nonzero
    // configuration remains pending until a world scope establishes usage.
    if configured!=0 && !observed {return label("Pending",fallback || requested!=configured);}
    // The per-frame flag aggregates all scopes. One or more scopes fell
    // back, but other scopes may still have used MSAA.
    if configured!=0 && unavailable {return "Partial/off!".into();}
    let text=match configured {0=>"Off",2=>"2x",4=>"4x",_=>"Custom"};
    label(text, fallback || requested!=configured)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn labels_show_effective_values_and_fallbacks() {
        assert_eq!(msaa_label(4,0,false,false,true),"Off (fallback)");
        assert_eq!(msaa_label(4,4,false,false,false),"Pending");
        assert_eq!(msaa_label(4,4,true,true,false),"Partial/off!");
        assert_eq!(msaa_label(0,0,false,false,false),"Off");
        assert_eq!(msaa_label(2,2,true,false,false),"2x");
        assert_eq!(msaa_label(4,4,true,false,false),"4x");
        assert_eq!(msaa_label(8,8,true,false,false),"Custom");
        assert_eq!(preference_label(anisotropy_label(0),16,0,false),"Off (fallback)");
        assert_eq!(preference_label(vsync_label(1),0,1,false),"On (fallback)");
        assert_eq!(preference_label(vsync_label(1),1,1,true),"On (fallback)");
        assert_eq!(preference_label(vsync_label(1),1,1,false),"On");
    }
}
impl Backend for Live<'_> {
    type Error = String;
    fn current(&mut self, kind: Value) -> Result<Value, String> {
        #[cfg(feature = "renderer-wgpu")]
        {
            let snapshot=unsafe {crate::render_device_owner::global_live_graphics_snapshot(self.access)}.map_err(|e|format!("{e}"))?;
            if self.original.is_none() {self.original=Some(snapshot);}
            match kind {
                Value::Anisotropy(_) => Ok(Value::Anisotropy(u8::try_from(snapshot.anisotropy).map_err(|_|"Invalid live anisotropy")?)),
                Value::Msaa(_) => Ok(Value::Msaa(u8::try_from(snapshot.msaa).map_err(|_|"Invalid live MSAA")?)),
                Value::VSync(_) => match snapshot.vsync_mode {Some(0)=>Ok(Value::VSync(VSync::Off)),Some(1)=>Ok(Value::VSync(VSync::On)),Some(2)=>Ok(Value::VSync(VSync::Adaptive)),_=>Err("Surface present mode is unknown".into())},
            }
        }
        #[cfg(not(feature = "renderer-wgpu"))]
        { let _ = kind; Err("WGPU renderer is unavailable".into()) }
    }
    fn supports(&mut self, _requested: Value) -> Result<bool, String> {
        // The renderer validates hardware support before publishing. A rejected
        // request is returned to the user and never persisted.
        Ok(true)
    }
    fn apply(&mut self, value: Value) -> Result<(), String> {
        #[cfg(feature = "renderer-wgpu")]
        {
            match value {
                Value::Anisotropy(v) => unsafe {crate::render_device_owner::set_global_anisotropy(self.access,u16::from(v))}.map_err(|e|format!("{e}")),
                Value::Msaa(v) => unsafe {crate::render_device_owner::set_global_msaa(self.access,i32::from(v))}.map_err(|e|format!("{e}")),
                Value::VSync(v) => unsafe {crate::render_device_owner::set_global_vsync_mode(self.access,match v {VSync::Off=>0,VSync::On=>1,VSync::Adaptive=>2})}.map_err(|e|format!("{e}")),
            }
        }
        #[cfg(not(feature = "renderer-wgpu"))]
        { let _ = value; Err("WGPU renderer is unavailable".into()) }
    }
    fn restore(&mut self, value: Value) -> Result<(), String> {
        #[cfg(feature = "renderer-wgpu")]
        {
            let old=self.original.ok_or("GPU snapshot unavailable")?;
            match value {
                Value::Anisotropy(_) => unsafe {crate::render_device_owner::restore_global_anisotropy(self.access,old)},
                Value::Msaa(_) => unsafe {crate::render_device_owner::restore_global_msaa(self.access,old)},
                Value::VSync(_) => unsafe {crate::render_device_owner::restore_global_vsync(self.access,old)},
            }.map_err(|e|format!("{e}"))
        }
        #[cfg(not(feature = "renderer-wgpu"))]
        { let _ = value; Err("WGPU renderer is unavailable".into()) }
    }
    fn committed(&mut self, value: Value) -> Result<(), String> {
        #[cfg(feature = "renderer-wgpu")]
        { unsafe { crate::render_device_owner::commit_global_graphics_preference(self.access, match value {Value::Msaa(_)=>0,Value::Anisotropy(_)=>1,Value::VSync(_)=>2}) }.map_err(|e|format!("{e}")) }
        #[cfg(not(feature = "renderer-wgpu"))]
        { let _ = value; Err("WGPU renderer is unavailable".into()) }
    }
    fn persist(&mut self, value: Value) -> Result<(), String> {
        let patch = match value {
            Value::Anisotropy(v) => SettingsPatch {anisotropy:Some(i32::from(v)),..Default::default()},
            Value::Msaa(v) => SettingsPatch {anti_aliasing_samples:Some(i32::from(v)),..Default::default()},
            Value::VSync(VSync::Off) => SettingsPatch {vertical_sync:Some(false),..Default::default()},
            Value::VSync(VSync::On) => SettingsPatch {vertical_sync:Some(true),..Default::default()},
            Value::VSync(VSync::Adaptive) => return Err("Adaptive is a rollback-only value".into()),
        };
        settings_config::apply(self.access,&patch).map(|_|()).map_err(|e|format!("{e:?}"))
    }
}
