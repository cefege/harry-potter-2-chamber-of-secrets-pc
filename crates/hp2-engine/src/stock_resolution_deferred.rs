//! Process retail's deferred timeout restore without synthetic input.
//! A failed restore remains pending even if HPConsole later consumes its field.
use hp2_abi::engine::generated::{TArrayRaw, UClass, UClient, UConsole, UGameEngine, UObject, UProperty, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::{cell::{Cell, RefCell}, time::{Duration, Instant}};

#[derive(Clone)]
struct Pending {
    viewport: usize, viewport_index: i32, console: usize, console_index: i32,
    units: Vec<u32>, next_attempt: Instant, warned: bool,
}
thread_local! {
    static PENDING: RefCell<Vec<Pending>> = const { RefCell::new(Vec::new()) };
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}
struct ActiveGuard;
impl Drop for ActiveGuard { fn drop(&mut self) { ACTIVE.with(|v| v.set(false)); } }

type Key = (usize, i32, usize, i32);
fn matches(p: &Pending, key: Key) -> bool { (p.viewport, p.viewport_index, p.console, p.console_index) == key }
fn remember_failure(entries: &mut Vec<Pending>, key: Key, units: Vec<u32>, now: Instant) -> bool {
    if let Some(p) = entries.iter_mut().find(|p| matches(p, key)) {
        p.next_attempt = now + Duration::from_secs(1);
        if p.warned { false } else { p.warned = true; true }
    } else {
        entries.push(Pending { viewport: key.0, viewport_index: key.1, console: key.2, console_index: key.3,
            units, next_attempt: now + Duration::from_secs(1), warned: true });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_injection_retains_original_request_throttles_and_warns_once() {
        let key = (10, 1, 20, 2);
        let old: Vec<u32> = "800x600x32".chars().map(u32::from).collect();
        let now = Instant::now();
        let mut entries = Vec::new();
        // The adapter first rejects the restore. A second failure must not
        // replace the original request even if the script field was cleared.
        assert!(remember_failure(&mut entries, key, old.clone(), now));
        assert_eq!(entries[0].units, old);
        assert!(entries[0].next_attempt > now);
        assert!(!remember_failure(&mut entries, key, Vec::new(), now + Duration::from_secs(1)));
        assert_eq!(entries[0].units, old);
        assert_eq!(entries.len(), 1);
        // A confirmed successful dispatch removes the retry, not a failure.
        entries.retain(|p| !matches(p, key));
        assert!(entries.is_empty());
    }
}

fn text(value: &TArrayRaw) -> RuntimeResult<Vec<u32>> {
    if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid deferred resolution string"));
    }
    if value.num <= 1 { return Ok(Vec::new()); }
    let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) };
    if units.last() != Some(&0) || units[..units.len()-1].contains(&0) {
        return Err(RuntimeError::new("Invalid deferred resolution string terminator"));
    }
    Ok(units[..units.len()-1].to_vec())
}

fn warn(access: CoreAccess<'_>, message: &str) {
    let warning = format!("The previous resolution could not be restored: {message}. The game will retry. Do not exit until the previous resolution is restored.");
    if let Ok(category) = access.intern_str("Warning", FindName::Add) {
        let _ = access.emit_log(category, &warning.chars().map(u32::from).collect::<Vec<_>>());
    }
    #[cfg(feature = "sdl")]
    {
        use std::ffi::{c_char, c_void, CString};
        unsafe extern "C" { fn SDL_ShowSimpleMessageBox(flags: u32, title: *const c_char, message: *const c_char, window: *mut c_void) -> i32; }
        let clean = warning.replace('\0', " ");
        if let Ok(message) = CString::new(clean) {
            // Show at most once per pending restore. A null parent is safe even
            // when the failing window has already lost focus or changed mode.
            let shown = unsafe { SDL_ShowSimpleMessageBox(0x10, c"Display settings could not be restored".as_ptr(), message.as_ptr(), std::ptr::null_mut()) };
            if shown != 0 {
                if let Ok(category) = access.intern_str("Warning", FindName::Add) {
                    let _ = access.emit_log(category, &"Could not show the display-restore warning".chars().map(u32::from).collect::<Vec<_>>());
                }
            }
        }
    }
}

/// Retain a failed direct stock rollback, whose script path does not populate
/// `ResTimeOutSettings`. The SDL adapter must call this only for a rollback,
/// not for an ordinary failed SetRes or preview.
pub(crate) unsafe fn queue_failed_rollback(access: CoreAccess<'_>, viewport: *mut UViewport, settings: &str, reason: &str) -> RuntimeResult<()> {
    if viewport.is_null() || settings.is_empty() || settings.contains('\0') { return Err(RuntimeError::new("Invalid failed resolution rollback")); }
    let index = unsafe { (*viewport.cast::<UObject>()).index };
    let console = unsafe { (*viewport).console.cast::<UConsole>() };
    if index < 0 || access.object_at(index) != viewport.cast() || console.is_null() {
        return Err(RuntimeError::new("Failed resolution rollback viewport unavailable"));
    }
    let console_index = unsafe { (*console.cast::<UObject>()).index };
    if console_index < 0 || access.object_at(console_index) != console.cast() || unsafe { (*console).viewport } != viewport.cast() {
        return Err(RuntimeError::new("Failed resolution rollback console unavailable"));
    }
    let key = (viewport as usize, index, console as usize, console_index);
    let units = settings.chars().map(u32::from).collect();
    if PENDING.with(|entries| remember_failure(&mut entries.borrow_mut(), key, units, Instant::now())) {
        warn(access, reason);
    }
    Ok(())
}

