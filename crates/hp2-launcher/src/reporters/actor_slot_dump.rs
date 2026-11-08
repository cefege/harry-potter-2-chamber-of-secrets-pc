//! Port of `ActorSlotDump`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 4925-5134). Unlike the other five reporters this one is a static,
//! always-present process singleton (the original C++ class has only
//! `static` members and no constructor); gated per-call on
//! `HP2_ACTOR_SLOT_DUMP` exactly like the original's per-method
//! `IsEnabled()` checks, not on an enabled/disabled instance lifecycle.
//! Called from three `FMainLoopPolicyFrame::Call` leaves
//! (`ActorSlotDumpEnabled`, `CapturePostStartup`, `WriteActorSlotDump`) and
//! the four `HP2ActorSlotDump*` raw-level-actor-table hooks forwarded from
//! `HarryPotter2/Unreal/Engine/Src/Hp2LevelServices.cpp`/`UnGame.cpp`'s weak
//! defaults.

use std::ffi::c_void;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use super::trace_services::{self as svc, Hp2TraceReporterServices, read_ptr_at, write_report};

const RAW_PREFIX: &[u8] = b"{\"checkpoint\":\"post_deserialize_raw\",\"sequence\":\"raw\",\"i_first_net_relevant_actor\":null,\"i_first_dynamic_actor\":null,\"actors\":[";

struct State {
    raw_post_deserialize: Vec<u8>,
    post_startup_before_first_tick: Vec<u8>,
    raw_level: *mut c_void,
    raw_actor_count: i32,
    raw_actor_table_open: bool,
}

static mut STATE: State = State {
    raw_post_deserialize: Vec::new(),
    post_startup_before_first_tick: Vec::new(),
    raw_level: std::ptr::null_mut(),
    raw_actor_count: 0,
    raw_actor_table_open: false,
};

fn env_bytes(name: &str) -> Option<Vec<u8>> {
    std::env::var_os(name).map(|v| v.as_bytes().to_vec())
}

/// `IsEnabled`.
pub fn enabled() -> bool {
    env_bytes("HP2_ACTOR_SLOT_DUMP").is_some_and(|v| !v.is_empty())
}

/// `CapturePostDeserialize`: inert in the original (the raw callback from
/// `ULevelBase::Serialize` is the only valid deserialization boundary; this
/// legacy `LoadMap` hook is intentionally dead), kept only as a documented
/// no-op so `HP2ActorSlotDumpPostDeserialize`'s signature/call site stay
/// stable.
pub fn capture_post_deserialize(_level: *mut c_void) {}

fn object_path_json(services: &Hp2TraceReporterServices, object: *mut c_void, out: &mut Vec<u8>) {
    if object.is_null() {
        out.extend_from_slice(b"null");
        return;
    }
    out.push(b'"');
    svc::escape_json(&unsafe { svc::object_path(services, object) }, out);
    out.push(b'"');
}

fn object_property(
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
    let field = unsafe { (services.find_property)(class, name) };
    if field.is_null() {
        return std::ptr::null_mut();
    }
    let offset = unsafe { (services.property_offset)(field) };
    unsafe { read_ptr_at(object, offset) }
}

