use core::ffi::c_void;
use hp2_engine::actor_rpc::{self, Hp2ActorRpcMode, Hp2ActorRpcServices, Hp2ActorRpcSnapshot};

const FUNCTION: usize = 10;
const ROOT: usize = 11;
const OWNER: usize = 20;
const CLIENT: usize = 30;
const SERVER: usize = 31;
const DEMO: usize = 32;
const CODE: usize = 40;
const FAILURE: i32 = 73;
fn ptr(value: usize) -> *mut c_void {
    value as *mut c_void
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Snapshot(usize),
    DemoDriver,
    RecordDemo(usize),
    Mode(i32),
    TopOwner,
    ClientConnection(usize),
    Super(usize),
    Condition(usize, i32, i32, i32),
    ServerConnection,
    DemoConnection,
    Check(usize),
    Ready(usize),
    Send(usize, usize, i32),
    Exchange,
    Flags(i32, i32),
    Reset,
    Code,
    Restore(usize),
}

#[derive(Clone)]
struct World {
    calls: Vec<Call>,
    fail_at: usize,
    flags: Hp2ActorRpcSnapshot,
    root_reliable: i32,
    role: i32,
    remote_role: i32,
    mode: i32,
    mode_after_condition: Option<i32>,
    demo: i32,
    playback: i32,
    owner: usize,
    client: usize,
    condition: i32,
    ready: i32,
    recording: i32,
    client_recording: i32,
    client_net_func: i32,
    has_stack: bool,
    code: usize,
}
impl Default for World {
    fn default() -> Self {
        Self {
            calls: vec![],
            fail_at: 0,
            flags: Hp2ActorRpcSnapshot {
                is_net: 1,
                ..Default::default()
            },
            root_reliable: 0,
            role: 4,
            remote_role: 2,
            mode: 2,
            mode_after_condition: None,
            demo: 0,
            playback: 0,
            owner: OWNER,
            client: CLIENT,
            condition: 1,
            ready: 1,
            recording: 0,
            client_recording: 0,
            client_net_func: 1,
            has_stack: true,
            code: CODE,
        }
    }
}
impl World {
    fn call(&mut self, call: Call) -> i32 {
        self.calls.push(call);
        if self.calls.len() == self.fail_at {
            FAILURE
        } else {
            0
        }
    }
}
macro_rules! enter {
    ($ctx:ident, $world:ident, $call:expr) => {
        let $world = unsafe { &mut *$ctx.cast::<World>() };
        let result = $world.call($call);
        if result != 0 {
            return result;
        }
    };
}
unsafe extern "C" fn snapshot(
    ctx: *mut c_void,
    function: *mut c_void,
    out: *mut Hp2ActorRpcSnapshot,
) -> i32 {
    enter!(ctx, w, Call::Snapshot(function as usize));
    let mut flags = w.flags;
    flags.authority = i32::from(w.role == 4);
    flags.simulated_proxy_or_less = i32::from(w.role <= 2);
    if function as usize == ROOT {
        flags.is_reliable = w.root_reliable;
    }
    unsafe {
        *out = flags;
    }
    0
}
unsafe extern "C" fn demo_driver(ctx: *mut c_void, present: *mut i32, playback: *mut i32) -> i32 {
    enter!(ctx, w, Call::DemoDriver);
    unsafe {
        *present = w.demo;
        *playback = w.playback;
    }
    0
}
unsafe extern "C" fn record_demo(ctx: *mut c_void, function: *mut c_void) -> i32 {
    {
        enter!(ctx, w, Call::RecordDemo(function as usize));
    }
    let s = services(ctx);
    let mut local_function = function;
    unsafe {
        actor_rpc::process_demo(&s, &mut local_function)
            .err()
            .unwrap_or(0)
    }
}
unsafe extern "C" fn mode(ctx: *mut c_void, demo: i32, out: *mut Hp2ActorRpcMode) -> i32 {
    enter!(ctx, w, Call::Mode(demo));
    unsafe {
        *out = Hp2ActorRpcMode {
            standalone: i32::from(w.mode == 0),
            client: i32::from(w.mode == 1),
            server: i32::from(w.mode == 2 || w.mode == 3),
        };
    }
    0
}
unsafe extern "C" fn top_owner(ctx: *mut c_void, out: *mut *mut c_void) -> i32 {
    enter!(ctx, w, Call::TopOwner);
    unsafe {
        *out = ptr(w.owner);
    }
    0
}
unsafe extern "C" fn client_connection(
    ctx: *mut c_void,
    top: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    enter!(ctx, w, Call::ClientConnection(top as usize));
    unsafe {
        *out = ptr(w.client);
    }
    0
}
unsafe extern "C" fn super_function(
    ctx: *mut c_void,
    function: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    enter!(ctx, w, Call::Super(function as usize));
    unsafe {
        *out = if function as usize == FUNCTION {
            ptr(ROOT)
        } else {
            ptr(0)
        };
    }
    0
}
unsafe extern "C" fn condition(ctx: *mut c_void, function: *mut c_void, out: *mut i32) -> i32 {
    enter!(
        ctx,
        w,
        Call::Condition(function as usize, w.role, w.recording, w.client_recording)
    );
    if let Some(mode) = w.mode_after_condition {
        w.mode = mode;
    }
    unsafe {
        *out = w.condition;
    }
    0
}
unsafe extern "C" fn server_connection(ctx: *mut c_void, out: *mut *mut c_void) -> i32 {
    enter!(ctx, w, Call::ServerConnection);
    unsafe {
        *out = ptr(SERVER);
    }
    0
}
unsafe extern "C" fn demo_connection(ctx: *mut c_void, out: *mut *mut c_void) -> i32 {
    enter!(ctx, w, Call::DemoConnection);
    unsafe {
        *out = ptr(DEMO);
    }
    0
}
unsafe extern "C" fn check_connection(ctx: *mut c_void, connection: *mut c_void) -> i32 {
    enter!(ctx, w, Call::Check(connection as usize));
    if connection.is_null() { FAILURE } else { 0 }
}
unsafe extern "C" fn net_ready(ctx: *mut c_void, connection: *mut c_void, out: *mut i32) -> i32 {
    enter!(ctx, w, Call::Ready(connection as usize));
    unsafe {
        *out = w.ready;
    }
    0
}
unsafe extern "C" fn send(
    ctx: *mut c_void,
    connection: *mut c_void,
    function: *mut c_void,
    server: i32,
) -> i32 {
    let w = unsafe { &mut *ctx.cast::<World>() };
    let result = w.call(Call::Send(connection as usize, function as usize, server));
    // Parameter stepping can advance the VM before serialization throws.
    if w.has_stack {
        w.code += 1;
    }
    result
}
unsafe extern "C" fn exchange_roles(ctx: *mut c_void) -> i32 {
    enter!(ctx, w, Call::Exchange);
    core::mem::swap(&mut w.role, &mut w.remote_role);
    0
}
unsafe extern "C" fn demo_flags(ctx: *mut c_void, recording: i32, client: i32) -> i32 {
    enter!(ctx, w, Call::Flags(recording, client));
    w.recording = recording;
    w.client_recording = client;
    0
}
unsafe extern "C" fn reset(ctx: *mut c_void) -> i32 {
    enter!(ctx, w, Call::Reset);
    w.client_net_func = 0;
    0
}
unsafe extern "C" fn stack_code(ctx: *mut c_void, out: *mut *mut c_void) -> i32 {
    enter!(ctx, w, Call::Code);
    unsafe {
        *out = if w.has_stack { ptr(w.code) } else { ptr(0) };
    }
    0
}
unsafe extern "C" fn restore(ctx: *mut c_void, code: *mut c_void) -> i32 {
    enter!(ctx, w, Call::Restore(code as usize));
    if w.has_stack {
        w.code = code as usize;
    }
    0
}
fn services(context: *mut c_void) -> Hp2ActorRpcServices {
    Hp2ActorRpcServices {
        context,
        snapshot,
        demo_driver,
        record_demo,
        mode,
        top_owner,
        client_connection,
        super_function,
        condition,
        server_connection,
        demo_connection,
        check_connection,
        net_ready,
        send,
        exchange_roles,
        demo_flags,
        reset_client_demo_net_func: reset,
        stack_code,
        restore_stack_code: restore,
    }
}
fn remote(w: &mut World) -> (Result<i32, i32>, usize) {
    let s = services((w as *mut World).cast());
    let mut function = ptr(FUNCTION);
    let result = unsafe { actor_rpc::process_remote(&s, &mut function) };
    (result, function as usize)
}
fn demo(w: &mut World) -> (Result<(), i32>, usize) {
    let s = services((w as *mut World).cast());
    let mut function = ptr(FUNCTION);
    let result = unsafe { actor_rpc::process_demo(&s, &mut function) };
    (result, function as usize)
}

