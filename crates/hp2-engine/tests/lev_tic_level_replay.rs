//! Exact fixture replays for `ULevel::Tick`, demo record/playback, and network ticks
//! in `UnLevTic.cpp`: 23 `level`, 12 `demorec`, 12 `demoplay`, 40 `servertick`,
//! and 12 `netserver` records.
//!
//! Each record drives the real Rust control flow. Leaf services are mocks that return
//! fixture base/knob values and print the harness hook, trace, and log lines. The emitted
//! stream is compared with each record's expected lines in order, including result and
//! output diffs. Service slots no family reaches stay `never` and abort if called.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, c_char, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APlayerPawn, UActorChannel, UChannel, ULevel, UNetConnection, UNetDriver,
};
use hp2_engine::lev_tic::{ActorPriority, Hp2LevTicServices, level_tick};
use hp2_engine::lev_tic_actor::actor_tick as checked_actor_tick;
use hp2_engine::lev_tic_demo::{tick_demo_playback, tick_demo_record};
use hp2_engine::lev_tic_net::{server_tick_client, tick_net_server};

const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
const KNOB_COUNT: usize = 22;
const SLOT_COUNT: usize = size_of::<Hp2LevTicServices>() / size_of::<usize>();
const IS_NET_READY: usize = 0;
const FAST_LINE_CHECK: usize = 2;
const OBJECT_TO_INDEX: usize = 11;
const GET_NET_PRIORITY: usize = 12;
const UPDATE_FREQUENCY: usize = 13;
const MAX_TICK_RATE: usize = 14;
const ACTOR_NAMES: [&str; 10] = ["LI", "A0", "A1", "A2", "A3", "A4", "A5", "PP0", "PP1", "Z0"];
/// Harness `GSecondsPerCycle` (`lev_tic_harness.cpp`).
const GSECONDS_PER_CYCLE: f32 = 1e-9;

type Row = (String, String, String);
type World = BTreeMap<(String, String), String>;

struct Record {
    index: usize,
    timing: bool,
    tick_type: i32,
    dt_bits: u32,
    connection: Option<String>,
    ins: Vec<Row>,
    knobs: BTreeMap<String, [i32; KNOB_COUNT]>,
    /// Fixture lines after `call`: hook/trace/priority/log, result, then output diffs.
    expected: Vec<String>,
}

struct Fixture {
    base: Vec<Row>,
    levels: Vec<Record>,
    timing_headers: usize,
}

thread_local! {
    static OUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static OBJECTS: RefCell<Vec<(String, usize)>> = const { RefCell::new(Vec::new()) };
    static WORLD: RefCell<World> = const { RefCell::new(BTreeMap::new()) };
    static KNOBS: RefCell<BTreeMap<String, [i32; KNOB_COUNT]>> = const { RefCell::new(BTreeMap::new()) };
    static CHANNEL_ACTOR: RefCell<BTreeMap<usize, usize>> = const { RefCell::new(BTreeMap::new()) };
    static DEMO_CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static NET_GMEM: Cell<usize> = const { Cell::new(0) };
    static NET_FRAND: Cell<u32> = const { Cell::new(7) };
    static NET_PRIORITY_LIST: Cell<usize> = const { Cell::new(0) };
    static NET_PRIORITY_CAPACITY: Cell<usize> = const { Cell::new(0) };
    static NET_PRIORITY_ARRAY: Cell<usize> = const { Cell::new(0) };
    static NET_MEM_PUSHES: Cell<usize> = const { Cell::new(0) };
    static NET_PRIORITY_AFTER: Cell<usize> = const { Cell::new(0) };
    static TIMING: Cell<bool> = const { Cell::new(false) };
    static CUR_LEVEL: Cell<usize> = const { Cell::new(0) };
    static SVC: Cell<usize> = const { Cell::new(0) };
    static ROOT: Cell<*const c_char> = const { Cell::new(ptr::null()) };
    static TOKEN: Cell<u64> = const { Cell::new(0) };
}

#[repr(C)]
struct MockMemStack {
    top: *mut u8,
    end: *mut u8,
}

fn split3(row: &str) -> Row {
    let mut parts = row.splitn(3, ' ');
    let obj = parts.next().expect("object").to_string();
    let field = parts.next().expect("field").to_string();
    let value = parts.next().expect("value").to_string();
    (obj, field, value)
}

fn parse_fixture() -> Fixture {
    let mut base = Vec::new();
    let mut levels = Vec::new();
    let mut timing_headers = 0;
    let mut seen_case = false;
    let mut timing_next = false;
    let mut after_call = false;
    let mut current: Option<Record> = None;

    for line in FIXTURE.lines() {
        if let Some(rest) = line.strip_prefix("case ") {
            seen_case = true;
            if let Some(done) = current.take() {
                levels.push(done);
            }
            after_call = false;
            let mut parts = rest.split_whitespace();
            let family = parts.next().expect("case family");
            let index: usize = parts
                .next()
                .expect("case index")
                .parse()
                .expect("case index is numeric");
            match family {
                "timing" => {
                    timing_headers += 1;
                    timing_next = true;
                }
                "level" => {
                    current = Some(Record {
                        index,
                        timing: timing_next,
                        tick_type: 0,
                        dt_bits: 0,
                        connection: None,
                        ins: Vec::new(),
                        knobs: BTreeMap::new(),
                        expected: Vec::new(),
                    });
                    timing_next = false;
                }
                _ => timing_next = false,
            }
            continue;
        }
        if !seen_case {
            if let Some(row) = line.strip_prefix("base ") {
                base.push(split3(row));
            }
            continue;
        }
        let Some(rec) = current.as_mut() else {
            continue;
        };
        if line.trim().is_empty() || line.starts_with("editor ") {
            // The editor id is not read on the level path.
        } else if let Some(row) = line.strip_prefix("knob ") {
            let mut parts = row.split_whitespace();
            let obj = parts.next().expect("knob object").to_string();
            let values: Vec<i32> = parts
                .map(|v| v.parse().expect("knob value is an integer"))
                .collect();
            let arr: [i32; KNOB_COUNT] = values
                .try_into()
                .unwrap_or_else(|v: Vec<i32>| panic!("knob {obj} has {} columns", v.len()));
            rec.knobs.insert(obj, arr);
        } else if let Some(row) = line.strip_prefix("in ") {
            rec.ins.push(split3(row));
        } else if let Some(call) = line.strip_prefix("call ") {
            assert!(!after_call, "record {} has two call lines", rec.index);
            let mut parts = call.split_whitespace();
            assert_eq!(parts.next(), Some("level"), "call family");
            let tick_type: i32 = parts
                .next()
                .and_then(|p| p.strip_prefix("type="))
                .expect("call type=")
                .parse()
                .expect("call type is an integer");
            let dt = parts
                .next()
                .and_then(|p| p.strip_prefix("dt="))
                .expect("call dt=");
            rec.tick_type = tick_type;
            rec.dt_bits = u32::from_str_radix(dt, 16).expect("call dt is hex f32 bits");
            after_call = true;
        } else if after_call
            && (line.starts_with("hook ")
                || line.starts_with("trace ")
                || line == "log"
                || line == "res level"
                || line.starts_with("out LEV Ticked "))
        {
            rec.expected.push(line.to_string());
        } else {
            panic!("unexpected line in level record {}: {line}", rec.index);
        }
    }
    if let Some(done) = current.take() {
        levels.push(done);
    }
    Fixture {
        base,
        levels,
        timing_headers,
    }
}

/// Parses a fixture family with the shared base, inputs, knobs, and expected record stream.
fn parse_demo_family(family: &str) -> (Vec<Row>, Vec<Record>) {
    let mut base = Vec::new();
    let mut records = Vec::new();
    let mut seen_case = false;
    let mut after_call = false;
    let mut current: Option<Record> = None;

    for line in FIXTURE.lines() {
        if let Some(header) = line.strip_prefix("case ") {
            seen_case = true;
            if let Some(done) = current.take() {
                records.push(done);
            }
            after_call = false;
            let mut fields = header.split_whitespace();
            let case_family = fields.next().expect("case family");
            if case_family == family {
                current = Some(Record {
                    index: fields.next().expect("case index").parse().unwrap(),
                    timing: false,
                    tick_type: 0,
                    dt_bits: 0,
                    connection: None,
                    ins: Vec::new(),
                    knobs: BTreeMap::new(),
                    expected: Vec::new(),
                });
            }
            continue;
        }
        if !seen_case {
            if let Some(row) = line.strip_prefix("base ") {
                base.push(split3(row));
            }
            continue;
        }
        let Some(rec) = current.as_mut() else {
            continue;
        };
        if line.trim().is_empty() || line.starts_with("editor ") {
            continue;
        }
        if let Some(row) = line.strip_prefix("knob ") {
            let mut fields = row.split_whitespace();
            let object = fields.next().expect("knob object").to_string();
            let values: Vec<i32> = fields.map(|v| v.parse().unwrap()).collect();
            let knobs: [i32; KNOB_COUNT] = values
                .try_into()
                .unwrap_or_else(|v: Vec<i32>| panic!("knob {object} has {} columns", v.len()));
            rec.knobs.insert(object, knobs);
        } else if let Some(row) = line.strip_prefix("in ") {
            rec.ins.push(split3(row));
        } else if let Some(call) = line.strip_prefix("call ") {
            assert!(!after_call, "record {} has two calls", rec.index);
            let mut fields = call.split_whitespace();
            assert_eq!(fields.next(), Some(family), "call family");
            let dt = fields
                .next()
                .and_then(|field| field.strip_prefix("dt="))
                .expect("call dt");
            rec.dt_bits = u32::from_str_radix(dt, 16).expect("call dt is hex f32 bits");
            rec.connection = fields.next().map(|connection| {
                connection
                    .strip_prefix("conn=")
                    .expect("optional call argument is conn=")
                    .to_string()
            });
            assert!(fields.next().is_none(), "unexpected call arguments");
            after_call = true;
        } else if after_call
            && (line.starts_with("hook ")
                || line.starts_with("trace ")
                || line.starts_with("priority ")
                || line.starts_with("event ")
                || line == "log"
                || line == format!("res {family}")
                || line.starts_with(&format!("res {family} "))
                || line.starts_with("out "))
        {
            rec.expected.push(line.to_string());
        } else {
            panic!("unexpected line in {family} record {}: {line}", rec.index);
        }
    }
    if let Some(done) = current {
        records.push(done);
    }
    (base, records)
}

