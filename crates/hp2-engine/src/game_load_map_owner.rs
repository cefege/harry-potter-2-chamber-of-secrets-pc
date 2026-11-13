//! Core-owned adapter for the shared GameEngine LoadMap policy.
use hp2_abi::FPlane;

use crate::game_load_map::{self, LoadMapOps, Op};
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, FURL, TArrayRaw, TMapStringString, UGameEngine, ULevel, UPendingLevel,
    UViewport, UObject,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{cell::{Cell, RefCell}, io::Write, path::PathBuf, ptr, sync::{Mutex, atomic::{AtomicBool, Ordering}}};

struct GameStateTraceSession {
    file: std::fs::File,
    records: u64,
    hash: u64,
    failed: bool,
}
static GAME_STATE_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
static GAME_STATE_TRACE_SESSION: Mutex<Option<GameStateTraceSession>> = Mutex::new(None);

fn trace_hash(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes { hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3); }
    hash
}
fn trace_failure(error: &impl std::fmt::Display) {
    let _ = writeln!(std::io::stderr().lock(), "HP2_GAME_STATE_TRACE incomplete: {error}");
}

// Called once per reporter lifetime. Never append to a previous session.
pub(crate) fn game_state_trace_start() -> bool {
    let Some(path) = std::env::var_os("HP2_GAME_STATE_TRACE").filter(|value| !value.is_empty()) else { return false };
    let id = match std::env::var("HP2_GAME_STATE_TRACE_ID") {
        Ok(id) if id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) => id,
        _ => { trace_failure(&"a fresh 32-digit lowercase hex HP2_GAME_STATE_TRACE_ID is required"); return false; }
    };
    let Ok(mut slot) = GAME_STATE_TRACE_SESSION.lock() else { trace_failure(&"session lock poisoned"); return false };
    if slot.is_some() { trace_failure(&"session already active"); return false; }
    let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) => { trace_failure(&error); return false; }
    };
    let header = format!("event=trace_start version=1 session={id} capture_selection=blocked\n");
    if let Err(error) = file.write_all(header.as_bytes()) { trace_failure(&error); return false; }
    *slot = Some(GameStateTraceSession { file, records: 0, hash: trace_hash(0xcbf29ce484222325, header.as_bytes()), failed: false });
    GAME_STATE_TRACE_ENABLED.store(true, Ordering::Release);
    true
}

// The footer certifies only the screening trace transport, not game success or
// cutscene capture coverage. Its absence means the consumer must reject it.
pub(crate) fn game_state_trace_finish(complete: bool) {
    GAME_STATE_TRACE_ENABLED.store(false, Ordering::Release);
    let Ok(mut slot) = GAME_STATE_TRACE_SESSION.lock() else { trace_failure(&"session lock poisoned"); return };
    let Some(mut session) = slot.take() else { return };
    if !complete || session.failed { trace_failure(&"session did not complete"); return; }
    if let Err(error) = session.file.sync_data() { trace_failure(&error); return; }
    let footer = format!("event=trace_end records={} hash={:016x} capture_selection=blocked\n", session.records, session.hash);
    if let Err(error) = session.file.write_all(footer.as_bytes()) { trace_failure(&error); }
}

// Trace failures do not change LoadMap results. Disabled calls do not read the
// environment, format details, allocate, or acquire a lock.
pub(crate) fn game_state_trace(event: &str, details: impl FnOnce() -> String) {
    if !GAME_STATE_TRACE_ENABLED.load(Ordering::Acquire) { return; }
    let details = details();
    let Ok(mut slot) = GAME_STATE_TRACE_SESSION.lock() else { trace_failure(&"session lock poisoned"); return };
    let Some(session) = slot.as_mut() else { return };
    if session.failed { return; }
    let line = format!("record={} event={event} {details}\n", session.records);
    match session.file.write_all(line.as_bytes()) {
        Ok(()) => { session.hash = trace_hash(session.hash, line.as_bytes()); session.records += 1; }
        Err(error) => { session.failed = true; trace_failure(&error); }
    }
}

// Infer the probed fixed-array extent without borrowing an unaligned field.
const fn array_extent<T, const N: usize>(_: *const [T; N]) -> usize { N }

struct ObjectIterator {
    class: *mut hp2_abi::engine::generated::UClass,
    index: i32,
}
impl ObjectIterator {
    fn new(access: CoreAccess<'_>, class_name: &str) -> RuntimeResult<Self> {
        let mut iterator = Self { class: access.native_class(class_name)?, index: -1 };
        iterator.advance(access)?;
        Ok(iterator)
    }
    fn advance(&mut self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        unsafe { access.next_object_index(&mut self.index, self.class) }
    }
}

struct LoadMapContext {
    engine: *mut UGameEngine,
    url: *const FURL,
    pending: Cell<*mut UPendingLevel>,
    travel_info: *const TMapStringString,
    error: *mut TArrayRaw,
    package_map: Cell<*mut hp2_abi::engine::generated::UPackageMap>,
    map_parent: Cell<*mut UObject>,
    info: Cell<*mut ALevelInfo>,
    saved_hub: Cell<i32>,
    previous_tick: Cell<Option<i64>>,
    load_started: Cell<i64>,
    load_process_start: Cell<i64>,
    options: Cell<*mut Vec<u32>>,
    game_class_name: Cell<*mut Vec<u32>>,
    init_error: Cell<*mut Vec<u32>>,
    listen_error: Cell<*mut Vec<u32>>,
    reordered_actors: Cell<*mut Vec<*mut AActor>>,
    actor_iterator: Cell<*mut ObjectIterator>,
    function_iterator: Cell<*mut ObjectIterator>,
    demo_file: Cell<*mut Vec<u32>>,
    demo_size: Cell<i32>,
    catch_error: Cell<*mut Vec<u32>>,
    filename: Cell<*mut Vec<u32>>,
}

#[repr(C)]
struct InitGameParams {
    options: TArrayRaw,
    error: TArrayRaw,
}

struct CoreOps<'a> {
    access: CoreAccess<'a>,
    context: LoadMapContext,
}

impl CoreOps<'_> {
    fn engine(&self) -> &UGameEngine {
        // SAFETY: the public entrypoint requires a live canonical GameEngine for the synchronous run.
        unsafe { &*self.context.engine }
    }

    fn level(&self) -> *mut ULevel {
        self.engine().g_level.cast()
    }

    fn output(&self, value: usize) -> RuntimeResult<usize> { Ok(value) }

    fn text(&self, destination: *mut TArrayRaw, text: &[u32]) -> RuntimeResult<()> {
        // SAFETY: the destination is an initialized FString owned by the LoadMap call or record.
        unsafe { self.access.assign_fstring(destination, text) }
    }

    fn url_map(&self) -> RuntimeResult<Vec<u32>> {
        let map = unsafe { ptr::addr_of!((*self.context.url).map) };
        let raw = unsafe { map.read_unaligned() };
        if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
            return Err(RuntimeError::new("Invalid LoadMap URL map storage"));
        }
        Ok(if raw.num == 0 { Vec::new() } else {
            unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) }.to_vec()
        })
    }

    fn set_empty_error(&self) -> RuntimeResult<()> { self.text(self.context.error, &[0]) }

    fn reflected_field(&self, object: *mut UObject, field: &str) -> RuntimeResult<*mut u8> {
        if object.is_null() { return Err(RuntimeError::new("Null LoadMap reflected object")); }
        let name = self.access.intern_str(field, FindName::Find)?;
        let property_class = self.access.native_class("Property")?;
        let property = unsafe { self.access.find_field((*object).class.cast(), name, property_class) };
        if property.is_null() {
            return Err(RuntimeError::new("LoadMap reflected property is missing").with_context(field));
        }
        let offset = unsafe { (*property.cast::<hp2_abi::engine::generated::UProperty>()).offset };
        if offset < 0 { return Err(RuntimeError::new("Negative LoadMap reflected property offset").with_context(field)); }
        Ok(unsafe { object.cast::<u8>().add(offset as usize) })
    }
    fn reflected_function(&self, object: *mut UObject, name: &str) -> RuntimeResult<*mut hp2_abi::engine::generated::UFunction> {
        if object.is_null() { return Err(RuntimeError::new("Null LoadMap event target")); }
        let name = self.access.intern_str(name, FindName::Find)?;
        let function = unsafe { self.access.find_object_function_by_name(object, name, false)? };
        if function.is_null() { return Err(RuntimeError::new("LoadMap reflected event is missing")); }
        Ok(function)
    }

    fn log_format(&self, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        let text: Vec<u32> = message.chars().map(u32::from).collect();
        self.access.emit_log(category, &text)
    }

    fn object_is_class(&self, object: *mut UObject, class_name: &str) -> RuntimeResult<bool> {
        if object.is_null() { return Ok(false); }
        let target = self.access.native_class(class_name)?;
        let mut class = unsafe { (*object).class.cast::<hp2_abi::engine::generated::UClass>() };
        while !class.is_null() {
            if class == target { return Ok(true); }
            class = unsafe { (*class).super_field.cast() };
        }
        Ok(false)
    }

    fn option_units(&self) -> Vec<u32> {
        let options = self.context.options.get();
        if options.is_null() { Vec::new() } else { unsafe { (*options).clone() } }
    }
    fn url_option(&self, index: usize) -> RuntimeResult<Vec<u32>> {
        let options = unsafe { ptr::addr_of!((*self.context.url).op).read_unaligned() };
        if options.num < 0 || index >= options.num as usize || options.data.is_null() {
            return Err(RuntimeError::new("LoadMap URL option index is out of range"));
        }
        let option = unsafe { options.data.cast::<TArrayRaw>().add(index).read_unaligned() };
        let mut units = string_units(&option);
        if units.last() == Some(&0) { units.pop(); }
        Ok(units)
    }

    fn write_reflected_bool(&self, object: *mut UObject, field: &str, value: bool) -> RuntimeResult<()> {
        let name = self.access.intern_str(field, FindName::Find)?;
        let bool_class = self.access.native_class("BoolProperty")?;
        let property = unsafe { self.access.find_field((*object).class.cast(), name, bool_class) };
        if property.is_null() { return Err(RuntimeError::new("LoadMap reflected boolean is missing").with_context(field)); }
        let address = unsafe { self.access.property_address(object.cast(), property.cast())? }.cast::<u32>();
        let mask = unsafe { (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask };
        let old = unsafe { address.read_unaligned() };
        unsafe { address.write_unaligned(if value { old | mask } else { old & !mask }); }
        Ok(())
    }

    unsafe fn read_reflected<T: Copy>(&self, object: *mut UObject, field: &str) -> RuntimeResult<T> {
        let address = self.reflected_field(object, field)?;
        Ok(unsafe { address.cast::<T>().read_unaligned() })
    }

    unsafe fn write_reflected<T: Copy>(&self, object: *mut UObject, field: &str, value: &T) -> RuntimeResult<()> {
        let address = self.reflected_field(object, field)?;
        unsafe { address.cast::<T>().write_unaligned(*value); }
        Ok(())
    }
}

struct CoreScreenActorsOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
    group_storage: RefCell<Vec<Vec<u32>>>,
}