#[test]
fn rejection_absorption_and_playback_precedence() {
    for deleted in [false, true] {
        let mut w = World {
            demo: 1,
            playback: 1,
            ..Default::default()
        };
        if deleted {
            w.flags.deleted = 1;
        } else {
            w.flags.is_static = 1;
        }
        assert_eq!(remote(&mut w), (Ok(0), FUNCTION));
        assert_eq!(w.calls, [Call::Snapshot(FUNCTION)]);
    }
    for role in 0..=4 {
        for simulated in [0, 1] {
            let absorb = i32::from(role <= 2 && simulated == 0);
            for playback in [0, 1] {
                let mut w = World {
                    role,
                    demo: playback,
                    playback,
                    ..Default::default()
                };
                w.flags.is_simulated = simulated;
                w.flags.is_net = 0;
                assert_eq!(remote(&mut w), (Ok(absorb), FUNCTION));
                assert!(
                    !w.calls
                        .iter()
                        .any(|c| matches!(c, Call::TopOwner | Call::Condition(..)))
                );
                if playback != 0 {
                    assert_eq!(w.calls, [Call::Snapshot(FUNCTION), Call::DemoDriver]);
                }
            }
        }
    }
    // Playback absorbs before both standalone and FUNC_Net checks.
    for is_net in [0, 1] {
        let mut w = World {
            role: 2,
            mode: 0,
            demo: 1,
            playback: 1,
            ..Default::default()
        };
        w.flags.is_net = is_net;
        assert_eq!(remote(&mut w), (Ok(1), FUNCTION));
        assert_eq!(w.calls, [Call::Snapshot(FUNCTION), Call::DemoDriver]);
    }
}

