//! Port of `CreatureGeneratorTraceReporter`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 4456-4723). A per-Tick, bounded observation of the interpreted
//! `CreatureGenerator` path. Observes existing script/native calls only;
//! property reads never resolve references or alter the VM's
//! scheduler/RNG state. Gated by `HP2_CREATURE_GENERATOR_TRACE`.

use std::ffi::c_void;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use super::trace_services::{
    self as svc, Hp2TraceReporterServices, object_name, object_path, push_cpp_float,
    push_json_bool, push_json_string, push_json_string_or_null, read_i32_at, read_ptr_at,
    write_report,
};

const DEFAULT_LIMIT: u64 = 4096;
const MAX_LIMIT: u64 = 65536;

#[derive(Default, Clone)]
struct Slot {
    present: bool,
    path: Vec<u8>,
    class: Vec<u8>,
}

struct Gate {
    name: &'static [u8],
    passed: bool,
}

struct Rng {
    kind: &'static [u8],
    resolved_class: Vec<u8>,
    raw: i32,
    bound: i32,
    index: i32,
    min: f32,
    max: f32,
    value: f32,
    has_resolved_class: bool,
}

struct Event {
    owner: *mut c_void,
    identity: Vec<u8>,
    tag: Vec<u8>,
    frame: u64,
    order: i32,
    curr_time: f32,
    wait_time: f32,
    delta_seconds: f32,
    trigger_waiting_time: f32,
    b_off: bool,
    generate_creature: bool,
    active_count: i32,
    active_max: i32,
    base_creature_count: i32,
    first_pp: Slot,
    has_camera_visible: bool,
    camera_visible: bool,
    gates: Vec<Gate>,
    rng: Vec<Rng>,
    spawn_request: Slot,
    has_spawn_request: bool,
    has_spawn_result: bool,
    has_spawn_published: bool,
    generate_prefix_recorded: bool,
    class_resolution_recorded: bool,
    spawn_result: Slot,
    spawn_published: Slot,
}

struct State {
    report_path: Vec<u8>,
    limit: usize,
    frame: u64,
    next_order: i32,
    truncated: bool,
    active_generator: *mut c_void,
    events: Vec<Event>,
}

static mut STATE: Option<State> = None;

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

fn is_enabled() -> bool {
    env_bytes("HP2_CREATURE_GENERATOR_TRACE").is_some_and(|v| !v.is_empty())
}

fn parse_limit(value: Option<Vec<u8>>) -> usize {
    let bytes = match value {
        Some(bytes) if !bytes.is_empty() => bytes,
        _ => return DEFAULT_LIMIT as usize,
    };
    match std::str::from_utf8(&bytes).ok().and_then(|t| t.parse::<u64>().ok()) {
        Some(parsed) => parsed.min(MAX_LIMIT) as usize,
        None => DEFAULT_LIMIT as usize,
    }
}

/// # Safety
/// Single game thread; no concurrent reporter call.
pub unsafe fn create() {
    if !is_enabled() {
        return;
    }
    let state = State {
        report_path: env_bytes("HP2_CREATURE_GENERATOR_TRACE").unwrap_or_default(),
        limit: parse_limit(env_bytes("HP2_CREATURE_GENERATOR_TRACE_LIMIT")),
        frame: 0,
        next_order: 0,
        truncated: false,
        active_generator: std::ptr::null_mut(),
        events: Vec::new(),
    };
    unsafe { (&raw mut STATE).write(Some(state)) };
}

/// # Safety
/// Single game thread; no concurrent reporter call.
pub unsafe fn reset() {
    unsafe { (&raw mut STATE).write(None) };
}

/// # Safety
/// Single game thread.
pub unsafe fn present() -> i32 {
    i32::from(unsafe { (&raw const STATE).as_ref().unwrap().is_some() })
}

/// # Safety
/// Single game thread.
pub unsafe fn set_frame(frame: u64) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        state.frame = frame;
        state.next_order = 0;
    }
}