impl crate::level::ScreenActorsOps for CoreScreenActorsOps<'_> {
    type Error = RuntimeError;
    unsafe fn screen_trace(&self, event: &'static str, actor: *mut std::ffi::c_void,
                           text: *mut std::ffi::c_void, value: i32) {
        game_state_trace(event, || {
            // The callback can destroy the actor. Never dereference it on return.
            let name = if actor.is_null() || event == "resolve_end" { None } else {
                Some(unsafe { self.access.object_full_name(actor.cast()) }
                    .map_err(|error| error.to_string()))
            };
            let units = if text.is_null() { None } else {
                Some(string_units(unsafe { &*text.cast::<TArrayRaw>() }))
            };
            format!("level={:p} actor={actor:p} name_utf32={name:?} text_utf32={units:?} value={value}", self.level)
        });
    }
    unsafe fn actors_num(&self, level: *mut std::ffi::c_void) -> RuntimeResult<i32> { crate::level_owner::actor_count(level.cast()) }
    unsafe fn actors_get(&self, level: *mut std::ffi::c_void, index: i32) -> RuntimeResult<*mut std::ffi::c_void> { Ok(crate::level_owner::actor_at(level.cast(), index).cast()) }
    unsafe fn actor_is_player(&self, actor: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        if actor.is_null() { return Ok(0); }
        let pawn = self.access.native_class("Engine.Pawn")?;
        let result = unsafe {
            crate::actor::is_player_with(actor.cast(), |candidate, _| {
                Ok(hp2_core::object_model::is_a(candidate, pawn.cast()))
            })
        }.map_err(|_| RuntimeError::new("Failed to resolve AActor::IsPlayer class"))?;
        Ok(i32::from(result))
    }
    unsafe fn fstring_caps(&self, dst: *mut std::ffi::c_void, src: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let units = uppercase_units(&string_units(unsafe { &*src.cast::<TArrayRaw>() }));
        let mut units = units; units.push(0);
        unsafe { self.access.assign_fstring(dst.cast(), &units) }
    }
    unsafe fn pawn_current_game_state(&self, pawn: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { ptr::addr_of_mut!((*pawn.cast::<hp2_abi::engine::generated::APlayerPawn>()).current_game_state).cast() })
    }
    unsafe fn fstring_eq_chars(&self, text: *mut std::ffi::c_void, chars: *const u32) -> RuntimeResult<i32> {
        let value = string_units(unsafe { &*text.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).collect::<Vec<_>>();
        Ok(i32::from(value == read_units(chars)))
    }
    unsafe fn pawn_game_state_master_list(&self, pawn: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { ptr::addr_of_mut!((*pawn.cast::<hp2_abi::engine::generated::APlayerPawn>()).game_state_master_list).cast() })
    }
    unsafe fn fstring_instr_fstring(&self, text: *mut std::ffi::c_void, sub: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let a = string_units(unsafe { &*text.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).collect::<Vec<_>>();
        let b = string_units(unsafe { &*sub.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).collect::<Vec<_>>();
        Ok(find_units(&a, &b).map_or(-1, |i| i as i32))
    }
    unsafe fn screen_invalid_state(&self, pawn: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        let text = if pawn.is_null() { "**** Can't find a valid player!!!! " }
            else { "**** Caution: The player pawn has an Invalid CurrentGameState for this package Load." };
        self.access.emit_log(category, &text.chars().map(u32::from).collect::<Vec<_>>())
    }
    unsafe fn fstring_assign_chars(&self, dst: *mut std::ffi::c_void, chars: *const u32) -> RuntimeResult<()> {
        let mut units = read_units(chars); units.push(0);
        unsafe { self.access.assign_fstring(dst.cast(), &units) }
    }
    unsafe fn actor_group_chars(&self, actor: *mut std::ffi::c_void) -> RuntimeResult<*const u32> {
        let name = self.access.intern_str("Group", FindName::Find)?;
        let property_class = self.access.native_class("Property")?;
        let field = unsafe { self.access.find_field((*actor.cast::<UObject>()).class.cast(), name, property_class) };
        if field.is_null() { return Err(RuntimeError::new("Actor.Group property is missing during state screening")); }
        let offset = unsafe { (*field.cast::<hp2_abi::engine::generated::UProperty>()).offset };
        if offset < 0 { return Err(RuntimeError::new("Negative Actor.Group property offset")); }
        let fname = unsafe { actor.cast::<u8>().add(offset as usize).cast::<hp2_abi::engine::generated::FName>().read_unaligned() };
        let mut text = self.access.name_text(fname.index)?; if text.last() != Some(&0) { text.push(0); }
        let mut storage = self.group_storage.borrow_mut(); storage.push(text); Ok(storage.last().unwrap().as_ptr())
    }
    unsafe fn actor_exclude_game_states(&self, actor: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { ptr::addr_of_mut!((*actor.cast::<AActor>()).exclude_game_states).cast() })
    }
    unsafe fn fstring_len(&self, text: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        Ok(string_units(unsafe { &*text.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).count() as i32)
    }
    unsafe fn exclude_caps_contains(&self, exclude: *mut std::ffi::c_void, state: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let text = unsafe { ptr::read_unaligned(exclude.cast::<TArrayRaw>()) };
        if text.num < 0 || (text.num > 0 && text.data.is_null()) { return Err(RuntimeError::new("Invalid Actor.ExcludeGameStates")); }
        // ExcludeGameStates is one FString, not an array of strings. The
        // screening policy has already uppercased the current game state.
        let excluded = if text.num == 0 { &[][..] } else {
            unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) }
        };
        let state = unsafe { &*state.cast::<TArrayRaw>() };
        let wanted = if state.num <= 0 || state.data.is_null() { &[][..] } else {
            unsafe { std::slice::from_raw_parts(state.data.cast::<u32>(), state.num as usize) }
        };
        let wanted = &wanted[..wanted.iter().position(|unit| *unit == 0).unwrap_or(wanted.len())];
        Ok(i32::from(contains_units(&uppercase_units(excluded), wanted)))
    }
    unsafe fn fstring_instr_chars(&self, text: *mut std::ffi::c_void, chars: *const u32) -> RuntimeResult<i32> {
        let text = string_units(unsafe { &*text.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).collect::<Vec<_>>();
        Ok(find_units(&text, &read_units(chars)).map_or(-1, |i| i as i32))
    }
    unsafe fn is_editor(&self) -> RuntimeResult<i32> { Ok(i32::from(self.access.process_mode().is_editor)) }
    unsafe fn actor_set_hidden_ed(&self, actor: *mut std::ffi::c_void, value: i32) -> RuntimeResult<()> {
        unsafe { (*actor.cast::<AActor>()).set_b_hidden_ed(value != 0); } Ok(())
    }
    unsafe fn fstring_mid(&self, dst: *mut std::ffi::c_void, src: *mut std::ffi::c_void, start: i32, count: i32) -> RuntimeResult<()> {
        let units = string_units(unsafe { &*src.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0).collect::<Vec<_>>();
        let begin = (start.max(0) as usize).min(units.len());
        let end = begin.saturating_add(count.max(0) as usize).min(units.len());
        let mut output = units[begin..end].to_vec(); output.push(0);
        unsafe { self.access.assign_fstring(dst.cast(), &output) }
    }
    unsafe fn pawn_game_state_token_len(&self, pawn: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*pawn.cast::<hp2_abi::engine::generated::APlayerPawn>()).game_state_token_len })
    }
    unsafe fn fstring_eq_fstring(&self, first: *mut std::ffi::c_void, second: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let a = string_units(unsafe { &*first.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0);
        let b = string_units(unsafe { &*second.cast::<TArrayRaw>() }).into_iter().take_while(|u| *u != 0);
        Ok(i32::from(a.eq(b)))
    }
    unsafe fn actor_set_in_current_game_state(&self, actor: *mut std::ffi::c_void, value: i32) -> RuntimeResult<()> {
        unsafe { (*actor.cast::<AActor>()).set_b_in_current_game_state(value != 0); } Ok(())
    }
    unsafe fn actor_event_on_resolve_game_state(&self, actor: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { self.access.execute_named_event(actor.cast(), "OnResolveGameState")?; } Ok(())
    }
    unsafe fn object_get_name(&self, dst: *mut std::ffi::c_void, object: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let mut name = self.access.name_text(i32::from_ne_bytes(unsafe { (*object.cast::<UObject>()).name }))?;
        if name.last() != Some(&0) { name.push(0); }
        unsafe { self.access.assign_fstring(dst.cast(), &name) }
    }
    unsafe fn screen_invalid_token(&self, actor_name: *mut std::ffi::c_void, token: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let category = self.access.intern_str("Error", FindName::Add)?;
        let actor = utf32_to_string(&string_units(unsafe { &*actor_name.cast::<TArrayRaw>() }))?;
        let token = utf32_to_string(&string_units(unsafe { &*token.cast::<TArrayRaw>() }))?;
        let message = format!("*** GameState ERROR Actor: {actor} has an invalid GameState Token: {token}!!!");
        self.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
    }
}