#[test]
fn standalone_records_demo_before_returning_zero_and_restores_code() {
    let mut w = World {
        role: 2,
        mode: 0,
        demo: 1,
        ..Default::default()
    };
    assert_eq!(remote(&mut w), (Ok(0), FUNCTION));
    assert_eq!(
        w.calls,
        [
            Call::Snapshot(FUNCTION),
            Call::DemoDriver,
            Call::RecordDemo(FUNCTION),
            Call::Snapshot(FUNCTION),
            Call::Mode(1),
            Call::Super(FUNCTION),
            Call::Super(ROOT),
            Call::Flags(1, 0),
            Call::Condition(ROOT, 2, 1, 0),
            Call::Flags(0, 0),
            Call::Reset,
            Call::DemoConnection,
            Call::Check(DEMO),
            Call::Code,
            Call::Send(DEMO, ROOT, 1),
            Call::Restore(CODE),
            Call::Mode(0),
        ]
    );
    assert_eq!(w.code, CODE);
    let mut w = World {
        role: 2,
        mode: 0,
        ..Default::default()
    };
    assert_eq!(remote(&mut w).0, Ok(0));
}

#[test]
fn authority_requires_player_owner_and_client_connection() {
    for (owner, client) in [(0, CLIENT), (OWNER, 0)] {
        let mut w = World {
            owner,
            client,
            ..Default::default()
        };
        assert_eq!(remote(&mut w), (Ok(0), FUNCTION));
        assert!(!w.calls.iter().any(|c| matches!(c, Call::Super(..))));
        assert_eq!(
            w.calls.last(),
            Some(&if owner == 0 {
                Call::TopOwner
            } else {
                Call::ClientConnection(OWNER)
            })
        );
    }
    let mut w = World {
        role: 2,
        mode: 1,
        owner: 0,
        client: 0,
        ..Default::default()
    };
    assert_eq!(remote(&mut w), (Ok(1), ROOT));
    assert!(w.calls.contains(&Call::TopOwner));
    assert!(!w.calls.contains(&Call::ClientConnection(OWNER)));
    assert!(w.calls.contains(&Call::Send(SERVER, ROOT, 0)));
}

