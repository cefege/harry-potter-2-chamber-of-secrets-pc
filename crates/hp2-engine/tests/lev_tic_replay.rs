//! Record-level replays for `UnLevTic.cpp` using the pinned harness fixture.
//!
//! Fixture records are compared after invoking the Rust entrypoint under test.
//! Service slots not reached by a fixture case abort via `never`.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::size_of;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, ULevel, UNetConnection, UNetDriver};
use hp2_engine::lev_tic::{
    ActorPriority, Hp2LevTicServices, actor_can_see, make_priority, sort_priority_ptrs,
};
use hp2_engine::lev_tic_actor::actor_tick as checked_actor_tick;
use hp2_engine::lev_tic_net::tick_net_client;

const LEVELTICK_ALL: i32 = 2;
const EVENT_TICK: i32 = 0;
const EVENT_TIMER: i32 = 1;
const EVENT_ANIM_END: i32 = 3;
const ROLE_SIMULATED_PROXY: u8 = 2;
const ROLE_AUTHORITY: u8 = 4;

thread_local! {
    static OUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static SCN: RefCell<&'static str> = const { RefCell::new("none") };
    static IDS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static EDITOR: RefCell<i32> = const { RefCell::new(0) };
    static NET_HOOKS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static NET_SERVICES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static ACTORSEE_FAST: Cell<i32> = const { Cell::new(0) };
}


unsafe extern "C" fn net_clock(level: *mut c_void, which: i32) {
    NET_SERVICES.with(|calls| {
        calls
            .borrow_mut()
            .push(format!("clock {} {which}", oid(level)));
    });
}

unsafe extern "C" fn net_unclock(level: *mut c_void, which: i32) {
    NET_SERVICES.with(|calls| {
        calls
            .borrow_mut()
            .push(format!("unclock {} {which}", oid(level)));
    });
}



unsafe extern "C" fn net_replicate_player_channels(connection: *mut c_void) {
    NET_SERVICES.with(|calls| {
        calls
            .borrow_mut()
            .push(format!("replicate_channels {}", oid(connection)));
    });
}

unsafe extern "C" fn net_viewports_num(level: *mut c_void) -> i32 {
    NET_SERVICES.with(|calls| {
        calls.borrow_mut().push(format!("viewports {}", oid(level)));
    });
    1
}

unsafe extern "C" fn net_client_travel(level: *mut c_void, url: i32) {
    NET_SERVICES.with(|calls| {
        calls
            .borrow_mut()
            .push(format!("client_travel {} {url}", oid(level)));
    });
    NET_HOOKS.with(|hooks| {
        hooks
            .borrow_mut()
            .push("hook SetClientTravel ENGINE".to_owned())
    });
}

unsafe extern "C" fn net_check_enabled() -> i32 {
    NET_SERVICES.with(|calls| calls.borrow_mut().push("check_enabled".to_owned()));
    0
}

fn netclient_services() -> Hp2LevTicServices {
    let mut svc = all_never();
    svc.level_cycles_clock = checked_level_cycles_clock_net_clock;
    svc.level_cycles_unclock = checked_level_cycles_unclock_net_unclock;
    svc.conn_replicate_player_channels =
        checked_conn_replicate_player_channels_net_replicate_player_channels;
    svc.level_client_viewports_num = checked_level_client_viewports_num_net_viewports_num;
    svc.level_engine_client_travel = checked_level_engine_client_travel_net_client_travel;
    svc.check_enabled = checked_check_enabled_net_check_enabled;
    svc.guard_scope = fixture_guard_scope;
    svc
}

/// Placeholder for every service slot. A call aborts the process, since panics cannot
/// unwind out of `extern "C"`.
unsafe extern "C" fn never() {
    panic!("unexpected service call: a hook not used by this scenario was reached")
}

/// Builds a services table in which every slot is `never`.
fn all_never() -> Hp2LevTicServices {
    let bytes = size_of::<Hp2LevTicServices>();
    assert_eq!(
        bytes % size_of::<usize>(),
        0,
        "services table must hold only pointer slots"
    );
    let slots: Vec<unsafe extern "C" fn()> = (0..bytes / size_of::<usize>())
        .map(|_| never as unsafe extern "C" fn())
        .collect();
    unsafe { std::ptr::read_unaligned(slots.as_ptr().cast::<Hp2LevTicServices>()) }
}

