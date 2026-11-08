//! `FidelityCheckpoint`/`ParseFidelityManifest` and `FidelityReporter`,
//! ported from `HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp:780-787` and
//! `:1019-1064` (manifest struct/parser) and `:3334-3964` (the reporter
//! itself, a `FScriptDispatchObserver`). `SDLLaunch.cpp`'s thin adapter
//! keeps the class name and the `IsEnabled`/ctor/`OnCallEnter`/`OnCallExit`/
//! `AfterTick`/`Flush` signatures so the untouched `FScriptDispatchObserver`
//! dispatch sites and `FMainLoopState`/`MainLoop` call sites keep compiling.
//!
//! Byte-safety: every piece of engine-sourced ANSI text (`GetPathName`,
//! `FName`/`FString` property values, state-frame names) is carried as
//! `Vec<u8>` end to end and escaped at the byte level by `escape`, which
//! mirrors `FidelityReporter::Escape`'s five-case table (`\\`, `"`, `\n`,
//! `\r`, `\t`; every other byte, including control bytes and bytes `>=0x80`,
//! passes through unescaped) -- this is deliberately *not*
//! `json::push_json_string`'s seven-case table, which would also escape
//! `\u0008`/`\u000c` where the C++ writes them raw.
//!
//! `ReadFrameHash`'s pixel seed-poisoning/readback-detection/hash is ported
//! as-is; `FColor`'s memory layout (`HarryPotter2/Unreal/Engine/Inc/UnTex.h:62`,
//! `BYTE R,G,B,A` under `__INTEL_BYTE_ORDER__`) is reproduced as a `[u8; 4]`
//! `R,G,B,A` pixel and reordered to `B,G,R,A` only when fed to SHA-256,
//! exactly matching `SDLLaunch.cpp:3635`.

use crate::reporters::json;
use crate::reporters::trace_services::{self, Hp2TraceReporterServices, ReportWriteError, write_report};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FRotator, UObject};
use sha2::{Digest, Sha256};
use std::ffi::{c_char, c_void};

const NAME_BUFFER_CAP: usize = 1024;
const STRING_ARRAY_BUFFER_CAP: usize = 4096;