/// `ActorJson`. `level` is `None` for the "no `Level`" default-argument call
/// (the raw-table path), `Some` for the rearranged `SerializeCheckpoint`
/// path.
fn actor_json(
    services: &Hp2TraceReporterServices,
    slot: i32,
    actor: *mut c_void,
    level: Option<*mut c_void>,
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(b"{\"slot\":");
    out.extend_from_slice(slot.to_string().as_bytes());
    if actor.is_null() {
        out.extend_from_slice(
            b",\"path\":null,\"class\":null,\"is_null\":true,\"is_deleted\":false,\"is_pending_kill\":false,\"game_relevant\":null,\"role\":null,\"remote_role\":null,\"owner_path\":null,\"owner_class\":null,\"effective_shadow_class\":null,\"spawn_origin\":null,\"spawn_owner_path\":null,\"spawn_owner_class\":null,\"spawn_request_id\":null,\"spawn_order\":null}",
        );
        return;
    }
    let owner = unsafe { (services.actor_owner)(actor) };
    let shadow_class = object_property(services, actor, c"ShadowClass".as_ptr());
    let owner_shadow_class_raw = object_property(services, owner, c"ShadowClass".as_ptr());
    let owner_shadow_class = unsafe { (services.cast_to_class)(owner_shadow_class_raw) };
    let class = unsafe { (services.object_class)(actor) };
    let first_dynamic_actor = level.map(|l| unsafe { (services.level_first_dynamic_actor)(l) });
    let is_dynamic_actor_shadow = level.is_some()
        && first_dynamic_actor.is_some_and(|first| slot >= first)
        && !owner_shadow_class.is_null()
        && unsafe { svc::class_is_child_of(services, class, owner_shadow_class) };
    out.extend_from_slice(b",\"path\":\"");
    svc::escape_json(&unsafe { svc::object_path(services, actor) }, out);
    out.extend_from_slice(b"\",\"class\":\"");
    svc::escape_json(&unsafe { svc::object_path(services, class) }, out);
    out.extend_from_slice(b"\",\"is_null\":false,\"is_deleted\":");
    svc::push_json_bool(unsafe { (services.actor_delete_marked)(actor) } != 0, out);
    out.extend_from_slice(b",\"is_pending_kill\":");
    svc::push_json_bool(unsafe { (services.actor_pending_kill)(actor) } != 0, out);
    out.extend_from_slice(b",\"game_relevant\":");
    svc::push_json_bool(unsafe { (services.actor_game_relevant)(actor) } != 0, out);
    out.extend_from_slice(b",\"role\":");
    out.extend_from_slice(unsafe { (services.actor_role)(actor) }.to_string().as_bytes());
    out.extend_from_slice(b",\"remote_role\":");
    out.extend_from_slice(
        unsafe { (services.actor_remote_role)(actor) }
            .to_string()
            .as_bytes(),
    );
    out.extend_from_slice(b",\"owner_path\":");
    object_path_json(services, owner, out);
    out.extend_from_slice(b",\"owner_class\":");
    let owner_class = if owner.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (services.object_class)(owner) }
    };
    object_path_json(services, owner_class, out);
    out.extend_from_slice(b",\"effective_shadow_class\":");
    object_path_json(services, shadow_class, out);
    out.extend_from_slice(b",\"spawn_origin\":");
    out.extend_from_slice(if is_dynamic_actor_shadow {
        b"\"level_dynamic_slot\""
    } else {
        b"null"
    });
    out.extend_from_slice(b",\"spawn_owner_path\":");
    if is_dynamic_actor_shadow {
        object_path_json(services, owner, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"spawn_owner_class\":");
    if is_dynamic_actor_shadow {
        object_path_json(services, owner_class, out);
    } else {
        out.extend_from_slice(b"null");
    }
    out.extend_from_slice(b",\"spawn_request_id\":null,\"spawn_order\":");
    if is_dynamic_actor_shadow {
        out.extend_from_slice((slot - first_dynamic_actor.unwrap()).to_string().as_bytes());
    } else {
        out.extend_from_slice(b"null");
    }
    out.push(b'}');
}

