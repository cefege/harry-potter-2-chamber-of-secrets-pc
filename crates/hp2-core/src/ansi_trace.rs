//! Rand-trace core: oracle `UnAnsi.cpp` lines 152-780 (state, helpers, capture,
//! record, tick) and 1441-1995 (scopes, phase, startup map, flush, seed, reset,
//! ordinal, RNG).
//!
//! Every function runs under the process-wide trace-state mutex. The Core owner
//! uses copied Rust snapshots; legacy FFI callers use [`Hp2AnsiTraceServices`].
//! Oracle null dereferences stay raw inside the legacy service rows, preserving
//! their original behavior for mixed-runtime callers.
//!
//! The oracle's `GAppRandTraceContext`, `GAppRandTracePreBeginEvent` and
//! `GAppRandTraceRelevanceEvent` are `thread_local`; they are thread-local here too.

use std::cell::Cell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr::{null, null_mut, read_unaligned};

use crate::ansi::{self, Tchar, Threw};
use crate::ansi_trace_write::{write_rand_trace, write_startup_trace};
use crate::mem::wide_z;

use std::sync::{LazyLock, Mutex, MutexGuard};

static PROCESS_TRACE: LazyLock<Mutex<TraceState>> =
    LazyLock::new(|| Mutex::new(TraceState::default()));

fn process_trace() -> MutexGuard<'static, TraceState> {
    PROCESS_TRACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Runs a trace operation against the process-wide trace state.
///
/// The closure must not reenter trace state. Engine callbacks and output hooks
/// are therefore not invoked by direct VM RNG recording.
pub fn with_process_state<R>(operation: impl FnOnce(&mut TraceState) -> R) -> R {
    let mut state = process_trace();
    if !state.initialized {
        initialize(&mut state);
        // Bootstrap can precede the first trace operation. Bind only once, after
        // tracing has read its own environment and established its gate state.
        if let Some((requested, effective)) = crate::unix_rng_bootstrap::diagnostic_rng_seed_state() {
            set_diagnostic_seed(&mut state, requested, effective);
        }
    }
    operation(&mut state)
}

/// Runs a trace operation without initializing the process-wide state.
pub fn with_process_state_raw<R>(operation: impl FnOnce(&mut TraceState) -> R) -> R {
    let mut state = process_trace();
    operation(&mut state)
}

// Bootstrap also handles the opposite order: tracing might already exist.
// Do not initialize it here or read its environment early.
pub(crate) fn publish_bootstrapped_diagnostic_seed(requested: u32, effective: u32) {
    with_process_state_raw(|state| {
        if state.initialized {
            set_diagnostic_seed(state, requested, effective);
        }
    });
}

/// Publishes the source `appSetRandTraceTick` operation to the process trace
/// without crossing a C service table.
pub fn set_process_tick(tick_index: u64, phase: &'static [u8]) {
    with_process_state(|state| set_tick(state, tick_index, Some(phase)));
}

/// Publishes the source `appSetStartupRandTraceMap` operation from canonical
/// UTF-32 URL fields, using the same strict `appToUtf8InPlace` conversion as
/// the ANSI service table.
pub fn set_process_startup_map(token: &[u32], url: &[u32]) -> crate::core_access::RuntimeResult<()> {
    fn ansi(input: &[u32]) -> crate::core_access::RuntimeResult<Vec<u8>> {
        let mut wide = input.iter().copied().take_while(|&unit| unit != 0).collect::<Vec<_>>();
        wide.push(0);
        let capacity = wide.len().saturating_mul(4).saturating_add(1);
        let capacity_i32 = i32::try_from(capacity)
            .map_err(|_| crate::core_access::RuntimeError::new("Startup map URL exceeds ANSI conversion capacity"))?;
        let mut output = vec![0u8; capacity];
        let converted = unsafe {
            crate::ansi::to_utf8_in_place(output.as_mut_ptr().cast(), wide.as_ptr().cast(), capacity_i32)
        };
        if converted == 0 {
            return Err(crate::core_access::RuntimeError::new("Could not convert startup map URL to UTF-8"));
        }
        let length = output.iter().position(|&byte| byte == 0).unwrap_or(output.len());
        output.truncate(length);
        Ok(output)
    }
    let token = ansi(token)?;
    let url = ansi(url)?;
    with_process_state(|state| {
        if !state.startup_phase_enabled || state.flushed { return; }
        state.startup_map_token = map_token(Some(&token));
        state.startup_map_url = copy_str(Some(&url), MAP_URL_CAPACITY, b"");
        state.startup_lifecycle_boundary = copy_str(Some(b"map_loaded"), MAP_URL_CAPACITY, b"map_loaded");
        state.startup_load_ordinal += 1;
    });
    Ok(())
}

/// Publishes the source `appSetStartupRandTraceLifecycleBoundary` operation
/// without routing through native service tables.
pub fn set_process_startup_lifecycle_boundary(boundary: &[u8]) {
    with_process_state(|state| set_startup_lifecycle_boundary(state, Some(boundary)));
}


/// Advances the canonical RNG and records its draw without an FFI service table.
pub fn direct_rand_value(st: &mut TraceState) -> i32 {
    let value = crate::rng_private::rand();
    if !st.enabled {
        if st.ordinal_enabled && !st.flushed {
            st.call_count += 1;
        }
        return value;
    }
    if st.flushed {
        return value;
    }
    record_direct(st, value);
    value
}

/// Flushes process trace output using the canonical reporter.
pub fn flush_process_state() {
    with_process_state(flush);
}

/// Applies the canonical direct draw formula and returns the recorded integer
/// and mapped value from that same draw.
pub fn direct_rand_range_float_with_raw(st: &mut TraceState, min: f32, max: f32) -> (i32, f32) {
    let raw = direct_rand_value(st);
    let unit = raw as f32 / RAND_MAX as f32;
    (raw, (max - min).mul_add(unit, min))
}

/// Applies the canonical direct draw formula while recording exactly one RNG draw.
pub fn direct_rand_range_float(st: &mut TraceState, min: f32, max: f32) -> f32 {
    direct_rand_range_float_with_raw(st, min, max).1
}

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
}

pub const RAND_MAX: i32 = 0x7fff_ffff;
pub const PATH_MAX: usize = 1024;
pub const DEFAULT_LIMIT: usize = 16384;
pub const MAX_LIMIT: usize = 262144;
pub const MAX_CONTEXT_BYTES: usize = 32 * 1024 * 1024;
pub const PHASE_CAPACITY: usize = 48;
pub const MAP_CAPACITY: usize = 256;
pub const MAP_URL_CAPACITY: usize = 1024;
pub const MAX_STARTUP_PHASES: usize = 32;
pub const INDEX_NONE: i32 = -1;

/// Services table (contract section 3). Rows are filled by the C++ adapters in
/// `Hp2AnsiTraceServices.cpp`. Engine objects and value types cross as `c_void`.
/// Row 13 returns a slot from a 128-entry ring; callers copy the bytes at once.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2AnsiTraceServices {
    pub obj_class: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub obj_outer: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub obj_name: unsafe extern "C" fn(*const c_void) -> *const c_void,
    pub obj_path_name: unsafe extern "C" fn(*const c_void, *mut *const c_void) -> i32,
    pub find_field: unsafe extern "C" fn(*const c_void, *const c_void, i32) -> *mut c_void,
    pub is_a: unsafe extern "C" fn(*const c_void, i32) -> i32,
    pub prop_offset: unsafe extern "C" fn(*const c_void) -> i32,
    pub prop_bit_mask: unsafe extern "C" fn(*const c_void) -> u32,
    pub prop_export_text:
        unsafe extern "C" fn(*const c_void, *mut c_void, i32, *const c_void, *mut i32) -> i32,
    pub frame_node: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub frame_object: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub frame_locals: unsafe extern "C" fn(*const c_void) -> *mut u8,
    pub to_ansi: unsafe extern "C" fn(*const c_void, *mut *const c_char) -> i32,
    pub check_fail: unsafe extern "C" fn(*const c_char, i32) -> i32,
    pub fname_text: unsafe extern "C" fn(*const c_void) -> *const c_void,
    pub struct_children: unsafe extern "C" fn(*const c_void) -> *mut c_void,
}

