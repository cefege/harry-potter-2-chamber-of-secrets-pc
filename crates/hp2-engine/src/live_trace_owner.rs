//! Typed, synchronous observations for launcher diagnostics not covered by VM
//! or spawn instrumentation. The callbacks are copied out of thread-local
//! registration before invocation, so nested engine/VM calls never overlap a
//! mutable observer borrow.

use hp2_abi::engine::generated::{AActor, UClass};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{cell::Cell, ffi::c_char, ptr};

use crate::core_owner::{EngineOwner, EngineState};

#[derive(Clone, Copy)]
pub struct LiveTraceObserver {
    pub post_init_execution: fn(*mut AActor),
    pub first_renderer_candidate: fn(*mut AActor, *mut std::ffi::c_void),
    pub shadow_admission_begin_pass: fn() -> i32,
    #[allow(clippy::too_many_arguments)]
    pub shadow_admission_record: fn(
        i32,
        *mut AActor,
        *mut std::ffi::c_void,
        *const c_char,
        bool,
        *mut AActor,
        *mut AActor,
        bool,
        *mut AActor,
        bool,
        bool,
    ),
    pub creature_tick_dispatch: fn(*mut AActor, f32),
    pub creature_class_resolution: fn(*mut UClass, *mut AActor),
    pub global_tick_begin_dispatch: fn(*mut AActor, *const c_char, *const c_char) -> u64,
    pub global_tick_event_tick: fn(u64),
    pub global_tick_process_state: fn(*mut AActor, *mut std::ffi::c_void, *const c_char),
    pub global_tick_end_dispatch: fn(u64, *const c_char),
}

thread_local! {
    static OBSERVER: Cell<Option<(*const EngineState, LiveTraceObserver)>> = const { Cell::new(None) };
}

fn owner_state(owner: &EngineOwner) -> RuntimeResult<*const EngineState> {
    owner
        .access()
        .owner_context::<EngineState>()
        .map(ptr::from_ref)
        .ok_or_else(|| RuntimeError::new("Live trace observer requires a canonical EngineState"))
}

/// Attaches the live canonical observer to this EngineOwner on its engine thread.
pub fn install_observer(owner: &EngineOwner, observer: LiveTraceObserver) -> RuntimeResult<()> {
    let state = owner_state(owner)?;
    OBSERVER.with(|slot| {
        if slot.get().is_some() {
            return Err(RuntimeError::new("A live trace observer is already installed"));
        }
        slot.set(Some((state, observer)));
        Ok(())
    })
}

/// Detaches the observer before its EngineOwner shuts down.
pub fn clear_observer(owner: &EngineOwner) -> RuntimeResult<()> {
    let state = owner_state(owner)?;
    OBSERVER.with(|slot| match slot.get() {
        Some((registered, _)) if registered == state => {
            slot.set(None);
            Ok(())
        }
        None => Ok(()),
        _ => Err(RuntimeError::new("Live trace observer does not belong to this EngineOwner")),
    })
}

fn attached(access: CoreAccess<'_>) -> Option<LiveTraceObserver> {
    let state = access.owner_context::<EngineState>().map(ptr::from_ref)?;
    OBSERVER.with(|slot| {
        slot.get()
            .and_then(|(registered, observer)| (registered == state).then_some(observer))
    })
}

pub(crate) fn post_init_execution(access: CoreAccess<'_>, actor: *mut AActor) {
    if let Some(observer) = attached(access) {
        (observer.post_init_execution)(actor);
    }
}

pub(crate) fn first_renderer_candidate(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    shadow: *mut std::ffi::c_void,
) {
    if let Some(observer) = attached(access) {
        (observer.first_renderer_candidate)(actor, shadow);
    }
}

pub(crate) fn shadow_admission_begin_pass(access: CoreAccess<'_>) -> i32 {
    attached(access).map_or(0, |observer| (observer.shadow_admission_begin_pass)())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn shadow_admission_record(
    access: CoreAccess<'_>,
    pass: i32,
    actor: *mut AActor,
    shadow: *mut std::ffi::c_void,
    candidate_status: *const c_char,
    update_eligible: bool,
    viewport_actor: *mut AActor,
    view_target: *mut AActor,
    behind_view: bool,
    recursion_parent_actor: *mut AActor,
    perspective: bool,
    world_dynamics: bool,
) {
    if let Some(observer) = attached(access) {
        (observer.shadow_admission_record)(
            pass,
            actor,
            shadow,
            candidate_status,
            update_eligible,
            viewport_actor,
            view_target,
            behind_view,
            recursion_parent_actor,
            perspective,
            world_dynamics,
        );
    }
}

pub(crate) fn creature_tick_dispatch(access: CoreAccess<'_>, actor: *mut AActor, delta: f32) {
    if let Some(observer) = attached(access) {
        (observer.creature_tick_dispatch)(actor, delta);
    }
}

pub(crate) fn creature_class_resolution(
    access: CoreAccess<'_>,
    class: *mut UClass,
    actor: *mut AActor,
) {
    if let Some(observer) = attached(access) {
        (observer.creature_class_resolution)(class, actor);
    }
}

pub(crate) fn global_tick_begin_dispatch(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    relation: *const c_char,
    root_admission: *const c_char,
) -> u64 {
    attached(access).map_or(u64::MAX, |observer| {
        (observer.global_tick_begin_dispatch)(actor, relation, root_admission)
    })
}

pub(crate) fn global_tick_event_tick(access: CoreAccess<'_>, token: u64) {
    if token != u64::MAX
        && let Some(observer) = attached(access)
    {
        (observer.global_tick_event_tick)(token);
    }
}

pub(crate) fn global_tick_process_state(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    state_object: *mut std::ffi::c_void,
    outcome: *const c_char,
) {
    if let Some(observer) = attached(access) {
        (observer.global_tick_process_state)(actor, state_object, outcome);
    }
}

pub(crate) fn global_tick_end_dispatch(
    access: CoreAccess<'_>,
    token: u64,
    tick_outcome: *const c_char,
) {
    if token != u64::MAX
        && let Some(observer) = attached(access)
    {
        (observer.global_tick_end_dispatch)(token, tick_outcome);
    }
}
