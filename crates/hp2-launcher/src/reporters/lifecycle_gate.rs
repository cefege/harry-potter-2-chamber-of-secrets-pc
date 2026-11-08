//! `LifecycleGateReporter`, ported from
//! `HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp:2953-3334`. Standalone:
//! no JSON scenario input, no shared `json` module dependency. Registers
//! itself as `GActorLifecycleObserver` (`Core.h`/`Core.cpp`) through the
//! `SDLLaunch.cpp` adapter, which keeps the class name, `IsEnabled`,
//! constructor/destructor, `BeginTick`/`Flush` signatures and every
//! `FActorLifecycleObserver` virtual override so `SDLLaunch.cpp`'s untouched
//! `FMainLoopState`/`MainLoop` call sites keep compiling.
//!
//! Engine reads go through [`Hp2LifecycleGateServices`] for the leaves with
//! no generic equivalent: `FStateFrame`/script-byte-range arithmetic
//! (`StatePcDiagnostic`), `GetStateFrame()->LatentAction`, and
//! `StableActorKey`'s typed `Cast<UNameProperty>` + `GetClass()->GetName() +
//! ":" + *Key` text. `ReadIntProperty`, `IsGateClass` and `IsGate` compose
//! `super::trace_services`'s published `find_int_property`/`object_class`/
//! `class_named`/`fname_ansi_or_empty` generics instead (no dedicated
//! leaves remain for these three).
//! `Actor->bDeleteMe` is a plain
//! field read through `hp2_abi::engine::generated::AActor`'s existing
//! bitfield accessor, same as in `world_collision_gate`. Any engine text an
//! `FActorLifecycleObserver` virtual call already carries (the lifecycle
//! `Event` literal, `BaseClass->GetName()`, `*Tag`) is resolved to ANSI text
//! by the adapter before crossing FFI, matching
//! `trace_services`'s "C++ resolves text into a static buffer, Rust copies
//! it" convention.

use crate::reporters::trace_services::{self, ReportWriteError, write_report};
use hp2_abi::engine::generated::UObject;
use std::ffi::c_void;

const COUNTER_COUNT: usize = 9;

const COUNTER_NAMES: [&str; COUNTER_COUNT] = [
    "SpawnedCount",
    "PreBeginPlayCount",
    "BeginPlayCount",
    "PostBeginPlayCount",
    "SetInitialStateCount",
    "BeginStateCount",
    "EndStateCount",
    "IteratorYieldCount",
    "DestroyedCount",
];

/// Engine leaves `LifecycleGateReporter` needs. Published once by
/// `SDLLaunch.cpp`'s adapter and captured by value at construction (same
/// rationale as `world_collision_gate::Hp2WorldCollisionGateServices`: a
/// single live instance per process).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LifecycleGateServices {
    /// `StableActorKey`: `""` if `!Actor->GetClass()` or no `StableKey`
    /// `UNameProperty`, else `Text(Actor->GetClass()->GetName())+":"+Text(*Key)`.
    /// Writes up to `cap` bytes (no NUL) into `buf`, returns the length
    /// actually available (same truncation contract as `snprintf`).
    pub stable_actor_key: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `StateName`: `Frame && Frame->StateNode && Frame->StateNode!=Actor->GetClass() ? Text(Frame->StateNode->GetName()) : "None"`;
    /// same buffer contract as `stable_actor_key`.
    pub state_name: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `StatePcDiagnostic`: returns 1 and writes `*out_offset` if
    /// `Frame && Frame->Node && Frame->Code && Frame->Node->Script.Num()`
    /// and `Frame->Code` falls within `Frame->Node->Script`'s byte range,
    /// else returns 0 and leaves `*out_offset` untouched.
    pub state_pc_diagnostic: unsafe extern "C" fn(actor: *mut c_void, out_offset: *mut i32) -> i32,
    /// `Actor->GetStateFrame() && Actor->GetStateFrame()->LatentAction==EPOLL_Sleep`.
    pub actor_state_latent_is_sleep: unsafe extern "C" fn(actor: *mut c_void) -> i32,
}

const NAME_BUFFER_CAP: usize = 1024;

