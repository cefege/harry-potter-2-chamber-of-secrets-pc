//! Per-instance stock resolution popup scrollbar adaptation.
//! The retail popup's first paint initializes its original background. Later
//! paints use UWindow's scrolling layout on this popup only; its HGame drawing
//! and selection functions, texture, focus, and all other combos stay intact.
use hp2_abi::engine::generated::{UClass, UObject, UFunction, UProperty, UStruct, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::ffi::c_void;

unsafe fn loaded(access: CoreAccess<'_>, name: &str) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { access.find_object_by_name(access.native_class("Core.Class")?, &name.chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}
unsafe fn field(access: CoreAccess<'_>, owner: *mut UStruct, name: &str, kind: &str) -> RuntimeResult<*mut UProperty> {
    let p = unsafe { access.find_field(owner, access.intern_str(name, FindName::Find)?, access.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new("Resolution popup field missing")); }
    Ok(p)
}
unsafe fn object_field(access: CoreAccess<'_>, obj: *mut UObject, class: *mut UClass, name: &str) -> RuntimeResult<*mut UObject> {
    let p = unsafe { field(access, class.cast(), name, "Core.ObjectProperty")? };
    let off = unsafe { (*p).offset };
    if off < 0 || unsafe { (*p).array_dim != 1 || (*p).element_size as usize != std::mem::size_of::<*mut c_void>() || off as usize + std::mem::size_of::<*mut c_void>() > (*class).properties_size as usize } {
        return Err(RuntimeError::new("Resolution popup object field layout"));
    }
    Ok(unsafe { access.property_address(obj, p)?.cast::<*mut UObject>().read_unaligned() })
}
fn valid(access: CoreAccess<'_>, p: *mut UObject, c: *mut UClass) -> bool {
    !p.is_null() && access.contains_object(p) && unsafe { hp2_core::object_model::is_a(p.cast(), c.cast()) }
}
unsafe fn function(access: CoreAccess<'_>, class: *mut UClass) -> RuntimeResult<*mut UFunction> {
    Ok(unsafe { field(access, class.cast(), "BeforePaint", "Core.Function")? }.cast())
}
unsafe fn compatible(access: CoreAccess<'_>, a: *mut UFunction, b: *mut UFunction) -> RuntimeResult<bool> {
    let (sa, sb) = unsafe { (&*a.cast::<UStruct>(), &*b.cast::<UStruct>()) };
    if sa.properties_size != sb.properties_size || sa.properties_align != sb.properties_align || unsafe { (*a).parms_size != (*b).parms_size } { return Ok(false); }
    for (name, kind) in [("C","Core.ObjectProperty"),("X","Core.FloatProperty"),("Y","Core.FloatProperty"),("W","Core.FloatProperty"),("H","Core.FloatProperty"),("MaxWidth","Core.FloatProperty"),("Count","Core.IntProperty"),("I","Core.ObjectProperty"),("ListX","Core.FloatProperty"),("ListY","Core.FloatProperty"),("ExtraWidth","Core.FloatProperty")] {
        let x = unsafe { field(access, a.cast(), name, kind)? };
        let y = unsafe { field(access, b.cast(), name, kind)? };
        if unsafe { (*x).offset != (*y).offset || (*x).element_size != (*y).element_size || (*x).array_dim != (*y).array_dim || ((*x).property_flags & 0x80) != ((*y).property_flags & 0x80) } { return Ok(false); }
    }
    Ok(true)
}
/// The returned guard is scoped to one Draw call, before package shutdown.
pub(crate) unsafe fn install(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(page_class) = (unsafe { loaded(access, "HGame.FESoundVideoPage")? }) else { return Ok(None); };
    let Some(root_class) = (unsafe { loaded(access, "UWindow.UWindowRootWindow")? }) else { return Ok(None); };
    let Some(combo_class) = (unsafe { loaded(access, "UWindow.UWindowComboControl")? }) else { return Ok(None); };
    let Some(list_class) = (unsafe { loaded(access, "HGame.HPMenuOptionComboList")? }) else { return Ok(None); };
    let Some(base_class) = (unsafe { loaded(access, "UWindow.UWindowComboList")? }) else { return Ok(None); };
    if viewport.is_null() { return Ok(None); }
    let console = unsafe { (*viewport).console.cast::<UObject>() };
    for i in 0..access.object_count() {
        let page = access.object_at(i).cast::<UObject>();
        if page.is_null() || !unsafe { hp2_core::object_model::is_a(page.cast(), page_class.cast()) } { continue; }
        let root = unsafe { object_field(access, page, page_class, "Root")? };
        let combo = unsafe { object_field(access, page, page_class, "ResolutionCombo")? };
        if !valid(access, root, root_class) || !valid(access, combo, combo_class) || unsafe { object_field(access, root, root_class, "Console")? } != console { continue; }
        let list = unsafe { object_field(access, combo, combo_class, "List")? };
        if !valid(access, list, list_class) { continue; }
        // Let the stock function initialize the original popup texture first.
        if unsafe { object_field(access, list, list_class, "bgImage")? }.is_null() { continue; }
        let from = unsafe { function(access, list_class)? };
        let to = unsafe { function(access, base_class)? };
        if unsafe { (*from.cast::<UObject>()).outer != list_class.cast() || (*to.cast::<UObject>()).outer != base_class.cast() }
            || !unsafe { compatible(access, from, to)? } { return Err(RuntimeError::new("Unsupported resolution popup script layout")); }
        for (name, value) in [("ItemHeight",15i32),("MaxVisible",5i32)] {
            let p = unsafe { field(access, base_class.cast(), name, "Core.IntProperty")? };
            let offset = unsafe { (*p).offset };
            if offset < 0 || unsafe { (*p).array_dim != 1 || (*p).element_size != 4 || offset as usize + 4 > (*list_class).properties_size as usize } { return Err(RuntimeError::new("Resolution popup integer layout")); }
            unsafe { access.property_address(list, p)?.cast::<i32>().write_unaligned(value); }
        }
        return Ok(Some(unsafe { access.install_script_redirect(list, from, to)? }));
    }
    Ok(None)
}
