//! Port of `GlobalTickTraceReporter`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 4728-4915). A bounded, opt-in observation of every native `AActor::Tick`
//! invocation. It records existing scheduling facts only after the `Tick`
//! body returns, so event admission and `bTicked` are the engine's actual
//! outcomes. Gated by `HP2_GLOBAL_TICK_TRACE`.

use std::ffi::{c_char, c_void};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use super::trace_services::{self as svc, ansi, object_path, push_json_bool, write_report};

const DEFAULT_LIMIT: u64 = 16384;
const MAX_LIMIT: u64 = 262144;
const TOKEN_LIMIT_REACHED: u64 = u64::MAX;

struct Event {
    actor: *mut c_void,
    frame: u64,
    ordinal: u64,
    tick_stamp: u64,
    rng_before: u64,
    rng_after: u64,
    path: Vec<u8>,
    class: Vec<u8>,
    relation: Vec<u8>,
    root_admission: Vec<u8>,
    tick_outcome: Vec<u8>,
    process_state_identity: Vec<u8>,
    process_state_outcome: Vec<u8>,
}

impl Event {
    fn new() -> Self {
        Event {
            actor: std::ptr::null_mut(),
            frame: 0,
            ordinal: 0,
            tick_stamp: 0,
            rng_before: 0,
            rng_after: 0,
            path: Vec::new(),
            class: Vec::new(),
            relation: Vec::new(),
            root_admission: Vec::new(),
            tick_outcome: b"skip_no_event".to_vec(),
            process_state_identity: b"None".to_vec(),
            process_state_outcome: b"not_called".to_vec(),
        }
    }
}

struct State {
    report_path: Vec<u8>,
    limit: usize,
    frame: u64,
    next_ordinal: u64,
    truncated: bool,
    events: Vec<Event>,
    active_dispatches: Vec<u64>,
}

static mut STATE: Option<State> = None;

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

fn is_enabled() -> bool {
    env_bytes("HP2_GLOBAL_TICK_TRACE").is_some_and(|v| !v.is_empty())
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
        report_path: env_bytes("HP2_GLOBAL_TICK_TRACE").unwrap_or_default(),
        limit: parse_limit(env_bytes("HP2_GLOBAL_TICK_TRACE_LIMIT")),
        frame: 0,
        next_ordinal: 0,
        truncated: false,
        events: Vec::new(),
        active_dispatches: Vec::new(),
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
    }
}

fn actor_path(services: &svc::Hp2TraceReporterServices, actor: *mut c_void) -> Vec<u8> {
    let mut out = b"<map>.".to_vec();
    if actor.is_null() {
        out.extend_from_slice(b"<missing>");
    } else {
        out.extend_from_slice(&unsafe { super::trace_services::object_name(services, actor) });
    }
    out
}

fn class_path(services: &svc::Hp2TraceReporterServices, actor: *mut c_void) -> Vec<u8> {
    let class = if actor.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (services.object_class)(actor) }
    };
    if class.is_null() {
        b"<missing>".to_vec()
    } else {
        unsafe { object_path(services, class) }
    }
}

fn c_str_or(ptr: *const c_char, default_: &[u8]) -> Vec<u8> {
    if ptr.is_null() {
        default_.to_vec()
    } else {
        unsafe { ansi(ptr) }
    }
}

/// `BeginDispatch`.
///
/// # Safety
/// Single game thread; `actor` null or live; `relation`/`root_admission`
/// null or a live NUL-terminated C string.
pub unsafe fn begin_dispatch(
    actor: *mut c_void,
    relation: *const c_char,
    root_admission: *const c_char,
) -> u64 {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return TOKEN_LIMIT_REACHED;
    };
    if state.events.len() >= state.limit {
        state.truncated = true;
        return TOKEN_LIMIT_REACHED;
    }
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return TOKEN_LIMIT_REACHED;
    };
    let mut event = Event::new();
    event.actor = actor;
    event.frame = state.frame;
    event.ordinal = state.next_ordinal;
    state.next_ordinal += 1;
    event.rng_before = unsafe { (services.rand_trace_ordinal)() };
    event.path = actor_path(&services, actor);
    event.class = class_path(&services, actor);
    event.relation = c_str_or(relation, b"direct");
    event.root_admission = c_str_or(root_admission, b"dynamic");
    state.events.push(event);
    let token = (state.events.len() - 1) as u64;
    state.active_dispatches.push(token);
    token
}

/// `EventTick`.
///
/// # Safety
/// Single game thread.
pub unsafe fn event_tick(token: u64) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() }
        && (token as usize) < state.events.len()
    {
        state.events[token as usize].tick_outcome = b"dispatched".to_vec();
    }
}

/// `ProcessState`.
///
/// # Safety
/// Single game thread; `actor`/`state_object` null or live; `outcome` null
/// or a live NUL-terminated C string.
pub unsafe fn process_state(actor: *mut c_void, state_object: *mut c_void, outcome: *const c_char) {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return;
    };
    for &token in state.active_dispatches.iter().rev() {
        if (token as usize) < state.events.len() && state.events[token as usize].actor == actor {
            let identity = if state_object.is_null() {
                b"None".to_vec()
            } else {
                unsafe { object_path(&services, state_object) }
            };
            let event = &mut state.events[token as usize];
            event.process_state_identity = identity;
            event.process_state_outcome = c_str_or(outcome, b"not_called");
            return;
        }
    }
}