/// Runtime cursors are source-specific: disk `line_index` is the current
/// `curScriptLine` row, while inline `line_index` is a `curScriptPosition`
/// character offset (not a normalized line number).
#[derive(Clone)]
enum CheckpointTrigger {
    Authored { kind: RuntimeKind, actor: String, path: Option<String>, thread_slot: i32, line_index: i32 },
    Tick { tick: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum RuntimeKind {
    Disk = 1,
    Inline = 2,
}

#[derive(Clone)]
pub(crate) struct FidelityCheckpoint {
    pub id: String,
    pub position: String,
    trigger: CheckpointTrigger,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn find_byte_from(haystack: &[u8], byte: u8, from: usize) -> Option<usize> {
    haystack.get(from..)?.iter().position(|&b| b == byte).map(|i| i + from)
}

/// `ParseFidelityManifest` (`SDLLaunch.cpp:1019-1064`) plus runtime-tick
/// checkpoints emitted by the parity harness.
pub(crate) fn parse_fidelity_manifest(filename: Option<&str>) -> Option<Vec<FidelityCheckpoint>> {
    parse_fidelity_manifest_bytes(&json::read_document(filename)?)
}

fn parse_fidelity_manifest_bytes(text: &[u8]) -> Option<Vec<FidelityCheckpoint>> {
    let marker = find_subslice(text, b"\"checkpoints\"")?;
    let mut cursor = find_byte_from(text, b'[', marker)? + 1;
    let mut out = Vec::new();
    loop {
        json::skip_ws(text, &mut cursor);
        if cursor >= text.len() {
            return None;
        }
        if text[cursor] == b']' {
            return Some(out);
        }
        let end = json::object_end(text, cursor)?;
        let object = &text[cursor..end];
        let id = json::string_member(object, "id")?;
        let position = json::string_member(object, "position")?;
        let trigger = if position == "tick" {
            let tick = json::int_member(object, "tick")?;
            if tick <= 0 {
                return None;
            }
            CheckpointTrigger::Tick { tick }
        } else {
            let locator = json::object_member(object, "runtime_locator")?;
            let kind = match json::string_member(&locator, "kind")?.as_str() {
                "disk" => RuntimeKind::Disk,
                "inline" => RuntimeKind::Inline,
                _ => return None,
            };
            let actor = json::string_member(&locator, "actor")?;
            let thread_slot = json::int_member(&locator, "thread_slot")?;
            let path = if kind == RuntimeKind::Disk {
                Some(json::string_member(&locator, "path")?)
            } else {
                None
            };
            let line_index = if kind == RuntimeKind::Disk {
                json::int_member(&locator, "line_index")?
            } else {
                json::int_member(&locator, "character_offset")?
            };
            if actor.is_empty() || thread_slot < 0 || line_index < 0 || path.as_deref() == Some("") {
                return None;
            }
            CheckpointTrigger::Authored { kind, actor, path, thread_slot, line_index }
        };
        out.push(FidelityCheckpoint { id, position, trigger });
        cursor = end;
        json::skip_ws(text, &mut cursor);
        if cursor < text.len() && text[cursor] == b',' {
            cursor += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disk_inline_and_tick_locators_with_source_specific_cursor_units() {
        let rows = parse_fidelity_manifest_bytes(
            br#"{"version":1,"checkpoints":[{"id":"disk","position":"before","runtime_locator":{"kind":"disk","actor":"Entry.CutScene0","path":"System/CUTSCENES/ENTRY.int","thread_slot":2,"line_index":7}},{"id":"inline","position":"at","runtime_locator":{"kind":"inline","actor":"AOLDemo.CutScene0","thread_slot":0,"character_offset":52}},{"id":"tick:4","position":"tick","tick":4}]}"#,
        )
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert!(matches!(
            &rows[0].trigger,
            CheckpointTrigger::Authored { kind: RuntimeKind::Disk, actor, path: Some(path), thread_slot: 2, line_index: 7 }
                if actor == "Entry.CutScene0" && path == "System/CUTSCENES/ENTRY.int"
        ));
        assert!(matches!(
            &rows[1].trigger,
            CheckpointTrigger::Authored { kind: RuntimeKind::Inline, actor, path: None, thread_slot: 0, line_index: 52 }
                if actor == "AOLDemo.CutScene0"
        ));
        assert!(matches!(&rows[2].trigger, CheckpointTrigger::Tick { tick: 4 }));
    }

    #[test]
    fn rejects_nonpositive_or_incomplete_tick_rows() {
        assert!(parse_fidelity_manifest_bytes(
            br#"{"checkpoints":[{"id":"tick:0","position":"tick","tick":0}]}"#
        ).is_none());
        assert!(parse_fidelity_manifest_bytes(
            br#"{"checkpoints":[{"id":"tick:1","position":"tick"}]}"#
        ).is_none());
    }

}

/// Typed, source-specific result returned by the engine ABI leaf.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RuntimeLocatorResult {
    pub kind: i32,
    pub thread_slot: i32,
    pub cursor: i32,
    pub actor_len: i32,
    pub path_len: i32,
}

#[derive(Clone)]
struct RuntimeLocator {
    kind: RuntimeKind,
    actor: Vec<u8>,
    path: Vec<u8>,
    thread_slot: i32,
    line_index: i32,
}

struct PendingBoundary {
    thread: *mut c_void,
    locator: RuntimeLocator,
}

struct FidelitySnapshot {
    checkpoint: FidelityCheckpoint,
    state: Vec<u8>,
    frame_hash: Vec<u8>,
}

struct AudioRecord {
    id: Vec<u8>,
    state: &'static str,
}

/// Engine leaves `FidelityReporter` needs. Published once by `SDLLaunch.cpp`'s
/// adapter and captured by value at construction (same rationale as
/// `world_collision_gate::Hp2WorldCollisionGateServices`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2FidelityServices {
    /// `Cast<UGameEngine>(Engine) ? GameEngine->GViewport : NULL`.
    pub engine_viewport: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    /// `Viewport->RenDev`.
    pub viewport_render_device: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `*out_x = Viewport->SizeX; *out_y = Viewport->SizeY;`.
    pub viewport_size: unsafe extern "C" fn(viewport: *mut c_void, out_x: *mut i32, out_y: *mut i32),
    /// `Viewport->Actor` (`APlayerPawn*`).
    pub viewport_actor: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `RenDev->ReadPixels(&Pixels[0])`; `buf` holds `pixel_count` `FColor`s
    /// (`R,G,B,A` bytes each, `UnTex.h:62`).
    pub read_pixels: unsafe extern "C" fn(render_device: *mut c_void, buf: *mut u8, pixel_count: usize),
    /// `FidelityReporter::ActorState(Actor)`: state-frame name, or the ANSI
    /// literal `"None"` baked in by the leaf when there is no live state
    /// node distinct from the actor's class. Same buffer contract as
    /// `object_path_name` used to have (see [`trace_services::object_path`]
    /// for the generic replacement used elsewhere in this file).
    pub actor_state_name: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `appToAnsi(*Actor->AnimSequence)`. Same buffer contract.
    pub actor_anim_sequence_name: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `FidelityReporter::StringArray(Object,Name,CountName)` minus the
    /// `SortNames` step (done in Rust): each non-empty, `Count`-clamped
    /// `FString` element's ANSI bytes, NUL-separated, written into `buf`;
    /// returns the total byte length written (0 if the field/count property
    /// is missing or every element is empty), independent of `cap`
    /// (truncation contract matches the old `object_path_name`'s).
    #[allow(clippy::too_many_arguments)]
    pub string_array: unsafe extern "C" fn(
        object: *mut c_void,
        name: *const u8,
        name_len: usize,
        count_name: *const u8,
        count_name_len: usize,
        buf: *mut u8,
        cap: usize,
    ) -> i32,
    /// `Cast<UALAudioSubsystem>(Engine->Audio) ? Sources.Num() : 0` after
    /// `GetFidelityActiveSources`.
    pub audio_fidelity_source_count: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `appToAnsi(*Sources(Index).Id)`. Same buffer contract as `object_path_name` used to have.
    pub audio_fidelity_source_id: unsafe extern "C" fn(engine: *mut c_void, index: i32, buf: *mut u8, cap: usize) -> i32,
    /// `Sources(Index).Paused`.
    pub audio_fidelity_source_paused: unsafe extern "C" fn(engine: *mut c_void, index: i32) -> i32,
    /// Resolves the owning CutScene path and source-specific runtime cursor.
    /// Returns `Hp2RuntimeLocatorResult.kind` (disk=1, inline=2, absent=0);
    /// disk writes the raw FileName and inline leaves `path` empty. Both
    /// kinds write the actual parent CutScene `GetPathName()` and aThreads slot.
    pub build_runtime_locator: unsafe extern "C" fn(
        thread: *mut c_void,
        actor_buf: *mut u8,
        actor_cap: usize,
        path_buf: *mut u8,
        path_cap: usize,
        result: *mut Hp2RuntimeLocatorResult,
    ) -> i32,
}

/// `FidelityReporter`.
pub struct State {
    services: Hp2FidelityServices,
    engine: *mut c_void,
    report_path: String,
    manifest_present: bool,
    checkpoints: Vec<FidelityCheckpoint>,
    snapshots: Vec<FidelitySnapshot>,
    pending: Vec<PendingBoundary>,
    frame_hash_snapshot_count: usize,
    tick_count: i32,
    interpreted_calls: i32,
}

fn env_non_empty(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// `FidelityReporter::IsEnabled`.
pub fn is_enabled() -> bool {
    env_non_empty("HP2_FIDELITY_REPORT").is_some()
}

/// `FidelityReporter::FidelityReporter`.
pub fn create(services: Hp2FidelityServices, engine: *mut c_void) -> Box<State> {
    let checkpoints = parse_fidelity_manifest(env_non_empty("HP2_FIDELITY_CHECKPOINTS").as_deref());
    Box::new(State {
        services,
        engine,
        report_path: env_non_empty("HP2_FIDELITY_REPORT").unwrap_or_default(),
        manifest_present: checkpoints.is_some(),
        checkpoints: checkpoints.unwrap_or_default(),
        snapshots: Vec::new(),
        pending: Vec::new(),
        frame_hash_snapshot_count: 0,
        tick_count: 0,
        interpreted_calls: 0,
    })
}

fn object_class(object: *mut c_void) -> *mut c_void {
    if object.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(object as *mut UObject)).class }
}

/// The generic reflection/object-graph leaf table published once by
/// `HP2TraceReporterServices.cpp`, reused here instead of duplicating
/// dedicated Fidelity-only C++ leaves for plain `FindField`/offset/`Cast`
/// work: by the time `FidelityReporter::create` runs (deep in `MainLoop`,
/// long after every translation unit's static initializers have run),
/// `HP2TraceReporterServices.cpp`'s publish-once static initializer has
/// already published this table.
fn trace_svc() -> Option<Hp2TraceReporterServices> {
    unsafe { trace_services::published_services() }.ok()
}

/// `Text(Object->GetPathName())`, replacing the old dedicated
/// `object_path_name` leaf with the generic `object_path_ansi` one.
fn object_path_text(object: *mut c_void) -> Vec<u8> {
    if object.is_null() {
        return Vec::new();
    }
    match trace_svc() {
        Some(svc) => unsafe { trace_services::object_path(&svc, object) },
        None => Vec::new(),
    }
}

/// `Cast<UGameEngine>(Engine) ? GameEngine->GLevel : NULL`, replacing the
/// old dedicated `engine_level` leaf with the generic `game_engine_level`
/// one (byte-identical body in `HP2TraceReporterServices.cpp`).
fn game_engine_level(engine: *mut c_void) -> *mut c_void {
    match trace_svc() {
        Some(svc) => unsafe { (svc.game_engine_level)(engine) },
        None => std::ptr::null_mut(),
    }
}

/// `Level->Actors.Num()`, replacing the old dedicated `level_actor_count`
/// leaf with the generic one (`ULevel`'s `Actors` is declared on its base
/// `ULevelBase`, so the two leaves read the exact same field).
fn level_actor_count(level: *mut c_void) -> i32 {
    match trace_svc() {
        Some(svc) => unsafe { (svc.level_actor_count)(level) },
        None => 0,
    }
}

/// `Level->Actors(Index)`, replacing the old dedicated `level_actor_at` leaf.
fn level_actor_at(level: *mut c_void, index: i32) -> *mut c_void {
    match trace_svc() {
        Some(svc) => unsafe { (svc.level_actor_at)(level, index) },
        None => std::ptr::null_mut(),
    }
}

/// `Actor->IsPendingKill()`, replacing the old dedicated
/// `actor_is_pending_kill` leaf with the generic `actor_pending_kill` one.
fn actor_is_pending_kill(actor: *mut c_void) -> bool {
    match trace_svc() {
        Some(svc) => unsafe { (svc.actor_pending_kill)(actor) != 0 },
        None => false,
    }
}

/// NUL-terminates `name` for the generic `find_*_property` leaves, which
/// take a plain `const char*` (unlike the old Fidelity-only leaves, which
/// took a UTF-8 `(ptr,len)` pair).
fn nul_terminated(name: &[u8]) -> Vec<u8> {
    let mut owned = Vec::with_capacity(name.len() + 1);
    owned.extend_from_slice(name);
    owned.push(0);
    owned
}

/// `FindField<UProperty>(Object->GetClass(),Name)` read as an object
/// pointer (`UObjectProperty`); null if the field is missing or null.
/// Replaces the old dedicated `object_property` leaf by composing the
/// generic `find_property`/`property_offset` leaves with
/// [`trace_services::read_ptr_at`].
fn object_property(object: *mut c_void, name: &[u8]) -> *mut c_void {
    let Some(svc) = trace_svc() else { return std::ptr::null_mut() };
    let class = object_class(object);
    if class.is_null() {
        return std::ptr::null_mut();
    }
    let field = unsafe { (svc.find_property)(class, nul_terminated(name).as_ptr() as *const c_char) };
    if field.is_null() {
        return std::ptr::null_mut();
    }
    let offset = unsafe { (svc.property_offset)(field) };
    unsafe { trace_services::read_ptr_at(object, offset) }
}

fn has_property(object: *mut c_void, name: &[u8]) -> bool {
    if object.is_null() {
        return false;
    }
    let Some(svc) = trace_svc() else { return false };
    let class = object_class(object);
    if class.is_null() {
        return false;
    }
    unsafe {
        !(svc.find_property)(class, nul_terminated(name).as_ptr() as *const c_char).is_null()
    }
}

/// Same lookup read as a `UBoolProperty`; `None` if the field does not
/// exist. Replaces the old dedicated `bool_property` leaf by composing the
/// generic `find_bool_property`/`property_offset`/`bool_property_bitmask`
/// leaves with [`trace_services::read_u32_at`].
fn bool_property(object: *mut c_void, name: &[u8]) -> Option<bool> {
    let svc = trace_svc()?;
    let class = object_class(object);
    if class.is_null() {
        return None;
    }
    let field = unsafe { (svc.find_bool_property)(class, nul_terminated(name).as_ptr() as *const c_char) };
    if field.is_null() {
        return None;
    }
    let offset = unsafe { (svc.property_offset)(field) };
    let mask = unsafe { (svc.bool_property_bitmask)(field) };
    let word = unsafe { trace_services::read_u32_at(object, offset) };
    Some((word & mask) != 0)
}

/// Same lookup read as a `UFloatProperty`; `None` if the field does not
/// exist. Replaces the old dedicated `float_property` leaf by composing the
/// generic `find_float_property`/`property_offset` leaves with
/// [`trace_services::read_f32_at`].
fn float_property(object: *mut c_void, name: &[u8]) -> Option<f32> {
    let svc = trace_svc()?;
    let class = object_class(object);
    if class.is_null() {
        return None;
    }
    let field = unsafe { (svc.find_float_property)(class, nul_terminated(name).as_ptr() as *const c_char) };
    if field.is_null() {
        return None;
    }
    let offset = unsafe { (svc.property_offset)(field) };
    Some(unsafe { trace_services::read_f32_at(object, offset) })
}

/// `Object && Object->GetClass() && !appStricmp(Object->GetClass()->GetName(),Name)`.
fn class_named(svc: &Hp2TraceReporterServices, class: *mut c_void, name: &[u8]) -> bool {
    if class.is_null() {
        return false;
    }
    unsafe { trace_services::object_name(svc, class) }.eq_ignore_ascii_case(name)
}

/// Matches CutScript and every subclass using Unreal's inherited class chain.
fn is_cut_script(object: *mut c_void) -> bool {
    if object.is_null() {
        return false;
    }
    let Some(svc) = trace_svc() else { return false };
    let mut class = object_class(object);
    while !class.is_null() {
        if class_named(&svc, class, b"CutScript") {
            return true;
        }
        class = unsafe { (svc.class_super)(class) };
    }
    false
}

/// `FidelityReporter::IsClassNamed(Object,TEXT("BaseCam"))`: walks
/// `Object->GetClass()`'s super-class chain. Replaces the old dedicated
/// `is_base_cam_class` leaf by composing the generic `object_class`/
/// `class_super` leaves.
fn is_base_cam_class(object: *mut c_void) -> bool {
    let Some(svc) = trace_svc() else { return false };
    let mut class = object_class(object);
    while !class.is_null() {
        if class_named(&svc, class, b"BaseCam") {
            return true;
        }
        class = unsafe { (svc.class_super)(class) };
    }
    false
}

/// `FidelityReporter::CapturedCamera(Thread)`: scans `Thread->aCapturedActors`
/// for the first element `IsClassNamed(...,"BaseCam")`; null if none.
/// Replaces the old dedicated `captured_camera` leaf by composing the
/// generic `find_property`/`property_offset`/`property_array_dim`/
/// `property_element_size` leaves with [`trace_services::read_ptr_at`].
fn captured_camera(thread: *mut c_void) -> *mut c_void {
    let Some(svc) = trace_svc() else { return std::ptr::null_mut() };
    let class = object_class(thread);
    if class.is_null() {
        return std::ptr::null_mut();
    }
    const NAME: &[u8] = b"aCapturedActors";
    let field = unsafe { (svc.find_property)(class, nul_terminated(NAME).as_ptr() as *const c_char) };
    if field.is_null() {
        return std::ptr::null_mut();
    }
    let offset = unsafe { (svc.property_offset)(field) };
    let array_dim = unsafe { (svc.property_array_dim)(field) };
    let element_size = unsafe { (svc.property_element_size)(field) };
    for index in 0..array_dim {
        let candidate = unsafe { trace_services::read_ptr_at(thread, offset + index * element_size) };
        if !candidate.is_null() && is_base_cam_class(candidate) {
            return candidate;
        }
    }
    std::ptr::null_mut()
}

fn actor_location(actor: *mut c_void) -> FVector {
    unsafe { (*(actor as *mut AActor)).location }
}

fn actor_rotation(actor: *mut c_void) -> FRotator {
    unsafe { (*(actor as *mut AActor)).rotation }
}

fn actor_velocity(actor: *mut c_void) -> FVector {
    unsafe { (*(actor as *mut AActor)).velocity }
}

fn actor_anim_frame(actor: *mut c_void) -> f32 {
    unsafe { (*(actor as *mut AActor)).anim_frame }
}

fn actor_anim_finished(actor: *mut c_void) -> bool {
    unsafe { (*(actor as *mut AActor)).b_anim_finished() }
}

fn actor_delete_me(actor: *mut c_void) -> bool {
    unsafe { (*(actor as *mut AActor)).b_delete_me() }
}

fn fold(value: &[u8]) -> Vec<u8> {
    value.iter().map(|b| b.to_ascii_lowercase()).collect()
}

fn sort_names(values: &mut [Vec<u8>]) {
    values.sort_by(|a, b| fold(a).cmp(&fold(b)).then_with(|| a.cmp(b)));
}

/// `FidelityReporter::Escape`: five-case table, everything else passes
/// through raw (including bytes `>=0x80` and control bytes `<0x20`).
fn escape(value: &[u8], out: &mut Vec<u8>) {
    for &byte in value {
        match byte {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            _ => out.push(byte),
        }
    }
}

fn observed(value: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(b"{\"value\":");
    out.extend_from_slice(value);
    out.extend_from_slice(b",\"availability\":{\"status\":\"observed\"}}");
}

const FRAME_HASH_UNAVAILABLE: &[u8] =
    b"{\"value\":null,\"availability\":{\"status\":\"known_null\",\"code\":\"fidelity.frame_capture_disabled\"}}";
const CAMERA_UNAVAILABLE: &[u8] =
    b"{\"value\":null,\"availability\":{\"status\":\"known_null\",\"code\":\"fidelity.camera_unavailable\"}}";
const CUTSCENE_ABSENT: &[u8] =
    b"{\"value\":null,\"availability\":{\"status\":\"known_null\",\"code\":\"fidelity.cutscene_absent\"}}";

fn quantize_1024(value: f32) -> i64 {
    (value as f64 * 1024.0).round() as i64
}

fn vector_json(value: FVector) -> String {
    format!("[{},{},{}]", quantize_1024(value.x), quantize_1024(value.y), quantize_1024(value.z))
}

fn rotator_json(value: FRotator) -> String {
    format!("[{},{},{}]", value.pitch, value.yaw, value.roll)
}

/// Fetches `appToAnsi`-sourced text via a `(object,buf,cap)->len` leaf,
/// retrying with an exactly sized buffer if the first call reports more
/// bytes were available than the initial buffer held (matches the C++'s
/// unbounded `std::string`; never silently truncates).
fn fetch_text(object: *mut c_void, leaf: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> i32) -> Vec<u8> {
    if object.is_null() {
        return Vec::new();
    }
    let mut buffer = vec![0u8; NAME_BUFFER_CAP];
    let len = unsafe { leaf(object, buffer.as_mut_ptr(), buffer.len()) };
    if len <= 0 {
        return Vec::new();
    }
    let len = len as usize;
    if len > buffer.len() {
        buffer.resize(len, 0);
        let len2 = unsafe { leaf(object, buffer.as_mut_ptr(), buffer.len()) };
        if len2 <= 0 {
            return Vec::new();
        }
        buffer.truncate((len2 as usize).min(buffer.len()));
        return buffer;
    }
    buffer.truncate(len);
    buffer
}

impl State {
    fn active_actors(&self) -> Vec<*mut c_void> {
        let level = game_engine_level(self.engine);
        let mut actors = Vec::new();
        if !level.is_null() {
            let count = level_actor_count(level);
            for index in 0..count {
                let actor = level_actor_at(level, index);
                if !actor.is_null() && !actor_delete_me(actor) && !actor_is_pending_kill(actor) {
                    actors.push(actor);
                }
            }
        }
        let mut keyed: Vec<(Vec<u8>, Vec<u8>, *mut c_void)> = actors
            .into_iter()
            .map(|actor| {
                let path = object_path_text(actor);
                (fold(&path), path, actor)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        keyed.into_iter().map(|(_, _, actor)| actor).collect()
    }

    fn string_array(&self, object: *mut c_void, name: &[u8], count_name: &[u8]) -> Vec<Vec<u8>> {
        let mut buffer = vec![0u8; STRING_ARRAY_BUFFER_CAP];
        let len = unsafe {
            (self.services.string_array)(
                object,
                name.as_ptr(),
                name.len(),
                count_name.as_ptr(),
                count_name.len(),
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if len <= 0 {
            return Vec::new();
        }
        let len = (len as usize).min(buffer.len());
        let mut values: Vec<Vec<u8>> = buffer[..len]
            .split(|&b| b == 0)
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_vec())
            .collect();
        sort_names(&mut values);
        values
    }

    fn string_array_json(values: &[Vec<u8>], out: &mut Vec<u8>) {
        out.push(b'[');
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            out.push(b'"');
            escape(value, out);
            out.push(b'"');
        }
        out.push(b']');
    }

    fn camera_observation(&self, thread: *mut c_void) -> Vec<u8> {
        let viewport = unsafe { (self.services.engine_viewport)(self.engine) };
        let player = if viewport.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (self.services.viewport_actor)(viewport) }
        };
        if player.is_null() {
            return CAMERA_UNAVAILABLE.to_vec();
        }
        let mut camera = captured_camera(thread);
        if camera.is_null() {
            let player_camera = object_property(player, b"cam");
            if !player_camera.is_null() && is_base_cam_class(player_camera) {
                camera = player_camera;
            }
        }
        if camera.is_null() {
            camera = player;
        }
        let fov = float_property(player, b"DesiredFOV")
            .or_else(|| float_property(player, b"FovAngle"))
            .unwrap_or(0.0);
        let mut body = Vec::new();
        body.extend_from_slice(b"{\"location\":");
        body.extend_from_slice(vector_json(actor_location(camera)).as_bytes());
        body.extend_from_slice(b",\"rotation\":");
        body.extend_from_slice(rotator_json(actor_rotation(camera)).as_bytes());
        body.extend_from_slice(b",\"fov_1024\":");
        body.extend_from_slice(quantize_1024(fov).to_string().as_bytes());
        body.push(b'}');
        let mut out = Vec::new();
        observed(&body, &mut out);
        out
    }

    fn actors_observation(&self) -> Vec<u8> {
        let actors = self.active_actors();
        let mut json = vec![b'['];
        for (index, &actor) in actors.iter().enumerate() {
            if index > 0 {
                json.push(b',');
            }
            json.extend_from_slice(b"{\"id\":\"");
            escape(&object_path_text(actor), &mut json);
            json.extend_from_slice(b"\",\"class\":\"");
            escape(&object_path_text(object_class(actor)), &mut json);
            json.extend_from_slice(b"\",\"location\":");
            json.extend_from_slice(vector_json(actor_location(actor)).as_bytes());
            json.extend_from_slice(b",\"rotation\":");
            json.extend_from_slice(rotator_json(actor_rotation(actor)).as_bytes());
            json.extend_from_slice(b",\"velocity\":");
            json.extend_from_slice(vector_json(actor_velocity(actor)).as_bytes());
            json.extend_from_slice(b",\"state\":\"");
            escape(&fetch_text(actor, self.services.actor_state_name), &mut json);
            json.extend_from_slice(b"\"}");
        }
        json.push(b']');
        let mut out = Vec::new();
        observed(&json, &mut out);
        out
    }

    fn animation_observation(&self) -> Vec<u8> {
        let actors = self.active_actors();
        let mut json = vec![b'['];
        for (index, &actor) in actors.iter().enumerate() {
            if index > 0 {
                json.push(b',');
            }
            json.extend_from_slice(b"{\"id\":\"");
            escape(&object_path_text(actor), &mut json);
            json.extend_from_slice(b"\",\"sequence\":\"");
            escape(&fetch_text(actor, self.services.actor_anim_sequence_name), &mut json);
            json.extend_from_slice(b"\",\"frame_1024\":");
            json.extend_from_slice(quantize_1024(actor_anim_frame(actor)).to_string().as_bytes());
            json.extend_from_slice(b",\"finished\":");
            json.extend_from_slice(if actor_anim_finished(actor) { b"true" } else { b"false" });
            json.extend_from_slice(b"}");
        }
        json.push(b']');
        let mut out = Vec::new();
        observed(&json, &mut out);
        out
    }

    fn audio_observation(&self) -> Vec<u8> {
        let count = unsafe { (self.services.audio_fidelity_source_count)(self.engine) };
        let mut records = Vec::new();
        for index in 0..count {
            let mut buffer = vec![0u8; NAME_BUFFER_CAP];
            let len = unsafe {
                (self.services.audio_fidelity_source_id)(self.engine, index, buffer.as_mut_ptr(), buffer.len())
            };
            let id = if len > 0 {
                buffer.truncate((len as usize).min(buffer.len()));
                buffer
            } else {
                Vec::new()
            };
            let paused = unsafe { (self.services.audio_fidelity_source_paused)(self.engine, index) != 0 };
            records.push(AudioRecord {
                id,
                state: if paused { "paused" } else { "playing" },
            });
        }
        records.sort_by(|a, b| {
            let folded_a = fold(&a.id);
            let folded_b = fold(&b.id);
            if folded_a != folded_b {
                return folded_a.cmp(&folded_b);
            }
            if a.id != b.id {
                return a.id.cmp(&b.id);
            }
            a.state.cmp(b.state)
        });
        let mut json = vec![b'['];
        for (index, record) in records.iter().enumerate() {
            if index > 0 {
                json.push(b',');
            }
            json.extend_from_slice(b"{\"id\":\"");
            escape(&record.id, &mut json);
            json.extend_from_slice(b"\",\"state\":\"");
            json.extend_from_slice(record.state.as_bytes());
            json.extend_from_slice(b"\"}");
        }
        json.push(b']');
        let mut out = Vec::new();
        observed(&json, &mut out);
        out
    }

    fn cutscene_observation(&self, thread: *mut c_void, locator: Option<&RuntimeLocator>) -> Vec<u8> {
        let playing = bool_property(thread, b"bPlaying");
        let pending_available = locator.is_some()
            || (has_property(thread, b"aPendingCues") && has_property(thread, b"nPendingCues"));
        let pending_cues = self.string_array(thread, b"aPendingCues", b"nPendingCues");
        let mut json = Vec::new();
        if let Some(locator) = locator {
            json.extend_from_slice(b"{\"thread_slot\":");
            json.extend_from_slice(locator.thread_slot.to_string().as_bytes());
            json.extend_from_slice(b",\"line_index\":");
            json.extend_from_slice(locator.line_index.to_string().as_bytes());
        } else {
            json.extend_from_slice(b"{\"thread\":\"");
            escape(&object_path_text(thread), &mut json);
            json.extend_from_slice(b"\",\"thread_slot\":null,\"line_index\":null");
        }
        json.extend_from_slice(b",\"playing\":");
        match playing {
            Some(value) => json.extend_from_slice(if value { b"true" } else { b"false" }),
            None => json.extend_from_slice(b"null"),
        }
        json.extend_from_slice(b",\"pending_cues\":");
        if pending_available {
            Self::string_array_json(&pending_cues, &mut json);
        } else {
            json.extend_from_slice(b"null");
        }
        json.push(b'}');
        let mut out = Vec::new();
        observed(&json, &mut out);
        out
    }

    fn active_script_threads(&self) -> Vec<(*mut c_void, Option<RuntimeLocator>)> {
        let mut threads = Vec::new();
        for actor in self.active_actors() {
            if is_cut_script(actor) {
                threads.push((actor, self.build_runtime_locator(actor)));
            }
        }
        threads
    }

    fn active_cutscene_observation(&self, threads: &[(*mut c_void, Option<RuntimeLocator>)]) -> Vec<u8> {
        if threads.is_empty() {
            return CUTSCENE_ABSENT.to_vec();
        }
        let mut value = vec![b'['];
        for (index, (thread, locator)) in threads.iter().enumerate() {
            if index > 0 { value.push(b','); }
            value.extend_from_slice(&self.cutscene_observation(*thread, locator.as_ref()));
        }
        value.push(b']');
        let mut out = Vec::new();
        observed(&value, &mut out);
        out
    }

    fn active_cues_observation(&self, threads: &[(*mut c_void, Option<RuntimeLocator>)]) -> Vec<u8> {
        if threads.is_empty() {
            return CUTSCENE_ABSENT.to_vec();
        }
        let mut value = vec![b'['];
        for (index, (thread, _)) in threads.iter().enumerate() {
            if index > 0 { value.push(b','); }
            if has_property(*thread, b"aCues") && has_property(*thread, b"nCues") {
                Self::string_array_json(&self.string_array(*thread, b"aCues", b"nCues"), &mut value);
            } else {
                value.extend_from_slice(b"null");
            }
        }
        value.push(b']');
        let mut out = Vec::new();
        observed(&value, &mut out);
        out
    }

    fn cues_observation(&self, thread: *mut c_void) -> Vec<u8> {
        let cues = self.string_array(thread, b"aCues", b"nCues");
        let mut json = Vec::new();
        Self::string_array_json(&cues, &mut json);
        let mut out = Vec::new();
        observed(&json, &mut out);
        out
    }

    fn build_runtime_locator(&self, thread: *mut c_void) -> Option<RuntimeLocator> {
        let mut actor_buffer = [0u8; NAME_BUFFER_CAP];
        let mut path_buffer = [0u8; NAME_BUFFER_CAP];
        let mut result = Hp2RuntimeLocatorResult {
            kind: 0,
            thread_slot: -1,
            cursor: -1,
            actor_len: 0,
            path_len: 0,
        };
        let status = unsafe {
            (self.services.build_runtime_locator)(
                thread,
                actor_buffer.as_mut_ptr(),
                actor_buffer.len(),
                path_buffer.as_mut_ptr(),
                path_buffer.len(),
                &mut result,
            )
        };
        if status <= 0 || result.thread_slot < 0 || result.cursor < 0
            || result.actor_len <= 0 || result.actor_len as usize > actor_buffer.len()
            || result.path_len < 0 || result.path_len as usize > path_buffer.len()
        {
            return None;
        }
        let kind = match result.kind {
            1 => RuntimeKind::Disk,
            2 => RuntimeKind::Inline,
            _ => return None,
        };
        if kind == RuntimeKind::Disk && result.path_len == 0 {
            return None;
        }
        let mut full_path = Vec::new();
        if kind == RuntimeKind::Disk {
            full_path.reserve(16 + result.path_len as usize + 4);
            full_path.extend_from_slice(b"System/CUTSCENES/");
            full_path.extend_from_slice(&path_buffer[..result.path_len as usize]);
            full_path.extend_from_slice(b".int");
        }
        Some(RuntimeLocator {
            kind,
            actor: actor_buffer[..result.actor_len as usize].to_vec(),
            path: full_path,
            thread_slot: result.thread_slot,
            line_index: result.cursor,
        })
    }
    fn find_checkpoint(&self, locator: &RuntimeLocator, position: &str) -> Option<&FidelityCheckpoint> {
        self.checkpoints.iter().find(|checkpoint| {
            checkpoint.position == position
                && matches!(&checkpoint.trigger, CheckpointTrigger::Authored { kind, actor, path, thread_slot, line_index }
                    if *kind == locator.kind
                        && actor.as_bytes().eq_ignore_ascii_case(&locator.actor)
                        && path.as_deref().map_or(true, |path| path.as_bytes() == locator.path.as_slice())
                        && *thread_slot == locator.thread_slot
                        && *line_index == locator.line_index)
        })
    }

    fn close_pending(&mut self, thread: *mut c_void) {
        if let Some(index) = self.pending.iter().position(|boundary| boundary.thread == thread) {
            let boundary = self.pending.remove(index);
            self.capture(thread, &boundary.locator, "after");
        }
    }

    fn capture_state(&self, thread: *mut c_void, locator: &RuntimeLocator) -> Vec<u8> {
        let mut json = Vec::new();
        json.extend_from_slice(b"\"camera\":");
        json.extend_from_slice(&self.camera_observation(thread));
        json.extend_from_slice(b",\"actors\":");
        json.extend_from_slice(&self.actors_observation());
        json.extend_from_slice(b",\"cutscene\":");
        json.extend_from_slice(&self.cutscene_observation(thread, Some(locator)));
        json.extend_from_slice(b",\"cues\":");
        json.extend_from_slice(&self.cues_observation(thread));
        json.extend_from_slice(b",\"animation\":");
        json.extend_from_slice(&self.animation_observation());
        json.extend_from_slice(b",\"audio\":");
        json.extend_from_slice(&self.audio_observation());
        json
    }

    fn capture(&mut self, thread: *mut c_void, locator: &RuntimeLocator, position: &str) {
        let checkpoint = match self.find_checkpoint(locator, position) {
            Some(checkpoint) => checkpoint.clone(),
            None => return,
        };
        if self.snapshots.iter().any(|snapshot| snapshot.checkpoint.id == checkpoint.id) {
            return;
        }
        let state = self.capture_state(thread, locator);
        self.snapshots.push(FidelitySnapshot {
            checkpoint,
            state,
            frame_hash: FRAME_HASH_UNAVAILABLE.to_vec(),
        });
    }

    /// `FidelityReporter::OnCallEnter`.
    pub fn on_call_enter(&mut self, object: *mut c_void, function_name: &[u8]) {
        self.interpreted_calls += 1;
        if !self.manifest_present
            || !is_cut_script(object)
            || !function_name.eq_ignore_ascii_case(b"GetNextLine")
        {
            return;
        }
        self.close_pending(object);
        let locator = match self.build_runtime_locator(object) {
            Some(locator) => locator,
            None => return,
        };
        if self.find_checkpoint(&locator, "before").is_some()
            || self.find_checkpoint(&locator, "at").is_some()
            || self.find_checkpoint(&locator, "after").is_some()
        {
            self.capture(object, &locator, "before");
            self.pending.push(PendingBoundary { thread: object, locator });
        }
    }

    /// `FidelityReporter::OnCallExit`. `result` is `None` for a null
    /// `Result`, otherwise the dereferenced `UBOOL`.
    pub fn on_call_exit(&mut self, object: *mut c_void, function_name: &[u8], result: Option<bool>) {
        if !self.manifest_present
            || !is_cut_script(object)
            || !function_name.eq_ignore_ascii_case(b"GetNextCommand")
            || result != Some(true)
        {
            return;
        }
        if let Some(boundary) = self.pending.iter().find(|boundary| boundary.thread == object) {
            let locator = boundary.locator.clone();
            self.capture(object, &locator, "at");
        }
    }

    fn read_frame_hash(&self) -> Vec<u8> {
        let viewport = unsafe { (self.services.engine_viewport)(self.engine) };
        if viewport.is_null() {
            return FRAME_HASH_UNAVAILABLE.to_vec();
        }
        let render_device = unsafe { (self.services.viewport_render_device)(viewport) };
        let mut size_x = 0i32;
        let mut size_y = 0i32;
        unsafe { (self.services.viewport_size)(viewport, &mut size_x, &mut size_y) };
        if render_device.is_null() || size_x <= 0 || size_y <= 0 {
            return FRAME_HASH_UNAVAILABLE.to_vec();
        }
        let pixel_count = (size_x as usize) * (size_y as usize);
        let mut pixels = vec![[0u8; 4]; pixel_count];
        for (index, pixel) in pixels.iter_mut().enumerate() {
            let seed = ((index as u64).wrapping_mul(131).wrapping_add(17) & 255) as u8;
            *pixel = [seed, seed ^ 0x5a, seed ^ 0xa5, seed ^ 0xff];
        }
        unsafe { (self.services.read_pixels)(render_device, pixels.as_mut_ptr().cast::<u8>(), pixel_count) };
        let mut readback_changed = false;
        for (index, pixel) in pixels.iter().enumerate() {
            let seed = ((index as u64).wrapping_mul(131).wrapping_add(17) & 255) as u8;
            if pixel[0] != seed || pixel[1] != (seed ^ 0x5a) || pixel[2] != (seed ^ 0xa5) || pixel[3] != (seed ^ 0xff) {
                readback_changed = true;
                break;
            }
        }
        if !readback_changed {
            return FRAME_HASH_UNAVAILABLE.to_vec();
        }
        let mut hasher = Sha256::new();
        for pixel in &pixels {
            hasher.update([pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            hex.push_str(&format!("{byte:02x}"));
        }
        let mut value = Vec::new();
        observed(format!("\"{hex}\"").as_bytes(), &mut value);
        value
    }

    /// `FidelityReporter::AfterTick`.
    pub fn after_tick(&mut self) {
        self.tick_count += 1;
        let tick = self.tick_count;
        for index in 0..self.checkpoints.len() {
            let checkpoint = &self.checkpoints[index];
            if !matches!(&checkpoint.trigger, CheckpointTrigger::Tick { tick: target } if *target == tick) {
                continue;
            }
            if self.snapshots.iter().any(|snapshot| snapshot.checkpoint.id == checkpoint.id) {
                continue;
            }
            let state = self.capture_tick_state();
            let checkpoint = self.checkpoints[index].clone();
            self.snapshots.push(FidelitySnapshot {
                checkpoint,
                state,
                frame_hash: FRAME_HASH_UNAVAILABLE.to_vec(),
            });
        }
        if self.frame_hash_snapshot_count == self.snapshots.len() {
            return;
        }
        let hash = self.read_frame_hash();
        for snapshot in self.snapshots.iter_mut().skip(self.frame_hash_snapshot_count) {
            snapshot.frame_hash = hash.clone();
        }
        self.frame_hash_snapshot_count = self.snapshots.len();
    }

    fn capture_tick_state(&self) -> Vec<u8> {
        let threads = self.active_script_threads();
        let mut json = Vec::new();
        json.extend_from_slice(b"\"camera\":");
        json.extend_from_slice(&self.camera_observation(std::ptr::null_mut()));
        json.extend_from_slice(b",\"actors\":");
        json.extend_from_slice(&self.actors_observation());
        json.extend_from_slice(b",\"cutscene\":");
        json.extend_from_slice(&self.active_cutscene_observation(&threads));
        json.extend_from_slice(b",\"cues\":");
        json.extend_from_slice(&self.active_cues_observation(&threads));
        json.extend_from_slice(b",\"animation\":");
        json.extend_from_slice(&self.animation_observation());
        json.extend_from_slice(b",\"audio\":");
        json.extend_from_slice(&self.audio_observation());
        json
    }
    fn serialize_checkpoint(snapshot: &FidelitySnapshot, out: &mut Vec<u8>) {
        let checkpoint = &snapshot.checkpoint;
        out.extend_from_slice(b"{\"id\":\"");
        escape(checkpoint.id.as_bytes(), out);
        out.extend_from_slice(b"\",\"position\":\"");
        escape(checkpoint.position.as_bytes(), out);
        out.push(b'"');
        match &checkpoint.trigger {
            CheckpointTrigger::Authored { kind, actor, path, thread_slot, line_index } => {
                out.extend_from_slice(b",\"runtime_locator\":{\"kind\":\"");
                out.extend_from_slice(match kind {
                    RuntimeKind::Disk => b"disk",
                    RuntimeKind::Inline => b"inline",
                });
                out.extend_from_slice(b"\",\"actor\":\"");
                escape(actor.as_bytes(), out);
                out.extend_from_slice(b"\",");
                if let Some(path) = path {
                    out.extend_from_slice(b"\"path\":\"");
                    escape(path.as_bytes(), out);
                    out.extend_from_slice(b"\",");
                }
                out.extend_from_slice(b"\"thread_slot\":");
                out.extend_from_slice(thread_slot.to_string().as_bytes());
                out.extend_from_slice(if *kind == RuntimeKind::Disk {
                    b",\"line_index\":"
                } else {
                    b",\"character_offset\":"
                });
                out.extend_from_slice(line_index.to_string().as_bytes());
                out.push(b'}');
            }
            CheckpointTrigger::Tick { tick } => {
                out.extend_from_slice(b",\"tick\":");
                out.extend_from_slice(tick.to_string().as_bytes());
            }
        }
        out.push(b',');
        out.extend_from_slice(&snapshot.state);
        out.extend_from_slice(b",\"frame_hash\":");
        out.extend_from_slice(&snapshot.frame_hash);
        out.push(b'}');
    }

    fn serialize(&self, test_ticks: i32, fixed_delta_seconds: f32, reason: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"{\"version\":1,\"checkpoints\":[");
        for (index, snapshot) in self.snapshots.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            Self::serialize_checkpoint(snapshot, &mut out);
        }
        out.push(b']');
        if self.snapshots.is_empty() {
            out.extend_from_slice(b",\"diagnostic\":{\"code\":\"");
            out.extend_from_slice(reason.as_bytes());
            out.extend_from_slice(b"\"}");
        }
        out.extend_from_slice(b",\"run\":{\"test_ticks\":");
        out.extend_from_slice(test_ticks.to_string().as_bytes());
        out.extend_from_slice(b",\"fixed_dt\":");
        out.extend_from_slice(json::format_fixed(fixed_delta_seconds as f64, 6).as_bytes());
        out.extend_from_slice(b",\"ticks_observed\":");
        out.extend_from_slice(self.tick_count.to_string().as_bytes());
        out.extend_from_slice(b",\"interpreted_calls\":");
        out.extend_from_slice(self.interpreted_calls.to_string().as_bytes());
        out.extend_from_slice(b"}}\n");
        out
    }

    /// `FidelityReporter::Flush`.
    pub fn flush(&self, test_ticks: i32, fixed_delta_seconds: f32) {
        let reason = if !self.manifest_present {
            "fidelity.checkpoint_manifest_missing"
        } else if self.snapshots.is_empty() {
            "fidelity.checkpoint_not_reached"
        } else {
            "fidelity.checkpoints_captured"
        };
        let document = self.serialize(test_ticks, fixed_delta_seconds, reason);
        match write_report(self.report_path.as_bytes(), &document) {
            Ok(()) => {
                println!(
                    "<HP2_FIDELITY> report={} checkpoints={} reason={}",
                    self.report_path,
                    self.snapshots.len(),
                    reason
                );
            }
            Err(ReportWriteError::Unwritable) => {
                eprintln!("<HP2_FIDELITY> report_error=fidelity.report_unwritable path={}", self.report_path);
            }
            Err(ReportWriteError::WriteFailed) => {
                eprintln!("<HP2_FIDELITY> report_error=fidelity.report_write_failed path={}", self.report_path);
            }
        }
    }
}
