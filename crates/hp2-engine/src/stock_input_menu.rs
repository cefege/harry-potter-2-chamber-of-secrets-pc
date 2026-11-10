//! Extra controls on the stock Controls page: an exact mouse-speed number box under the slider and
//! a mouse-smoothing toggle. Like the other stock-menu extensions it validates every object it
//! touches on each call and never edits a retail package.
use std::{cell::RefCell, ffi::c_void, ptr};
use hp2_abi::engine::generated::{UClass, UFunction, UObject};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use crate::{settings_config, settings_policy::SettingsPatch, stock_graphics_menu::{self as menu, children, create_combo, create_control, live, object, place, place_before_back, prop, read_boolean, boolean, call, set_float, set_text, text, Arg, Geometry, Weak, ART_W}};

const MIN_SPEED: f32 = 0.2;
const MAX_SPEED: f32 = 10.;
/// Left column, page units (the page is 640x480).
const LABEL_X: f32 = 15.;
const ART_X: f32 = 130.;
const SPEED_ROW_Y: f32 = 122.;
const CHECK_ROWS: [(&str, f32); 4] = [("MouseInvertCheck", 152.), ("AutoCenterCamCheck", 200.), ("AutoJumpCheck", 224.), ("AutoDrinkPotionCheck", 248.)];
const SMOOTHING_ROW_Y: f32 = 176.;
/// Everything from the Quidditch block down moves this far to make room.
const BLOCK_SHIFT: f32 = 22.;
const BLOCK_FROM_Y: f32 = 255.;
const LEFT_COLUMN_END_X: f32 = 320.;

#[derive(Clone, Copy)]
struct Controls { page: Weak, slider: Weak, number: Weak, smoothing: Weak }
#[derive(Default)]
struct State { owner: usize, controls: Vec<Controls> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); static SELFTEST_DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

