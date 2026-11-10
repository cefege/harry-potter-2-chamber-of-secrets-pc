//! Stock sound/video page world-render-scale slider. All changes happen on the
//! engine thread inside the stock script post-call hook.
use std::{cell::RefCell, ffi::c_void, ptr};
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UClass, UConsole, UFunction, UObject, UProperty, UStruct, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::{settings_config, settings_policy::SettingsPatch, stock_menu_extensions::{self, PageEvent}, stock_render_scale_transaction::{Backend, ChangeError, Scale, Session}};

#[derive(Clone, Copy)]
struct Weak { p: *mut UObject, index: i32 }
impl Weak {
    unsafe fn new(p: *mut UObject) -> Self { Self { p, index: unsafe { (*p).index } } }
    fn live(self, a: CoreAccess<'_>) -> bool { !self.p.is_null() && self.index >= 0 && a.object_at(self.index) == self.p.cast() }
}
struct Control { page: Weak, slider: Weak, device: Weak, session: Session, unavailable: bool, failure: Option<Failure> }
#[derive(Default)]
struct State { owner: usize, controls: Vec<Control>, failure: Option<Failure>, recovery: Option<Session> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn owner(a: CoreAccess<'_>, s: &mut State) -> RuntimeResult<()> {
    let id = a.owner_context::<crate::core_owner::EngineState>().map(|o| o as *const _ as usize).ok_or_else(|| RuntimeError::new("Render scale owner unavailable"))?;
    if s.owner != id { *s = State { owner: id, ..Default::default() }; }
    Ok(())
}
fn trace(a: CoreAccess<'_>, event: &str, scale: Scale, unavailable: bool) {
    if std::env::var_os("HP2_STOCK_SCALE_TRACE").as_deref() != Some(std::ffi::OsStr::new("1")) { return; }
    if let Ok(category) = a.intern_str("Log", FindName::Add) {
        let message: Vec<u32> = format!("StockScaleProbe event={event} percent={} unavailable={unavailable}", scale.percent()).chars().map(u32::from).collect();
        let _ = a.emit_log(category, &message);
    }
}
fn live(a: CoreAccess<'_>, p: *mut UObject) -> bool { !p.is_null() && (0..a.object_count()).any(|i| a.object_at(i) == p.cast()) }
unsafe fn field(a: CoreAccess<'_>, st: *mut UStruct, name: &str, kind: &str, width: usize, limit: usize, param: bool) -> RuntimeResult<usize> {
    let p = unsafe { a.find_field(st, a.intern_str(name, FindName::Find)?, a.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new(&format!("Render scale missing field {name}"))); }
    let off = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Render scale negative field offset"))?;
    if unsafe { (*p).array_dim } != 1 || usize::try_from(unsafe { (*p).element_size }).ok() != Some(width) || off.checked_add(width).is_none_or(|end| end > limit) || (param && unsafe { (*p).property_flags } & 0x80 == 0) { return Err(RuntimeError::new(&format!("Render scale incompatible field {name}"))); }
    Ok(off)
}
unsafe fn prop(a: CoreAccess<'_>, p: *mut UObject, name: &str, kind: &str, width: usize) -> RuntimeResult<*mut u8> {
    let class = unsafe { (*p).class.cast::<UClass>() };
    let size = usize::try_from(unsafe { (*class).properties_size }).map_err(|_| RuntimeError::new("Render scale class size"))?;
    let off = unsafe { field(a, class.cast(), name, kind, width, size, false)? };
    Ok(unsafe { p.cast::<u8>().add(off) })
}
unsafe fn object(a: CoreAccess<'_>, p: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    Ok(unsafe { prop(a, p, name, "Core.ObjectProperty", size_of::<*mut c_void>())?.cast::<*mut UObject>().read_unaligned() })
}
unsafe fn boolean(a: CoreAccess<'_>, p: *mut UObject, name: &str, value: bool) -> RuntimeResult<()> {
    let f = unsafe { a.find_field((*p).class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.BoolProperty")?) }.cast::<UBoolProperty>();
    if f.is_null() { return Err(RuntimeError::new("Render scale boolean missing")); }
    let mask = unsafe { (*f).bit_mask }; if mask.count_ones() != 1 { return Err(RuntimeError::new("Render scale invalid boolean")); }
    let slot = unsafe { prop(a, p, name, "Core.BoolProperty", 4)? }.cast::<u32>();
    let v = unsafe { slot.read_unaligned() }; unsafe { slot.write_unaligned(if value { v | mask } else { v & !mask }); } Ok(())
}
unsafe fn function(a: CoreAccess<'_>, p: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { a.find_field((*p).class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { Err(RuntimeError::new("Render scale function missing")) } else { Ok(f) }
}
unsafe fn device(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<*mut UObject> {
    let root = unsafe { object(a, page, "Root")? }; if !live(a, root) { return Err(RuntimeError::new("Render scale root unavailable")); }
    let console = unsafe { object(a, root, "Console")? };
    if !live(a, console) || !unsafe { hp2_core::object_model::is_a(console.cast(), a.native_class("Engine.Console")?.cast()) } { return Err(RuntimeError::new("Render scale console unavailable")); }
    let vp = unsafe { (*console.cast::<UConsole>()).viewport.cast::<UViewport>() };
    if !live(a, vp.cast()) || unsafe { (*vp).console.cast::<UObject>() } != console || !unsafe { hp2_core::object_model::is_a(vp.cast(), a.native_class("Engine.Viewport")?.cast()) } { return Err(RuntimeError::new("Render scale viewport unavailable")); }
    let dev = unsafe { (*vp).ren_dev.cast::<UObject>() };
    if !live(a, dev) || !unsafe { hp2_core::object_model::is_a(dev.cast(), a.native_class("WgpuDrv.WgpuRenderDevice")?.cast()) } { return Err(RuntimeError::new("Render scale renderer unavailable")); }
    Ok(dev)
}
unsafe fn live_scale(a: CoreAccess<'_>, dev: *mut UObject) -> RuntimeResult<Scale> {
    let n = unsafe { prop(a, dev, "RenderScale", "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned() } * 100.0;
    if !n.is_finite() || (n - n.round()).abs() > 0.001 || !(0.0..=255.0).contains(&n) { return Err(RuntimeError::new("Unsupported live render scale")); }
    Scale::new(n.round() as u8).ok_or_else(|| RuntimeError::new("Unsupported live render scale"))
}
unsafe fn graphics_scales(a: CoreAccess<'_>) -> RuntimeResult<(Scale, Scale)> {
    let snapshot = unsafe { crate::render_device_owner::global_live_graphics_snapshot(a)? };
    let requested = Scale::new(snapshot.requested_scale).ok_or_else(|| RuntimeError::new("Unsupported requested render scale"))?;
    let effective = Scale::new(snapshot.effective_scale).ok_or_else(|| RuntimeError::new("Unsupported effective render scale"))?;
    Ok((requested, effective))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure { Live, Persist, Rollback, Commit, Invalid }
impl Failure {
    fn from_error<E>(error: &ChangeError<E>, previous: Option<Self>) -> Self {
        // Startup fallback is not a rollback failure. Preserve a prior rollback
        // warning until a later change succeeds.
        if previous == Some(Self::Rollback) && !matches!(error, ChangeError::Commit(_)) { return Self::Rollback; }
        match error { ChangeError::Live(_) => Self::Live, ChangeError::Persist(_) => Self::Persist, ChangeError::Rollback { .. } => Self::Rollback, ChangeError::Commit(_) => Self::Commit }
    }
    fn message(self) -> &'static str {
        match self {
            Self::Live => "3D scale could not be changed.",
            Self::Persist => "Scale not saved. Previous scale restored.",
            Self::Rollback => "Scale not saved. Restore failed. Restart may change scale.",
            Self::Commit => "Scale saved, but renderer status could not update. Try again.",
            Self::Invalid => "Invalid 3D scale. Previous scale restored.",
        }
    }
    fn short(self) -> &'static str {
        match self { Self::Live | Self::Invalid => "Scale change failed", Self::Persist => "Scale not saved", Self::Rollback => "Restore failed!", Self::Commit => "Status update failed" }
    }
}
fn label(requested: Scale, effective: Scale, unavailable: bool, failure: Option<Failure>) -> String {
    if let Some(failure) = failure { failure.short().to_owned() }
    else if requested != effective {
        // Startup fallback changes the configured GPU scale for all scopes.
        format!("{}% -> {}%!", requested.percent(), effective.percent())
    } else if unavailable {
        // At least one scope fell back. Other scopes in the same frame may
        // still have used the requested scale, so do not claim a full fallback.
        format!("{}% partial!", requested.percent())
    } else { format!("3D Scale: {}%", effective.percent()) }
}
unsafe fn display(a: CoreAccess<'_>, slider: *mut UObject, session: Session, unavailable: bool, failure: Option<Failure>) -> RuntimeResult<()> {
    let value = unsafe { prop(a, slider, "Value", "Core.FloatProperty", 4)? }.cast::<f32>();
    let text = unsafe { prop(a, slider, "Text", "Core.StrProperty", size_of::<TArrayRaw>())? }.cast::<TArrayRaw>();
    // Keep the warning short enough for the stock slider's left text area.
    let label = label(session.committed(), session.visible(), unavailable, failure);
    let label: Vec<u32> = label.chars().map(u32::from).chain([0]).collect();
    // write_vm_fstring_result owns and releases the existing value.
    let mut old = unsafe { text.read_unaligned() };
    let result = unsafe { a.write_vm_fstring_result(&mut old, &label[..label.len()-1].iter().map(|&n| n as i32).collect::<Vec<_>>()) };
    unsafe { text.write_unaligned(old); } result?;
    unsafe { value.write_unaligned(session.visible().slider_value()); } Ok(())
}
/// Use the stock modal so a failed selection is visible at the point of use.
/// The compact slider label remains as a fallback if the modal cannot open.
unsafe fn show_failure(a: CoreAccess<'_>, page: *mut UObject, failure: Failure) -> RuntimeResult<()> {
    let f = unsafe { function(a, page, "doHPMessageBox")? };
    let size = usize::from(unsafe { (*f).parms_size });
    let msg = unsafe { field(a, f.cast(), "msg", "Core.StrProperty", size_of::<TArrayRaw>(), size, true)? };
    let button = unsafe { field(a, f.cast(), "textButton1", "Core.StrProperty", size_of::<TArrayRaw>(), size, true)? };
    unsafe { field(a, f.cast(), "ReturnValue", "Core.ObjectProperty", size_of::<*mut c_void>(), size, true)?; }
    let mut frame = vec![0u64; (size + 7) / 8];
    let base = frame.as_mut_ptr().cast::<u8>();
    let mut allocated = Vec::new();
    let result = (|| -> RuntimeResult<()> {
        for (off, text) in [(msg, failure.message()), (button, "Okay")] {
            let mut value: TArrayRaw = unsafe { std::mem::zeroed() };
            unsafe { a.assign_fstring(&mut value, &text.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?; base.add(off).cast::<TArrayRaw>().write_unaligned(value); }
            allocated.push(off);
        }
        let mut dialog: *mut UObject = ptr::null_mut();
        unsafe { a.execute_event(page, f, base.cast(), (&raw mut dialog).cast())?; }
        if !live(a, dialog) { return Err(RuntimeError::new("Render scale warning dialog unavailable")); }
        Ok(())
    })();
    let mut cleanup_error = None;
    for off in allocated {
        let mut value = unsafe { base.add(off).cast::<TArrayRaw>().read_unaligned() };
        if let Err(error) = unsafe { a.destroy_owned_fstring(&mut value) } { cleanup_error = Some(error); }
    }
    result?;
    if let Some(error) = cleanup_error { return Err(error); }
    Ok(())
}
// Move the new item immediately before the stock Back button, preserving the
// order of every existing item and validating the complete circular chain.
unsafe fn place_before_back(a: CoreAccess<'_>, page: *mut UObject, slider: *mut UObject) -> RuntimeResult<()> {
    let back = unsafe { object(a, page, "BackPageButton")? };
    let last = unsafe { object(a, page, "TabLast")? };
    let dialog = a.native_class("UWindow.UWindowDialogControl")?;
    let valid = |p: *mut UObject| live(a, p) && unsafe { hp2_core::object_model::is_a(p.cast(), dialog.cast()) };
    if !valid(back) || !valid(last) { return Err(RuntimeError::new("Render scale tab chain unavailable")); }
    let first = unsafe { object(a, last, "TabNext")? };
    let mut items = Vec::new(); let mut p = first;
    loop {
        if !valid(p) || items.len() >= 4096 || unsafe { object(a, p, "NotifyWindow")? } != page { return Err(RuntimeError::new("Render scale invalid tab chain")); }
        if items.contains(&p) { if p != first { return Err(RuntimeError::new("Render scale tab cycle")); } break; }
        items.push(p); p = unsafe { object(a, p, "TabNext")? };
    }
    if items.last().copied() != Some(last) { return Err(RuntimeError::new("Render scale invalid tab tail")); }
    for (i, &item) in items.iter().enumerate() {
        if unsafe { object(a, item, "TabPrev")? } != items[(i + items.len() - 1) % items.len()] { return Err(RuntimeError::new("Render scale inconsistent tab links")); }
    }
    let old = items.iter().position(|&p| p == slider).ok_or_else(|| RuntimeError::new("Render scale slider absent from tab chain"))?;
    items.remove(old);
    let target = items.iter().position(|&p| p == back).ok_or_else(|| RuntimeError::new("Render scale back button absent from tab chain"))?;
    items.insert(target, slider);
    // Validate all destinations before changing any link.
    let mut slots = Vec::new();
    for &item in &items { slots.push((unsafe { prop(a, item, "TabPrev", "Core.ObjectProperty", size_of::<*mut c_void>())? }, unsafe { prop(a, item, "TabNext", "Core.ObjectProperty", size_of::<*mut c_void>())? })); }
    let tail = unsafe { prop(a, page, "TabLast", "Core.ObjectProperty", size_of::<*mut c_void>())? };
    for (i, &(prev, next)) in slots.iter().enumerate() {
        unsafe { prev.cast::<*mut UObject>().write_unaligned(items[(i + items.len() - 1) % items.len()]); next.cast::<*mut UObject>().write_unaligned(items[(i + 1) % items.len()]); }
    }
    unsafe { tail.cast::<*mut UObject>().write_unaligned(*items.last().unwrap()); }
    Ok(())
}
struct LiveBackend<'a> { access: CoreAccess<'a> }
impl Backend for LiveBackend<'_> {
    type Error = String;
    fn set_live(&mut self, scale: Scale) -> Result<(), String> {
        unsafe { crate::render_device_owner::set_global_world_scale(self.access, scale.percent()) }.map_err(|e| format!("{e}"))
    }
    fn persist(&mut self, scale: Scale) -> Result<(), String> {
        settings_config::apply(self.access, &SettingsPatch { render_scale: Some(scale.config_value()), ..Default::default() }).map(|_| ()).map_err(|e| format!("{e:?}"))
    }
    fn commit(&mut self) -> Result<(), String> {
        unsafe { crate::render_device_owner::commit_global_graphics_preference(self.access, 3) }.map_err(|e| format!("{e}"))
    }
}
unsafe fn create(a: CoreAccess<'_>, page: *mut UObject, state: &mut State) -> RuntimeResult<()> {
    state.controls.retain(|c| c.page.live(a) && c.slider.live(a) && c.device.live(a));
    if state.controls.iter().any(|c| c.page.p == page) { return Ok(()); }
    settings_config::read(a)?;
    let dev = unsafe { device(a, page)? }; let scale = unsafe { live_scale(a, dev)? };
    let (requested, effective) = unsafe { graphics_scales(a)? };
    if scale != effective { return Err(RuntimeError::new("Renderer scale reflection mismatch")); }
    let session = state.recovery.filter(|s| s.visible() == effective).unwrap_or_else(|| Session::with_fallback(requested, effective));
    let class = unsafe { a.static_load_class(a.native_class("Core.Object")?, ptr::null_mut(), &"HGame.HPMenuOptionHSlider\0".chars().map(u32::from).collect::<Vec<_>>(), 2)? };
    if class.is_null() { return Err(RuntimeError::new("Render scale slider class unavailable")); }
    let f = unsafe { function(a, page, "CreateControl")? }; let size = usize::from(unsafe { (*f).parms_size });
    let arg = |n, k, w| unsafe { field(a, f.cast(), n, k, w, size, true) };
    let cl = arg("ControlClass", "Core.ClassProperty", size_of::<*mut c_void>())?;
    let x = arg("X", "Core.FloatProperty", 4)?; let y = arg("Y", "Core.FloatProperty", 4)?; let w = arg("W", "Core.FloatProperty", 4)?; let h = arg("H", "Core.FloatProperty", 4)?;
    arg("ReturnValue", "Core.ObjectProperty", size_of::<*mut c_void>())?;
    let mut frame = vec![0u64; (size + 7) / 8]; let p = frame.as_mut_ptr().cast::<u8>();
    unsafe { p.add(cl).cast::<*mut UClass>().write_unaligned(class); for (off, v) in [(x, 30f32), (y, 220f32), (w, 280f32), (h, 1f32)] { p.add(off).cast::<f32>().write_unaligned(v); } }
    let mut slider: *mut UObject = ptr::null_mut();
    unsafe { a.execute_event(page, f, p.cast(), (&raw mut slider).cast())?; }
    if !live(a, slider) || !unsafe { hp2_core::object_model::is_a(slider.cast(), class.cast()) } { return Err(RuntimeError::new("Render scale CreateControl failed")); }
    unsafe { boolean(a, slider, "bWindowVisible", false)?; boolean(a, slider, "bAcceptsFocus", false)?; }
    // The stock slider has no text and its default color is black. Copy the
    // adjacent stock label, which uses the page's visible font and color.
    let source = unsafe { object(a, page, "BrightnessLabel")? };
    if !live(a, source) || !unsafe { hp2_core::object_model::is_a(source.cast(), a.native_class("UWindow.UWindowLabelControl")?.cast()) } { return Err(RuntimeError::new("Stock brightness label unavailable")); }
    for (name, kind) in [("Font", "Core.IntProperty"), ("TextColor", "Core.StructProperty")] {
        let v = unsafe { prop(a, source, name, kind, 4)?.cast::<u32>().read_unaligned() };
        unsafe { prop(a, slider, name, kind, 4)?.cast::<u32>().write_unaligned(v); }
    }
    for (name, value) in [("MinValue", 0f32), ("MaxValue", 11f32)] { unsafe { prop(a, slider, name, "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(value); } }
    unsafe { prop(a, slider, "Step", "Core.IntProperty", 4)?.cast::<i32>().write_unaligned(1); boolean(a, slider, "bNoSlidingNotify", false)?; display(a, slider, session, false, state.failure)?; }
    unsafe { place_before_back(a, page, slider)?; }
    state.controls.push(Control { page: unsafe { Weak::new(page) }, slider: unsafe { Weak::new(slider) }, device: unsafe { Weak::new(dev) }, session, unavailable: false, failure: state.failure });
    unsafe { boolean(a, slider, "bAcceptsFocus", true)?; boolean(a, slider, "bWindowVisible", true)?; }
    trace(a, "created", scale, false);
    Ok(())
}
unsafe fn callback(a: CoreAccess<'_>, page: *mut UObject, event: PageEvent, params: *const u8, size: usize) -> RuntimeResult<()> {
    STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else { return Ok(()) }; owner(a, &mut state)?;
        if event == PageEvent::Created { return unsafe { create(a, page, &mut state) }; }
        let f = unsafe { function(a, page, "Notify")? }; let expected = usize::from(unsafe { (*f).parms_size });
        if params.is_null() || size < expected { return Ok(()); }
        let c = unsafe { field(a, f.cast(), "C", "Core.ObjectProperty", size_of::<*mut c_void>(), expected, true)? };
        let e = unsafe { field(a, f.cast(), "E", "Core.ByteProperty", 1, expected, true)? };
        if unsafe { *params.add(e) } != 1 { return Ok(()); }
        let slider = unsafe { params.add(c).cast::<*mut UObject>().read_unaligned() };
        let Some(control) = state.controls.iter_mut().find(|c| c.page.p == page && c.slider.p == slider && c.page.live(a) && c.slider.live(a) && c.device.live(a)) else { return Ok(()) };
        let v = unsafe { prop(a, slider, "Value", "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned() };
        let Some(next) = Scale::from_slider(v) else {
            let failure = if control.failure == Some(Failure::Rollback) { Failure::Rollback } else { Failure::Invalid };
            control.failure = Some(failure);
            let label_result = unsafe { display(a, slider, control.session, control.unavailable, control.failure) };
            let dialog_result = unsafe { show_failure(a, page, failure) };
            state.failure = Some(failure);
            return Err(RuntimeError::new(&format!("Invalid render scale slider value; label: {label_result:?}; dialog: {dialog_result:?}")));
        };
        match control.session.change(&mut LiveBackend { access: a }, next) {
            Ok(()) => {
                control.failure = None;
                unsafe { display(a, slider, control.session, control.unavailable, None)?; }
                trace(a, "changed", control.session.visible(), control.unavailable);
                state.failure = None;
                state.recovery = None;
                Ok(())
            }
            Err(error) => {
                let failure = Failure::from_error(&error, control.failure);
                control.failure = Some(failure);
                // Try both independently. The persistent inline warning is the
                // fallback if the stock modal cannot be created.
                let label_result = unsafe { display(a, slider, control.session, control.unavailable, control.failure) };
                let dialog_result = unsafe { show_failure(a, page, failure) };
                state.recovery = (failure == Failure::Rollback || control.session.pending_commit()).then_some(control.session);
                state.failure = Some(failure);
                Err(RuntimeError::new(&format!("Render scale change failed: {error:?}; label: {label_result:?}; dialog: {dialog_result:?}")))
            }
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    // SAFETY: `owner` is a live reflected class/function, `limit` is its actual
    // object/parameter storage size, and all returned properties belong to owner.
    unsafe fn slot(
        a: CoreAccess<'_>, owner: *mut UStruct, name: &str, kind: &str,
        width: usize, limit: usize, parameter: bool,
    ) -> (*mut UProperty, usize) {
        let id = a.intern_str(name, FindName::Find).unwrap();
        let p = unsafe { a.find_field(owner, id, a.native_class(kind).unwrap()) }.cast::<UProperty>();
        assert!(!p.is_null(), "missing {name}: {kind}");
        let offset = usize::try_from(unsafe { (*p).offset }).expect("nonnegative property offset");
        assert_eq!(unsafe { (*p).array_dim }, 1, "{name}");
        assert_eq!(unsafe { (*p).element_size }, width as i32, "{name}");
        assert!(offset.checked_add(width).is_some_and(|end| end <= limit), "{name} bounds");
        if parameter { assert_ne!(unsafe { (*p).property_flags } & 0x80, 0, "{name} parameter flag"); }
        (p, offset)
    }

    // SAFETY: class is live and is the receiver's class. The returned function
    // belongs to its class hierarchy and stays live for the owner lifetime.
    unsafe fn function(a: CoreAccess<'_>, class: *mut UClass, name: &str) -> *mut UFunction {
        let id = a.intern_str(name, FindName::Find).unwrap();
        let f = unsafe { a.find_field(class.cast(), id, a.native_class("Core.Function").unwrap()) }.cast::<UFunction>();
        assert!(!f.is_null(), "missing function {name}");
        f
    }

    #[test]
    #[ignore = "needs prototype data: run by stock_render_scale_slider_vm"]
    fn loaded_slider_checkvalue_setvalue_and_scale_mapping() {
        // Prototype data only (data-prototype). The compiled CheckValue, SetValue and
        // Notify exports were decoded passively (slider-decode-uwindow) before this ran.
        let owner = crate::render_owner::render_test_support::owner("scale-slider-vm");
        let a = owner.access();
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
            .join(format!("render-scale-slider-vm-{}", std::process::id()));
        unsafe { a.load_package(&root.join("System/HGame.u"), 1) }.unwrap();
        let slider = crate::render_owner::render_test_support::construct(a, "HGame.HPMenuOptionHSlider");
        assert!(!slider.is_null(), "slider construction");

        // SAFETY: slider is constructed by the owner from the loaded class, remains
        // live until owner drop, and fields/parameter buffers are validated below.
        unsafe {
            let class = (*slider).class.cast::<UClass>();
            assert!(!class.is_null());
            let size = usize::try_from((*class).properties_size).unwrap();
            let fslot = |name| slot(a, class.cast(), name, "Core.FloatProperty", 4, size, false).1;
            let min = fslot("MinValue");
            let max = fslot("MaxValue");
            let value = fslot("Value");
            let step = slot(a, class.cast(), "Step", "Core.IntProperty", 4, size, false).1;
            let notify = slot(a, class.cast(), "NotifyWindow", "Core.ObjectProperty", size_of::<*mut c_void>(), size, false).1;
            // Do not manufacture a dialog client or invoke Created. The loaded
            // Notify implementation must be inspected before running this test.
            assert!(slider.cast::<u8>().add(notify).cast::<*mut UObject>().read_unaligned().is_null(), "NotifyWindow must be None");

            let check = function(a, class, "CheckValue");
            let set = function(a, class, "SetValue");
            let notify_function = function(a, class, "Notify");
            for (name, f) in [("CheckValue", check), ("SetValue", set), ("Notify", notify_function)] {
                assert_eq!((*f).function_flags & 0x400, 0, "{name} must be script, not native");
                let script = (*f.cast::<UStruct>()).script;
                assert!(script.num > 0 && script.max >= script.num && !script.data.is_null(), "{name} script storage");
            }
            let check_size = usize::from((*check).parms_size);
            let test = slot(a, check.cast(), "Test", "Core.FloatProperty", 4, check_size, true).1;
            let ret = slot(a, check.cast(), "ReturnValue", "Core.FloatProperty", 4, check_size, true).1;
            assert_eq!(usize::from((*check).return_value_offset), ret, "return metadata");
            assert!(test + 4 <= ret || ret + 4 <= test, "CheckValue parameters overlap");
            assert!(usize::try_from((*check.cast::<UStruct>()).properties_size).unwrap() >= check_size);
            let set_size = usize::from((*set).parms_size);
            let input = slot(a, set.cast(), "NewValue", "Core.FloatProperty", 4, set_size, true).1;
            let (no_notify, no_notify_off) = slot(a, set.cast(), "bNoNotify", "Core.BoolProperty", 4, set_size, true);
            let mask = (*no_notify.cast::<UBoolProperty>()).bit_mask;
            assert_eq!(mask.count_ones(), 1, "bNoNotify mask");
            assert!(input + 4 <= no_notify_off || no_notify_off + 4 <= input, "SetValue parameters overlap");
            assert_eq!((*set).return_value_offset, u16::MAX, "SetValue has no return");
            assert!(usize::try_from((*set.cast::<UStruct>()).properties_size).unwrap() >= set_size);

            let set_range_fields = |lo: f32, hi: f32, increment: i32| {
                slider.cast::<u8>().add(min).cast::<f32>().write_unaligned(lo);
                slider.cast::<u8>().add(max).cast::<f32>().write_unaligned(hi);
                slider.cast::<u8>().add(step).cast::<i32>().write_unaligned(increment);
            };
            let check_value = |input_value: f32| -> f32 {
                let mut frame = vec![0u64; check_size.div_ceil(size_of::<u64>()).max(1)];
                frame.as_mut_ptr().cast::<u8>().add(test).cast::<f32>().write_unaligned(input_value);
                let mut result = f32::NAN;
                a.execute_event(slider, check, frame.as_mut_ptr().cast(), (&raw mut result).cast()).unwrap();
                result
            };
            let set_value = |input_value: f32| -> f32 {
                let mut frame = vec![0u64; set_size.div_ceil(size_of::<u64>()).max(1)];
                let bytes = frame.as_mut_ptr().cast::<u8>();
                bytes.add(input).cast::<f32>().write_unaligned(input_value);
                bytes.add(no_notify_off).cast::<u32>().write_unaligned(mask);
                a.execute_event(slider, set, frame.as_mut_ptr().cast(), ptr::null_mut()).unwrap();
                slider.cast::<u8>().add(value).cast::<f32>().read_unaligned()
            };

            set_range_fields(50.0, 100.0, 5);
            for (input, expected) in [(0.0, 50.0), (49.0, 50.0), (52.49, 50.0), (52.5, 55.0),
                (67.0, 65.0), (67.5, 70.0), (100.0, 100.0), (110.0, 100.0)] {
                assert_eq!(check_value(input), expected, "old CheckValue({input})");
                assert_eq!(set_value(input), expected, "old SetValue({input})");
            }
            set_range_fields(0.0, 11.0, 1);
            for (input, expected) in [(-2.0, 0.0), (0.0, 0.0), (3.49, 3.0), (3.5, 4.0),
                (4.49, 4.0), (4.5, 5.0), (11.0, 11.0), (12.0, 11.0)] {
                assert_eq!(check_value(input), expected, "ordinal CheckValue({input})");
                assert_eq!(set_value(input), expected, "ordinal SetValue({input})");
            }
            for index in 0..12 {
                assert_eq!(check_value(index as f32), index as f32, "CheckValue index {index}");
                assert_eq!(set_value(index as f32), index as f32, "SetValue index {index}");
            }
            // The production mapping survives the loaded script's own rounding.
            for percent in (0..=u8::MAX).filter_map(Scale::new) {
                let stored = set_value(percent.slider_value());
                assert_eq!(Scale::from_slider(stored), Some(percent), "{}%", percent.percent());
            }
        }
    }

    #[test]
    fn unavailable_warning_preserves_requested_percent() {
        let scale = Scale::new(50).unwrap();
        assert_eq!(label(scale, scale, false, None), "3D Scale: 50%");
        assert_eq!(label(scale, scale, true, None), "50% partial!");
        assert_eq!(label(scale, Scale::new(100).unwrap(), false, None), "50% -> 100%!");
        assert_eq!(label(scale, Scale::new(100).unwrap(), true, None), "50% -> 100%!");
        assert_eq!(label(scale, scale, false, None), "3D Scale: 50%");
    }
    #[derive(Default)]
    struct Inject { fail_target: bool, fail_persist: bool, fail_restore: bool, calls: Vec<(char, u8)> }
    impl Backend for Inject {
        type Error = &'static str;
        fn set_live(&mut self, scale: Scale) -> Result<(), Self::Error> {
            self.calls.push(('L', scale.percent()));
            if (scale.percent() == 75 && self.fail_target) || (scale.percent() == 100 && self.fail_restore) { Err("GPU") } else { Ok(()) }
        }
        fn persist(&mut self, scale: Scale) -> Result<(), Self::Error> {
            self.calls.push(('P', scale.percent()));
            if self.fail_persist { Err("save") } else { Ok(()) }
        }
    }
    #[test]
    fn injected_failures_have_distinct_modal_and_inline_warnings() {
        for (mut backend, expected, shown, saved, calls) in [
            (Inject { fail_target: true, ..Default::default() }, Failure::Live, 100, 100, vec![('L',75)]),
            (Inject { fail_persist: true, ..Default::default() }, Failure::Persist, 100, 100, vec![('L',75),('P',75),('L',100)]),
            (Inject { fail_persist: true, fail_restore: true, ..Default::default() }, Failure::Rollback, 75, 100, vec![('L',75),('P',75),('L',100)]),
        ] {
            let mut session = Session::new(Scale::new(100).unwrap());
            let error = session.change(&mut backend, Scale::new(75).unwrap()).unwrap_err();
            let failure = Failure::from_error(&error, None);
            assert_eq!(failure, expected);
            assert_eq!(session.visible().percent(), shown);
            assert_eq!(session.committed().percent(), saved);
            assert_eq!(backend.calls, calls);
            assert_eq!(label(session.committed(), session.visible(), false, Some(failure)), failure.short());
            assert!(!failure.message().is_empty());
        }
        assert!(Failure::Persist.message().contains("restored"));
        assert!(Failure::Rollback.message().contains("Restore failed"));
        assert!(Failure::Rollback.message().contains("Restart"));
    }
    #[test]
    fn startup_fallback_is_not_reported_as_failed_rollback() {
        let mut backend = Inject { fail_target: true, ..Default::default() };
        let mut session = Session::with_fallback(Scale::new(50).unwrap(), Scale::new(100).unwrap());
        let error = session.change(&mut backend, Scale::new(75).unwrap()).unwrap_err();
        assert_eq!(Failure::from_error(&error, None), Failure::Live);
        assert_eq!((session.committed().percent(), session.visible().percent()), (50, 100));
        assert_eq!(label(session.committed(), session.visible(), false, None), "50% -> 100%!");
    }
    #[test]
    fn failed_rollback_reopen_keeps_divergence_and_retries_save() {
        let mut backend = Inject { fail_persist: true, fail_restore: true, ..Default::default() };
        let mut session = Session::new(Scale::new(100).unwrap());
        session.change(&mut backend, Scale::new(75).unwrap()).unwrap_err();
        let mut reopened = session;
        backend.fail_persist = false;
        backend.fail_restore = false;
        let count = backend.calls.len();
        reopened.change(&mut backend, Scale::new(75).unwrap()).unwrap();
        assert_eq!(&backend.calls[count..], [('L', 75), ('P', 75)]);
        assert_eq!(reopened.committed().percent(), 75);
    }
}
/// Register for the duration of each engine tick, before script delivery.
pub(crate) fn install() -> RuntimeResult<stock_menu_extensions::Guard> { stock_menu_extensions::install("render-scale", callback) }
/// Called every tick, even when the hook could not be installed. Never leave
/// a visible interactive slider whose notifications are not being delivered.
pub(crate) unsafe fn tick(a: CoreAccess<'_>, unavailable: Option<bool>) -> RuntimeResult<()> {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut(); owner(a, &mut state)?;
        for c in &state.controls {
            // A surviving window must not keep an interactive orphan if its
            // page or renderer is destroyed before the control itself.
            if c.slider.live(a) && (!c.page.live(a) || !c.device.live(a)) {
                unsafe { boolean(a, c.slider.p, "bAcceptsFocus", false)?; boolean(a, c.slider.p, "bWindowVisible", false)?; }
            }
        }
        state.controls.retain(|c| c.page.live(a) && c.slider.live(a) && c.device.live(a));
        for c in &mut state.controls {
            let active = stock_menu_extensions::active();
            unsafe { boolean(a, c.slider.p, "bAcceptsFocus", active)?; boolean(a, c.slider.p, "bWindowVisible", active)?; }
            let actual = unsafe { live_scale(a, c.device.p)? };
            let (requested, effective) = unsafe { graphics_scales(a)? };
            if actual != effective { return Err(RuntimeError::new("Renderer scale reflection mismatch")); }
            let unavailable = unavailable.unwrap_or(c.unavailable);
            let refresh = actual != c.session.visible() || (c.failure.is_none() && requested != c.session.committed());
            let changed = refresh || c.unavailable != unavailable;
            if refresh { c.session = Session::with_fallback(requested, actual); }
            if changed { unsafe { display(a, c.slider.p, c.session, unavailable, c.failure)?; } c.unavailable = unavailable; trace(a, "updated", actual, unavailable); }
        }
        Ok(())
    })
}