fn rec(event: &str, args: &str) {
    let scn = SCN.with(|s| *s.borrow());
    OUT.with(|o| o.borrow_mut().push(format!("{scn}\t{event}\t{args}")));
}

fn rec_f(event: &str, id: i32, f: f32) {
    rec(event, &format!("{id}\t{:08x}", f.to_bits()));
}

fn rec_id(event: &str, id: i32) {
    rec(event, &id.to_string());
}

unsafe extern "C" fn mock_trace_enabled() -> i32 {
    0
}

/// The oracle's zeroed model has `LastRenderTime == 0` and `TimeSeconds == 100`.
unsafe extern "C" fn mock_zone_time_since_render(_level: *mut c_void, _zone: i32) -> f32 {
    100.0
}

/// The product's `hp2_rs_launcher_creature_generator_tick_dispatch` only prints a diagnostic line
/// that is not part of the record stream.
unsafe extern "C" fn mock_creature_trace_tick(_actor: *mut c_void, _delta: f32) {}

/// Registers an object and returns its fixture id.
fn reg(p: *const c_void) -> i32 {
    IDS.with(|ids| {
        let mut ids = ids.borrow_mut();
        ids.push(p as usize);
        (ids.len() - 1) as i32
    })
}

fn oid(p: *const c_void) -> i32 {
    if p.is_null() {
        return -1;
    }
    IDS.with(|ids| {
        ids.borrow()
            .iter()
            .position(|&q| q == p as usize)
            .map_or(-2, |i| i as i32)
    })
}

unsafe extern "C" fn mock_is_probing(_actor: *mut c_void, _probe: i32) -> i32 {
    1
}

unsafe extern "C" fn mock_actor_event(actor: *mut c_void, event: i32, delta: f32) {
    let id = oid(actor);
    match event {
        EVENT_TICK => rec_f("event_tick", id, delta),
        EVENT_TIMER => rec_id("event_timer", id),
        EVENT_ANIM_END => rec_id("event_anim_end", id),
        _ => panic!("unexpected actor event {event}"),
    }
}

unsafe extern "C" fn mock_process_state(actor: *mut c_void, delta: f32) {
    rec_f("process_state", oid(actor), delta);
}

unsafe extern "C" fn mock_level_is_editor() -> i32 {
    EDITOR.with(|e| *e.borrow())
}
unsafe extern "C" fn actorsee_fast_line(_: *mut c_void, _: *const f32, _: *const f32) -> i32 {
    NET_HOOKS.with(|hooks| {
        hooks
            .borrow_mut()
            .push("hook FastLineCheck MODEL".to_owned())
    });
    ACTORSEE_FAST.with(Cell::get)
}

fn actorsee_services() -> Hp2LevTicServices {
    let mut svc = all_never();
    svc.level_fast_line_check = checked_level_fast_line_check_actorsee_fast_line;
    svc.guard_scope = fixture_guard_scope;
    svc
}

fn services() -> Hp2LevTicServices {
    let mut svc = all_never();
    svc.actor_is_probing = checked_actor_is_probing_mock_is_probing;
    svc.actor_event = checked_actor_event_mock_actor_event;
    svc.actor_process_state = checked_actor_process_state_mock_process_state;
    svc.level_is_editor = checked_level_is_editor_mock_level_is_editor;
    svc.level_zone_time_since_render =
        checked_level_zone_time_since_render_mock_zone_time_since_render;
    svc.actor_creature_trace_tick = checked_actor_creature_trace_tick_mock_creature_trace_tick;
    svc.guard_scope = fixture_guard_scope;
    svc
}
unsafe extern "C" fn mock_priority_channel(_: *mut c_void, _: *mut c_void) -> *mut c_void {
    std::ptr::null_mut()
}



unsafe extern "C" fn mock_net_priority(_: *mut c_void, _: *mut c_void, _: f32, _: f32) -> f32 {
    1.0
}