fn utf32_pointer_units(pointer: *const u32, limit: usize) -> RuntimeResult<Vec<u32>> {
    if pointer.is_null() { return Err(RuntimeError::new("Null LoadMap TCHAR text")); }
    let mut units = Vec::new();
    for index in 0..limit {
        let unit = unsafe { pointer.add(index).read_unaligned() };
        if unit == 0 { return Ok(units); }
        units.push(unit);
    }
    Err(RuntimeError::new("LoadMap TCHAR text is not terminated"))
}
impl LoadMapOps for CoreOps<'_> {
    type Error = RuntimeError;

    fn call(&self, op: Op, a: usize, b: usize) -> RuntimeResult<usize> {
        use Op::*;
        let engine = self.context.engine;
        let level = self.level();
        // SAFETY: every pointer passed through this backend originates in the live canonical
        // GameEngine graph or in the synchronous source LoadMap arguments/scopes.
        unsafe {
            match op {
                ClearError => { self.set_empty_error()?; Ok(0) }
                Level => self.output((*engine).g_level as usize),
                ResetGuard => { unsafe { hp2_core::vm_guard::reset(1) }; Ok(0) }
                SaveHub => { self.context.saved_hub.set(a as i32); Ok(0) }
                SavedHub => self.output(self.context.saved_hub.get() as usize),
                Client => self.output((*engine).client as usize),
                ViewportCount => {
                    let client = (*engine).client.cast::<hp2_abi::engine::generated::UClient>();
                    self.output(if client.is_null() { 0 } else { (*client).viewports.num.max(0) as usize })
                }
                Viewport => {
                    let client = (*engine).client.cast::<hp2_abi::engine::generated::UClient>();
                    if client.is_null() { return Ok(0); }
                    let views = (*client).viewports;
                    if a >= views.num.max(0) as usize || views.data.is_null() { return Ok(0); }
                    self.output(views.data.cast::<*mut UViewport>().add(a).read_unaligned() as usize)
                }
                Pending => self.output(self.context.pending.get() as usize),
                PackageMap => {
                    let pending = self.context.pending.get();
                    if pending.is_null() { return Err(RuntimeError::new("LoadMap pending level is null")); }
                    let driver = (*pending).net_driver.cast::<hp2_abi::engine::generated::UNetDriver>();
                    if driver.is_null() { return Err(RuntimeError::new("LoadMap pending driver is null")); }
                    let connection = (*driver).server_connection.cast::<hp2_abi::engine::generated::UNetConnection>();
                    if connection.is_null() { return Err(RuntimeError::new("LoadMap server connection is null")); }
                    self.context.package_map.set((*connection).package_map.cast());
                    Ok(0)
                }
                BeginLoad => { self.access.begin_load()?; Ok(0) }
                EndLoad => { self.access.end_load()?; Ok(0) }
                PackageCount => {
                    let map = self.context.package_map.get();
                    self.output(if map.is_null() { 0 } else { (*map).list.num.max(0) as usize })
                }
                ComputePackages => {
                    let map = self.context.package_map.get();
                    if !map.is_null() { self.access.package_map_compute(map)?; }
                    Ok(0)
                }
                DemoDriver => {
                    let pending = self.context.pending.get();
                    self.output(if pending.is_null() { 0 } else { (*pending).demo_rec_driver as usize })
                }
                ClearPending => { self.context.pending.set(ptr::null_mut()); Ok(0) }
                LonePlayer => {
                    let pending = self.context.pending.get();
                    self.output(usize::from(!pending.is_null() && (*pending).lone_player != 0))
                }
                LoadInfo => {
                    let map = self.url_map()?;
                    let class = self.access.native_class("Engine.LevelInfo")?;
                    let object = self.access.static_load_object_from_file(
                        class, self.context.map_parent.get(), &[76,101,118,101,108,73,110,102,111,48,0],
                        &map, 0x0000_0001,
                    )?;
                    self.context.info.set(object.cast());
                    self.output(object as usize)
                }
                CheckLevel => {
                    if level.is_null() { return Err(RuntimeError::new("LoadMap produced no canonical ULevel")); }
                    Ok(0)
                }
                GetInfo => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    self.context.info.set(info);
                    Ok(0)
                }
                ActorCount => self.output(crate::level_owner::actor_count(level)?.max(0) as usize),
                Actor => self.output(crate::level_owner::actor_at(level, a as i32) as usize),
                Collision => { crate::level_owner::set_actor_collision(self.access, level, true)?; Ok(0) }
                SetZone => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    crate::level_zone_owner::set_actor_zone(self.access, level, actor, true, true)?;
                    Ok(0)
                }
                SetBase => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    let base = crate::level_owner::actor_at(level, b as i32);
                    crate::level_lifecycle_owner::set_base(self.access, actor, base, false)?;
                    Ok(0)
                }
                ClearBase => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    crate::level_lifecycle_owner::set_base(self.access, actor, ptr::null_mut(), false)?;
                    Ok(0)
                }
                DestroyActor => {
                    crate::level_lifecycle_owner::destroy_actor(self.access, a as *mut ULevel, b as *mut AActor, false)?;
                    Ok(0)
                }
                DeletePending => {
                    crate::network_owner::cancel_game_pending(self.access, engine)?;
                    self.context.pending.set(ptr::null_mut());
                    Ok(0)
                }
                ClearEnginePending => { (*engine).g_pending_level = ptr::null_mut(); Ok(0) }
                EnginePending => self.output((*engine).g_pending_level as usize),
                Entry => self.output((*engine).g_entry as usize),
                IsClient => self.output(usize::from(self.access.process_mode().is_client)),
                Hub => {
                    let info = crate::package_map_level_owner::level_info(a as *mut ULevel)?;
                    self.output(self.read_reflected::<i32>(info.cast(), "HubStackLevel")? as usize)
                }
                Pauser => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    self.text(ptr::addr_of_mut!((*info).pauser), &[0])?;
                    Ok(0)
                }
                ClearAction => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    ptr::addr_of_mut!((*info).level_action).write_unaligned(0);
                    Ok(0)
                }
                ViewportActor => self.output((*(a as *mut UViewport)).actor as usize),
                HideMenu => {
                    (*(a as *mut hp2_abi::engine::generated::APlayerPawn)).set_b_show_menu(false);
                    Ok(0)
                }
                FadePositive => {
                    let console = (*(a as *mut UViewport)).console.cast::<hp2_abi::engine::generated::UConsole>();
                    self.output(usize::from(!console.is_null() && (*console).fadeout_time > 0.0))
                }
                FogPositive => {
                    let actor = (*(a as *mut UViewport)).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    self.output(usize::from(!actor.is_null() && (*actor).flash_fog.w > 0.0))
                }
                Draw => {
                    crate::game_draw_owner::draw(self.access, engine, a as *mut UViewport, false, ptr::null_mut(), ptr::null_mut())?;
                    Ok(0)
                }
                Audio => self.output((*engine).audio as usize),
                AudioViewport => {
                    let audio = a as *mut hp2_abi::engine::generated::UALAudioSubsystem;
                    self.output(if audio.is_null() { 0 } else { (*audio).viewport as usize })
                }
                AudioSetViewport => {
                    let audio = crate::audio_owner::engine_audio(
                        self.access,
                        std::ptr::NonNull::new(engine.cast::<hp2_abi::engine::generated::UEngine>())
                            .ok_or_else(|| RuntimeError::new("Null GameEngine"))?,
                    )?;
                    crate::audio_owner::set_viewport(self.access, audio.as_ptr(), b as *mut UViewport)?;
                    Ok(0)
                }
                InitExecution => { self.access.initialize_execution((a as *mut AActor).cast())?; Ok(0) }
                BegunPlay => self.output(usize::from((*self.context.info.get()).b_begun_play())),
                LogBringUp => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    let full_name = utf32_to_string(&self.access.object_full_name(info.cast())?)?;
                    let rate = crate::game_engine_owner::get_max_tick_rate(self.access, engine)?.round() as i32;
                    self.log_format(&format!("Bringing {full_name} up for play ({rate})..."))?;
                    Ok(0)
                }
                ClearTime => { ptr::addr_of_mut!((*level).time_seconds).cast::<f64>().write_unaligned(0.0); Ok(0) }
                ClearInfoTime => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    (*info).time_seconds = 0.0;
                    Ok(0)
                }
                SyncTime => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    ptr::addr_of_mut!((*level).time_seconds).cast::<f64>().write_unaligned(f64::from((*info).time_seconds));
                    Ok(0)
                }
                FirstDynamic => { (*level).i_first_dynamic_actor = a as i32; Ok(0) }
                DrewWorld => {
                    let console = (*(a as *mut UViewport)).console.cast::<hp2_abi::engine::generated::UConsole>();
                    self.output(usize::from(!console.is_null() && (*console).b_drew_world()))
                }
                ClearFog => {
                    let actor = (*(a as *mut UViewport)).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    if !actor.is_null() { ptr::addr_of_mut!((*actor).flash_fog.w).write_unaligned(0.0); }
                    Ok(0)
                }
                FadeStep => {
                    let viewport = a as *mut UViewport;
                    let console = (*viewport).console.cast::<hp2_abi::engine::generated::UConsole>();
                    let actor = (*viewport).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    if console.is_null() || actor.is_null() { return Err(RuntimeError::new("LoadMap fade requires viewport console and actor")); }
                    let current = hp2_core::unix_clock::seconds();
                    let previous = self.context.previous_tick.get()
                        .ok_or_else(|| RuntimeError::new("LoadMap fade-time scope is absent"))?;
                    (*actor).flash_fog.w = game_load_map::fade_fog(current, previous, (*console).fadeout_time * load_fade_scale(), (*actor).flash_fog.w);
                    self.context.previous_tick.set(Some(current));
                    Ok(0)
                }
                PackageLinker => {
                    let map = self.context.package_map.get();
                    if map.is_null() { return Err(RuntimeError::new("Null LoadMap package map")); }
                    let entries = (*map).list;
                    if entries.data.is_null() || a >= entries.num.max(0) as usize {
                        return Err(RuntimeError::new("LoadMap package index is out of range"));
                    }
                    let entry = entries.data.cast::<hp2_abi::engine::generated::FPackageInfo>().add(a);
                    let parent = ptr::addr_of!((*entry).parent).read_unaligned().cast::<UObject>();
                    let units = ptr::addr_of!((*entry).url).read_unaligned();
                    if units.num <= 0 || units.data.is_null() {
                        return Err(RuntimeError::new("LoadMap package has an empty URL"));
                    }
                    let path_text = std::slice::from_raw_parts(units.data.cast::<u32>(), units.num as usize);
                    let package_name = hp2_core::native_path::path_from_units(path_text)?;
                    let guid = ptr::addr_of!((*entry).guid).read_unaligned();
                    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
                    let flags = constants.verify | constants.throw_flag | 0x20 | constants.no_verify;
                    let linker = self.access.get_package_linker(&package_name, flags, parent, Some(&guid))?;
                    ptr::addr_of_mut!((*entry).linker).write_unaligned(linker.cast());
                    Ok(0)
                }
                VerifyLinker => {
                    let map = self.context.package_map.get();
                    if map.is_null() { return Err(RuntimeError::new("Null LoadMap package map")); }
                    let entries = (*map).list;
                    if entries.data.is_null() || a >= entries.num.max(0) as usize {
                        return Err(RuntimeError::new("LoadMap package index is out of range"));
                    }
                    let linker = ptr::addr_of!((*entries.data.cast::<hp2_abi::engine::generated::FPackageInfo>().add(a)).linker)
                        .read_unaligned().cast::<UObject>();
                    if linker.is_null() { return Err(RuntimeError::new("LoadMap package linker was not created")); }
                    unsafe { self.access.verify_linker(linker)?; }
                    Ok(0)
                }
                MapParent => {
                    let map = self.context.package_map.get();
                    if map.is_null() { return Err(RuntimeError::new("Null LoadMap package map")); }
                    let entries = (*map).list;
                    if entries.num > 0 && !entries.data.is_null() {
                        let entry = entries.data.cast::<hp2_abi::engine::generated::FPackageInfo>();
                        self.context.map_parent.set(ptr::addr_of!((*entry).parent).read_unaligned().cast());
                    }
                    Ok(0)
                }
                VerifyMap => {
                    let filename = self.url_map()?;
                    let class = self.access.native_class("Engine.Level")?;
                    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
                    let flags = constants.verify | constants.throw_flag | 0x20;
                    let _ = self.access.static_load_object_from_file(
                        class, self.context.map_parent.get(), &[77,121,76,101,118,101,108,0], &filename, flags,
                    )?;
                    Ok(0)
                }
                SetHub => {
                    let info = self.context.info.get();
                    self.write_reflected(info.cast(), "HubStackLevel", &(a as i32))?;
                    Ok(0)
                }
                GenerationMismatch => {
                    let map = self.context.package_map.get();
                    if map.is_null() { return Err(RuntimeError::new("Null LoadMap package map")); }
                    let entries = (*map).list;
                    if entries.data.is_null() || a >= entries.num.max(0) as usize {
                        return Err(RuntimeError::new("LoadMap package index is out of range"));
                    }
                    let entry = entries.data.cast::<hp2_abi::engine::generated::FPackageInfo>().add(a);
                    let local = ptr::addr_of!((*entry).local_generation).read_unaligned();
                    let remote = ptr::addr_of!((*entry).remote_generation).read_unaligned();
                    self.output(usize::from(local != remote))
                }
                CheckPending => {
                    if self.context.pending.get() != (*engine).g_pending_level.cast() {
                        return Err(RuntimeError::new("LoadMap pending level does not match GameEngine"));
                    }
                    Ok(0)
                }
                AttachNet => {
                    if !level.is_null() && !self.context.pending.get().is_null() {
                        (*level).base.net_driver = (*self.context.pending.get()).net_driver;
                    }
                    Ok(0)
                }
                NotifyNet => {
                    let driver = (*level).base.net_driver.cast::<hp2_abi::engine::generated::UNetDriver>();
                    if !driver.is_null() { (*driver).notify = level.cast(); }
                    Ok(0)
                }
                AttachDemo => {
                    if !level.is_null() && !self.context.pending.get().is_null() {
                        (*level).base.demo_rec_driver = (*self.context.pending.get()).demo_rec_driver;
                    }
                    Ok(0)
                }
                NotifyDemo => {
                    let driver = (*level).base.demo_rec_driver.cast::<UObject>();
                    if !driver.is_null() {
                        let notify = ptr::addr_of_mut!((*driver.cast::<hp2_abi::engine::generated::UDemoRecDriver>()).base.notify);
                        notify.write_unaligned(level.cast());
                    }
                    Ok(0)
                }
                ClientMode => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    ptr::addr_of_mut!((*info).net_mode).write_unaligned(3);
                    Ok(0)
                }
                LogMap => {
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    let mut rendered = crate::url_owner::UrlOwner::string(self.access, &defaults, self.context.url, false)?;
                    let mut message: Vec<u32> = "LoadMap: ".chars().map(u32::from).collect();
                    message.extend(string_units(&rendered));
                    let category = self.access.intern_str("Log", FindName::Add)?;
                    self.access.emit_log(category, &message)?;
                    self.access.destroy_owned_fstring(&mut rendered)?;
                    Ok(0)
                }
                CheckNoNet => {
                    if !(*level).base.net_driver.is_null() {
                        return Err(RuntimeError::new("LoadMap expected no level net driver"));
                    }
                    Ok(0)
                }
                IsEntry => {
                    let outer = (*level).base.base.outer;
                    let name = self.access.name_text(i32::from_ne_bytes((*outer.cast::<hp2_abi::engine::generated::UObject>()).name))?;
                    self.output(usize::from(name.iter().copied().filter(|&c| c != 0).eq("Entry".chars().map(u32::from))))
                }
                AssignUrl => {
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    crate::url_owner::UrlOwner::assign_copy(
                        self.access, ptr::addr_of_mut!((*level).base.url), self.context.url,
                    )?;
                    Ok(0)
                }
                Version => {
                    let info = self.context.info.get();
                    let version = format!("{}", crate::level_net::ENGINE_VERSION);
                    let text: Vec<u32> = version.chars().map(u32::from).chain(std::iter::once(0)).collect();
                    self.text(self.reflected_field(info.cast(), "EngineVersion")?.cast(), &text)?;
                    Ok(0)
                }
                MinVersion => {
                    let info = self.context.info.get();
                    let version = format!("{}", crate::level_net::ENGINE_MIN_NET_VERSION);
                    let text: Vec<u32> = version.chars().map(u32::from).chain(std::iter::once(0)).collect();
                    self.text(self.reflected_field(info.cast(), "MinNetVersion")?.cast(), &text)?;
                    Ok(0)
                }
                SetEngine => { (*level).base.engine = engine.cast(); Ok(0) }
                Travel => self.output(self.context.travel_info as usize),
                AssignTravel => {
                    game_state_trace("travel_assign_begin", || format!("level={level:p} travel={:p}", self.context.travel_info));
                    let result = if !self.context.travel_info.is_null() {
                        crate::level_owner::assign_travel_map(self.access, ptr::addr_of_mut!((*level).travel_info), self.context.travel_info)
                    } else { Ok(()) };
                    game_state_trace("travel_assign_end", || format!("level={level:p} returned={}", result.is_ok()));
                    result?;
                    Ok(0)
                }
                Flush => { crate::engine_owner::flush(self.access, engine.cast(), false)?; Ok(0) }
                SaveFilename => {
                    let system = self.access.system()?;
                    let raw = ptr::addr_of!((*system).save_slot_path).read_unaligned();
                    if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
                        return Err(RuntimeError::new("Invalid canonical SaveSlotPath"));
                    }
                    let prefix = utf32_to_string(if raw.num > 0 {
                        std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize)
                    } else { &[] })?;
                    let filename = format!("{prefix}{}Game{}.usa", std::path::MAIN_SEPARATOR, self.context.saved_hub.get());
                    let units: Vec<u32> = filename.chars().map(u32::from).chain(std::iter::once(0)).collect();
                    let target = self.context.filename.get();
                    if target.is_null() { return Err(RuntimeError::new("LoadMap filename scope is absent")); }
                    (*target).clear();
                    (*target).extend(units);
                    Ok(0)
                }
                SavePackage => {
                    let filename = self.context.filename.get();
                    if filename.is_null() { return Err(RuntimeError::new("LoadMap filename scope is absent")); }
                    let path = utf32_to_string(&(**filename)[..(**filename).len().saturating_sub(1)])?;
                    let outer = (*level).base.base.outer.cast::<UObject>();
                    let saved = self.access.save_package(outer, level.cast(), 0, std::path::Path::new(&path), None)?;
                    self.output(usize::from(saved))
                }
                Persistent => {
                    let map = hp2_core::native_text::fstring_text(ptr::addr_of!((*self.context.url).map));
                    game_state_trace("persistent_begin", || format!("level={level:p} travel={:p}", self.context.travel_info));
                    let outcome = crate::level_persist_owner::load_persistent_actors(self.access, level, map);
                    game_state_trace("persistent_end", || match &outcome {
                        Ok(value) => format!("level={level:p} returned=ok value={value}"),
                        Err(error) => format!("level={level:p} returned=error error={error:?}"),
                    });
                    outcome?;
                    Ok(0)
                }
                IteratorValid => {
                    let actors = self.context.actor_iterator.get();
                    if actors.is_null() { return Err(RuntimeError::new("LoadMap actor iterator scope is absent")); }
                    self.output(usize::from((*actors).index < self.access.object_count()))
                }
                IteratorActor => {
                    let actors = self.context.actor_iterator.get();
                    if actors.is_null() { return Err(RuntimeError::new("LoadMap actor iterator scope is absent")); }
                    self.output(self.access.object_at((*actors).index) as usize)
                }
                IteratorNext => {
                    let iterator = self.context.actor_iterator.get().as_mut()
                        .ok_or_else(|| RuntimeError::new("LoadMap actor iterator scope is absent"))?;
                    iterator.advance(self.access)?;
                    Ok(0)
                }
                InLevel => {
                    let actor = a as *mut AActor;
                    let outer = (*level).base.base.outer;
                    self.output(usize::from(hp2_core::object_model::is_in(actor.cast(), outer)))
                }
                Eliminate => { (*(a as *mut UObject)).object_flags |= hp2_core::object_gc::RF_ELIMINATE_OBJECT; Ok(0) }
                KeepActor => { (*(a as *mut UObject)).object_flags &= !hp2_core::object_gc::RF_ELIMINATE_OBJECT; Ok(0) }
                Relevant => self.output(usize::from((*(a as *mut AActor)).b_always_relevant())),
                ReorderedCount => {
                    let actors = self.context.reordered_actors.get();
                    self.output(if actors.is_null() { 0 } else { (*actors).len() })
                }
                AddActor => {
                    let actors = self.context.reordered_actors.get();
                    if actors.is_null() { return Err(RuntimeError::new("LoadMap actor reorder scope is absent")); }
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    (*actors).push(actor);
                    Ok(0)
                }
                FirstRelevant => { (*level).i_first_net_relevant_actor = a as i32; Ok(0) }
                EmptyActors => {
                    hp2_core::array_storage::empty(
                        hp2_core::array_storage::Hp2ArraySlots {
                            data: ptr::addr_of_mut!((*level).base.actors.data).cast(),
                            num: ptr::addr_of_mut!((*level).base.actors.array_num),
                            max: ptr::addr_of_mut!((*level).base.actors.array_max),
                        }, 0, std::mem::size_of::<*mut AActor>() as i32, ptr::null(), hp2_core::array_storage::system_allocator(),
                    ).map_err(|_| RuntimeError::new("Unable to empty LoadMap actor array"))?;
                    Ok(0)
                }
                AllocateActors => {
                    let actors = self.context.reordered_actors.get();
                    if actors.is_null() { return Err(RuntimeError::new("LoadMap actor reorder scope is absent")); }
                    hp2_core::array_storage::add(
                        hp2_core::array_storage::Hp2ArraySlots {
                            data: ptr::addr_of_mut!((*level).base.actors.data).cast(),
                            num: ptr::addr_of_mut!((*level).base.actors.array_num),
                            max: ptr::addr_of_mut!((*level).base.actors.array_max),
                        }, (*actors).len() as i32, std::mem::size_of::<*mut AActor>() as i32, ptr::null(), hp2_core::array_storage::system_allocator(),
                    ).map_err(|_| RuntimeError::new("Unable to allocate reordered LoadMap actors"))?;
                    Ok(0)
                }
                CopyActor => {
                    let actors = self.context.reordered_actors.get();
                    if actors.is_null() { return Err(RuntimeError::new("LoadMap actor reorder scope is absent")); }
                    let source = (&*actors).get(a).copied().ok_or_else(|| RuntimeError::new("LoadMap actor reorder index is out of range"))?;
                    (*level).base.actors.data.cast::<*mut AActor>().add(a).write_unaligned(source);
                    Ok(0)
                }
                TouchCount => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    self.output(array_extent(ptr::addr_of!((*actor).touching)))
                }
                ClearTouch => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    if b >= array_extent(ptr::addr_of!((*actor).touching)) { return Err(RuntimeError::new("LoadMap touching actor index is out of range")); }
                    (*actor).touching[b] = ptr::null_mut();
                    Ok(0)
                }
                Static => self.output(usize::from((*(a as *mut AActor)).b_static())),
                NoDelete => self.output(usize::from((*(a as *mut AActor)).b_no_delete())),
                ExchangeRoles => {
                    let actor = a as *mut AActor;
                    let role = (*actor).role;
                    (*actor).role = (*actor).remote_role;
                    (*actor).remote_role = role;
                    Ok(0)
                }
                ScriptInitialized => self.output(usize::from((*(a as *mut AActor)).b_script_initialized())),
                HasAttach => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    self.output(usize::from(self.read_reflected::<i32>(actor.cast(), "AttachTag")? != 0))
                }
                AttachMatches => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    let target = crate::level_owner::actor_at(level, b as i32);
                    let attach = self.read_reflected::<i32>(actor.cast(), "AttachTag")?;
                    let tag = self.read_reflected::<i32>(target.cast(), "Tag")?;
                    self.output(usize::from(tag == attach))
                }
                Base => self.output((*(crate::level_owner::actor_at(level, a as i32))).base as usize),
                CollideWorld => self.output(usize::from((*(crate::level_owner::actor_at(level, a as i32))).b_collide_world())),
                PhysicsNone => self.output(usize::from((*(crate::level_owner::actor_at(level, a as i32))).physics == 0)),
                PhysicsRotating => self.output(usize::from((*(crate::level_owner::actor_at(level, a as i32))).physics == 5)),
                Info => self.output(self.context.info.get() as usize),
                ClearPlayer => { (*(a as *mut hp2_abi::engine::generated::APlayerPawn)).player = ptr::null_mut(); Ok(0) }
                ClearViewportActor => {
                    let client = (*engine).client.cast::<hp2_abi::engine::generated::UClient>();
                    let views = (*client).viewports;
                    let viewport = views.data.cast::<*mut UViewport>().add(a).read_unaligned();
                    (*viewport).actor = ptr::null_mut();
                    Ok(0)
                }
                Notify => { crate::game_travel_owner::run(self.access, engine, level, self.context.url, 102, 0)?; Ok(0) }
                RestoreFog => {
                    let actor = (*(a as *mut UViewport)).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    ptr::addr_of_mut!((*actor).flash_fog.w).write_unaligned(1.0);
                    Ok(0)
                }
                RestoreGlow => {
                    let actor = (*(a as *mut UViewport)).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    ptr::addr_of_mut!((*actor).constant_glow_fog.w).write_unaligned(0.0);
                    Ok(0)
                }
                RestoreFade => {
                    let actor = (*(a as *mut UViewport)).actor.cast::<hp2_abi::engine::generated::APlayerPawn>();
                    ptr::addr_of_mut!((*actor).fade_rate).write_unaligned(0.0);
                    Ok(0)
                }
                FindBase => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    crate::actor_physics_owner::find_base(self.access, actor)?;
                    Ok(0)
                }
                PreBegin => { let _ = self.access.execute_named_event(crate::level_owner::actor_at(level, a as i32).cast(), "PreBeginPlay")?; Ok(0) }
                BeginPlay => { let _ = self.access.execute_named_event(crate::level_owner::actor_at(level, a as i32).cast(), "BeginPlay")?; Ok(0) }
                PostBegin => { let _ = self.access.execute_named_event(crate::level_owner::actor_at(level, a as i32).cast(), "PostBeginPlay")?; Ok(0) }
                InitialState => { let _ = self.access.execute_named_event(crate::level_owner::actor_at(level, a as i32).cast(), "SetInitialState")?; Ok(0) }
                TraceMap => {
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    let mut rendered = crate::url_owner::UrlOwner::string(self.access, &defaults, self.context.url, false)?;
                    let map = self.url_map()?;
                    hp2_core::ansi_trace::set_process_startup_map(&map, &string_units(&rendered))?;
                    self.access.destroy_owned_fstring(&mut rendered)?;
                    Ok(0)
                }
                AudioCleanup => Ok(0), // UAudioSubsystem::CleanUp is the native empty virtual.
                ActorLevel => self.output(crate::actor::get_level(a as *const AActor) as usize),
                Option => {
                    const NAMES: [&[u32]; 5] = [&[108,111,97,100,0], &[112,117,115,104,0], &[112,111,112,0], &[112,101,101,114,0], &[76,105,115,116,101,110,0]];
                    let name = NAMES.get(a).ok_or_else(|| RuntimeError::new("LoadMap URL option index is out of range"))?;
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    self.output(usize::from(unsafe { crate::url_owner::UrlOwner::has_option(self.access, &defaults, self.context.url, name.as_ptr())? }))
                }
                LoadOption => {
                    const LOAD: [u32; 5] = [108,111,97,100,0];
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    let value = unsafe { crate::url_owner::UrlOwner::get_option(self.access, &defaults, self.context.url, LOAD.as_ptr(), ptr::null())? };
                    self.output(value as usize)
                }
                ClearLevel => { (*engine).g_level = ptr::null_mut(); Ok(0) }
                EnterTextEmpty => {
                    let info = self.context.info.get();
                    let text = ptr::addr_of!((*info).level_enter_text).read_unaligned();
                    self.output(usize::from(text.num == 0 || text.data.is_null() || *text.data.cast::<u32>() == 0))
                }
                EnterText => {
                    let info = self.context.info.get();
                    let map = self.url_map()?;
                    self.text(ptr::addr_of_mut!((*info).level_enter_text), &map)?;
                    Ok(0)
                }
                DemoKind => self.output(0),
                DemoSuffix => {
                    let path = self.context.demo_file.get();
                    if path.is_null() { return Err(RuntimeError::new("LoadMap demo filename scope is absent")); }
                    let path = &*path;
                    let length = path.iter().position(|&unit| unit == 0).unwrap_or(path.len());
                    let suffix = path.as_ptr().add(length.saturating_sub(4));
                    self.output(usize::from(hp2_core::ansi::stricmp(
                        suffix.cast(), hp2_core::lit!(".unr").as_ptr().cast()) == 0))
                }
                DemoAppend => {
                    let path = self.context.demo_file.get();
                    if path.is_null() { return Err(RuntimeError::new("LoadMap demo filename scope is absent")); }
                    (*path).pop();
                    (*path).extend(".unr".chars().map(u32::from));
                    (*path).push(0);
                    Ok(0)
                }
                DemoSize => {
                    let path = self.context.demo_file.get();
                    if path.is_null() { return Err(RuntimeError::new("LoadMap demo filename scope is absent")); }
                    let path = hp2_core::native_path::path_from_units(&*path)?;
                    self.context.demo_size.set(hp2_core::unix_file_manager_owner::file_size(self.access, &path)?);
                    Ok(0)
                }
                DemoLog => {
                    let path = self.context.demo_file.get();
                    if path.is_null() { return Err(RuntimeError::new("LoadMap demo filename scope is absent")); }
                    let text = utf32_to_string(&*path)?;
                    self.log_format(&format!("Looking for file: {text} {}", self.context.demo_size.get()))?;
                    Ok(0)
                }
                DemoName => {
                    const DEMOS: [&[u32]; 11] = [
                        hp2_core::lit!("../MAPS/DEMO_PC.UNR"), hp2_core::lit!("../MAPS/ENTRY.UNR"),
                        hp2_core::lit!("../MAPS/STARTUP.UNR"), hp2_core::lit!("../MAPS/DM-TURBINEDEMO.UNR"),
                        hp2_core::lit!("../MAPS/DM-PHOBOSDEMO.UNR"), hp2_core::lit!("../MAPS/DM-MORPHEUSDEMO.UNR"),
                        hp2_core::lit!("../MAPS/DM-TEMPESTDEMO.UNR"), hp2_core::lit!("../MAPS/CTF-CORETDEMO.UNR"),
                        hp2_core::lit!("../MAPS/DOM-SESMARDEMO.UNR"), hp2_core::lit!("../MAPS/ENTRY.UNR"),
                        hp2_core::lit!("../MAPS/UT-LOGO-MAP.UNR"),
                    ];
                    let file = self.context.demo_file.get();
                    if file.is_null() { return Err(RuntimeError::new("LoadMap demo filename scope is absent")); }
                    let name = DEMOS.get(a).ok_or_else(|| RuntimeError::new("LoadMap demo-name index is out of range"))?;
                    self.output(usize::from(hp2_core::ansi::stricmp((*file).as_ptr().cast(), name.as_ptr().cast()) == 0))
                }
                DemoFileSize => self.output(self.context.demo_size.get() as usize),
                DemoError => { self.text(self.context.error, &"Sorry, only the retail version of UT can load third party maps.".chars().map(u32::from).chain([0]).collect::<Vec<_>>())?; Ok(0) }
                Progress => {
                    let section = [69,114,114,111,114,115,0];
                    let key = [85,114,108,70,97,105,108,101,100,0];
                    let msg = self.access.localize_text(&section, &key, Some(&[67,111,114,101,0]), None, false)?;
                    let error = string_units(unsafe { &*self.context.error });
                    let empty = [0u32];
                    crate::game_engine_owner::set_progress(self.access, engine, msg.as_ptr(), if error.is_empty() { empty.as_ptr() } else { error.as_ptr() }, 6.0)?;
                    Ok(0)
                }
                CatchError => {
                    let caught = self.context.catch_error.get();
                    if caught.is_null() { return Err(RuntimeError::new("LoadMap catch-error scope is absent")); }
                    self.text(self.context.error, &**caught)?;
                    Ok(0)
                }
                DumpAllocs => {
                    let category = self.access.intern_str("Exit", FindName::Find)?;
                    let message: Vec<u32> = "Allocation checking disabled".chars().map(u32::from).collect();
                    self.access.emit_log(category, &message)?;
                    Ok(0)
                }
                DrawInfo => {
                    let viewport = a as *mut UViewport;
                    let console = (*viewport).console.cast::<UObject>();
                    let canvas = (*viewport).canvas.cast::<hp2_abi::engine::generated::UCanvas>();
                    let info = self.context.info.get();
                    let function = self.reflected_function(console, "DrawLevelInfo")?;
                    #[repr(C)]
                    struct DrawLevelInfoParams { canvas: *mut hp2_abi::engine::generated::UCanvas, url: TArrayRaw }
                    let mut params = DrawLevelInfoParams { canvas, url: ptr::addr_of!((*info).level_enter_text).read_unaligned() };
                    self.access.execute_event(console, function, (&mut params as *mut DrawLevelInfoParams).cast(), ptr::null_mut())?;
                    Ok(0)
                }
                ViewportLock => {
                    let zero = FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
                    let locked = crate::platform_owner::lock_viewport(
                        self.access, a as *mut UViewport, &zero, &zero, &zero, 0, ptr::null_mut(), ptr::null_mut(),
                    )?;
                    self.output(usize::from(locked))
                }
                EndFlash => {
                    let viewport = a as *mut UViewport;
                    crate::render_device_owner::end_flash(self.access, (*viewport).ren_dev.cast())?;
                    Ok(0)
                }
                Console => self.output((*(a as *mut UViewport)).console as usize),
                Canvas => self.output((*(a as *mut UViewport)).canvas as usize),
                Unlock => {
                    crate::platform_owner::unlock_viewport(self.access, a as *mut UViewport, true)?;
                    Ok(0)
                }
                LoadLevel => {
                    let filename = self.url_map()?;
                    let class = self.access.native_class("Engine.Level")?;
                    let loaded = self.access.static_load_object_from_file(
                        class, self.context.map_parent.get(), &[77,121,76,101,118,101,108,0], &filename, 0x0000_0001,
                    )?;
                    (*engine).g_level = loaded.cast();
                    Ok(0)
                }
                HasSlotDump => self.output(usize::from(crate::core_owner::actor_slot_dump_available(self.access)?)),
                SlotDump => {
                    crate::core_owner::observe_actor_slot_dump(
                        self.access,
                        crate::core_owner::ActorSlotDumpEvent::PostDeserialize { level: level.cast() },
                    )?;
                    Ok(0)
                }
                ResetLoaders => { self.access.reset_loaders((*level).base.base.outer.cast(), false, true)?; Ok(0) }
                Brush => self.output((*level).brush_tracker as usize),
                DeleteBrush => { crate::dynbsp_owner::destroy(self.access, level)?; Ok(0) }
                ClearBrush => { (*level).brush_tracker = ptr::null_mut(); Ok(0) }
                NetDriver => self.output((*level).base.net_driver as usize),
                DeleteNet => {
                    let driver = (*level).base.net_driver.cast::<UObject>();
                    if !driver.is_null() { self.access.conditional_destroy(driver)?; self.access.delete_object(driver)?; }
                    Ok(0)
                }
                ClearNet => { (*level).base.net_driver = ptr::null_mut(); Ok(0) }
                Demo => self.output((*level).base.demo_rec_driver as usize),
                DeleteDemo => {
                    let driver = (*level).base.demo_rec_driver.cast::<UObject>();
                    if !driver.is_null() { self.access.conditional_destroy(driver)?; self.access.delete_object(driver)?; }
                    Ok(0)
                }
                ClearDemo => { (*level).base.demo_rec_driver = ptr::null_mut(); Ok(0) }
                CleanupDestroyed => { crate::level_lifecycle_owner::cleanup_destroyed(self.access, level)?; Ok(0) }
                LogHave => {
                    let map = self.context.package_map.get();
                    let entries = if map.is_null() { return Err(RuntimeError::new("Null LoadMap package map")); } else { (*map).list };
                    if entries.data.is_null() || a >= entries.num.max(0) as usize { return Err(RuntimeError::new("LoadMap package index is out of range")); }
                    let entry = entries.data.cast::<hp2_abi::engine::generated::FPackageInfo>().add(a);
                    let guid = ptr::addr_of!((*entry).guid).read_unaligned();
                    let generation = ptr::addr_of!((*entry).local_generation).read_unaligned();
                    let pending = self.context.pending.get();
                    if pending.is_null() { return Err(RuntimeError::new("LoadMap generation mismatch has no PendingLevel")); }
                    let driver = (*pending).net_driver.cast::<hp2_abi::engine::generated::UNetDriver>();
                    if driver.is_null() || (*driver).server_connection.is_null() { return Err(RuntimeError::new("LoadMap PendingLevel has no server connection")); }
                    let text: Vec<u32> = format!("HAVE GUID={:08X}{:08X}{:08X}{:08X} GEN={generation}", guid[0], guid[1], guid[2], guid[3]).chars().map(u32::from).collect();
                    unsafe { crate::level_net_owner::log_connection(self.access, (*driver).server_connection.cast(), &text)?; }
                    Ok(0)
                }
                VerifyClasses => {
                    for (class_name, field, offset) in [
                        ("Engine.Actor", "Owner", std::mem::offset_of!(AActor, owner)),
                        ("Engine.Actor", "TimerCounter", std::mem::offset_of!(AActor, timer_counter)),
                        ("Engine.PlayerPawn", "Player", std::mem::offset_of!(hp2_abi::engine::generated::APlayerPawn, player)),
                        ("Engine.PlayerPawn", "MaxStepHeight", std::mem::offset_of!(hp2_abi::engine::generated::APlayerPawn, base.max_step_height)),
                    ] {
                        let class = self.access.native_class(class_name)?;
                        let name = self.access.intern_str(field, FindName::Find)?;
                        let property = self.access.find_field(class.cast(), name, self.access.native_class("Property")?);
                        if property.is_null() { return Err(RuntimeError::new("LoadMap class layout property is missing").with_context(field)); }
                        let actual = (*property.cast::<hp2_abi::engine::generated::UProperty>()).offset;
                        if actual != offset as i32 { return Err(RuntimeError::new("LoadMap class layout offset mismatch").with_context(field)); }
                    }
                    Ok(0)
                }
                ComputerName => {
                    let services = hp2_core::unix_text::Hp2UnixTextServices {
                        opaque: ptr::null_mut(), allocate: loadmap_text_allocate, fatal: loadmap_text_fatal,
                    };
                    let name = unsafe { hp2_core::unix_identity::computer_name(&services) }
                        .map_err(|_| RuntimeError::new("appComputerName failed"))?;
                    let mut units = Vec::new();
                    let mut current = name;
                    unsafe { while !current.is_null() && *current != 0 { units.push(*current as u32); current = current.add(1); } }
                    units.push(0);
                    self.text(self.reflected_field(self.context.info.get().cast(), "ComputerName")?.cast(), &units)?;
                    Ok(0)
                }
                InfoHub => self.output(self.read_reflected::<i32>(self.context.info.get().cast(), "HubStackLevel")? as usize),
                Gc => {
                    let keep = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.native;
                    // Debug mode: HP2_GC_TRACE=<substring> logs every GC mark
                    // edge whose object name contains the substring (with the
                    // referrer) plus matching purges, to stderr.
                    let trace: Vec<u32> = std::env::var("HP2_GC_TRACE").unwrap_or_default().chars().map(u32::from).collect();
                    let output = |event: hp2_core::core_gc::GcLog| -> RuntimeResult<()> {
                        if !trace.is_empty() {
                            let text = |units: &[u32]| units.iter().take_while(|&&unit| unit != 0).filter_map(|&unit| char::from_u32(unit)).collect::<String>();
                            let matches = |units: &[u32]| !trace.is_empty() && units.windows(trace.len()).any(|window| window == trace);
                            match event {
                                hp2_core::core_gc::GcLog::Collecting => {
                                    let index = (*level.cast::<hp2_abi::engine::generated::UObject>()).index;
                                    let name = self.access.object_full_name(level.cast()).map(|units| text(&units)).unwrap_or_default();
                                    eprintln!("HP2_GC pass begin level={name} index={index}");
                                }
                                hp2_core::core_gc::GcLog::Marked { object, context } if matches(&object) => {
                                    eprintln!("HP2_GC marked {} <- {:?}", text(&object), context.as_ref().map(|units| text(units)));
                                }
                                hp2_core::core_gc::GcLog::Referenced { object, context } if matches(&object) => {
                                    eprintln!("HP2_GC referenced {} <- {:?}", text(&object), context.as_ref().map(|units| text(units)));
                                }
                                hp2_core::core_gc::GcLog::GarbageCollected { object, .. } if matches(&object) => {
                                    eprintln!("HP2_GC purged {}", text(&object));
                                }
                                _ => {}
                            }
                        }
                        Ok(())
                    };
                    self.access.collect_garbage(keep, 0, &output)?;
                    Ok(0)
                }
                Connectivity => {
                    let output = b as *mut u64;
                    if output.is_null() { return Err(RuntimeError::new("Null LoadMap zone connectivity output")); }
                    let model = (*level).model.cast::<hp2_abi::engine::generated::UModel>();
                    if model.is_null() || a >= 64 { return Err(RuntimeError::new("Invalid LoadMap zone connectivity access")); }
                    let connectivity = ptr::addr_of!((*model).zones).cast::<u8>().add(a * 32 + 16).cast::<u64>().read_unaligned();
                    output.write_unaligned(connectivity);
                    Ok(0)
                }
                ZoneDistances => self.output(ptr::addr_of_mut!((*level).zone_dist).cast::<u8>() as usize),
                UpdateTime => {
                    let info = crate::package_map_level_owner::level_info(level)?;
                    crate::level_tick_owner::update_level_system_time(self.access, info)?;
                    Ok(0)
                }
                OptionCount => self.output((*self.context.url).op.num.max(0) as usize),
                AppendQuestion => {
                    let options = self.context.options.get();
                    if options.is_null() { return Err(RuntimeError::new("LoadMap game-options scope is absent")); }
                    if (*options).last() == Some(&0) { (*options).pop(); }
                    (*options).push(u32::from('?')); (*options).push(0);
                    Ok(0)
                }
                AppendOption => {
                    let options = self.context.options.get();
                    if options.is_null() { return Err(RuntimeError::new("LoadMap game-options scope is absent")); }
                    if (*options).last() == Some(&0) { (*options).pop(); }
                    (*options).extend(self.url_option(a)?); (*options).push(0);
                    Ok(0)
                }
                ParseGame => {
                    let mut option = self.url_option(a)?;
                    option.push(0);
                    let name = self.context.game_class_name.get();
                    if name.is_null() { return Err(RuntimeError::new("LoadMap game-class-name scope is absent")); }
                    let mut parsed = [0u32; 256];
                    let keyword = [71,65,77,69,61,0];
                    unsafe { hp2_core::text_parse::parse_string(option.as_ptr().cast(), keyword.as_ptr(), parsed.as_mut_ptr().cast(), parsed.len() as i32); }
                    (*name).clear(); (*name).extend(parsed.into_iter().take_while(|&u| u != 0)); (*name).push(0);
                    Ok(0)
                }
                IsServer => {
                    let net = (*level).base.net_driver.cast::<hp2_abi::engine::generated::UNetDriver>();
                    let demo = (*level).base.demo_rec_driver.cast::<hp2_abi::engine::generated::UDemoRecDriver>();
                    self.output(usize::from((net.is_null() || (*net).server_connection.is_null()) && (demo.is_null() || (*demo).base.server_connection.is_null())))
                }
                Game => self.output((*self.context.info.get()).game as usize),
                GameName => {
                    let name = self.context.game_class_name.get();
                    if name.is_null() { return Err(RuntimeError::new("LoadMap game-class-name scope is absent")); }
                    self.output((*name).first().copied().unwrap_or(0) as usize)
                }
                DefaultGame => self.output(self.read_reflected::<*mut UObject>(self.context.info.get().cast(), "DefaultGameType")? as usize),
                DefaultGameName => {
                    static DEFAULT: [u32; 30] = [105,110,105,58,69,110,103,105,110,101,46,69,110,103,105,110,101,46,68,101,102,97,117,108,116,71,97,109,101,0];
                    self.output(DEFAULT.as_ptr() as usize)
                }
                DefaultServerGameName => {
                    static DEFAULT: [u32; 36] = [105,110,105,58,69,110,103,105,110,101,46,69,110,103,105,110,101,46,68,101,102,97,117,108,116,83,101,114,118,101,114,71,97,109,101,0];
                    self.output(DEFAULT.as_ptr() as usize)
                }
                GameNamePointer => {
                    let name = self.context.game_class_name.get();
                    if name.is_null() { return Err(RuntimeError::new("LoadMap game-class-name scope is absent")); }
                    self.output((*name).as_ptr() as usize)
                }
                LoadDefaultGame => {
                    if a == 0 { return Err(RuntimeError::new("LoadMap default game class name is null")); }
                    let mut class_name = Vec::with_capacity(256);
                    for offset in 0..256 {
                        let unit = (a as *const u32).add(offset).read_unaligned();
                        if unit == 0 { break; }
                        class_name.push(unit);
                    }
                    if class_name.len() == 256 { return Err(RuntimeError::new("LoadMap default game class name is not terminated")); }
                    class_name.push(0);
                    let class = self.access.native_class("Engine.GameInfo")?;
                    self.access.static_load_class(class, ptr::null_mut(), &class_name, 1).map(|class| class as usize)
                }
                LogGame => {
                    let class = a as *mut UObject;
                    let name = self.access.name_text(i32::from_ne_bytes((*class).name))?;
                    let text = utf32_to_string(&name)?;
                    self.log_format(&format!("Game class is '{text}'"))?;
                    Ok(0)
                }
                SpawnGame => {
                    let actor = crate::level_spawn_actor_owner::spawn_actor(
                        self.access, level, a as *mut hp2_abi::engine::generated::UClass,
                        hp2_abi::engine::generated::FName { index: 0 }, ptr::null_mut(), ptr::null_mut(),
                        hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 }, hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 },
                        ptr::null_mut(), false, false,
                    )?;
                    (*self.context.info.get()).game = actor.cast();
                    Ok(0)
                }
                InitGame => {
                    let game = (*self.context.info.get()).game.cast::<UObject>();
                    let mut options = hp2_abi::engine::generated::TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                    let mut error = hp2_abi::engine::generated::TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                    let result = (|| {
                        self.text(&mut options, &self.option_units())?;
                        let function = self.reflected_function(game, "InitGame")?;
                        let mut params = InitGameParams { options, error };
                        let event_result = self.access.execute_event(game.cast(), function, ptr::addr_of_mut!(params).cast(), ptr::null_mut());
                        options = params.options;
                        error = params.error;
                        event_result.map_err(|error| error.with_context("AGameInfo::eventInitGame"))?;
                        if !hp2_core::native_text::fstring_is_empty(&error) {
                            if let Some(target) = self.context.init_error.get().as_mut() { *target = string_units(&error); }
                        }
                        Ok(0)
                    })();
                    let options_cleanup = self.access.destroy_owned_fstring(&mut options);
                    let error_cleanup = self.access.destroy_owned_fstring(&mut error);
                    result.and(options_cleanup).and(error_cleanup)?;
                    Ok(0)
                }
                CheckGame => {
                    if (*self.context.info.get()).game.is_null() { return Err(RuntimeError::new("LoadMap failed to spawn GameInfo")); }
                    Ok(0)
                }
                CheckNotPending => {
                    if !self.context.pending.get().is_null() { return Err(RuntimeError::new("LoadMap expected no pending level")); }
                    Ok(0)
                }
                Listen => {
                    let error = self.context.listen_error.get();
                    if error.is_null() {
                        return Err(RuntimeError::new("LoadMap listen-error scope is absent"));
                    }
                    let has_driver = crate::level_listen_owner::listen(
                        self.access,
                        level,
                        &mut *error,
                    )?;
                    self.output(usize::from(has_driver))
                }
                ListenError => {
                    let error = self.context.listen_error.get();
                    if error.is_null() { return Err(RuntimeError::new("LoadMap listen-error scope is absent")); }
                    Err(RuntimeError::from_utf32((*error).clone()))
                }
                HighDetail => { crate::game_init_owner::set_level_high_detail(self.access, level, a as i32)?; Ok(0) }
                RenderDevice => {
                    let client = (*engine).client.cast::<hp2_abi::engine::generated::UClient>();
                    let viewport = if client.is_null() || (*client).viewports.num <= 0 || (*client).viewports.data.is_null() { ptr::null_mut() }
                        else { (*client).viewports.data.cast::<*mut UViewport>().read_unaligned() };
                    self.output(if viewport.is_null() { 0 } else { (*viewport).ren_dev as usize })
                }
                DeviceDetail => {
                    let device = a as *mut hp2_abi::engine::generated::URenderDevice;
                    self.output(usize::from(!device.is_null() && (*device).high_detail_actors()))
                }
                SetBegun => { (*self.context.info.get()).set_b_begun_play(true); Ok(0) }
                Startup => { self.write_reflected_bool(self.context.info.get().cast(), "bStartup", a != 0)?; Ok(0) }
                Lifecycle => { hp2_core::ansi_trace::set_process_startup_lifecycle_boundary(b"bring_up_for_play_complete"); Ok(0) }
                Decoration => self.output(usize::from(self.object_is_class(crate::level_owner::actor_at(level, a as i32).cast(), "Engine.Decoration")?)),
                Inventory => self.output(usize::from(self.object_is_class(crate::level_owner::actor_at(level, a as i32).cast(), "Engine.Inventory")?)),
                Pawn => self.output(usize::from(self.object_is_class(crate::level_owner::actor_at(level, a as i32).cast(), "Engine.Pawn")?)),
                ScreenActors => {
                    let mut current = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                    let mut buffer = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                    let mut token = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                    let ops = CoreScreenActorsOps { access: self.access, level, group_storage: RefCell::new(Vec::new()) };
                    game_state_trace("screen_begin", || format!("level={level:p} travel={:p}", self.context.travel_info));
                    let result = crate::level::level_screen_actors_body_with(
                        &ops, level.cast(), (&mut current as *mut TArrayRaw).cast(),
                        (&mut buffer as *mut TArrayRaw).cast(), (&mut token as *mut TArrayRaw).cast(),
                    );
                    let current_cleanup = if current.data.is_null() { Ok(()) } else { self.access.destroy_owned_fstring(&mut current) };
                    let buffer_cleanup = if buffer.data.is_null() { Ok(()) } else { self.access.destroy_owned_fstring(&mut buffer) };
                    let token_cleanup = if token.data.is_null() { Ok(()) } else { self.access.destroy_owned_fstring(&mut token) };
                    let result = result.and(current_cleanup).and(buffer_cleanup).and(token_cleanup);
                    game_state_trace("screen_end", || format!("level={level:p} returned={}", result.is_ok()));
                    result?;
                    Ok(0)
                }
                ProfileEnabled => self.output(usize::from(hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0)),
                FunctionValid => {
                    let functions = self.context.function_iterator.get();
                    if functions.is_null() { return Err(RuntimeError::new("LoadMap function iterator scope is absent")); }
                    self.output(usize::from((*functions).index < self.access.object_count()))
                }
                FunctionNext => {
                    let iterator = self.context.function_iterator.get().as_mut()
                        .ok_or_else(|| RuntimeError::new("LoadMap function iterator scope is absent"))?;
                    iterator.advance(self.access)?;
                    Ok(0)
                }
                ClearProfile => {
                    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0 {
                        let functions = self.context.function_iterator.get();
                        if functions.is_null() { return Err(RuntimeError::new("LoadMap function iterator scope is absent")); }
                        let function = self.access.object_at((*functions).index);
                        self.write_reflected(function.cast(), "Calls", &0i64)?;
                        self.write_reflected(function.cast(), "Cycles", &0i64)?;
                    }
                    Ok(0)
                }
                ResetTicks => {
                    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0 {
                        unsafe { hp2_core::timing_globals::GTicks = 1; }
                    }
                    Ok(0)
                }
                PreBefore => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    crate::level_spawn_actor_owner::notify_spawn_instrumentation(
                        self.access, crate::core_owner::SpawnInstrumentationEvent::PreBeginBefore { actor },
                    )?;
                    crate::level_spawn_actor_owner::notify_actor_lifecycle(
                        self.access, actor, crate::core_owner::ActorLifecycleEvent::PreBeginPlay,
                    )?;
                    Ok(0)
                }
                PreAfter => {
                    let actor = crate::level_owner::actor_at(level, a as i32);
                    crate::level_spawn_actor_owner::notify_spawn_instrumentation(
                        self.access, crate::core_owner::SpawnInstrumentationEvent::PreBeginAfter { actor },
                    )?;
                    Ok(0)
                }
                NewBrush => { crate::dynbsp_owner::initialize(self.access, level)?; Ok(0) }
                ResetInput => { crate::camera_owner::reset_input(self.access, a as *mut UViewport)?; Ok(0) }
                ViewportDevice => self.output((*(a as *mut UViewport)).ren_dev as usize),
                FlushDevice => {
                    let viewport = a as *mut UViewport;
                    crate::render_device_owner::flush(self.access, (*viewport).ren_dev.cast(), true)?;
                    Ok(0)
                }
                LastUrl => {
                    crate::url_owner::UrlOwner::assign_copy(self.access, ptr::addr_of_mut!((*engine).last_url), self.context.url)?;
                    Ok(0)
                }
                IsClient => self.output(usize::from(self.access.process_mode().is_client)),
                SaveConfig => {
                    const KEYS: [&[u32]; 7] = [&[78,97,109,101,0], &[84,101,97,109,0], &[67,108,97,115,115,0], &[83,107,105,110,0], &[70,97,99,101,0], &[86,111,105,99,101,0], &[79,118,101,114,114,105,100,101,67,108,97,115,115,0]];
                    let key = KEYS.get(a).ok_or_else(|| RuntimeError::new("LoadMap player configuration key is out of range"))?;
                    let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                    unsafe { crate::url_owner::UrlOwner::save_config(self.access, &defaults, self.context.url, hp2_core::lit!("DefaultPlayer").as_ptr(), key.as_ptr(), hp2_core::lit!("User").as_ptr())?; }
                    Ok(0)
                }
                LogTiming => {
                    let total = hp2_core::time::subtract(hp2_core::unix_clock::seconds(), self.context.load_started.get());
                    let app = hp2_core::time::subtract(hp2_core::unix_clock::process_time(), self.context.load_process_start.get());
                    let map = utf32_to_string(&self.url_map()?)?;
                    self.log_format(&format!("Load time {map}: {total:.6} seconds total, {app:.6} app"))?;
                    Ok(0)
                }
                MatchViewports => {
                    crate::game_travel_owner::run(self.access, engine, a as *mut ULevel, self.context.url, 101, 0)?;
                    Ok(0)
                }
                Fixup => { crate::game_travel_owner::run(self.access, engine, level, self.context.url, 103, 0)?; Ok(0) }
                DetailChange => {
                    let detail = crate::game_init_owner::level_high_detail(self.access, level)?;
                    crate::game_init_owner::level_detail_change(self.access, level, detail)?;
                    Ok(0)
                }
            }
        }
    }

    fn scope(&self, entry: u32, arg: usize) -> RuntimeResult<usize> {
        match entry {
            2 => {
                match game_load_map::run_with(CoreOpsRef(self), entry, arg) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mut caught = error.diagnostic().to_vec();
                        caught.push(0);
                        self.context.catch_error.set(&mut caught);
                        let recovered = game_load_map::run_with(CoreOpsRef(self), 29, 0);
                        self.context.catch_error.set(ptr::null_mut());
                        recovered
                    }
                }
            }
            3 => {
                let mut filename: Vec<u32> = "../Maps/".chars().map(u32::from).collect();
                filename.extend(self.url_map()?);
                filename.push(0);
                self.context.demo_file.set(&mut filename);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.demo_file.set(ptr::null_mut());
                result
            }
            7 => {
                let mut filename = Vec::with_capacity(256);
                self.context.filename.set(&mut filename);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.filename.set(ptr::null_mut());
                result
            }
            16 => {
                let mut actors = ObjectIterator::new(self.access, "Engine.Actor")?;
                self.context.actor_iterator.set(&mut actors);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.actor_iterator.set(ptr::null_mut());
                result
            }
            18 => {
                let mut options = vec![0];
                let mut game_name = vec![0];
                let mut init_error = vec![0];
                self.context.options.set(&mut options);
                self.context.game_class_name.set(&mut game_name);
                self.context.init_error.set(&mut init_error);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.options.set(ptr::null_mut());
                self.context.game_class_name.set(ptr::null_mut());
                self.context.init_error.set(ptr::null_mut());
                result
            }
            22 => {
                let mut error = vec![0];
                self.context.listen_error.set(&mut error);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.listen_error.set(ptr::null_mut());
                result
            }
            24 => {
                let mut actors: Vec<*mut AActor> = Vec::new();
                self.context.reordered_actors.set(&mut actors);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.reordered_actors.set(ptr::null_mut());
                result
            }
            31 => {
                self.context.previous_tick.set(Some(hp2_core::unix_clock::seconds()));
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.previous_tick.set(None);
                result
            }
            32 => {
                self.context.load_started.set(hp2_core::unix_clock::seconds());
                self.context.load_process_start.set(hp2_core::unix_clock::process_time());
                game_load_map::run_with(CoreOpsRef(self), entry, arg)
            }
            33 => {
                let mut functions = ObjectIterator::new(self.access, "Core.Function")?;
                self.context.function_iterator.set(&mut functions);
                let result = game_load_map::run_with(CoreOpsRef(self), entry, arg);
                self.context.function_iterator.set(ptr::null_mut());
                result
            }
            _ if (1..=33).contains(&entry) => game_load_map::run_with(CoreOpsRef(self), entry, arg),
            _ => Err(RuntimeError::new("Invalid LoadMap scope entry").with_context(&entry.to_string())),
        }
    }

    fn invalid_entry(&self) -> RuntimeResult<usize> {
        Err(RuntimeError::new("Invalid LoadMap policy entry"))
    }
}
struct CoreOpsRef<'b, 'a>(&'b CoreOps<'a>);