struct Snapshot {
    id: &'static str,
    tick: i32,
    events: Vec<Vec<u8>>,
    state: Vec<u8>,
    pc_diagnostic: Option<i32>,
    latent_wake_tick: Option<i32>,
    iterator_yields: Option<Vec<Vec<u8>>>,
    counters: [i32; COUNTER_COUNT],
    state_stage: i32,
    live: bool,
    delete_marked: bool,
}

/// `LifecycleGateReporter`.
pub struct State {
    services: Hp2LifecycleGateServices,
    report_path: String,
    current_tick: i32,
    startup_events: Vec<Vec<u8>>,
    iterator_yields: Vec<Vec<u8>>,
    checkpoints: Vec<Snapshot>,
}


fn env_non_empty(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// `LifecycleGateReporter::IsEnabled`.
pub fn is_enabled() -> bool {
    env_non_empty("HP2_LIFECYCLE_GATE_TRACE").is_some()
}

/// `LifecycleGateReporter::LifecycleGateReporter`.
pub fn create(services: Hp2LifecycleGateServices) -> Box<State> {
    Box::new(State {
        services,
        report_path: env_non_empty("HP2_LIFECYCLE_GATE_TRACE").unwrap_or_default(),
        current_tick: 0,
        startup_events: Vec::new(),
        iterator_yields: Vec::new(),
        checkpoints: Vec::new(),
    })
}

fn object_class(object: *mut c_void) -> *mut c_void {
    if object.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(object as *mut UObject)).class }
}

/// Owned NUL-terminated copy of `name`, for leaves that still want
/// `*const c_char` (the engine's `name`/`name_len` convention took a
/// UTF-8 `(ptr,len)` pair).
fn nul_terminated(name: &[u8]) -> Vec<u8> {
    let mut owned = Vec::with_capacity(name.len() + 1);
    owned.extend_from_slice(name);
    owned.push(0);
    owned
}

/// `Object && Object->GetClass() && !appStricmp(Object->GetClass()->GetName(),Name)`.
fn class_named(svc: &trace_services::Hp2TraceReporterServices, class: *mut c_void, name: &[u8]) -> bool {
    if class.is_null() {
        return false;
    }
    unsafe { trace_services::object_name(svc, class) }.eq_ignore_ascii_case(name)
}

/// `Actor && Actor->GetClass() && appStricmp(Actor->GetClass()->GetName(),TEXT("LifecycleGateActor"))==0`.
/// Replaces the old dedicated `is_gate_class` leaf by composing the
/// already-published generic `object_class`/`object_name` leaves (same
/// pattern as `fidelity::is_cut_script_disk`).
fn is_gate_class(actor: *mut c_void) -> bool {
    if actor.is_null() {
        return false;
    }
    match unsafe { trace_services::published_services() } {
        Ok(svc) => class_named(&svc, object_class(actor), b"LifecycleGateActor"),
        Err(_) => false,
    }
}

/// `IsGateClass(Actor) && appStricmp(*Actor->Tag,TEXT("LifecycleGate"))==0`.
/// Replaces the old dedicated `is_gate` leaf: `fname_ansi_or_empty` maps
/// `NAME_None` to `""` rather than the literal `"None"` `appToAnsi(*Tag)`
/// would produce, but this call site only ever compares the result against
/// the fixed non-empty, non-"None" literal `"LifecycleGate"`, so both
/// possible `NAME_None` encodings agree on the (false) outcome.
fn is_gate(actor: *mut c_void) -> bool {
    if !is_gate_class(actor) {
        return false;
    }
    match unsafe { trace_services::published_services() } {
        Ok(svc) => {
            let tag_index = unsafe { (svc.actor_tag_fname)(actor) };
            let tag = unsafe { trace_services::ansi((svc.fname_ansi_or_empty)(tag_index)) };
            tag.eq_ignore_ascii_case(b"LifecycleGate")
        }
        Err(_) => false,
    }
}