static mut TRACE_SERVICES: Option<Hp2AnsiTraceServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2AnsiTraceServices) {
    unsafe {
        (&raw mut TRACE_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2AnsiTraceServices, i32> {
    unsafe { (&raw const TRACE_SERVICES).read().ok_or(1) }
}

/// Maps a service status (0 ok, 1 raised) to `Result`. On `Err` the caller makes no
/// further service calls; the C++ adapter has already recorded the pending exception.
pub(crate) fn status(code: i32) -> Result<(), Threw> {
    if code == 0 { Ok(()) } else { Err(Threw) }
}

// The services table is the ABI boundary. Its methods take pointers the oracle would
// pass to the same engine call; those pointers are valid by the caller's contract.
impl Hp2AnsiTraceServices {
    fn class_of(&self, obj: *const c_void) -> *mut c_void {
        unsafe { (self.obj_class)(obj) }
    }
    fn outer_of(&self, obj: *const c_void) -> *mut c_void {
        unsafe { (self.obj_outer)(obj) }
    }
    fn name_of(&self, obj: *const c_void) -> *const Tchar {
        unsafe { (self.obj_name)(obj).cast() }
    }
    fn path_of(&self, obj: *const c_void) -> Result<*const Tchar, Threw> {
        let mut out = null();
        status(unsafe { (self.obj_path_name)(obj, &mut out) })?;
        Ok(out.cast())
    }
    fn field(&self, owner: *const c_void, name: *const Tchar, kind: i32) -> *mut c_void {
        unsafe { (self.find_field)(owner, name.cast(), kind) }
    }
    fn is_kind(&self, obj: *const c_void, kind: i32) -> bool {
        unsafe { (self.is_a)(obj, kind) != 0 }
    }
    fn offset_of(&self, prop: *const c_void) -> usize {
        unsafe { (self.prop_offset)(prop) as usize }
    }
    fn mask_of(&self, prop: *const c_void) -> u32 {
        unsafe { (self.prop_bit_mask)(prop) }
    }
    fn node_of(&self, frame: *const c_void) -> *mut c_void {
        unsafe { (self.frame_node)(frame) }
    }
    fn object_of(&self, frame: *const c_void) -> *mut c_void {
        unsafe { (self.frame_object)(frame) }
    }
    fn locals_of(&self, frame: *const c_void) -> *mut u8 {
        unsafe { (self.frame_locals)(frame) }
    }
    fn ansi(&self, text: *const Tchar) -> Result<*const c_char, Threw> {
        let mut out = null();
        status(unsafe { (self.to_ansi)(text.cast(), &mut out) })?;
        Ok(out)
    }
    fn children_of(&self, owner: *const c_void) -> *mut c_void {
        unsafe { (self.struct_children)(owner) }
    }
    /// Returns true when the hook raised.
    fn check_failed(&self, expr: *const c_char, line: i32) -> bool {
        unsafe { (self.check_fail)(expr, line) != 0 }
    }
}

// ---- state types (contract section 5) ----

#[derive(Default)]
pub struct Sample {
    pub ordinal: u64,
    pub value: i32,
    pub tick_index: u64,
    pub phase: Vec<u8>,
    pub has_consumer: bool,
    pub actor: Option<Vec<u8>>,
    pub class: Option<Vec<u8>>,
    pub function: Option<Vec<u8>>,
    pub native: Option<Vec<u8>>,
    pub native_slot: i32,
    pub callsite_offset: i32,
    pub has_outer_consumer: bool,
    pub outer_actor: Option<Vec<u8>>,
    pub outer_class: Option<Vec<u8>>,
    pub outer_function: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct TickBoundary {
    pub index: u64,
    pub phase: Vec<u8>,
}

#[derive(Default)]
pub struct PhaseEvent {
    pub phase: Vec<u8>,
    pub map_token: Vec<u8>,
    pub map_url: Vec<u8>,
    pub lifecycle_boundary: Vec<u8>,
    pub load_ordinal: u64,
    pub ordinal_before: u64,
    pub ordinal_after: u64,
}

#[derive(Default)]
pub struct FunctionEvent {
    pub call_id: u64,
    pub entry_ordinal: u64,
    pub exit_ordinal: u64,
    pub has_exit: bool,
    pub function: Option<Vec<u8>>,
    pub receiver: Option<Vec<u8>>,
    pub actor: Option<Vec<u8>>,
    pub root_actor: Option<Vec<u8>>,
    pub root_class: Option<Vec<u8>>,
    pub root_function: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct RelevanceEvent {
    pub previous: Option<usize>,
    pub entry_ordinal: u64,
    pub exit_ordinal: u64,
    pub has_exit: bool,
    pub has_always_keep_result: bool,
    pub always_keep_result: bool,
    pub has_is_relevant_result: bool,
    pub is_relevant_result: bool,
    pub has_super_relevant: bool,
    pub super_relevant: u8,
    pub has_final_result: bool,
    pub final_result: bool,
    pub return_source_line: i32,
    pub early_return_branch: Option<&'static [u8]>,
    pub direct_call_depth: i32,
    pub function: Option<Vec<u8>>,
    pub receiver: Option<Vec<u8>>,
    pub other: Option<Vec<u8>>,
    pub other_class: Option<Vec<u8>>,
    pub base_mutator: Option<Vec<u8>>,
    pub base_mutator_class: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct PreBeginEvent {
    pub previous: Option<usize>,
    /// Engine actor pointer. Read only under the state lock.
    pub actor_object: *const c_void,
    pub entry_ordinal: u64,
    pub exit_ordinal: u64,
    pub has_exit: bool,
    pub game_relevant: bool,
    pub relevant_in_software_renderer: bool,
    pub delete_me_at_entry: bool,
    pub delete_me_at_exit: bool,
    pub has_net_mode: bool,
    pub net_mode: i32,
    pub has_software_rendering: bool,
    pub is_software_rendering: bool,
    pub relevance_will_dispatch: bool,
    pub relevance_dispatched: bool,
    pub relevance_completed: bool,
    pub has_relevance_result: bool,
    pub relevance_result: bool,
    pub has_relevance_ordinals: bool,
    pub relevance_entry_ordinal: u64,
    pub relevance_exit_ordinal: u64,
    pub relevance_app_rand_consumed: bool,
    pub has_level_game: bool,
    pub actor: Option<Vec<u8>>,
    pub function: Option<Vec<u8>>,
    pub level_game: Option<Vec<u8>>,
}

/// Global rand-trace state. Guarded by the FFI layer's mutex.
#[derive(Default)]
pub struct TraceState {
    pub initialized: bool,
    pub enabled: bool,
    pub ordinal_enabled: bool,
    pub storage_available: bool,
    pub truncated: bool,
    pub flushed: bool,
    pub replay_seeded: bool,
    pub diagnostic_seeded: bool,
    pub diagnostic_seed: u32,
    pub diagnostic_requested_seed: u32,
    pub path: Vec<u8>,
    pub limit: usize,
    pub count: usize,
    pub tick_boundary_count: usize,
    pub context_bytes: usize,
    pub call_count: u64,
    pub tick_index: u64,
    pub phase: Vec<u8>,
    pub samples: Vec<Sample>,
    pub tick_boundaries: Vec<TickBoundary>,
    pub startup_phase_enabled: bool,
    pub startup_phase_path: Vec<u8>,
    pub startup_phase_count: usize,
    pub startup_phases: Vec<PhaseEvent>,
    pub startup_map_token: Vec<u8>,
    pub startup_map_url: Vec<u8>,
    pub startup_lifecycle_boundary: Vec<u8>,
    pub startup_load_ordinal: u64,
    pub function_event_count: usize,
    pub function_call_count: u64,
    pub function_events: Vec<FunctionEvent>,
    pub prebegin_event_count: usize,
    pub prebegin_events: Vec<PreBeginEvent>,
    pub relevance_event_count: usize,
    pub relevance_events: Vec<RelevanceEvent>,
}

// `actor_object` is an engine pointer. It is only touched under the state lock.
unsafe impl Send for TraceState {}

/// Oracle `FAppRandTraceContext`: node of the thread-local context chain.
pub struct Context {
    previous: *mut Context,
    object: *const c_void,
    function: *const c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
    direct_outer_actor: Option<Vec<u8>>,
    direct_outer_class: Option<Vec<u8>>,
    direct_outer_function: Option<Vec<u8>>,
    direct_native_actor: Option<Vec<u8>>,
    direct_native_class: Option<Vec<u8>>,
    direct_native_function: Option<Vec<u8>>,
    direct_native_name: Option<Vec<u8>>,
}

/// Scope handle: `FAppRandTracePreBeginScope`.
pub struct PreBeginScope {
    event: usize,
}

/// Scope handle: `FAppRandTracePhaseScope`.
pub struct PhaseScope {
    event: usize,
}

/// Scope handle: `FAppRandTraceRelevanceScope`.
pub struct RelevanceScope {
    event: usize,
}

/// Scope handle: `FAppRandTraceFunctionScope`.
pub struct FunctionScope {
    event: Option<usize>,
    prebegin_event: Option<usize>,
}

/// Scope handle: `FAppRandTracePreBeginRelevanceScope`.
pub struct PreBeginRelevanceScope {
    event: usize,
    result: *const c_void,
}

/// Scope handle: `FAppRandTraceRelevanceCallScope`.
pub struct RelevanceCallScope {
    event: usize,
    result: *const c_void,
    super_relevant: *const u8,
    capture_kind: i32,
}

thread_local! {
    static CONTEXT: Cell<*mut Context> = const { Cell::new(null_mut()) };
    static PREBEGIN_EVENT: Cell<Option<usize>> = const { Cell::new(None) };
    static RELEVANCE_EVENT: Cell<Option<usize>> = const { Cell::new(None) };
}

// ---- byte helpers ----

fn cbytes<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        None
    } else {
        // SAFETY: non-null C strings come from the services table or the environment.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes())
    }
}

fn env_nonempty(name: &CStr) -> Option<Vec<u8>> {
    // SAFETY: `name` is NUL-terminated; getenv returns a stable pointer or null.
    let p = unsafe { getenv(name.as_ptr()) };
    cbytes(p).filter(|b| !b.is_empty()).map(<[u8]>::to_vec)
}

/// Oracle `appCopyRandTraceString`: null or empty source yields the default; at most `capacity-1` bytes.
fn copy_str(src: Option<&[u8]>, capacity: usize, default: &[u8]) -> Vec<u8> {
    let s = match src {
        Some(s) if !s.is_empty() => s,
        _ => default,
    };
    s[..s.len().min(capacity - 1)].to_vec()
}

/// Oracle `appSetRandTraceMapToken`: text after the last `/` or `\`, up to the first `.`, clamped.
fn map_token(src: Option<&[u8]>) -> Vec<u8> {
    let s = src.unwrap_or(b"");
    let start = s
        .iter()
        .rposition(|&c| c == b'/' || c == b'\\')
        .map_or(0, |i| i + 1);
    let rest = &s[start..];
    let len = rest.iter().position(|&c| c == b'.').unwrap_or(rest.len());
    rest[..len.min(MAP_CAPACITY - 1)].to_vec()
}

/// Oracle `appDuplicateRandTraceString`: `None` duplicates as "None"; accounts bytes against the cap.
fn dup(st: &mut TraceState, value: Option<&[u8]>) -> Option<Vec<u8>> {
    let value = value.unwrap_or(b"None");
    let len = value.len() + 1;
    if len > MAX_CONTEXT_BYTES - st.context_bytes {
        return None;
    }
    st.context_bytes += len;
    Some(value.to_vec())
}

/// Oracle `appDuplicateRandTraceObjectPath`.
fn dup_path(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    obj: *const c_void,
) -> Result<Option<Vec<u8>>, Threw> {
    if obj.is_null() {
        return Ok(dup(st, None));
    }
    let text = svc.ansi(svc.path_of(obj)?)?;
    Ok(dup(st, cbytes(text)))
}

fn dup_cstr(st: &mut TraceState, text: *const c_char) -> Option<Vec<u8>> {
    dup(st, cbytes(text))
}

fn release_string(st: &mut TraceState, s: &mut Option<Vec<u8>>) {
    if let Some(v) = s.take() {
        st.context_bytes -= v.len() + 1;
    }
}

fn release_sample(st: &mut TraceState, s: &mut Sample) {
    release_string(st, &mut s.actor);
    release_string(st, &mut s.class);
    release_string(st, &mut s.function);
    release_string(st, &mut s.native);
    release_string(st, &mut s.outer_actor);
    release_string(st, &mut s.outer_class);
    release_string(st, &mut s.outer_function);
    *s = Sample::default();
}

fn release_function_event(st: &mut TraceState, e: &mut FunctionEvent) {
    release_string(st, &mut e.function);
    release_string(st, &mut e.receiver);
    release_string(st, &mut e.actor);
    release_string(st, &mut e.root_actor);
    release_string(st, &mut e.root_class);
    release_string(st, &mut e.root_function);
    *e = FunctionEvent::default();
}

fn release_relevance_event(st: &mut TraceState, e: &mut RelevanceEvent) {
    release_string(st, &mut e.function);
    release_string(st, &mut e.receiver);
    release_string(st, &mut e.other);
    release_string(st, &mut e.other_class);
    release_string(st, &mut e.base_mutator);
    release_string(st, &mut e.base_mutator_class);
    *e = RelevanceEvent::default();
}

fn release_prebegin_event(st: &mut TraceState, e: &mut PreBeginEvent) {
    release_string(st, &mut e.actor);
    release_string(st, &mut e.function);
    release_string(st, &mut e.level_game);
    *e = PreBeginEvent::default();
}

fn alloc_array<T: Default>(n: usize) -> Vec<T> {
    let mut v = Vec::new();
    if v.try_reserve_exact(n).is_ok() {
        v.resize_with(n, T::default);
        v
    } else {
        Vec::new()
    }
}

/// Oracle `appRandTraceLimit`.
fn rand_trace_limit() -> usize {
    let Some(value) = env_nonempty(c"HP2_RNG_TRACE_LIMIT") else {
        return DEFAULT_LIMIT;
    };
    if !value.iter().all(u8::is_ascii_digit) {
        return DEFAULT_LIMIT;
    }
    let mut parsed: u64 = 0;
    for &d in &value {
        parsed = match parsed
            .checked_mul(10)
            .and_then(|p| p.checked_add(u64::from(d - b'0')))
        {
            Some(p) => p,
            None => return MAX_LIMIT,
        };
    }
    if parsed > MAX_LIMIT as u64 {
        MAX_LIMIT
    } else {
        parsed as usize
    }
}

// ---- tick and init ----

fn append_tick_boundary(st: &mut TraceState) {
    if st.tick_boundary_count >= st.limit || st.tick_boundaries.is_empty() {
        if st.limit > 0 {
            st.truncated = true;
        }
        return;
    }
    let i = st.tick_boundary_count;
    st.tick_boundary_count += 1;
    st.tick_boundaries[i] = TickBoundary {
        index: st.tick_index,
        phase: st.phase.clone(),
    };
}

fn reset_tick_state(st: &mut TraceState) {
    st.tick_index = 0;
    st.phase = copy_str(None, PHASE_CAPACITY, b"startup");
    st.tick_boundary_count = 0;
    append_tick_boundary(st);
}

/// Oracle `appInitRandTrace`. Callers hold the state lock.
pub fn initialize(st: &mut TraceState) {
    st.phase = b"startup".to_vec();
    st.startup_lifecycle_boundary = b"map_loaded".to_vec();
    st.startup_phases = (0..MAX_STARTUP_PHASES)
        .map(|_| PhaseEvent::default())
        .collect();

    let startup = env_nonempty(c"HP2_STARTUP_RNG_PHASE_TRACE");
    let global_tick = env_nonempty(c"HP2_GLOBAL_TICK_TRACE");
    let fire_texture = env_nonempty(c"HP2_FIRE_TEXTURE_TRACE");
    if let Some(path) = startup
        && path.len() < PATH_MAX
    {
        st.startup_phase_path = path;
        st.startup_phase_enabled = true;
        st.ordinal_enabled = true;
    }
    // Fire's InitTables oracle needs the real global appRand interval but intentionally
    // does not request a separate RNG artifact.
    st.ordinal_enabled =
        global_tick.is_some() || fire_texture.is_some() || st.startup_phase_enabled;
    if let Some(path) = env_nonempty(c"HP2_RNG_TRACE")
        && path.len() < PATH_MAX
    {
        st.path = path;
        st.limit = rand_trace_limit();
        st.enabled = true;
        st.ordinal_enabled = true;
        if st.limit > 0 {
            let n = st.limit;
            st.samples = alloc_array(n);
            st.tick_boundaries = alloc_array(n);
            st.function_events = alloc_array(n);
            st.prebegin_events = alloc_array(n);
            st.relevance_events = alloc_array(n);
        }
        st.storage_available = st.limit == 0
            || (!st.samples.is_empty()
                && !st.tick_boundaries.is_empty()
                && !st.function_events.is_empty()
                && !st.prebegin_events.is_empty()
                && !st.relevance_events.is_empty());
        reset_tick_state(st);
    }
    st.initialized = true;
}

// ---- consumer capture and record ----

/// Oracle `appFindRandTraceNativeContext`.
fn find_native_context() -> *mut Context {
    let top = CONTEXT.get();
    // SAFETY: context nodes are live while they are on the thread-local chain.
    if !top.is_null() && unsafe { !(*top).native.is_null() } {
        top
    } else {
        null_mut()
    }
}

/// Oracle `appFindRandTraceRootContext`: the outermost non-native node.
fn find_root_context() -> *mut Context {
    let mut root = null_mut();
    let mut ctx = CONTEXT.get();
    while !ctx.is_null() {
        // SAFETY: context nodes are live while they are on the thread-local chain.
        unsafe {
            if (*ctx).native.is_null() {
                root = ctx;
            }
            ctx = (*ctx).previous;
        }
    }
    root
}

fn capture_consumer(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    sample: &mut Sample,
) -> Result<bool, Threw> {
    let native = find_native_context();
    if !native.is_null() {
        // SAFETY: non-null native node is live on the thread-local chain.
        let ctx = unsafe { &*native };
        sample.has_consumer = true;
        sample.actor = dup_path(svc, st, ctx.object)?;
        let class = if ctx.object.is_null() {
            null()
        } else {
            svc.class_of(ctx.object)
        };
        sample.class = dup_path(svc, st, class)?;
        sample.function = dup_path(svc, st, ctx.function)?;
        sample.native = dup_cstr(st, ctx.native);
        sample.native_slot = ctx.native_slot;
        sample.callsite_offset = ctx.callsite_offset;
        if sample.actor.is_none()
            || sample.class.is_none()
            || sample.function.is_none()
            || sample.native.is_none()
        {
            release_sample(st, sample);
            return Ok(false);
        }
    }

    let root = find_root_context();
    if !root.is_null() {
        // SAFETY: root node is live on the thread-local chain.
        let ctx = unsafe { &*root };
        sample.has_outer_consumer = true;
        sample.outer_actor = dup_path(svc, st, ctx.object)?;
        let class = if ctx.object.is_null() {
            null()
        } else {
            svc.class_of(ctx.object)
        };
        sample.outer_class = dup_path(svc, st, class)?;
        sample.outer_function = dup_path(svc, st, ctx.function)?;
        if sample.outer_actor.is_none()
            || sample.outer_class.is_none()
            || sample.outer_function.is_none()
        {
            release_sample(st, sample);
            return Ok(false);
        }
    }
    Ok(true)
}

fn capture_direct_consumer(st: &mut TraceState, sample: &mut Sample) -> bool {
    let native = find_native_context();
    if !native.is_null() {
        // SAFETY: the native context is live on this thread's scope chain.
        let context = unsafe { &*native };
        sample.has_consumer = true;
        sample.actor = dup(st, context.direct_native_actor.as_deref());
        sample.class = dup(st, context.direct_native_class.as_deref());
        sample.function = dup(st, context.direct_native_function.as_deref());
        sample.native = dup(st, context.direct_native_name.as_deref());
        sample.native_slot = context.native_slot;
        sample.callsite_offset = context.callsite_offset;
        if sample.actor.is_none()
            || sample.class.is_none()
            || sample.function.is_none()
            || sample.native.is_none()
        {
            release_sample(st, sample);
            return false;
        }
    }
    let root = find_root_context();
    if !root.is_null() {
        // SAFETY: root is a live context node on this thread's scope chain.
        let context = unsafe { &*root };
        sample.has_outer_consumer = true;
        sample.outer_actor = dup(st, context.direct_outer_actor.as_deref());
        sample.outer_class = dup(st, context.direct_outer_class.as_deref());
        sample.outer_function = dup(st, context.direct_outer_function.as_deref());
        if sample.outer_actor.is_none()
            || sample.outer_class.is_none()
            || sample.outer_function.is_none()
        {
            release_sample(st, sample);
            return false;
        }
    }
    true
}

fn record_sample<E>(
    st: &mut TraceState,
    value: i32,
    capture: impl FnOnce(&mut TraceState, &mut Sample) -> Result<bool, E>,
) -> Result<(), E> {
    let ordinal = st.call_count;
    st.call_count += 1;
    if st.count >= st.limit || st.samples.is_empty() {
        st.truncated = true;
        return Ok(());
    }
    let mut sample = Sample {
        ordinal,
        value,
        tick_index: st.tick_index,
        phase: st.phase.clone(),
        ..Sample::default()
    };
    if !capture(st, &mut sample)? {
        st.truncated = true;
        return Ok(());
    }
    let index = st.count;
    st.samples[index] = sample;
    st.count += 1;
    Ok(())
}

/// Oracle `appRecordRandTrace`.
fn record(svc: &Hp2AnsiTraceServices, st: &mut TraceState, value: i32) -> Result<(), Threw> {
    record_sample(st, value, |st, sample| capture_consumer(svc, st, sample))
}

fn record_direct(st: &mut TraceState, value: i32) {
    let result = record_sample(st, value, |st, sample| {
        Ok::<bool, std::convert::Infallible>(capture_direct_consumer(st, sample))
    });
    if let Err(never) = result {
        match never {}
    }
}

// ---- engine reads ----

fn name_is(svc: &Hp2AnsiTraceServices, obj: *const c_void, lit: &str) -> bool {
    let w = wide_z(lit);
    // SAFETY: `w` is NUL-terminated; `ansi::strcmp` is wcscmp.
    unsafe { ansi::strcmp(svc.name_of(obj), w.as_ptr()) == 0 }
}

/// Oracle `appIsGameInfoIsRelevant`.
fn is_game_info_is_relevant(svc: &Hp2AnsiTraceServices, function: *const c_void) -> bool {
    if function.is_null() {
        return false;
    }
    let owner = svc.outer_of(function);
    !owner.is_null() && name_is(svc, function, "IsRelevant") && name_is(svc, owner, "GameInfo")
}

/// Oracle `appIsActorPreBeginPlay`.
fn is_actor_prebegin_play(svc: &Hp2AnsiTraceServices, function: *const c_void) -> bool {
    if function.is_null() {
        return false;
    }
    let owner = svc.outer_of(function);
    !owner.is_null() && name_is(svc, function, "PreBeginPlay") && name_is(svc, owner, "Actor")
}

/// Oracle `Cast<UFunction>`: the node if it is a UFunction, else null.
fn cast_fn(svc: &Hp2AnsiTraceServices, node: *mut c_void) -> *const c_void {
    if svc.is_kind(node, 0) {
        node as *const c_void
    } else {
        null()
    }
}

fn class_or_null(svc: &Hp2AnsiTraceServices, obj: *const c_void) -> *const c_void {
    if obj.is_null() {
        null()
    } else {
        svc.class_of(obj)
    }
}

/// Oracle `FindField<UBoolProperty>` read. Null objects yield false.
fn read_bool(svc: &Hp2AnsiTraceServices, obj: *const c_void, name: &str) -> bool {
    if obj.is_null() {
        return false;
    }
    let w = wide_z(name);
    let prop = svc.field(svc.class_of(obj), w.as_ptr(), 1);
    if prop.is_null() {
        return false;
    }
    // SAFETY: the property offset addresses a field of `obj` (services row 7).
    let word = unsafe { read_unaligned((obj as *const u8).add(svc.offset_of(prop)) as *const u32) };
    (word & svc.mask_of(prop)) != 0
}

/// Oracle `FindField<UObjectProperty>` read. Null objects yield null.
fn read_object(svc: &Hp2AnsiTraceServices, obj: *const c_void, name: &str) -> *mut c_void {
    if obj.is_null() {
        return null_mut();
    }
    let w = wide_z(name);
    let prop = svc.field(svc.class_of(obj), w.as_ptr(), 2);
    if prop.is_null() {
        return null_mut();
    }
    // SAFETY: the property offset addresses an object pointer field of `obj`.
    unsafe { read_unaligned((obj as *const u8).add(svc.offset_of(prop)) as *const *mut c_void) }
}

/// Oracle `FindField<UByteProperty>` read. Null objects yield None.
fn read_byte(svc: &Hp2AnsiTraceServices, obj: *const c_void, name: &str) -> Option<i32> {
    if obj.is_null() {
        return None;
    }
    let w = wide_z(name);
    let prop = svc.field(svc.class_of(obj), w.as_ptr(), 3);
    if prop.is_null() {
        return None;
    }
    // SAFETY: the property offset addresses a byte of `obj`.
    Some(unsafe { *(obj as *const u8).add(svc.offset_of(prop)) } as i32)
}

/// Oracle `appFindGameInfoRelevantActor`: the `Other` parameter of an IsRelevant frame.
fn find_game_info_relevant_actor(
    svc: &Hp2AnsiTraceServices,
    function: *const c_void,
    frame: *const c_void,
) -> *mut c_void {
    let prop = if function.is_null() {
        null_mut()
    } else {
        svc.children_of(function)
    };
    if prop.is_null() || !name_is(svc, prop, "Other") || !svc.is_kind(prop, 2) {
        return null_mut();
    }
    // SAFETY: frame locals plus the property offset address the Other pointer slot.
    unsafe { read_unaligned(svc.locals_of(frame).add(svc.offset_of(prop)) as *const *mut c_void) }
}

/// Oracle `appGetGameInfoRelevanceReturnBranch`.
fn branch_name(ev: &RelevanceEvent) -> &'static [u8] {
    if ev.has_always_keep_result && ev.always_keep_result {
        return b"always_keep";
    }
    if ev.has_is_relevant_result && !ev.is_relevant_result {
        return b"mutator_is_relevant_rejected";
    }
    if ev.has_is_relevant_result
        && ev.is_relevant_result
        && ev.has_super_relevant
        && ev.super_relevant == 1
    {
        return b"mutator_super_relevant";
    }
    match ev.return_source_line {
        453 => b"always_keep",
        457 => b"mutator_super_relevant",
        459 => b"mutator_is_relevant_rejected",
        469 => b"difficulty_or_network_filter",
        472 => b"no_monsters_filter",
        475 => b"odds_of_appearing",
        487 => b"relevant",
        _ => b"unknown",
    }
}

// ---- event begin and end ----

fn begin_actor_prebegin_trace(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    obj: *const c_void,
    function: *const c_void,
) -> Result<Option<usize>, Threw> {
    let relevant = is_actor_prebegin_play(svc, function);
    if !relevant || st.prebegin_event_count >= st.limit || st.prebegin_events.is_empty() {
        if relevant && st.limit > 0 {
            st.truncated = true;
        }
        return Ok(None);
    }
    let level = read_object(svc, obj, "Level");
    let level_game = read_object(svc, level, "Game");
    let mut event = PreBeginEvent {
        actor_object: obj,
        entry_ordinal: st.call_count,
        has_level_game: !level_game.is_null(),
        game_relevant: read_bool(svc, obj, "bGameRelevant"),
        relevant_in_software_renderer: read_bool(svc, obj, "bRelevantInSoftwareRenderer"),
        delete_me_at_entry: read_bool(svc, obj, "bDeleteMe"),
        ..PreBeginEvent::default()
    };
    let net_mode = read_byte(svc, level, "NetMode");
    event.has_net_mode = net_mode.is_some();
    event.net_mode = net_mode.unwrap_or(0);
    event.relevance_will_dispatch =
        !event.game_relevant && event.has_net_mode && event.net_mode != 3;
    event.actor = dup_path(svc, st, obj)?;
    event.function = dup_path(svc, st, function)?;
    event.level_game = dup_path(svc, st, level_game)?;
    if event.actor.is_none() || event.function.is_none() || event.level_game.is_none() {
        release_prebegin_event(st, &mut event);
        st.truncated = true;
        return Ok(None);
    }
    let index = st.prebegin_event_count;
    event.previous = PREBEGIN_EVENT.get();
    PREBEGIN_EVENT.set(Some(index));
    st.prebegin_events[index] = event;
    st.prebegin_event_count += 1;
    Ok(Some(index))
}

fn begin_prebegin_relevance_trace(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    function: *const c_void,
    frame: *const c_void,
) -> Option<usize> {
    if !is_game_info_is_relevant(svc, function) {
        return None;
    }
    let actor = find_game_info_relevant_actor(svc, function, frame) as *const c_void;
    let index = PREBEGIN_EVENT.get()?;
    if st.prebegin_events[index].actor_object == actor {
        st.prebegin_events[index].relevance_dispatched = true;
        Some(index)
    } else {
        None
    }
}

fn begin_game_info_is_relevant_trace(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    obj: *const c_void,
    function: *const c_void,
    frame: *const c_void,
) -> Result<Option<usize>, Threw> {
    let relevant = is_game_info_is_relevant(svc, function);
    if !relevant || st.function_event_count >= st.limit || st.function_events.is_empty() {
        if relevant && st.limit > 0 {
            st.truncated = true;
        }
        return Ok(None);
    }
    let call_id = st.function_call_count;
    st.function_call_count += 1;
    let mut event = FunctionEvent {
        call_id,
        entry_ordinal: st.call_count,
        ..FunctionEvent::default()
    };
    event.function = dup_path(svc, st, function)?;
    event.receiver = dup_path(svc, st, obj)?;
    let actor = find_game_info_relevant_actor(svc, function, frame);
    event.actor = dup_path(svc, st, actor)?;
    let root = find_root_context();
    // SAFETY: a non-null root node is live on the thread-local chain.
    let (root_object, root_function) = if root.is_null() {
        (null(), null())
    } else {
        unsafe { ((*root).object, (*root).function) }
    };
    event.root_actor = dup_path(svc, st, root_object)?;
    event.root_class = dup_path(svc, st, class_or_null(svc, root_object))?;
    event.root_function = dup_path(svc, st, root_function)?;
    if event.function.is_none()
        || event.receiver.is_none()
        || event.actor.is_none()
        || event.root_actor.is_none()
        || event.root_class.is_none()
        || event.root_function.is_none()
    {
        release_function_event(st, &mut event);
        st.truncated = true;
        return Ok(None);
    }
    let index = st.function_event_count;
    st.function_events[index] = event;
    st.function_event_count += 1;
    Ok(Some(index))
}

fn begin_game_info_relevance_trace(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    obj: *const c_void,
    function: *const c_void,
    frame: *const c_void,
) -> Result<Option<usize>, Threw> {
    let relevant = is_game_info_is_relevant(svc, function);
    if !relevant || st.relevance_event_count >= st.limit || st.relevance_events.is_empty() {
        if relevant && st.limit > 0 {
            st.truncated = true;
        }
        return Ok(None);
    }
    let other = find_game_info_relevant_actor(svc, function, frame);
    let base_mutator = read_object(svc, obj, "BaseMutator");
    let mut event = RelevanceEvent {
        entry_ordinal: st.call_count,
        return_source_line: INDEX_NONE,
        ..RelevanceEvent::default()
    };
    event.function = dup_path(svc, st, function)?;
    event.receiver = dup_path(svc, st, obj)?;
    event.other = dup_path(svc, st, other)?;
    event.other_class = dup_path(svc, st, class_or_null(svc, other))?;
    event.base_mutator = dup_path(svc, st, base_mutator)?;
    event.base_mutator_class = dup_path(svc, st, class_or_null(svc, base_mutator))?;
    if event.function.is_none()
        || event.receiver.is_none()
        || event.other.is_none()
        || event.other_class.is_none()
        || event.base_mutator.is_none()
        || event.base_mutator_class.is_none()
    {
        release_relevance_event(st, &mut event);
        st.truncated = true;
        return Ok(None);
    }
    let index = st.relevance_event_count;
    event.previous = RELEVANCE_EVENT.get();
    RELEVANCE_EVENT.set(Some(index));
    st.relevance_events[index] = event;
    st.relevance_event_count += 1;
    Ok(Some(index))
}

/// Oracle `check(RecordedEvent->DirectCallDepth>0)` at UnAnsi.cpp line 1744.
fn check_relevance_depth(svc: &Hp2AnsiTraceServices, depth: i32) -> Result<(), Threw> {
    if depth > 0 {
        return Ok(());
    }
    if svc.check_failed(c"RecordedEvent->DirectCallDepth>0".as_ptr(), 1744) {
        Err(Threw)
    } else {
        Ok(())
    }
}

// ---- record and tick ----

/// Oracle `appSetRandTraceTick`.
pub fn set_tick(st: &mut TraceState, tick_index: u64, phase: Option<&[u8]>) {
    if !st.enabled || st.flushed {
        return;
    }
    st.tick_index = tick_index;
    st.phase = copy_str(phase, PHASE_CAPACITY, b"startup");
    append_tick_boundary(st);
}

/// Oracle `appRand`: returns the next value and records it when enabled.
pub fn rand_value(svc: &Hp2AnsiTraceServices, st: &mut TraceState) -> Result<i32, Threw> {
    let value = crate::rng_private::rand();
    if !st.enabled {
        if st.ordinal_enabled && !st.flushed {
            st.call_count += 1;
        }
        return Ok(value);
    }
    if !st.flushed {
        record(svc, st, value)?;
    }
    Ok(value)
}

/// Oracle `appFrand()`.
pub fn frand(svc: &Hp2AnsiTraceServices, st: &mut TraceState) -> Result<f32, Threw> {
    Ok(rand_value(svc, st)? as f32 / RAND_MAX as f32)
}

/// Oracle `appFrand(From,To)` (oracle git 4f69478, UnAnsi.cpp:1979): compiled to `fmadd s0,s1,s0,s9`.
pub fn frand_from_to(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    from: f32,
    to: f32,
) -> Result<f32, Threw> {
    let r = rand_value(svc, st)? as f32 / RAND_MAX as f32;
    Ok((to - from).mul_add(r, from))
}

/// Oracle `appRandRange(FLOAT,FLOAT)` (oracle git 4f69478, UnAnsi.cpp:1984): compiled to `fmadd s0,s9,s0,s8`.
pub fn rand_range_float(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    min: f32,
    max: f32,
) -> Result<f32, Threw> {
    Ok((max - min).mul_add(frand(svc, st)?, min))
}

/// Oracle `appRandRange(INT,INT)`.
pub fn rand_range_int(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    min: i32,
    max: i32,
) -> Result<i32, Threw> {
    let range = max.wrapping_sub(min);
    let r = if range > 0 {
        rand_value(svc, st)? % range
    } else {
        0
    };
    Ok(r.wrapping_add(min))
}

/// Oracle `appGetRandTraceOrdinal`.
pub fn get_ordinal(st: &TraceState) -> u64 {
    if st.ordinal_enabled { st.call_count } else { 0 }
}

/// Oracle `appSetRandTraceDiagnosticSeed`.
pub fn set_diagnostic_seed(st: &mut TraceState, requested_seed: u32, seed: u32) {
    if !st.enabled && !st.ordinal_enabled {
        return;
    }
    if !st.flushed {
        st.diagnostic_seed = seed;
        st.diagnostic_requested_seed = requested_seed;
        st.diagnostic_seeded = true;
    }
}

/// Oracle `appSetStartupRandTraceLifecycleBoundary`.
pub fn set_startup_lifecycle_boundary(st: &mut TraceState, boundary: Option<&[u8]>) {
    if !st.startup_phase_enabled || st.flushed {
        return;
    }
    st.startup_lifecycle_boundary = copy_str(boundary, MAP_URL_CAPACITY, b"map_loaded");
}

/// Oracle `appSetStartupRandTraceMap`.
///
/// # Safety
/// `token` and `url` are null or valid NUL-terminated `Tchar` strings.
pub unsafe fn set_startup_map(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    token: *const Tchar,
    url: *const Tchar,
) -> Result<(), Threw> {
    if !st.startup_phase_enabled || st.flushed {
        return Ok(());
    }
    let token_text = cbytes(svc.ansi(token)?).map(<[u8]>::to_vec);
    let url_text = cbytes(svc.ansi(url)?).map(<[u8]>::to_vec);
    st.startup_map_token = map_token(token_text.as_deref());
    st.startup_map_url = copy_str(url_text.as_deref(), MAP_URL_CAPACITY, b"");
    st.startup_lifecycle_boundary = copy_str(Some(b"map_loaded"), MAP_URL_CAPACITY, b"map_loaded");
    st.startup_load_ordinal += 1;
    Ok(())
}

/// Oracle `appFlushRandTrace`.
pub fn flush(st: &mut TraceState) {
    if !st.enabled && !st.startup_phase_enabled {
        return;
    }
    if st.flushed {
        return;
    }
    st.flushed = true;
    if st.startup_phase_enabled {
        write_startup_trace(st);
    }
    if st.enabled {
        write_rand_trace(st);
    }
    for i in 0..st.count {
        let mut s = std::mem::take(&mut st.samples[i]);
        release_sample(st, &mut s);
    }
    for i in 0..st.function_event_count {
        let mut e = std::mem::take(&mut st.function_events[i]);
        release_function_event(st, &mut e);
    }
    for i in 0..st.relevance_event_count {
        let mut e = std::mem::take(&mut st.relevance_events[i]);
        release_relevance_event(st, &mut e);
    }
    for i in 0..st.prebegin_event_count {
        let mut e = std::mem::take(&mut st.prebegin_events[i]);
        release_prebegin_event(st, &mut e);
    }
    st.samples = Vec::new();
    st.tick_boundaries = Vec::new();
    st.function_events = Vec::new();
    st.prebegin_events = Vec::new();
    st.relevance_events = Vec::new();
}

/// Oracle `appResetRandTraceForReplay` body after the not-initialized check.
/// Callers hold the state lock.
pub fn reset_for_replay(st: &mut TraceState) {
    if !st.initialized {
        let any = env_nonempty(c"HP2_RNG_TRACE").is_some()
            || env_nonempty(c"HP2_GLOBAL_TICK_TRACE").is_some()
            || env_nonempty(c"HP2_STARTUP_RNG_PHASE_TRACE").is_some();
        if !any {
            return;
        }
        initialize(st);
    }
    if !st.enabled && !st.ordinal_enabled {
        return;
    }
    if st.flushed {
        return;
    }
    for i in 0..st.count {
        let mut s = std::mem::take(&mut st.samples[i]);
        release_sample(st, &mut s);
    }
    st.count = 0;
    st.call_count = 0;
    st.truncated = false;
    for i in 0..st.function_event_count {
        let mut e = std::mem::take(&mut st.function_events[i]);
        release_function_event(st, &mut e);
    }
    st.function_event_count = 0;
    st.function_call_count = 0;
    for i in 0..st.relevance_event_count {
        let mut e = std::mem::take(&mut st.relevance_events[i]);
        release_relevance_event(st, &mut e);
    }
    st.relevance_event_count = 0;
    RELEVANCE_EVENT.set(None);
    for i in 0..st.prebegin_event_count {
        let mut e = std::mem::take(&mut st.prebegin_events[i]);
        release_prebegin_event(st, &mut e);
    }
    st.prebegin_event_count = 0;
    PREBEGIN_EVENT.set(None);
    st.replay_seeded = true;
    reset_tick_state(st);
}

// ---- scope begin and end ----
/// Pushes a Core-owned outer consumer scope with copied path snapshots.
pub fn direct_outer_scope_begin(
    st: &TraceState,
    object: *const c_void,
    function: *const c_void,
    actor: Vec<u8>,
    class: Vec<u8>,
    function_path: Vec<u8>,
) -> *mut Context {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    let context = push_context(object, function, null(), INDEX_NONE, INDEX_NONE);
    // SAFETY: the new node is current and owned by its returned scope handle.
    unsafe {
        (*context).direct_outer_actor = Some(actor);
        (*context).direct_outer_class = Some(class);
        (*context).direct_outer_function = Some(function_path);
    }
    context
}

/// Pushes the direct VM native scope with copied consumer identity.
pub fn direct_native_scope_begin(
    st: &TraceState,
    object: *const c_void,
    function: *const c_void,
    native: &'static CStr,
    native_slot: i32,
    callsite_offset: i32,
    actor: Vec<u8>,
    class: Vec<u8>,
    function_path: Vec<u8>,
) -> *mut Context {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    let context = push_context(
        object,
        function,
        native.as_ptr(),
        native_slot,
        callsite_offset,
    );
    // SAFETY: the new node is current and freed by its matching scope guard.
    unsafe {
        (*context).direct_native_actor = Some(actor);
        (*context).direct_native_class = Some(class);
        (*context).direct_native_function = Some(function_path);
        (*context).direct_native_name = Some(native.to_bytes().to_vec());
    }
    context
}

/// Oracle `FAppRandTraceOuterScope` constructor. Returns the pushed node or null.
pub fn outer_scope_begin(
    st: &TraceState,
    object: *const c_void,
    function: *const c_void,
) -> *mut Context {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    push_context(object, function, null(), INDEX_NONE, INDEX_NONE)
}

/// Oracle `FAppRandTraceNativeScope` constructor (frame object and function).
///
/// # Safety
/// `stack` is a valid `FFrame*`. `native` is a null or NUL-terminated string that
/// outlives the scope.
pub unsafe fn native_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &TraceState,
    stack: *const c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
) -> *mut Context {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    let object = svc.object_of(stack) as *const c_void;
    let function = cast_fn(svc, svc.node_of(stack));
    push_context(object, function, native, native_slot, callsite_offset)
}

/// Oracle `FAppRandTraceNativeObjectScope` constructor.
pub fn native_object_scope_begin(
    st: &TraceState,
    object: *const c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
) -> *mut Context {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    push_context(object, null(), native, native_slot, callsite_offset)
}

fn push_context(
    object: *const c_void,
    function: *const c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
) -> *mut Context {
    let previous = CONTEXT.get();
    let node = Box::into_raw(Box::new(Context {
        previous,
        object,
        function,
        native,
        native_slot,
        callsite_offset,
        direct_outer_actor: None,
        direct_outer_class: None,
        direct_outer_function: None,
        direct_native_actor: None,
        direct_native_class: None,
        direct_native_function: None,
        direct_native_name: None,
    }));
    CONTEXT.set(node);
    node
}

/// Oracle outer, native and native-object destructors. Null handles are no-ops.
///
/// # Safety
/// `node` is null or a handle returned by a `*_scope_begin` in this module, ended once.
pub unsafe fn context_scope_end(node: *mut Context) {
    if node.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw in push_context and is ended once.
    let node = unsafe { Box::from_raw(node) };
    CONTEXT.set(node.previous);
}

/// Oracle `FAppRandTracePreBeginScope` constructor.
///
/// # Safety
/// `object` and `function` are valid engine pointers or null.
pub unsafe fn prebegin_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    object: *const c_void,
    function: *const c_void,
) -> Result<*mut PreBeginScope, Threw> {
    if !st.enabled || st.flushed || !is_actor_prebegin_play(svc, function) {
        return Ok(null_mut());
    }
    Ok(
        match begin_actor_prebegin_trace(svc, st, object, function)? {
            Some(event) => Box::into_raw(Box::new(PreBeginScope { event })),
            None => null_mut(),
        },
    )
}

/// Oracle `FAppRandTracePreBeginScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`prebegin_scope_begin`], ended once.
pub unsafe fn prebegin_scope_end(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    handle: *mut PreBeginScope,
) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if st.flushed {
        return;
    }
    let actor = st.prebegin_events[scope.event].actor_object;
    let delete_me_at_exit = read_bool(svc, actor, "bDeleteMe");
    let exit_ordinal = st.call_count;
    let event = &mut st.prebegin_events[scope.event];
    event.delete_me_at_exit = delete_me_at_exit;
    event.exit_ordinal = exit_ordinal;
    event.has_exit = true;
    PREBEGIN_EVENT.set(event.previous);
    event.actor_object = null();
}

