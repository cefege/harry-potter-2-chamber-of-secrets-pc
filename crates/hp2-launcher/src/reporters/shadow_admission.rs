//! Port of `ShadowAdmissionTraceReporter`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 3966-4158). Bounded, atomically-flushed JSON trace of per-pass shadow
//! admission decisions, gated by `HP2_SHADOW_ADMISSION_TRACE`.

use std::ffi::c_void;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

const DEFAULT_LIMIT: u64 = 4096;
const MAX_LIMIT: u64 = 1_000_000;

struct Event {
    frame: u64,
    pass: i32,
    shadow_path: Vec<u8>,
    owner_path: Vec<u8>,
    viewport_actor_path: Vec<u8>,
    view_target_path: Vec<u8>,
    recursion_parent_path: Vec<u8>,
    candidate_status: Vec<u8>,
    has_shadow: bool,
    has_owner: bool,
    has_viewport_actor: bool,
    has_view_target: bool,
    has_recursion_parent: bool,
    behind_view: bool,
    perspective: bool,
    world_dynamics: bool,
    update_eligible: bool,
}

struct State {
    report_path: Vec<u8>,
    limit: usize,
    frame: u64,
    next_pass: i32,
    truncated: bool,
    events: Vec<Event>,
}

static mut STATE: Option<State> = None;

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

fn is_enabled() -> bool {
    env_bytes("HP2_SHADOW_ADMISSION_TRACE").is_some_and(|v| !v.is_empty())
}

/// `ParseLimit`: missing/empty -> 4096; a value that fails to parse fully as
/// an unsigned decimal integer -> 4096; otherwise `min(Parsed, 1000000)`.
fn parse_limit(value: Option<Vec<u8>>) -> usize {
    let bytes = match value {
        Some(bytes) if !bytes.is_empty() => bytes,
        _ => return DEFAULT_LIMIT as usize,
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(_) => return DEFAULT_LIMIT as usize,
    };
    match text.parse::<u64>() {
        Ok(parsed) => parsed.min(MAX_LIMIT) as usize,
        Err(_) => DEFAULT_LIMIT as usize,
    }
}

/// `ShadowAdmissionTraceReporter::IsEnabled()` and constructor, called
/// unconditionally; a no-op (state stays absent) when the trace is disabled.
///
/// # Safety
/// Single game thread; no concurrent reporter call.
pub unsafe fn create() {
    if !is_enabled() {
        return;
    }
    let state = State {
        report_path: env_bytes("HP2_SHADOW_ADMISSION_TRACE").unwrap_or_default(),
        limit: parse_limit(env_bytes("HP2_SHADOW_ADMISSION_TRACE_LIMIT")),
        frame: 0,
        next_pass: 0,
        truncated: false,
        events: Vec::new(),
    };
    unsafe { (&raw mut STATE).write(Some(state)) };
}

/// Drops the reporter state, matching `ClearTraceReporterGlobals`'s
/// `GShadowAdmissionTraceReporter = NULL`.
///
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

/// `SetFrame`.
///
/// # Safety
/// Single game thread.
pub unsafe fn set_frame(frame: u64) {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        state.frame = frame;
        state.next_pass = 0;
    }
}

/// `BeginPass`.
///
/// # Safety
/// Single game thread.
pub unsafe fn begin_pass() -> i32 {
    if let Some(state) = unsafe { (&raw mut STATE).as_mut().unwrap() } {
        let pass = state.next_pass;
        state.next_pass += 1;
        pass
    } else {
        0
    }
}

/// `Record`.
///
/// # Safety
/// Single game thread; `services` is published; every `*mut c_void` is null
/// or a live engine object pointer; `candidate_status` is null or a live
/// NUL-terminated C string.
#[allow(clippy::too_many_arguments)]
pub unsafe fn record(
    pass: i32,
    owner: *mut c_void,
    shadow: *mut c_void,
    candidate_status: *const std::ffi::c_char,
    update_eligible: i32,
    viewport_actor: *mut c_void,
    view_target: *mut c_void,
    behind_view: i32,
    recursion_parent_actor: *mut c_void,
    perspective: i32,
    world_dynamics: i32,
) {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    if state.events.len() >= state.limit {
        state.truncated = true;
        return;
    }
    let svc = match unsafe { super::trace_services::published_services() } {
        Ok(svc) => svc,
        Err(_) => return,
    };
    let candidate_status = unsafe { super::trace_services::ansi(candidate_status) };
    let event = Event {
        frame: state.frame,
        pass,
        shadow_path: unsafe { super::trace_services::object_path(&svc, shadow) },
        owner_path: unsafe { super::trace_services::object_path(&svc, owner) },
        viewport_actor_path: unsafe { super::trace_services::object_path(&svc, viewport_actor) },
        view_target_path: unsafe { super::trace_services::object_path(&svc, view_target) },
        recursion_parent_path: unsafe {
            super::trace_services::object_path(&svc, recursion_parent_actor)
        },
        candidate_status,
        has_shadow: !shadow.is_null(),
        has_owner: !owner.is_null(),
        has_viewport_actor: !viewport_actor.is_null(),
        has_view_target: !view_target.is_null(),
        has_recursion_parent: !recursion_parent_actor.is_null(),
        behind_view: behind_view != 0,
        perspective: perspective != 0,
        world_dynamics: world_dynamics != 0,
        update_eligible: update_eligible != 0,
    };
    state.events.push(event);
}