#[test]
fn replication_condition_returns_original_absorb_and_precedes_connection_selection() {
    for role in [2, 3, 4] {
        let mut w = World {
            role,
            condition: 0,
            ..Default::default()
        };
        assert_eq!(remote(&mut w), (Ok(i32::from(role <= 2)), ROOT));
        assert_eq!(w.calls.last(), Some(&Call::Condition(ROOT, role, 0, 0)));
    }
    let mut w = World {
        mode_after_condition: Some(1),
        ..Default::default()
    };
    assert_eq!(remote(&mut w).0, Ok(1));
    assert!(w.calls.contains(&Call::Send(SERVER, ROOT, 0)));
}

#[test]
fn server_client_routes_reliability_and_saturation() {
    for mode in [1, 2, 3] {
        for reliable in [0, 1] {
            for ready in [0, 1] {
                let mut w = World {
                    mode,
                    root_reliable: reliable,
                    ready,
                    ..Default::default()
                };
                // Root flags, not override flags, decide reliability.
                w.flags.is_reliable = 1 - reliable;
                assert_eq!(remote(&mut w), (Ok(1), ROOT));
                let connection = if mode == 1 { SERVER } else { CLIENT };
                assert!(w.calls.contains(&Call::Check(connection)));
                assert_eq!(w.calls.contains(&Call::Ready(connection)), reliable == 0);
                assert_eq!(
                    w.calls
                        .contains(&Call::Send(connection, ROOT, i32::from(mode != 1))),
                    reliable != 0 || ready != 0
                );
                assert_eq!(w.code, CODE + usize::from(reliable != 0 || ready != 0));
            }
        }
    }
}

#[test]
fn demo_eligibility_and_client_local_authority_rejection() {
    for kind in 0..4 {
        let mut w = World::default();
        match kind {
            0 => w.flags.is_static = 1,
            1 => w.flags.is_net = 0,
            2 => w.flags.net_temporary = 1,
            _ => w.mode = 1,
        }
        assert_eq!(demo(&mut w), (Ok(()), FUNCTION));
        let expected = if kind == 3 {
            vec![Call::Snapshot(FUNCTION), Call::Mode(1)]
        } else {
            vec![Call::Snapshot(FUNCTION)]
        };
        assert_eq!(w.calls, expected);
        assert_eq!(w.client_net_func, 1);
    }
}

#[test]
fn demo_client_server_flags_roles_condition_and_optional_stack() {
    for client in [false, true] {
        for condition in [0, 1] {
            for has_stack in [false, true] {
                let mut w = World {
                    mode: if client { 1 } else { 2 },
                    role: 2,
                    remote_role: 4,
                    condition,
                    has_stack,
                    ..Default::default()
                };
                assert_eq!(demo(&mut w), (Ok(()), ROOT));
                let mut expected = vec![
                    Call::Snapshot(FUNCTION),
                    Call::Mode(1),
                    Call::Super(FUNCTION),
                    Call::Super(ROOT),
                ];
                if client {
                    expected.push(Call::Exchange);
                }
                expected.extend([
                    Call::Flags(1, i32::from(client)),
                    Call::Condition(ROOT, if client { 4 } else { 2 }, 1, i32::from(client)),
                    Call::Flags(0, 0),
                ]);
                if client {
                    expected.push(Call::Exchange);
                }
                expected.push(Call::Reset);
                if condition != 0 {
                    expected.extend([
                        Call::DemoConnection,
                        Call::Check(DEMO),
                        Call::Code,
                        Call::Send(DEMO, ROOT, 1),
                        Call::Restore(if has_stack { CODE } else { 0 }),
                    ]);
                }
                assert_eq!(w.calls, expected);
                assert_eq!(
                    (
                        w.role,
                        w.remote_role,
                        w.recording,
                        w.client_recording,
                        w.client_net_func,
                        w.code
                    ),
                    (2, 4, 0, 0, 0, CODE)
                );
            }
        }
    }
}