/// Oracle `FAppRandTracePreBeginRelevanceScope` constructor.
///
/// # Safety
/// `caller`, `object`, `function`, `result` are null or valid engine pointers.
pub unsafe fn prebegin_relevance_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    caller: *const c_void,
    object: *const c_void,
    function: *const c_void,
    result: *const c_void,
) -> *mut PreBeginRelevanceScope {
    if !st.enabled || st.flushed || !is_game_info_is_relevant(svc, function) {
        return null_mut();
    }
    let Some(active) = PREBEGIN_EVENT.get() else {
        return null_mut();
    };
    if !is_actor_prebegin_play(svc, cast_fn(svc, svc.node_of(caller))) {
        return null_mut();
    }
    let actor_object = st.prebegin_events[active].actor_object;
    let level = read_object(svc, actor_object, "Level");
    if !std::ptr::eq(object, read_object(svc, level, "Game")) {
        return null_mut();
    }
    let entry_ordinal = st.call_count;
    let event = &mut st.prebegin_events[active];
    event.relevance_dispatched = true;
    event.has_relevance_ordinals = true;
    event.relevance_entry_ordinal = entry_ordinal;
    Box::into_raw(Box::new(PreBeginRelevanceScope {
        event: active,
        result,
    }))
}

/// Oracle `FAppRandTracePreBeginRelevanceScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`prebegin_relevance_scope_begin`], ended once.
pub unsafe fn prebegin_relevance_scope_end(
    st: &mut TraceState,
    handle: *mut PreBeginRelevanceScope,
) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if !st.enabled || st.flushed {
        return;
    }
    let exit_ordinal = st.call_count;
    let result = scope.result;
    // SAFETY: a non-null result points at the engine's bool return slot.
    let relevance_result =
        !result.is_null() && unsafe { read_unaligned(result as *const u32) } != 0;
    let event = &mut st.prebegin_events[scope.event];
    event.relevance_exit_ordinal = exit_ordinal;
    event.relevance_app_rand_consumed = exit_ordinal != event.relevance_entry_ordinal;
    event.has_relevance_result = !result.is_null();
    event.relevance_result = relevance_result;
    event.relevance_completed = true;
}