fn build_world(base: &[Row], ins: &[Row]) -> World {
    let mut world = World::new();
    for (obj, field, value) in base.iter().chain(ins.iter()) {
        world.insert((obj.clone(), field.clone()), value.clone());
    }
    world
}

/// Object names referenced by base/`in` rows and knob rows. Uppercase-initial value tokens
/// (`NETDRV`, `LEV`, `HASH`, ...) become objects; extra opaque names are harmless.
fn object_names(
    base: &[Row],
    ins: &[Row],
    knobs: &BTreeMap<String, [i32; KNOB_COUNT]>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (obj, _, value) in base.iter().chain(ins.iter()) {
        names.insert(obj.clone());
        for token in value.split(|c: char| !c.is_ascii_alphanumeric()) {
            if token.starts_with(|c: char| c.is_ascii_uppercase()) && token != "NULL" {
                names.insert(token.to_string());
            }
        }
    }
    names.extend(knobs.keys().cloned());
    names
}

fn alloc_objects(names: &BTreeSet<String>) {
    OBJECTS.with(|objects| {
        let mut table = objects.borrow_mut();
        table.clear();
        for name in names {
            let ptr: *mut c_void = match name.as_str() {
                "LI" => Box::into_raw(Box::new(unsafe { zeroed::<ALevelInfo>() })).cast(),
                "PP0" | "PP1" => Box::into_raw(Box::new(unsafe { zeroed::<APlayerPawn>() })).cast(),
                "LEV" => Box::into_raw(Box::new(unsafe { zeroed::<ULevel>() })).cast(),
                "NETDRV" | "DEMOREC" => {
                    Box::into_raw(Box::new(unsafe { zeroed::<UNetDriver>() })).cast()
                }
                name if name.starts_with("CONN") => {
                    Box::into_raw(Box::new(unsafe { zeroed::<UNetConnection>() })).cast()
                }
                name if name.starts_with("CH") => {
                    Box::into_raw(Box::new(unsafe { zeroed::<UActorChannel>() })).cast()
                }
                actor if ACTOR_NAMES.contains(&actor) => {
                    Box::into_raw(Box::new(unsafe { zeroed::<AActor>() })).cast()
                }
                _ => Box::into_raw(Box::new([0u64; 8])).cast(),
            };
            table.push((name.clone(), ptr as usize));
        }
    });
}

fn ptr_of(name: &str) -> *mut c_void {
    OBJECTS
        .with(|objects| {
            objects
                .borrow()
                .iter()
                .find(|e| e.0 == name)
                .map(|e| e.1 as *mut c_void)
        })
        .unwrap_or_else(|| panic!("unknown fixture object {name}"))
}

fn name_of(p: *const c_void) -> String {
    if p.is_null() {
        return "NULL".to_string();
    }
    OBJECTS
        .with(|objects| {
            objects
                .borrow()
                .iter()
                .find(|e| e.1 == p as usize)
                .map(|e| e.0.clone())
        })
        .unwrap_or_else(|| panic!("unregistered fixture object {p:?}"))
}

fn field(obj: &str, name: &str) -> Option<String> {
    WORLD.with(|w| {
        w.borrow()
            .get(&(obj.to_string(), name.to_string()))
            .cloned()
    })
}

/// Missing integer fields read as zero, matching zero-initialized harness objects.
fn int_field(obj: &str, name: &str) -> i64 {
    match field(obj, name) {
        Some(v) => v
            .parse()
            .unwrap_or_else(|_| panic!("{obj} {name} = {v} is not an integer")),
        None => 0,
    }
}

/// Fixture floats are raw hexadecimal bits; plain numeric fields also occur.
fn float_field(obj: &str, name: &str) -> f32 {
    field(obj, name).map_or(0.0, |value| {
        value.parse().unwrap_or_else(|_| {
            f32::from_bits(
                u32::from_str_radix(&value, 16)
                    .unwrap_or_else(|_| panic!("{obj} {name} = {value} is not a float")),
            )
        })
    })
}

/// Missing object fields read as NULL, matching zero-initialized harness objects.
fn ptr_field(obj: &str, name: &str) -> *mut c_void {
    match field(obj, name).as_deref() {
        None | Some("NULL") => ptr::null_mut(),
        Some(v) => ptr_of(v),
    }
}

fn list_field(obj: &str, name: &str) -> Vec<String> {
    let raw = field(obj, name).unwrap_or_else(|| panic!("{obj} {name} missing"));
    let inner = raw
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or_else(|| panic!("{obj} {name} is not a list: {raw}"));
    if inner.is_empty() {
        Vec::new()
    } else {
        inner.split(',').map(str::to_string).collect()
    }
}

fn knob(name: &str, index: usize) -> i32 {
    KNOBS
        .with(|k| k.borrow().get(name).map(|row| row[index]))
        .unwrap_or_else(|| panic!("no knob row for {name}"))
}

fn emit(line: String) {
    flush_net_priority_trace();
    OUT.with(|o| o.borrow_mut().push(line));
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        "NULL".to_string()
    } else {
        unsafe { CStr::from_ptr(p) }
            .to_str()
            .expect("trace label is UTF-8")
            .to_string()
    }
}

fn current_level_name() -> String {
    name_of(CUR_LEVEL.with(|c| c.get()) as *const c_void)
}

/// Copies fixture base/`in` values into the `ULevel`, `ALevelInfo`, and `AActor` fields that
/// the level path reads.
fn sync_structs() {
    let lv = ptr_of("LEV").cast::<ULevel>();
    let li = ptr_of("LI").cast::<ALevelInfo>();
    unsafe {
        (*lv).ticked = int_field("LEV", "Ticked") as i32;
        (*lv).time_seconds = int_field("LEV", "TimeSeconds").to_ne_bytes();
        (*lv).i_first_dynamic_actor = int_field("LEV", "iFirstDynamicActor") as i32;
        (*lv).base.net_driver = ptr_field("LEV", "NetDriver");
        (*lv).base.demo_rec_driver = ptr_field("LEV", "DemoRecDriver");
        (*lv).hash = ptr_field("LEV", "Hash");
        (*lv).net_tag = int_field("LEV", "NetTag") as i32;
        (*lv).i_first_net_relevant_actor = int_field("LEV", "iFirstNetRelevantActor") as i32;
        (*li).net_mode = int_field("LI", "NetMode") as u8;
        (*li).level_action = int_field("LI", "LevelAction") as u8;
        (*li).set_b_players_only(int_field("LI", "bPlayersOnly") != 0);
        for name in ["NETDRV", "DEMOREC"] {
            let driver = ptr_of(name).cast::<UNetDriver>();
            (*driver).server_connection = ptr_field(name, "ServerConnection");
            (*driver).time = int_field(name, "Time").to_ne_bytes();
            (*driver).relevant_timeout = float_field(name, "RelevantTimeout");
        }
        for name in ["CONN0", "CONN1"] {
            let conn = ptr_of(name).cast::<UNetConnection>();
            (*conn).state = int_field(name, "State") as i32;
            (*conn).actor = ptr_field(name, "Actor");
            (*conn).driver = ptr_field(name, "Driver");
            (*conn).last_receive_time = int_field(name, "LastReceiveTime").to_ne_bytes();
            (*conn).last_rep_time = int_field(name, "LastRepTime").to_ne_bytes();
            (*conn).tick_count = int_field(name, "TickCount") as i32;
            (*conn).best_lag = float_field(name, "BestLag");
        }
        for name in ["CH0", "CH1", "CH2", "CH3"] {
            let channel = ptr_of(name).cast::<UActorChannel>();
            (*channel).relevant_time = int_field(name, "RelevantTime").to_ne_bytes();
            (*channel).last_update_time = int_field(name, "LastUpdateTime").to_ne_bytes();
            (*channel.cast::<UChannel>()).closing = int_field(name, "Closing") as i32;
        }
        for name in ACTOR_NAMES {
            let a = ptr_of(name).cast::<AActor>();
            (*a).set_b_ticked(int_field(name, "bTicked") != 0);
            (*a).set_b_static(int_field(name, "bStatic") != 0);
            (*a).set_b_net_temporary(int_field(name, "bNetTemporary") != 0);
            (*a).set_b_always_tick(int_field(name, "bAlwaysTick") != 0);
            (*a).role = int_field(name, "Role") as u8;
            (*a).net_tag = int_field(name, "NetTag") as i32;
            (*a).set_b_always_relevant(int_field(name, "bAlwaysRelevant") != 0);
            (*a).set_b_net_optional(int_field(name, "bNetOptional") != 0);
            (*a).physics = int_field(name, "Physics") as u8;
            (*a).life_span = float_field(name, "LifeSpan");
            (*a).net_update_frequency = float_field(name, "NetUpdateFrequency");
            (*a).location.x = float_field(name, "Location.X");
            (*a).location.y = float_field(name, "Location.Y");
            (*a).location.z = float_field(name, "Location.Z");
            (*a).velocity.x = float_field(name, "Velocity.X");
            (*a).velocity.y = float_field(name, "Velocity.Y");
            (*a).velocity.z = float_field(name, "Velocity.Z");
            (*a).base = ptr_field(name, "Base");
            (*a).instigator = ptr_field(name, "Instigator");
            (*a).ambient_sound = ptr_field(name, "AmbientSound");
            if name == "PP0" || name == "PP1" {
                let pawn = ptr_of(name).cast::<APlayerPawn>();
                (*pawn).base.weapon = ptr_field(name, "Weapon");
                (*pawn).base.view_rotation.pitch = int_field(name, "ViewRotation.Pitch") as i32;
                (*pawn).base.view_rotation.yaw = int_field(name, "ViewRotation.Yaw") as i32;
                (*pawn).base.view_rotation.roll = int_field(name, "ViewRotation.Roll") as i32;
            }
            (*a).remote_role = int_field(name, "RemoteRole") as u8;
            (*a).tick_parent = ptr_field(name, "TickParent");
            (*a).tick_parent2 = ptr_field(name, "TickParent2");
            (*a).owner = ptr_field(name, "Owner");
            (*a).x_level = ptr_field(name, "XLevel");
            (*a).level = ptr_field(name, "Level");
        }
    }
}

