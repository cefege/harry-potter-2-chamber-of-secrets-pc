//! Add SDL-provided logical sizes to the unmodified retail resolution combo.
//! Called immediately after the stock LoadAvailableSettings script, which has
//! cleared and rebuilt the list. It does not change the edit value or focus.
use std::{ffi::c_void, ptr};
use hp2_abi::engine::generated::{TArrayRaw, UClass, UConsole, UFunction, UObject, UProperty, UStruct, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};

unsafe fn class(access: CoreAccess<'_>, path: &str) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { access.find_object_by_name(access.native_class("Core.Class")?, &path.chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}
unsafe fn field(access: CoreAccess<'_>, structure: *mut UStruct, size: usize, name: &str, kind: &str, width: usize, param: bool) -> RuntimeResult<*mut UProperty> {
    let p = unsafe { access.find_field(structure, access.intern_str(name, FindName::Find)?, access.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new("Stock resolution menu field missing")); }
    let offset = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Stock resolution menu field offset"))?;
    if unsafe { (*p).array_dim } != 1 || usize::try_from(unsafe { (*p).element_size }).ok() != Some(width)
        || offset.checked_add(width).is_none_or(|n| n > size) || (param && unsafe { (*p).property_flags } & 0x80 == 0) {
        return Err(RuntimeError::new("Stock resolution menu field layout"));
    }
    Ok(p)
}
unsafe fn object_field(access: CoreAccess<'_>, object: *mut UObject, class: *mut UClass, name: &str) -> RuntimeResult<*mut UObject> {
    let size = usize::try_from(unsafe { (*class).properties_size }).map_err(|_| RuntimeError::new("Stock resolution menu class size"))?;
    let p = unsafe { field(access, class.cast(), size, name, "Core.ObjectProperty", std::mem::size_of::<*mut c_void>(), false)? };
    Ok(unsafe { access.property_address(object, p)?.cast::<*mut UObject>().read_unaligned() })
}
fn live(access: CoreAccess<'_>, p: *mut UObject) -> bool {
    // Compare against the object table before dereferencing an untrusted script reference.
    !p.is_null() && (0..access.object_count()).any(|i| access.object_at(i) == p.cast())
}
fn valid(access: CoreAccess<'_>, p: *mut UObject, c: *mut UClass) -> bool {
    live(access, p) && unsafe { hp2_core::object_model::is_a(p.cast(), c.cast()) }
}

/// Parse only canonical, positive WxH values. A malformed command result must not
/// become a selectable size or an injection into the stock SetRes command.
fn modern_values(text: &str) -> RuntimeResult<Vec<String>> {
    let mut result = Vec::new();
    for token in text.split_whitespace() {
        let Some((w, h)) = token.split_once('x') else { return Err(RuntimeError::new("Invalid GETRES response")); };
        let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) else { return Err(RuntimeError::new("Invalid GETRES size")); };
        if w <= 0 || h <= 0 || token != format!("{w}x{h}") { return Err(RuntimeError::new("Invalid GETRES size")); }
        // The stock script has already inserted precisely these four values.
        if matches!((w,h), (640,480)|(800,600)|(1024,768)|(1280,1024)) { continue; }
        if !result.iter().any(|s| s == token) { result.push(token.to_owned()); }
    }
    Ok(result)
}

/// Called in the engine thread after the stock script has returned.
/// All object identities and reflection offsets are validated for this call.
pub(crate) unsafe fn populate(access: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<()> {
    let Some(page_class) = (unsafe { class(access, "HGame.FESoundVideoPage")? }) else { return Ok(()); };
    let Some(root_class) = (unsafe { class(access, "UWindow.UWindowRootWindow")? }) else { return Ok(()); };
    let Some(combo_class) = (unsafe { class(access, "UWindow.UWindowComboControl")? }) else { return Ok(()); };
    if !valid(access, page, page_class) { return Ok(()); }
    let root = unsafe { object_field(access, page, page_class, "Root")? };
    let combo = unsafe { object_field(access, page, page_class, "ResolutionCombo")? };
    if !valid(access, root, root_class) || !valid(access, combo, combo_class) { return Ok(()); }
    let console = unsafe { object_field(access, root, root_class, "Console")? };
    if !live(access, console) || !unsafe { hp2_core::object_model::is_a(console.cast(), access.native_class("Engine.Console")?.cast()) } { return Ok(()); }
    let viewport = unsafe { (*console.cast::<UConsole>()).viewport.cast::<UViewport>() };
    if !live(access, viewport.cast()) || unsafe { (*viewport).console.cast::<UObject>() } != console { return Ok(()); }
    if !unsafe { hp2_core::object_model::is_a(viewport.cast(), access.native_class("SDLDrv.SDLViewport")?.cast()) } { return Ok(()); }
    let mut output = Vec::new();
    let command: Vec<u32> = "GETRES\0".chars().map(u32::from).collect();
    if !unsafe { crate::camera_owner::exec(access, viewport, command.as_ptr(), &mut |s| { output.extend_from_slice(s); Ok(()) })? } {
        return Err(RuntimeError::new("GETRES was not handled"));
    }
    let text: String = output.iter().copied().map(char::from_u32).collect::<Option<_>>().ok_or_else(|| RuntimeError::new("Invalid GETRES text"))?;
    let values = modern_values(text.trim_end_matches('\0'))?;
    if values.is_empty() { return Ok(()); }
    let name = access.intern_str("AddItem", FindName::Find)?;
    let add = unsafe { access.find_field(combo_class.cast(), name, access.native_class("Core.Function")?) }.cast::<UFunction>();
    if add.is_null() { return Err(RuntimeError::new("Stock resolution AddItem missing")); }
    let size = usize::from(unsafe { (*add).parms_size });
    let arg = unsafe { field(access, add.cast(), size, "S", "Core.StrProperty", std::mem::size_of::<TArrayRaw>(), true)? };
    // Check all parameters before invoking a script with a constructed frame.
    unsafe { field(access, add.cast(), size, "S2", "Core.StrProperty", std::mem::size_of::<TArrayRaw>(), true)?; }
    unsafe { field(access, add.cast(), size, "SortWeight", "Core.IntProperty", 4, true)?; }
    let offset = unsafe { (*arg).offset as usize };
    for value in values {
        let mut frame = vec![0u64; (size + 7) / 8];
        let p = frame.as_mut_ptr().cast::<u8>();
        let mut string: TArrayRaw = unsafe { std::mem::zeroed() };
        let units: Vec<u32> = value.chars().map(u32::from).chain([0]).collect();
        unsafe { access.assign_fstring(&mut string, &units)?; ptr::write_unaligned(p.add(offset).cast::<TArrayRaw>(), string); }
        let outcome = unsafe { access.execute_event(combo, add, p.cast(), ptr::null_mut()) };
        // VM parameters may be only four-byte aligned. Destroy the frame's
        // owned value, not an aligned pointer into that frame.
        let mut owned = unsafe { ptr::read_unaligned(p.add(offset).cast::<TArrayRaw>()) };
        let cleanup = unsafe { access.destroy_owned_fstring(&mut owned) };
        outcome?;
        cleanup?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn accepts_unique_modern_values_and_skips_stock_entries() {
        assert_eq!(modern_values("640x480 1280x720 1280x720 1920x1080").unwrap(), vec!["1280x720", "1920x1080"]);
    }
    #[test] fn rejects_invalid_or_ambiguous_values() {
        for value in ["-1x720", "0x720", "1280x720x32", "1280X720", "01280x720", "failed"] { assert!(modern_values(value).is_err(), "{value}"); }
    }
}