impl LoadMapOps for CoreOpsRef<'_, '_> {
    type Error = RuntimeError;

    fn call(&self, op: Op, a: usize, b: usize) -> RuntimeResult<usize> {
        let _timer = load_profile::start(0, op as u32);
        self.0.call(op, a, b)
    }

    fn scope(&self, entry: u32, arg: usize) -> RuntimeResult<usize> {
        let _timer = load_profile::start(1, entry);
        self.0.scope(entry, arg)
    }

    fn invalid_entry(&self) -> RuntimeResult<usize> {
        self.0.invalid_entry()
    }
}

/// Executes the shared LoadMap policy against canonical Core-owned engine records.
///
/// # Errors
/// Returns the first checked Core owner or policy error.
///
/// # Safety
/// `engine`, `url`, optional `pending`/`travel_info`, and `error` must be live canonical
/// records for this Core owner. Their storage remains valid for the synchronous operation.
pub unsafe fn load_map(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    url: *const FURL,
    pending: *mut UPendingLevel,
    travel_info: *const TMapStringString,
    error: *mut TArrayRaw,
) -> RuntimeResult<*mut ULevel> {
    if engine.is_null() || url.is_null() || error.is_null() {
        return Err(RuntimeError::new("Invalid canonical LoadMap arguments"));
    }
    // A background save may still be writing the slot files this load can read.
    crate::game_save_owner::finish_pending_save(access)?;
    let ops = CoreOps {
        access,
        context: LoadMapContext {
            engine, url, pending: Cell::new(pending), travel_info, error,
            package_map: Cell::new(ptr::null_mut()), map_parent: Cell::new(ptr::null_mut()),
            info: Cell::new(ptr::null_mut()), saved_hub: Cell::new(0),
            previous_tick: Cell::new(None), load_started: Cell::new(0),
            load_process_start: Cell::new(0),
            options: Cell::new(ptr::null_mut()), game_class_name: Cell::new(ptr::null_mut()),
            init_error: Cell::new(ptr::null_mut()), listen_error: Cell::new(ptr::null_mut()),
            reordered_actors: Cell::new(ptr::null_mut()), actor_iterator: Cell::new(ptr::null_mut()),
            function_iterator: Cell::new(ptr::null_mut()),
            demo_file: Cell::new(ptr::null_mut()), demo_size: Cell::new(0),
            catch_error: Cell::new(ptr::null_mut()), filename: Cell::new(ptr::null_mut()),
        },
    };
    let load_started = std::time::Instant::now();
    let loaded = game_load_map::run_with(ops, 0, 0);
    load_profile::report(access, load_started.elapsed());
    loaded?;
    // The source API returns GLevel after policy success, not the policy's auxiliary value.
    Ok(unsafe { (*engine).g_level.cast() })
}
fn utf32_to_string(text: &[u32]) -> RuntimeResult<String> {
    let mut result = String::with_capacity(text.len());
    for &unit in text.iter().take_while(|&&unit| unit != 0) {
        let ch = char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid UTF-32 LoadMap text"))?;
        result.push(ch);
    }
    Ok(result)
}