/// `UIntProperty* P = Cast<UIntProperty>(FindField<UProperty>(Actor->GetClass(),Name)); P ? *(INT*)((BYTE*)Actor+P->Offset) : 0`.
/// Replaces the old dedicated `read_int_property` leaf by composing the
/// already-published generic `find_int_property`/`property_offset`
/// leaves (same pattern as `fidelity::bool_property`/`float_property`);
/// `name` is always one of this module's own ASCII literals.
fn read_int_property(actor: *mut c_void, name: &str) -> i32 {
    if actor.is_null() {
        return 0;
    }
    let class = object_class(actor);
    if class.is_null() {
        return 0;
    }
    match unsafe { trace_services::published_services() } {
        Ok(svc) => {
            let owned = nul_terminated(name.as_bytes());
            let field = unsafe { (svc.find_int_property)(class, owned.as_ptr() as *const std::ffi::c_char) };
            if field.is_null() {
                return 0;
            }
            let offset = unsafe { (svc.property_offset)(field) };
            unsafe { trace_services::read_i32_at(actor, offset) }
        }
        Err(_) => 0,
    }
}

impl State {
    fn is_gate_class(&self, actor: *mut c_void) -> bool {
        is_gate_class(actor)
    }

    fn is_gate(&self, actor: *mut c_void) -> bool {
        is_gate(actor)
    }

    fn read_int_property(&self, actor: *mut c_void, name: &str) -> i32 {
        read_int_property(actor, name)
    }

    fn stable_actor_key(&self, actor: *mut c_void) -> Vec<u8> {
        let mut buffer = [0u8; NAME_BUFFER_CAP];
        let len = unsafe { (self.services.stable_actor_key)(actor, buffer.as_mut_ptr(), buffer.len()) };
        if len <= 0 {
            return Vec::new();
        }
        let len = (len as usize).min(buffer.len());
        buffer[..len].to_vec()
    }

    fn state_name(&self, actor: *mut c_void) -> Vec<u8> {
        let mut buffer = [0u8; NAME_BUFFER_CAP];
        let len = unsafe { (self.services.state_name)(actor, buffer.as_mut_ptr(), buffer.len()) };
        if len <= 0 {
            return b"None".to_vec();
        }
        let len = (len as usize).min(buffer.len());
        buffer[..len].to_vec()
    }

    fn state_pc_diagnostic(&self, actor: *mut c_void) -> Option<i32> {
        let mut offset = 0i32;
        let present = unsafe { (self.services.state_pc_diagnostic)(actor, &mut offset) != 0 };
        present.then_some(offset)
    }

    fn actor_state_latent_is_sleep(&self, actor: *mut c_void) -> bool {
        if actor.is_null() {
            return false;
        }
        unsafe { (self.services.actor_state_latent_is_sleep)(actor) != 0 }
    }

    /// `LifecycleGateReporter::BeginTick`.
    pub fn begin_tick(&mut self, tick: i32) {
        self.current_tick = tick;
    }

    /// `LifecycleGateReporter::OnActorLifecycleEvent`; `event` is `None` for
    /// a null `const TCHAR* Event`.
    pub fn on_actor_lifecycle_event(&mut self, actor: *mut c_void, event: Option<&[u8]>) {
        if self.is_gate_class(actor) {
            if let Some(event) = event {
                self.startup_events.push(event.to_vec());
            }
        }
    }

    /// `LifecycleGateReporter::OnActorSpawned`.
    pub fn on_actor_spawned(&mut self, actor: *mut c_void) {
        if self.is_gate_class(actor) {
            self.capture("spawn", actor, false);
        }
    }

    /// `LifecycleGateReporter::OnActorProcessState`.
    pub fn on_actor_process_state(&mut self, actor: *mut c_void) {
        if self.is_gate(actor) && self.current_tick == 0 && self.actor_state_latent_is_sleep(actor) {
            self.capture("await_resume", actor, false);
        }
    }

    /// `LifecycleGateReporter::OnActorDestroying`.
    pub fn on_actor_destroying(&mut self, actor: *mut c_void) {
        if self.is_gate(actor) {
            self.capture("destroy", actor, false);
        }
    }

