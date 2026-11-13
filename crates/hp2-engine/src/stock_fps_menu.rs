//! Adapter for the compiled stock sound/video page. No package bytes are changed.
//!
//! The hook is installed only while the engine tick runs. All metadata and
//! controls are weak identities belonging to the EngineOwner; callbacks must
//! revalidate them before use. Unsupported package layouts leave stock UI alone.
use std::{cell::RefCell, ffi::c_void, ptr};
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UClass, UClient, UFunction, UObject, UProperty, UStruct};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::{settings_config, settings_policy::SettingsPatch};
#[cfg(test)]
#[path = "stock_fps_menu_tests.rs"]
mod tests;

#[derive(Default)]
pub(crate) struct State { bindings: Option<Bindings>, controls: Vec<Control>, resolution_warned: bool, hook_warned: bool, suspended: bool,
    probe_tick: u64, probe_last: Option<(i32, i32, bool, bool, bool, bool, usize)> }
#[derive(Clone, Copy)]
struct Control { page: *mut UObject, page_index: i32, control: *mut UObject, control_index: i32 }
#[derive(Clone, Copy)]
struct Bindings {
    page: *mut UClass, checkbox: *mut UClass, label: *mut UClass, dialog: *mut UClass,
    created: *mut UFunction, notify: *mut UFunction, create: *mut UFunction, load_settings: *mut UFunction,
    indices: [i32; 8],
    class_arg: usize, x: usize, y: usize, w: usize, h: usize, owner: usize,
    notify_control: usize, notify_event: usize,
    checked: (usize, u32), disabled: (usize, u32), visible: (usize, u32), focused: (usize, u32), page_visible: (usize, u32), text: usize,
    font: usize, color: usize, source_font: usize, source_color: usize, source_label: usize,
    tab_last: usize, tab_next: usize, tab_prev: usize, notify_window: usize, accepts_focus: (usize, u32),
}