fn push_quoted_or_null(has: bool, value: &[u8], out: &mut Vec<u8>) {
    if has {
        super::trace_services::push_json_string(value, out);
    } else {
        out.extend_from_slice(b"null");
    }
}

fn serialize_event(event: &Event, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"frame\":");
    out.extend_from_slice(event.frame.to_string().as_bytes());
    out.extend_from_slice(b",\"pass\":");
    out.extend_from_slice(event.pass.to_string().as_bytes());
    out.extend_from_slice(b",\"shadow_path\":");
    push_quoted_or_null(event.has_shadow, &event.shadow_path, out);
    out.extend_from_slice(b",\"owner_path\":");
    push_quoted_or_null(event.has_owner, &event.owner_path, out);
    out.extend_from_slice(b",\"candidate_status\":");
    super::trace_services::push_json_string(&event.candidate_status, out);
    out.extend_from_slice(b",\"raw_facts\":{\"viewport_actor_path\":");
    push_quoted_or_null(event.has_viewport_actor, &event.viewport_actor_path, out);
    out.extend_from_slice(b",\"view_target_path\":");
    push_quoted_or_null(event.has_view_target, &event.view_target_path, out);
    out.extend_from_slice(b",\"behind_view\":");
    super::trace_services::push_json_bool(event.behind_view, out);
    out.extend_from_slice(b",\"recursion_parent_path\":");
    push_quoted_or_null(
        event.has_recursion_parent,
        &event.recursion_parent_path,
        out,
    );
    out.extend_from_slice(b",\"perspective\":");
    super::trace_services::push_json_bool(event.perspective, out);
    out.extend_from_slice(b",\"world_dynamics\":");
    super::trace_services::push_json_bool(event.world_dynamics, out);
    out.extend_from_slice(b"},\"update_eligible\":");
    super::trace_services::push_json_bool(event.update_eligible, out);
    out.push(b'}');
}

fn serialize(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"version\":1,\"enabled\":true,\"limit\":");
    out.extend_from_slice(state.limit.to_string().as_bytes());
    out.extend_from_slice(b",\"count\":");
    out.extend_from_slice(state.events.len().to_string().as_bytes());
    out.extend_from_slice(b",\"truncated\":");
    super::trace_services::push_json_bool(state.truncated, &mut out);
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