fn is_creature(services: &Hp2TraceReporterServices, actor: *mut c_void) -> bool {
    let mut class = if actor.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (services.object_class)(actor) }
    };
    while !class.is_null() {
        let name = unsafe { object_name(services, class) };
        if name.eq_ignore_ascii_case(b"CreatureGenerator") {
            return true;
        }
        class = unsafe { (services.class_super)(class) };
    }
    false
}

fn function_named(services: &Hp2TraceReporterServices, function: *mut c_void, name: &[u8]) -> bool {
    unsafe { object_name(services, function) }.eq_ignore_ascii_case(name)
}

fn make_slot(services: &Hp2TraceReporterServices, object: *mut c_void) -> Slot {
    if object.is_null() {
        return Slot::default();
    }
    let class = unsafe { (services.object_class)(object) };
    Slot {
        present: true,
        path: unsafe { object_path(services, object) },
        class: unsafe { object_path(services, class) },
    }
}

/// `Property`: `Object && Object->GetClass() ? FindField<UProperty>(Object->GetClass(), Name) : NULL`.
fn property(
    services: &Hp2TraceReporterServices,
    object: *mut c_void,
    name: *const std::ffi::c_char,
) -> *mut c_void {
    if object.is_null() {
        return std::ptr::null_mut();
    }
    let class = unsafe { (services.object_class)(object) };
    if class.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (services.find_property)(class, name) }
}

fn object_property(
    services: &Hp2TraceReporterServices,
    object: *mut c_void,
    name: *const std::ffi::c_char,
) -> *mut c_void {
    let field = property(services, object, name);
    if field.is_null() {
        return std::ptr::null_mut();
    }
    let offset = unsafe { (services.property_offset)(field) };
    unsafe { read_ptr_at(object, offset) }
}

fn bool_property(
    services: &Hp2TraceReporterServices,
    object: *mut c_void,
    name: *const std::ffi::c_char,
) -> Option<bool> {
    if object.is_null() {
        return None;
    }
    let class = unsafe { (services.object_class)(object) };
    if class.is_null() {
        return None;
    }
    let field = unsafe { (services.find_bool_property)(class, name) };
    if field.is_null() {
        return None;
    }
    let offset = unsafe { (services.property_offset)(field) };
    let bitmask = unsafe { (services.bool_property_bitmask)(field) };
    let word = unsafe { super::trace_services::read_u32_at(object, offset) };
    Some(word & bitmask != 0)
}

fn int_property(
    services: &Hp2TraceReporterServices,
    object: *mut c_void,
    name: *const std::ffi::c_char,
) -> Option<i32> {
    if object.is_null() {
        return None;
    }
    let class = unsafe { (services.object_class)(object) };
    if class.is_null() {
        return None;
    }
    let field = unsafe { (services.find_int_property)(class, name) };
    if field.is_null() {
        return None;
    }
    let offset = unsafe { (services.property_offset)(field) };
    Some(unsafe { read_i32_at(object, offset) })
}

fn float_property(
    services: &Hp2TraceReporterServices,
    object: *mut c_void,
    name: *const std::ffi::c_char,
) -> Option<f32> {
    if object.is_null() {
        return None;
    }
    let class = unsafe { (services.object_class)(object) };
    if class.is_null() {
        return None;
    }
    let field = unsafe { (services.find_float_property)(class, name) };
    if field.is_null() {
        return None;
    }
    let offset = unsafe { (services.property_offset)(field) };
    Some(unsafe { super::trace_services::read_f32_at(object, offset) })
}

fn contiguous_base_creature_count(services: &Hp2TraceReporterServices, actor: *mut c_void) -> i32 {
    let field = property(services, actor, c"BaseCreatureToSpawn".as_ptr());
    if field.is_null() {
        return 0;
    }
    let offset = unsafe { (services.property_offset)(field) };
    let array_dim = unsafe { (services.property_array_dim)(field) };
    let element_size = unsafe { (services.property_element_size)(field) };
    let mut count = 0;
    for index in 0..array_dim {
        let ptr = unsafe { read_ptr_at(actor, offset + index * element_size) };
        if ptr.is_null() {
            break;
        }
        count += 1;
    }
    count
}