unsafe fn field(access: CoreAccess<'_>, structure: *mut UStruct, name: &str, kind: &str) -> RuntimeResult<*mut UProperty> {
    let name = access.intern_str(name, FindName::Find)?;
    let p = unsafe { access.find_field(structure, name, access.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new("Stock FPS field missing")); }
    Ok(p)
}
unsafe fn slot(access: CoreAccess<'_>, structure: *mut UStruct, size: usize, name: &str, kind: &str, width: usize, param: bool) -> RuntimeResult<usize> {
    let p = unsafe { field(access, structure, name, kind)? };
    let (offset, len, count, flags) = unsafe { ((*p).offset, (*p).element_size, (*p).array_dim, (*p).property_flags) };
    let offset = usize::try_from(offset).map_err(|_| RuntimeError::new("Stock FPS negative offset"))?;
    if count != 1 || usize::try_from(len).ok() != Some(width) || offset.checked_add(width).is_none_or(|n| n > size) || (param && flags & 0x80 == 0) {
        return Err(RuntimeError::new("Stock FPS incompatible field layout"));
    }
    Ok(offset)
}
unsafe fn boolean(access: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<(usize, u32)> {
    let size = usize::try_from(unsafe { (*class).properties_size }).map_err(|_| RuntimeError::new("Stock FPS class size"))?;
    let offset = unsafe { slot(access, class.cast(), size, name, "Core.BoolProperty", 4, false)? };
    let property = unsafe { field(access, class.cast(), name, "Core.BoolProperty")? }.cast::<UBoolProperty>();
    let mask = unsafe { (*property).bit_mask };
    if mask.count_ones() != 1 { return Err(RuntimeError::new("Stock FPS invalid boolean mask")); }
    Ok((offset, mask))
}
unsafe fn function(access: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<*mut UFunction> {
    Ok(unsafe { field(access, class.cast(), name, "Core.Function")? }.cast())
}
unsafe fn resolve(access: CoreAccess<'_>) -> RuntimeResult<Bindings> {
    let load = |name: &str| -> RuntimeResult<*mut UClass> {
        let path: Vec<u32> = name.chars().map(u32::from).chain([0]).collect();
        let class = unsafe { access.static_load_class(access.native_class("Core.Object")?, ptr::null_mut(), &path, 2)? };
        if class.is_null() { Err(RuntimeError::new("Stock FPS class missing")) } else { Ok(class) }
    };
    let page = load("HGame.FESoundVideoPage")?;
    let checkbox = load("HGame.HPMenuOptionCheckBox")?;
    let label = load("UWindow.UWindowLabelControl")?;
    let dialog = load("UWindow.UWindowDialogControl")?;
    let created = unsafe { function(access, page, "Created")? };
    let notify = unsafe { function(access, page, "Notify")? };
    let create = unsafe { function(access, page, "CreateControl")? };
    let load_settings = unsafe { function(access, page, "LoadAvailableSettings")? };
    let cs = usize::from(unsafe { (*create).parms_size });
    let ns = usize::from(unsafe { (*notify).parms_size });
    let ps = usize::try_from(unsafe { (*page).properties_size }).map_err(|_| RuntimeError::new("Stock FPS page size"))?;
    let bs = usize::try_from(unsafe { (*checkbox).properties_size }).map_err(|_| RuntimeError::new("Stock FPS checkbox size"))?;
    let ls = usize::try_from(unsafe { (*label).properties_size }).map_err(|_| RuntimeError::new("Stock FPS label size"))?;
    let ds = usize::try_from(unsafe { (*dialog).properties_size }).map_err(|_| RuntimeError::new("Stock FPS dialog size"))?;
    let arg = |name, kind, width| unsafe { slot(access, create.cast(), cs, name, kind, width, true) };
    let prop = |class: *mut UClass, size, name, kind, width| unsafe { slot(access, class.cast(), size, name, kind, width, false) };
    let pointer = std::mem::size_of::<*mut c_void>();
    // Check the return slot as well, even though execute_event writes to `result`.
    unsafe { slot(access, create.cast(), cs, "ReturnValue", "Core.ObjectProperty", pointer, true)?; }
    Ok(Bindings {
        page, checkbox, label, dialog, created, notify, create, load_settings,
        indices: unsafe { [(*page.cast::<UObject>()).index, (*checkbox.cast::<UObject>()).index,
            (*label.cast::<UObject>()).index, (*created.cast::<UObject>()).index,
            (*notify.cast::<UObject>()).index, (*create.cast::<UObject>()).index, (*dialog.cast::<UObject>()).index, (*load_settings.cast::<UObject>()).index] },
        class_arg: arg("ControlClass", "Core.ClassProperty", pointer)?,
        x: arg("X", "Core.FloatProperty", 4)?, y: arg("Y", "Core.FloatProperty", 4)?,
        w: arg("W", "Core.FloatProperty", 4)?, h: arg("H", "Core.FloatProperty", 4)?,
        owner: arg("OwnerWindow", "Core.ObjectProperty", pointer)?,
        notify_control: unsafe { slot(access, notify.cast(), ns, "C", "Core.ObjectProperty", pointer, true)? },
        notify_event: unsafe { slot(access, notify.cast(), ns, "E", "Core.ByteProperty", 1, true)? },
        checked: unsafe { boolean(access, checkbox, "bChecked")? },
        disabled: unsafe { boolean(access, checkbox, "bDisabled")? },
        visible: unsafe { boolean(access, checkbox, "bWindowVisible")? },
        focused: unsafe { boolean(access, checkbox, "bHasKeyboardFocus")? },
        page_visible: unsafe { boolean(access, page, "bWindowVisible")? },
        text: prop(checkbox, bs, "Text", "Core.StrProperty", std::mem::size_of::<TArrayRaw>())?,
        font: prop(checkbox, bs, "Font", "Core.IntProperty", 4)?,
        color: prop(checkbox, bs, "TextColor", "Core.StructProperty", 4)?,
        source_label: prop(page, ps, "MusicVolumeLabel", "Core.ObjectProperty", pointer)?,
        source_font: prop(label, ls, "Font", "Core.IntProperty", 4)?,
        source_color: prop(label, ls, "TextColor", "Core.StructProperty", 4)?,
        tab_last: prop(page, ps, "TabLast", "Core.ObjectProperty", pointer)?,
        tab_next: prop(dialog, ds, "TabNext", "Core.ObjectProperty", pointer)?,
        tab_prev: prop(dialog, ds, "TabPrev", "Core.ObjectProperty", pointer)?,
        notify_window: prop(dialog, ds, "NotifyWindow", "Core.ObjectProperty", pointer)?,
        accepts_focus: unsafe { boolean(access, dialog, "bAcceptsFocus")? },
    })
}

unsafe fn live(access: CoreAccess<'_>, object: *mut UObject, index: i32) -> bool {
    index >= 0 && access.object_at(index) == object && !object.is_null()
}
unsafe fn read_bool(object: *mut UObject, field: (usize, u32)) -> bool {
    unsafe { ptr::read_unaligned(object.cast::<u8>().add(field.0).cast::<u32>()) & field.1 != 0 }
}
unsafe fn write_bool(object: *mut UObject, field: (usize, u32), value: bool) {
    let p = unsafe { object.cast::<u8>().add(field.0).cast::<u32>() };
    let old = unsafe { ptr::read_unaligned(p) };
    unsafe { ptr::write_unaligned(p, if value { old | field.1 } else { old & !field.1 }) };
}
/// Move the added control immediately before the next focusable stock control.
/// The relative order of every stock control remains unchanged.
fn tab_order(order: &[usize], added: usize, focusable: impl Fn(usize) -> bool) -> Option<Vec<usize>> {
    let pos = order.iter().position(|&item| item == added)?;
    if order.iter().filter(|&&item| item == added).count() != 1 { return None; }
    let target = (1..order.len()).map(|step| order[(pos + step) % order.len()]).find(|&item| focusable(item))?;
    let mut result = order.to_vec();
    result.remove(pos);
    let target_pos = result.iter().position(|&item| item == target)?;
    result.insert(target_pos, added);
    Some(result)
}

unsafe fn fix_tab_chain(access: CoreAccess<'_>, b: Bindings, page: *mut UObject, control: *mut UObject) -> RuntimeResult<()> {
    let pointer = |object: *mut UObject, offset: usize| unsafe { ptr::read_unaligned(object.cast::<u8>().add(offset).cast::<*mut UObject>()) };
    let set_pointer = |object: *mut UObject, offset: usize, value: *mut UObject| unsafe { ptr::write_unaligned(object.cast::<u8>().add(offset).cast::<*mut UObject>(), value) };
    let valid = |object: *mut UObject| access.contains_object(object) && unsafe { hp2_core::object_model::is_a(object.cast(), b.dialog.cast()) };
    let last = pointer(page, b.tab_last);
    if !valid(last) { return Err(RuntimeError::new("Stock FPS tab tail is invalid")); }
    let first = pointer(last, b.tab_next);
    let mut order = Vec::new();
    let mut current = first;
    loop {
        if !valid(current) || pointer(current, b.notify_window) != page || order.len() >= 4096 {
            return Err(RuntimeError::new("Stock FPS tab chain is invalid"));
        }
        if order.contains(&(current as usize)) {
            if current != first { return Err(RuntimeError::new("Stock FPS tab chain has a cycle")); }
            break;
        }
        order.push(current as usize);
        current = pointer(current, b.tab_next);
    }
    if order.last().copied() != Some(last as usize) { return Err(RuntimeError::new("Stock FPS tab tail does not match chain")); }
    for (i, &item) in order.iter().enumerate() {
        let previous = order[(i + order.len() - 1) % order.len()] as *mut UObject;
        if pointer(item as *mut UObject, b.tab_prev) != previous { return Err(RuntimeError::new("Stock FPS tab links are inconsistent")); }
    }
    let reordered = tab_order(&order, control as usize, |item| unsafe {
        read_bool(item as *mut UObject, b.accepts_focus) && read_bool(item as *mut UObject, b.visible)
    }).ok_or_else(|| RuntimeError::new("Stock FPS tab chain has no focusable destination"))?;
    for (i, &item) in reordered.iter().enumerate() {
        let object = item as *mut UObject;
        set_pointer(object, b.tab_prev, reordered[(i + reordered.len() - 1) % reordered.len()] as *mut UObject);
        set_pointer(object, b.tab_next, reordered[(i + 1) % reordered.len()] as *mut UObject);
    }
    set_pointer(page, b.tab_last, *reordered.last().unwrap() as *mut UObject);
    Ok(())
}

fn state(access: CoreAccess<'_>) -> Option<&RefCell<State>> {
    access.owner_context::<crate::core_owner::EngineState>().map(|s| &s.stock_fps_menu)
}

/// The page's added Show FPS checkbox.
pub(crate) fn checkbox_of(access: CoreAccess<'_>, page: *mut UObject) -> Option<*mut UObject> {
    state(access)?.try_borrow().ok()?.controls.iter().find(|c| c.page == page).map(|c| c.control)
}

/// Install for one complete engine tick, including SDL input and window creation.
/// The guard must be dropped before Core or its packages are shut down.
pub(crate) unsafe fn install(access: CoreAccess<'_>) -> Option<impl Drop + '_> {
    let state = state(access)?;
    let cached = { state.borrow().bindings }.filter(|b| {
        let objects = [b.page.cast::<UObject>(), b.checkbox.cast(), b.label.cast(),
            b.created.cast(), b.notify.cast(), b.create.cast(), b.dialog.cast(), b.load_settings.cast()];
        objects.into_iter().zip(b.indices).all(|(object, index)| unsafe { live(access, object, index) })
    });
    if cached.is_none() {
        let mut state = state.borrow_mut();
        if let Some(old) = state.bindings {
            // If only a function was unloaded, hide any surviving controls
            // before forgetting their routing identities.
            if unsafe { live(access, old.checkbox.cast(), old.indices[1]) } {
                for c in &state.controls {
                    if unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) && hp2_core::object_model::is_a(c.control.cast(), old.checkbox.cast()) } {
                        unsafe { write_bool(c.control, old.disabled, true); write_bool(c.control, old.visible, false); }
                    }
                }
            }
        }
        state.bindings = None;
        state.controls.clear();
    }
    let bindings = match cached {
        Some(b) => b,
        None => {
            // Do not load the retail package as a side effect of ticking a
            // prototype or an unrelated application.
            let name = access.intern_str("HGame", FindName::Find).ok()?;
            let package = unsafe { access.find_object(access.native_class("Core.Package").ok()?, ptr::null_mut(), name, true) };
            if package.is_null() { return None; }
            let b = match unsafe { resolve(access) } {
                Ok(b) => b,
                Err(error) => {
                    let mut state = state.borrow_mut();
                    if !state.resolution_warned { log_error(access, &error); state.resolution_warned = true; }
                    return None;
                }
            };
            state.borrow_mut().bindings = Some(b);
            b
        }
    };
    match unsafe { access.install_script_post_call_hook_three([bindings.created, bindings.notify, bindings.load_settings], callback) } {
        Ok(guard) => {
            if state.borrow().suspended {
                // The hook is active again. Only show controls if the owning
                // client and configuration are available for a real action.
                let client = access.owner_context::<crate::core_owner::EngineState>().and_then(|s| s.bitmap_client.get());
                if let Some(client) = client.filter(|c| unsafe { live(access, c.as_ptr().cast(), (*c.as_ptr().cast::<UObject>()).index) }) {
                    if settings_config::read(access).is_ok() {
                        let controls = state.borrow().controls.clone();
                        for c in controls {
                            if unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) } {
                                unsafe { write_bool(c.control, bindings.checked, (*client.as_ptr()).show_fps()); write_bool(c.control, bindings.disabled, false); write_bool(c.control, bindings.visible, true); }
                            }
                        }
                        state.borrow_mut().suspended = false;
                    }
                }
            }
            Some(guard)
        }
        Err(error) => {
            let mut state = state.borrow_mut();
            if !state.hook_warned { log_error(access, &error); state.hook_warned = true; }
            // A visible checkbox without its Notify hook would lie to users.
            for c in &state.controls {
                if unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) } {
                    unsafe { write_bool(c.control, bindings.disabled, true); write_bool(c.control, bindings.visible, false); }
                }
            }
            state.suspended = true;
            None
        }
    }
}

