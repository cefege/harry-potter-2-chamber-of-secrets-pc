//! Reflected entry points for the unmodified stock resolution lifecycle.
//! The transaction owner must also validate the active page and confirmation.
use hp2_abi::engine::generated::{UClass, UFunction, UObject, UProperty};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName, ScriptScopeHook};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    SettingsChanged,
    WindowDone { window: *mut UObject },
}

unsafe fn class(a: CoreAccess<'_>) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { a.find_object_by_name(a.native_class("Core.Class")?, &"HGame.FESoundVideoPage".chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}
unsafe fn function(a: CoreAccess<'_>, class: *mut UClass, name: &str) -> RuntimeResult<*mut UFunction> {
    let f = unsafe { a.find_field(class.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.Function")?) }.cast::<UFunction>();
    if f.is_null() { return Err(RuntimeError::new("Resolution lifecycle function missing")); }
    Ok(f)
}
fn live(a: CoreAccess<'_>, object: *mut UObject) -> bool {
    !object.is_null() && (0..a.object_count()).any(|i| a.object_at(i) == object.cast())
}

/// Distinguish the exact stock methods and validate the receiver and W frame.
/// A caller must additionally check that the page and message box are the
/// transaction's own live objects, and interpret the localized Result itself.
pub(crate) unsafe fn classify(a: CoreAccess<'_>, receiver: *mut UObject, f: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<Option<Event>> {
    let Some(page) = (unsafe { class(a)? }) else { return Ok(None); };
    if !live(a, receiver) || !unsafe { hp2_core::object_model::is_a(receiver.cast(), page.cast()) } { return Ok(None); }
    let settings = unsafe { function(a, page, "SettingsChanged")? };
    let done = unsafe { function(a, page, "WindowDone")? };
    if f != settings && f != done { return Ok(None); }
    if size != usize::from(unsafe { (*f).parms_size }) || unsafe { (*f).return_value_offset } != u16::MAX {
        return Err(RuntimeError::new("Invalid resolution lifecycle frame"));
    }
    if f == settings {
        if size != 0 { return Err(RuntimeError::new("Unexpected SettingsChanged parameters")); }
        return Ok(Some(Event::SettingsChanged));
    }
    if params.is_null() { return Err(RuntimeError::new("Missing WindowDone parameters")); }
    let property = unsafe { a.find_field(f.cast(), a.intern_str("W", FindName::Find)?, a.native_class("Core.ObjectProperty")?) }.cast::<UProperty>();
    if property.is_null() { return Err(RuntimeError::new("WindowDone W parameter missing")); }
    let offset = usize::try_from(unsafe { (*property).offset }).map_err(|_| RuntimeError::new("Invalid WindowDone W offset"))?;
    let width = std::mem::size_of::<*mut UObject>();
    if unsafe { (*property).array_dim != 1 || (*property).element_size as usize != width || (*property).property_flags & 0x80 == 0 }
        || offset.checked_add(width).is_none_or(|end| end > size) {
        return Err(RuntimeError::new("Invalid WindowDone W layout"));
    }
    let window = unsafe { params.add(offset).cast::<*mut UObject>().read_unaligned() };
    if !window.is_null() && !live(a, window) { return Err(RuntimeError::new("Stale WindowDone W")); }
    Ok(Some(Event::WindowDone { window }))
}

thread_local! { static INSTALLED: Cell<Option<([usize; 3], usize)>> = const { Cell::new(None) }; }
struct Scope<G: Drop> { guard: Option<G> }
impl<G: Drop> Drop for Scope<G> {
    fn drop(&mut self) {
        INSTALLED.with(|slot| { if let Some((ids, depth)) = slot.get() { slot.set(if depth == 1 { None } else { Some((ids, depth - 1)) }); } });
        // Nested scopes borrow the outer guard and must be dropped first.
        drop(self.guard.take());
    }
}
/// Install around both input and draw; nested scopes borrow the outer guard.
pub(crate) unsafe fn install(a: CoreAccess<'_>, callback: ScriptScopeHook) -> RuntimeResult<Option<impl Drop + '_>> {
    let Some(page) = (unsafe { class(a)? }) else { return Ok(None); };
    let functions = [unsafe { function(a, page, "SettingsChanged")? }, unsafe { function(a, page, "WindowDone")? }];
    let ids = [functions[0] as usize, functions[1] as usize, callback as usize];
    let current = INSTALLED.with(Cell::get);
    if current.is_some_and(|(other, _)| other != ids) { return Err(RuntimeError::new("Resolution lifecycle owner changed during scope")); }
    let guard = if current.is_some() { None } else { Some(unsafe { a.install_script_scope_observer("stock-resolution-lifecycle", &functions, callback)? }) };
    INSTALLED.with(|slot| slot.set(Some((ids, current.map_or(1, |(_, depth)| depth + 1)))));
    Ok(Some(Scope { guard }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_abi::engine::generated::FFrame;
    use hp2_core::ScriptScopePhase;
    use std::{cell::Cell, ptr};
    thread_local! { static SEEN: Cell<u32> = const { Cell::new(0) }; static CALLS: Cell<u32> = const { Cell::new(0) }; static WINDOW: Cell<*mut UObject> = const { Cell::new(ptr::null_mut()) }; }
    unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, f: *mut UFunction, params: *const u8, size: usize, phase: ScriptScopePhase) -> RuntimeResult<()> {
        CALLS.with(|count| count.set(count.get() + 1));
        let event = unsafe { classify(a, receiver, f, params, size)? }.expect("stock lifecycle receiver");
        match event {
            Event::SettingsChanged => SEEN.with(|s| s.set(s.get() | if phase == ScriptScopePhase::Enter { 1 } else { 4 })),
            Event::WindowDone { window } => { assert_eq!(window, WINDOW.with(Cell::get)); SEEN.with(|s| s.set(s.get() | if phase == ScriptScopePhase::Enter { 2 } else { 8 })); }
        }
        assert!(!matches!(phase, ScriptScopePhase::Exit { succeeded: false }));
        Ok(())
    }
    #[test]
    fn stock_functions_route_from_bytecode_before_their_bodies() {
        let owner = crate::render_owner::render_test_support::owner("resolution-lifecycle-vm");
        let a = owner.access();
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-resolution-lifecycle-vm-{}", std::process::id()));
        unsafe {
            a.load_package(&root.join("System/HGame.u"), 1).unwrap();
            let page = crate::render_owner::render_test_support::construct(a, "HGame.FESoundVideoPage");
            let window = crate::render_owner::render_test_support::construct(a, "HGame.HPMessageBox");
            WINDOW.with(|slot| slot.set(window));
            let page_class = class(a).unwrap().unwrap();
            let caller = a.construct_object(a.native_class("Core.Function").unwrap(), a.native_class("Core.Object").unwrap().cast(), 0, 0, ptr::null_mut()).unwrap().cast();
            let guard = install(a, callback).unwrap().expect("hook installed");
            let nested = install(a, callback).unwrap().expect("nested hook installed");
            for (name, parameter) in [("SettingsChanged", false), ("WindowDone", true)] {
                let f = function(a, page_class, name).unwrap();
                let mut bytecode = vec![hp2_abi::engine::generated::EX_FinalFunction as u8];
                bytecode.extend((*f.cast::<UObject>()).index.to_le_bytes());
                if parameter {
                    bytecode.push(hp2_abi::engine::generated::EX_ObjectConst as u8);
                    bytecode.extend((*window).index.to_le_bytes());
                }
                bytecode.push(hp2_abi::engine::generated::EX_EndFunctionParms as u8);
                let mut frame = FFrame { node: caller, object: page.cast(), code: bytecode.as_mut_ptr(), locals: ptr::null_mut(), parent: ptr::null_mut() };
                a.execute_state_frame_step(&raw mut frame, ptr::null_mut()).unwrap();
            }
            assert_eq!(SEEN.with(Cell::get), 15);
            assert_eq!(CALLS.with(Cell::get), 4, "nested scopes do not duplicate callbacks");
            drop(nested);
            drop(guard);
            #[cfg(feature = "sdl")]
            {
                // The production callback must leave an uninitialized page
                // and an unrelated message box entirely to stock script.
                let guard = install(a, crate::stock_resolution_transaction::callback).unwrap().unwrap();
                for (name, parameter) in [("SettingsChanged", false), ("WindowDone", true)] {
                    let f = function(a, page_class, name).unwrap();
                    let mut code = vec![hp2_abi::engine::generated::EX_FinalFunction as u8];
                    code.extend((*f.cast::<UObject>()).index.to_le_bytes());
                    if parameter { code.push(hp2_abi::engine::generated::EX_ObjectConst as u8); code.extend((*window).index.to_le_bytes()); }
                    code.push(hp2_abi::engine::generated::EX_EndFunctionParms as u8);
                    let mut frame = FFrame { node: caller, object: page.cast(), code: code.as_mut_ptr(), locals: ptr::null_mut(), parent: ptr::null_mut() };
                    a.execute_state_frame_step(&raw mut frame, ptr::null_mut()).unwrap();
                }
                drop(guard);
            }
        }
    }
}
