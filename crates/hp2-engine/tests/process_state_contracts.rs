//! Data-free contracts for the production ProcessState policy and VM loop.
use std::collections::VecDeque;
use std::ffi::{CStr, c_char, c_void};

use hp2_engine::lev_tic::{
    Hp2ProcessStateServices, Hp2ProcessStateSnapshot, process_state, process_state_steps,
};

const DELTA: f32 = 0.125;
const FAILURE: i32 = 73;

#[derive(Debug, PartialEq)]
enum Call {
    Snapshot,
    Trace(String, usize),
    Enter,
    RandBegin,
    TimerBegin,
    Poll(u32),
    Step,
    TimerEnd,
    RandEnd,
    Observe(u32),
}

impl Call {
    fn kind(&self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Trace(..) => "trace",
            Self::Enter => "scope",
            Self::Poll(..) => "latent",
            Self::Step => "step",
            Self::Observe(..) => "observe",
            _ => "instrumentation",
        }
    }
}

struct Context {
    state: Hp2ProcessStateSnapshot,
    after_latent: Option<Hp2ProcessStateSnapshot>,
    after_steps: VecDeque<Hp2ProcessStateSnapshot>,
    calls: Vec<Call>,
    fail: Option<(&'static str, usize)>,
    diagnostic_state: usize,
}

fn state(node: usize) -> Hp2ProcessStateSnapshot {
    Hp2ProcessStateSnapshot {
        state_node: node as *mut c_void,
        has_frame: 1,
        has_code: 1,
        role: 4,
        ..Default::default()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self {
            state: state(1),
            after_latent: None,
            after_steps: VecDeque::new(),
            calls: Vec::new(),
            fail: None,
            diagnostic_state: 0,
        }
    }
}

impl Context {
    fn record(&mut self, call: Call) -> i32 {
        let kind = call.kind();
        self.calls.push(call);
        let count = self.calls.iter().filter(|call| call.kind() == kind).count();
        if self.fail == Some((kind, count)) {
            FAILURE
        } else {
            0
        }
    }

    fn run(&mut self) -> i32 {
        let services = services(self);
        unsafe { process_state(&services, DELTA) }
    }