/// Optional engine-thread observation for retail replay evidence. It does not
/// change focus or settings, and emits a log record only when state changes.
pub(crate) unsafe fn probe(access: CoreAccess<'_>) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("HP2_STOCK_FPS_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))) { return; }
    let Some(state) = state(access) else { return };
    let mut state = state.borrow_mut();
    state.probe_tick += 1;
    let Some(b) = state.bindings else { return };
    let Some(client) = access.owner_context::<crate::core_owner::EngineState>().and_then(|s| s.bitmap_client.get()) else { return };
    if !unsafe { live(access, client.as_ptr().cast(), (*client.as_ptr().cast::<UObject>()).index) } { return; }
    let Some(c) = state.controls.iter().find(|c| unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) }) else { return };
    let snapshot = (c.page_index, c.control_index,
        unsafe { read_bool(c.page, b.page_visible) && read_bool(c.control, b.visible) },
        unsafe { read_bool(c.control, b.checked) }, unsafe { read_bool(c.control, b.focused) },
        unsafe { (*client.as_ptr()).show_fps() }, state.controls.iter().filter(|other| other.page == c.page && unsafe { live(access, other.control, other.control_index) }).count());
    if state.probe_last == Some(snapshot) { return; }
    state.probe_last = Some(snapshot);
    let message = format!("StockFPSProbe tick={} page={} control={} visible={} checked={} focus={} fps={} count={}",
        state.probe_tick, snapshot.0, snapshot.1, snapshot.2, snapshot.3, snapshot.4, snapshot.5, snapshot.6);
    if let Ok(category) = access.intern_str("Log", FindName::Add) {
        let _ = access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>());
    }
}