unsafe fn class(a: CoreAccess<'_>, path: &str) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { a.find_object_by_name(a.native_class("Core.Class")?, &path.chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}
unsafe fn function(a: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { a.find_field(class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { Err(RuntimeError::new("Controls page function missing")) } else { Ok(f) }
}
fn name_of(a: CoreAccess<'_>, object: *mut UObject) -> String {
    a.name_text(i32::from_ne_bytes(unsafe { (*object).name })).map(|u| u.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect()).unwrap_or_default()
}
fn class_name_of(a: CoreAccess<'_>, object: *mut UObject) -> String { name_of(a, unsafe { (*object).class.cast() }) }
fn warn(a: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = a.intern_str("Warning", FindName::Add) {
        let _ = a.emit_log(category, &format!("Controls page extension: {error}").chars().map(u32::from).collect::<Vec<_>>());
    }
}
unsafe fn float(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> RuntimeResult<f32> {
    Ok(unsafe { prop(a, o, name, "Core.FloatProperty", 4)?.cast::<f32>().read_unaligned() })
}
unsafe fn dump(a: CoreAccess<'_>, page: *mut UObject) {
    let f = |p: *mut UObject, n: &str| unsafe { float(a, p, n) }.unwrap_or(f32::NAN);
    let mut lines = Vec::new();
    if let Ok(root) = unsafe { object(a, page, "Root") } { if live(a, root) { lines.push(format!("root {}x{}", f(root, "WinWidth"), f(root, "WinHeight"))); } }
    for (child, class) in unsafe { children(a, page) } {
        lines.push(format!("child {} {class}: {}x{} at {},{}", name_of(a, child), f(child, "WinWidth"), f(child, "WinHeight"), f(child, "WinLeft"), f(child, "WinTop")));
    }
    if let Ok(category) = a.intern_str("Log", FindName::Add) { for line in lines { let _ = a.emit_log(category, &format!("HP2_INPUT_LAYOUT {line}").chars().map(u32::from).collect::<Vec<_>>()); } }
}

/// `3` -> "3.0", `3.35` -> "3.35": one to two decimals, never a bare integer.
fn speed_text(value: f32) -> String {
    let text = format!("{value:.2}");
    let trimmed = text.trim_end_matches('0');
    if trimmed.ends_with('.') { format!("{trimmed}0") } else { trimmed.to_owned() }
}
/// A usable speed from what the player typed, or `None` while the text is incomplete or out of range.
fn parse_speed(text: &str) -> Option<f32> {
    let value: f32 = text.trim().parse().ok()?;
    (value.is_finite() && (MIN_SPEED..=MAX_SPEED).contains(&value)).then_some(value)
}

unsafe fn player(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<*mut UObject> {
    let mut owner: *mut UObject = ptr::null_mut();
    unsafe { call(a, page, "GetPlayerOwner", &[], (&raw mut owner).cast::<c_void>())?; }
    if !live(a, owner) { return Err(RuntimeError::new("Controls page player unavailable")); }
    Ok(owner)
}
/// Replace the box's text without notifying, and put the caret at its end so editing starts there.
unsafe fn show(a: CoreAccess<'_>, number: *mut UObject, shown: &str) -> RuntimeResult<()> {
    unsafe {
        set_text(a, number, shown)?;
        let edit = object(a, number, "EditBox")?;
        prop(a, edit, "CaretOffset", "Core.IntProperty", 4)?.cast::<i32>().write_unaligned(shown.len() as i32);
    }
    Ok(())
}
unsafe fn persist(a: CoreAccess<'_>, patch: SettingsPatch) {
    if let Err(error) = settings_config::apply(a, &patch) { warn(a, &format!("could not save the setting: {error:?}")); }
}

unsafe fn create(a: CoreAccess<'_>, page: *mut UObject, state: &mut State) -> RuntimeResult<()> {
    state.controls.retain(|c| c.page.live(a) && c.slider.live(a) && c.number.live(a) && c.smoothing.live(a));
    if state.controls.iter().any(|c| c.page.p == page) { return Ok(()); }
    if std::env::var_os("HP2_LAYOUT_DUMP").is_some() { unsafe { dump(a, page) }; }
    settings_config::read(a)?;
    let slider = unsafe { object(a, page, "SensitivitySlider")? };
    let caption = unsafe { object(a, page, "MouseSensitivityLabel")? };
    let value_color = unsafe { object(a, page, "MouseHiLabel")? };
    let invert = unsafe { object(a, page, "MouseInvertCheck")? };
    if ![slider, caption, value_color, invert].into_iter().all(|o| live(a, o)) { return Err(RuntimeError::new("Controls page layout is not the stock one")); }
    let player = unsafe { player(a, page)? };

    // Make room: the Quidditch and duelling blocks move down, the checkboxes spread out.
    for (child, class) in unsafe { children(a, page) } {
        let (x, y) = unsafe { (float(a, child, "WinLeft")?, float(a, child, "WinTop")?) };
        if x < LEFT_COLUMN_END_X && y >= BLOCK_FROM_Y { unsafe { set_float(a, child, "WinTop", y + BLOCK_SHIFT)?; } }
        // "Cycle Current Spell" is longer than its 115-unit label and was cut off by the button.
        if class == "UWindowLabelControl" && x < 20. && unsafe { float(a, child, "WinWidth")? } == 115. {
            unsafe { set_float(a, child, "WinLeft", 6.)?; set_float(a, child, "WinWidth", 124.)?; }
        }
    }
    for (name, y) in CHECK_ROWS { unsafe { place(a, page, name, ART_X, y, None)?; } }

    // The number box is a stock combo made editable, so it matches the other value boxes.
    let combo_class = unsafe { class(a, "HGame.HPMenuOptionCombo")? }.ok_or_else(|| RuntimeError::new("Controls page combo class unavailable"))?;
    let geometry = Geometry { x: LABEL_X, y: SPEED_ROW_Y, w: ART_X + ART_W - LABEL_X, edit: ART_W };
    let number = unsafe { create_combo(a, page, combo_class, geometry, "Exact value", caption, value_color)? };
    unsafe {
        call(a, number, "SetEditable", &[("bNewCanEdit", Arg::Bool(true))], ptr::null_mut())?;
        call(a, number, "SetNumericFloat", &[("bNumericFloat", Arg::Bool(true))], ptr::null_mut())?;
        call(a, number, "SetMaxLength", &[("MaxLength", Arg::Int(5))], ptr::null_mut())?;
        // Retail's value box skips UWindowEditBox.Created, so it never accepts keyboard focus.
        let edit = object(a, number, "EditBox")?;
        call(a, edit, "SetAcceptsFocus", &[], ptr::null_mut())?;
        boolean(a, edit, "bSelectOnFocus", true)?;
        let button = object(a, number, "Button")?;
        // The drop-down button is as wide as the value box and sits on top of the edit box, so it must leave the window list (HideWindow), not just turn invisible.
        if live(a, button) { boolean(a, button, "bDisabled", true)?; call(a, button, "HideWindow", &[], ptr::null_mut())?; }
        show(a, number, &speed_text(float(a, slider, "Value")?))?;
    }

    // The smoothing toggle is another instance of the page's own checkbox class.
    let check_class = unsafe { (*invert).class.cast::<UClass>() };
    let smoothing = unsafe { create_control(a, page, check_class, ART_X, SMOOTHING_ROW_Y, 160., 1.)? };
    unsafe {
        boolean(a, smoothing, "bWindowVisible", false)?; boolean(a, smoothing, "bAcceptsFocus", false)?;
        call(a, smoothing, "SetText", &[("NewText", Arg::Str("Mouse Smoothing"))], ptr::null_mut())?;
        let font = prop(a, invert, "Font", "Core.IntProperty", 4)?.cast::<i32>().read_unaligned();
        call(a, smoothing, "SetFont", &[("NewFont", Arg::Int(font))], ptr::null_mut())?;
        let color = prop(a, invert, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().read_unaligned();
        prop(a, smoothing, "TextColor", "Core.StructProperty", 4)?.cast::<u32>().write_unaligned(color);
        boolean(a, smoothing, "bChecked", read_boolean(a, player, "bMaxMouseSmoothing")?)?;
        place_before_back(a, page, &[number, smoothing])?;
        boolean(a, number, "bAcceptsFocus", true)?; boolean(a, number, "bWindowVisible", true)?;
        boolean(a, smoothing, "bAcceptsFocus", true)?; boolean(a, smoothing, "bWindowVisible", true)?;
    }
    state.controls.push(Controls { page: unsafe { Weak::new(page) }, slider: unsafe { Weak::new(slider) }, number: unsafe { Weak::new(number) }, smoothing: unsafe { Weak::new(smoothing) } });
    Ok(())
}

/// A control on this page changed (`DE_Change`).
unsafe fn changed(a: CoreAccess<'_>, page: *mut UObject, sender: *mut UObject, state: &State) -> RuntimeResult<()> {
    let Some(c) = state.controls.iter().find(|c| c.page.p == page && c.page.live(a) && c.slider.live(a) && c.number.live(a) && c.smoothing.live(a)) else { return Ok(()) };
    let player = unsafe { player(a, page)? };
    if sender == c.slider.p {
        // The stock handler already applied the value to the player; keep it on disk too.
        let value = unsafe { float(a, c.slider.p, "Value")? };
        unsafe { persist(a, SettingsPatch { mouse_sensitivity: Some(f64::from(value)), ..Default::default() }); }
    } else if sender == c.number.p {
        let typed = unsafe { text(a, c.number.p)? };
        // Incomplete or out-of-range text is left alone until it becomes a usable number.
        let Some(value) = parse_speed(&typed) else { return Ok(()) };
        if std::env::var_os("HP2_INPUT_SELFTEST").is_some() {
            if let Ok(category) = a.intern_str("Log", FindName::Add) { let _ = a.emit_log(category, &format!("HP2_INPUT_SELFTEST typed={typed} applied={value}").chars().map(u32::from).collect::<Vec<_>>()); }
        }
        unsafe {
            prop(a, c.slider.p, "Value", "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(value);
            prop(a, player, "MouseSensitivity", "Core.FloatProperty", 4)?.cast::<f32>().write_unaligned(value);
            persist(a, SettingsPatch { mouse_sensitivity: Some(f64::from(value)), ..Default::default() });
        }
    } else if sender == c.smoothing.p {
        let on = unsafe { read_boolean(a, c.smoothing.p, "bChecked")? };
        unsafe {
            boolean(a, player, "bMaxMouseSmoothing", on)?;
            persist(a, SettingsPatch { mouse_smoothing: Some(on), ..Default::default() });
        }
    }
    Ok(())
}

unsafe fn callback(a: CoreAccess<'_>, page: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    if page.is_null() || !live(a, page) || class_name_of(a, page) != "FEInputPage" { return Ok(()); }
    let result: RuntimeResult<()> = STATE.with(|slot| {
        let Ok(mut state) = slot.try_borrow_mut() else { return Ok(()) };
        let id = a.owner_context::<crate::core_owner::EngineState>().map(|s| s as *const _ as usize).ok_or_else(|| RuntimeError::new("Controls page owner unavailable"))?;
        if state.owner != id { *state = State { owner: id, controls: Vec::new() }; }
        match name_of(a, function.cast()).as_str() {
            "Created" => unsafe { create(a, page, &mut state) },
            "Notify" => {
                let f = unsafe { menu::function(a, page, "Notify")? };
                let expected = usize::from(unsafe { (*f).parms_size });
                if params.is_null() || size < expected { return Ok(()); }
                let sender_slot = unsafe { menu::field(a, f.cast(), "C", "Core.ObjectProperty", size_of::<*mut c_void>(), expected, true)? };
                let event_slot = unsafe { menu::field(a, f.cast(), "E", "Core.ByteProperty", 1, expected, true)? };
                if unsafe { *params.add(event_slot) } != 1 { return Ok(()); }
                let sender = unsafe { params.add(sender_slot).cast::<*mut UObject>().read_unaligned() };
                unsafe { changed(a, page, sender, &state) }
            }
            _ => Ok(()),
        }
    });
    if let Err(error) = &result { warn(a, error); }
    Ok(())
}

/// Called every tick: show the slider's live value in the number box unless the player is typing in it.
pub(crate) unsafe fn tick(a: CoreAccess<'_>) -> RuntimeResult<()> {
    // Copy the controls out first: the VM calls below can re-enter `callback`, which needs the state.
    let controls = STATE.with(|slot| {
        let Ok(mut state) = slot.try_borrow_mut() else { return Vec::new() };
        state.controls.retain(|c| c.page.live(a) && c.slider.live(a) && c.number.live(a) && c.smoothing.live(a));
        state.controls.clone()
    });
    {
        for c in &controls {
            // Replays cannot type text, so `HP2_INPUT_SELFTEST=<number>` feeds the box the same
            // `SetValue` call a typed edit makes, once, to exercise the change handler end to end.
            if let Some(value) = std::env::var("HP2_INPUT_SELFTEST").ok().filter(|_| !SELFTEST_DONE.with(|d| d.replace(true))) {
                unsafe { call(a, c.number.p, "SetValue", &[("NewValue", Arg::Str(&value))], ptr::null_mut())?; }
            }
            let edit = unsafe { object(a, c.number.p, "EditBox")? };
            if !live(a, edit) || unsafe { read_boolean(a, edit, "bHasKeyboardFocus")? } { continue; }
            let shown = speed_text(unsafe { float(a, c.slider.p, "Value")? });
            if unsafe { text(a, c.number.p)? } != shown { unsafe { show(a, c.number.p, &shown)?; } }
        }
        Ok(())
    }
}

/// Observe the Controls page for one engine tick. Absent (not yet loaded) packages are normal.
pub(crate) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(page) = (unsafe { class(a, "HGame.FEInputPage")? }) else { return Ok(None) };
    let functions = [unsafe { function(a, page, "Created")? }, unsafe { function(a, page, "Notify")? }];
    Ok(Some(unsafe { a.install_script_observer("controls-page", &functions, callback)? }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn speed_text_keeps_one_to_two_decimals() {
        assert_eq!(speed_text(3.), "3.0"); assert_eq!(speed_text(3.2), "3.2"); assert_eq!(speed_text(3.35), "3.35"); assert_eq!(speed_text(10.), "10.0"); assert_eq!(speed_text(0.2), "0.2");
    }
    #[test] fn typed_speed_must_be_in_range() {
        assert_eq!(parse_speed("3.5"), Some(3.5)); assert_eq!(parse_speed(" 7 "), Some(7.)); assert_eq!(parse_speed("3."), Some(3.));
        assert_eq!(parse_speed(""), None); assert_eq!(parse_speed("."), None); assert_eq!(parse_speed("0.1"), None); assert_eq!(parse_speed("10.1"), None); assert_eq!(parse_speed("nan"), None); assert_eq!(parse_speed("abc"), None);
    }
    #[test] fn new_rows_fit_between_stock_rows() {
        let rows: Vec<f32> = CHECK_ROWS.iter().map(|r| r.1).chain([SMOOTHING_ROW_Y, SPEED_ROW_Y]).collect();
        let mut sorted = rows.clone(); sorted.sort_by(f32::total_cmp);
        assert!(sorted.windows(2).all(|w| w[1] - w[0] >= 24.), "rows must not overlap: {sorted:?}");
        assert!(*sorted.last().unwrap() + 16. + 4. <= BLOCK_FROM_Y + BLOCK_SHIFT, "last checkbox must end above the shifted Quidditch block");
    }
}