unsafe extern "C" fn loadmap_text_allocate(
    _opaque: *mut std::ffi::c_void,
    size: usize,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    if out.is_null() { return 1; }
    let allocation = unsafe { libc::malloc(size.max(1)) };
    if allocation.is_null() { return 1; }
    unsafe { out.write(allocation); }
    0
}

unsafe extern "C" fn loadmap_text_fatal(
    _opaque: *mut std::ffi::c_void,
    _failure: hp2_core::unix_text::Hp2UnixTextFailure,
) -> i32 {
    1
}

fn string_units(raw: &TArrayRaw) -> Vec<u32> {
    if raw.num <= 0 || raw.data.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) }.to_vec()
    }
}
fn read_units(pointer: *const u32) -> Vec<u32> {
    if pointer.is_null() { return Vec::new(); }
    let mut units = Vec::new();
    for index in 0..4096 {
        let unit = unsafe { pointer.add(index).read_unaligned() };
        if unit == 0 { break; }
        units.push(unit);
    }
    units
}

fn uppercase_units(text: &[u32]) -> Vec<u32> {
    text.iter().copied().take_while(|unit| *unit != 0)
        .filter_map(char::from_u32).flat_map(char::to_uppercase).map(u32::from).collect()
}

fn find_units(haystack: &[u32], needle: &[u32]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn contains_units(haystack: &[u32], needle: &[u32]) -> bool {
    find_units(haystack, needle).is_some()
}
/// Applies the Core cache policy that stages persistent actor files for LoadMap.
///
/// # Errors
/// Returns canonical Engine path or directory-enumeration errors.
pub fn load_persistent_actor_cache(access: CoreAccess<'_>) -> RuntimeResult<()> {
    use std::{fs, path::PathBuf};

    let system = access.system()?;
    let raw = unsafe { ptr::addr_of!((*system).save_slot_path).read_unaligned() };
    if raw.num <= 0 {
        return Ok(());
    }
    if raw.data.is_null() {
        return Err(RuntimeError::new("Invalid canonical SaveSlotPath"));
    }
    let text = utf32_to_string(unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) })?;
    let configured = PathBuf::from(text);
    let slot = if configured.is_absolute() {
        configured
    } else {
        access.data_root().join("System").join(configured)
    };
    let cache = slot.join("cache");

    // LoadPersistentActorCache clears the cache directory, then copies the
    // saved slot files into it. The native policy ignores individual delete/copy
    // return values but retains the checked directory discovery operation.
    let cache_files = cache_directory_files(&cache)?;
    for (_, path, size) in cache_files {
        if hp2_core::runtime_init::cache_file_selected(size) {
            let _ = fs::remove_file(path);
        }
    }
    for (name, source, size) in cache_directory_files(&slot)? {
        if hp2_core::runtime_init::cache_file_selected(size) {
            let _ = fs::copy(source, cache.join(name));
        }
    }
    Ok(())
}