fn log_error(access: CoreAccess<'_>, error: &impl std::fmt::Display) {
    let message: Vec<u32> = format!("Stock FPS menu: {error}").chars().map(u32::from).collect();
    if let Ok(category) = access.intern_str("Warning", FindName::Add) {
        // Logging is best effort; a failed log must not abort the stock menu.
        let _ = access.emit_log(category, &message);
    }
}

fn label(access: CoreAccess<'_>) -> RuntimeResult<Vec<u32>> {
    // Distributors can supply HP2Menu.<language> [StockMenu] ShowFPS. Core
    // falls back to .int; the built-in English text covers missing translations.
    let mut text = access.localize_text(hp2_core::lit!("StockMenu"), hp2_core::lit!("ShowFPS"), Some(hp2_core::lit!("HP2Menu")), None, true)?;
    if text.is_empty() { text.extend("Show FPS".chars().map(u32::from)); }
    text.push(0);
    Ok(text)
}

unsafe fn commit_selection(access: CoreAccess<'_>, client: *mut UClient, control: *mut UObject, checked: (usize, u32)) -> RuntimeResult<()> {
    let value = unsafe { read_bool(control, checked) };
    match settings_config::apply(access, &SettingsPatch { show_fps: Some(value), ..Default::default() }) {
        Ok(_) => unsafe { (*client).set_show_fps(value); },
        Err(error) => {
            unsafe { write_bool(control, checked, (*client).show_fps()); }
            return Err(RuntimeError::new(&format!("Could not save ShowFPS: {error:?}")));
        }
    }
    Ok(())
}