#[test]
fn recording_routes_before_network_checks_and_uses_independent_function_slot() {
    let mut w = World {
        demo: 1,
        ..Default::default()
    };
    assert_eq!(remote(&mut w), (Ok(1), ROOT));
    let sends: Vec<_> = w
        .calls
        .iter()
        .filter(|c| matches!(c, Call::Send(..)))
        .cloned()
        .collect();
    assert_eq!(
        sends,
        [Call::Send(DEMO, ROOT, 1), Call::Send(CLIENT, ROOT, 1)]
    );
    assert_eq!(
        w.calls
            .iter()
            .filter(|c| **c == Call::Super(FUNCTION))
            .count(),
        2
    );
    assert_eq!(w.code, CODE + 1);
    let mut w = World {
        role: 2,
        demo: 1,
        ..Default::default()
    };
    w.flags.is_net = 0;
    assert_eq!(remote(&mut w), (Ok(1), FUNCTION));
    assert_eq!(
        w.calls,
        [
            Call::Snapshot(FUNCTION),
            Call::DemoDriver,
            Call::RecordDemo(FUNCTION),
            Call::Snapshot(FUNCTION),
            Call::Mode(0),
            Call::Snapshot(FUNCTION)
        ]
    );
}

#[test]
fn every_callback_failure_stops_at_the_exact_prefix_and_preserves_status() {
    for is_demo in [false, true] {
        for client in [false, true] {
            let seed = World {
                mode: if client { 1 } else { 2 },
                role: if client { 2 } else { 4 },
                remote_role: 4,
                demo: 1,
                ..Default::default()
            };
            let mut baseline = seed.clone();
            if is_demo {
                assert_eq!(demo(&mut baseline).0, Ok(()));
            } else {
                assert_eq!(remote(&mut baseline).0, Ok(1));
            }
            for fail_at in 1..=baseline.calls.len() {
                let mut failed = seed.clone();
                failed.fail_at = fail_at;
                let result = if is_demo {
                    demo(&mut failed).0.map(|_| 1)
                } else {
                    remote(&mut failed).0
                };
                assert_eq!(
                    result,
                    Err(FAILURE),
                    "demo={is_demo} client={client} index={fail_at}"
                );
                assert_eq!(failed.calls, baseline.calls[..fail_at]);
            }
        }
    }
}

#[test]
fn throwing_demo_condition_does_not_run_legacy_cleanup() {
    let mut w = World {
        mode: 1,
        role: 2,
        remote_role: 4,
        fail_at: 7,
        ..Default::default()
    };
    assert_eq!(demo(&mut w), (Err(FAILURE), ROOT));
    assert_eq!(w.calls.last(), Some(&Call::Condition(ROOT, 4, 1, 1)));
    assert_eq!(
        (
            w.role,
            w.remote_role,
            w.recording,
            w.client_recording,
            w.client_net_func
        ),
        (4, 2, 1, 1, 1)
    );
}

#[test]
fn throwing_demo_serializer_does_not_restore_stack_code() {
    let mut baseline = World::default();
    assert_eq!(demo(&mut baseline).0, Ok(()));
    let send_index = baseline
        .calls
        .iter()
        .position(|c| matches!(c, Call::Send(..)))
        .unwrap()
        + 1;
    let mut w = World {
        fail_at: send_index,
        ..Default::default()
    };
    assert_eq!(demo(&mut w), (Err(FAILURE), ROOT));
    assert_eq!(w.calls.last(), Some(&Call::Send(DEMO, ROOT, 1)));
    assert_eq!(w.code, CODE + 1);
    assert_eq!(
        (w.recording, w.client_recording, w.client_net_func),
        (0, 0, 0)
    );
}