    /// `LifecycleGateReporter::OnActorAllActors`; `base_class_name` and
    /// `tag` are the adapter's already-resolved `BaseClass->GetName()`
    /// (`None` for a null `BaseClass`) and `*Tag` ANSI text. The native
    /// reporter receives `slot` but does not include it in its contract.
    pub fn on_actor_all_actors(
        &mut self,
        requestor: *mut c_void,
        base_class_name: Option<&[u8]>,
        tag: &[u8],
        yielded: *mut c_void,
        exhausted: bool,
    ) {
        if !self.is_gate(requestor) {
            return;
        }
        let Some(base_class_name) = base_class_name else {
            return;
        };
        if !base_class_name.eq_ignore_ascii_case(b"Actor") || !tag.eq_ignore_ascii_case(b"LifecycleGateProbe") {
            return;
        }
        if !yielded.is_null() {
            let key = self.stable_actor_key(yielded);
            self.iterator_yields.push(key);
        } else if exhausted {
            self.capture("scan", requestor, true);
        }
    }

    fn has_checkpoint(&self, id: &str) -> bool {
        self.checkpoints.iter().any(|snapshot| snapshot.id == id)
    }

    /// `LifecycleGateReporter::Capture`.
    fn capture(&mut self, id: &'static str, actor: *mut c_void, include_iterator: bool) {
        if actor.is_null() || self.has_checkpoint(id) {
            return;
        }
        let events = match id {
            "spawn" => self.startup_events.clone(),
            "scan" => vec![
                b"AllActorsYield".to_vec(),
                b"AllActorsYield".to_vec(),
                b"AllActorsExhausted".to_vec(),
            ],
            "await_resume" => vec![b"EndState".to_vec(), b"BeginState".to_vec(), b"Sleep".to_vec()],
            "destroy" => vec![b"EndState".to_vec(), b"Destroyed".to_vec(), b"DestroyActor".to_vec()],
            _ => Vec::new(),
        };
        let mut counters = [0i32; COUNTER_COUNT];
        for (index, name) in COUNTER_NAMES.iter().enumerate() {
            counters[index] = self.read_int_property(actor, name);
        }
        let state_stage = self.read_int_property(actor, "StateStage");
        let state = self.state_name(actor);
        let pc_diagnostic = self.state_pc_diagnostic(actor);
        let has_sleep = self.actor_state_latent_is_sleep(actor);
        let latent_wake_tick = has_sleep.then_some(self.current_tick + 1);
        let iterator_yields = include_iterator.then(|| self.iterator_yields.clone());
        let delete_marked = unsafe { (*(actor as *mut hp2_abi::engine::generated::AActor)).b_delete_me() };
        self.checkpoints.push(Snapshot {
            id,
            tick: self.current_tick,
            events,
            state,
            pc_diagnostic,
            latent_wake_tick,
            iterator_yields,
            counters,
            state_stage,
            live: !delete_marked,
            delete_marked,
        });
    }