    fn step_count(&self) -> usize {
        self.calls
            .iter()
            .filter(|call| **call == Call::Step)
            .count()
    }
}

unsafe extern "C" fn snapshot(
    context: *mut c_void,
    _read_gate: i32,
    out: *mut Hp2ProcessStateSnapshot,
) -> i32 {
    let context = unsafe { &mut *context.cast::<Context>() };
    let status = context.record(Call::Snapshot);
    if status == 0 {
        unsafe { *out = context.state };
    }
    status
}

unsafe extern "C" fn trace(context: *mut c_void, node: *mut c_void, outcome: *const c_char) -> i32 {
    let context = unsafe { &mut *context.cast::<Context>() };
    let outcome = unsafe { CStr::from_ptr(outcome) }
        .to_str()
        .unwrap()
        .to_owned();
    context.record(Call::Trace(outcome, node as usize))
}

unsafe extern "C" fn scoped(context: *mut c_void, delta: f32) -> i32 {
    let context = unsafe { &mut *context.cast::<Context>() };
    let status = context.record(Call::Enter);
    if status != 0 {
        return status;
    }
    let mut old_state = context.state.state_node;
    context.calls.push(Call::RandBegin);
    context.calls.push(Call::TimerBegin);
    let services = services(context);
    let status = unsafe { process_state_steps(&services, delta, &mut old_state) };
    context.calls.push(Call::TimerEnd);
    context.calls.push(Call::RandEnd);
    context.diagnostic_state = old_state as usize;
    status
}

unsafe extern "C" fn poll(context: *mut c_void, delta: f32) -> i32 {
    let context = unsafe { &mut *context.cast::<Context>() };
    let status = context.record(Call::Poll(delta.to_bits()));
    if status == 0 {
        if let Some(state) = context.after_latent.take() {
            context.state = state;
        }
    }
    status
}

unsafe extern "C" fn step(context: *mut c_void) -> i32 {
    let context = unsafe { &mut *context.cast::<Context>() };
    let status = context.record(Call::Step);
    if status == 0 {
        context.state = context
            .after_steps
            .pop_front()
            .unwrap_or(Hp2ProcessStateSnapshot {
                has_code: 0,
                ..context.state
            });
    }
    status
}

unsafe extern "C" fn observe(context: *mut c_void, delta: f32) -> i32 {
    unsafe { &mut *context.cast::<Context>() }.record(Call::Observe(delta.to_bits()))
}

fn services(context: &mut Context) -> Hp2ProcessStateServices {
    Hp2ProcessStateServices {
        context: (context as *mut Context).cast(),
        snapshot,
        trace,
        run_scoped: scoped,
        poll_latent: poll,
        step,
        observe,
    }
}

#[test]
fn every_skip_traces_once_then_observes_with_gate_precedence() {
    for (frame, code, role, pending, expected) in [
        (0, 0, 0, 1, "skip_no_frame"),
        (1, 0, 0, 1, "skip_no_code"),
        (1, 1, 3, 1, "skip_role"),
        (1, 1, 4, 1, "skip_pending_kill"),
    ] {
        let mut context = Context::default();
        context.state.has_frame = frame;
        context.state.has_code = code;
        context.state.role = role;
        context.state.pending_kill = pending;
        if frame == 0 {
            context.state.state_node = std::ptr::null_mut();
        }
        assert_eq!(context.run(), 0);
        assert_eq!(
            context.calls,
            [
                Call::Snapshot,
                Call::Trace(expected.into(), if frame == 0 { 0 } else { 1 }),
                Call::Observe(DELTA.to_bits()),
            ]
        );
    }
}

#[test]
fn authority_or_simulated_dispatches_with_exact_service_order() {
    for (role, simulated) in [(4, 0), (5, 0), (0, 1), (3, 1)] {
        let mut context = Context::default();
        context.state.role = role;
        context.state.simulated = simulated;
        assert_eq!(context.run(), 0);
        assert_eq!(
            context.calls,
            [
                Call::Snapshot,
                Call::Trace("dispatched".into(), 1),
                Call::Enter,
                Call::RandBegin,
                Call::TimerBegin,
                Call::Snapshot,
                Call::Step,
                Call::Snapshot,
                Call::TimerEnd,
                Call::RandEnd,
                Call::Observe(DELTA.to_bits()),
            ]
        );
    }
}

#[test]
fn latent_poll_runs_once_and_only_completion_allows_steps() {
    for completes in [false, true] {
        let mut context = Context::default();
        context.state.latent_action = 123;
        if completes {
            context.after_latent = Some(state(1));
        }
        assert_eq!(context.run(), 0);
        assert_eq!(context.step_count(), usize::from(completes));
        assert_eq!(
            context
                .calls
                .iter()
                .filter(|call| call.kind() == "latent")
                .count(),
            1
        );
        assert_eq!(
            &context.calls[5..8],
            &[Call::Snapshot, Call::Poll(DELTA.to_bits()), Call::Snapshot,]
        );
        assert_eq!(context.calls.last(), Some(&Call::Observe(DELTA.to_bits())));
    }
}

#[test]
fn latent_state_change_counts_on_the_first_vm_step() {
    let mut context = Context::default();
    context.state.latent_action = 123;
    context.after_latent = Some(state(2));
    context.after_steps = [state(2), state(3), state(4), state(5), state(6), state(7)].into();
    assert_eq!(context.run(), 0);
    assert_eq!(context.step_count(), 5);
    assert_eq!(context.diagnostic_state, 6);
    assert_eq!(context.after_steps.len(), 1);
}

#[test]
fn unchanged_state_has_no_step_count_limit() {
    let mut context = Context::default();
    context.after_steps = vec![state(1); 9].into();
    assert_eq!(context.run(), 0);
    assert_eq!(context.step_count(), 10);
    assert_eq!(context.diagnostic_state, 1);
}

#[test]
fn four_transitions_continue_but_the_fifth_breaks_immediately() {
    for transitions in [4, 5] {
        let mut context = Context::default();
        context.after_steps = (2..=transitions + 1).map(state).collect();
        context.after_steps.push_back(state(transitions + 1));
        assert_eq!(context.run(), 0);
        assert_eq!(context.diagnostic_state, transitions + 1);
        if transitions == 4 {
            assert_eq!(context.step_count(), 6);
            assert_eq!(context.state.has_code, 0);
        } else {
            assert_eq!(context.step_count(), 5);
            assert_eq!(context.state.has_code, 1);
            assert_eq!(context.after_steps.len(), 1);
        }
        assert_eq!(context.calls.last(), Some(&Call::Observe(DELTA.to_bits())));
    }
}

#[test]
fn each_loop_gate_stops_before_another_step() {
    for (deleted, code, latent) in [(1, 1, 0), (0, 0, 0), (0, 1, 123)] {
        let mut context = Context::default();
        context.after_steps.push_back(Hp2ProcessStateSnapshot {
            delete_me: deleted,
            has_code: code,
            latent_action: latent,
            ..state(2)
        });
        assert_eq!(context.run(), 0);
        assert_eq!(context.step_count(), 1);
        assert_eq!(context.diagnostic_state, 2);
        assert!(!context.calls.iter().any(|call| call.kind() == "latent"));
    }
}

#[test]
fn deleted_actor_still_polls_latent_before_testing_loop_gate() {
    let mut context = Context::default();
    context.state.delete_me = 1;
    context.state.latent_action = 123;
    assert_eq!(context.run(), 0);
    assert_eq!(context.step_count(), 0);
    assert!(context.calls.contains(&Call::Poll(DELTA.to_bits())));
    assert_eq!(context.calls.last(), Some(&Call::Observe(DELTA.to_bits())));
}

#[test]
fn service_failures_stop_calls_and_unwind_scope_before_returning() {
    for fail in [
        ("snapshot", 1),
        ("trace", 1),
        ("scope", 1),
        ("snapshot", 2),
        ("latent", 1),
        ("snapshot", 3),
        ("step", 1),
        ("snapshot", 4),
        ("observe", 1),
    ] {
        let mut context = Context::default();
        context.state.latent_action = 123;
        context.after_latent = Some(state(1));
        context.fail = Some(fail);
        assert_eq!(context.run(), FAILURE, "{fail:?}");
        let failed_index = context
            .calls
            .iter()
            .enumerate()
            .filter(|(_, call)| call.kind() == fail.0)
            .nth(fail.1 - 1)
            .unwrap()
            .0;
        let expected_tail: &[Call] =
            if context.calls[..failed_index].contains(&Call::TimerBegin) && fail.0 != "observe" {
                &[Call::TimerEnd, Call::RandEnd]
            } else {
                &[]
            };
        assert_eq!(
            &context.calls[failed_index + 1..],
            expected_tail,
            "{fail:?}"
        );
    }
}

#[test]
fn observer_failure_is_returned_even_when_dispatch_is_skipped() {
    let mut context = Context::default();
    context.state.has_code = 0;
    context.fail = Some(("observe", 1));
    assert_eq!(context.run(), FAILURE);
    assert_eq!(
        context.calls,
        [
            Call::Snapshot,
            Call::Trace("skip_no_code".into(), 1),
            Call::Observe(DELTA.to_bits()),
        ]
    );
}