fn normalized_identity(services: &Hp2TraceReporterServices, actor: *mut c_void) -> Vec<u8> {
    let mut identity = b"<map>.".to_vec();
    identity.extend_from_slice(&unsafe { object_name(services, actor) });
    identity
}

fn append_gate(event: &mut Event, name: &'static [u8], passed: bool) {
    event.gates.push(Gate { name, passed });
}

fn current(services: &Hp2TraceReporterServices, state: &mut State, actor: *mut c_void) -> Option<usize> {
    if !is_creature(services, actor) {
        return None;
    }
    state.events.iter().rposition(|e| e.owner == actor)
}

fn append_generate_prefix(services: &Hp2TraceReporterServices, event: &mut Event, actor: *mut c_void) {
    append_gate(event, b"b_off", !event.b_off);
    if event.b_off {
        return;
    }
    let trigger_pass = !(event.trigger_waiting_time != 0.0 && !event.generate_creature);
    append_gate(event, b"trigger_waiting", trigger_pass);
    if !trigger_pass {
        return;
    }
    let count = int_property(services, actor, c"NumCreatures".as_ptr()).unwrap_or(0);
    event.active_count = count;
    let capacity_pass = count < event.active_max;
    append_gate(event, b"active_at_capacity", capacity_pass);
    if !capacity_pass {
        return;
    }
    event.first_pp = make_slot(services, object_property(services, actor, c"FirstPP".as_ptr()));
    append_gate(event, b"missing_first_pp", event.first_pp.present);
}

fn append_base_creature_gate(services: &Hp2TraceReporterServices, event: &mut Event, actor: *mut c_void) {
    event.base_creature_count = contiguous_base_creature_count(services, actor);
    append_gate(event, b"empty_base_creatures", event.base_creature_count > 0);
}

fn with_state(f: impl FnOnce(&Hp2TraceReporterServices, &mut State)) {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return;
    };
    f(&services, state);
}

/// `TickDispatch`.
///
/// # Safety
/// Single game thread; `actor` is null or a live `AActor*`.
pub unsafe fn tick_dispatch(actor: *mut c_void, delta_seconds: f32) {
    with_state(|s, state| {
        let creature = is_creature(s, actor);
        if !creature || state.events.len() >= state.limit {
            if creature {
                state.truncated = true;
            }
            return;
        }
        let mut event = Event {
            owner: actor,
            identity: normalized_identity(s, actor),
            tag: Vec::new(),
            frame: state.frame,
            order: state.next_order,
            curr_time: 0.0,
            wait_time: 0.0,
            delta_seconds,
            trigger_waiting_time: 0.0,
            b_off: false,
            generate_creature: false,
            active_count: 0,
            active_max: 0,
            base_creature_count: 0,
            first_pp: Slot::default(),
            has_camera_visible: false,
            camera_visible: false,
            gates: Vec::new(),
            rng: Vec::new(),
            spawn_request: Slot::default(),
            has_spawn_request: false,
            has_spawn_result: false,
            has_spawn_published: false,
            generate_prefix_recorded: false,
            class_resolution_recorded: false,
            spawn_result: Slot::default(),
            spawn_published: Slot::default(),
        };
        state.next_order += 1;
        event.curr_time = float_property(s, actor, c"fCurrTime".as_ptr()).unwrap_or(0.0);
        event.wait_time = float_property(s, actor, c"fWaitTime".as_ptr()).unwrap_or(0.0);
        event.b_off = bool_property(s, actor, c"bOff".as_ptr()).unwrap_or(false);
        event.trigger_waiting_time =
            float_property(s, actor, c"TriggerWaitingTime".as_ptr()).unwrap_or(0.0);
        event.generate_creature =
            bool_property(s, actor, c"bGenerateCreature".as_ptr()).unwrap_or(false);
        let tag_index = unsafe { (s.actor_tag_fname)(actor) };
        event.tag = unsafe { svc::ansi((s.fname_ansi_or_empty)(tag_index)) };
        event.first_pp = make_slot(s, object_property(s, actor, c"FirstPP".as_ptr()));
        event.base_creature_count = contiguous_base_creature_count(s, actor);
        let tick_wait_pass = event.curr_time + delta_seconds >= event.wait_time;
        append_gate(&mut event, b"tick_wait", tick_wait_pass);
        state.events.push(event);
    });
}

