//! Compact advanced graphics selectors on the stock sound/video page.
//! The first stock combo chooses a setting; the second chooses its live value.
//! No retail package is modified. All pointers are revalidated on each callback.
use std::{cell::RefCell, ffi::c_void, ptr};
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UClass, UClient, UConsole, UFunction, UGameEngine, UObject, UProperty, USDLClient, UStruct, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::{stock_graphics_settings::{self, Change}, stock_graphics_gpu::{self, Backend, Value}, stock_graphics_gpu_backend, stock_menu_extensions::{self, PageEvent}};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Setting { FrameLimit, UiScale, VerticalFov, NativeText, ScreenFlashes, VSync, Anisotropy, Msaa }
const SETTINGS: &[(Setting, &str)] = &[(Setting::FrameLimit,"Frame limit"), (Setting::UiScale,"UI scale"), (Setting::VerticalFov,"Vertical FOV"), (Setting::NativeText,"Native text"), (Setting::ScreenFlashes,"Screen flashes"), (Setting::VSync,"VSync"), (Setting::Anisotropy,"Anisotropy"), (Setting::Msaa,"Anti-aliasing")];
// Stock HPMenuOptionComboList has a fixed 89-pixel popup and no scrollbar.
// At most four rows leave enough height for its built-in text drawing.
const PAGES: &[&[(Setting,&str)]] = &[
    &[(Setting::FrameLimit,"Frame limit"),(Setting::UiScale,"UI scale"),(Setting::VerticalFov,"Vertical FOV")],
    &[(Setting::NativeText,"Native text"),(Setting::ScreenFlashes,"Screen flashes"),(Setting::VSync,"VSync")],
    &[(Setting::Anisotropy,"Anisotropy"),(Setting::Msaa,"Anti-aliasing")],
];
const MORE: &str = "More...";
const MORE_VALUES: &str = "More values...";
fn supported(s: Setting) -> bool { match s { Setting::VSync | Setting::Anisotropy | Setting::Msaa => cfg!(feature = "renderer-wgpu"), _ => true } }
fn choices(s: Setting) -> &'static [&'static str] { match s {
    Setting::FrameLimit => &["Unlimited", "30 FPS", "60 FPS", "120 FPS", "144 FPS"],
    Setting::UiScale => &["75%", "100%", "125%", "150%", "175%", "200%"],
    Setting::VSync => &["Off", "On"],
    Setting::Anisotropy => &["Off", "4x", "8x", "16x"],
    Setting::Msaa => &["Off", "2x", "4x"],
    _ => &["Off", "On"],
} }
fn change(s: Setting, text: &str) -> Option<Change> {
    let i = choices(s).iter().position(|v| *v == text)?;
    Some(match s {
        Setting::FrameLimit => Change::FrameLimit([0,30,60,120,144][i]),
        Setting::UiScale => Change::UiScale([75,100,125,150,175,200][i]),
        Setting::VerticalFov => Change::VerticalFov(i == 1),
        Setting::NativeText => Change::NativeText(i == 1),
        Setting::ScreenFlashes => Change::ScreenFlashes(i == 1),
        Setting::VSync | Setting::Anisotropy | Setting::Msaa => return None,
    })
}
fn gpu_change(s:Setting, text:&str)->Option<Value> {
    let i=choices(s).iter().position(|v|*v==text)?;
    match s {Setting::VSync=>Some(Value::VSync(if i==0 {stock_graphics_gpu::VSync::Off} else {stock_graphics_gpu::VSync::On})), Setting::Anisotropy=>Some(Value::Anisotropy([0,4,8,16][i])), Setting::Msaa=>Some(Value::Msaa([0,2,4][i])), _=>None}
}
fn gpu_kind(s:Setting)->Option<Value> {match s {Setting::VSync=>Some(Value::VSync(stock_graphics_gpu::VSync::Off)),Setting::Anisotropy=>Some(Value::Anisotropy(0)),Setting::Msaa=>Some(Value::Msaa(0)),_=>None}}
fn gpu_label(value:Value)->Option<&'static str> { match value {
    Value::Anisotropy(0)|Value::Msaa(0)=>Some("Off"), Value::Anisotropy(4)|Value::Msaa(4)=>Some("4x"),
    Value::Anisotropy(8)=>Some("8x"),Value::Anisotropy(16)=>Some("16x"),Value::Msaa(2)=>Some("2x"),
    Value::VSync(stock_graphics_gpu::VSync::Off)=>Some("Off"), Value::VSync(stock_graphics_gpu::VSync::On)=>Some("On"), Value::VSync(stock_graphics_gpu::VSync::Adaptive)=>Some("Adaptive"),_=>None,
}}
fn wire(s:Setting, label:&str)->Option<&'static str> {match (s,label) {
    (Setting::FrameLimit,"Unlimited")=>Some("0"),(Setting::FrameLimit,"30 FPS")=>Some("30"),(Setting::FrameLimit,"60 FPS")=>Some("60"),(Setting::FrameLimit,"120 FPS")=>Some("120"),(Setting::FrameLimit,"144 FPS")=>Some("144"),
    (Setting::UiScale,"75%")=>Some("75"),(Setting::UiScale,"100%")=>Some("100"),(Setting::UiScale,"125%")=>Some("125"),(Setting::UiScale,"150%")=>Some("150"),(Setting::UiScale,"175%")=>Some("175"),(Setting::UiScale,"200%")=>Some("200"),
    (Setting::VerticalFov|Setting::NativeText|Setting::ScreenFlashes|Setting::VSync,"On")=>Some("on"),
    (Setting::VerticalFov|Setting::NativeText|Setting::ScreenFlashes|Setting::VSync,"Off")=>Some("off"),
    (Setting::VSync,"Adaptive")=>Some("adaptive"),
    (Setting::Anisotropy|Setting::Msaa,"Off")=>Some("0"), (Setting::Anisotropy|Setting::Msaa,"2x")=>Some("2"),
    (Setting::Anisotropy|Setting::Msaa,"4x")=>Some("4"),(Setting::Anisotropy,"8x")=>Some("8"),(Setting::Anisotropy,"16x")=>Some("16"),_=>None,
}}
fn key(s:Setting)->&'static str {match s {Setting::FrameLimit=>"frame_limit",Setting::UiScale=>"ui_scale",Setting::VerticalFov=>"vertical_fov",Setting::NativeText=>"native_text",Setting::ScreenFlashes=>"screen_flashes",Setting::VSync=>"vsync",Setting::Anisotropy=>"anisotropy",Setting::Msaa=>"msaa"}}
fn trace(a:CoreAccess<'_>, event:&str, setting:Setting, requested:&str, actual:&str) {
    let actual=actual.strip_suffix(" (fallback)").unwrap_or(actual);
    if std::env::var_os("HP2_STOCK_GRAPHICS_GPU_TRACE").as_deref()!=Some(std::ffi::OsStr::new("1")){return;}
    if let (Some(requested),Some(actual),Ok(category))=(wire(setting,requested),wire(setting,actual),a.intern_str("Log",FindName::Add)) {
        let prefix=if gpu_kind(setting).is_some(){"StockGraphicsGPU"}else{"StockGraphicsProbe"};
        let message:Vec<u32>=format!("{prefix} event={event} setting={} requested={requested} actual={actual}",key(setting)).chars().map(u32::from).collect();
        let _=a.emit_log(category,&message);
    }
}
#[derive(Clone, Copy)]
struct Weak { p: *mut UObject, index: i32 }
impl Weak {
    unsafe fn new(p: *mut UObject) -> Self { Self { p, index: unsafe { (*p).index } } }
    fn live(self, a: CoreAccess<'_>) -> bool { !self.p.is_null() && self.index >= 0 && a.object_at(self.index) == self.p.cast() }
}
#[derive(Clone, Copy)]
struct Controls { page: Weak, selector: Weak, value: Weak, client: Weak, engine: Weak, selected: Setting, group: usize, value_group: usize, msaa_created_probe: Option<u8>, msaa_applied_probe: Option<u8> }
#[derive(Default)]
struct State { owner: usize, controls: Vec<Controls> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn owner(a: CoreAccess<'_>, state: &mut State) -> RuntimeResult<()> {
    let id = a.owner_context::<crate::core_owner::EngineState>().map(|s| s as *const _ as usize).ok_or_else(|| RuntimeError::new("Graphics menu owner unavailable"))?;
    if state.owner != id { *state = State { owner: id, ..Default::default() }; } Ok(())
}
fn live(a: CoreAccess<'_>, p: *mut UObject) -> bool { !p.is_null() && (0..a.object_count()).any(|i| a.object_at(i) == p.cast()) }
unsafe fn is_a(a: CoreAccess<'_>, p: *mut UObject, class: *mut UClass) -> bool { live(a,p) && unsafe { hp2_core::object_model::is_a(p.cast(),class.cast()) } }
unsafe fn field(a: CoreAccess<'_>, st: *mut UStruct, name: &str, kind: &str, width: usize, size: usize, arg: bool) -> RuntimeResult<usize> {
    let p = unsafe { a.find_field(st,a.intern_str(name,FindName::Find)?,a.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new(&format!("Graphics menu missing {name}"))); }
    let off = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Graphics menu negative offset"))?;
    if unsafe { (*p).array_dim } != 1 || usize::try_from(unsafe { (*p).element_size }).ok() != Some(width) || off.checked_add(width).is_none_or(|n| n > size) || (arg && unsafe { (*p).property_flags } & 0x80 == 0) { return Err(RuntimeError::new(&format!("Graphics menu invalid field {name}"))); }
    Ok(off)
}
unsafe fn prop(a: CoreAccess<'_>, p: *mut UObject, name: &str, kind: &str, width: usize) -> RuntimeResult<*mut u8> {
    let class = unsafe { (*p).class.cast::<UClass>() }; let size = usize::try_from(unsafe { (*class).properties_size }).map_err(|_| RuntimeError::new("Graphics menu class size"))?;
    Ok(unsafe { p.cast::<u8>().add(field(a,class.cast(),name,kind,width,size,false)?) })
}
unsafe fn object(a: CoreAccess<'_>, p: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> { Ok(unsafe { prop(a,p,name,"Core.ObjectProperty",size_of::<*mut c_void>())?.cast::<*mut UObject>().read_unaligned() }) }
unsafe fn boolean(a: CoreAccess<'_>, p: *mut UObject, name: &str, v: bool) -> RuntimeResult<()> {
    let f = unsafe { a.find_field((*p).class.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.BoolProperty")?) }.cast::<UBoolProperty>();
    if f.is_null() || unsafe { (*f).bit_mask }.count_ones() != 1 { return Err(RuntimeError::new("Graphics menu invalid boolean")); }
    let slot = unsafe { prop(a,p,name,"Core.BoolProperty",4)? }.cast::<u32>(); let old = unsafe { slot.read_unaligned() }; let mask = unsafe { (*f).bit_mask };
    unsafe { slot.write_unaligned(if v {old | mask} else {old & !mask}); } Ok(())
}
unsafe fn read_boolean(a: CoreAccess<'_>, p:*mut UObject, name:&str)->RuntimeResult<bool> {
    let f=unsafe {a.find_field((*p).class.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.BoolProperty")?)}.cast::<UBoolProperty>();
    if f.is_null() || unsafe {(*f).bit_mask}.count_ones()!=1 {return Err(RuntimeError::new("Graphics menu invalid boolean"));}
    Ok(unsafe {prop(a,p,name,"Core.BoolProperty",4)?.cast::<u32>().read_unaligned()} & unsafe {(*f).bit_mask} != 0)
}
unsafe fn function(a: CoreAccess<'_>, p: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { a.find_field((*p).class.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { Err(RuntimeError::new(&format!("Graphics menu missing function {name}"))) } else { Ok(f) }
}
enum Arg<'a> { Str(&'a str), Int(i32), Bool(bool) }
unsafe fn call(a: CoreAccess<'_>, p: *mut UObject, name: &str, args: &[(&str,Arg<'_>)], result: *mut c_void) -> RuntimeResult<()> {
    let f = unsafe { function(a,p,name)? }; let size = usize::from(unsafe { (*f).parms_size });
    let mut frame = vec![0u64;(size+7)/8]; let base = frame.as_mut_ptr().cast::<u8>(); let mut offsets=Vec::new();
    for (name,arg) in args { let (kind,width) = match arg { Arg::Str(_) => ("Core.StrProperty",size_of::<TArrayRaw>()), Arg::Int(_) => ("Core.IntProperty",4), Arg::Bool(_) => ("Core.BoolProperty",4) }; offsets.push(unsafe { field(a,f.cast(),name,kind,width,size,true)? }); }
    let mut strings=Vec::new();
    let outcome = (|| -> RuntimeResult<()> {
        for ((name,arg),&off) in args.iter().zip(&offsets) { unsafe { match arg {
            Arg::Str(s) => { let mut v:TArrayRaw=std::mem::zeroed(); a.assign_fstring(&mut v,&s.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?; base.add(off).cast::<TArrayRaw>().write_unaligned(v); strings.push(off); },
            Arg::Int(v) => base.add(off).cast::<i32>().write_unaligned(*v),
            Arg::Bool(v) => { let field=a.find_field(f.cast(),a.intern_str(name,FindName::Find)?,a.native_class("Core.BoolProperty")?).cast::<UBoolProperty>(); let mask=(*field).bit_mask; if mask.count_ones()!=1 { return Err(RuntimeError::new("Graphics menu invalid argument")); } let slot=base.add(off).cast::<u32>(); let old=slot.read_unaligned(); slot.write_unaligned(if *v {old|mask} else {old&!mask}); }
        } } }
        unsafe { a.execute_event(p,f,base.cast(),result) }
    })();
    let mut cleanup=None; for off in strings { let mut v=unsafe { base.add(off).cast::<TArrayRaw>().read_unaligned() }; if let Err(e)=unsafe { a.destroy_owned_fstring(&mut v) } { cleanup=Some(e); } }
    outcome?; if let Some(e)=cleanup { return Err(e); } Ok(())
}
unsafe fn set_text(a: CoreAccess<'_>, combo: *mut UObject, text: &str) -> RuntimeResult<()> {
    let edit = unsafe { object(a,combo,"EditBox")? }; if !live(a,edit) { return Err(RuntimeError::new("Graphics menu edit unavailable")); }
    let p=unsafe { prop(a,edit,"Value","Core.StrProperty",size_of::<TArrayRaw>())? }.cast::<TArrayRaw>(); let mut old=unsafe {p.read_unaligned()};
    let result=unsafe { a.write_vm_fstring_result(&mut old,&text.chars().map(|v|v as i32).collect::<Vec<_>>()) }; unsafe {p.write_unaligned(old)}; result
}
unsafe fn text(a: CoreAccess<'_>, combo: *mut UObject) -> RuntimeResult<String> {
    let edit=unsafe {object(a,combo,"EditBox")?}; if !live(a,edit) { return Err(RuntimeError::new("Graphics menu edit unavailable")); }
    let v=unsafe {prop(a,edit,"Value","Core.StrProperty",size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned()};
    if v.num < 0 || v.max < v.num || v.num > 256 || (v.num>0 && v.data.is_null()) { return Err(RuntimeError::new("Graphics menu invalid value")); }
    if v.num==0 { return Ok(String::new()); }
    let units=unsafe {std::slice::from_raw_parts(v.data.cast::<u32>(),v.num as usize)};
    if units.last()!=Some(&0) || units[..units.len()-1].contains(&0) { return Err(RuntimeError::new("Graphics menu invalid text")); }
    units[..units.len()-1].iter().copied().map(char::from_u32).collect::<Option<_>>().ok_or_else(||RuntimeError::new("Graphics menu invalid Unicode"))
}
unsafe fn objects(a: CoreAccess<'_>, page:*mut UObject) -> RuntimeResult<(*mut UGameEngine,*mut USDLClient)> {
    let root=unsafe {object(a,page,"Root")?}; if !live(a,root) {return Err(RuntimeError::new("Graphics menu root unavailable"));}
    let console=unsafe {object(a,root,"Console")?}; if !unsafe {is_a(a,console,a.native_class("Engine.Console")?)} {return Err(RuntimeError::new("Graphics menu console unavailable"));}
    let vp=unsafe {(*console.cast::<UConsole>()).viewport.cast::<UViewport>()};
    if !unsafe {is_a(a,vp.cast(),a.native_class("Engine.Viewport")?)} || unsafe {(*vp).console.cast::<UObject>()} != console {return Err(RuntimeError::new("Graphics menu viewport unavailable"));}
    let client=unsafe {(*vp).outer.cast::<USDLClient>()}; if !unsafe {is_a(a,client.cast(),a.native_class("SDLDrv.SDLClient")?)} {return Err(RuntimeError::new("Graphics menu SDL client unavailable"));}
    let engine=unsafe {(*client.cast::<UClient>()).engine.cast::<UGameEngine>()};
    if !unsafe {is_a(a,engine.cast(),a.native_class("Engine.GameEngine")?)} || unsafe {(*engine).client} != client.cast() {return Err(RuntimeError::new("Graphics menu engine unavailable"));}
    Ok((engine,client))
}
unsafe fn current(s:Setting, engine:*mut UGameEngine, client:*mut USDLClient) -> Option<&'static str> {
    let index=unsafe {match s {
        Setting::FrameLimit => [0.,30.,60.,120.,144.].iter().position(|v| *v==(*engine).frame_rate_limit),
        Setting::UiScale => {let n=ptr::addr_of!((*client).ui_scale).read_unaligned(); [0.75,1.,1.25,1.5,1.75,2.].iter().position(|v|*v==n)},
        Setting::VerticalFov => Some(usize::from((*client.cast::<UClient>()).maintain_vertical_fov())),
        Setting::NativeText => Some(usize::from((*client.cast::<UClient>()).native_text())),
        Setting::ScreenFlashes => Some(usize::from((*client.cast::<UClient>()).screen_flashes())),
        Setting::VSync | Setting::Anisotropy | Setting::Msaa => None,
    }}; index.map(|i|choices(s)[i])
}
unsafe fn shown_value(a:CoreAccess<'_>, c:&Controls)->String {
    if let Some(kind)=gpu_kind(c.selected) {return stock_graphics_gpu_backend::Live::new(a).display(kind).unwrap_or_else(|_|"Unavailable".into());}
    unsafe {current(c.selected,c.engine.p.cast(),c.client.p.cast())}.unwrap_or(if supported(c.selected) {"Custom"} else {"Unavailable"}).into()
}
unsafe fn populate_selector(a:CoreAccess<'_>, c:&Controls)->RuntimeResult<()> {
    let combo=c.selector.p; unsafe {call(a,combo,"Clear",&[],ptr::null_mut())?;}
    for &(_,name) in PAGES[c.group] {unsafe {call(a,combo,"AddItem",&[("S",Arg::Str(name))],ptr::null_mut())?;}}
    unsafe {call(a,combo,"AddItem",&[("S",Arg::Str(MORE))],ptr::null_mut())?;}
    let label=SETTINGS.iter().find(|(s,_)|*s==c.selected).ok_or_else(||RuntimeError::new("Graphics selector state invalid"))?.1;
    unsafe {set_text(a,combo,label)}
}
unsafe fn populate(a: CoreAccess<'_>, c:&Controls) -> RuntimeResult<()> {
    let combo=c.value.p; unsafe {call(a,combo,"Clear",&[],ptr::null_mut())?;}
    let all=choices(c.selected); let chunks=all.len().div_ceil(3);
    if chunks>0 {
        for item in all.chunks(3).nth(c.value_group.min(chunks-1)).unwrap() {unsafe {call(a,combo,"AddItem",&[("S",Arg::Str(item))],ptr::null_mut())?;}}
        if chunks>1 {unsafe {call(a,combo,"AddItem",&[("S",Arg::Str(MORE_VALUES))],ptr::null_mut())?;}}
    }
    let value=unsafe {shown_value(a,c)};
    unsafe {boolean(a,combo,"bAcceptsFocus",supported(c.selected))?; set_text(a,combo,&value)}
}
unsafe fn create_combo(a:CoreAccess<'_>,page:*mut UObject,class:*mut UClass,y:f32,label:&str) -> RuntimeResult<*mut UObject> {
    let f=unsafe {function(a,page,"CreateControl")?}; let size=usize::from(unsafe {(*f).parms_size}); let mut frame=vec![0u64;(size+7)/8]; let p=frame.as_mut_ptr().cast::<u8>();
    let arg=|n,k,w|unsafe {field(a,f.cast(),n,k,w,size,true)};
    let cl=arg("ControlClass","Core.ClassProperty",size_of::<*mut c_void>())?; let x=arg("X","Core.FloatProperty",4)?; let yy=arg("Y","Core.FloatProperty",4)?; let w=arg("W","Core.FloatProperty",4)?; let h=arg("H","Core.FloatProperty",4)?;
    arg("ReturnValue","Core.ObjectProperty",size_of::<*mut c_void>())?;
    unsafe {p.add(cl).cast::<*mut UClass>().write_unaligned(class); for(off,v)in[(x,360f32),(yy,y),(w,260f32),(h,24f32)] {p.add(off).cast::<f32>().write_unaligned(v);}}
    let mut combo:*mut UObject=ptr::null_mut(); unsafe {a.execute_event(page,f,p.cast(),(&raw mut combo).cast())?;}
    if !unsafe {is_a(a,combo,class)} {return Err(RuntimeError::new("Graphics menu control creation failed"));}
    unsafe {boolean(a,combo,"bWindowVisible",false)?; boolean(a,combo,"bAcceptsFocus",false)?;
        call(a,combo,"SetText",&[("NewText",Arg::Str(label))],ptr::null_mut())?;
        call(a,combo,"SetEditable",&[("bNewCanEdit",Arg::Bool(false))],ptr::null_mut())?;}
    let source=unsafe {object(a,page,"ResolutionCombo")?}; if !unsafe {is_a(a,source,class)} {return Err(RuntimeError::new("Graphics menu source unavailable"));}
    let font=unsafe {prop(a,source,"Font","Core.IntProperty",4)?.cast::<i32>().read_unaligned()};
    unsafe {call(a,combo,"SetFont",&[("NewFont",Arg::Int(font))],ptr::null_mut())?;
        let color=prop(a,source,"TextColor","Core.StructProperty",4)?.cast::<u32>().read_unaligned(); prop(a,combo,"TextColor","Core.StructProperty",4)?.cast::<u32>().write_unaligned(color);
        prop(a,combo,"EditBoxWidth","Core.FloatProperty",4)?.cast::<f32>().write_unaligned(150.);
        let edit=object(a,combo,"EditBox")?; let source_edit=object(a,source,"EditBox")?;
        if !is_a(a,edit,a.native_class("UWindow.UWindowEditBox")?) || !live(a,source_edit) {return Err(RuntimeError::new("Graphics menu edit unavailable"));}
        let color=prop(a,source_edit,"TextColor","Core.StructProperty",4)?.cast::<u32>().read_unaligned(); prop(a,edit,"TextColor","Core.StructProperty",4)?.cast::<u32>().write_unaligned(color);
        for (name,v) in [("WinLeft",110f32),("WinTop",0.),("WinWidth",150.),("WinHeight",24.)] {prop(a,edit,name,"Core.FloatProperty",4)?.cast::<f32>().write_unaligned(v);}
    } Ok(combo)
}
// Insert both selectors before Back, after validating the complete circular tab chain.
unsafe fn place_before_back(a:CoreAccess<'_>,page:*mut UObject,controls:[*mut UObject;2])->RuntimeResult<()> {
    let back=unsafe {object(a,page,"BackPageButton")?}; let last=unsafe {object(a,page,"TabLast")?}; let dialog=a.native_class("UWindow.UWindowDialogControl")?;
    let valid=|p|unsafe {is_a(a,p,dialog)};
    if !valid(back)||!valid(last){return Err(RuntimeError::new("Graphics menu tab chain unavailable"));}
    let first=unsafe {object(a,last,"TabNext")?}; let mut order=Vec::new(); let mut p=first;
    loop {if !valid(p)||order.len()>=4096||unsafe {object(a,p,"NotifyWindow")?}!=page{return Err(RuntimeError::new("Graphics menu invalid tab chain"));}
        if order.contains(&p){if p!=first{return Err(RuntimeError::new("Graphics menu invalid tab cycle"));}break;} order.push(p); p=unsafe {object(a,p,"TabNext")?};}
    if order.last().copied()!=Some(last){return Err(RuntimeError::new("Graphics menu invalid tab tail"));}
    for (i,&p) in order.iter().enumerate(){if unsafe {object(a,p,"TabPrev")?}!=order[(i+order.len()-1)%order.len()]{return Err(RuntimeError::new("Graphics menu inconsistent tab links"));}}
    for control in controls {let pos=order.iter().position(|&p|p==control).ok_or_else(||RuntimeError::new("Graphics menu control missing from tab chain"))?; order.remove(pos);}
    let pos=order.iter().position(|&p|p==back).ok_or_else(||RuntimeError::new("Graphics menu back missing from tab chain"))?;
    order.splice(pos..pos,controls);
    let mut slots=Vec::new(); for &p in &order {slots.push((unsafe {prop(a,p,"TabPrev","Core.ObjectProperty",size_of::<*mut c_void>())?},unsafe {prop(a,p,"TabNext","Core.ObjectProperty",size_of::<*mut c_void>())?}));}
    let tail=unsafe {prop(a,page,"TabLast","Core.ObjectProperty",size_of::<*mut c_void>())?};
    for (i,&(prev,next)) in slots.iter().enumerate(){unsafe {prev.cast::<*mut UObject>().write_unaligned(order[(i+order.len()-1)%order.len()]);next.cast::<*mut UObject>().write_unaligned(order[(i+1)%order.len()]);}}
    unsafe {tail.cast::<*mut UObject>().write_unaligned(*order.last().unwrap());} Ok(())
}
unsafe fn create(a:CoreAccess<'_>,page:*mut UObject,state:&mut State) -> RuntimeResult<()> {
    state.controls.retain(|c|c.page.live(a)&&c.selector.live(a)&&c.value.live(a)&&c.client.live(a)&&c.engine.live(a));
    if state.controls.iter().any(|c|c.page.p==page) {return Ok(());}
    crate::settings_config::read(a)?; let (engine,client)=unsafe {objects(a,page)?};
    let class=unsafe {a.static_load_class(a.native_class("Core.Object")?,ptr::null_mut(),&"HGame.HPMenuOptionCombo\0".chars().map(u32::from).collect::<Vec<_>>(),2)?};
    if class.is_null() {return Err(RuntimeError::new("Graphics menu combo unavailable"));}
    let selector=unsafe {create_combo(a,page,class,365.,"Advanced")?}; let value=unsafe {create_combo(a,page,class,395.,"Value")?};
    let mut c=Controls {page:unsafe {Weak::new(page)},selector:unsafe {Weak::new(selector)},value:unsafe {Weak::new(value)},client:unsafe {Weak::new(client.cast())},engine:unsafe {Weak::new(engine.cast())},selected:Setting::FrameLimit,group:0,value_group:0,msaa_created_probe:None,msaa_applied_probe:None};
    unsafe {populate_selector(a,&c)?; populate(a,&c)?; place_before_back(a,page,[selector,value])?;}
    // Emit actual state for all settings only when the opt-in test probe is on.
    if std::env::var_os("HP2_STOCK_GRAPHICS_GPU_TRACE").as_deref()==Some(std::ffi::OsStr::new("1")) {
        for &(selected,_) in SETTINGS {let view=Controls {selected,..c}; let value=unsafe {shown_value(a,&view)};
            let requested=gpu_kind(selected).and_then(|kind|stock_graphics_gpu_backend::Live::new(a).requested(kind).ok()).and_then(gpu_label);
            if selected==Setting::Msaa && value.starts_with("Pending") {
                if let Ok(Value::Msaa(requested))=stock_graphics_gpu_backend::Live::new(a).requested(Value::Msaa(0)) {c.msaa_created_probe=Some(requested);}
            } else {trace(a,"created",selected,requested.unwrap_or(&value),&value);}
        }
    }
    // Keep ownership before exposing either control, including on a partial failure.
    state.controls.push(c);
    unsafe {boolean(a,selector,"bAcceptsFocus",true)?; boolean(a,value,"bAcceptsFocus",true)?; boolean(a,selector,"bWindowVisible",true)?; boolean(a,value,"bWindowVisible",true)?;}
    Ok(())
}
unsafe fn apply_selected(a:CoreAccess<'_>,page:*mut UObject,c:&mut Controls,selected:&str)->RuntimeResult<()> {
    let (engine,client)=unsafe {objects(a,page)?}; if engine.cast::<UObject>()!=c.engine.p||client.cast::<UObject>()!=c.client.p{return Err(RuntimeError::new("Graphics menu objects changed"));}
    let result:Result<(),String> = if let Some(next)=gpu_change(c.selected,selected) {
        match stock_graphics_gpu::change(&mut stock_graphics_gpu_backend::Live::new(a),next) {
            Err(stock_graphics_gpu::Error::Commit(error)) => {
                unsafe {populate(a,c)?;}
                let _=unsafe {call(a,page,"doHPMessageBox",&[("msg",Arg::Str("Setting saved, but status update failed. Restart to refresh.")),("textButton1",Arg::Str("OK"))],ptr::null_mut())};
                return Err(RuntimeError::new(&format!("Graphics setting saved; status update failed: {error}")));
            }
            other => other.map_err(|e|format!("{e:?}")),
        }
    } else if let Some(next)=change(c.selected,selected) {
        unsafe {stock_graphics_settings::apply(a,engine,client,next)}.map_err(|e|format!("{e:?}"))
    } else {unsafe {populate(a,c)?;} return Err(RuntimeError::new("Unknown graphics setting value"));};
    unsafe {populate(a,c)?;}
    if let Err(error)=result {
        let _=unsafe {call(a,page,"doHPMessageBox",&[("msg",Arg::Str("Could not apply graphics setting. Check the displayed value.")),("textButton1",Arg::Str("OK"))],ptr::null_mut())};
        return Err(RuntimeError::new(&format!("Graphics setting failed: {error}")));
    }
    let actual=unsafe {shown_value(a,c)};
    if c.selected==Setting::Msaa && actual=="Pending" {
        if let Some(Value::Msaa(requested))=gpu_change(c.selected,selected) {c.msaa_applied_probe=Some(requested);}
    } else {trace(a,"applied",c.selected,selected,&actual);}
    Ok(())
}
unsafe fn callback(a:CoreAccess<'_>,page:*mut UObject,event:PageEvent,params:*const u8,size:usize)->RuntimeResult<()> {
    STATE.with(|slot| {let Ok(mut state)=slot.try_borrow_mut() else{return Ok(())}; owner(a,&mut state)?;
        if event==PageEvent::Created {return unsafe {create(a,page,&mut state)};}
        let f=unsafe {function(a,page,"Notify")?}; let expected=usize::from(unsafe {(*f).parms_size}); if params.is_null()||size<expected{return Ok(());}
        let cp=unsafe {field(a,f.cast(),"C","Core.ObjectProperty",size_of::<*mut c_void>(),expected,true)?}; let ep=unsafe {field(a,f.cast(),"E","Core.ByteProperty",1,expected,true)?};
        if unsafe {*params.add(ep)}!=1{return Ok(());} let sender=unsafe {params.add(cp).cast::<*mut UObject>().read_unaligned()};
        let Some(c)=state.controls.iter_mut().find(|c| c.page.p==page && (c.selector.p==sender||c.value.p==sender) && c.page.live(a)&&c.selector.live(a)&&c.value.live(a)&&c.client.live(a)&&c.engine.live(a)) else{return Ok(())};
        let (engine,client)=unsafe {objects(a,page)?}; if engine.cast::<UObject>()!=c.engine.p||client.cast::<UObject>()!=c.client.p{return Err(RuntimeError::new("Graphics menu objects changed"));}
        let selected=unsafe {text(a,sender)?};
        if sender==c.selector.p {
            if selected==MORE {c.group=(c.group+1)%PAGES.len(); c.selected=PAGES[c.group][0].0; c.value_group=0; unsafe {populate_selector(a,c)?;} return unsafe {populate(a,c)};}
            let Some((s,_))=PAGES[c.group].iter().find(|(_,name)|*name==selected) else{return Err(RuntimeError::new("Unknown graphics setting"));}; c.selected=*s; c.value_group=0; return unsafe {populate(a,c)};
        }
        if selected==MORE_VALUES {let pages=choices(c.selected).len().div_ceil(3); if pages<2{return Err(RuntimeError::new("Invalid graphics values page"));} c.value_group=(c.value_group+1)%pages; return unsafe {populate(a,c)};}
        unsafe {apply_selected(a,page,c,&selected)}
    })
}
/// Place a root-owned stock popup above its owner when it would extend below
/// the logical root. BeforePaint recomputes placement every frame, so this
/// function must run immediately after that stock script returns.
fn popup_position(x:f32,y:f32,width:f32,height:f32,owner_height:f32,root_width:f32,root_height:f32)->Option<(f32,f32)> {
    if ![x,y,width,height,owner_height,root_width,root_height].iter().all(|n|n.is_finite()) || width<=0. || height<=0. || owner_height<0. || root_width<=0. || root_height<=0. || width>root_width || height>root_height {return None;}
    let x=x.clamp(0.,root_width-width);
    let y=if y+height>root_height { (y-owner_height-height).clamp(0.,root_height-height) } else { y.clamp(0.,root_height-height) };
    Some((x,y))
}
/// Shared VM dispatcher calls this after the stock popup's BeforePaint.
/// Only the two live advanced combo lists are eligible for adjustment.
pub(crate) unsafe fn popup_after_paint(a:CoreAccess<'_>,list:*mut UObject)->RuntimeResult<()> {
    if !stock_menu_extensions::active() {return Ok(());}
    STATE.with(|slot| {let Ok(state)=slot.try_borrow() else{return Ok(())};
        for c in &state.controls {
            if !c.page.live(a) || !c.selector.live(a) || !c.value.live(a) {continue;}
            for combo in [c.selector.p,c.value.p] {
                if unsafe {object(a,combo,"List")?}!=list || !live(a,list) || !unsafe {read_boolean(a,combo,"bListVisible")?} {continue;}
                let root=unsafe {object(a,c.page.p,"Root")?}; if !live(a,root) {return Err(RuntimeError::new("Graphics popup root unavailable"));}
                let f=|p,n|->RuntimeResult<f32>{Ok(unsafe {prop(a,p,n,"Core.FloatProperty",4)?.cast::<f32>().read_unaligned()})};
                let x=f(list,"WinLeft")?; let y=f(list,"WinTop")?; let width=f(list,"WinWidth")?; let height=f(list,"WinHeight")?;
                let position=popup_position(x,y,width,height,f(combo,"WinHeight")?,f(root,"WinWidth")?,f(root,"WinHeight")?)
                    .ok_or_else(||RuntimeError::new("Graphics popup cannot fit inside the viewport"))?;
                unsafe {prop(a,list,"WinLeft","Core.FloatProperty",4)?.cast::<f32>().write_unaligned(position.0); prop(a,list,"WinTop","Core.FloatProperty",4)?.cast::<f32>().write_unaligned(position.1);}
                return Ok(());
            }
        } Ok(())
    })
}
fn step(index:usize,len:usize,forward:bool)->usize {if forward {(index+1)%len} else {(index+len-1)%len}}
/// Called after KeyDown on the stock combo, its edit child, or its list.
/// Arrows cycle settings/values directly, including values on other pages.
pub(crate) unsafe fn keyboard_after_keydown(a:CoreAccess<'_>,receiver:*mut UObject,key:i32)->RuntimeResult<()> {
    if !stock_menu_extensions::active() {return Ok(());}
    let forward=match key {37|38=>false,39|40=>true,_=>return Ok(())};
    STATE.with(|slot| {let Ok(mut state)=slot.try_borrow_mut() else{return Ok(())}; owner(a,&mut state)?;
        for c in &mut state.controls {
            if !c.page.live(a)||!c.selector.live(a)||!c.value.live(a)||!c.client.live(a)||!c.engine.live(a) {continue;}
            for (is_selector,combo) in [(true,c.selector.p),(false,c.value.p)] {
                let edit=unsafe {object(a,combo,"EditBox")?}; let list=unsafe {object(a,combo,"List")?};
                if receiver!=combo && receiver!=edit && receiver!=list {continue;}
                if unsafe {read_boolean(a,combo,"bListVisible")?} {unsafe {call(a,combo,"CloseUpWithNoSound",&[],ptr::null_mut())?;}}
                if is_selector {
                    let old=SETTINGS.iter().position(|(s,_)|*s==c.selected).ok_or_else(||RuntimeError::new("Graphics keyboard selection invalid"))?;
                    c.selected=SETTINGS[step(old,SETTINGS.len(),forward)].0;
                    c.group=PAGES.iter().position(|p|p.iter().any(|(s,_)|*s==c.selected)).ok_or_else(||RuntimeError::new("Graphics keyboard page invalid"))?;
                    c.value_group=0; unsafe {populate_selector(a,c)?; populate(a,c)?;} return Ok(());
                }
                let options=choices(c.selected); if options.is_empty()||!supported(c.selected) {return Ok(());}
                let current=if let Some(kind)=gpu_kind(c.selected) {stock_graphics_gpu_backend::Live::new(a).current(kind).ok().and_then(gpu_label).unwrap_or("").to_owned()} else {unsafe {shown_value(a,c)}};
                let index=options.iter().position(|&v|v==current).map(|i|step(i,options.len(),forward)).unwrap_or(if forward {0} else {options.len()-1});
                c.value_group=index/3; let selected=options[index]; return unsafe {apply_selected(a,c.page.p,c,selected)};
            }
        } Ok(())
    })
}
pub(crate) fn install()->RuntimeResult<stock_menu_extensions::Guard>{stock_menu_extensions::install("advanced-graphics",callback)}
/// Called before input each tick. Hide controls if notifications are unavailable.
pub(crate) unsafe fn tick(a:CoreAccess<'_>)->RuntimeResult<()> { STATE.with(|slot| {let mut state=slot.borrow_mut(); owner(a,&mut state)?;
    for c in &mut state.controls { let valid=c.page.live(a)&&c.client.live(a)&&c.engine.live(a)&&stock_menu_extensions::active() && unsafe {objects(a,c.page.p)}.ok().is_some_and(|(e,client)|e.cast::<UObject>()==c.engine.p && client.cast::<UObject>()==c.client.p); for control in [c.selector,c.value] {if control.live(a) {unsafe {
        // Lists are root-owned, so hiding their combo alone does not hide an open list.
        if !valid && read_boolean(a,control.p,"bListVisible")? {call(a,control.p,"CloseUpWithNoSound",&[],ptr::null_mut())?;}
        boolean(a,control.p,"bAcceptsFocus",valid && (control.p!=c.value.p || supported(c.selected)))?; boolean(a,control.p,"bWindowVisible",valid)?;
    }}}
        // A successful MSAA request is only effective after a world scope.
        // Emit its probe after observation, never from a pending/fallback label.
        if valid && (c.msaa_created_probe.is_some() || c.msaa_applied_probe.is_some()) {
            let actual=stock_graphics_gpu_backend::Live::new(a).msaa_display().unwrap_or_else(|_|"Unavailable".into());
            if !actual.starts_with("Pending") && actual!="Unavailable" {
                for (event,request) in [("created",c.msaa_created_probe.take()),("applied",c.msaa_applied_probe.take())] {
                    if let Some(request)=request {if let Some(label)=gpu_label(Value::Msaa(request)) {trace(a,event,Setting::Msaa,label,&actual);}}
                }
            }
        }
        // Observe external changes without disturbing an open value list.
        if valid && c.value.live(a) && unsafe {!read_boolean(a,c.value.p,"bListVisible")?} {
            let expected=unsafe {shown_value(a,c)};
            if unsafe {text(a,c.value.p)?} != expected {unsafe {set_text(a,c.value.p,&expected)?;}}
        }
    }
    state.controls.retain(|c|c.page.live(a)&&c.selector.live(a)&&c.value.live(a)&&c.client.live(a)&&c.engine.live(a)); Ok(()) }) }

#[cfg(test)]
mod tests {use super::*;
    #[test] fn popup_stays_in_logical_root_at_both_scales() {
        assert_eq!(popup_position(560.,520.,150.,89.,24.,800.,600.),Some((560.,407.)));
        assert_eq!(popup_position(560.,520.,150.,61.,24.,800.,600.),Some((560.,520.)));
        assert_eq!(popup_position(710.,700.,150.,89.,24.,1066.,800.),Some((710.,700.)));
        assert_eq!(popup_position(0.,0.,150.,900.,24.,800.,600.),None);
        assert_eq!(popup_position(f32::NAN,0.,150.,89.,24.,800.,600.),None);
    }
    #[test] fn arrows_wrap_and_reach_every_value_including_120_fps() {
        let values=choices(Setting::FrameLimit); let mut i=0; for expected in ["30 FPS","60 FPS","120 FPS","144 FPS","Unlimited"] {i=step(i,values.len(),true);assert_eq!(values[i],expected);} assert_eq!(values[step(0,values.len(),false)],"144 FPS");
    }
    #[test] fn every_choice_maps_to_a_valid_patch(){for &(setting,_) in SETTINGS {for value in choices(setting) {if let Some(v)=change(setting,value) {assert!(v.patch().plan().is_ok());} else {assert!(gpu_change(setting,value).unwrap().selectable());}} assert_eq!(change(setting,"invalid"),None);assert_eq!(gpu_change(setting,"invalid"),None);}}
    #[test] fn selector_pages_fit_stock_popup_and_cover_every_setting() {
        assert!(PAGES.iter().all(|p|!p.is_empty() && p.len()+1<=4));
        for &(setting,_) in SETTINGS {assert!(choices(setting).chunks(3).all(|page|page.len()+usize::from(choices(setting).len()>3)<=4));}
        let all:Vec<_>=PAGES.iter().flat_map(|p|p.iter().copied()).collect(); assert_eq!(all,SETTINGS);
    }
    #[test] fn retail_probe_values_are_unambiguous() {
        for (s,label,value) in [(Setting::FrameLimit,"120 FPS","120"),(Setting::UiScale,"125%","125"),(Setting::NativeText,"Off","off"),(Setting::VSync,"On","on"),(Setting::Anisotropy,"4x","4"),(Setting::Msaa,"4x","4")] {assert_eq!(wire(s,label),Some(value));}
        assert_eq!(wire(Setting::Msaa,"Unavailable"),None);
    }
    #[repr(align(8))] struct Client(USDLClient);
    #[test] fn readback_uses_live_fields_and_reports_custom_or_unavailable() {
        let mut engine:UGameEngine=unsafe {std::mem::zeroed()}; let mut client=Client(unsafe {std::mem::zeroed()});
        let e=&raw mut engine; let c=&raw mut client.0;
        engine.frame_rate_limit=120.; unsafe {ptr::addr_of_mut!((*c).ui_scale).write_unaligned(1.25); (*c.cast::<UClient>()).set_native_text(true);}
        assert_eq!(unsafe {current(Setting::FrameLimit,e,c)},Some("120 FPS"));
        assert_eq!(unsafe {current(Setting::UiScale,e,c)},Some("125%"));
        assert_eq!(unsafe {current(Setting::NativeText,e,c)},Some("On"));
        assert_eq!(unsafe {current(Setting::ScreenFlashes,e,c)},Some("Off"));
        engine.frame_rate_limit=75.; assert_eq!(unsafe {current(Setting::FrameLimit,e,c)},None);
        assert_eq!(supported(Setting::VSync),cfg!(feature="renderer-wgpu")); assert_eq!(supported(Setting::Anisotropy),cfg!(feature="renderer-wgpu"));
    }
}
