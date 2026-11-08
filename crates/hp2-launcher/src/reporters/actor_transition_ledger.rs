//! Port of `ActorTransitionLedger`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 4164-4451). A fixed-shape, bounded diagnostic artifact: each owner's
//! `first_observed_actor` records its lifecycle/spawn/renderer observation,
//! not `ULevel::Actors` serialized membership, and the ledger never
//! participates in the observed control flow. Gated by
//! `HP2_ACTOR_TRANSITION_LEDGER`.

use std::ffi::c_void;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use super::trace_services::{
    self as svc, Hp2TraceReporterServices, object_name, object_path, push_json_bool,
    push_json_string, push_json_string_or_null, read_ptr_at, write_report,
};

const DEFAULT_LIMIT: u64 = 4096;
const MAX_LIMIT: u64 = 65536;

#[derive(Default, Clone)]
struct Slot {
    present: bool,
    path: Vec<u8>,
    class: Vec<u8>,
}

#[derive(Default, Clone)]
struct Snapshot {
    /// `Snapshot::Set` (SDLLaunch.cpp ~4258): written by `MakeSnapshot` but
    /// never read by `SnapshotJson`/`SerializeOwner` in the original either;
    /// kept for fidelity, not functional.
    #[allow(dead_code)]
    set: bool,
    active_slot: Slot,
    delete_marked: bool,
    pending_kill: bool,
    effective_shadow_class: Vec<u8>,
    shadow_class_cdo: Vec<u8>,
}

#[derive(Default, Clone)]
struct Owner {
    identity: Vec<u8>,
    first_observed_actor: Slot,
    post_init_execution: Snapshot,
    pre_begin_before: Snapshot,
    pre_begin_after: Snapshot,
    spawn_requested: bool,
    spawn_class: Vec<u8>,
    spawn_name: Vec<u8>,
    spawn_owner: Vec<u8>,
    spawn_result_set: bool,
    spawn_result: Slot,
    spawn_published_set: bool,
    spawn_published: Slot,
    shadow_post_continuation: Vec<u8>,
    renderer_candidate_set: bool,
    renderer_candidate: Vec<u8>,
}

struct State {
    report_path: Vec<u8>,
    limit: usize,
    truncated: bool,
    owners: Vec<Owner>,
}

static mut STATE: Option<State> = None;

enum SnapshotField {
    PostInitExecution,
    PreBeginBefore,
    PreBeginAfter,
}

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