/// `Flush`: atomic tmp-write-rename, with the exact `<HP2_SHADOW_ADMISSION_TRACE>`
/// stdout/stderr lines on success/failure.
///
/// # Safety
/// Single game thread.
pub unsafe fn flush() {
    let Some(state) = (unsafe { (&raw mut STATE).as_mut().unwrap() }) else {
        return;
    };
    let document = serialize(state);
    match super::trace_services::write_report(&state.report_path, &document) {
        Ok(()) => {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(b"<HP2_SHADOW_ADMISSION_TRACE> report=");
            let _ = out.write_all(&state.report_path);
            let _ = out.write_all(format!(" events={} truncated={}\n", state.events.len(), i32::from(state.truncated)).as_bytes());
        }
        Err(kind) => {
            let reason: &[u8] = match kind {
                super::trace_services::ReportWriteError::Unwritable => b"unwritable",
                super::trace_services::ReportWriteError::WriteFailed => b"write_failed",
            };
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(b"<HP2_SHADOW_ADMISSION_TRACE> report_error=");
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

    // Ground truth: `ShadowAdmissionTraceReporter::ParseLimit`
    // (SDLLaunch.cpp, deleted, formerly ~3879-3887): `if( !Value || !Value[0]
    // ) return DefaultLimit;` ... `return End && !*End ? (size_t)Min(Parsed,
    // 1000000) : DefaultLimit;` — missing/empty/non-fully-numeric -> 4096,
    // else `min(parsed, 1000000)`.
    #[test]
    fn parse_limit_matches_cpp_cases() {
        assert_eq!(parse_limit(None), 4096);
        assert_eq!(parse_limit(Some(b"".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"not_a_number".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"12abc".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"0".to_vec())), 0);
        assert_eq!(parse_limit(Some(b"42".to_vec())), 42);
        assert_eq!(parse_limit(Some(b"1000000".to_vec())), 1_000_000);
        assert_eq!(parse_limit(Some(b"2000000".to_vec())), 1_000_000);
    }

    fn empty_event() -> Event {
        Event {
            frame: 0,
            pass: 0,
            shadow_path: Vec::new(),
            owner_path: Vec::new(),
            viewport_actor_path: Vec::new(),
            view_target_path: Vec::new(),
            recursion_parent_path: Vec::new(),
            candidate_status: Vec::new(),
            has_shadow: false,
            has_owner: false,
            has_viewport_actor: false,
            has_view_target: false,
            has_recursion_parent: false,
            behind_view: false,
            perspective: false,
            world_dynamics: false,
            update_eligible: false,
        }
    }

    // Ground truth: `ShadowAdmissionTraceReporter::Serialize` on an empty
    // reporter (SDLLaunch.cpp, deleted, formerly ~3927-3936).
    #[test]
    fn serialize_of_an_empty_state_matches_cpp_serialize() {
        let state = State {
            report_path: Vec::new(),
            limit: 4096,
            frame: 0,
            next_pass: 0,
            truncated: false,
            events: Vec::new(),
        };
        assert_eq!(
            serialize(&state),
            b"{\"version\":1,\"enabled\":true,\"limit\":4096,\"count\":0,\"truncated\":false,\"events\":[]}\n"
                .to_vec()
        );
    }

    // Ground truth: `ShadowAdmissionTraceReporter::SerializeEvent`
    // (SDLLaunch.cpp, deleted, formerly ~3908-3925): absent pointers
    // (`Has*=false`) serialize their path field as `null`, not `""`; present
    // fields are quoted+escaped; `CandidateStatus` is always quoted
    // (`Has*`-independent).
    #[test]
    fn serialize_event_with_absent_pointers_emits_null_paths() {
        let mut event = empty_event();
        event.frame = 7;
        event.pass = 2;
        event.candidate_status = b"rejected".to_vec();
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        assert_eq!(
            out,
            b"{\"frame\":7,\"pass\":2,\"shadow_path\":null,\"owner_path\":null,\"candidate_status\":\"rejected\",\"raw_facts\":{\"viewport_actor_path\":null,\"view_target_path\":null,\"behind_view\":false,\"recursion_parent_path\":null,\"perspective\":false,\"world_dynamics\":false},\"update_eligible\":false}"
                .to_vec()
        );
    }

    #[test]
    fn serialize_event_with_present_pointers_and_escaping() {
        let mut event = empty_event();
        event.frame = 1;
        event.pass = 0;
        event.has_shadow = true;
        event.shadow_path = b"Package.Decal\"1".to_vec();
        event.has_owner = true;
        event.owner_path = b"Package.Owner".to_vec();
        event.candidate_status = b"ok".to_vec();
        event.has_viewport_actor = true;
        event.viewport_actor_path = b"Package.Viewport".to_vec();
        event.has_view_target = true;
        event.view_target_path = b"Package.Target".to_vec();
        event.behind_view = true;
        event.has_recursion_parent = true;
        event.recursion_parent_path = b"Package.Parent".to_vec();
        event.perspective = true;
        event.world_dynamics = true;
        event.update_eligible = true;
        let mut out = Vec::new();
        serialize_event(&event, &mut out);
        assert_eq!(
            out,
            b"{\"frame\":1,\"pass\":0,\"shadow_path\":\"Package.Decal\\\"1\",\"owner_path\":\"Package.Owner\",\"candidate_status\":\"ok\",\"raw_facts\":{\"viewport_actor_path\":\"Package.Viewport\",\"view_target_path\":\"Package.Target\",\"behind_view\":true,\"recursion_parent_path\":\"Package.Parent\",\"perspective\":true,\"world_dynamics\":true},\"update_eligible\":true}"
                .to_vec()
        );
    }

    // Ground truth: `Serialize`'s event loop joins with `,` and no trailing
    // comma (SDLLaunch.cpp, deleted, formerly ~3932-3936:
    // `if( It != Events.begin() ) Json += ",";`).
    #[test]
    fn serialize_joins_multiple_events_with_commas_and_reports_truncated() {
        let state = State {
            report_path: Vec::new(),
            limit: 1,
            frame: 3,
            next_pass: 2,
            truncated: true,
            events: vec![empty_event(), empty_event()],
        };
        let out = serialize(&state);
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("{\"version\":1,\"enabled\":true,\"limit\":1,\"count\":2,\"truncated\":true,\"events\":[{"));
        assert_eq!(text.matches("},{").count(), 1);
        assert!(text.ends_with("]}\n"));
    }
}
