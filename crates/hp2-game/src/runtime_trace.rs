use hp2_abi::engine::generated::{AActor, UClass};
use hp2_core::core_access::RuntimeResult;
use hp2_engine::{EngineOwner, live_trace_owner};
use hp2_launcher::reporters::{
    actor_transition_ledger, creature_generator, global_tick, shadow_admission,
};
use std::ffi::{c_char, c_void};

pub(super) fn install(owner: &EngineOwner) -> RuntimeResult<()> {
    live_trace_owner::install_observer(
        owner,
        live_trace_owner::LiveTraceObserver {
            post_init_execution,
            first_renderer_candidate,
            shadow_admission_begin_pass,
            shadow_admission_record,
            creature_tick_dispatch,
            creature_class_resolution,
            global_tick_begin_dispatch,
            global_tick_event_tick,
            global_tick_process_state,
            global_tick_end_dispatch,
        },
    )
}

pub(super) fn detach(owner: &EngineOwner) -> RuntimeResult<()> {
    live_trace_owner::clear_observer(owner)
}

fn post_init_execution(actor: *mut AActor) {
    crate::reporters::with_owner((), |_| {
        unsafe { actor_transition_ledger::post_init_execution(actor.cast()) };
        Ok(())
    });
}

fn first_renderer_candidate(actor: *mut AActor, shadow: *mut c_void) {
    crate::reporters::with_owner((), |_| {
        unsafe { actor_transition_ledger::first_renderer_candidate(actor.cast(), shadow) };
        Ok(())
    });
}

fn shadow_admission_begin_pass() -> i32 {
    crate::reporters::with_owner(0, |_| Ok(unsafe { shadow_admission::begin_pass() }))
}

#[allow(clippy::too_many_arguments)]
fn shadow_admission_record(
    pass: i32,
    actor: *mut AActor,
    shadow: *mut c_void,
    candidate_status: *const c_char,
    update_eligible: bool,
    viewport_actor: *mut AActor,
    view_target: *mut AActor,
    behind_view: bool,
    recursion_parent_actor: *mut AActor,
    perspective: bool,
    world_dynamics: bool,
) {
    crate::reporters::with_owner((), |_| {
        unsafe {
            shadow_admission::record(
                pass,
                actor.cast(),
                shadow,
                candidate_status,
                i32::from(update_eligible),
                viewport_actor.cast(),
                view_target.cast(),
                i32::from(behind_view),
                recursion_parent_actor.cast(),
                i32::from(perspective),
                i32::from(world_dynamics),
            )
        };
        Ok(())
    });
}

fn creature_tick_dispatch(actor: *mut AActor, delta: f32) {
    crate::reporters::with_owner((), |_| {
        unsafe { creature_generator::tick_dispatch(actor.cast(), delta) };
        Ok(())
    });
}

fn creature_class_resolution(class: *mut UClass, actor: *mut AActor) {
    crate::reporters::with_owner((), |_| {
        unsafe { creature_generator::class_resolution(class.cast(), actor.cast()) };
        Ok(())
    });
}

fn global_tick_begin_dispatch(
    actor: *mut AActor,
    relation: *const c_char,
    root_admission: *const c_char,
) -> u64 {
    crate::reporters::with_owner(u64::MAX, |_| {
        Ok(unsafe { global_tick::begin_dispatch(actor.cast(), relation, root_admission) })
    })
}

fn global_tick_event_tick(token: u64) {
    crate::reporters::with_owner((), |_| {
        unsafe { global_tick::event_tick(token) };
        Ok(())
    });
}

fn global_tick_process_state(actor: *mut AActor, state: *mut c_void, outcome: *const c_char) {
    crate::reporters::with_owner((), |_| {
        unsafe { global_tick::process_state(actor.cast(), state, outcome) };
        Ok(())
    });
}

fn global_tick_end_dispatch(token: u64, outcome: *const c_char) {
    crate::reporters::with_owner((), |_| {
        unsafe { global_tick::end_dispatch(token, outcome) };
        Ok(())
    });
}