unsafe extern "C" fn never() {
    panic!("unexpected service call in level-family replay");
}

unsafe extern "C" fn m_trace_enabled() -> i32 {
    1
}

unsafe extern "C" fn m_trace_begin_dispatch(
    actor: *mut c_void,
    relation: *const c_char,
    root: *const c_char,
) -> u64 {
    emit(format!(
        "trace begin {} {} {}",
        name_of(actor),
        cstr(relation),
        cstr(root)
    ));
    TOKEN.with(|t| {
        let next = t.get() + 1;
        t.set(next);
        next
    })
}

unsafe extern "C" fn m_trace_end_dispatch(_token: u64, skip_reason: *const c_char) {
    emit(format!("trace end {}", cstr(skip_reason)));
}

unsafe extern "C" fn m_trace_relation_get() -> *const c_char {
    c"direct".as_ptr()
}

unsafe extern "C" fn m_trace_root_get() -> *const c_char {
    ROOT.with(|r| r.get())
}

unsafe extern "C" fn m_trace_root_set(admission: *const c_char) -> *const c_char {
    ROOT.with(|r| r.replace(admission))
}

unsafe extern "C" fn m_tick_actor(actor: *mut c_void, delta: f32, tick_type: i32) -> i32 {
    emit(format!("hook Tick {}", name_of(actor)));
    let svc = SVC.with(|s| s.get()) as *const Hp2LevTicServices;
    unsafe { actor_tick(&*svc, actor, delta, tick_type) }
}

unsafe extern "C" fn m_frame_timing_enabled() -> i32 {
    TIMING.with(|t| t.get()) as i32
}

unsafe extern "C" fn m_frame_timing_now() -> i64 {
    0
}

unsafe extern "C" fn m_frame_timing_log(_start: i64) {
    emit("log".to_string());
}


unsafe extern "C" fn m_driver_tick_dispatch(driver: *mut c_void, _delta: f32) {
    emit(format!("hook TickDispatch {}", name_of(driver)));
}

unsafe extern "C" fn m_driver_tick_flush(driver: *mut c_void) {
    emit(format!("hook TickFlush {}", name_of(driver)));
}


unsafe extern "C" fn m_driver_client_connection_count(driver: *mut c_void) -> i32 {
    list_field(&name_of(driver), "ClientConnections").len() as i32
}

unsafe extern "C" fn m_driver_client_connection(driver: *mut c_void, index: i32) -> *mut c_void {
    let list = list_field(&name_of(driver), "ClientConnections");
    ptr_of(&list[index as usize])
}




unsafe extern "C" fn m_conn_is_net_ready(conn: *mut c_void, _saturate: i32) -> i32 {
    let name = name_of(conn);
    emit(format!("hook IsNetReady {name}"));
    knob(&name, IS_NET_READY)
}