/// Oracle `FAppRandTraceSoftwareRendering` record (`appRecordRandTracePreBeginSoftwareRendering`).
///
/// # Safety
/// `object` is null or a valid engine pointer compared by identity only.
pub unsafe fn record_software_rendering(
    st: &mut TraceState,
    object: *const c_void,
    is_software_rendering: bool,
) {
    if !st.enabled || st.flushed {
        return;
    }
    let Some(index) = PREBEGIN_EVENT.get() else {
        return;
    };
    let event = &mut st.prebegin_events[index];
    if event.actor_object == object {
        event.has_software_rendering = true;
        event.is_software_rendering = is_software_rendering;
    }
}

/// Oracle `FAppRandTraceFunctionScope` constructor.
///
/// # Safety
/// `object`, `function`, `stack` are null or valid engine pointers.
pub unsafe fn function_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    object: *const c_void,
    function: *const c_void,
    stack: *const c_void,
) -> Result<*mut FunctionScope, Threw> {
    if !st.enabled || st.flushed || !is_game_info_is_relevant(svc, function) {
        return Ok(null_mut());
    }
    let prebegin_event = begin_prebegin_relevance_trace(svc, st, function, stack);
    let event = begin_game_info_is_relevant_trace(svc, st, object, function, stack)?;
    if event.is_none() && prebegin_event.is_none() {
        return Ok(null_mut());
    }
    Ok(Box::into_raw(Box::new(FunctionScope {
        event,
        prebegin_event,
    })))
}

/// Oracle `FAppRandTraceFunctionScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`function_scope_begin`], ended once.
pub unsafe fn function_scope_end(st: &mut TraceState, handle: *mut FunctionScope) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if st.flushed {
        return;
    }
    let exit_ordinal = st.call_count;
    if let Some(index) = scope.event {
        let event = &mut st.function_events[index];
        event.exit_ordinal = exit_ordinal;
        event.has_exit = true;
    }
    if let Some(index) = scope.prebegin_event {
        st.prebegin_events[index].relevance_completed = true;
    }
}

/// Oracle `FAppRandTraceRelevanceScope` constructor.
///
/// # Safety
/// `object`, `function`, `stack` are null or valid engine pointers.
pub unsafe fn relevance_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    object: *const c_void,
    function: *const c_void,
    stack: *const c_void,
) -> Result<*mut RelevanceScope, Threw> {
    if !st.enabled || st.flushed || !is_game_info_is_relevant(svc, function) {
        return Ok(null_mut());
    }
    Ok(
        match begin_game_info_relevance_trace(svc, st, object, function, stack)? {
            Some(event) => Box::into_raw(Box::new(RelevanceScope { event })),
            None => null_mut(),
        },
    )
}

/// Oracle `FAppRandTraceRelevanceScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`relevance_scope_begin`], ended once.
pub unsafe fn relevance_scope_end(st: &mut TraceState, handle: *mut RelevanceScope) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if st.flushed {
        return;
    }
    let exit_ordinal = st.call_count;
    let event = &mut st.relevance_events[scope.event];
    event.exit_ordinal = exit_ordinal;
    event.has_exit = true;
    RELEVANCE_EVENT.set(event.previous);
}