    fn counters_json(counters: &[i32; COUNTER_COUNT], out: &mut Vec<u8>) {
        out.push(b'{');
        for (index, name) in COUNTER_NAMES.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            push_json_string(name.as_bytes(), out);
            out.push(b':');
            out.extend_from_slice(counters[index].to_string().as_bytes());
        }
        out.push(b'}');
    }

    fn events_json(events: &[Vec<u8>], out: &mut Vec<u8>) {
        out.push(b'[');
        for (index, event) in events.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            push_json_string(event, out);
        }
        out.push(b']');
    }

    fn properties_json(snapshot: &Snapshot, out: &mut Vec<u8>) {
        Self::counters_json(&snapshot.counters, out);
        out.pop();
        out.extend_from_slice(b",\"StateStage\":");
        out.extend_from_slice(snapshot.state_stage.to_string().as_bytes());
        out.push(b'}');
    }

    fn iterator_json(snapshot: &Snapshot, out: &mut Vec<u8>) {
        let Some(yields) = &snapshot.iterator_yields else {
            out.extend_from_slice(b"null");
            return;
        };
        out.extend_from_slice(b"{\"base_class\":\"Actor\",\"tag\":\"LifecycleGateProbe\",\"yields\":[");
        for (index, yielded) in yields.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            push_json_string(yielded, out);
        }
        out.extend_from_slice(b"],\"exhausted\":true,\"output\":null}");
    }

    fn snapshot_json(snapshot: &Snapshot, out: &mut Vec<u8>) {
        out.extend_from_slice(b"{\"id\":");
        push_json_string(snapshot.id.as_bytes(), out);
        out.extend_from_slice(b",\"tick\":");
        out.extend_from_slice(snapshot.tick.to_string().as_bytes());
        out.extend_from_slice(b",\"events\":");
        Self::events_json(&snapshot.events, out);
        out.extend_from_slice(b",\"frame\":{\"state\":");
        push_json_string(&snapshot.state, out);
        out.extend_from_slice(b",\"pc_diagnostic\":");
        match snapshot.pc_diagnostic {
            Some(offset) => out.extend_from_slice(offset.to_string().as_bytes()),
            None => out.extend_from_slice(b"null"),
        }
        out.extend_from_slice(b",\"latent\":");
        match snapshot.latent_wake_tick {
            Some(wake_tick) => {
                out.extend_from_slice(b"{\"kind\":\"Sleep\",\"wake_tick\":");
                out.extend_from_slice(wake_tick.to_string().as_bytes());
                out.push(b'}');
            }
            None => out.extend_from_slice(b"null"),
        }
        out.extend_from_slice(b"},\"iterator\":");
        Self::iterator_json(snapshot, out);
        out.extend_from_slice(b",\"properties\":");
        Self::properties_json(snapshot, out);
        out.extend_from_slice(b",\"live\":");
        out.extend_from_slice(if snapshot.live { b"true" } else { b"false" });
        out.extend_from_slice(b",\"delete_marked\":");
        out.extend_from_slice(if snapshot.delete_marked { b"true" } else { b"false" });
        out.push(b'}');
    }

    /// `LifecycleGateReporter::Serialize`.
    fn serialize(&self, requested_ticks: i32, fixed_delta_seconds: f32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"{\"version\":1,\"fixture\":\"LifecycleGate\",\"fixed_dt\":");
        out.extend_from_slice(crate::reporters::json::format_g(fixed_delta_seconds as f64, 9).as_bytes());
        out.extend_from_slice(b",\"requested_ticks\":");
        out.extend_from_slice(requested_ticks.to_string().as_bytes());
        out.extend_from_slice(
            b",\"actor\":{\"class\":\"LifecycleGateActor\",\"tag\":\"LifecycleGate\"},\"checkpoints\":[",
        );
        for (index, snapshot) in self.checkpoints.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            Self::snapshot_json(snapshot, &mut out);
        }
        out.extend_from_slice(b"],\"random_calls\":[]}\n");
        out
    }

    /// `LifecycleGateReporter::Flush`.
    pub fn flush(&self, requested_ticks: i32, fixed_delta_seconds: f32) {
        let document = self.serialize(requested_ticks, fixed_delta_seconds);
        match write_report(self.report_path.as_bytes(), &document) {
            Ok(()) => {
                println!(
                    "<HP2_LIFECYCLE_GATE> report={} checkpoints={}",
                    self.report_path,
                    self.checkpoints.len()
                );
            }
            Err(ReportWriteError::Unwritable) => {
                eprintln!("<HP2_LIFECYCLE_GATE> report_error=unwritable path={}", self.report_path);
            }
            Err(ReportWriteError::WriteFailed) => {
                eprintln!("<HP2_LIFECYCLE_GATE> report_error=write_failed path={}", self.report_path);
            }
        }
    }
}

/// `LifecycleGateReporter::JsonString`: same escapes as other reporters'
/// `JsonString`, plus `\u%04x` for control bytes below `0x20` that aren't
/// one of the named escapes -- the one reporter among the four whose
/// `JsonString` has this extra branch (checked against the other three
/// `JsonString`/`Escape` bodies in `SDLLaunch.cpp`).
fn push_json_string(value: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    for &byte in value {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0c => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            byte if byte < 0x20 => out.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
            byte => out.push(byte),
        }
    }
    out.push(b'"');
}