unsafe extern "C" fn m_conn_sent_temp_count(_conn: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn m_actor_event_player_calc_view(
    viewer: *mut c_void,
    _location: *mut f32,
    _rotation: *mut i32,
) -> *mut c_void {
    emit(format!("event PlayerCalcView {}", name_of(viewer)));
    viewer
}

unsafe extern "C" fn m_actor_update_frequency(
    actor: *mut c_void,
    _viewer: *mut c_void,
    _view_dir: *mut f32,
    _view_pos: *mut f32,
) -> f32 {
    let name = name_of(actor);
    emit(format!("hook UpdateFrequency {name}"));
    knob(&name, UPDATE_FREQUENCY) as f32
}

unsafe extern "C" fn m_actor_net_priority(
    actor: *mut c_void,
    _sent: *mut c_void,
    _time: f32,
    _lag: f32,
) -> f32 {
    let name = name_of(actor);
    emit(format!("hook GetNetPriority {name}"));
    knob(&name, GET_NET_PRIORITY) as f32
}

unsafe extern "C" fn m_actor_no_variables_to_replicate(
    actor: *mut c_void,
    _old_version: *mut c_void,
) -> i32 {
    let name = name_of(actor);
    emit(format!("hook NoVariablesToReplicate {name}"));
    knob(&name, 8)
}

unsafe extern "C" fn m_actor_check_recent_changes(actor: *mut c_void) -> i32 {
    let name = name_of(actor);
    emit(format!("hook CheckRecentChanges {name}"));
    knob(&name, 7)
}

unsafe extern "C" fn m_actor_class_default_life_span(_actor: *mut c_void) -> f32 {
    0.0
}

unsafe extern "C" fn m_level_fast_line_check(
    level: *mut c_void,
    _end: *const f32,
    _start: *const f32,
) -> i32 {
    assert_eq!(level, ptr_of("LEV"));
    flush_net_priority_trace();
    emit("hook FastLineCheck MODEL".to_string());
    knob("MODEL", FAST_LINE_CHECK)
}

unsafe extern "C" fn m_level_model_line_check(
    level: *mut c_void,
    _out_location: *mut f32,
    _start: *const f32,
    _end: *const f32,
    _node_flags: i32,
) {
    assert_eq!(level, ptr_of("LEV"));
    emit("hook LineCheck MODEL".to_string());
}

unsafe extern "C" fn m_level_max_tick_rate(level: *mut c_void) -> f32 {
    assert_eq!(level, ptr_of("LEV"));
    emit("hook GetMaxTickRate ENGINE".to_string());
    knob("ENGINE", MAX_TICK_RATE) as f32
}

unsafe extern "C" fn m_level_app_frand() -> f32 {
    NET_FRAND.with(|state| {
        let next = state.get().wrapping_mul(1_103_515_245).wrapping_add(12_345);
        state.set(next);
        ((next >> 8) & 0xffff) as f32 / 65_536.0
    })
}

fn net_priority_count() -> usize {
    let base = NET_PRIORITY_LIST.with(Cell::get) as *const ActorPriority;
    let capacity = NET_PRIORITY_CAPACITY.with(Cell::get);
    if base.is_null() {
        return 0;
    }
    let mut count = 0;
    while count < capacity && !unsafe { (*base.add(count)).actor.is_null() } {
        count += 1;
    }
    count
}

fn net_priority_lines(phase: &str, count: usize) -> Vec<String> {
    let array = NET_PRIORITY_ARRAY.with(Cell::get) as *const *mut ActorPriority;
    assert!(!array.is_null(), "priority pointer array was allocated");
    let mut lines = vec![format!("priority {phase} -1 NULL {count}")];
    for index in 0..count {
        let priority = unsafe { &**array.add(index) };
        lines.push(format!(
            "priority {phase} {index} {} {}",
            name_of(priority.actor),
            priority.priority
        ));
    }
    lines
}

fn flush_net_priority_trace() {
    let count = NET_PRIORITY_AFTER.with(|pending| pending.replace(0));
    if count == 0 {
        return;
    }
    let lines = net_priority_lines("after", count);
    OUT.with(|output| output.borrow_mut().extend(lines));
}

unsafe extern "C" fn m_channel_recent_count(channel: *mut c_void) -> i32 {
    int_field(&name_of(channel), "RecentNum") as i32
}

unsafe extern "C" fn m_channel_recent_first(_channel: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}

unsafe extern "C" fn m_channel_dirty_count(_channel: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn m_channel_close(channel: *mut c_void) {
    emit(format!("hook Close {}", name_of(channel)));
}

unsafe extern "C" fn m_level_log_running_net(_level: *mut c_void, _client_count: i32) {
    emit("log".to_string());
}

unsafe extern "C" fn m_level_send_net_stats(_level: *mut c_void, _updated: i32) {}

unsafe extern "C" fn m_gmath_sin_tab(index: i32) -> f32 {
    assert_eq!(index, 0, "oracle server-tick view rotation is zero");
    0.0
}

unsafe extern "C" fn m_gmath_cos_tab(index: i32) -> f32 {
    assert_eq!(index, 0, "oracle server-tick view rotation is zero");
    1.0
}

unsafe extern "C" fn m_conn_replicate_player_channels(_conn: *mut c_void) {}

unsafe extern "C" fn m_conn_actor_channel(_conn: *mut c_void, _actor: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}

unsafe extern "C" fn m_conn_create_actor_channel(conn: *mut c_void) -> *mut c_void {
    emit(format!("hook CreateChannel {}", name_of(conn)));
    ptr_of("CH0")
}

unsafe extern "C" fn m_conn_set_channel_actor(channel: *mut c_void, actor: *mut c_void) {
    emit(format!("hook SetChannelActor {}", name_of(channel)));
    CHANNEL_ACTOR.with(|m| m.borrow_mut().insert(channel as usize, actor as usize));
}

unsafe extern "C" fn m_conn_package_map_class_index(
    _conn: *mut c_void,
    _actor: *mut c_void,
) -> i32 {
    flush_net_priority_trace();
    emit("hook ObjectToIndex PKGMAP".to_string());
    knob("PKGMAP", OBJECT_TO_INDEX)
}

unsafe extern "C" fn m_channel_is_net_ready(channel: *mut c_void, _saturate: i32) -> i32 {
    let name = name_of(channel);
    emit(format!("hook IsNetReady {name}"));
    knob(&name, IS_NET_READY)
}

unsafe extern "C" fn m_channel_replicate_actor(channel: *mut c_void) {
    let actor = CHANNEL_ACTOR
        .with(|m| m.borrow().get(&(channel as usize)).copied())
        .expect("replicated channel has an actor");
    emit(format!(
        "hook ReplicateActor {} {}",
        name_of(channel),
        name_of(actor as *const c_void)
    ));
}

unsafe extern "C" fn m_actor_class_default_is_static(_actor: *mut c_void) -> i32 {
    0
}

unsafe extern "C" fn m_actor_is_a(actor: *mut c_void, kind: i32) -> i32 {
    let class = field(&name_of(actor), "Class");
    i32::from(kind == 2 && class.as_deref() == Some("AZoneInfo"))
}

unsafe extern "C" fn m_level_engine_set_progress_empty(level: *mut c_void) {
    assert_eq!(level, ptr_of("LEV"));
    DEMO_CALLS.with(|calls| calls.borrow_mut().push("progress_empty".to_string()));
}

unsafe extern "C" fn m_level_engine_client_travel(level: *mut c_void, url: i32) {
    assert_eq!(level, ptr_of("LEV"));
    DEMO_CALLS.with(|calls| calls.borrow_mut().push(format!("client_travel {url}")));
}

unsafe extern "C" fn m_level_client_viewports_num(level: *mut c_void) -> i32 {
    assert_eq!(level, ptr_of("LEV"));
    1
}

unsafe extern "C" fn m_level_cleanup_destroyed(level: *mut c_void) {
    emit(format!("hook CleanupDestroyed {}", name_of(level)));
}

unsafe extern "C" fn m_level_init_stats(level: *mut c_void) {
    emit(format!("hook InitStats {}", name_of(level)));
}

unsafe extern "C" fn m_level_init_runaway() {}

unsafe extern "C" fn m_level_gseconds_per_cycle() -> f32 {
    GSECONDS_PER_CYCLE
}

unsafe extern "C" fn m_level_cycles_clock(_level: *mut c_void, _which: i32) {}

unsafe extern "C" fn m_level_update_time(info: *mut c_void) {
    assert_eq!(info, ptr_of("LI"), "ULevel::UpdateTime receives ALevelInfo");
    emit(format!("hook UpdateTime {}", current_level_name()));
}

unsafe extern "C" fn m_level_startup_phase_trace(_level: *mut c_void) {}

unsafe extern "C" fn m_level_hash_tick(level: *mut c_void) {
    let hash = unsafe { (*level.cast::<ULevel>()).hash };
    emit(format!("hook Tick {}", name_of(hash)));
}

unsafe extern "C" fn m_level_actor_count(level: *mut c_void) -> i32 {
    list_field(&name_of(level), "Actors").len() as i32
}

unsafe extern "C" fn m_level_actor(level: *mut c_void, index: i32) -> *mut c_void {
    let list = list_field(&name_of(level), "Actors");
    ptr_of(&list[index as usize])
}

/// `Actors[0]` is the level info (`LI`) in the fixture base.
unsafe extern "C" fn m_level_info(level: *mut c_void) -> *mut c_void {
    let list = list_field(&name_of(level), "Actors");
    ptr_of(&list[0])
}

unsafe extern "C" fn m_level_info_pauser_empty(_info: *mut c_void) -> i32 {
    1
}

unsafe extern "C" fn m_mem_mark_save(_mem: *mut c_void, _buffer: *mut c_void) {}

unsafe extern "C" fn m_mem_mark_pop(_buffer: *mut c_void) {}

static MEM_TOKEN: u64 = 0;

unsafe extern "C" fn m_level_gmem() -> *mut c_void {
    (&raw const MEM_TOKEN).cast_mut().cast()
}

unsafe extern "C" fn m_level_gengine_mem() -> *mut c_void {
    (&raw const MEM_TOKEN).cast_mut().cast()
}

unsafe extern "C" fn m_check_enabled() -> i32 {
    1
}

unsafe extern "C" fn m_net_gmem() -> *mut c_void {
    NET_GMEM.with(|stack| stack.get() as *mut c_void)
}

unsafe extern "C" fn m_net_mem_mark_save(mem: *mut c_void, buffer: *mut c_void) {
    let stack = mem.cast::<MockMemStack>();
    let saved = [mem, unsafe { (*stack).top.cast() }, ptr::null_mut()];
    unsafe { buffer.cast::<[*mut c_void; 3]>().write(saved) };
}

unsafe extern "C" fn m_net_mem_mark_pop(buffer: *mut c_void) {
    let saved = unsafe { buffer.cast::<[*mut c_void; 3]>().read() };
    let stack = saved[0].cast::<MockMemStack>();
    unsafe { (*stack).top = saved[1].cast() };
    NET_MEM_PUSHES.with(|calls| calls.set(0));
}

unsafe extern "C" fn m_net_mem_push(mem: *mut c_void, size: u64, count: i32) -> *mut c_void {
    let stack = mem.cast::<MockMemStack>();
    let call = NET_MEM_PUSHES.with(|calls| {
        let call = calls.get();
        calls.set(call + 1);
        call
    });
    assert!(
        call < 2,
        "server tick allocates exactly two priority arrays"
    );
    unsafe {
        let start = ((*stack).top as usize + 7) & !7;
        let Some(bytes) = (size as usize).checked_mul(count as usize) else {
            std::process::abort();
        };
        let Some(next) = start.checked_add(bytes) else {
            std::process::abort();
        };
        if next > (*stack).end as usize {
            std::process::abort();
        }
        (*stack).top = next as *mut u8;
        if call == 0 {
            assert_eq!(size as usize, size_of::<ActorPriority>());
            std::ptr::write_bytes(start as *mut u8, 0, bytes);
            NET_PRIORITY_LIST.with(|list| list.set(start));
            NET_PRIORITY_CAPACITY.with(|capacity| capacity.set(count as usize));
        } else {
            assert_eq!(size as usize, size_of::<*mut ActorPriority>());
            let list = NET_PRIORITY_LIST.with(Cell::get) as *mut ActorPriority;
            let actors = start as *mut *mut ActorPriority;
            for index in 0..count as usize {
                actors.add(index).write(list);
            }
            NET_PRIORITY_ARRAY.with(|array| array.set(start));
        }
        start as *mut c_void
    }
}

/// Every slot starts as `never`; the level-family mocks above replace the reached ones.
fn services() -> Hp2LevTicServices {
    let bytes = size_of::<Hp2LevTicServices>();
    assert_eq!(
        bytes % size_of::<usize>(),
        0,
        "services table must hold only pointer slots"
    );
    let slots: Vec<unsafe extern "C" fn()> = (0..SLOT_COUNT)
        .map(|_| never as unsafe extern "C" fn())
        .collect();
    let mut svc = unsafe { ptr::read_unaligned(slots.as_ptr().cast::<Hp2LevTicServices>()) };
    svc.actor_is_a = checked_actor_is_a_m_actor_is_a;
    svc.level_frame_timing_enabled = checked_level_frame_timing_enabled_m_frame_timing_enabled;
    svc.level_frame_timing_now = checked_level_frame_timing_now_m_frame_timing_now;
    svc.level_frame_timing_log = checked_level_frame_timing_log_m_frame_timing_log;
    svc.driver_tick_dispatch = checked_driver_tick_dispatch_m_driver_tick_dispatch;
    svc.driver_tick_flush = checked_driver_tick_flush_m_driver_tick_flush;
    svc.driver_client_connection_count =
        checked_driver_client_connection_count_m_driver_client_connection_count;
    svc.driver_client_connection = checked_driver_client_connection_m_driver_client_connection;
    svc.conn_is_net_ready = checked_conn_is_net_ready_m_conn_is_net_ready;
    svc.conn_replicate_player_channels =
        checked_conn_replicate_player_channels_m_conn_replicate_player_channels;
    svc.conn_actor_channel = checked_conn_actor_channel_m_conn_actor_channel;
    svc.conn_create_actor_channel = checked_conn_create_actor_channel_m_conn_create_actor_channel;
    svc.conn_set_channel_actor = checked_conn_set_channel_actor_m_conn_set_channel_actor;
    svc.conn_package_map_class_index =
        checked_conn_package_map_class_index_m_conn_package_map_class_index;
    svc.channel_is_net_ready = checked_channel_is_net_ready_m_channel_is_net_ready;
    svc.channel_replicate_actor = checked_channel_replicate_actor_m_channel_replicate_actor;
    svc.actor_class_default_is_static =
        checked_actor_class_default_is_static_m_actor_class_default_is_static;
    svc.level_cleanup_destroyed = checked_level_cleanup_destroyed_m_level_cleanup_destroyed;
    svc.level_init_stats = checked_level_init_stats_m_level_init_stats;
    svc.level_init_runaway = checked_level_init_runaway_m_level_init_runaway;
    svc.level_gseconds_per_cycle = checked_level_gseconds_per_cycle_m_level_gseconds_per_cycle;
    svc.level_cycles_clock = checked_level_cycles_clock_m_level_cycles_clock;
    svc.level_cycles_unclock = checked_level_cycles_unclock_m_level_cycles_clock;
    svc.level_update_time = checked_level_update_time_m_level_update_time;
    svc.level_startup_phase_trace = checked_level_startup_phase_trace_m_level_startup_phase_trace;
    svc.level_hash_tick = checked_level_hash_tick_m_level_hash_tick;
    svc.level_engine_set_progress_empty =
        checked_level_engine_set_progress_empty_m_level_engine_set_progress_empty;
    svc.level_engine_client_travel =
        checked_level_engine_client_travel_m_level_engine_client_travel;
    svc.level_client_viewports_num =
        checked_level_client_viewports_num_m_level_client_viewports_num;
    svc.level_actor_count = checked_level_actor_count_m_level_actor_count;
    svc.level_actor = checked_level_actor_m_level_actor;
    svc.level_info = checked_level_info_m_level_info;
    svc.level_info_pauser_empty = checked_level_info_pauser_empty_m_level_info_pauser_empty;
    svc.mem_mark_pop = checked_mem_mark_pop_m_mem_mark_pop;
    svc.level_gmem = checked_level_gmem_m_level_gmem;
    svc.check_enabled = checked_check_enabled_m_check_enabled;
    svc.guard_scope = fixture_guard_scope;
    svc.level_tick_scoped = fixture_level_tick_scoped;
    svc.level_tick_net_client = fixture_level_tick_net_client;
    svc.level_tick_net_server = fixture_level_tick_net_server;
    svc.level_tick_demo_record = fixture_level_tick_demo_record;
    svc.level_tick_demo_playback = fixture_level_tick_demo_playback;
    svc.level_server_tick_client = fixture_level_server_tick_client;

    svc.server_tick_client_scoped = fixture_server_tick_client_scoped;
    svc.sort_priorities = fixture_sort_priorities;
    svc.tick_actor_scoped = fixture_tick_actor_scoped;
    svc
}

fn net_services() -> Hp2LevTicServices {
    let mut svc = services();
    svc.actor_event_player_calc_view =
        checked_actor_event_player_calc_view_m_actor_event_player_calc_view;
    svc.actor_update_frequency = checked_actor_update_frequency_m_actor_update_frequency;
    svc.actor_net_priority = checked_actor_net_priority_m_actor_net_priority;
    svc.actor_no_variables_to_replicate =
        checked_actor_no_variables_to_replicate_m_actor_no_variables_to_replicate;
    svc.actor_check_recent_changes =
        checked_actor_check_recent_changes_m_actor_check_recent_changes;
    svc.actor_class_default_life_span =
        checked_actor_class_default_life_span_m_actor_class_default_life_span;
    svc.level_fast_line_check = checked_level_fast_line_check_m_level_fast_line_check;
    svc.level_model_line_check = checked_level_model_line_check_m_level_model_line_check;
    svc.level_max_tick_rate = checked_level_max_tick_rate_m_level_max_tick_rate;
    svc.level_app_frand = checked_level_app_frand_m_level_app_frand;
    svc.level_log_running_net = checked_level_log_running_net_m_level_log_running_net;
    svc.level_send_net_stats = checked_level_send_net_stats_m_level_send_net_stats;
    svc.conn_sent_temp_count = checked_conn_sent_temp_count_m_conn_sent_temp_count;
    svc.channel_recent_count = checked_channel_recent_count_m_channel_recent_count;
    svc.channel_recent_first = checked_channel_recent_first_m_channel_recent_first;
    svc.channel_dirty_count = checked_channel_dirty_count_m_channel_dirty_count;
    svc.channel_close = checked_channel_close_m_channel_close;
    svc.gmath_sin_tab = checked_gmath_sin_tab_m_gmath_sin_tab;
    svc.gmath_cos_tab = checked_gmath_cos_tab_m_gmath_cos_tab;
    svc.mem_mark_pop = checked_mem_mark_pop_m_net_mem_mark_pop;
    svc.level_gmem = checked_level_gmem_m_net_gmem;
    svc.mem_push = checked_mem_push_m_net_mem_push;
    svc.guard_scope = fixture_guard_scope;
    svc.level_tick_scoped = fixture_level_tick_scoped;
    svc.level_tick_net_client = fixture_level_tick_net_client;
    svc.level_tick_net_server = fixture_level_tick_net_server;
    svc.level_tick_demo_record = fixture_level_tick_demo_record;
    svc.level_tick_demo_playback = fixture_level_tick_demo_playback;
    svc.level_server_tick_client = fixture_level_server_tick_client;

    svc.server_tick_client_scoped = fixture_server_tick_client_scoped;
    svc.sort_priorities = fixture_sort_priorities;
    svc.tick_actor_scoped = fixture_tick_actor_scoped;
    svc
}

fn replay(base: &[Row], rec: &Record) {
    OUT.with(|o| o.borrow_mut().clear());
    CHANNEL_ACTOR.with(|m| m.borrow_mut().clear());
    ROOT.with(|r| r.set(ptr::null()));
    TOKEN.with(|t| t.set(0));
    TIMING.with(|t| t.set(rec.timing));
    WORLD.with(|w| *w.borrow_mut() = build_world(base, &rec.ins));
    KNOBS.with(|k| *k.borrow_mut() = rec.knobs.clone());
    alloc_objects(&object_names(base, &rec.ins, &rec.knobs));
    sync_structs();

    let lv = ptr_of("LEV");
    CUR_LEVEL.with(|c| c.set(lv as usize));
    let svc = services();
    SVC.with(|s| s.set(&svc as *const Hp2LevTicServices as usize));
    let mut frame_start = 0_i64;
    unsafe {
        level_tick(
            &svc,
            lv,
            rec.tick_type,
            f32::from_bits(rec.dt_bits),
            &mut frame_start,
        )
        .unwrap()
    };

    let ticked = unsafe { (*lv.cast::<ULevel>()).ticked };
    let mut actual = OUT.with(|o| o.borrow().clone());
    actual.push("res level".to_string());
    actual.push(format!("out LEV Ticked {ticked}"));
    assert_eq!(
        actual, rec.expected,
        "level record {} (timing={}) diverged from fixture",
        rec.index, rec.timing
    );
}

#[test]
fn lev_tic_level_family_counts_match_fixture() {
    let fx = parse_fixture();
    assert_eq!(fx.levels.len(), 23, "level family record count");
    assert_eq!(fx.timing_headers, 3, "timing family header count");
    assert_eq!(
        fx.levels.iter().filter(|r| r.timing).count(),
        3,
        "timing-flagged level records"
    );
}

#[test]
fn lev_tic_level_family_replays_every_record() {
    let fx = parse_fixture();
    assert_eq!(fx.levels.len(), 23, "level family record count");
    for rec in &fx.levels {
        replay(&fx.base, rec);
    }
}
thread_local! {
    static TEST_PAUSED: Cell<bool> = const { Cell::new(true) };
    static TEST_CLOSE_MENU: Cell<bool> = const { Cell::new(false) };
    static TEST_EVENT_HISTORY: Cell<[f32; 4]> = const { Cell::new([0.0; 4]) };
}

unsafe extern "C" fn test_pauser_empty(_: *mut c_void, out: *mut i32) -> i32 {
    unsafe { *out = i32::from(TEST_PAUSED.with(|p| !p.get())) };
    0
}
unsafe extern "C" fn test_player_class(actor: *mut c_void, _: i32, out: *mut i32) -> i32 {
    unsafe { *out = i32::from(actor == ptr_of("PP0")) };
    0
}
unsafe extern "C" fn test_read_input(_: *mut c_void, _: f32) -> i32 { 0 }
unsafe extern "C" fn test_zero_input(_: *mut c_void) -> i32 { 0 }
unsafe extern "C" fn test_player_input(actor: *mut c_void, _: i32, _: f32) -> i32 {
    let p = unsafe { &*actor.cast::<APlayerPawn>() };
    TEST_EVENT_HISTORY.with(|h| h.set([p.smooth_mouse_x, p.smooth_mouse_y, p.borrowed_mouse_x, p.borrowed_mouse_y]));
    if TEST_CLOSE_MENU.with(Cell::get) { TEST_PAUSED.with(|p| p.set(false)); }
    0
}
unsafe extern "C" fn test_gameplay_tick(_: *mut c_void, _: f32, _: i32, _: *const c_char, _: *const c_char, out: *mut i32) -> i32 {
    unsafe { *out = 1 };
    0
}

#[test]
fn paused_level_clears_mouse_history_before_menu_tick_and_preserves_gameplay() {
    use hp2_engine::lev_tic::level_tick_body;
    let fx = parse_fixture();
    WORLD.with(|w| *w.borrow_mut() = build_world(&fx.base, &[]));
    alloc_objects(&object_names(&fx.base, &[], &BTreeMap::new()));
    sync_structs();
    WORLD.with(|w| { w.borrow_mut().insert(("LEV".into(), "Actors".into()), "[LI,PP0]".into()); });
    let lv = ptr_of("LEV").cast::<ULevel>();
    let pawn = ptr_of("PP0").cast::<APlayerPawn>();
    unsafe {
        (*lv).base.net_driver = ptr::null_mut();
        (*lv).base.demo_rec_driver = ptr::null_mut();
        (*lv).hash = ptr::null_mut();
        (*pawn).player = ptr_of("CONN0");
        (*pawn).smooth_mouse_x = 12.0;
        (*pawn).smooth_mouse_y = -8.0;
        (*pawn).borrowed_mouse_x = 3.0;
        (*pawn).borrowed_mouse_y = -2.0;
        (*pawn).a_forward = 7.0;
    }
    let mut svc = services();
    svc.level_info_pauser_empty = test_pauser_empty;
    svc.actor_is_a = test_player_class;
    svc.player_read_input = test_read_input;
    svc.pawn_zero_input_floats = test_zero_input;
    svc.actor_event = test_player_input;
    svc.tick_actor_scoped = test_gameplay_tick;
    TEST_PAUSED.with(|p| p.set(true));
    TEST_CLOSE_MENU.with(|p| p.set(true));
    unsafe {
        level_tick_body(&svc, lv.cast(), 2, 0.016, ptr_of("LI"), 0, 0, ptr::null_mut(), ptr::null_mut()).unwrap();
        assert_eq!(TEST_EVENT_HISTORY.with(Cell::get), [0.0; 4]);
        assert!(!TEST_PAUSED.with(Cell::get));
        assert_eq!((*pawn).a_forward, 7.0);
        // A later normal tick must not clear newly accumulated smoothing.
        (*pawn).smooth_mouse_x = 5.0;
        (*pawn).borrowed_mouse_y = 6.0;
        level_tick_body(&svc, lv.cast(), 2, 0.016, ptr_of("LI"), 0, 0, ptr::null_mut(), ptr::null_mut()).unwrap();
        assert_eq!((*pawn).smooth_mouse_x, 5.0);
        assert_eq!((*pawn).borrowed_mouse_y, 6.0);
    }
}

fn demo_snapshot() -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    unsafe {
        rows.push((
            "LI".to_string(),
            "LevelAction".to_string(),
            (*ptr_of("LI").cast::<ALevelInfo>())
                .level_action
                .to_string(),
        ));
        for name in ["A0", "A1", "A2", "A3", "A4", "A5", "PP0", "PP1", "Z0"] {
            let actor = ptr_of(name).cast::<AActor>();
            rows.push((
                name.to_string(),
                "Role".to_string(),
                (*actor).role.to_string(),
            ));
            rows.push((
                name.to_string(),
                "RemoteRole".to_string(),
                (*actor).remote_role.to_string(),
            ));
        }
    }
    rows
}

fn expected_demo_playback_calls() -> Vec<String> {
    const LEVEL_ACTION_CONNECTING: i32 = 3;
    const CONNECTION_CLOSED: i32 = 1;
    const CONNECTION_PENDING: i32 = 2;
    const TRAVEL_ENTRY: i32 = 1;

    let action = int_field("LI", "LevelAction") as i32;
    let server_connection = ptr_field("DEMOREC", "ServerConnection");
    let state = int_field(&name_of(server_connection), "State") as i32;
    let mut calls = Vec::new();
    if action == LEVEL_ACTION_CONNECTING && state != CONNECTION_PENDING {
        calls.push("progress_empty".to_string());
    }
    if state == CONNECTION_CLOSED {
        calls.push(format!("client_travel {TRAVEL_ENTRY}"));
    }
    calls
}

fn replay_demo(base: &[Row], rec: &Record, family: &str) {
    OUT.with(|o| o.borrow_mut().clear());
    CHANNEL_ACTOR.with(|m| m.borrow_mut().clear());
    DEMO_CALLS.with(|calls| calls.borrow_mut().clear());
    ROOT.with(|r| r.set(ptr::null()));
    TOKEN.with(|t| t.set(0));
    TIMING.with(|t| t.set(false));
    WORLD.with(|w| *w.borrow_mut() = build_world(base, &rec.ins));
    KNOBS.with(|k| *k.borrow_mut() = rec.knobs.clone());
    alloc_objects(&object_names(base, &rec.ins, &rec.knobs));
    sync_structs();

    let lv = ptr_of("LEV");
    CUR_LEVEL.with(|c| c.set(lv as usize));
    let svc = services();
    SVC.with(|s| s.set(&svc as *const Hp2LevTicServices as usize));
    let before = demo_snapshot();
    let delta_seconds = f32::from_bits(rec.dt_bits);
    let ret = unsafe {
        match family {
            "demorec" => tick_demo_record(&svc, lv, delta_seconds).unwrap(),
            "demoplay" => tick_demo_playback(&svc, lv, delta_seconds).unwrap(),
            _ => panic!("unsupported demo family {family}"),
        }
    };
    let after = demo_snapshot();

    let mut actual = OUT.with(|o| o.borrow().clone());
    actual.push(format!("res {family} ret={ret}"));
    assert_eq!(before.len(), after.len());
    for (before, after) in before.iter().zip(&after) {
        assert_eq!((&before.0, &before.1), (&after.0, &after.1));
        if before.2 != after.2 {
            actual.push(format!("out {} {} {}", after.0, after.1, after.2));
        }
    }
    assert_eq!(
        actual, rec.expected,
        "{family} record {} diverged from fixture",
        rec.index
    );

    let expected_calls = if family == "demoplay" {
        expected_demo_playback_calls()
    } else {
        Vec::new()
    };
    let calls = DEMO_CALLS.with(|calls| calls.borrow().clone());
    assert_eq!(
        calls, expected_calls,
        "{family} record {} engine service calls",
        rec.index
    );
    if family == "demorec" {
        for name in ["A0", "A1", "A2", "A3", "A4", "A5", "PP0", "PP1", "Z0"] {
            let actor = unsafe { &*ptr_of(name).cast::<AActor>() };
            assert!(!actor.b_demo_recording(), "demo flag not reset for {name}");
            assert!(
                !actor.b_client_demo_recording(),
                "client demo flag not reset for {name}"
            );
        }
    }
}

#[test]
fn lev_tic_demo_record_family_replays_every_record() {
    let (base, records) = parse_demo_family("demorec");
    assert_eq!(records.len(), 12, "demo-record family record count");
    for rec in &records {
        replay_demo(&base, rec, "demorec");
    }
}

#[test]
fn lev_tic_demo_playback_family_replays_every_record() {
    let (base, records) = parse_demo_family("demoplay");
    assert_eq!(records.len(), 12, "demo-playback family record count");
    for rec in &records {
        replay_demo(&base, rec, "demoplay");
    }
}

fn net_snapshot() -> Vec<(String, String, String)> {
    let mut rows = vec![(
        "LEV".to_string(),
        "NetTag".to_string(),
        unsafe { (*ptr_of("LEV").cast::<ULevel>()).net_tag }.to_string(),
    )];
    let object_names: Vec<String> = OBJECTS.with(|objects| {
        objects
            .borrow()
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    });
    for name in object_names.iter().filter(|name| name.starts_with("CONN")) {
        let conn = ptr_of(name).cast::<UNetConnection>();
        rows.push((
            name.clone(),
            "TickCount".to_string(),
            unsafe { (*conn).tick_count }.to_string(),
        ));
        rows.push((
            name.clone(),
            "LastRepTime".to_string(),
            i64::from_ne_bytes(unsafe { (*conn).last_rep_time }).to_string(),
        ));
    }
    for name in object_names.iter().filter(|name| name.starts_with("CH")) {
        let channel = ptr_of(name).cast::<UActorChannel>();
        rows.push((
            name.clone(),
            "RelevantTime".to_string(),
            i64::from_ne_bytes(unsafe { (*channel).relevant_time }).to_string(),
        ));
    }
    for name in ["A0", "A1", "A2", "A3", "A4", "A5", "PP0", "PP1", "Z0"] {
        rows.push((
            name.to_string(),
            "NetTag".to_string(),
            unsafe { (*ptr_of(name).cast::<AActor>()).net_tag }.to_string(),
        ));
    }
    rows
}

fn replay_net(base: &[Row], rec: &Record, family: &str) {
    OUT.with(|output| output.borrow_mut().clear());
    NET_FRAND.with(|state| state.set(7));
    CHANNEL_ACTOR.with(|actors| actors.borrow_mut().clear());
    NET_PRIORITY_LIST.with(|list| list.set(0));
    NET_PRIORITY_CAPACITY.with(|capacity| capacity.set(0));
    NET_PRIORITY_ARRAY.with(|array| array.set(0));
    NET_MEM_PUSHES.with(|calls| calls.set(0));
    NET_PRIORITY_AFTER.with(|pending| pending.set(0));
    WORLD.with(|world| *world.borrow_mut() = build_world(base, &rec.ins));
    KNOBS.with(|knobs| *knobs.borrow_mut() = rec.knobs.clone());
    alloc_objects(&object_names(base, &rec.ins, &rec.knobs));
    sync_structs();

    let level = ptr_of("LEV");
    CUR_LEVEL.with(|current| current.set(level as usize));
    let svc = net_services();
    SVC.with(|stored| stored.set(&svc as *const Hp2LevTicServices as usize));
    #[repr(align(16))]
    struct Arena([u8; 4096]);
    let mut arena = Arena([0; 4096]);
    let arena_base = arena.0.as_mut_ptr();
    let mut mem = MockMemStack {
        top: arena_base,
        end: unsafe { arena_base.add(arena.0.len()) },
    };
    NET_GMEM.with(|stack| stack.set(&mut mem as *mut MockMemStack as usize));

    let before = net_snapshot();
    unsafe {
        match family {
            "servertick" => {
                let connection = rec
                    .connection
                    .as_deref()
                    .expect("servertick record has a connection");
                server_tick_client(&svc, level, ptr_of(connection)).unwrap();
            }
            "netserver" => tick_net_server(&svc, level, f32::from_bits(rec.dt_bits)).unwrap(),
            _ => panic!("unsupported network family {family}"),
        }
    }
    flush_net_priority_trace();
    assert_eq!(
        NET_PRIORITY_AFTER.with(Cell::get),
        0,
        "network replay leaves no deferred priority trace"
    );
    let after = net_snapshot();

    let mut actual = OUT.with(|output| output.borrow().clone());
    actual.push(format!("res {family}"));
    assert_eq!(before.len(), after.len());
    for (before, after) in before.iter().zip(&after) {
        assert_eq!((&before.0, &before.1), (&after.0, &after.1));
        if before.2 != after.2 {
            actual.push(format!("out {} {} {}", after.0, after.1, after.2));
        }
    }
    assert_eq!(
        actual, rec.expected,
        "{family} record {} diverged from fixture; snapshots before={before:?}, after={after:?}",
        rec.index
    );
    NET_GMEM.with(|stack| stack.set(0));
}

#[test]
fn lev_tic_server_tick_family_replays_every_record() {
    let (base, records) = parse_demo_family("servertick");
    assert_eq!(records.len(), 40, "server-tick family record count");
    for rec in &records {
        replay_net(&base, rec, "servertick");
    }
}

#[test]
fn lev_tic_net_server_family_replays_every_record() {
    let (base, records) = parse_demo_family("netserver");
    assert_eq!(records.len(), 12, "net-server family record count");
    for rec in &records {
        replay_net(&base, rec, "netserver");
    }
}

// Checked adapters for the existing deterministic fixture callbacks.
unsafe extern "C" fn checked_actor_is_a_m_actor_is_a(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_actor_is_a(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_level_frame_timing_enabled_m_frame_timing_enabled(
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_frame_timing_enabled());
    }
    0
}
unsafe extern "C" fn checked_level_frame_timing_now_m_frame_timing_now(out: *mut i64) -> i32 {
    unsafe {
        out.write(m_frame_timing_now());
    }
    0
}
unsafe extern "C" fn checked_level_frame_timing_log_m_frame_timing_log(arg0: i64) -> i32 {
    unsafe {
        m_frame_timing_log(arg0);
    }
    0
}
unsafe extern "C" fn checked_driver_tick_dispatch_m_driver_tick_dispatch(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        m_driver_tick_dispatch(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_driver_tick_flush_m_driver_tick_flush(arg0: *mut c_void) -> i32 {
    unsafe {
        m_driver_tick_flush(arg0);
    }
    0
}
unsafe extern "C" fn checked_driver_client_connection_count_m_driver_client_connection_count(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_driver_client_connection_count(arg0));
    }
    0
}
unsafe extern "C" fn checked_driver_client_connection_m_driver_client_connection(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_driver_client_connection(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_conn_is_net_ready_m_conn_is_net_ready(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_conn_is_net_ready(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_conn_replicate_player_channels_m_conn_replicate_player_channels(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        m_conn_replicate_player_channels(arg0);
    }
    0
}
unsafe extern "C" fn checked_conn_actor_channel_m_conn_actor_channel(
    arg0: *mut c_void,
    arg1: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_conn_actor_channel(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_conn_create_actor_channel_m_conn_create_actor_channel(
    arg0: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_conn_create_actor_channel(arg0));
    }
    0
}
unsafe extern "C" fn checked_conn_set_channel_actor_m_conn_set_channel_actor(
    arg0: *mut c_void,
    arg1: *mut c_void,
) -> i32 {
    unsafe {
        m_conn_set_channel_actor(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_conn_package_map_class_index_m_conn_package_map_class_index(
    arg0: *mut c_void,
    arg1: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_conn_package_map_class_index(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_channel_is_net_ready_m_channel_is_net_ready(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_channel_is_net_ready(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_channel_replicate_actor_m_channel_replicate_actor(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        m_channel_replicate_actor(arg0);
    }
    0
}
unsafe extern "C" fn checked_actor_class_default_is_static_m_actor_class_default_is_static(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_actor_class_default_is_static(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_cleanup_destroyed_m_level_cleanup_destroyed(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        m_level_cleanup_destroyed(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_init_stats_m_level_init_stats(arg0: *mut c_void) -> i32 {
    unsafe {
        m_level_init_stats(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_init_runaway_m_level_init_runaway() -> i32 {
    unsafe {
        m_level_init_runaway();
    }
    0
}
unsafe extern "C" fn checked_level_gseconds_per_cycle_m_level_gseconds_per_cycle(
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(m_level_gseconds_per_cycle());
    }
    0
}
unsafe extern "C" fn checked_level_cycles_clock_m_level_cycles_clock(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        m_level_cycles_clock(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_cycles_unclock_m_level_cycles_clock(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        m_level_cycles_clock(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_update_time_m_level_update_time(arg0: *mut c_void) -> i32 {
    unsafe {
        m_level_update_time(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_startup_phase_trace_m_level_startup_phase_trace(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        m_level_startup_phase_trace(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_hash_tick_m_level_hash_tick(arg0: *mut c_void) -> i32 {
    unsafe {
        m_level_hash_tick(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_engine_set_progress_empty_m_level_engine_set_progress_empty(
    arg0: *mut c_void,
) -> i32 {
    unsafe {
        m_level_engine_set_progress_empty(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_engine_client_travel_m_level_engine_client_travel(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        m_level_engine_client_travel(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_m_level_client_viewports_num(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_level_client_viewports_num(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_actor_count_m_level_actor_count(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_level_actor_count(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_actor_m_level_actor(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_level_actor(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_level_info_m_level_info(
    arg0: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_level_info(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_info_pauser_empty_m_level_info_pauser_empty(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_level_info_pauser_empty(arg0));
    }
    0
}
unsafe extern "C" fn checked_mem_mark_pop_m_mem_mark_pop(arg0: *mut c_void) -> i32 {
    unsafe {
        m_mem_mark_pop(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_gmem_m_level_gmem(out: *mut *mut c_void) -> i32 {
    unsafe {
        out.write(m_level_gmem());
    }
    0
}
unsafe extern "C" fn checked_check_enabled_m_check_enabled(out: *mut i32) -> i32 {
    unsafe {
        out.write(m_check_enabled());
    }
    0
}
unsafe extern "C" fn checked_actor_event_player_calc_view_m_actor_event_player_calc_view(
    arg0: *mut c_void,
    arg1: *mut f32,
    arg2: *mut i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_actor_event_player_calc_view(arg0, arg1, arg2));
    }
    0
}
unsafe extern "C" fn checked_actor_update_frequency_m_actor_update_frequency(
    arg0: *mut c_void,
    arg1: *mut c_void,
    arg2: *mut f32,
    arg3: *mut f32,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(m_actor_update_frequency(arg0, arg1, arg2, arg3));
    }
    0
}
unsafe extern "C" fn checked_actor_net_priority_m_actor_net_priority(
    arg0: *mut c_void,
    arg1: *mut c_void,
    arg2: f32,
    arg3: f32,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(m_actor_net_priority(arg0, arg1, arg2, arg3));
    }
    0
}
unsafe extern "C" fn checked_actor_no_variables_to_replicate_m_actor_no_variables_to_replicate(
    arg0: *mut c_void,
    arg1: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_actor_no_variables_to_replicate(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_check_recent_changes_m_actor_check_recent_changes(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_actor_check_recent_changes(arg0));
    }
    0
}
unsafe extern "C" fn checked_actor_class_default_life_span_m_actor_class_default_life_span(
    arg0: *mut c_void,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(m_actor_class_default_life_span(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_fast_line_check_m_level_fast_line_check(
    arg0: *mut c_void,
    arg1: *const f32,
    arg2: *const f32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_level_fast_line_check(arg0, arg1, arg2));
    }
    0
}
unsafe extern "C" fn checked_level_model_line_check_m_level_model_line_check(
    arg0: *mut c_void,
    arg1: *mut f32,
    arg2: *const f32,
    arg3: *const f32,
    arg4: i32,
) -> i32 {
    unsafe {
        m_level_model_line_check(arg0, arg1, arg2, arg3, arg4);
    }
    0
}
unsafe extern "C" fn checked_level_max_tick_rate_m_level_max_tick_rate(
    arg0: *mut c_void,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(m_level_max_tick_rate(arg0));
    }
    0
}
unsafe extern "C" fn checked_level_app_frand_m_level_app_frand(out: *mut f32) -> i32 {
    unsafe {
        out.write(m_level_app_frand());
    }
    0
}
unsafe extern "C" fn checked_level_log_running_net_m_level_log_running_net(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        m_level_log_running_net(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_level_send_net_stats_m_level_send_net_stats(
    arg0: *mut c_void,
    arg1: i32,
) -> i32 {
    unsafe {
        m_level_send_net_stats(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_conn_sent_temp_count_m_conn_sent_temp_count(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_conn_sent_temp_count(arg0));
    }
    0
}
unsafe extern "C" fn checked_channel_recent_count_m_channel_recent_count(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_channel_recent_count(arg0));
    }
    0
}
unsafe extern "C" fn checked_channel_recent_first_m_channel_recent_first(
    arg0: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_channel_recent_first(arg0));
    }
    0
}
unsafe extern "C" fn checked_channel_dirty_count_m_channel_dirty_count(
    arg0: *mut c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(m_channel_dirty_count(arg0));
    }
    0
}
unsafe extern "C" fn checked_channel_close_m_channel_close(arg0: *mut c_void) -> i32 {
    unsafe {
        m_channel_close(arg0);
    }
    0
}
unsafe extern "C" fn checked_gmath_sin_tab_m_gmath_sin_tab(arg0: i32, out: *mut f32) -> i32 {
    unsafe {
        out.write(m_gmath_sin_tab(arg0));
    }
    0
}
unsafe extern "C" fn checked_gmath_cos_tab_m_gmath_cos_tab(arg0: i32, out: *mut f32) -> i32 {
    unsafe {
        out.write(m_gmath_cos_tab(arg0));
    }
    0
}
unsafe extern "C" fn checked_mem_mark_pop_m_net_mem_mark_pop(arg0: *mut c_void) -> i32 {
    unsafe {
        m_net_mem_mark_pop(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_gmem_m_net_gmem(out: *mut *mut c_void) -> i32 {
    unsafe {
        out.write(m_net_gmem());
    }
    0
}
unsafe extern "C" fn checked_mem_push_m_net_mem_push(
    arg0: *mut c_void,
    arg1: u64,
    arg2: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        out.write(m_net_mem_push(arg0, arg1, arg2));
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
    if unsafe { m_trace_enabled() } != 0 {
        dispatch.token =
            unsafe { m_trace_begin_dispatch(actor, m_trace_relation_get(), m_trace_root_get()) };
    }
    let result = unsafe { checked_actor_tick(svc, actor, delta, tick_type, &mut dispatch) };
    if dispatch.token != u64::MAX {
        unsafe {
            m_trace_end_dispatch(
                dispatch.token,
                if dispatch.event_tick_dispatched != 0 {
                    c"dispatched".as_ptr()
                } else {
                    dispatch.skip_reason
                },
            )
        };
    }
    result.unwrap()
}
unsafe extern "C" fn fixture_tick_actor_scoped(
    actor: *mut c_void,
    delta: f32,
    tick_type: i32,
    relation: *const c_char,
    root: *const c_char,
    out: *mut i32,
) -> i32 {
    unsafe {
        let previous_root = m_trace_root_set(root);
        out.write(m_tick_actor(actor, delta, tick_type));
        m_trace_root_set(previous_root);
    }
    0
}
unsafe extern "C" fn fixture_server_tick_client_scoped(
    level: *mut c_void,
    connection: *mut c_void,
    out: *mut i32,
) -> i32 {
    let svc = unsafe { *(&*(SVC.with(|s| s.get()) as *const Hp2LevTicServices)) };
    let mut mark = [0usize; 3];
    unsafe {
        m_net_mem_mark_save(m_net_gmem(), mark.as_mut_ptr().cast());
        match hp2_engine::lev_tic_net::server_tick_client_body(
            &svc,
            level,
            connection,
            mark.as_mut_ptr().cast(),
        ) {
            Ok(value) => {
                out.write(value);
                0
            }
            Err(status) => status,
        }
    }
}
unsafe extern "C" fn fixture_sort_priorities(
    actors: *mut *mut hp2_engine::lev_tic::ActorPriority,
    count: i32,
) -> i32 {
    let before_count = net_priority_count();
    if before_count != 0 {
        for line in net_priority_lines("before", before_count) {
            emit(line);
        }
        NET_PRIORITY_AFTER.with(|pending| pending.set(before_count));
    }
    unsafe {
        hp2_engine::lev_tic::sort_priority_ptrs(std::slice::from_raw_parts_mut(
            actors,
            count as usize,
        ));
    }
    0
}
unsafe extern "C" fn fixture_level_tick_scoped(
    level: *mut c_void,
    tick_type: i32,
    delta: f32,
    info: *mut c_void,
    timing: i32,
    start: i64,
) -> i32 {
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    let mut mark = [0usize; 3];
    let mut engine_mark = [0usize; 3];
    unsafe {
        m_mem_mark_save(m_level_gmem(), mark.as_mut_ptr().cast());
        m_mem_mark_save(m_level_gengine_mem(), engine_mark.as_mut_ptr().cast());
        hp2_engine::lev_tic::level_tick_body(
            svc,
            level,
            tick_type,
            delta,
            info,
            timing,
            start,
            mark.as_mut_ptr().cast(),
            engine_mark.as_mut_ptr().cast(),
        )
        .map_or_else(|s| s, |()| 0)
    }
}
unsafe extern "C" fn fixture_level_tick_net_client(level: *mut c_void, delta: f32) -> i32 {
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    unsafe { hp2_engine::lev_tic_net::tick_net_client(svc, level).map_or_else(|s| s, |_| 0) }
}
unsafe extern "C" fn fixture_level_tick_net_server(level: *mut c_void, delta: f32) -> i32 {
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    unsafe { hp2_engine::lev_tic_net::tick_net_server(svc, level, delta).map_or_else(|s| s, |_| 0) }
}
unsafe extern "C" fn fixture_level_tick_demo_record(level: *mut c_void, delta: f32) -> i32 {
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    unsafe {
        hp2_engine::lev_tic_demo::tick_demo_record(svc, level, delta).map_or_else(|s| s, |_| 0)
    }
}
unsafe extern "C" fn fixture_level_tick_demo_playback(level: *mut c_void, delta: f32) -> i32 {
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    unsafe {
        hp2_engine::lev_tic_demo::tick_demo_playback(svc, level, delta).map_or_else(|s| s, |_| 0)
    }
}
unsafe extern "C" fn fixture_level_server_tick_client(
    level: *mut c_void,
    connection: *mut c_void,
    _delta: f32,
    out: *mut i32,
) -> i32 {
    flush_net_priority_trace();
    let svc = unsafe { &*(SVC.with(|s| s.get()) as *const Hp2LevTicServices) };
    match unsafe { hp2_engine::lev_tic_net::server_tick_client(svc, level, connection) } {
        Ok(value) => {
            unsafe { out.write(value) };
            0
        }
        Err(status) => status,
    }
}