/// `RawActorJson`.
#[allow(clippy::too_many_arguments)]
fn raw_actor_json(
    services: &Hp2TraceReporterServices,
    slot: i32,
    actor: *mut c_void,
    reference_offset_before: i32,
    reference_offset_after: i32,
    compact_package_index: i32,
    compact_package_index_captured: bool,
    compact_package_index_supported: bool,
    out: &mut Vec<u8>,
) {
    let mut json = Vec::new();
    actor_json(services, slot, actor, None, &mut json);
    json.pop(); // drop ActorJson's closing '}'
    out.extend_from_slice(&json);
    out.extend_from_slice(b",\"raw_reference\":{\"offset_before\":");
    if reference_offset_before == -1 {
        out.extend_from_slice(b"null");
    } else {
        out.extend_from_slice(reference_offset_before.to_string().as_bytes());
    }
    out.extend_from_slice(b",\"offset_after\":");
    if reference_offset_after == -1 {
        out.extend_from_slice(b"null");
    } else {
        out.extend_from_slice(reference_offset_after.to_string().as_bytes());
    }
    out.extend_from_slice(b",\"compact_package_index\":");
    if compact_package_index_captured {
        out.extend_from_slice(compact_package_index.to_string().as_bytes());
    } else {
        out.extend_from_slice(b"null");
    }
    if !compact_package_index_supported {
        out.extend_from_slice(
            b",\"status\":\"unsupported\",\"reason\":\"archive_does_not_expose_actor_slot_compact_index\"",
        );
    } else if !compact_package_index_captured {
        out.extend_from_slice(
            b",\"status\":\"unavailable\",\"reason\":\"actor_reference_did_not_invoke_linker_compact_index_decoder\"",
        );
    } else if reference_offset_before == -1 || reference_offset_after == -1 {
        out.extend_from_slice(b",\"status\":\"unavailable\",\"reason\":\"archive_tell_unavailable\"");
    } else {
        out.extend_from_slice(b",\"status\":\"observed\",\"reason\":null");
    }
    out.extend_from_slice(b"}}");
}

/// `SerializeCheckpoint`.
fn serialize_checkpoint(services: &Hp2TraceReporterServices, level: *mut c_void, checkpoint: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"{\"checkpoint\":\"");
    out.extend_from_slice(checkpoint);
    out.extend_from_slice(b"\",\"sequence\":\"rearranged\",\"i_first_net_relevant_actor\":");
    out.extend_from_slice(
        unsafe { (services.level_first_net_relevant_actor)(level) }
            .to_string()
            .as_bytes(),
    );
    out.extend_from_slice(b",\"i_first_dynamic_actor\":");
    out.extend_from_slice(
        unsafe { (services.level_first_dynamic_actor)(level) }
            .to_string()
            .as_bytes(),
    );
    out.extend_from_slice(b",\"actors\":[");
    let count = unsafe { (services.level_actor_count)(level) };
    for slot in 0..count {
        if slot != 0 {
            out.push(b',');
        }
        let actor = unsafe { (services.level_actor_at)(level, slot) };
        actor_json(services, slot, actor, Some(level), &mut out);
    }
    out.extend_from_slice(b"]}");
    out
}

/// `CapturePostStartupBeforeFirstTick`.
///
/// # Safety
/// `engine` is null or a live `UEngine*`.
pub unsafe fn capture_post_startup_before_first_tick(engine: *mut c_void) {
    if !enabled() {
        return;
    }
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return;
    };
    let level = if engine.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (services.game_engine_level)(engine) }
    };
    let state = unsafe { &mut *(&raw mut STATE) };
    if !level.is_null()
        && !state.raw_actor_table_open
        && state.raw_level == level
        && !state.raw_post_deserialize.is_empty()
    {
        state.post_startup_before_first_tick =
            serialize_checkpoint(&services, level, b"post_startup_before_first_tick");
    }
}

/// `BeginRawLevelActorTable`.
///
/// # Safety
/// Single game thread.
pub unsafe fn begin_raw_level_actor_table(level: *mut c_void) {
    if !enabled() {
        return;
    }
    let state = unsafe { &mut *(&raw mut STATE) };
    state.raw_level = level;
    state.raw_actor_count = 0;
    state.raw_actor_table_open = true;
    state.raw_post_deserialize = RAW_PREFIX.to_vec();
}