pub(crate) fn cache_directory_files(directory: &std::path::Path) -> RuntimeResult<Vec<(std::ffi::OsString, PathBuf, i64)>> {
    use std::fs;

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(RuntimeError::new("Unable to enumerate persistent actor cache").with_context(&error.to_string())),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| RuntimeError::new("Unable to enumerate persistent actor cache").with_context(&error.to_string()))?;
        if !entry.file_type().map_err(|error| RuntimeError::new("Unable to inspect persistent actor cache entry").with_context(&error.to_string()))?.is_file() {
            continue;
        }
        let size = fs::metadata(entry.path()).map(|metadata| i64::try_from(metadata.len()).unwrap_or(i64::MAX)).unwrap_or(-1);
        files.push((entry.file_name(), entry.path(), size));
    }
    Ok(files)
}

#[cfg(all(test, feature = "renderer-wgpu"))]
mod render_host_contracts {
    use super::*;
    use crate::level::ScreenActorsOps;

    #[test]
    fn game_state_exclusion_matches_single_fstring_substrings() {
        let owner = crate::render_owner::render_test_support::owner("game-state-exclusion");
        let access = owner.access();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Scout");
        let exclude = unsafe {
            ptr::addr_of_mut!((*actor.cast::<AActor>()).exclude_game_states).cast::<TArrayRaw>()
        };
        let excluded: Vec<u32> = "gs_Intro,gs_Hub,gs_Finale".chars().map(u32::from).chain([0]).collect();
        unsafe { access.assign_fstring(exclude, &excluded) }.unwrap();
        let ops = CoreScreenActorsOps { access, level: ptr::null_mut(), group_storage: RefCell::new(Vec::new()) };
        let mut state = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        for (current, expected) in [("GS_HUB", 1), ("GS_HUB_EXT", 0), ("GS_HU", 1)] {
            let units: Vec<u32> = current.chars().map(u32::from).chain([0]).collect();
            unsafe { access.assign_fstring(&mut state, &units) }.unwrap();
            assert_eq!(
                unsafe { ops.exclude_caps_contains(exclude.cast(), (&mut state as *mut TArrayRaw).cast()) }.unwrap(),
                expected,
                "native Caps().InStr() exclusion semantics for {current}",
            );
        }
        unsafe { access.destroy_owned_fstring(&mut state) }.unwrap();
        owner.shutdown().unwrap();
    }
}