/// `EndDispatch`.
///
/// # Safety
/// Single game thread; `tick_outcome` null or a live NUL-terminated C
/// string.
pub unsafe fn end_dispatch(token: u64, tick_outcome: *const c_char) {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return;
    };
    if !state.active_dispatches.is_empty() {
        state.active_dispatches.pop();
    }
    if (token as usize) >= state.events.len() {
        return;
    }
    let event = &mut state.events[token as usize];
    event.tick_outcome = c_str_or(tick_outcome, b"skip_no_event");
    event.tick_stamp = if event.actor.is_null() {
        0
    } else {
        unsafe { (services.actor_ticked)(event.actor) as u64 }
    };
    event.rng_after = unsafe { (services.rand_trace_ordinal)() };
}

fn serialize_event(event: &Event, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"frame\":");
    out.extend_from_slice(event.frame.to_string().as_bytes());
    out.extend_from_slice(b",\"ordinal\":");
    out.extend_from_slice(event.ordinal.to_string().as_bytes());
    out.extend_from_slice(b",\"actor\":{\"path\":");
    svc::push_json_string(&event.path, out);
    out.extend_from_slice(b",\"class\":");
    svc::push_json_string(&event.class, out);
    out.extend_from_slice(b"},\"relation\":");
    svc::push_json_string(&event.relation, out);
    out.extend_from_slice(b",\"root_admission\":");
    svc::push_json_string(&event.root_admission, out);
    out.extend_from_slice(b",\"rng\":{\"before\":");
    out.extend_from_slice(event.rng_before.to_string().as_bytes());
    out.extend_from_slice(b",\"after\":");
    out.extend_from_slice(event.rng_after.to_string().as_bytes());
    out.extend_from_slice(b"},\"tick\":{\"identity\":\"Tick\",\"outcome\":");
    svc::push_json_string(&event.tick_outcome, out);
    out.extend_from_slice(b"},\"process_state\":{\"identity\":");
    svc::push_json_string(&event.process_state_identity, out);
    out.extend_from_slice(b",\"outcome\":");
    svc::push_json_string(&event.process_state_outcome, out);
    out.extend_from_slice(b"},\"tick_stamp\":");
    out.extend_from_slice(event.tick_stamp.to_string().as_bytes());
    out.push(b'}');
}

fn serialize(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"version\":2,\"enabled\":true,\"limit\":");
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
            let _ = out.write_all(b"<HP2_GLOBAL_TICK_TRACE> report=");
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
            let _ = err.write_all(b"<HP2_GLOBAL_TICK_TRACE> report_error=");
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

    // Ground truth: `GlobalTickTraceReporter::ParseLimit` (SDLLaunch.cpp,
    // deleted, formerly ~4637-4644): default 16384, clamp 262144.
    #[test]
    fn parse_limit_matches_cpp_cases() {
        assert_eq!(parse_limit(None), 16384);
        assert_eq!(parse_limit(Some(b"".to_vec())), 16384);
        assert_eq!(parse_limit(Some(b"zz".to_vec())), 16384);
        assert_eq!(parse_limit(Some(b"1".to_vec())), 1);
        assert_eq!(parse_limit(Some(b"262144".to_vec())), 262144);
        assert_eq!(parse_limit(Some(b"999999999".to_vec())), 262144);
    }

    // Ground truth: `GlobalTickTraceReporter::SerializeEvent` (SDLLaunch.cpp,
    // deleted, formerly ~4677-4690): every string field is unconditionally
    // quoted (no `JsonStringOrNull`/`Has*` gating), unlike the other four
    // reporters.
    #[test]
    fn serialize_event_matches_cpp_serialize_event() {
        let event = Event {
            actor: std::ptr::null_mut(),
            frame: 9,
            ordinal: 3,
            tick_stamp: 42,
            rng_before: 100,
            rng_after: 101,
            path: b"<map>.Creature_0".to_vec(),
            class: b"Package.CreatureClass".to_vec(),
            relation: b"root".to_vec(),
            root_admission: b"admitted".to_vec(),
            tick_outcome: b"ran".to_vec(),
            process_state_identity: b"State_Idle".to_vec(),
            process_state_outcome: b"entered".to_vec(),
        };
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        assert_eq!(
            out,
            b"{\"frame\":9,\"ordinal\":3,\"actor\":{\"path\":\"<map>.Creature_0\",\"class\":\"Package.CreatureClass\"},\"relation\":\"root\",\"root_admission\":\"admitted\",\"rng\":{\"before\":100,\"after\":101},\"tick\":{\"identity\":\"Tick\",\"outcome\":\"ran\"},\"process_state\":{\"identity\":\"State_Idle\",\"outcome\":\"entered\"},\"tick_stamp\":42}"
                .to_vec()
        );
    }

    // Ground truth: `Event`'s defaults for a token that was never followed
    // by `EventTick`/`ProcessState`/`EndDispatch` with an outcome (the
    // `Event::new()` defaults mirror `BeginDispatch`'s initial values).
    #[test]
    fn serialize_event_default_outcomes_match_begin_dispatch_defaults() {
        let event = Event::new();
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"outcome\":\"skip_no_event\""));
        assert!(text.contains("\"identity\":\"None\""));
        assert!(text.contains("\"outcome\":\"not_called\""));
    }

    // Ground truth: `GlobalTickTraceReporter::Serialize` on an empty
    // reporter (SDLLaunch.cpp, deleted, formerly ~4694-4703).
    #[test]
    fn serialize_of_an_empty_state_matches_cpp_serialize() {
        let state = State {
            report_path: Vec::new(),
            limit: 16384,
            frame: 0,
            next_ordinal: 0,
            truncated: false,
            events: Vec::new(),
            active_dispatches: Vec::new(),
        };
        assert_eq!(
            serialize(&state),
            b"{\"version\":2,\"enabled\":true,\"limit\":16384,\"count\":0,\"truncated\":false,\"events\":[]}\n"
                .to_vec()
        );
    }
}
