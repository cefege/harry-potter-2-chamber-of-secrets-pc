//! Canonical Core/geometry composition for the existing Pawn reachability policy.

use std::{cell::{Cell, RefCell}, ffi::c_void, ptr};
use hp2_abi::{FVector, engine::generated::{AActor, APawn, FRotator, ULevel}};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::{pawn::{Hp2PawnServices, PawnHitBody}, pawn_reach::Hp2PawnReachServices};

struct Context { access: CoreAccess<'static>, error: RefCell<Option<RuntimeError>> }
thread_local! { static ACTIVE: Cell<*mut Context> = const { Cell::new(ptr::null_mut()) }; }
struct Scope(*mut Context);
impl Drop for Scope { fn drop(&mut self) { ACTIVE.with(|slot| slot.set(self.0)); } }
fn with_context<T>(access: CoreAccess<'_>, body: impl FnOnce() -> Result<T, i32>) -> RuntimeResult<T> {
    let mut context = Context {
        // The synchronous, reentrant scope is popped before the borrowed Core can expire.
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
        error: RefCell::new(None),
    };
    let _scope = Scope(ACTIVE.with(|slot| slot.replace(&raw mut context)));
    let result = body();
    match result {
        Ok(value) => Ok(value),
        Err(status) => Err(context.error.into_inner().unwrap_or_else(||
            RuntimeError::new(&format!("Canonical Pawn reach policy failed ({status})")))),
    }
}
fn output<T>(out: *mut T, body: impl FnOnce(CoreAccess<'_>) -> RuntimeResult<T>) -> i32 {
    ACTIVE.with(|slot| {
        let context = slot.get();
        if context.is_null() { return 1; }
        let context = unsafe { &*context };
        match body(context.access) {
            Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
            Err(error) => { let mut first = context.error.borrow_mut(); if first.is_none() { *first = Some(error); } 1 }
        }
    })
}
fn status(body: impl FnOnce(CoreAccess<'_>) -> RuntimeResult<()>) -> i32 {
    let mut result = ();
    output(&raw mut result, body)
}
unsafe extern "C" fn move_actor(level: *mut c_void, actor: *mut c_void, delta: *const c_void, rotation: *const c_void, hit: *mut c_void, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        crate::level_move_owner::move_actor(access, level.cast(), actor.cast(), delta.cast::<FVector>().read_unaligned(),
            rotation.cast::<FRotator>().read_unaligned(), hit.cast(), true, true, false, false).map(i32::from)
    })
}
unsafe extern "C" fn far_move(level: *mut c_void, actor: *mut c_void, dest: *const c_void, out: *mut i32) -> i32 {
    unsafe { far_move_flags(level, actor, dest, 1, 1, out) }
}
unsafe extern "C" fn far_move_flags(level: *mut c_void, actor: *mut c_void, dest: *const c_void, test: i32, no_check: i32, out: *mut i32) -> i32 {
    output(out, |access| unsafe {
        crate::level_move_owner::far_move_actor(access, level.cast(), actor.cast(), dest.cast::<FVector>().read_unaligned(), test != 0, no_check != 0).map(i32::from)
    })
}
unsafe extern "C" fn water_line(pawn: *mut c_void, start: *const c_void, end: *mut c_void) -> i32 {
    status(|access| unsafe { crate::actor_physics_owner::find_water_line(access, pawn.cast(), start.cast::<FVector>().read_unaligned(), end.cast()) })
}
unsafe extern "C" fn with_hit(raw: *mut c_void, body: PawnHitBody) -> i32 {
    let mut hit = crate::levact::check_result(1.);
    unsafe { body(raw, (&raw mut hit).cast()) }
}
unsafe extern "C" fn line_check(level: *mut c_void, source: *mut c_void, hit: *mut c_void, end: *const c_void, start: *const c_void) -> i32 {
    status(|access| unsafe {
        crate::level_trace_owner::single_line_check(access, level.cast(), hit.cast(), source.cast(),
            end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned(), 0x06,
            FVector { x: 0., y: 0., z: 0. }, 0).map(|_| ())
    })
}
unsafe extern "C" fn point_region(level: *mut c_void, actor: *mut c_void, point: *const c_void, out: *mut c_void) -> i32 {
    output(out.cast::<hp2_abi::engine::generated::FPointRegion>(), |_| unsafe {
        let level = level.cast::<ULevel>();
        let actor = actor.cast::<AActor>();
        crate::collision_owner::CollisionOwner::model_point_region((*level).model.cast(), (*actor).level, point.cast::<FVector>().read_unaligned())
    })
}
unsafe extern "C" fn fast_line(level: *mut c_void, end: *const c_void, start: *const c_void, out: *mut i32) -> i32 {
    output(out, |_| unsafe {
        crate::collision_owner::model_fast_line_check((*level.cast::<ULevel>()).model.cast(), end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned()).map(i32::from)
    })
}
unsafe extern "C" fn is_a(object: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    output(out, |access| {
        let name = match kind { 0 => "Pawn", 1 => "Inventory", 2 => "Trigger", 3 => "WarpZoneMarker",
            _ => return Err(RuntimeError::new("Invalid canonical reach class query")) };
        unsafe { crate::actor_physics_owner::is_a_named(access, object, name).map(i32::from) }
    })
}
unsafe extern "C" fn is_editor(out: *mut i32) -> i32 { output(out, |access| Ok(i32::from(access.process_mode().is_editor))) }

pub(crate) static SERVICES: Hp2PawnReachServices = Hp2PawnReachServices {
    pawn: Hp2PawnServices { move_actor, far_move_actor: far_move, find_water_line: water_line, with_hit },
    far_move_actor_flags: far_move_flags, single_line_check: line_check, point_region,
    fast_line_check: fast_line, is_a, is_editor,
};

pub(crate) fn with_services<T>(access: CoreAccess<'_>, body: impl FnOnce(&'static Hp2PawnReachServices) -> RuntimeResult<T>) -> RuntimeResult<T> {
    let mut failure = None;
    let result = with_context(access, || match body(&SERVICES) {
        Ok(value) => Ok(value),
        Err(error) => { failure = Some(error); Err(1) }
    });
    result.map_err(|error| failure.unwrap_or(error))
}

/// Runs low-level source point reachability, retaining its real pose changes and restoration.
///
/// # Safety
/// Pawn/world/model/regions and reached actors remain live through synchronous movement.
pub unsafe fn point_reachable(access: CoreAccess<'_>, pawn: *mut APawn, point: FVector, know_visible: i32) -> RuntimeResult<i32> {
    with_context(access, || unsafe { crate::pawn_reach::point_reachable(&SERVICES, pawn, point, know_visible) })
}

/// Low-level actor reachability used by sensing and routes, without script-native shortcuts.
///
/// # Safety
/// Same live world/region contract as point_reachable; target is null or a live actor.
pub unsafe fn actor_reachable(access: CoreAccess<'_>, pawn: *mut APawn, target: *mut AActor, know_visible: i32) -> RuntimeResult<i32> {
    with_context(access, || unsafe { crate::pawn_reach::actor_reachable(&SERVICES, pawn, target, know_visible) })
}