/// # Safety
/// `object`/`function` null or live.
pub unsafe fn function_enter(object: *mut c_void, function: *mut c_void) {
    with_state(|s, state| {
        let actor = if object.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (s.cast_to_actor)(object) }
        };
        if !is_creature(s, actor) || function.is_null() {
            return;
        }
        if function_named(s, function, b"GenerateCreature") {
            state.active_generator = actor;
        }
    });
}

/// # Safety
/// `object`/`function` null or live; `result`, when non-null, points to a
/// live byte for exactly as long as this call (the original's `void*
/// Result` from `CameraCanSeeYou`'s return slot, read as `*(bool*)Result`
/// in the deleted C++ — a raw byte compare preserves that without reading
/// through an invalid Rust `bool`).
pub unsafe fn function_exit(object: *mut c_void, function: *mut c_void, result: *const u8) {
    with_state(|s, state| {
        let actor = if object.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (s.cast_to_actor)(object) }
        };
        if !is_creature(s, actor) || function.is_null() || state.active_generator != actor {
            return;
        }
        if function_named(s, function, b"CameraCanSeeYou") {
            if let Some(index) = current(s, state, actor) {
                let camera_visible = !result.is_null() && unsafe { *result } != 0;
                let event = &mut state.events[index];
                event.has_camera_visible = true;
                event.camera_visible = camera_visible;
                append_gate(event, b"camera_visible", !camera_visible);
                if !camera_visible {
                    append_base_creature_gate(s, event, actor);
                }
            }
        } else if function_named(s, function, b"GenerateCreature") {
            if let Some(index) = current(s, state, actor) {
                state.events[index].active_max =
                    int_property(s, actor, c"MaxCreatures".as_ptr()).unwrap_or(0);
            }
            state.active_generator = std::ptr::null_mut();
        }
    });
}

/// # Safety
/// `actor` null or live.
pub unsafe fn software_rendering(actor: *mut c_void, is_software_rendering: i32) {
    with_state(|s, state| {
        if state.active_generator.is_null() || actor.is_null() {
            return;
        }
        let active_generator = state.active_generator;
        let Some(index) = current(s, state, active_generator) else {
            return;
        };
        if state.events[index].generate_prefix_recorded {
            return;
        }
        let fast = int_property(s, active_generator, c"MaxCreaturesOnFastMachine".as_ptr()).unwrap_or(0);
        let slow = int_property(s, active_generator, c"MaxCreaturesOnSlowMachine".as_ptr()).unwrap_or(0);
        let event = &mut state.events[index];
        event.generate_prefix_recorded = true;
        event.active_max = if is_software_rendering != 0 { slow } else { fast };
        event.active_count =
            int_property(s, active_generator, c"NumCreatures".as_ptr()).unwrap_or(0);
        append_generate_prefix(s, event, active_generator);
    });
}

/// `Rand`.
///
/// # Safety
/// `object` null or live.
pub unsafe fn rand(object: *mut c_void, raw: i32, bound: i32, index: i32) {
    with_state(|s, state| {
        let actor = if object.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (s.cast_to_actor)(object) }
        };
        if actor != state.active_generator {
            return;
        }
        if let Some(event_index) = current(s, state, actor) {
            state.events[event_index].rng.push(Rng {
                kind: b"rand",
                resolved_class: Vec::new(),
                raw,
                bound,
                index,
                min: 0.0,
                max: 0.0,
                value: 0.0,
                has_resolved_class: false,
            });
        }
    });
}

/// `RandRange`.
///
/// # Safety
/// `object` null or live.
pub unsafe fn rand_range(object: *mut c_void, raw: i32, min: f32, max: f32, value: f32) {
    with_state(|s, state| {
        let actor = if object.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (s.cast_to_actor)(object) }
        };
        if !is_creature(s, actor) {
            return;
        }
        if let Some(event_index) = current(s, state, actor) {
            state.events[event_index].rng.push(Rng {
                kind: b"rand_range",
                resolved_class: Vec::new(),
                raw,
                bound: 0,
                index: 0,
                min,
                max,
                value,
                has_resolved_class: false,
            });
        }
    });
}

