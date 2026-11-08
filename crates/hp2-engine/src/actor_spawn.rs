//! Rust-owned policy for the AActor Spawn and Destroy script natives.
use std::ffi::c_void;

#[repr(C)]
pub struct Hp2ActorSpawnServices {
    pub context: *mut c_void,
    pub trace_class_resolution: unsafe extern "C" fn(
        context: *mut c_void,
        actor_class: *mut c_void,
        actor: *mut c_void,
    ) -> i32,
    pub trace_spawn_request: unsafe extern "C" fn(
        context: *mut c_void,
        actor_class: *mut c_void,
        actor: *mut c_void,
    ) -> i32,
    pub spawn: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        actor_class: *mut c_void,
        owner: *mut c_void,
        location: *const c_void,
        rotation: *const c_void,
        out: *mut *mut c_void,
    ) -> i32,
    pub trace_spawn_result:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, spawned: *mut c_void) -> i32,
    pub trace_spawn_published:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, spawned: *mut c_void) -> i32,
    pub set_tag: unsafe extern "C" fn(
        context: *mut c_void,
        spawned: *mut c_void,
        name: *const c_void,
    ) -> i32,
    pub destroy:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, out: *mut u32) -> i32,
}

/// Runs the script-native spawn sequence; object and ABI storage is owned by the engine.
///
/// # Safety
/// All pointers and callbacks must be valid for the duration of the call.
pub unsafe fn exec_spawn(
    actor: *mut c_void,
    actor_class: *mut c_void,
    owner: *mut c_void,
    name: *const c_void,
    location: *const c_void,
    rotation: *const c_void,
    services: &Hp2ActorSpawnServices,
    out: *mut *mut c_void,
) -> Result<(), i32> {
    if out.is_null() {
        return Err(1);
    }
    let context = services.context;
    let status = unsafe { (services.trace_class_resolution)(context, actor_class, actor) };
    if status != 0 {
        return Err(status);
    }
    let mut spawned = std::ptr::null_mut();
    if !actor_class.is_null() {
        let status = unsafe { (services.trace_spawn_request)(context, actor_class, actor) };
        if status != 0 {
            return Err(status);
        }
        let status = unsafe {
            (services.spawn)(
                context,
                actor,
                actor_class,
                owner,
                location,
                rotation,
                &mut spawned,
            )
        };
        if status != 0 {
            return Err(status);
        }
        let status = unsafe { (services.trace_spawn_result)(context, actor, spawned) };
        if status != 0 {
            return Err(status);
        }
        let status = unsafe { (services.trace_spawn_published)(context, actor, spawned) };
        if status != 0 {
            return Err(status);
        }
        if !spawned.is_null() {
            let status = unsafe { (services.set_tag)(context, spawned, name) };
            if status != 0 {
                return Err(status);
            }
        }
    }
    unsafe { *out = spawned };
    Ok(())
}

/// Runs the script-native destroy operation.
///
/// # Safety
/// The actor, result storage and service callback must be valid for the duration of the call.
pub unsafe fn exec_destroy(
    actor: *mut c_void,
    services: &Hp2ActorSpawnServices,
    out: *mut u32,
) -> Result<(), i32> {
    if out.is_null() {
        return Err(1);
    }
    let status = unsafe { (services.destroy)(services.context, actor, out) };
    if status == 0 { Ok(()) } else { Err(status) }
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's `GActorSpawnServices` table.
-----------------------------------------------------------------------------*/

static mut SPAWN_SERVICES: Option<Hp2ActorSpawnServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorSpawnServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ActorSpawnServices) {
    unsafe { (&raw mut SPAWN_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ActorSpawnServices, i32> {
    unsafe { (&raw const SPAWN_SERVICES).read().ok_or(1) }
}
