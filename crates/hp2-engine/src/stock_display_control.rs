//! Retail sound/video page display-mode control, created through stock scripts.
//! No package bytecode or retail assets are modified.
use std::{cell::RefCell, ffi::c_void, ptr, time::Instant};
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UClass, UConsole, UFunction, UObject, UProperty, UStruct, USDLViewport, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_platform::live_display_mode::{Backend, DisplayMode};
use crate::{stock_display_menu::{Session, CHOICES}, stock_display_transition::ViewportBackend, stock_menu_extensions::{self, PageEvent}};

#[derive(Clone, Copy)]
struct Weak { object: *mut UObject, index: i32 }
impl Weak {
    unsafe fn new(object: *mut UObject) -> Self { Self { object, index: unsafe { (*object).index } } }
    fn live(self, access: CoreAccess<'_>) -> bool {
        self.index >= 0 && !self.object.is_null() && access.object_at(self.index) == self.object.cast()
            && unsafe { (*self.object).object_flags & hp2_core::object_lifecycle::RF_DESTROYED == 0 }
    }
}
struct Control { page: Weak, combo: Weak, viewport: Weak }
struct Dialog { object: Weak, page: Weak }
#[derive(Default)]
struct State { owner: usize, controls: Vec<Control>, requested: Option<(Weak, DisplayMode)>, session: Session, dialog: Option<Dialog>, epoch: Option<Instant> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
pub(crate) fn display_transition_pending() -> bool {
    STATE.with(|cell| cell.try_borrow().map(|state| state.session.pending()).unwrap_or(true))
}
fn now(state: &mut State) -> u64 { state.epoch.get_or_insert_with(Instant::now).elapsed().as_millis().min(u128::from(u64::MAX)) as u64 }
fn owner(access: CoreAccess<'_>) -> RuntimeResult<usize> { access.owner_context::<crate::core_owner::EngineState>().map(|s| s as *const _ as usize).ok_or_else(|| RuntimeError::new("Display menu engine owner unavailable")) }
fn ensure_owner(access: CoreAccess<'_>, state: &mut State) -> RuntimeResult<()> {
    let current = owner(access)?;
    if state.owner != current { *state = State { owner: current, ..State::default() }; }
    Ok(())
}
unsafe fn field(access: CoreAccess<'_>, structure: *mut UStruct, name: &str, kind: &str, width: usize, limit: usize, parameter: bool) -> RuntimeResult<usize> {
    let p = unsafe { access.find_field(structure, access.intern_str(name, FindName::Find)?, access.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new(&format!("Display menu field missing: {name}"))); }
    let off = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Display menu negative offset"))?;
    if unsafe { (*p).array_dim } != 1 || usize::try_from(unsafe { (*p).element_size }).ok() != Some(width)
        || off.checked_add(width).is_none_or(|n| n > limit) || (parameter && unsafe { (*p).property_flags } & 0x80 == 0) {
        return Err(RuntimeError::new(&format!("Invalid display menu field: {name}")));
    }
    Ok(off)
}
unsafe fn function(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { access.find_field((*object).class.cast(), access.intern_str(name, FindName::Find)?, access.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { Err(RuntimeError::new(&format!("Display menu function missing: {name}"))) } else { Ok(f) }
}
unsafe fn property(access: CoreAccess<'_>, object: *mut UObject, name: &str, kind: &str, width: usize) -> RuntimeResult<*mut u8> {
    let class = unsafe { (*object).class.cast::<UClass>() };
    let size = usize::try_from(unsafe { (*class).properties_size }).map_err(|_| RuntimeError::new("Display menu class size"))?;
    let off = unsafe { field(access, class.cast(), name, kind, width, size, false)? };
    Ok(unsafe { object.cast::<u8>().add(off) })
}
unsafe fn obj_prop(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    Ok(unsafe { property(access, object, name, "Core.ObjectProperty", size_of::<*mut c_void>())?.cast::<*mut UObject>().read_unaligned() })
}
fn live(access: CoreAccess<'_>, p: *mut UObject) -> bool { !p.is_null() && access.contains_object(p) }
unsafe fn is_a(access: CoreAccess<'_>, p: *mut UObject, class: *mut UClass) -> bool { live(access, p) && unsafe { hp2_core::object_model::is_a(p.cast(), class.cast()) } }
unsafe fn string(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<String> {
    let a = unsafe { property(access, object, name, "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned() };
    if a.num < 0 || a.max < a.num || (a.num > 0 && a.data.is_null()) { return Err(RuntimeError::new("Invalid display menu string")); }
    if a.num == 0 { return Ok(String::new()); }
    let units = unsafe { std::slice::from_raw_parts(a.data.cast::<u32>(), a.num as usize) };
    if units.last() != Some(&0) || units[..units.len()-1].contains(&0) { return Err(RuntimeError::new("Invalid display menu string terminator")); }
    units[..units.len()-1].iter().copied().map(char::from_u32).collect::<Option<String>>().ok_or_else(|| RuntimeError::new("Invalid display menu string encoding"))
}
unsafe fn set_edit(access: CoreAccess<'_>, combo: *mut UObject, text: &str) -> RuntimeResult<()> {
    let edit = unsafe { obj_prop(access, combo, "EditBox")? };
    if !live(access, edit) { return Err(RuntimeError::new("Display menu edit box unavailable")); }
    if unsafe { string(access, edit, "Value")? } == text { return Ok(()); }
    let p = unsafe { property(access, edit, "Value", "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>() };
    let mut old = unsafe { p.read_unaligned() };
    let result = unsafe { access.write_vm_fstring_result(&mut old, &text.chars().map(|c| c as i32).collect::<Vec<_>>()) };
    unsafe { p.write_unaligned(old); } result
}
unsafe fn bool_prop(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<bool> {
    let class = unsafe { (*object).class };
    let p = unsafe { access.find_field(class.cast(), access.intern_str(name, FindName::Find)?, access.native_class("Core.BoolProperty")?) }.cast::<UBoolProperty>();
    if p.is_null() { return Err(RuntimeError::new("Display menu boolean missing")); }
    let address = unsafe { property(access, object, name, "Core.BoolProperty", 4)? }.cast::<u32>();
    let mask = unsafe { (*p).bit_mask };
    if mask.count_ones() != 1 { return Err(RuntimeError::new("Invalid display menu boolean")); }
    Ok(unsafe { address.read_unaligned() } & mask != 0)
}
unsafe fn write_bool(access: CoreAccess<'_>, object: *mut UObject, name: &str, value: bool) -> RuntimeResult<()> {
    let class = unsafe { (*object).class };
    let p = unsafe { access.find_field(class.cast(), access.intern_str(name, FindName::Find)?, access.native_class("Core.BoolProperty")?) }.cast::<UBoolProperty>();
    if p.is_null() { return Err(RuntimeError::new("Display menu boolean missing")); }
    let address = unsafe { property(access, object, name, "Core.BoolProperty", 4)? }.cast::<u32>();
    let mask = unsafe { (*p).bit_mask };
    if mask.count_ones() != 1 { return Err(RuntimeError::new("Invalid display menu boolean")); }
    let old = unsafe { address.read_unaligned() };
    unsafe { address.write_unaligned(if value { old | mask } else { old & !mask }); } Ok(())
}
// Script frames have zeroed optional arguments and own any FString parameters.
enum Arg<'a> { Str(&'a str), Float(f32), Int(i32), Bool(bool) }
unsafe fn call(access: CoreAccess<'_>, object: *mut UObject, name: &str, args: &[(&str, Arg<'_>)], result: *mut c_void) -> RuntimeResult<()> {
    let f = unsafe { function(access, object, name)? };
    let size = usize::from(unsafe { (*f).parms_size });
    let mut frame = vec![0u64; (size + 7) / 8]; let base = frame.as_mut_ptr().cast::<u8>();
    let mut strings = Vec::new();
    // Validate every parameter before allocating any owned string.
    let mut offsets = Vec::new();
    for (name, arg) in args {
        let (kind, width) = match arg { Arg::Str(_) => ("Core.StrProperty", size_of::<TArrayRaw>()), Arg::Float(_) => ("Core.FloatProperty", 4), Arg::Int(_) => ("Core.IntProperty", 4), Arg::Bool(_) => ("Core.BoolProperty", 4) };
        offsets.push(unsafe { field(access, f.cast(), name, kind, width, size, true)? });
    }
    let outcome = (|| -> RuntimeResult<()> {
        for ((name, arg), &off) in args.iter().zip(&offsets) {
            unsafe { match arg {
                Arg::Str(s) => { let mut value: TArrayRaw = std::mem::zeroed(); access.assign_fstring(&mut value, &s.chars().map(u32::from).chain([0]).collect::<Vec<_>>())?; base.add(off).cast::<TArrayRaw>().write_unaligned(value); strings.push(off); }
                Arg::Float(v) => base.add(off).cast::<f32>().write_unaligned(*v),
                Arg::Int(v) => base.add(off).cast::<i32>().write_unaligned(*v),
                Arg::Bool(v) => {
                    let p = access.find_field(f.cast(), access.intern_str(name, FindName::Find)?, access.native_class("Core.BoolProperty")?).cast::<UBoolProperty>();
                    let mask = (*p).bit_mask;
                    if mask.count_ones() != 1 { return Err(RuntimeError::new("Invalid display menu argument mask")); }
                    let slot = base.add(off).cast::<u32>(); let old = slot.read_unaligned();
                    slot.write_unaligned(if *v { old | mask } else { old & !mask });
                },
            }}
        }
        unsafe { access.execute_event(object, f, base.cast(), result) }
    })();
    let mut cleanup_error = None;
    for off in strings { let mut value = unsafe { base.add(off).cast::<TArrayRaw>().read_unaligned() }; if let Err(e) = unsafe { access.destroy_owned_fstring(&mut value) } { cleanup_error = Some(e); } }
    outcome?; if let Some(e) = cleanup_error { return Err(e); } Ok(())
}
unsafe fn viewport_for_page(access: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<*mut USDLViewport> {
    let root = unsafe { obj_prop(access, page, "Root")? };
    if !live(access, root) { return Err(RuntimeError::new("Display menu root unavailable")); }
    let console = unsafe { obj_prop(access, root, "Console")? };
    if !live(access, console) || !unsafe { hp2_core::object_model::is_a(console.cast(), access.native_class("Engine.Console")?.cast()) } { return Err(RuntimeError::new("Display menu console unavailable")); }
    let viewport = unsafe { (*console.cast::<UConsole>()).viewport.cast::<UViewport>() };
    if !live(access, viewport.cast()) || unsafe { (*viewport).console.cast::<UObject>() } != console
        || !unsafe { hp2_core::object_model::is_a(viewport.cast(), access.native_class("SDLDrv.SDLViewport")?.cast()) } { return Err(RuntimeError::new("Display menu viewport unavailable")); }
    Ok(viewport.cast())
}
unsafe fn current(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<DisplayMode> {
    unsafe { ViewportBackend::new(access, viewport)? }.current()
}
fn label(mode: DisplayMode) -> &'static str { CHOICES.iter().find(|(m,_)| *m == mode).map(|(_, s)| *s).unwrap() }
unsafe fn create(access: CoreAccess<'_>, page: *mut UObject, state: &mut State) -> RuntimeResult<()> {
    if state.controls.iter().any(|c| c.page.object == page && c.page.live(access) && c.combo.live(access)) { return Ok(()); }
    let viewport = unsafe { viewport_for_page(access, page)? };
    let mode = unsafe { current(access, viewport)? };
    let class = unsafe { access.static_load_class(access.native_class("Core.Object")?, ptr::null_mut(), &"HGame.HPMenuOptionCombo\0".chars().map(u32::from).collect::<Vec<_>>(), 2)? };
    if class.is_null() { return Err(RuntimeError::new("Display menu combo class unavailable")); }
    let f = unsafe { function(access, page, "CreateControl")? };
    let size = usize::from(unsafe { (*f).parms_size }); let mut frame = vec![0u64; (size + 7) / 8]; let p = frame.as_mut_ptr().cast::<u8>();
    let a = |name, kind, width| unsafe { field(access, f.cast(), name, kind, width, size, true) };
    let cl = a("ControlClass", "Core.ClassProperty", size_of::<*mut c_void>())?;
    let x = a("X", "Core.FloatProperty", 4)?; let y = a("Y", "Core.FloatProperty", 4)?; let w = a("W", "Core.FloatProperty", 4)?; let h = a("H", "Core.FloatProperty", 4)?;
    a("ReturnValue", "Core.ObjectProperty", size_of::<*mut c_void>())?;
    unsafe { p.add(cl).cast::<*mut UClass>().write_unaligned(class); for (off, v) in [(x, 326f32), (y, 214f32), (w, 280f32), (h, 24f32)] { p.add(off).cast::<f32>().write_unaligned(v); } }
    let mut combo: *mut UObject = ptr::null_mut();
    unsafe { access.execute_event(page, f, p.cast(), (&raw mut combo).cast())?; }
    if !unsafe { is_a(access, combo, class) } { return Err(RuntimeError::new("Display menu CreateControl failed")); }
    // Do not expose an incomplete control if any script call fails.
    unsafe { write_bool(access, combo, "bAcceptsFocus", false)?; write_bool(access, combo, "bWindowVisible", false)?; }
    unsafe { call(access, combo, "SetText", &[("NewText", Arg::Str("Display"))], ptr::null_mut())?; }
    unsafe { call(access, combo, "SetEditable", &[("bNewCanEdit", Arg::Bool(false))], ptr::null_mut())?; }
    let source = unsafe { obj_prop(access, page, "ResolutionCombo")? };
    if unsafe { is_a(access, source, class) } {
        // SDL SetRes changes logical render/window dimensions, not the
        // exclusive physical display mode. Make that distinction visible.
        unsafe { call(access, source, "SetText", &[("NewText", Arg::Str("Logical res."))], ptr::null_mut())?; }
        let font = unsafe { property(access, source, "Font", "Core.IntProperty", 4)?.cast::<i32>().read_unaligned() };
        unsafe { call(access, combo, "SetFont", &[("NewFont", Arg::Int(font))], ptr::null_mut())?; }
        let color = unsafe { property(access, source, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().read_unaligned() };
        unsafe { property(access, combo, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().write_unaligned(color); }
        let source_edit = unsafe { obj_prop(access, source, "EditBox")? };
        let edit = unsafe { obj_prop(access, combo, "EditBox")? };
        if live(access, source_edit) && live(access, edit) {
            let color = unsafe { property(access, source_edit, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().read_unaligned() };
            unsafe { property(access, edit, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().write_unaligned(color); }
        }
    }
    for (_, text) in CHOICES { unsafe { call(access, combo, "AddItem", &[("S", Arg::Str(text))], ptr::null_mut())?; } }
    unsafe {
        property(access, combo, "EditBoxWidth", "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(180.0);
        // Retail combos are TA_Right, which misplaces their edit box, button and list; see stock_graphics_menu::align_left.
        property(access, combo, "Align", "Core.ByteProperty", 1)?.write_unaligned(0);
        // This control is created after the stock page's Created callback. Its
        // edit child can retain the constructor's full-width geometry instead
        // of the stock controls' laid-out geometry. Match the displayed edit
        // area explicitly so its text cannot overlap the label.
        let edit = obj_prop(access, combo, "EditBox")?;
        if !live(access, edit) || !hp2_core::object_model::is_a(edit.cast(), access.native_class("UWindow.UWindowEditBox")?.cast()) {
            return Err(RuntimeError::new("Display menu edit box unavailable"));
        }
        for (field, value) in [("WinLeft", 100.0f32), ("WinTop", 0.0), ("WinWidth", 180.0), ("WinHeight", 24.0)] {
            property(access, edit, field, "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(value);
        }
        set_edit(access, combo, label(mode))?;
    }
    state.controls.retain(|c| c.page.live(access) && c.combo.live(access));
    state.controls.push(Control { page: unsafe { Weak::new(page) }, combo: unsafe { Weak::new(combo) }, viewport: unsafe { Weak::new(viewport.cast()) } });
    trace(access, &format!("created page={} control={} mode={mode:?}", unsafe { (*page).index }, unsafe { (*combo).index }));
    unsafe { write_bool(access, combo, "bAcceptsFocus", true)?; write_bool(access, combo, "bWindowVisible", true)?; }
    Ok(())
}
unsafe fn callback(access: CoreAccess<'_>, page: *mut UObject, event: PageEvent, params: *const u8, size: usize) -> RuntimeResult<()> {
    STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else { return Ok(()) };
        ensure_owner(access, &mut state)?;
        if event == PageEvent::Created { return unsafe { create(access, page, &mut state) }; }
        let f = unsafe { function(access, page, "Notify")? }; let expected = usize::from(unsafe { (*f).parms_size });
        if params.is_null() || size < expected { return Ok(()); }
        let c = unsafe { field(access, f.cast(), "C", "Core.ObjectProperty", size_of::<*mut c_void>(), expected, true)? };
        let e = unsafe { field(access, f.cast(), "E", "Core.ByteProperty", 1, expected, true)? };
        if unsafe { *params.add(e) } != 1 { return Ok(()); }
        let combo = unsafe { params.add(c).cast::<*mut UObject>().read_unaligned() };
        let Some(control) = state.controls.iter().find(|c| c.page.object == page && c.combo.object == combo && c.page.live(access) && c.combo.live(access) && c.viewport.live(access)) else { return Ok(()) };
        let viewport = control.viewport;
        let edit = unsafe { obj_prop(access, combo, "EditBox")? }; if !live(access, edit) { return Ok(()); }
        let value = unsafe { string(access, edit, "Value")? };
        let Some((mode, _)) = CHOICES.iter().find(|(_, label)| *label == value) else { return Err(RuntimeError::new("Unknown display mode selection")); };
        if state.session.pending() { return Ok(()); }
        state.requested = Some((viewport, *mode)); Ok(())
    })
}
/// Install once per engine tick, before the stock script hook runs.
/// The page's added display-mode combo and its choices.
pub(crate) fn combo_of(page: *mut UObject) -> Option<*mut UObject> {
    STATE.with(|cell| cell.try_borrow().ok()?.controls.iter().find(|c| c.page.object == page).map(|c| c.combo.object))
}
pub(crate) fn choice_labels() -> Vec<&'static str> { CHOICES.iter().map(|(_, label)| *label).collect() }
pub(crate) fn install() -> RuntimeResult<stock_menu_extensions::Guard> { stock_menu_extensions::install("display-mode", callback) }

unsafe fn prompt(access: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<Weak> {
    let mut dialog: *mut UObject = ptr::null_mut();
    unsafe { call(access, page, "doHPMessageBox", &[("msg", Arg::Str("Keep the new display mode?")), ("textButton1", Arg::Str("Yes")), ("textButton2", Arg::Str("No")), ("timeOut", Arg::Float(20.0))], (&raw mut dialog).cast())?; }
    if !live(access, dialog) { return Err(RuntimeError::new("Display confirmation dialog unavailable")); }
    Ok(unsafe { Weak::new(dialog) })
}
unsafe fn notice(access: CoreAccess<'_>, page: *mut UObject, message: &str) -> RuntimeResult<()> {
    let mut dialog: *mut UObject = ptr::null_mut();
    unsafe { call(access, page, "doHPMessageBox", &[("msg", Arg::Str(message)), ("textButton1", Arg::Str("Okay"))], (&raw mut dialog).cast())?; }
    if !live(access, dialog) { return Err(RuntimeError::new("Display error dialog unavailable")); }
    Ok(())
}
fn trace(access: CoreAccess<'_>, message: &str) {
    if std::env::var_os("HP2_STOCK_DISPLAY_TRACE").as_deref() == Some(std::ffi::OsStr::new("1")) {
        if let Ok(category) = access.intern_str("Log", FindName::Add) { let _ = access.emit_log(category, &format!("StockDisplayProbe {message}").chars().map(u32::from).collect::<Vec<_>>()); }
    }
}
fn warn(access: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = access.intern_str("Warning", FindName::Add) { let _ = access.emit_log(category, &format!("Display mode: {error}").chars().map(u32::from).collect::<Vec<_>>()); }
}
/// Run after every engine tick, including ticks where the stock hook was absent.
/// Errors are reported and pending rollback is retained for a later tick.
pub(crate) unsafe fn tick(access: CoreAccess<'_>) -> RuntimeResult<()> {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut(); ensure_owner(access, &mut state)?;
        let active = stock_menu_extensions::active();
        state.controls.retain(|c| c.page.live(access) && c.combo.live(access) && c.viewport.live(access));
        for c in &state.controls {
            // The popup list belongs to Root, not to the combo. Close it
            // before hiding the control when event delivery is unavailable.
            if !active && unsafe { bool_prop(access, c.combo.object, "bListVisible")? } {
                unsafe { call(access, c.combo.object, "CloseUpWithNoSound", &[], ptr::null_mut())?; }
            }
            unsafe { write_bool(access, c.combo.object, "bAcceptsFocus", active)?; write_bool(access, c.combo.object, "bWindowVisible", active)?; }
        }
        let now = now(&mut state);
        if state.session.pending() {
            let answer = if let Some(d) = &state.dialog {
                if !d.object.live(access) || !d.page.live(access) { Some((false, "dialog lost")) }
                else if unsafe { bool_prop(access, d.object.object, "bClosing")? } {
                    let result = unsafe { string(access, d.object.object, "Result")? };
                    Some((result == "Yes", if result.is_empty() { "dialog timeout restored" } else { "cancelled" }))
                } else { None }
            } else { None };
            if let Some((keep, reason)) = answer {
                let result = if keep { unsafe { state.session.confirm(access, now) } } else { unsafe { state.session.cancel(access) } };
                let succeeded = result.is_ok();
                if let Err(e) = result { warn(access, &e); }
                if !state.session.pending() {
                    if succeeded { trace(access, if keep { "confirmed" } else { reason }); }
                    state.dialog = None;
                }
            }
            if state.session.pending() {
                match unsafe { state.session.tick(access, now) } {
                    Ok(true) => {
                        trace(access, "timeout restored");
                        if let Some(d) = &state.dialog {
                            if d.object.live(access) && !unsafe { bool_prop(access, d.object.object, "bClosing")? } {
                                if let Err(e) = unsafe { call(access, d.object.object, "Close", &[], ptr::null_mut()) } { warn(access, &e); }
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(e) => warn(access, &e),
                }
                if !state.session.pending() {
                    if let Some(d) = &state.dialog {
                        if d.object.live(access) && !unsafe { bool_prop(access, d.object.object, "bClosing")? } {
                            if let Err(e) = unsafe { call(access, d.object.object, "Close", &[], ptr::null_mut()) } { warn(access, &e); }
                        }
                    }
                    state.dialog = None;
                }
            }
        }
        if !state.session.pending() {
            if let Some((vp, mode)) = state.requested.take() {
                let page = state.controls.iter().find(|c| c.viewport.object == vp.object && c.page.live(access)).map(|c| c.page);
                if vp.live(access) && unsafe { current(access, vp.object.cast())? } != mode {
                    match unsafe { state.session.begin(access, vp.object.cast(), mode, now) } {
                        Ok(()) => if let Some(page) = page {
                            trace(access, &format!("applied mode={mode:?}"));
                            match unsafe { prompt(access, page.object) } {
                                Ok(object) => state.dialog = Some(Dialog { object, page }),
                                Err(e) => { warn(access, &e); if let Err(e) = unsafe { state.session.cancel(access) } { warn(access, &e); } }
                            }
                        } else if let Err(e) = unsafe { state.session.cancel(access) } { warn(access, &e); },
                        Err(e) => {
                            warn(access, &e);
                            if let Some(page) = page {
                                let message = if state.session.pending() { "Display mode failed. The previous mode is being restored." }
                                    else { "Display mode is unavailable. The previous mode was restored." };
                                if let Err(error) = unsafe { notice(access, page.object, message) } { warn(access, &error); }
                            }
                        }
                    }
                }
            }
        }
        for c in &state.controls {
            if let Ok(mode) = unsafe { current(access, c.viewport.object.cast()) } { unsafe { set_edit(access, c.combo.object, label(mode))?; } }
        }
        Ok(())
    })
}