/// Opt-in (`HP2_FRAME_TIMING=1`) per-operation timing of one `LoadMap`: inclusive wall time and
/// call counts of every policy operation (kind 0, `Op` number) and scope (kind 1, entry number).
mod load_profile {
    use std::{cell::RefCell, collections::HashMap, time::{Duration, Instant}};

    thread_local! {
        static ENABLED: bool = std::env::var_os("HP2_FRAME_TIMING").as_deref() == Some(std::ffi::OsStr::new("1"));
        static TOTALS: RefCell<HashMap<(u32, u32), (u32, Duration)>> = RefCell::new(HashMap::new());
    }

    pub(super) struct Timer(Option<(u32, u32, Instant)>);

    pub(super) fn start(kind: u32, id: u32) -> Timer {
        Timer(ENABLED.with(|on| *on).then(|| (kind, id, Instant::now())))
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            if let Some((kind, id, started)) = self.0.take() {
                TOTALS.with(|totals| {
                    let mut totals = totals.borrow_mut();
                    let entry = totals.entry((kind, id)).or_default();
                    entry.0 += 1;
                    entry.1 += started.elapsed();
                });
            }
        }
    }

    pub(super) fn report(access: hp2_core::core_access::CoreAccess<'_>, total: Duration) {
        if !ENABLED.with(|on| *on) { return; }
        let mut rows: Vec<_> = TOTALS.with(|totals| totals.borrow_mut().drain().collect());
        rows.sort_by(|a, b| b.1.1.cmp(&a.1.1));
        let Ok(category) = access.intern_str("Log", hp2_core::name_owner::FindName::Add) else { return };
        let mut lines = vec![format!("<HP2_PERF> load_map total_ms={:.1}", total.as_secs_f64() * 1000.0)];
        for ((kind, id), (calls, spent)) in rows.into_iter().take(14) {
            lines.push(format!("<HP2_PERF> load_phase kind={kind} id={id} calls={calls} ms={:.1}", spent.as_secs_f64() * 1000.0));
        }
        for line in lines {
            let _ = access.emit_log(category, &line.chars().map(u32::from).collect::<Vec<_>>());
        }
    }
}

/// The fade-out before a level is replaced is drawn frame by frame for `Console.FadeoutTime`
/// seconds of wall time (about half a second in retail), which was most of a saved-game load.
/// It now runs this fraction of that time. `HP2_LOAD_FADE_SCALE=1` restores the original
/// duration; `0.05` is almost a cut.
fn load_fade_scale() -> f32 {
    thread_local! {
        static SCALE: f32 = std::env::var("HP2_LOAD_FADE_SCALE").ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(0.25);
    }
    SCALE.with(|scale| *scale)
}