fn is_enabled() -> bool {
    env_bytes("HP2_ACTOR_TRANSITION_LEDGER").is_some_and(|v| !v.is_empty())
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
        report_path: env_bytes("HP2_ACTOR_TRANSITION_LEDGER").unwrap_or_default(),
        limit: parse_limit(env_bytes("HP2_ACTOR_TRANSITION_LEDGER_LIMIT")),
        truncated: false,
        owners: Vec::new(),
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

fn make_slot(services: &Hp2TraceReporterServices, actor: *mut c_void) -> Slot {
    if actor.is_null() {
        return Slot::default();
    }
    let class = unsafe { (services.object_class)(actor) };
    Slot {
        present: true,
        path: unsafe { object_path(services, actor) },
        class: unsafe { object_path(services, class) },
    }
}

fn active_slot(services: &Hp2TraceReporterServices, actor: *mut c_void) -> i32 {
    if actor.is_null() {
        return -1;
    }
    let level = unsafe { (services.actor_x_level)(actor) };
    if level.is_null() {
        return -1;
    }
    let count = unsafe { (services.level_actor_count)(level) };
    for slot in 0..count {
        if unsafe { (services.level_actor_at)(level, slot) } == actor {
            return slot;
        }
    }
    -1
}

/// `ShadowClassDefiner`.
fn shadow_class_definer(services: &Hp2TraceReporterServices, actor: *mut c_void) -> *mut c_void {
    if actor.is_null() {
        return std::ptr::null_mut();
    }
    let class = unsafe { (services.object_class)(actor) };
    let property = unsafe {
        (services.find_property)(class, c"ShadowClass".as_ptr())
    };
    if property.is_null() {
        return std::ptr::null_mut();
    }
    let outer = unsafe { (services.property_outer)(property) };
    unsafe { (services.cast_to_class)(outer) }
}

fn make_snapshot(services: &Hp2TraceReporterServices, actor: *mut c_void) -> Snapshot {
    if actor.is_null() {
        return Snapshot::default();
    }
    let class = unsafe { (services.object_class)(actor) };
    let property = unsafe { (services.find_property)(class, c"ShadowClass".as_ptr()) };
    let shadow_class = if property.is_null() {
        std::ptr::null_mut()
    } else {
        let offset = unsafe { (services.property_offset)(property) };
        unsafe { read_ptr_at(actor, offset) }
    };
    let definer = shadow_class_definer(services, actor);
    let shadow_class_cdo = if definer.is_null() {
        Vec::new()
    } else {
        let cdo = unsafe { (services.class_default_actor)(definer) };
        unsafe { object_path(services, cdo) }
    };
    Snapshot {
        set: true,
        active_slot: make_slot(services, actor),
        delete_marked: unsafe { (services.actor_delete_marked)(actor) } != 0,
        pending_kill: unsafe { (services.actor_pending_kill)(actor) } != 0,
        effective_shadow_class: unsafe { object_path(services, shadow_class) },
        shadow_class_cdo,
    }
}

/// `FindOrAdd`.
fn find_or_add(services: &Hp2TraceReporterServices, state: &mut State, actor: *mut c_void) -> Option<usize> {
    if actor.is_null() {
        return None;
    }
    let mut identity = b"<map>.".to_vec();
    identity.extend_from_slice(&unsafe { object_name(services, actor) });
    // Dead code in the original C++ (`SDLLaunch.cpp` `ActorTransitionLedger::
    // FindOrAdd`): `Identity` is always `"<map>." + name`, so it is never
    // empty and always contains a '.'; this check can never return `None`
    // here. Preserved rather than "fixed" per the port's bit-exact-behaviour
    // rule.
    if identity.is_empty() || !identity.contains(&b'.') {
        return None;
    }
    let actor_path = unsafe { object_path(services, actor) };
    for (index, owner) in state.owners.iter().enumerate() {
        if owner.identity == identity {
            if owner.first_observed_actor.path == actor_path {
                return Some(index);
            }
            let slot = active_slot(services, actor);
            identity.extend_from_slice(b"#");
            identity.extend_from_slice(slot.to_string().as_bytes());
            break;
        }
    }
    if let Some(index) = state
        .owners
        .iter()
        .position(|o| o.identity == identity && o.first_observed_actor.path == actor_path)
    {
        return Some(index);
    }
    if state.owners.len() >= state.limit {
        state.truncated = true;
        return None;
    }
    state.owners.push(Owner {
        identity,
        first_observed_actor: make_slot(services, actor),
        ..Owner::default()
    });
    Some(state.owners.len() - 1)
}

fn set_snapshot(
    services: &Hp2TraceReporterServices,
    state: &mut State,
    actor: *mut c_void,
    field: SnapshotField,
) {
    let Some(index) = find_or_add(services, state, actor) else {
        return;
    };
    let snapshot = make_snapshot(services, actor);
    let owner = &mut state.owners[index];
    match field {
        SnapshotField::PostInitExecution => owner.post_init_execution = snapshot,
        SnapshotField::PreBeginBefore => owner.pre_begin_before = snapshot,
        SnapshotField::PreBeginAfter => owner.pre_begin_after = snapshot,
    }
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

/// # Safety
/// Single game thread; `services` published; `actor` is null or live.
pub unsafe fn post_init_execution(actor: *mut c_void) {
    with_state(|s, state| set_snapshot(s, state, actor, SnapshotField::PostInitExecution));
}

/// # Safety
/// Same as [`post_init_execution`].
pub unsafe fn pre_begin_before(actor: *mut c_void) {
    with_state(|s, state| set_snapshot(s, state, actor, SnapshotField::PreBeginBefore));
}

/// # Safety
/// Same as [`post_init_execution`].
pub unsafe fn pre_begin_after(actor: *mut c_void) {
    with_state(|s, state| {
        set_snapshot(s, state, actor, SnapshotField::PreBeginAfter);
        if let Some(index) = find_or_add(s, state, actor) {
            let shadow = if actor.is_null() {
                std::ptr::null_mut()
            } else {
                unsafe { (s.actor_shadow)(actor) }
            };
            state.owners[index].shadow_post_continuation = unsafe { object_path(s, shadow) };
        }
    });
}

/// `SpawnRequest`.
///
/// # Safety
/// Same as [`post_init_execution`]; `class` is null or a live `UClass*`;
/// `name_index` is a raw `FName` index.
pub unsafe fn spawn_request(class: *mut c_void, name_index: i32, actor: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = find_or_add(s, state, actor) else {
            return;
        };
        let owner = &mut state.owners[index];
        if owner.spawn_requested {
            return;
        }
        owner.spawn_requested = true;
        owner.spawn_class = unsafe { object_path(s, class) };
        owner.spawn_name = unsafe { svc::ansi((s.fname_ansi_or_empty)(name_index)) };
        owner.spawn_owner = unsafe { object_path(s, actor) };
    });
}

/// # Safety
/// Same as [`post_init_execution`]; `child` is null or live.
pub unsafe fn spawn_result(owner_actor: *mut c_void, child: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = find_or_add(s, state, owner_actor) else {
            return;
        };
        let owner = &mut state.owners[index];
        if owner.spawn_result_set {
            return;
        }
        owner.spawn_result_set = true;
        owner.spawn_result = make_slot(s, child);
    });
}