fn priority_services() -> Hp2LevTicServices {
    let mut svc = all_never();
    svc.conn_actor_channel = checked_conn_actor_channel_mock_priority_channel;
    svc.actor_net_priority = checked_actor_net_priority_mock_net_priority;
    svc.guard_scope = fixture_guard_scope;
    svc
}

fn make_level() -> *mut ULevel {
    let lv = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<ULevel>() }));
    reg(lv.cast());
    lv
}

fn make_info() -> *mut ALevelInfo {
    let li = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<ALevelInfo>() }));
    reg(li.cast());
    li
}

fn make_actor(lv: *mut ULevel, li: *mut ALevelInfo) -> *mut AActor {
    let a = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<AActor>() }));
    reg(a.cast());
    unsafe {
        (*a).x_level = lv.cast();
        (*a).level = li.cast();
    }
    a
}

#[test]
fn lev_tic_fixture_has_exact_family_counts() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let mut records = 0usize;
    for line in FIXTURE.lines() {
        if let Some(rest) = line.strip_prefix("case ") {
            let family = rest.split_whitespace().next().expect("case family");
            *counts.entry(family).or_default() += 1;
            records += 1;
        }
    }
    assert_eq!(records, 222, "fixture case count");
    assert_eq!(
        counts,
        [
            ("actor", 8),
            ("actorsee", 40),
            ("demoplay", 12),
            ("demorec", 12),
            ("level", 23),
            ("netclient", 12),
            ("netserver", 12),
            ("servertick", 40),
            ("tick", 60),
            ("timing", 3),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn lev_tic_actor_fixture_replays_recorded_results() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    struct Case<'a> {
        index: usize,
        input: Vec<&'a str>,
        call: &'a str,
        result: (i32, bool),
        output: Vec<&'a str>,
    }
    let mut cases = Vec::<Case<'_>>::new();
    let mut active: Option<Case<'_>> = None;
    for line in FIXTURE.lines() {
        if let Some(header) = line.strip_prefix("case ") {
            if let Some(case) = active.take() {
                cases.push(case);
            }
            let mut fields = header.split_whitespace();
            if fields.next() == Some("actor") {
                active = Some(Case {
                    index: fields.next().unwrap().parse().unwrap(),
                    input: Vec::new(),
                    call: "",
                    result: (0, false),
                    output: Vec::new(),
                });
            }
        } else if let Some(case) = active.as_mut() {
            if line.starts_with("in ") {
                case.input.push(line);
            } else if let Some(call) = line.strip_prefix("call actor ") {
                case.call = call;
            } else if let Some(result) = line.strip_prefix("res actor ret=") {
                let (ret, b_ticked) = result.split_once(" bTicked=").unwrap();
                case.result = (ret.parse().unwrap(), b_ticked == "1");
            } else if let Some(output) = line.strip_prefix("out ") {
                case.output.push(output);
            }
        }
    }
    if let Some(case) = active {
        cases.push(case);
    }
    assert_eq!(cases.len(), 8, "actor family case count");

    let svc = services();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(case.index, index);
        OUT.with(|out| out.borrow_mut().clear());
        IDS.with(|ids| ids.borrow_mut().clear());
        let level = make_level();
        let info = make_info();
        let actor = make_actor(level, info);
        EDITOR.with(|editor| {
            *editor.borrow_mut() = case
                .call
                .split("editor=")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
        });
        unsafe {
            for input in &case.input {
                let mut fields = input.split_whitespace();
                assert_eq!(fields.next(), Some("in"));
                let object = fields.next().unwrap();
                let field = fields.next().unwrap();
                let value = fields.next().unwrap();
                match (object, field) {
                    ("LEV", "Ticked") => (*level).ticked = value.parse().unwrap(),
                    ("A0", "bTicked") => (*actor).set_b_ticked(value == "1"),
                    ("A0", "bStasis") => (*actor).set_b_stasis(value == "1"),
                    ("A0", "TweenRate") => {
                        (*actor).tween_rate =
                            f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "TweenAlpha") => {
                        (*actor).tween_alpha =
                            f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "AnimFrame") => {
                        (*actor).anim_frame =
                            f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "AnimLast") => {
                        (*actor).anim_last = f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "TimerRate") => {
                        (*actor).timer_rate =
                            f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "TimerCounter") => {
                        (*actor).timer_counter =
                            f32::from_bits(u32::from_str_radix(value, 16).unwrap())
                    }
                    ("A0", "bTimerLoop") => (*actor).set_b_timer_loop(value == "1"),
                    other => panic!("unexpected actor fixture input: {other:?}"),
                }
            }
            let dt = u32::from_str_radix(
                case.call
                    .split("dt=")
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap(),
                16,
            )
            .unwrap();
            let mode: i32 = case
                .call
                .split("mode=")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(case.call.split_whitespace().last(), Some("actor=A0"));
            assert_eq!(
                actor_tick(&svc, actor.cast(), f32::from_bits(dt), mode),
                case.result.0
            );
            assert_eq!((*actor).b_ticked(), case.result.1, "case {}", case.index);
            assert!(
                case.output.is_empty(),
                "case {} output: {:?}",
                case.index,
                case.output
            );
        }
        assert!(
            OUT.with(|out| out.borrow().is_empty()),
            "case {} unexpectedly called a service",
            case.index
        );
    }
}