/// # Safety
/// `class`/`owner` null or live.
pub unsafe fn class_resolution(class: *mut c_void, owner: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = current(s, state, owner) else {
            return;
        };
        if state.events[index].class_resolution_recorded {
            return;
        }
        state.events[index].class_resolution_recorded = true;
        let has_class = !class.is_null();
        append_gate(&mut state.events[index], b"class_resolution", has_class);
        for rng in state.events[index].rng.iter_mut().rev() {
            if rng.kind == b"rand" && !rng.has_resolved_class {
                rng.has_resolved_class = true;
                if has_class {
                    rng.resolved_class = unsafe { object_path(s, class) };
                }
                break;
            }
        }
    });
}

/// # Safety
/// `class`/`owner` null or live.
pub unsafe fn spawn_request(class: *mut c_void, owner: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = current(s, state, owner) else {
            return;
        };
        state.events[index].has_spawn_request = true;
        state.events[index].spawn_request = make_slot(s, class);
    });
}

/// # Safety
/// `owner`/`child` null or live.
pub unsafe fn spawn_result(owner: *mut c_void, child: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = current(s, state, owner) else {
            return;
        };
        state.events[index].has_spawn_result = true;
        state.events[index].spawn_result = make_slot(s, child);
    });
}

/// # Safety
/// `owner`/`child` null or live.
pub unsafe fn spawn_published(owner: *mut c_void, child: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = current(s, state, owner) else {
            return;
        };
        state.events[index].has_spawn_published = true;
        state.events[index].spawn_published = make_slot(s, child);
    });
}

fn push_slot_json(slot: &Slot, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"path\":");
    if slot.present {
        push_json_string_or_null(&slot.path, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"class\":");
    if slot.present {
        push_json_string_or_null(&slot.class, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.push(b'}');
}

fn serialize_event(event: &Event, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"identity\":");
    push_json_string(&event.identity, out);
    out.extend_from_slice(b",\"frame\":");
    out.extend_from_slice(event.frame.to_string().as_bytes());
    out.extend_from_slice(b",\"tick\":{\"eligible\":");
    push_json_bool(event.gates[0].passed, out);
    out.extend_from_slice(b",\"order\":");
    out.extend_from_slice(event.order.to_string().as_bytes());
    out.extend_from_slice(b"},\"timing\":{\"curr_time\":");
    push_cpp_float(event.curr_time, out);
    out.extend_from_slice(b",\"wait_time\":");
    push_cpp_float(event.wait_time, out);
    out.extend_from_slice(b",\"delta_seconds\":");
    push_cpp_float(event.delta_seconds, out);
    out.extend_from_slice(b"},\"b_off\":");
    push_json_bool(event.b_off, out);
    out.extend_from_slice(b",\"trigger\":{\"waiting_time\":");
    push_cpp_float(event.trigger_waiting_time, out);
    out.extend_from_slice(b",\"generate_creature\":");
    push_json_bool(event.generate_creature, out);
    out.extend_from_slice(b",\"tag\":");
    push_json_string_or_null(&event.tag, out);
    out.extend_from_slice(b"},\"active\":{\"count\":");
    out.extend_from_slice(event.active_count.to_string().as_bytes());
    out.extend_from_slice(b",\"max\":");
    out.extend_from_slice(event.active_max.to_string().as_bytes());
    out.extend_from_slice(b"},\"first_pp\":");
    push_slot_json(&event.first_pp, out);
    out.extend_from_slice(b",\"camera_visible\":");
    if event.has_camera_visible {
        push_json_bool(event.camera_visible, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"base_creature_count\":");
    out.extend_from_slice(event.base_creature_count.to_string().as_bytes());
    out.extend_from_slice(b",\"gates\":[");
    for (index, gate) in event.gates.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"name\":");
        push_json_string(gate.name, out);
        out.extend_from_slice(b",\"passed\":");
        push_json_bool(gate.passed, out);
        out.push(b'}');
    }
    out.extend_from_slice(b"],\"rng\":[");
    for (index, rng) in event.rng.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"kind\":");
        push_json_string(rng.kind, out);
        out.extend_from_slice(b",\"raw\":");
        out.extend_from_slice(rng.raw.to_string().as_bytes());
        out.extend_from_slice(b",\"index\":");
        if rng.kind == b"rand" {
            out.extend_from_slice(rng.index.to_string().as_bytes());
        } else {
            out.extend_from_slice(b"null");
        }
        out.extend_from_slice(b",\"resolved_class\":");
        if rng.has_resolved_class {
            push_json_string_or_null(&rng.resolved_class, out);
        } else {
            out.extend_from_slice(b"null");
        }
        if rng.kind == b"rand" {
            out.extend_from_slice(b",\"bound\":");
            out.extend_from_slice(rng.bound.to_string().as_bytes());
        } else {
            out.extend_from_slice(b",\"min\":");
            push_cpp_float(rng.min, out);
            out.extend_from_slice(b",\"max\":");
            push_cpp_float(rng.max, out);
            out.extend_from_slice(b",\"result\":");
            push_cpp_float(rng.value, out);
        }
        out.push(b'}');
    }
    out.extend_from_slice(b"],\"spawn\":{\"request\":");
    if event.has_spawn_request {
        push_slot_json(&event.spawn_request, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"result\":");
    if event.has_spawn_result {
        push_slot_json(&event.spawn_result, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"published_child\":");
    if event.has_spawn_published {
        push_slot_json(&event.spawn_published, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b"}}");
}

fn serialize(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"version\":1,\"enabled\":true,\"limit\":");
    out.extend_from_slice(state.limit.to_string().as_bytes());
    out.extend_from_slice(b",\"count\":");
    out.extend_from_slice(state.events.len().to_string().as_bytes());
    out.extend_from_slice(b",\"truncated\":");
    push_json_bool(state.truncated, &mut out);
    out.extend_from_slice(b",\"events\":[");
    for (index, event) in state.events.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        serialize_event(event, &mut out);
    }
    out.extend_from_slice(b"]}\n");
    out
}