/// `CaptureRawLevelActor`.
///
/// # Safety
/// Single game thread; `level`/`actor` null or live.
#[allow(clippy::too_many_arguments)]
pub unsafe fn capture_raw_level_actor(
    level: *mut c_void,
    slot: i32,
    actor: *mut c_void,
    reference_offset_before: i32,
    reference_offset_after: i32,
    compact_package_index: i32,
    compact_package_index_captured: i32,
    compact_package_index_supported: i32,
) {
    let state = unsafe { &mut *(&raw mut STATE) };
    if !state.raw_actor_table_open || state.raw_level != level {
        return;
    }
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return;
    };
    let had_previous = state.raw_actor_count != 0;
    state.raw_actor_count += 1;
    if had_previous {
        state.raw_post_deserialize.push(b',');
    }
    raw_actor_json(
        &services,
        slot,
        actor,
        reference_offset_before,
        reference_offset_after,
        compact_package_index,
        compact_package_index_captured != 0,
        compact_package_index_supported != 0,
        &mut state.raw_post_deserialize,
    );
}

/// `EndRawLevelActorTable`.
///
/// # Safety
/// Single game thread.
pub unsafe fn end_raw_level_actor_table(level: *mut c_void) {
    let state = unsafe { &mut *(&raw mut STATE) };
    if state.raw_actor_table_open && state.raw_level == level {
        state.raw_post_deserialize.extend_from_slice(b"]}");
        state.raw_actor_table_open = false;
    }
}

/// `WriteIfReady`.
///
/// # Safety
/// `engine` is null or a live `UEngine*`.
pub unsafe fn write_if_ready(engine: *mut c_void) -> i32 {
    match unsafe { write_if_ready_with(engine, || Ok::<(), std::convert::Infallible>(())) } {
        Ok(result) => result,
        Err(never) => match never {},
    }
}