/// # Safety
/// Same as [`spawn_result`].
pub unsafe fn spawn_published(owner_actor: *mut c_void, child: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = find_or_add(s, state, owner_actor) else {
            return;
        };
        let owner = &mut state.owners[index];
        if owner.spawn_published_set {
            return;
        }
        owner.spawn_published_set = true;
        owner.spawn_published = make_slot(s, child);
    });
}

/// # Safety
/// Same as [`spawn_result`].
pub unsafe fn first_renderer_candidate(owner_actor: *mut c_void, candidate: *mut c_void) {
    with_state(|s, state| {
        let Some(index) = find_or_add(s, state, owner_actor) else {
            return;
        };
        let owner = &mut state.owners[index];
        if owner.renderer_candidate_set {
            return;
        }
        owner.renderer_candidate_set = true;
        owner.renderer_candidate = unsafe { object_path(s, candidate) };
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

fn push_snapshot_json(snapshot: &Snapshot, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"active_slot\":");
    push_slot_json(&snapshot.active_slot, out);
    out.extend_from_slice(b",\"delete_marked\":");
    push_json_bool(snapshot.delete_marked, out);
    out.extend_from_slice(b",\"pending_kill\":");
    push_json_bool(snapshot.pending_kill, out);
    out.extend_from_slice(b",\"effective_shadow_class\":");
    push_json_string_or_null(&snapshot.effective_shadow_class, out);
    out.extend_from_slice(b",\"shadow_class_cdo\":");
    push_json_string_or_null(&snapshot.shadow_class_cdo, out);
    out.push(b'}');
}

fn serialize_owner(owner: &Owner, out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"identity\":");
    push_json_string(&owner.identity, out);
    out.extend_from_slice(b",\"first_observed_actor\":");
    push_slot_json(&owner.first_observed_actor, out);
    out.extend_from_slice(b",\"post_init_execution\":");
    push_snapshot_json(&owner.post_init_execution, out);
    out.extend_from_slice(b",\"pre_begin_before\":");
    push_snapshot_json(&owner.pre_begin_before, out);
    out.extend_from_slice(b",\"pre_begin_after\":");
    push_snapshot_json(&owner.pre_begin_after, out);
    out.extend_from_slice(b",\"spawn\":{\"request\":");
    if owner.spawn_requested {
        out.extend_from_slice(b"{\"class\":");
        push_json_string_or_null(&owner.spawn_class, out);
        out.extend_from_slice(b",\"name\":");
        push_json_string_or_null(&owner.spawn_name, out);
        out.extend_from_slice(b",\"owner\":");
        push_json_string_or_null(&owner.spawn_owner, out);
        out.push(b'}');
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"result\":");
    if owner.spawn_result_set {
        push_slot_json(&owner.spawn_result, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"published_child_slot\":");
    if owner.spawn_published_set {
        push_slot_json(&owner.spawn_published, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.push(b'}');
    out.extend_from_slice(b",\"shadow_post_continuation\":");
    push_json_string_or_null(&owner.shadow_post_continuation, out);
    out.extend_from_slice(b",\"first_renderer_candidate\":");
    push_json_string_or_null(&owner.renderer_candidate, out);
    out.push(b'}');
}

fn serialize(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"version\":2,\"enabled\":true,\"limit\":");
    out.extend_from_slice(state.limit.to_string().as_bytes());
    out.extend_from_slice(b",\"count\":");
    out.extend_from_slice(state.owners.len().to_string().as_bytes());
    out.extend_from_slice(b",\"truncated\":");
    push_json_bool(state.truncated, &mut out);
    out.extend_from_slice(b",\"owners\":[");
    for (index, owner) in state.owners.iter().enumerate() {
        if index != 0 {
            out.push(b',');
        }
        serialize_owner(owner, &mut out);
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
            let _ = out.write_all(b"<HP2_ACTOR_TRANSITION_LEDGER> report=");
            let _ = out.write_all(&state.report_path);
            let _ = out.write_all(
                format!(
                    " owners={} truncated={}\n",
                    state.owners.len(),
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
            let _ = err.write_all(b"<HP2_ACTOR_TRANSITION_LEDGER> report_error=");
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

    // Ground truth: `ActorTransitionLedger::ParseLimit` (SDLLaunch.cpp,
    // deleted, formerly ~4080-4088): same shape as the shadow-admission
    // reporter's, but clamped to 65536, not 1000000.
    #[test]
    fn parse_limit_matches_cpp_cases() {
        assert_eq!(parse_limit(None), 4096);
        assert_eq!(parse_limit(Some(b"".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"nope".to_vec())), 4096);
        assert_eq!(parse_limit(Some(b"7".to_vec())), 7);
        assert_eq!(parse_limit(Some(b"65536".to_vec())), 65536);
        assert_eq!(parse_limit(Some(b"999999".to_vec())), 65536);
    }

    // Ground truth: `ActorTransitionLedger::SlotJson` (SDLLaunch.cpp,
    // deleted, formerly ~4163-4167): an absent slot is `{"path":null,
    // "class":null}` regardless of any stale `Path`/`Class` strings.
    #[test]
    fn push_slot_json_absent_ignores_stale_strings() {
        let slot = Slot {
            present: false,
            path: b"stale".to_vec(),
            class: b"stale".to_vec(),
        };
        let mut out = Vec::new();
        push_slot_json(&slot, &mut out);
        assert_eq!(out, b"{\"path\":null,\"class\":null}");
    }

    #[test]
    fn push_slot_json_present_quotes_both_fields() {
        let slot = Slot {
            present: true,
            path: b"Package.Actor".to_vec(),
            class: b"Package.Class".to_vec(),
        };
        let mut out = Vec::new();
        push_slot_json(&slot, &mut out);
        assert_eq!(out, b"{\"path\":\"Package.Actor\",\"class\":\"Package.Class\"}");
    }

    // Ground truth: `SerializeOwner` on a freshly-added owner (SDLLaunch.cpp,
    // deleted, formerly ~4216-4231): every `Snapshot` field defaults absent
    // (`SnapshotJson` on a default-constructed `Snapshot`), `spawn.request`
    // is `null` until `SpawnRequested`, `result`/`published_child_slot` are
    // `null` until their `*Set` flags are set.
    #[test]
    fn serialize_owner_of_a_freshly_added_owner_matches_cpp_serialize_owner() {
        let owner = Owner {
            identity: b"<map>.Foo".to_vec(),
            first_observed_actor: Slot {
                present: true,
                path: b"Package.Foo".to_vec(),
                class: b"Package.FooClass".to_vec(),
            },
            ..Owner::default()
        };
        let mut out = Vec::new();
        serialize_owner(&owner, &mut out);
        let empty_snapshot = "{\"active_slot\":{\"path\":null,\"class\":null},\"delete_marked\":false,\"pending_kill\":false,\"effective_shadow_class\":null,\"shadow_class_cdo\":null}";
        let expected = format!(
            "{{\"identity\":\"<map>.Foo\",\"first_observed_actor\":{{\"path\":\"Package.Foo\",\"class\":\"Package.FooClass\"}},\"post_init_execution\":{empty_snapshot},\"pre_begin_before\":{empty_snapshot},\"pre_begin_after\":{empty_snapshot},\"spawn\":{{\"request\":null,\"result\":null,\"published_child_slot\":null}},\"shadow_post_continuation\":null,\"first_renderer_candidate\":null}}"
        );
        assert_eq!(out, expected.into_bytes());
    }

    #[test]
    fn serialize_owner_with_a_spawn_request_and_published_child() {
        let owner = Owner {
            identity: b"<map>.Bar".to_vec(),
            first_observed_actor: Slot {
                present: true,
                path: b"Package.Bar".to_vec(),
                class: b"Package.BarClass".to_vec(),
            },
            spawn_requested: true,
            spawn_class: b"Package.SpawnClass".to_vec(),
            spawn_name: Vec::new(),
            spawn_owner: b"Package.Bar".to_vec(),
            spawn_published_set: true,
            spawn_published: Slot {
                present: true,
                path: b"Package.Child".to_vec(),
                class: b"Package.ChildClass".to_vec(),
            },
            ..Owner::default()
        };
        let mut out = Vec::new();
        serialize_owner(&owner, &mut out);
        let text = String::from_utf8(out).unwrap();
        // `SpawnName` is `Name == NAME_None ? "" : Text(*Name)`, and an empty
        // string renders through `JsonStringOrNull` as `null`.
        assert!(text.contains(
            "\"spawn\":{\"request\":{\"class\":\"Package.SpawnClass\",\"name\":null,\"owner\":\"Package.Bar\"},\"result\":null,\"published_child_slot\":{\"path\":\"Package.Child\",\"class\":\"Package.ChildClass\"}}"
        ));
    }

    // Ground truth: `ActorTransitionLedger::Serialize` on an empty ledger
    // (SDLLaunch.cpp, deleted, formerly ~4236-4245).
    #[test]
    fn serialize_of_an_empty_state_matches_cpp_serialize() {
        let state = State {
            report_path: Vec::new(),
            limit: 4096,
            truncated: false,
            owners: Vec::new(),
        };
        assert_eq!(
            serialize(&state),
            b"{\"version\":2,\"enabled\":true,\"limit\":4096,\"count\":0,\"truncated\":false,\"owners\":[]}\n"
                .to_vec()
        );
    }
}