/// Oracle `FAppRandTraceRelevanceCallScope` constructor.
///
/// # Safety
/// `caller`, `object`, `function`, `result` are null or valid engine pointers.
pub unsafe fn relevance_call_scope_begin(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    caller: *const c_void,
    function: *const c_void,
    result: *const c_void,
) -> *mut RelevanceCallScope {
    if !st.enabled || st.flushed {
        return null_mut();
    }
    let Some(active) = RELEVANCE_EVENT.get() else {
        return null_mut();
    };
    let depth = st.relevance_events[active].direct_call_depth;
    let mut capture_kind = 0;
    let mut super_relevant: *const u8 = null();
    if depth == 0 && !function.is_null() && name_is(svc, function, "IsRelevant") {
        capture_kind = 2;
        // Oracle passes the raw Cast<UFunction>(CallerStack.Node) through unguarded;
        // a null node is dereferenced by the services row exactly as the original does.
        let property = svc.field(
            cast_fn(svc, svc.node_of(caller)),
            wide_z("bSuperRelevant").as_ptr(),
            3,
        );
        if !property.is_null() {
            // SAFETY: `locals_of(caller)` is the live frame locals base returned by the
            // services row; `offset_of(property)` is the engine property offset, so the
            // result addresses the `bSuperRelevant` byte inside the frame. Oracle reads it
            // through the same pointer arithmetic.
            super_relevant =
                unsafe { svc.locals_of(caller).add(svc.offset_of(property)) } as *const u8;
        }
    } else if depth == 0 && !function.is_null() && name_is(svc, function, "AlwaysKeep") {
        capture_kind = 1;
    }
    st.relevance_events[active].direct_call_depth += 1;
    Box::into_raw(Box::new(RelevanceCallScope {
        event: active,
        result,
        super_relevant,
        capture_kind,
    }))
}