/// Checked host boundary for `WriteIfReady`: all engine reads and report
/// serialization complete before `before_write` runs, and no bytes are
/// written if that synchronous canonical callback reported an error.
///
/// # Safety
/// `engine` is null or a live `UEngine*`.
pub unsafe fn write_if_ready_with<E>(
    engine: *mut c_void,
    before_write: impl FnOnce() -> Result<(), E>,
) -> Result<i32, E> {
    let Ok(services) = (unsafe { svc::published_services() }) else {
        return Ok(0);
    };
    let level = if engine.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (services.game_engine_level)(engine) }
    };
    let state = unsafe { &mut *(&raw mut STATE) };
    if level.is_null()
        || state.raw_post_deserialize.is_empty()
        || state.post_startup_before_first_tick.is_empty()
        || state.raw_actor_table_open
        || state.raw_level != level
    {
        return Ok(0);
    }
    let Some(report_path) = env_bytes("HP2_ACTOR_SLOT_DUMP").filter(|v| !v.is_empty()) else {
        return Ok(1);
    };
    let mut document = Vec::new();
    document.extend_from_slice(b"{\"version\":1,\"checkpoints\":[");
    document.extend_from_slice(&state.raw_post_deserialize);
    document.push(b',');
    document.extend_from_slice(&state.post_startup_before_first_tick);
    document.push(b',');
    document.extend_from_slice(&serialize_checkpoint(&services, level, b"post_startup"));
    document.extend_from_slice(b"]}\x0a");
    before_write()?;
    if let Err(kind) = write_report(&report_path, &document) {
        let reason: &[u8] = match kind {
            svc::ReportWriteError::Unwritable => b"unwritable",
            svc::ReportWriteError::WriteFailed => b"write_failed",
        };
        let mut err = std::io::stderr().lock();
        let _ = err.write_all(b"<HP2_ACTOR_SLOT_DUMP> report_error=");
        let _ = err.write_all(reason);
        let _ = err.write_all(b" path=");
        let _ = err.write_all(&report_path);
        let _ = err.write_all(b"\x0a");
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_char;

    #[repr(C)]
    struct OwnerMem {
        _pad: [u8; 16],
        shadow_class: *mut c_void,
    }

    const ACTOR0: usize = 0x1000;
    const ACTOR_CLASS0: usize = 0x2000;
    const OWNER_CLASS0: usize = 0x3000;

    unsafe extern "C" fn object_path_ansi(obj: *mut c_void, owner_mem: usize) -> *const c_char {
        if obj as usize == ACTOR0 {
            c"Package.Actor0".as_ptr()
        } else if obj as usize == ACTOR_CLASS0 {
            c"Package.ActorClass".as_ptr()
        } else if obj as usize == owner_mem {
            c"Package.Owner0".as_ptr()
        } else if obj as usize == OWNER_CLASS0 {
            c"Package.OwnerClass".as_ptr()
        } else {
            c"".as_ptr()
        }
    }

    // `object_path_ansi` must be a bare `unsafe extern "C" fn(*mut c_void) ->
    // *const c_char` with no closure state, so the owner pointer is baked in
    // via a thread-local set by the test before each `actor_json` call.
    thread_local! {
        static OWNER_PTR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    unsafe extern "C" fn fake_object_path_ansi(obj: *mut c_void) -> *const c_char {
        unsafe { object_path_ansi(obj, OWNER_PTR.get()) }
    }
    unsafe extern "C" fn fake_object_class(obj: *mut c_void) -> *mut c_void {
        if obj as usize == ACTOR0 {
            ACTOR_CLASS0 as *mut c_void
        } else if obj as usize == OWNER_PTR.get() && obj as usize != 0 {
            OWNER_CLASS0 as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    }
    unsafe extern "C" fn fake_actor_owner(actor: *mut c_void) -> *mut c_void {
        if actor as usize == ACTOR0 {
            OWNER_PTR.get() as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    }
    unsafe extern "C" fn fake_find_property(klass: *mut c_void, name: *const c_char) -> *mut c_void {
        let name = unsafe { std::ffi::CStr::from_ptr(name) };
        if klass as usize == OWNER_CLASS0 && name.to_bytes() == b"ShadowClass" {
            0x99 as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    }
    unsafe extern "C" fn fake_property_offset(_field: *mut c_void) -> i32 {
        std::mem::offset_of!(OwnerMem, shadow_class) as i32
    }
    unsafe extern "C" fn fake_cast_to_class(obj: *mut c_void) -> *mut c_void {
        obj
    }
    unsafe extern "C" fn null_fn(_obj: *mut c_void) -> *mut c_void {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn zero_fn(_obj: *mut c_void) -> i32 {
        0
    }
    unsafe extern "C" fn one_fn(_obj: *mut c_void) -> i32 {
        1
    }
    unsafe extern "C" fn fake_actor_role(_actor: *mut c_void) -> i32 {
        3
    }
    unsafe extern "C" fn fake_actor_remote_role(_actor: *mut c_void) -> i32 {
        1
    }
    unsafe extern "C" fn fake_level_first_dynamic_actor(_level: *mut c_void) -> i32 {
        0
    }
    unsafe extern "C" fn unused_i32_fn2(_a: *mut c_void, _b: i32) -> *mut c_void {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn unused_name_fn(_index: i32) -> *const c_char {
        c"".as_ptr()
    }
    unsafe extern "C" fn unused_i32_name_fn(_klass: *mut c_void, _name: *const c_char) -> *mut c_void {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn unused_u32_fn(_prop: *mut c_void) -> u32 {
        0
    }
    unsafe extern "C" fn unused_u64_fn() -> u64 {
        0
    }

    fn fake_services() -> Hp2TraceReporterServices {
        Hp2TraceReporterServices {
            object_path_ansi: fake_object_path_ansi,
            object_name_ansi: fake_object_path_ansi,
            object_class: fake_object_class,
            fname_ansi_or_empty: unused_name_fn,
            actor_tag_fname: zero_fn,
            actor_shadow: null_fn,
            actor_x_level: null_fn,
            actor_owner: fake_actor_owner,
            actor_delete_marked: zero_fn,
            actor_pending_kill: zero_fn,
            actor_ticked: zero_fn,
            actor_game_relevant: one_fn,
            actor_role: fake_actor_role,
            actor_remote_role: fake_actor_remote_role,
            level_actor_count: zero_fn,
            level_actor_at: unused_i32_fn2,
            level_first_dynamic_actor: fake_level_first_dynamic_actor,
            level_first_net_relevant_actor: zero_fn,
            game_engine_level: null_fn,
            class_super: null_fn,
            class_default_actor: null_fn,
            cast_to_class: fake_cast_to_class,
            cast_to_actor: null_fn,
            find_property: fake_find_property,
            find_bool_property: unused_i32_name_fn,
            find_int_property: unused_i32_name_fn,
            find_float_property: unused_i32_name_fn,
            property_offset: fake_property_offset,
            property_array_dim: zero_fn,
            property_element_size: zero_fn,
            bool_property_bitmask: unused_u32_fn,
            property_outer: null_fn,
            rand_trace_ordinal: unused_u64_fn,
        }
    }

    // Ground truth: `ActorJson` on a null actor (SDLLaunch.cpp, deleted,
    // formerly ~4848-4850): every field is a fixed null/false literal,
    // independent of `Slot`.
    #[test]
    fn actor_json_of_a_null_actor_matches_cpp_fixed_literal() {
        OWNER_PTR.set(0);
        let mut out = Vec::new();
        actor_json(&fake_services(), 3, std::ptr::null_mut(), None, &mut out);
        assert_eq!(
            out,
            b"{\"slot\":3,\"path\":null,\"class\":null,\"is_null\":true,\"is_deleted\":false,\"is_pending_kill\":false,\"game_relevant\":null,\"role\":null,\"remote_role\":null,\"owner_path\":null,\"owner_class\":null,\"effective_shadow_class\":null,\"spawn_origin\":null,\"spawn_owner_path\":null,\"spawn_owner_class\":null,\"spawn_request_id\":null,\"spawn_order\":null}"
                .to_vec()
        );
    }

    // Ground truth: `ActorJson` on a non-null actor with no `Owner` and no
    // `Level` (the raw-table default-argument call path; SDLLaunch.cpp,
    // deleted, formerly ~4871-4924): `IsDynamicActorShadow` is false whenever
    // `Level` is absent, so every `spawn_*` field stays `null`.
    #[test]
    fn actor_json_of_a_non_null_actor_with_no_owner_and_no_level() {
        OWNER_PTR.set(0);
        let mut out = Vec::new();
        actor_json(&fake_services(), 7, ACTOR0 as *mut c_void, None, &mut out);
        assert_eq!(
            out,
            b"{\"slot\":7,\"path\":\"Package.Actor0\",\"class\":\"Package.ActorClass\",\"is_null\":false,\"is_deleted\":false,\"is_pending_kill\":false,\"game_relevant\":true,\"role\":3,\"remote_role\":1,\"owner_path\":null,\"owner_class\":null,\"effective_shadow_class\":null,\"spawn_origin\":null,\"spawn_owner_path\":null,\"spawn_owner_class\":null,\"spawn_request_id\":null,\"spawn_order\":null}"
                .to_vec()
        );
    }

    // Ground truth: the `IsDynamicActorShadow` branch (SDLLaunch.cpp,
    // deleted, formerly ~4878-4882, 4903-4921): when `Level` is present,
    // `Slot >= iFirstDynamicActor`, and the actor's class is a child of its
    // owner's `ShadowClass`, `spawn_origin`/`spawn_owner_*`/`spawn_order` are
    // populated from the owner instead of staying `null`.
    #[test]
    fn actor_json_of_a_dynamic_actor_shadow_populates_spawn_fields() {
        let owner_mem = OwnerMem {
            _pad: [0; 16],
            shadow_class: ACTOR_CLASS0 as *mut c_void,
        };
        OWNER_PTR.set(std::ptr::addr_of!(owner_mem) as usize);
        let mut out = Vec::new();
        actor_json(
            &fake_services(),
            0,
            ACTOR0 as *mut c_void,
            Some(0x4000 as *mut c_void),
            &mut out,
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"owner_path\":\"Package.Owner0\",\"owner_class\":\"Package.OwnerClass\""));
        assert!(text.contains("\"effective_shadow_class\":null"));
        assert!(text.contains(
            "\"spawn_origin\":\"level_dynamic_slot\",\"spawn_owner_path\":\"Package.Owner0\",\"spawn_owner_class\":\"Package.OwnerClass\",\"spawn_request_id\":null,\"spawn_order\":0"
        ));
    }

    // Ground truth: `RawActorJson`'s `raw_reference`/`status` branches
    // (SDLLaunch.cpp, deleted, formerly ~4857-4869).
    #[test]
    fn raw_actor_json_status_branches_match_cpp() {
        OWNER_PTR.set(0);
        let services = fake_services();

        let mut unsupported = Vec::new();
        raw_actor_json(&services, 0, std::ptr::null_mut(), -1, -1, 0, false, false, &mut unsupported);
        assert!(String::from_utf8(unsupported).unwrap().ends_with(
            "\"raw_reference\":{\"offset_before\":null,\"offset_after\":null,\"compact_package_index\":null,\"status\":\"unsupported\",\"reason\":\"archive_does_not_expose_actor_slot_compact_index\"}}"
        ));

        let mut not_captured = Vec::new();
        raw_actor_json(&services, 0, std::ptr::null_mut(), -1, -1, 0, false, true, &mut not_captured);
        assert!(String::from_utf8(not_captured).unwrap().ends_with(
            "\"raw_reference\":{\"offset_before\":null,\"offset_after\":null,\"compact_package_index\":null,\"status\":\"unavailable\",\"reason\":\"actor_reference_did_not_invoke_linker_compact_index_decoder\"}}"
        ));

        let mut tell_unavailable = Vec::new();
        raw_actor_json(&services, 0, std::ptr::null_mut(), -1, 40, 9, true, true, &mut tell_unavailable);
        assert!(String::from_utf8(tell_unavailable).unwrap().ends_with(
            "\"raw_reference\":{\"offset_before\":null,\"offset_after\":40,\"compact_package_index\":9,\"status\":\"unavailable\",\"reason\":\"archive_tell_unavailable\"}}"
        ));

        let mut observed = Vec::new();
        raw_actor_json(&services, 0, std::ptr::null_mut(), 30, 40, 9, true, true, &mut observed);
        assert!(String::from_utf8(observed).unwrap().ends_with(
            "\"raw_reference\":{\"offset_before\":30,\"offset_after\":40,\"compact_package_index\":9,\"status\":\"observed\",\"reason\":null}}"
        ));
    }

    // Ground truth: `SerializeCheckpoint` (SDLLaunch.cpp, deleted, formerly
    // ~4873-4879): `i_first_net_relevant_actor`/`i_first_dynamic_actor` are
    // emitted once up front, then every slot `0..Actors.Num()` (including
    // null slots) is serialized via `ActorJson`.
    #[test]
    fn serialize_checkpoint_emits_header_and_every_slot() {
        OWNER_PTR.set(0);
        unsafe extern "C" fn level_actor_count(_level: *mut c_void) -> i32 {
            2
        }
        unsafe extern "C" fn level_actor_at(_level: *mut c_void, slot: i32) -> *mut c_void {
            if slot == 1 {
                ACTOR0 as *mut c_void
            } else {
                std::ptr::null_mut()
            }
        }
        unsafe extern "C" fn first_net_relevant(_level: *mut c_void) -> i32 {
            5
        }
        let mut services = fake_services();
        services.level_actor_count = level_actor_count;
        services.level_actor_at = level_actor_at;
        services.level_first_net_relevant_actor = first_net_relevant;

        let out = serialize_checkpoint(&services, 0x5000 as *mut c_void, b"post_startup");
        let mut expected = Vec::new();
        expected.extend_from_slice(
            b"{\"checkpoint\":\"post_startup\",\"sequence\":\"rearranged\",\"i_first_net_relevant_actor\":5,\"i_first_dynamic_actor\":0,\"actors\":[",
        );
        actor_json(&services, 0, std::ptr::null_mut(), Some(0x5000 as *mut c_void), &mut expected);
        expected.push(b',');
        actor_json(&services, 1, ACTOR0 as *mut c_void, Some(0x5000 as *mut c_void), &mut expected);
        expected.extend_from_slice(b"]}");
        assert_eq!(out, expected);
    }
}