unsafe fn callback(access: CoreAccess<'_>, page: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if let Err(error) = unsafe { callback_inner(access, page, function, params, size) } { log_error(access, &error); }
    // Independent controls share this hook but own their state and validation.
    let bindings = state(access).and_then(|s| s.borrow().bindings);
    if let Some(b) = bindings {
        let event = if function == b.created { Some(crate::stock_menu_extensions::PageEvent::Created) }
            else if function == b.notify { Some(crate::stock_menu_extensions::PageEvent::Notify) } else { None };
        if let Some(event) = event {
            if !page.is_null() && unsafe { live(access, page, (*page).index) && hp2_core::object_model::is_a(page.cast(), b.page.cast()) } {
                if let Err(error) = unsafe { crate::stock_menu_extensions::dispatch(access, page, event, params, size) } { log_error(access, &error); }
            }
        }
    }
    Ok(())
}
unsafe fn callback_inner(access: CoreAccess<'_>, page: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    let Some(state) = state(access) else { return Ok(()) };
    let Some(b) = state.borrow().bindings else { return Ok(()) };
    if page.is_null() || !unsafe { live(access, page, (*page).index) } || !unsafe { hp2_core::object_model::is_a(page.cast(), b.page.cast()) } { return Ok(()); }
    let Some(client) = access.owner_context::<crate::core_owner::EngineState>().and_then(|s| s.bitmap_client.get()) else { return Ok(()) };
    let client = client.as_ptr();
    if !unsafe { live(access, client.cast(), (*client.cast::<UObject>()).index) } { return Ok(()); }
    if function == b.load_settings {
        unsafe { crate::stock_resolution_menu::populate(access, page)?; }
    } else if function == b.created {
        let page_index = unsafe { (*page).index };
        if state.borrow().controls.iter().any(|c| c.page == page && c.page_index == page_index && unsafe { live(access, c.control, c.control_index) }) { return Ok(()); }
        // Validate all fallible inputs before registering a stock control.
        settings_config::read(access)?;
        let text = label(access)?;
        let n = usize::from(unsafe { (*b.create).parms_size });
        let mut frame = vec![0u64; (n + 7) / 8];
        let p = frame.as_mut_ptr().cast::<u8>();
        unsafe {
            ptr::write_unaligned(p.add(b.class_arg).cast::<*mut UClass>(), b.checkbox);
            for (offset, value) in [(b.x, 473.0f32), (b.y, 246.0), (b.w, 133.0), (b.h, 1.0)] { ptr::write_unaligned(p.add(offset).cast::<f32>(), value); }
            ptr::write_unaligned(p.add(b.owner).cast::<*mut UObject>(), ptr::null_mut());
        }
        let mut control: *mut UObject = ptr::null_mut();
        unsafe { access.execute_event(page, b.create, p.cast(), (&mut control as *mut *mut UObject).cast())?; }
        if control.is_null() || !unsafe { live(access, control, (*control).index) } || !unsafe { hp2_core::object_model::is_a(control.cast(), b.checkbox.cast()) } { return Err(RuntimeError::new("CreateControl returned an invalid checkbox")); }
        // A partially initialized control must never be interactive or shown.
        unsafe { write_bool(control, b.disabled, true); write_bool(control, b.visible, false); }
        // Use the stock label's font and colour, rather than package-specific constants.
        let label = unsafe { ptr::read_unaligned(page.cast::<u8>().add(b.source_label).cast::<*mut UObject>()) };
        if !label.is_null() && unsafe { live(access, label, (*label).index) && hp2_core::object_model::is_a(label.cast(), b.label.cast()) } {
            unsafe {
                let font = ptr::read_unaligned(label.cast::<u8>().add(b.source_font).cast::<u32>());
                let color = ptr::read_unaligned(label.cast::<u8>().add(b.source_color).cast::<u32>());
                ptr::write_unaligned(control.cast::<u8>().add(b.font).cast::<u32>(), font);
                ptr::write_unaligned(control.cast::<u8>().add(b.color).cast::<u32>(), color);
            }
        }
        unsafe { access.assign_fstring(control.cast::<u8>().add(b.text).cast(), &text)?; }
        unsafe { fix_tab_chain(access, b, page, control)?; }
        unsafe { write_bool(control, b.checked, (*client).show_fps()); }
        let mut state = state.borrow_mut();
        state.controls.retain(|c| unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) });
        state.controls.push(Control { page, page_index, control, control_index: unsafe { (*control).index } });
        unsafe { write_bool(control, b.disabled, false); write_bool(control, b.visible, true); }
    } else if function == b.notify && !params.is_null() && size >= usize::from(unsafe { (*b.notify).parms_size }) {
        let event = unsafe { *params.add(b.notify_event) };
        if event != 1 { return Ok(()); }
        let control = unsafe { ptr::read_unaligned(params.add(b.notify_control).cast::<*mut UObject>()) };
        if !state.borrow().controls.iter().any(|c| c.page == page && c.control == control && unsafe { live(access, c.page, c.page_index) && live(access, c.control, c.control_index) }) { return Ok(()); }
        unsafe { commit_selection(access, client, control, b.checked)?; }
    }
    Ok(())
}
