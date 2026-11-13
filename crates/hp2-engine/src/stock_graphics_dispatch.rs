//! Scoped VM observation for the advanced stock graphics controls.
//! The menu validates exact weak control identities before changing anything.
use hp2_abi::engine::generated::{UClass, UFunction, UObject, UProperty};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};

unsafe fn class(a: CoreAccess<'_>, path: &str) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { a.find_object_by_name(a.native_class("Core.Class")?, &path.chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}
unsafe fn function(a: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { a.find_field(class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { return Err(RuntimeError::new("Graphics VM observer function missing")); }
    Ok(f)
}
fn warn(a: CoreAccess<'_>, error: &impl std::fmt::Display) {
    if let Ok(category) = a.intern_str("Warning", FindName::Add) {
        let _ = a.emit_log(category, &format!("Graphics VM observer: {error}").chars().map(u32::from).collect::<Vec<_>>());
    }
}
unsafe fn popup(a: CoreAccess<'_>, receiver: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    if let Err(error) = unsafe { crate::stock_graphics_menu::popup_after_paint(a, receiver) } { warn(a, &error); }
    Ok(())
}
unsafe fn keyboard(a: CoreAccess<'_>, receiver: *mut UObject, f: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    let result = (|| -> RuntimeResult<()> {
        if receiver.is_null() || !a.contains_object(receiver) { return Ok(()); }
        // Ignore explicit Super.KeyDown calls; only the receiver's actual
        // dispatch target must advance the control once per key event.
        let key_name = a.intern_str("KeyDown", FindName::Find)?;
        if unsafe { a.find_object_function_by_name(receiver, key_name, false)? } != f { return Ok(()); }
        if params.is_null() || size != usize::from(unsafe { (*f).parms_size }) { return Err(RuntimeError::new("Invalid graphics KeyDown frame")); }
        let p = unsafe { a.find_field(f.cast(), a.intern_str("Key", FindName::Find)?, a.native_class("Core.IntProperty")?) }.cast::<UProperty>();
        if p.is_null() { return Err(RuntimeError::new("Graphics KeyDown key missing")); }
        let offset = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Graphics KeyDown key offset"))?;
        if unsafe { (*p).array_dim != 1 || (*p).element_size != 4 || (*p).property_flags & 0x80 == 0 } || offset.checked_add(4).is_none_or(|end| end > size) {
            return Err(RuntimeError::new("Invalid graphics KeyDown key layout"));
        }
        let key = unsafe { params.add(offset).cast::<i32>().read_unaligned() };
        unsafe { crate::stock_graphics_menu::keyboard_after_keydown(a, receiver, key) }
    })();
    if let Err(error) = result { warn(a, &error); }
    Ok(())
}
/// Keep alive around input dispatch. Missing unloaded packages are normal.
pub(crate) unsafe fn install_input(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(window) = (unsafe { class(a, "UWindow.UWindowWindow")? }) else { return Ok(None); };
    let Some(edit) = (unsafe { class(a, "UWindow.UWindowEditBox")? }) else { return Ok(None); };
    let Some(dialog) = (unsafe { class(a, "UWindow.UWindowDialogControl")? }) else { return Ok(None); };
    let funcs = [unsafe { function(a, window, "KeyDown")? }, unsafe { function(a, dialog, "KeyDown")? }, unsafe { function(a, edit, "KeyDown")? }];
    Ok(Some(unsafe { a.install_script_observer("graphics-input", &funcs, keyboard)? }))
}
/// Keep alive around Draw, alongside the resolution popup redirect.
pub(crate) unsafe fn install_paint(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(list) = (unsafe { class(a, "HGame.HPMenuOptionComboList")? }) else { return Ok(None); };
    let funcs = [unsafe { function(a, list, "BeforePaint")? }];
    Ok(Some(unsafe { a.install_script_observer("graphics-paint", &funcs, popup)? }))
}