#[test]
fn lev_tic_netclient_fixture_replays_services_and_results() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    struct Case<'a> {
        index: usize,
        state: i32,
        call: &'a str,
        hooks: Vec<&'a str>,
        result_state: i32,
    }
    let mut cases = Vec::<Case<'_>>::new();
    let mut active: Option<Case<'_>> = None;
    for line in FIXTURE.lines() {
        if let Some(header) = line.strip_prefix("case ") {
            if let Some(case) = active.take() {
                cases.push(case);
            }
            let mut fields = header.split_whitespace();
            if fields.next() == Some("netclient") {
                active = Some(Case {
                    index: fields.next().unwrap().parse().unwrap(),
                    state: 3,
                    call: "",
                    hooks: Vec::new(),
                    result_state: 0,
                });
            }
        } else if let Some(case) = active.as_mut() {
            if let Some(rest) = line.strip_prefix("in CONN0 State ") {
                case.state = rest.parse().unwrap();
            } else if let Some(call) = line.strip_prefix("call netclient ") {
                case.call = call;
            } else if let Some(rest) = line.strip_prefix("hook ") {
                case.hooks.push(line);
                assert_eq!(rest, "SetClientTravel ENGINE");
            } else if let Some(rest) = line.strip_prefix("res netclient state=") {
                case.result_state = rest.parse().unwrap();
            }
        }
    }
    if let Some(case) = active {
        cases.push(case);
    }
    assert_eq!(cases.len(), 12, "netclient family case count");
    let svc = netclient_services();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(case.index, index);
        let mut level = unsafe { std::mem::zeroed::<ULevel>() };
        let mut driver = unsafe { std::mem::zeroed::<UNetDriver>() };
        let mut connection = unsafe { std::mem::zeroed::<UNetConnection>() };
        connection.state = case.state;
        let level_ptr = std::ptr::from_mut(&mut level).cast::<c_void>();
        let driver_ptr = std::ptr::from_mut(&mut driver).cast::<c_void>();
        let connection_ptr = std::ptr::from_mut(&mut connection).cast::<c_void>();
        driver.server_connection = connection_ptr;
        IDS.with(|ids| ids.borrow_mut().clear());
        let level_id = reg(level_ptr);
        let _driver_id = reg(driver_ptr);
        let connection_id = reg(connection_ptr);
        unsafe {
            level.base.net_driver = driver_ptr;
            NET_HOOKS.with(|value| value.borrow_mut().clear());
            NET_SERVICES.with(|value| value.borrow_mut().clear());
            tick_net_client(&svc, level_ptr).unwrap();
        }
        let hooks = NET_HOOKS.with(|value| std::mem::take(&mut *value.borrow_mut()));
        assert_eq!(hooks, case.hooks, "case {} service records", case.index);

        let mut expected_services = vec![format!("clock {level_id} 1")];
        match case.state {
            1 => {
                expected_services.push(format!("viewports {level_id}"));
                expected_services.push(format!("client_travel {level_id} 0"));
            }
            2 => {}
            3 => expected_services.push(format!("replicate_channels {connection_id}")),
            state => panic!("unexpected net client state {state}"),
        }
        expected_services.push(format!("unclock {level_id} 1"));
        let services = NET_SERVICES.with(|value| std::mem::take(&mut *value.borrow_mut()));
        assert_eq!(
            services, expected_services,
            "case {} service order",
            case.index
        );
        assert_eq!(
            connection.state, case.result_state,
            "case {} result state",
            case.index
        );
        assert!(case.call.starts_with("dt="));
    }
}

