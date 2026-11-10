//! Keep the stock resolution combo consistent with its actual owning viewport.
//!
//! Retail reads the startup preference rather than the current window mode and
//! refreshes before the deferred timeout restore runs. Correct only the display
//! field, without changing config, invoking Notify, or modifying package bytes.
use hp2_abi::engine::generated::{TArrayRaw, UClass, UClient, UGameEngine, UObject, UProperty, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::ffi::c_void;

unsafe fn loaded_class(access: CoreAccess<'_>, path: &str) -> RuntimeResult<Option<*mut UClass>> {
    let text: Vec<u32> = path.chars().map(u32::from).collect();
    Ok(unsafe { access.find_object_by_name(access.native_class("Core.Class")?, &text)? }.map(|p| p.cast()))
}
unsafe fn field(access: CoreAccess<'_>, class: *mut UClass, name: &str, kind: &str, width: usize) -> RuntimeResult<Option<*mut UProperty>> {
    let name = access.intern_str(name, FindName::Find)?;
    let property = unsafe { access.find_field(class.cast(), name, access.native_class(kind)?) }.cast::<UProperty>();
    if property.is_null() { return Ok(None); }
    let offset = usize::try_from(unsafe { (*property).offset }).map_err(|_| RuntimeError::new("Invalid stock resolution field offset"))?;
    if unsafe { (*property).array_dim } != 1 || usize::try_from(unsafe { (*property).element_size }).ok() != Some(width)
        || offset.checked_add(width).is_none_or(|end| end > unsafe { (*class).properties_size.max(0) as usize }) {
        return Err(RuntimeError::new("Invalid stock resolution field layout"));
    }
    Ok(Some(property))
}
unsafe fn object_field(access: CoreAccess<'_>, object: *mut UObject, property: *mut UProperty) -> RuntimeResult<*mut UObject> {
    Ok(unsafe { access.property_address(object, property)?.cast::<*mut UObject>().read_unaligned() })
}

// Keep the texture combo's edit area after the stock label's declared region.
// Both coordinates are in the same parent window's virtual UI space.
fn texture_combo_left(label_left: f32, label_width: f32, combo_left: f32, combo_width: f32, edit_width: f32, right_limit: f32) -> Option<f32> {
    let v = [label_left, label_width, combo_left, combo_width, edit_width, right_limit];
    if v.iter().any(|x| !x.is_finite()) || label_left < 0.0 || label_width <= 0.0 || combo_left < 0.0
        || combo_width <= 0.0 || edit_width <= 0.0 || edit_width > combo_width || right_limit <= 0.0 {
        return None;
    }
    let edit_left = combo_left + combo_width - edit_width;
    let new_left = combo_left + (label_left + label_width + 4.0 - edit_left).max(0.0);
    (new_left + combo_width <= right_limit).then_some(new_left)
}

#[cfg(test)]
mod texture_layout_tests {
    use super::texture_combo_left;
    #[test]
    fn texture_edit_reserves_label_region_without_overlapping_next_column() {
        assert_eq!(texture_combo_left(100.0, 130.0, 82.0, 242.0, 134.0, 380.0), Some(126.0));
        assert_eq!(texture_combo_left(100.0, 130.0, 126.0, 242.0, 134.0, 380.0), Some(126.0));
        assert_eq!(texture_combo_left(100.0, 130.0, 82.0, 242.0, 134.0, 360.0), None);
        assert_eq!(texture_combo_left(f32::NAN, 130.0, 82.0, 242.0, 134.0, 380.0), None);
    }
}

/// The engine and its client are live. All reflected objects are borrowed only
/// within this synchronous post-tick pass; no raw pointers are retained.
pub(crate) unsafe fn sync(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    let client = unsafe { (*engine).client.cast::<UClient>() };
    if client.is_null() { return Ok(()); }
    let Some(page_class) = (unsafe { loaded_class(access, "HGame.FESoundVideoPage")? }) else { return Ok(()); };
    let Some(root_class) = (unsafe { loaded_class(access, "UWindow.UWindowRootWindow")? }) else { return Ok(()); };
    let Some(combo_class) = (unsafe { loaded_class(access, "UWindow.UWindowComboControl")? }) else { return Ok(()); };
    let Some(edit_class) = (unsafe { loaded_class(access, "UWindow.UWindowEditBox")? }) else { return Ok(()); };
    let object_width = std::mem::size_of::<*mut c_void>();
    let Some(page_root) = (unsafe { field(access, page_class, "Root", "Core.ObjectProperty", object_width)? }) else { return Ok(()); };
    let Some(page_combo) = (unsafe { field(access, page_class, "ResolutionCombo", "Core.ObjectProperty", object_width)? }) else { return Ok(()); };
    let Some(root_console) = (unsafe { field(access, root_class, "Console", "Core.ObjectProperty", object_width)? }) else { return Ok(()); };
    let Some(combo_edit) = (unsafe { field(access, combo_class, "EditBox", "Core.ObjectProperty", object_width)? }) else { return Ok(()); };
    let Some(edit_value) = (unsafe { field(access, edit_class, "Value", "Core.StrProperty", std::mem::size_of::<TArrayRaw>())? }) else { return Ok(()); };
    let Some(edit_caret) = (unsafe { field(access, edit_class, "CaretOffset", "Core.IntProperty", 4)? }) else { return Ok(()); };
    // Retail has a separate texture label. The prototype does not, so leave
    // its layout untouched when any of these optional fields is absent.
    let texture_layout = if let Some(window_class) = unsafe { loaded_class(access, "UWindow.UWindowWindow")? } {
        match (unsafe { field(access, page_class, "TextureDetailLabel", "Core.ObjectProperty", object_width)? },
            unsafe { field(access, page_class, "TextureDetailCombo", "Core.ObjectProperty", object_width)? },
            unsafe { field(access, page_class, "AudioLabel", "Core.ObjectProperty", object_width)? },
            unsafe { field(access, window_class, "ParentWindow", "Core.ObjectProperty", object_width)? },
            unsafe { field(access, window_class, "WinLeft", "Core.FloatProperty", 4)? },
            unsafe { field(access, window_class, "WinWidth", "Core.FloatProperty", 4)? },
            unsafe { field(access, combo_class, "EditBoxWidth", "Core.FloatProperty", 4)? }) {
            (Some(label), Some(combo), Some(audio), Some(parent), Some(left), Some(width), Some(edit_width)) =>
                Some((window_class, label, combo, audio, parent, left, width, edit_width)),
            _ => None,
        }
    } else { None };
    let list = unsafe { (*client).viewports };
    if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
        return Err(RuntimeError::new("Invalid viewport array during resolution display sync"));
    }
    // Snapshot the live set before following reflected references. The pass is
    // synchronous and does not invoke script, so GC cannot invalidate it.
    let live: Vec<*mut UObject> = (0..access.object_count()).map(|i| access.object_at(i).cast()).filter(|p: &*mut UObject| !p.is_null()).collect();
    let valid = |object: *mut UObject, class: *mut UClass| !object.is_null() && live.contains(&object)
        && unsafe { hp2_core::object_model::is_a(object.cast(), class.cast()) };
    let mut viewports = Vec::new();
    for i in 0..list.num as usize {
        let viewport = unsafe { list.data.cast::<*mut UViewport>().add(i).read() };
        if viewport.is_null() || !live.contains(&viewport.cast()) { continue; }
        let console = unsafe { (*viewport).console.cast::<UObject>() };
        if console.is_null() || !live.contains(&console) { continue; }
        if let Some(size) = unsafe { crate::camera_owner::sdl_display_exec::current_resolution(access, viewport)? } {
            viewports.push((console, size));
        }
    }
    if viewports.is_empty() { return Ok(()); }
    for page in live.iter().copied().filter(|&p| unsafe { hp2_core::object_model::is_a(p.cast(), page_class.cast()) }) {
        if let Some((window_class, label_field, texture_field, audio_field, parent_field, left_field, width_field, edit_width_field)) = texture_layout {
            let label = unsafe { object_field(access, page, label_field)? };
            let texture = unsafe { object_field(access, page, texture_field)? };
            let audio = unsafe { object_field(access, page, audio_field)? };
            if valid(label, window_class) && valid(texture, combo_class) && valid(audio, window_class)
                && unsafe { object_field(access, label, parent_field)? } == page
                && unsafe { object_field(access, texture, parent_field)? } == page
                && unsafe { object_field(access, audio, parent_field)? } == page {
                let read = |object, property| unsafe { access.property_address(object, property) }.map(|p| unsafe { p.cast::<f32>().read_unaligned() });
                let left = unsafe { access.property_address(texture, left_field)? }.cast::<f32>();
                if let Some(value) = texture_combo_left(read(label, left_field)?, read(label, width_field)?,
                    unsafe { left.read_unaligned() }, read(texture, width_field)?, read(texture, edit_width_field)?, read(audio, left_field)?) {
                    unsafe { left.write_unaligned(value); }
                }
            }
        }
        let root = unsafe { object_field(access, page, page_root)? };
        let combo = unsafe { object_field(access, page, page_combo)? };
        if !valid(root, root_class) || !valid(combo, combo_class) { continue; }
        let console = unsafe { object_field(access, root, root_console)? };
        let Some((_, (width, height))) = viewports.iter().find(|(candidate, _)| *candidate == console) else { continue; };
        let edit = unsafe { object_field(access, combo, combo_edit)? };
        if !valid(edit, edit_class) { continue; }
        let address = unsafe { access.property_address(edit, edit_value)? }.cast::<TArrayRaw>();
        let mut value = unsafe { address.read_unaligned() };
        if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
            return Err(RuntimeError::new("Invalid stock resolution display string"));
        }
        let desired: Vec<i32> = format!("{width}x{height}").chars().map(|c| c as i32).collect();
        let old = if value.num == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(value.data.cast::<i32>(), value.num as usize) } };
        if old.len() == desired.len() + 1 && old.last() == Some(&0) && old[..desired.len()] == desired { continue; }
        let result = unsafe { access.write_vm_fstring_result(&mut value, &desired) };
        unsafe { address.write_unaligned(value); }
        result?;
        let caret = unsafe { access.property_address(edit, edit_caret)? }.cast::<i32>();
        let current = unsafe { caret.read_unaligned() };
        if current > desired.len() as i32 { unsafe { caret.write_unaligned(desired.len() as i32); } }
    }
    Ok(())
}