/// # Safety
/// Single game thread.
pub unsafe fn flush() {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let document = serialize(state);
    match write_report(&state.report_path, &document) {
        Ok(()) => {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(b"<HP2_CREATURE_GENERATOR_TRACE> report=");
            let _ = out.write_all(&state.report_path);
            let _ = out.write_all(
                format!(
                    " events={} truncated={}\n",
                    state.events.len(),
                    i32::from(state.truncated)
                )
                .as_bytes(),
            );
        }
        Err(kind) => {
            let reason: &[u8] = match kind {
                svc::ReportWriteError::Unwritable => b"unwritable",
                svc::ReportWriteError::WriteFailed => b"write_failed",
            };
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(b"<HP2_CREATURE_GENERATOR_TRACE> report_error=");
            let _ = err.write_all(reason);
            let _ = err.write_all(b" path=");
            let _ = err.write_all(&state.report_path);
            let _ = err.write_all(b"\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ground truth: `CreatureGeneratorTraceReporter::ParseLimit`
    // (SDLLaunch.cpp, deleted, formerly ~4429-4437): same shape as the
    // actor-transition-ledger reporter's, clamped to 65536.
    #[test]
    fn parse_limit_matches_cpp_cases() {
        assert_eq!(parse_limit(None), 4096);
        assert_eq!(parse_limit(Some(b"".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"x".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"100".to_vec())), 100);
        assert_eq!(parse_limit(Some(b"65536".to_vec())), 65536);
        assert_eq!(parse_limit(Some(b"70000".to_vec())), 65536);
    }

    fn minimal_event() -> Event {
        Event {
            owner: std::ptr::null_mut(),
            identity: b"Package.Creature".to_vec(),
            tag: Vec::new(),
            frame: 5,
            order: 0,
            curr_time: 1.0,
            wait_time: 2.0,
            delta_seconds: 0.016,
            trigger_waiting_time: 0.0,
            b_off: false,
            generate_creature: false,
            active_count: 0,
            active_max: 0,
            base_creature_count: 0,
            first_pp: Slot::default(),
            has_camera_visible: false,
            camera_visible: false,
            gates: vec![Gate {
                name: b"tick_wait",
                passed: true,
            }],
            rng: Vec::new(),
            spawn_request: Slot::default(),
            has_spawn_request: false,
            has_spawn_result: false,
            has_spawn_published: false,
            generate_prefix_recorded: false,
            class_resolution_recorded: false,
            spawn_result: Slot::default(),
            spawn_published: Slot::default(),
        }
    }

    // Ground truth: `SerializeEvent` on a minimal event with no RNG samples
    // and `HasCameraVisible=false` (SDLLaunch.cpp, deleted, formerly
    // ~4504-4513): `camera_visible` renders `null`, not `false`, when the
    // flag is unset; `tag` renders `null` via `JsonStringOrNull` when empty.
    #[test]
    fn serialize_event_minimal_matches_cpp_serialize_event() {
        let event = minimal_event();
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        assert_eq!(
            out,
            b"{\"identity\":\"Package.Creature\",\"frame\":5,\"tick\":{\"eligible\":true,\"order\":0},\"timing\":{\"curr_time\":1.000000,\"wait_time\":2.000000,\"delta_seconds\":0.016000},\"b_off\":false,\"trigger\":{\"waiting_time\":0.000000,\"generate_creature\":false,\"tag\":null},\"active\":{\"count\":0,\"max\":0},\"first_pp\":{\"path\":null,\"class\":null},\"camera_visible\":null,\"base_creature_count\":0,\"gates\":[{\"name\":\"tick_wait\",\"passed\":true}],\"rng\":[],\"spawn\":{\"request\":null,\"result\":null,\"published_child\":null}}"
                .to_vec()
        );
    }

    // Ground truth: `rand`'s `Index` is emitted, `rand_range`'s is `null`;
    // `rand` emits `bound`, `rand_range` emits `min`/`max`/`result` instead
    // (SDLLaunch.cpp, deleted, formerly ~4511: the `It->Kind=="rand"`
    // ternary chain).
    #[test]
    fn serialize_event_rng_entries_branch_on_kind() {
        let mut event = minimal_event();
        event.rng.push(Rng {
            kind: b"rand",
            resolved_class: b"Package.ResolvedClass".to_vec(),
            raw: 10,
            bound: 3,
            index: 1,
            min: 0.0,
            max: 0.0,
            value: 0.0,
            has_resolved_class: true,
        });
        event.rng.push(Rng {
            kind: b"rand_range",
            resolved_class: Vec::new(),
            raw: 20,
            bound: 0,
            index: 0,
            min: 1.5,
            max: 2.5,
            value: 2.0,
            has_resolved_class: false,
        });
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(
            "\"rng\":[{\"kind\":\"rand\",\"raw\":10,\"index\":1,\"resolved_class\":\"Package.ResolvedClass\",\"bound\":3},{\"kind\":\"rand_range\",\"raw\":20,\"index\":null,\"resolved_class\":null,\"min\":1.500000,\"max\":2.500000,\"result\":2.000000}]"
        ));
    }

    #[test]
    fn serialize_event_camera_visible_present_renders_bool_not_null() {
        let mut event = minimal_event();
        event.has_camera_visible = true;
        event.camera_visible = true;
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"camera_visible\":true"));
    }

    // Ground truth: `CreatureGeneratorTraceReporter::Serialize` on an empty
    // reporter (SDLLaunch.cpp, deleted, formerly ~4518-4522).
    #[test]
    fn serialize_of_an_empty_state_matches_cpp_serialize() {
        let state = State {
            report_path: Vec::new(),
            limit: 4096,
            frame: 0,
            next_order: 0,
            truncated: false,
            active_generator: std::ptr::null_mut(),
            events: Vec::new(),
        };
        assert_eq!(
            serialize(&state),
            b"{\"version\":1,\"enabled\":true,\"limit\":4096,\"count\":0,\"truncated\":false,\"events\":[]}\n"
                .to_vec()
        );
    }
}