#[test]
fn lev_tic_actorsee_fixture_replays_records_and_results() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    struct Case<'a> {
        index: usize,
        inputs: Vec<&'a str>,
        model_fast: i32,
        call: &'a str,
        hooks: Vec<&'a str>,
        result: bool,
        output: Vec<&'a str>,
    }
    let mut cases = Vec::<Case<'_>>::new();
    let mut active: Option<Case<'_>> = None;
    for line in FIXTURE.lines() {
        if let Some(header) = line.strip_prefix("case ") {
            if let Some(case) = active.take() {
                cases.push(case);
            }
            let mut fields = header.split_whitespace();
            if fields.next() == Some("actorsee") {
                active = Some(Case {
                    index: fields.next().unwrap().parse().unwrap(),
                    inputs: Vec::new(),
                    model_fast: 0,
                    call: "",
                    hooks: Vec::new(),
                    result: false,
                    output: Vec::new(),
                });
            }
        } else if let Some(case) = active.as_mut() {
            if line.starts_with("in ") {
                case.inputs.push(line);
            } else if let Some(rest) = line.strip_prefix("knob MODEL ") {
                case.model_fast = rest.split_whitespace().nth(2).unwrap().parse().unwrap();
            } else if let Some(call) = line.strip_prefix("call actorsee ") {
                case.call = call;
            } else if line.starts_with("hook ") || line.starts_with("event ") {
                case.hooks.push(line);
            } else if let Some(result) = line.strip_prefix("res actorsee ret=") {
                case.result = result == "1";
            } else if let Some(output) = line.strip_prefix("out ") {
                case.output.push(output);
            }
        }
    }
    if let Some(case) = active {
        cases.push(case);
    }
    assert_eq!(cases.len(), 40, "actorsee family case count");
    let svc = actorsee_services();
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(case.index, index);
        let level = make_level();
        let info = make_info();
        let actor = make_actor(level, info);
        let viewer = make_actor(level, info);
        let weapon = make_actor(level, info);
        let player = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<APlayerPawn>() }));
        let real_viewer = std::ptr::from_mut(unsafe { &mut (*player).base.base }).cast::<c_void>();
        ACTORSEE_FAST.with(|value| value.set(case.model_fast));
        NET_HOOKS.with(|hooks| hooks.borrow_mut().clear());
        unsafe {
            (*real_viewer.cast::<AActor>()).set_b_is_pawn(true);
            (*actor).x_level = level.cast();
            (*viewer).x_level = level.cast();
            (*weapon).x_level = level.cast();
            for input in &case.inputs {
                let mut fields = input.split_whitespace();
                assert_eq!(fields.next(), Some("in"));
                let object = fields.next().unwrap();
                let field = fields.next().unwrap();
                let value = fields.next().unwrap();
                let float = || f32::from_bits(u32::from_str_radix(value, 16).unwrap());
                let pointer = |name: &str| match name {
                    "NULL" => std::ptr::null_mut(),
                    "LI" => info.cast(),
                    "A0" => actor.cast(),
                    "A1" => viewer.cast(),
                    "PP0" => real_viewer,
                    "A4" => weapon.cast(),
                    "A5" => viewer.cast(),
                    _ => panic!("unknown actorsee pointer {name}"),
                };
                match (object, field) {
                    ("A0", "SoundRadius") => (*actor).sound_radius = value.parse().unwrap(),
                    ("A0", "Location.X") => (*actor).location.x = float(),
                    ("A0", "Location.Y") => (*actor).location.y = float(),
                    ("A0", "Location.Z") => (*actor).location.z = float(),
                    ("A0", "bBlockPlayers") => (*actor).set_b_block_players(value == "1"),
                    ("A0", "AmbientSound") => (*actor).ambient_sound = pointer(value),
                    ("A0", "Owner") => (*actor).owner = pointer(value),
                    ("A0", "Instigator") => (*actor).instigator = pointer(value),
                    ("A0", "bAlwaysRelevant") => (*actor).set_b_always_relevant(value == "1"),
                    ("A0", "bHidden") => (*actor).set_b_hidden(value == "1"),
                    ("A0", "bOnlyOwnerSee") => (*actor).set_b_only_owner_see(value == "1"),
                    ("A4", "Owner") => (*weapon).owner = pointer(value),
                    ("PP0", "bIsPawn") => {
                        (*real_viewer.cast::<AActor>()).set_b_is_pawn(value == "1")
                    }
                    ("PP0", "Weapon") => (*player).base.weapon = pointer(value),
                    _ => panic!("unexpected actorsee input {object}.{field}={value}"),
                }
            }
            let coords = case
                .call
                .strip_prefix("src=")
                .unwrap()
                .split(',')
                .map(|value| f32::from_bits(u32::from_str_radix(value, 16).unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(coords.len(), 3);
            let got = actor_can_see(
                &svc,
                actor.cast(),
                real_viewer,
                viewer.cast(),
                FVector {
                    x: coords[0],
                    y: coords[1],
                    z: coords[2],
                },
            )
            .unwrap();
            assert_eq!(got, case.result, "case {}", case.index);
            assert!(
                case.output.is_empty(),
                "case {} output: {:?}",
                case.index,
                case.output
            );
        }
        let hooks = NET_HOOKS.with(|value| std::mem::take(&mut *value.borrow_mut()));
        assert_eq!(hooks, case.hooks, "case {} services", case.index);
    }
}
#[test]
fn lev_tic_priority_sort_replays_recorded_pointer_arrays() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    struct Group {
        phase: String,
        count: usize,
        entries: Vec<(String, i32)>,
    }

    let mut groups = Vec::<Group>::new();
    for line in FIXTURE.lines().filter(|line| line.starts_with("priority ")) {
        let mut fields = line.split_whitespace();
        assert_eq!(fields.next(), Some("priority"));
        let phase = fields.next().expect("priority phase");
        let index: i32 = fields.next().expect("priority index").parse().unwrap();
        let actor = fields.next().expect("priority actor").to_owned();
        let priority: i32 = fields.next().expect("priority value").parse().unwrap();
        if index == -1 {
            assert!(matches!(phase, "before" | "after"));
            groups.push(Group {
                phase: phase.to_owned(),
                count: priority as usize,
                entries: Vec::new(),
            });
        } else {
            let group = groups.last_mut().expect("priority trace header");
            assert_eq!(group.phase, phase);
            assert_eq!(index as usize, group.entries.len());
            group.entries.push((actor, priority));
        }
    }

    let before: Vec<_> = groups.iter().filter(|g| g.phase == "before").collect();
    let after: Vec<_> = groups.iter().filter(|g| g.phase == "after").collect();
    assert!(!before.is_empty(), "fixture contains sort input");
    assert_eq!(before.len(), after.len(), "sort trace group count");
    for (input, expected) in before.into_iter().zip(after) {
        assert_eq!(input.count, input.entries.len(), "input record count");
        assert_eq!(
            expected.count,
            expected.entries.len(),
            "output record count"
        );
        assert_eq!(input.count, expected.count, "sort pointer count");

        let mut values: Vec<ActorPriority> = input
            .entries
            .iter()
            .enumerate()
            .map(|(i, (_, priority))| ActorPriority {
                priority: *priority,
                actor: (i + 1) as *mut c_void,
                channel: std::ptr::null_mut(),
            })
            .collect();
        let base = values.as_mut_ptr();
        let mut pointers: Vec<*mut ActorPriority> =
            (0..values.len()).map(|i| unsafe { base.add(i) }).collect();
        sort_priority_ptrs(&mut pointers);
        let got: Vec<_> = pointers
            .iter()
            .map(|pointer| {
                let id = unsafe { (**pointer).actor as usize - 1 };
                input.entries[id].clone()
            })
            .collect();
        assert_eq!(got, expected.entries, "priority sort output");
    }
}