/// Oracle `FAppRandTraceRelevanceCallScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`relevance_call_scope_begin`], ended once.
pub unsafe fn relevance_call_scope_end(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    handle: *mut RelevanceCallScope,
) -> Result<(), Threw> {
    if handle.is_null() {
        return Ok(());
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if !st.enabled || st.flushed {
        return Ok(());
    }
    let result = scope.result;
    let result_bool = !result.is_null() && unsafe { read_unaligned(result as *const u32) } != 0;
    let event = &mut st.relevance_events[scope.event];
    if scope.capture_kind == 2 {
        event.has_is_relevant_result = !result.is_null();
        event.is_relevant_result = result_bool;
        if !scope.super_relevant.is_null() {
            event.has_super_relevant = true;
            // SAFETY: super_relevant points at the engine's bSuperRelevant byte.
            event.super_relevant = unsafe { *scope.super_relevant };
        }
    } else if scope.capture_kind == 1 && !result.is_null() {
        event.has_always_keep_result = true;
        event.always_keep_result = result_bool;
    }
    let depth = event.direct_call_depth;
    check_relevance_depth(svc, depth)?;
    st.relevance_events[scope.event].direct_call_depth -= 1;
    Ok(())
}

/// Oracle `appRecordRandTraceRelevanceReturnDebugInfo`.
///
/// # Safety
/// `stack` is a valid `FFrame*`.
pub unsafe fn record_relevance_return_debug_info(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    stack: *const c_void,
    line_number: i32,
) {
    if !st.enabled || st.flushed || !is_game_info_is_relevant(svc, cast_fn(svc, svc.node_of(stack)))
    {
        return;
    }
    if let Some(index) = RELEVANCE_EVENT.get() {
        st.relevance_events[index].return_source_line = line_number;
    }
}

/// Oracle `appRecordRandTraceRelevanceReturn`.
///
/// # Safety
/// `stack` is a valid `FFrame*`; `result` is null or points at a bool return slot.
pub unsafe fn record_relevance_return(
    svc: &Hp2AnsiTraceServices,
    st: &mut TraceState,
    stack: *const c_void,
    result: *const c_void,
) {
    if !st.enabled || st.flushed || !is_game_info_is_relevant(svc, cast_fn(svc, svc.node_of(stack)))
    {
        return;
    }
    let Some(index) = RELEVANCE_EVENT.get() else {
        return;
    };
    if result.is_null() {
        return;
    }
    // SAFETY: result is non-null and points at the engine's bool return slot.
    let final_result = unsafe { read_unaligned(result as *const u32) } != 0;
    let branch = branch_name(&st.relevance_events[index]);
    let event = &mut st.relevance_events[index];
    event.has_final_result = true;
    event.final_result = final_result;
    event.early_return_branch = Some(branch);
}

/// Oracle `FAppRandTracePhaseScope` constructor.
pub fn phase_scope_begin(st: &mut TraceState, phase: Option<&[u8]>) -> *mut PhaseScope {
    if !st.startup_phase_enabled || st.flushed {
        return null_mut();
    }
    if st.startup_phase_count >= MAX_STARTUP_PHASES {
        return null_mut();
    }
    // Oracle strcmp on a null phase is undefined; an absent phase compares as empty here.
    let phase_bytes = phase.unwrap_or(b"");
    if phase_bytes == b"pre_render_fglobalrandoms" {
        let mut has_fire_init = false;
        for entry in &st.startup_phases[..st.startup_phase_count] {
            if entry.phase == b"fire_init_tables" {
                has_fire_init = true;
            }
            if entry.phase == b"pre_render_fglobalrandoms"
                && entry.load_ordinal == st.startup_load_ordinal
            {
                return null_mut();
            }
        }
        if !has_fire_init {
            return null_mut();
        }
    }
    let index = st.startup_phase_count;
    st.startup_phase_count += 1;
    st.startup_phases[index] = PhaseEvent {
        phase: copy_str(phase, PHASE_CAPACITY, b"startup"),
        map_token: copy_str(Some(&st.startup_map_token), MAP_CAPACITY, b""),
        map_url: copy_str(Some(&st.startup_map_url), MAP_URL_CAPACITY, b""),
        lifecycle_boundary: copy_str(
            Some(&st.startup_lifecycle_boundary),
            MAP_URL_CAPACITY,
            b"map_loaded",
        ),
        load_ordinal: st.startup_load_ordinal,
        ordinal_before: st.call_count,
        ordinal_after: 0,
    };
    Box::into_raw(Box::new(PhaseScope { event: index }))
}

/// Oracle `FAppRandTracePhaseScope` destructor.
///
/// # Safety
/// `handle` is null or a handle from [`phase_scope_begin`], ended once.
pub unsafe fn phase_scope_end(st: &mut TraceState, handle: *mut PhaseScope) {
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle came from Box::into_raw and is ended once.
    let scope = unsafe { Box::from_raw(handle) };
    if st.flushed {
        return;
    }
    st.startup_phases[scope.event].ordinal_after = st.call_count;
}

#[cfg(test)]
mod direct_trace_tests {
    use super::*;

    #[test]
    fn direct_rng_sample_retains_native_and_outer_consumer_identity() {
        let mut state = TraceState {
            enabled: true,
            limit: 1,
            samples: vec![Sample::default()],
            ..TraceState::default()
        };
        let outer = direct_outer_scope_begin(
            &state,
            std::ptr::null(),
            std::ptr::null(),
            b"Actor_0".to_vec(),
            b"Core.Pawn".to_vec(),
            b"Core.Pawn.Tick".to_vec(),
        );
        let native = direct_native_scope_begin(
            &state,
            std::ptr::null(),
            std::ptr::null(),
            c"Core.Object.Rand",
            167,
            23,
            b"Actor_0".to_vec(),
            b"Core.Pawn".to_vec(),
            b"Core.Pawn.Tick".to_vec(),
        );

        let value = direct_rand_value(&mut state);
        let sample = &state.samples[0];
        assert_eq!(sample.value, value);
        assert!(sample.has_consumer);
        assert_eq!(sample.actor.as_deref(), Some(b"Actor_0".as_slice()));
        assert_eq!(sample.class.as_deref(), Some(b"Core.Pawn".as_slice()));
        assert_eq!(sample.function.as_deref(), Some(b"Core.Pawn.Tick".as_slice()));
        assert_eq!(sample.native.as_deref(), Some(b"Core.Object.Rand".as_slice()));
        assert_eq!(sample.native_slot, 167);
        assert_eq!(sample.callsite_offset, 23);
        assert!(sample.has_outer_consumer);
        assert_eq!(sample.outer_actor.as_deref(), Some(b"Actor_0".as_slice()));

        // SAFETY: both scopes were opened on this thread and are ended exactly once.
        unsafe {
            context_scope_end(native);
            context_scope_end(outer);
        }
    }
}

#[cfg(test)]
mod rng_trace_metadata_tests {
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use super::*;

    /// The `HP2_RNG_TRACE` + `HP2_RNG_TRACE_LIMIT=8` startup state from
    /// `Tests/RngTraceMetadataTests.cpp`, built directly so the contract does
    /// not depend on process environment.
    fn trace_state(path: &Path) -> TraceState {
        TraceState {
            enabled: true,
            ordinal_enabled: true,
            storage_available: true,
            path: path.as_os_str().as_bytes().to_vec(),
            limit: 8,
            samples: (0..8).map(|_| Sample::default()).collect(),
            tick_boundaries: (0..8).map(|_| TickBoundary::default()).collect(),
            function_events: (0..8).map(|_| FunctionEvent::default()).collect(),
            prebegin_events: (0..8).map(|_| PreBeginEvent::default()).collect(),
            relevance_events: (0..8).map(|_| RelevanceEvent::default()).collect(),
            ..TraceState::default()
        }
    }

    fn unique_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "hp2-rng-trace-metadata-{}-{label}.json",
            std::process::id()
        ))
    }

    fn read_trace(path: &Path) -> Vec<u8> {
        std::fs::read(path).expect("trace was not written")
    }

    #[test]
    fn rng_trace_metadata_reports_diagnostic_seed() {
        let path = unique_path("diagnostic");
        let _ = std::fs::remove_file(&path);
        let mut state = trace_state(&path);

        set_diagnostic_seed(&mut state, RAND_MAX as u32, 1);
        direct_rand_value(&mut state);
        flush(&mut state);

        let trace = read_trace(&path);
        let _ = std::fs::remove_file(&path);
        let trace = String::from_utf8_lossy(&trace);
        let expected = format!(
            "\"seed\":1,\"requested_seed\":{},\"start_reason\":\"diagnostic\"",
            RAND_MAX
        );
        assert!(
            trace.contains(&expected),
            "diagnostic effective/requested metadata missing: {trace}"
        );
    }

    #[test]
    fn rng_trace_metadata_reports_replay_start() {
        let path = unique_path("replay");
        let _ = std::fs::remove_file(&path);
        let mut state = trace_state(&path);
        state.initialized = true;

        set_diagnostic_seed(&mut state, 42, 42);
        crate::rng_private::srand(1);
        reset_for_replay(&mut state);
        direct_rand_value(&mut state);
        flush(&mut state);

        let trace = read_trace(&path);
        let _ = std::fs::remove_file(&path);
        let trace = String::from_utf8_lossy(&trace);
        assert!(
            trace.contains("\"seed\":1,\"start_reason\":\"replay\""),
            "replay metadata missing: {trace}"
        );
        assert!(
            !trace.contains("\"requested_seed\""),
            "replay retained diagnostic requested seed: {trace}"
        );
    }
}