/// The engine and its client are live for this call. Pending pointers are weak
/// identities and are revalidated against the object table before every use.
pub(crate) unsafe fn process(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    if ACTIVE.with(|v| v.replace(true)) { return Ok(()); }
    let _guard = ActiveGuard;
    let client = unsafe { (*engine).client.cast::<UClient>() };
    if client.is_null() { return Ok(()); }
    let list = unsafe { (*client).viewports };
    if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
        return Err(RuntimeError::new("Invalid viewport array during deferred resolution"));
    }
    let name: Vec<u32> = "HGame.HPConsole".chars().map(u32::from).collect();
    let Some(class) = (unsafe { access.find_object_by_name(access.native_class("Core.Class")?, &name)? }) else { return Ok(()); };
    let prop_name = access.intern_str("ResTimeOutSettings", FindName::Find)?;
    let property = unsafe { access.find_field(class.cast(), prop_name, access.native_class("Core.StrProperty")?) }.cast::<UProperty>();
    if property.is_null() { return Ok(()); }
    let size = std::mem::size_of::<TArrayRaw>();
    let offset = usize::try_from(unsafe { (*property).offset }).map_err(|_| RuntimeError::new("Invalid timeout property offset"))?;
    if unsafe { (*property).array_dim } != 1 || usize::try_from(unsafe { (*property).element_size }).ok() != Some(size)
        || offset.checked_add(size).is_none_or(|end| end > unsafe { (*class.cast::<UClass>()).properties_size.max(0) as usize }) {
        return Err(RuntimeError::new("Invalid timeout property layout"));
    }
    let mut viewports = Vec::new();
    for index in 0..list.num as usize {
        let viewport = unsafe { list.data.cast::<*mut UViewport>().add(index).read() };
        if !viewport.is_null() { viewports.push((viewport, unsafe { (*viewport.cast::<UObject>()).index })); }
    }
    PENDING.with(|pending| pending.borrow_mut().retain(|p| p.viewport_index >= 0 && p.console_index >= 0
        && access.object_at(p.viewport_index) as usize == p.viewport && access.object_at(p.console_index) as usize == p.console));
    for (viewport, index) in viewports {
        if index < 0 || access.object_at(index) != viewport.cast() { continue; }
        let console = unsafe { (*viewport).console.cast::<UConsole>() };
        if console.is_null() || unsafe { (*console).viewport } != viewport.cast() || unsafe { (*viewport).actor.is_null() }
            || !unsafe { hp2_core::object_model::is_a(console.cast(), class.cast()) } { continue; }
        let console_index = unsafe { (*console.cast::<UObject>()).index };
        if console_index < 0 || access.object_at(console_index) != console.cast() { continue; }
        let field = unsafe { access.property_address(console.cast(), property)? }.cast::<TArrayRaw>();
        let value = unsafe { field.read_unaligned() };
        let current = text(&value)?;
        let key = (viewport as usize, index, console as usize, console_index);
        let pending = PENDING.with(|entries| entries.borrow().iter().find(|p| matches(p, key)).cloned());
        let now = Instant::now();
        let units = if let Some(pending) = &pending {
            if now < pending.next_attempt { continue; }
            pending.units.clone()
        } else {
            if current.is_empty() { continue; }
            current
        };
        let mut command: Vec<u32> = "SetRes ".chars().map(u32::from).collect();
        command.extend_from_slice(&units); command.push(0);
        let mut reply = Vec::new();
        let result = unsafe { crate::camera_owner::exec(access, viewport, command.as_ptr(), &mut |t| { reply.extend_from_slice(t); Ok(()) }) };
        let failure = match result {
            Ok(true) if reply.is_empty() => None,
            Ok(true) => Some(reply.iter().map(|&u| char::from_u32(u).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>()),
            Ok(false) => Some("SetRes was not handled".into()),
            Err(error) => Some(format!("{error}")),
        };
        if let Some(message) = failure {
            let should_warn = PENDING.with(|entries| remember_failure(&mut entries.borrow_mut(), key, units, now));
            if should_warn { warn(access, &message); }
            continue;
        }
        // Dispatch can re-enter code and replace the field or destroy objects.
        // Clear only the exact restore we successfully applied.
        if access.object_at(index) != viewport.cast() || access.object_at(console_index) != console.cast() { continue; }
        let field = unsafe { access.property_address(console.cast(), property)? }.cast::<TArrayRaw>();
        let mut value = unsafe { field.read_unaligned() };
        if text(&value)? == units {
            let cleared = unsafe { access.write_vm_fstring_result(&mut value, &[]) };
            unsafe { field.write_unaligned(value); }
            cleared?;
        }
        PENDING.with(|entries| entries.borrow_mut().retain(|p| !matches(p, key)));
    }
    Ok(())
}