#[test]
fn lev_tic_replication_replays_sorted_priority_order() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    #[derive(Default)]
    struct Trace {
        sorted: Vec<String>,
        replications: Vec<String>,
    }

    let mut cases = Vec::<Trace>::new();
    let mut current: Option<Trace> = None;
    let mut after_sort = false;
    for line in FIXTURE.lines() {
        if line.starts_with("case ") {
            if let Some(trace) = current.take() {
                cases.push(trace);
            }
            current = Some(Trace::default());
            after_sort = false;
        } else if let Some(rest) = line.strip_prefix("priority after ") {
            let mut fields = rest.split_whitespace();
            let index: i32 = fields.next().expect("priority index").parse().unwrap();
            if index == -1 {
                after_sort = true;
            } else {
                assert!(after_sort);
                current
                    .as_mut()
                    .expect("case header")
                    .sorted
                    .push(fields.next().expect("priority actor").to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("hook ReplicateActor ") {
            let mut fields = rest.split_whitespace();
            let _channel = fields.next().expect("channel name");
            current
                .as_mut()
                .expect("case header")
                .replications
                .push(fields.next().expect("actor name").to_owned());
        }
    }
    if let Some(trace) = current {
        cases.push(trace);
    }

    let sorted_cases: Vec<_> = cases
        .iter()
        .filter(|case| !case.sorted.is_empty())
        .collect();
    assert!(
        !sorted_cases.is_empty(),
        "fixture contains sorted server cases"
    );
    for case in sorted_cases {
        let mut next = 0;
        for actor in &case.replications {
            while next < case.sorted.len() && case.sorted[next] != *actor {
                next += 1;
            }
            assert!(
                next < case.sorted.len(),
                "replication order must be a subsequence of sorted priorities"
            );
            next += 1;
        }
    }
}

#[test]
fn editor_tween_uses_fused_rounding() {
    let svc = services();
    EDITOR.with(|e| *e.borrow_mut() = 1);
    let lv = make_level();
    let li = make_info();
    let a = make_actor(lv, li);
    unsafe {
        (*lv).ticked = 1;
        (*a).set_b_ticked(false);
        (*a).physics = 0;
        (*a).role = ROLE_AUTHORITY;
        (*a).remote_role = ROLE_SIMULATED_PROXY;
        (*a).tween_rate = f32::from_bits(0x3f80_0001);
        (*a).tween_alpha = -1.0;
        actor_tick(&svc, a.cast(), f32::from_bits(0x3f7f_fffe), LEVELTICK_ALL);
        assert_eq!((*a).tween_alpha.to_bits(), 0xa880_0000);
    }
    EDITOR.with(|e| *e.borrow_mut() = 0);
}

#[test]
fn animation_frame_uses_fused_rounding() {
    let svc = services();
    let lv = make_level();
    let li = make_info();
    let a = make_actor(lv, li);
    unsafe {
        (*lv).ticked = 1;
        (*a).set_b_ticked(false);
        (*a).physics = 0;
        (*a).role = ROLE_AUTHORITY;
        (*a).remote_role = ROLE_SIMULATED_PROXY;
        (*a).anim_sequence = [1, 0, 0, 0];
        (*a).anim_rate = f32::from_bits(0x3f80_0001);
        (*a).anim_frame = -1.0;
        (*a).anim_last = 1.0;
        (*a).set_b_anim_loop(false);
        actor_tick(&svc, a.cast(), f32::from_bits(0x3f7f_fffe), LEVELTICK_ALL);
        assert_eq!((*a).anim_frame.to_bits(), 0xa880_0000);
    }
}

#[test]
fn timer_remainder_uses_fused_rounding() {
    let svc = services();
    let lv = make_level();
    let li = make_info();
    let a = make_actor(lv, li);
    unsafe {
        (*lv).ticked = 1;
        (*a).set_b_ticked(false);
        (*a).physics = 0;
        (*a).role = ROLE_AUTHORITY;
        (*a).remote_role = ROLE_SIMULATED_PROXY;
        (*a).timer_rate = 0.1;
        (*a).timer_counter = 0.3;
        (*a).set_b_timer_loop(true);
        actor_tick(&svc, a.cast(), 0.0, LEVELTICK_ALL);
        assert_eq!((*a).timer_counter.to_bits(), 0x3200_0000);
    }
}

#[test]
fn priority_uses_weapon_dot() {
    let svc = priority_services();
    let actor = make_actor(make_level(), make_info());
    let viewer = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<APlayerPawn>() }));
    let mut driver = unsafe { std::mem::zeroed::<UNetDriver>() };
    let mut conn = unsafe { std::mem::zeroed::<UNetConnection>() };
    driver.spawn_priority_seconds = 1.0;
    conn.driver = std::ptr::from_mut(&mut driver).cast();
    conn.actor = viewer.cast();
    unsafe {
        (*actor).remote_role = 0;
        (*actor).physics = 1;
        (*viewer).base.weapon = actor.cast();
        let result = make_priority(
            &svc,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            FVector {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            std::ptr::from_mut(&mut conn).cast(),
            actor.cast(),
        )
        .unwrap();
        assert_eq!(result.priority, 262_144);
        assert_eq!(result.actor, actor.cast());
    }
}

// Checked adapters for the existing deterministic fixture callbacks.
unsafe extern "C" fn checked_level_cycles_clock_net_clock(arg0: *mut c_void, arg1: i32) -> i32 {
    unsafe {
        net_clock(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_cycles_unclock_net_unclock(arg0: *mut c_void, arg1: i32) -> i32 {
    unsafe {
        net_unclock(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_conn_replicate_player_channels_net_replicate_player_channels(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        net_replicate_player_channels(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_net_viewports_num(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(net_viewports_num(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_engine_client_travel_net_client_travel(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        net_client_travel(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_check_enabled_net_check_enabled(out: *mut i32) -> i32 {
    unsafe {
        out.write(net_check_enabled());
    }
    0
}
unsafe extern "C" fn checked_level_fast_line_check_actorsee_fast_line(
    arg0: *mut c_void,
    arg1: *const f32,
    arg2: *const f32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(actorsee_fast_line(arg0, arg1, arg2));
    }
    0
}
unsafe extern "C" fn checked_actor_is_probing_mock_is_probing(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(mock_is_probing(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_event_mock_actor_event(
    arg0: *mut c_void,
    arg1: i32,
    arg2: f32,
) -> i32 {
    unsafe {
        mock_actor_event(arg0, arg1, arg2);
    }
    0
}
unsafe extern "C" fn checked_actor_process_state_mock_process_state(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        mock_process_state(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_is_editor_mock_level_is_editor(out: *mut i32) -> i32 {
    unsafe {
        out.write(mock_level_is_editor());
    }
    0
}
unsafe extern "C" fn checked_level_zone_time_since_render_mock_zone_time_since_render(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(mock_zone_time_since_render(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_creature_trace_tick_mock_creature_trace_tick(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        mock_creature_trace_tick(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_conn_actor_channel_mock_priority_channel(
    arg0: *mut c_void,
    arg1: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(mock_priority_channel(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_net_priority_mock_net_priority(
    arg0: *mut c_void,
    arg1: *mut c_void,
    arg2: f32,
    arg3: f32,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(mock_net_priority(arg0, arg1, arg2, arg3));
    }
    0
}
unsafe extern "C" fn fixture_guard_scope(
    _scope: i32,
    context: *mut c_void,
    continuation: unsafe extern "C" fn(*mut c_void) -> i32,
) -> i32 {
    unsafe { continuation(context) }
}
unsafe fn actor_tick(
    svc: &Hp2LevTicServices,
    actor: *mut c_void,
    delta: f32,
    tick_type: i32,
) -> i32 {
    let mut dispatch = hp2_engine::lev_tic::Hp2TickDispatch {
        token: u64::MAX,
        skip_reason: c"skip_no_event".as_ptr(),
        event_tick_dispatched: 0,
    };
    let result = unsafe { checked_actor_tick(svc, actor, delta, tick_type, &mut dispatch) };
    result.unwrap()
}
