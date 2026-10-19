//! Replays the `core` family of the original `ULevel` fixture
//! (`tests/fixtures/level_oracle.txt`, produced by
//! `Build/level_fixture_harness/gen_level.sh` from the unmodified `UnLevel.cpp`).
//!
//! Each scenario runs one `level` entry point against a mock service table and compares,
//! in order, the records the port's hooks produced with the fixture block. Silent service
//! calls (fixture has no record) are compared by name. Excluded record prefixes are output
//! of the C++ shell or body, not of the port; each call site says why.
//!
//! Scenario notes:
//! - `core.is_audible_occlusion_none`: `level::level_is_audible_at` with `OCCLUSION_None`
//!   on a zeroed `ULevel`. No service is reached.
//! - `core.is_server_no_drivers`: `level_net::is_server` on a zeroed `ULevel` with no
//!   drivers. No service is reached.
//! - `core.init_stats`: `level::level_init_stats`. Every counter is seeded non-zero and
//!   must end at zero. The two script-cycle clear services are silent service calls.
//! - `core.get_stats`: `level::level_get_stats`. The `vals` and `num_moves` passed to
//!   `app_sprintf_stats` are compared with the fixture `stats` record. A second case checks
//!   a distinct non-zero `vals[0..9]` order and `num_moves`.
//!
//! Service tables start with every slot set to `never`. Panics cannot unwind out of
//! `extern "C"`, so `never` returns zero and counts the call; `check_scenario` fails the
//! test if an unwired slot was reached. Each scenario overwrites exactly the slots it
//! reaches. Tables are `#[repr(C)]` and hold only fn pointers; `all_never` asserts that layout.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FCheckResult, ULevel};
use hp2_engine::level::{self, Hp2LevelCoreServices, Obj};
use hp2_engine::level_net::{self, Hp2LevelNetServices};

const EXPECTED: &str = include_str!("fixtures/level_oracle.txt");
/// `ESoundOcclusion::OCCLUSION_StaticMeshes` (`Engine/Inc/EngineClasses.h:502`).
const OCCLUSION_STATIC_MESHES: i32 = 3;

// Count of service calls that reached a slot no scenario wired. Panics cannot unwind
// out of `extern "C"`, so `never` only counts; `assert_fully_wired` fails the test.
thread_local! {
    static UNWIRED_CALLS: Cell<u32> = const { Cell::new(0) };
}

/// Placeholder for every service slot. Returns zero (null, false, or 0 for integer
/// and pointer results) so that the caller's control flow stays defined.
unsafe extern "C" fn never() -> usize {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    0
}

/// Fails the test if any unwired slot was reached since the last check.
fn assert_fully_wired(name: &str) {
    let calls = UNWIRED_CALLS.with(|c| c.replace(0));
    assert_eq!(calls, 0, "{name} reached unwired service slots");
}

/// Builds a services table in which every slot is `never`.
///
/// Valid only for tables of fn pointers. The size check rejects any table whose
/// size is not a whole number of pointers; it does not check slot kinds.
fn all_never<T>() -> T {
    let bytes = size_of::<T>();
    assert_eq!(
        bytes % size_of::<usize>(),
        0,
        "services table must hold only pointer-sized slots"
    );
    let slots = vec![never as *const () as usize; bytes / size_of::<usize>()];
    unsafe { std::ptr::read_unaligned(slots.as_ptr().cast::<T>()) }
}

/// Lines of one fixture scenario block: after its `scenario` header, up to the next
/// `scenario` header. The harness prints each `case` header before the next `scenario`,
/// so `case` lines also end the block.
fn fixture_block(name: &str) -> Vec<&'static str> {
    let header = format!("scenario\t{name}");
    let mut lines = EXPECTED.lines();
    assert!(
        lines.any(|l| l == header),
        "scenario {name} missing from fixture"
    );
    lines
        .take_while(|l| !l.starts_with("scenario\t") && !l.starts_with("case\t"))
        .collect()
}

/// The recorded return value (`=>` line) of one fixture scenario.
fn fixture_return(name: &str) -> i32 {
    fixture_block(name)
        .into_iter()
        .find_map(|l| l.strip_prefix("=>\t"))
        .unwrap_or_else(|| panic!("scenario {name} has no return record"))
        .parse()
        .expect("integer return value")
}

/// The recorded `stats` text of one fixture scenario.
fn fixture_stats(name: &str) -> String {
    fixture_block(name)
        .into_iter()
        .find_map(|l| l.strip_prefix("stats\t"))
        .unwrap_or_else(|| panic!("scenario {name} has no stats record"))
        .to_string()
}

/// Locks a shared mock. A poisoned lock from an earlier failed test still holds valid state.
fn lock_shared<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Serializes the tests that share the `static` mocks below.
fn core_lock() -> MutexGuard<'static, ()> {
    lock_shared(&CORE_TEST_LOCK)
}

static CORE_TEST_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    /// Records of the current scenario, in the order the port's hooks reached them.
    static CORE_TRACE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Silent service calls of the current scenario, compared by name.
    static SERVICE_CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// `(vals, num_moves)` of each `app_sprintf_stats` call in the current scenario.
    static STATS_CALLS: RefCell<Vec<(Vec<f32>, i32)>> = const { RefCell::new(Vec::new()) };
    /// `HighDetail` as the harness dump reports it.
    static HIGH_DETAIL: Cell<i32> = const { Cell::new(0) };
    /// Stubbed `GSecondsPerCycle` and `ScriptCycles` for `level_get_stats`.
    static GSECONDS_PER_CYCLE: Cell<f32> = const { Cell::new(0.0) };
    static SCRIPT_CYCLES: Cell<f32> = const { Cell::new(0.0) };
}

fn begin_scenario() {
    CORE_TRACE.with(|t| t.borrow_mut().clear());
    SERVICE_CALLS.with(|s| s.borrow_mut().clear());
    STATS_CALLS.with(|s| s.borrow_mut().clear());
    HIGH_DETAIL.with(|h| h.set(0));
    UNWIRED_CALLS.with(|c| c.set(0));
}

fn push_record(record: impl Into<String>) {
    CORE_TRACE.with(|t| t.borrow_mut().push(record.into()));
}

fn push_service(call: impl Into<String>) {
    SERVICE_CALLS.with(|s| s.borrow_mut().push(call.into()));
}

/// The harness's post-call `high_detail` dump.
fn dump_high_detail() {
    push_record(format!("high_detail\t{}", HIGH_DETAIL.with(Cell::get)));
}

fn take_stats_calls() -> Vec<(Vec<f32>, i32)> {
    STATS_CALLS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

/// The fixture records a scenario must produce, in order, minus `input` and `=>` lines and
/// any prefix in `excluded`.
fn expected_records(name: &str, excluded: &[&str]) -> Vec<String> {
    fixture_block(name)
        .into_iter()
        .filter(|line| !line.starts_with("input\t") && !line.starts_with("=>\t"))
        .filter(|line| !excluded.iter().any(|prefix| line.starts_with(*prefix)))
        .map(str::to_string)
        .collect()
}

/// Compares the records and silent service calls of the scenario that just ran, then checks
/// that no unwired slot was reached.
fn check_scenario(name: &str, excluded: &[&str], expected_services: &[&str]) {
    let records = CORE_TRACE.with(|t| std::mem::take(&mut *t.borrow_mut()));
    assert_eq!(
        records,
        expected_records(name, excluded),
        "records for {name}"
    );
    let services = SERVICE_CALLS.with(|s| std::mem::take(&mut *s.borrow_mut()));
    assert_eq!(services, expected_services, "service calls for {name}");
    assert_fully_wired(name);
}

/// Runs one fixture scenario and checks its records, service calls, and return value.
fn run_scenario(name: &str, excluded: &[&str], expected_services: &[&str], call: impl FnOnce()) {
    begin_scenario();
    call();
    check_scenario(name, excluded, expected_services);
    assert_eq!(fixture_return(name), 0, "return for {name}");
}

unsafe extern "C" fn core_has_game(_info: Obj) -> i32 {
    1
}

unsafe extern "C" fn core_obj_bool(_obj: Obj) -> i32 {
    0
}

unsafe extern "C" fn core_line_clear(
    _model: Obj,
    _end: *const c_void,
    _start: *const c_void,
) -> i32 {
    push_record("model_fast_line_check\t1");
    1
}

unsafe extern "C" fn core_zero_viewports(_obj: Obj) -> i32 {
    0
}

unsafe extern "C" fn core_client_available(_obj: Obj) -> i32 {
    1
}

unsafe extern "C" fn core_one_viewport(_obj: Obj) -> i32 {
    1
}

unsafe extern "C" fn core_get_viewport(_obj: Obj, index: i32) -> Obj {
    assert_eq!(index, 0);
    0x20usize as Obj
}

unsafe extern "C" fn record_viewport_remember(viewport: Obj) {
    push_service(format!("viewport_remember_actor\t{:#x}", viewport as usize));
}

unsafe extern "C" fn core_single_line_clear(
    _obj: Obj,
    _hit: *mut c_void,
    source: Obj,
    _end: *const c_void,
    _start: *const c_void,
    flags: u32,
    _extent: *const c_void,
    _node_flags: u8,
) -> i32 {
    push_record(line_check_record(flags, source));
    1
}
struct ScreenActor {
    /// The harness's `actor_index`, printed by `actor_flags`.
    index: i32,
    player: bool,
    group: Vec<u32>,
    name: String,
    exclude: String,
    hidden: i32,
    in_state: i32,
}

struct ScreenMock {
    actors: Vec<Option<ScreenActor>>,
    current: String,
    master: String,
    editor: i32,
}

static SCREEN: Mutex<Option<ScreenMock>> = Mutex::new(None);

fn with_screen<R>(f: impl FnOnce(&mut ScreenMock) -> R) -> R {
    f(lock_shared(&SCREEN)
        .as_mut()
        .expect("screen mock installed"))
}

fn screen_actor_index(actor: Obj) -> Option<usize> {
    let index = actor as usize;
    (index != 0).then_some(index - 1)
}

fn screen_u32_string(chars: *const u32) -> String {
    let mut result = String::new();
    let mut index = 0;
    unsafe {
        loop {
            let character = *chars.add(index);
            if character == 0 {
                break;
            }
            result.push(char::from_u32(character).expect("fixture TCHAR"));
            index += 1;
        }
    }
    result
}

/// Mock `FString`: the text, plus a NUL-terminated TCHAR copy that `fstring_chars` hands out.
#[derive(Default)]
struct ScreenFStr {
    text: String,
    chars: Vec<u32>,
}

unsafe fn fstring(handle: Obj) -> &'static mut ScreenFStr {
    unsafe { &mut *handle.cast::<ScreenFStr>() }
}

unsafe extern "C" fn screen_fstring_new() -> Obj {
    Box::into_raw(Box::new(ScreenFStr::default())).cast()
}

unsafe extern "C" fn screen_fstring_delete(handle: Obj) {
    unsafe { drop(Box::from_raw(handle.cast::<ScreenFStr>())) };
}

unsafe extern "C" fn screen_fstring_chars(handle: Obj) -> *const u32 {
    let value = unsafe { fstring(handle) };
    value.chars = value.text.chars().map(|c| c as u32).chain([0]).collect();
    value.chars.as_ptr()
}

unsafe extern "C" fn screen_fstring_caps(dst: Obj, src: Obj) {
    let value = unsafe { fstring(src) }.text.to_ascii_uppercase();
    unsafe { fstring(dst) }.text = value;
}

unsafe extern "C" fn screen_fstring_len(handle: Obj) -> i32 {
    unsafe { fstring(handle) }.text.len() as i32
}

unsafe extern "C" fn screen_fstring_assign_chars(dst: Obj, src: *const u32) {
    unsafe { fstring(dst) }.text = screen_u32_string(src);
}

unsafe extern "C" fn screen_fstring_mid(dst: Obj, src: Obj, start: i32, count: i32) {
    let value = unsafe { fstring(src) }.text.clone();
    let start = (start.max(0) as usize).min(value.len());
    let end = start.saturating_add(count.max(0) as usize).min(value.len());
    unsafe { fstring(dst) }.text = value[start..end].to_string();
}

unsafe extern "C" fn screen_fstring_instr_chars(handle: Obj, chars: *const u32) -> i32 {
    let value = unsafe { fstring(handle) }.text.to_ascii_uppercase();
    let needle = screen_u32_string(chars).to_ascii_uppercase();
    value.find(&needle).map_or(-1, |index| index as i32)
}

unsafe extern "C" fn screen_fstring_instr_fstring(a: Obj, b: Obj) -> i32 {
    let value = unsafe { fstring(a) }.text.to_ascii_uppercase();
    let needle = unsafe { fstring(b) }.text.to_ascii_uppercase();
    value.find(&needle).map_or(-1, |index| index as i32)
}

unsafe extern "C" fn screen_fstring_eq_chars(handle: Obj, chars: *const u32) -> i32 {
    (unsafe { fstring(handle) }
        .text
        .eq_ignore_ascii_case(&screen_u32_string(chars))) as i32
}

unsafe extern "C" fn screen_fstring_eq_fstring(a: Obj, b: Obj) -> i32 {
    (unsafe { fstring(a) }
        .text
        .eq_ignore_ascii_case(&unsafe { fstring(b) }.text)) as i32
}

unsafe extern "C" fn screen_actors_num(_obj: Obj) -> i32 {
    with_screen(|screen| screen.actors.len() as i32)
}

unsafe extern "C" fn screen_actor_at(_obj: Obj, index: i32) -> Obj {
    with_screen(|screen| {
        screen
            .actors
            .get(index as usize)
            .and_then(Option::as_ref)
            .map_or(ptr::null_mut(), |_| (index as usize + 1) as Obj)
    })
}

unsafe extern "C" fn screen_actor_is_player(actor: Obj) -> i32 {
    let Some(index) = screen_actor_index(actor) else {
        return 0;
    };
    with_screen(|screen| {
        screen.actors[index]
            .as_ref()
            .map_or(0, |actor| actor.player as i32)
    })
}

unsafe extern "C" fn screen_actor_group(actor: Obj) -> *const u32 {
    let Some(index) = screen_actor_index(actor) else {
        return ptr::null();
    };
    with_screen(|screen| {
        screen.actors[index]
            .as_ref()
            .map_or(ptr::null(), |actor| actor.group.as_ptr())
    })
}

unsafe extern "C" fn screen_actor_exclude(actor: Obj) -> Obj {
    let index = screen_actor_index(actor).expect("non-null actor");
    with_screen(|screen| {
        (&screen.actors[index].as_ref().expect("actor").exclude as *const String)
            .cast_mut()
            .cast()
    })
}

unsafe extern "C" fn screen_actor_set_hidden(actor: Obj, value: i32) {
    let index = screen_actor_index(actor).expect("non-null actor");
    with_screen(|screen| screen.actors[index].as_mut().unwrap().hidden = value);
}

unsafe extern "C" fn screen_actor_set_in_state(actor: Obj, value: i32) {
    let index = screen_actor_index(actor).expect("non-null actor");
    with_screen(|screen| screen.actors[index].as_mut().unwrap().in_state = value);
}

unsafe extern "C" fn screen_actor_resolve(_actor: Obj) {
    push_record("resolve_game_state\t1");
}

unsafe extern "C" fn screen_pawn_current(_pawn: Obj) -> Obj {
    with_screen(|screen| (&screen.current as *const String).cast_mut().cast())
}

unsafe extern "C" fn screen_pawn_master(_pawn: Obj) -> Obj {
    with_screen(|screen| (&screen.master as *const String).cast_mut().cast())
}

unsafe extern "C" fn screen_pawn_token_len(_pawn: Obj) -> i32 {
    10
}

unsafe extern "C" fn screen_object_name(dst: Obj, actor: Obj) {
    let index = screen_actor_index(actor).expect("non-null actor");
    let name = with_screen(|screen| screen.actors[index].as_ref().unwrap().name.clone());
    unsafe { fstring(dst) }.text = name;
}

unsafe extern "C" fn screen_debug_log(kind: i32, message: *const u32) {
    let kind = if kind == 0 { "log" } else { "error" };
    let message = screen_u32_string(message);
    push_record(format!("debugf\t{kind}\t{message}"));
}

unsafe extern "C" fn screen_is_editor() -> i32 {
    with_screen(|screen| screen.editor)
}

/// `index` is the harness's `actor_index` (its `make_actor` slot), which `actor_flags` prints.
fn screen_actor(
    index: i32,
    player: bool,
    group: &str,
    name: &str,
    exclude: &str,
    hidden: i32,
) -> ScreenActor {
    ScreenActor {
        index,
        player,
        group: group
            .chars()
            .map(|character| character as u32)
            .chain([0])
            .collect(),
        name: name.to_string(),
        exclude: exclude.to_string(),
        hidden,
        in_state: 0,
    }
}

fn screen_services() -> Hp2LevelCoreServices {
    let mut services: Hp2LevelCoreServices = all_never();
    services.fstring_new = screen_fstring_new;
    services.fstring_delete = screen_fstring_delete;
    services.fstring_caps = screen_fstring_caps;
    services.fstring_len = screen_fstring_len;
    services.fstring_chars = screen_fstring_chars;
    services.fstring_assign_chars = screen_fstring_assign_chars;
    services.fstring_mid = screen_fstring_mid;
    services.fstring_instr_chars = screen_fstring_instr_chars;
    services.fstring_instr_fstring = screen_fstring_instr_fstring;
    services.fstring_eq_chars = screen_fstring_eq_chars;
    services.fstring_eq_fstring = screen_fstring_eq_fstring;
    services.debug_log = screen_debug_log;
    services.is_editor = screen_is_editor;
    services.object_get_name = screen_object_name;
    services.actors_num = screen_actors_num;
    services.actors_get = screen_actor_at;
    services.actor_is_player = screen_actor_is_player;
    services.actor_group_chars = screen_actor_group;
    services.actor_exclude_game_states = screen_actor_exclude;
    services.actor_set_hidden_ed = screen_actor_set_hidden;
    services.actor_set_in_current_game_state = screen_actor_set_in_state;
    services.actor_event_on_resolve_game_state = screen_actor_resolve;
    services.pawn_current_game_state = screen_pawn_current;
    services.pawn_game_state_master_list = screen_pawn_master;
    services.pawn_game_state_token_len = screen_pawn_token_len;
    services
}

fn run_screen_case(name: &str, mock: ScreenMock) {
    *lock_shared(&SCREEN) = Some(mock);
    let services = screen_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(name, &[], &[], || {
        unsafe {
            level::level_screen_actors_by_game_state(&services, &mut level as *mut ULevel as Obj)
        };
        dump_actor_flags();
    });
    *lock_shared(&SCREEN) = None;
}

/// The harness's post-call `actor_flags` dump: one line per non-null slot, in array order,
/// labelled with each actor's `actor_index`.
fn dump_actor_flags() {
    let lines: Vec<String> = lock_shared(&SCREEN)
        .as_ref()
        .expect("screen mock installed")
        .actors
        .iter()
        .filter_map(|actor| {
            actor.as_ref().map(|actor| {
                format!(
                    "actor_flags\t{}\t{}\t{}",
                    actor.index, actor.in_state, actor.hidden
                )
            })
        })
        .collect();
    for line in lines {
        push_record(line);
    }
}

fn screen_mock(actors: Vec<Option<ScreenActor>>, current: &str, editor: i32) -> ScreenMock {
    ScreenMock {
        actors,
        current: current.to_string(),
        master: "GSTATEMAIN,GSTATELOBY".to_string(),
        editor,
    }
}

#[test]
fn core_screen_actors_by_game_state_scenarios_match_fixture() {
    let _lock = core_lock();
    run_screen_case(
        "core.screen_actors_by_game_state_no_player",
        screen_mock(
            vec![Some(screen_actor(0, false, "GSTATEMAIN", "Actor0", "", 0))],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_none_state",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATEMAIN", "Actor1", "", 0)),
            ],
            "NONE",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_invalid_state",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATEMAIN", "Actor1", "", 0)),
            ],
            "GSTATEFOO",
            0,
        ),
    );
    // The harness builds `[NULL, group actor (index 1), pawn (index 0)]`
    // (`level_harness.cpp:770-773`), so the identity is not the array position here.
    run_screen_case(
        "core.screen_actors_by_game_state_null_slot",
        screen_mock(
            vec![
                None,
                Some(screen_actor(1, false, "GSTATEMAIN", "Actor1", "", 0)),
                Some(screen_actor(0, true, "PAWN", "Player", "", 0)),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_exclude_hit",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(
                    1,
                    false,
                    "GSTATEMAIN",
                    "Actor1",
                    "gstatemain",
                    0,
                )),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_exclude_miss",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(
                    1,
                    false,
                    "GSTATEMAIN",
                    "Actor1",
                    "GSTATELOBY",
                    0,
                )),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_no_gstate",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "PAWNLIGHT", "Actor1", "", 0)),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_editor_hidden",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATELOBY", "Actor1", "", 1)),
            ],
            "GSTATEMAIN",
            1,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_match",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATEMAIN", "Actor1", "", 0)),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_multi_token",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(
                    1,
                    false,
                    "GSTATELOBY,GSTATEMAIN",
                    "Actor1",
                    "",
                    0,
                )),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_bad_token_length",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATEX", "Actor1", "", 0)),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
    run_screen_case(
        "core.screen_actors_by_game_state_bad_token_unknown",
        screen_mock(
            vec![
                Some(screen_actor(0, true, "", "Player", "", 0)),
                Some(screen_actor(1, false, "GSTATEXXXX", "Actor1", "", 0)),
            ],
            "GSTATEMAIN",
            0,
        ),
    );
}

unsafe extern "C" fn core_line_blocked(
    _model: Obj,
    _end: *const c_void,
    _start: *const c_void,
) -> i32 {
    push_record("model_fast_line_check\t0");
    0
}

unsafe extern "C" fn core_single_line_blocked(
    _obj: Obj,
    _hit: *mut c_void,
    source: Obj,
    _end: *const c_void,
    _start: *const c_void,
    flags: u32,
    _extent: *const c_void,
    _node_flags: u8,
) -> i32 {
    push_record(line_check_record(flags, source));
    0
}

unsafe extern "C" fn collision_scope_enter(name: *const std::ffi::c_char) -> Obj {
    let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
    push_service(format!("app_rand_trace_phase_scope_enter\t{name}"));
    0x30usize as Obj
}

unsafe extern "C" fn collision_scope_leave(scope: Obj) {
    push_service(format!(
        "app_rand_trace_phase_scope_leave\t{:#x}",
        scope as usize
    ));
}

unsafe extern "C" fn collision_hash_new() -> Obj {
    push_record("collision_hash_new");
    0x10usize as Obj
}

unsafe extern "C" fn collision_actors_num(_obj: Obj) -> i32 {
    3
}

unsafe extern "C" fn collision_actor_at(_obj: Obj, index: i32) -> Obj {
    match index {
        0 => 1usize as Obj,
        2 => 3usize as Obj,
        _ => ptr::null_mut(),
    }
}

unsafe extern "C" fn collision_actor_collides(_actor: Obj) -> i32 {
    1
}

/// Fixture actor numbering for the collision mock: actor 1 is index 0, actor 3 is index 2.
fn actor_index(actor: Obj) -> i64 {
    match actor as usize {
        1 => 0,
        3 => 2,
        _ => -1,
    }
}

unsafe extern "C" fn collision_hash_add(_hash: Obj, actor: Obj) {
    push_record(format!("collision_hash_add_actor\t{}", actor_index(actor)));
}

/// The shell hardcodes `RemoveActor(Actor, true)` (Hp2LevelServices.cpp:108), so the `1` is fixed.
unsafe extern "C" fn collision_hash_remove(_hash: Obj, actor: Obj) {
    push_record(format!(
        "collision_hash_remove_actor\t{}\t1",
        actor_index(actor)
    ));
}

unsafe extern "C" fn collision_hash_delete(_hash: Obj) {
    push_record("collision_hash_destroy");
}

unsafe extern "C" fn record_uobject_destroy(_obj: Obj) {
    push_record("uobject_destroy");
}

#[test]
fn core_collision_scenarios_match_fixture() {
    let _lock = core_lock();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let obj = &mut level as *mut ULevel as Obj;
    let mut services: Hp2LevelCoreServices = all_never();
    services.app_rand_trace_phase_scope_enter = collision_scope_enter;
    services.app_rand_trace_phase_scope_leave = collision_scope_leave;
    services.collision_hash_new = collision_hash_new;
    services.actors_num = collision_actors_num;
    services.actors_get = collision_actor_at;
    services.actor_collide_actors = collision_actor_collides;
    services.collision_hash_add_actor = collision_hash_add;
    run_scenario(
        "core.set_collision_enable",
        &[],
        &[
            "app_rand_trace_phase_scope_enter\tcollision_hash",
            "app_rand_trace_phase_scope_leave\t0x30",
        ],
        || unsafe { level::level_set_actor_collision(&services, obj, 1) },
    );

    services = all_never();
    services.collision_hash_new = collision_hash_new;
    services.actors_num = collision_actors_num;
    services.actors_get = collision_actor_at;
    services.actor_collide_actors = collision_actor_collides;
    services.collision_hash_remove_actor = collision_hash_remove;
    services.collision_hash_delete = collision_hash_delete;
    run_scenario("core.set_collision_disable", &[], &[], || {
        let hash = unsafe { (services.collision_hash_new)() };
        unsafe { (*(obj as *mut ULevel)).hash = hash };
        unsafe { level::level_set_actor_collision(&services, obj, 0) };
    });
}

#[test]
fn core_destroy_hash_present_matches_fixture() {
    let _lock = core_lock();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let obj = &mut level as *mut ULevel as Obj;
    let mut services: Hp2LevelCoreServices = all_never();
    services.collision_hash_new = collision_hash_new;
    services.collision_hash_delete = collision_hash_delete;
    services.uobject_destroy = record_uobject_destroy;
    run_scenario("core.destroy_hash_present", &[], &[], || {
        let hash = unsafe { (services.collision_hash_new)() };
        unsafe { (*(obj as *mut ULevel)).hash = hash };
        unsafe { level::level_destroy(&services, obj) };
    });
}

struct TraceMock {
    hit_actor: Obj,
    item: i32,
    zone: u8,
    hit_at_start: bool,
    source_is_pawn: bool,
    owned: bool,
    level_info: bool,
    visible: bool,
    class_matches: bool,
    single_line_clear: bool,
}

// SAFETY: the raw actor pointer is an opaque test token, never dereferenced, and
// every access goes through the TRACE mutex.
unsafe impl Send for TraceMock {}

static TRACE: Mutex<Option<TraceMock>> = Mutex::new(None);

fn with_trace<R>(f: impl FnOnce(&mut TraceMock) -> R) -> R {
    f(lock_shared(&TRACE).as_mut().expect("trace mock installed"))
}

unsafe extern "C" fn trace_source_is_pawn(_source: Obj) -> i32 {
    with_trace(|trace| trace.source_is_pawn as i32)
}

/// The harness prints `actor_index(SourceActor)`; the trace source is actor 0 in these scenarios.
const TRACE_SOURCE: usize = 0x30;

/// The harness prints `level_single_line_check\t%08x\t%d` with the source index (0 or -1).
fn line_check_record(flags: u32, source: Obj) -> String {
    let source_index = if source as usize == TRACE_SOURCE {
        0
    } else {
        -1
    };
    format!("level_single_line_check\t{flags:08x}\t{source_index}")
}

unsafe extern "C" fn trace_line_check(
    _obj: Obj,
    first_hit: *mut c_void,
    source: Obj,
    end: *const c_void,
    start: *const c_void,
    flags: u32,
    _extent: *const c_void,
    _node_flags: u8,
) -> i32 {
    push_record(line_check_record(flags, source));
    let (clear, actor, item, location) = with_trace(|trace| {
        // The harness reports the scripted hit at `Start` or `End` of the line check.
        let location = unsafe {
            if trace.hit_at_start {
                *start.cast::<FVector>()
            } else {
                *end.cast::<FVector>()
            }
        };
        (
            trace.single_line_clear,
            trace.hit_actor,
            trace.item,
            location,
        )
    });
    if clear {
        return 1;
    }
    unsafe {
        *first_hit.cast::<FCheckResult>() = FCheckResult {
            actor,
            item,
            time: 0.5,
            location,
            ..std::mem::zeroed()
        };
    }
    0
}

unsafe extern "C" fn trace_actor_owned(_source: Obj, _actor: Obj) -> i32 {
    with_trace(|trace| trace.owned as i32)
}

unsafe extern "C" fn trace_is_level_info(actor: Obj) -> i32 {
    with_trace(|trace| trace.level_info && actor as usize == 100) as i32
}

unsafe extern "C" fn trace_model_zone(_actor: Obj, _item: i32) -> u8 {
    with_trace(|trace| trace.zone)
}

unsafe extern "C" fn trace_zone_visible(_pawn: Obj, zone: u8) -> i32 {
    let visible = with_trace(|trace| trace.visible);
    push_record(format!("zone_visible\t{zone}\t{}", visible as u8));
    visible as i32
}

unsafe extern "C" fn trace_actor_class(_actor: Obj, _parent: Obj) -> i32 {
    with_trace(|trace| trace.class_matches as i32)
}

unsafe extern "C" fn trace_result_assign(dst: *mut c_void, src: *const c_void) {
    unsafe { *dst.cast::<FCheckResult>() = *src.cast::<FCheckResult>() };
}

struct TraceCase {
    name: &'static str,
    objects: bool,
    distance: i32,
    direction_x: f32,
    source_is_pawn: bool,
    level_info: bool,
    item: i32,
    zone: u8,
    visible: bool,
    owned: bool,
    class_matches: bool,
    single_line_clear: bool,
    hit_at_start: bool,
}

fn run_trace_case(case: TraceCase) {
    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.region.zone_number = case.zone;
    let hit_actor = if case.level_info {
        100usize as Obj
    } else {
        (&mut actor as *mut AActor).cast()
    };
    begin_scenario();
    *lock_shared(&TRACE) = Some(TraceMock {
        hit_actor,
        item: case.item,
        zone: case.zone,
        hit_at_start: case.hit_at_start,
        source_is_pawn: case.source_is_pawn,
        owned: case.owned,
        level_info: case.level_info,
        visible: case.visible,
        class_matches: case.class_matches,
        single_line_clear: case.single_line_clear,
    });
    let mut services: Hp2LevelCoreServices = all_never();
    services.level_single_line_check = trace_line_check;
    services.actor_is_a_aplayer_pawn = trace_source_is_pawn;
    services.actor_is_owned_by = trace_actor_owned;
    services.actor_is_a_alevel_info = trace_is_level_info;
    services.model_node_izone1_of_actor = trace_model_zone;
    services.pawn_is_zone_visible = trace_zone_visible;
    services.actor_class_is_child_of = trace_actor_class;
    services.check_result_assign = trace_result_assign;

    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.item = -1;
    let mut first_hit: FCheckResult = unsafe { std::mem::zeroed() };
    let direction = FVector {
        x: case.direction_x,
        y: 0.0,
        z: 0.0,
    };
    let start = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let source = TRACE_SOURCE as Obj;
    unsafe {
        if case.objects {
            level::level_trace_visible_objects(
                &services,
                &mut level as *mut ULevel as Obj,
                0x50usize as Obj,
                &direction,
                &mut hit,
                source,
                &start,
                6,
                case.distance,
                &mut first_hit,
            );
        } else {
            level::level_trace_visible(
                &services,
                &mut level as *mut ULevel as Obj,
                &direction,
                &mut hit,
                source,
                &start,
                6,
                case.distance,
                &mut first_hit,
            );
        }
    }
    let hit_actor = if hit.actor.is_null() {
        -1
    } else if hit.actor as usize == 100 {
        100
    } else {
        1
    };
    let hit_time = format!("{}", hit.time);
    let hit_item = hit.item;
    push_record(format!("hit_time\t{hit_time}"));
    push_record(format!("hit_actor\t{hit_actor}"));
    push_record(format!("hit_item\t{hit_item}"));
    *lock_shared(&TRACE) = None;
    check_scenario(case.name, &[], &[]);
    assert_eq!(fixture_return(case.name), 0);
}

#[test]
fn core_trace_visible_scenarios_match_fixture() {
    let _lock = core_lock();
    for case in [
        TraceCase {
            name: "core.trace_visible_no_distance",
            objects: false,
            distance: 0,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 0,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_clear",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 0,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: true,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_levelinfo_visible",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: true,
            item: 5,
            zone: 5,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_levelinfo_rock",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: true,
            item: 0,
            zone: 5,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_levelinfo_hidden",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: true,
            item: 5,
            zone: 5,
            visible: false,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_actor_visible",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_actor_hidden_retry",
            objects: false,
            distance: 48,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: false,
            item: 0,
            zone: 2,
            visible: false,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: true,
        },
        TraceCase {
            name: "core.trace_visible_owned_skip",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: true,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_nonplayer_levelinfo",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: true,
            item: 5,
            zone: 5,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_nonplayer_actor",
            objects: false,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_no_distance",
            objects: true,
            distance: 0,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 0,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_clear",
            objects: true,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 0,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: true,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_nonplayer_match",
            objects: true,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_player_visible",
            objects: true,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_player_hidden_retry",
            objects: true,
            distance: 48,
            direction_x: 1.0,
            source_is_pawn: true,
            level_info: false,
            item: 0,
            zone: 2,
            visible: false,
            owned: false,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: true,
        },
        TraceCase {
            name: "core.trace_visible_objects_class_mismatch",
            objects: true,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: false,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_owned_skip",
            objects: true,
            distance: 100,
            direction_x: 1.0,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: true,
            class_matches: true,
            single_line_clear: false,
            hit_at_start: false,
        },
        TraceCase {
            name: "core.trace_visible_objects_loop_band",
            objects: true,
            distance: 47,
            direction_x: 0.97,
            source_is_pawn: false,
            level_info: false,
            item: 0,
            zone: 2,
            visible: true,
            owned: false,
            class_matches: false,
            single_line_clear: false,
            hit_at_start: true,
        },
    ] {
        run_trace_case(case);
    }
}
fn fixture_return_f32(name: &str) -> f32 {
    fixture_block(name)
        .into_iter()
        .find_map(|line| line.strip_prefix("=>\t"))
        .unwrap_or_else(|| panic!("scenario {name} has no return record"))
        .parse()
        .expect("float return value")
}

fn audible_case(name: &str, services: &Hp2LevelCoreServices, obj: Obj, occlusion: i32) {
    let src: FVector = unsafe { std::mem::zeroed() };
    let dst: FVector = unsafe { std::mem::zeroed() };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    begin_scenario();
    let audible = unsafe {
        level::level_is_audible_at(
            services,
            obj,
            &src,
            &dst,
            ptr::null_mut(),
            occlusion,
            &mut hit,
        )
    };
    check_scenario(name, &[], &[]);
    assert_eq!(audible, fixture_return(name));
}

#[test]
fn core_radius_and_audible_scenarios_match_fixture() {
    let _lock = core_lock();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    level.zone_dist[66] = 2;
    let obj = &mut level as *mut ULevel as Obj;

    begin_scenario();
    let same = unsafe { level::level_calculate_radius_multiplier(obj, 1, 1) };
    check_scenario("core.radius_multiplier_same_zone", &[], &[]);
    assert_eq!(same, fixture_return_f32("core.radius_multiplier_same_zone"));

    begin_scenario();
    let two = unsafe { level::level_calculate_radius_multiplier(obj, 1, 2) };
    check_scenario("core.radius_multiplier_two_zones", &[], &[]);
    assert_eq!(two, fixture_return_f32("core.radius_multiplier_two_zones"));

    let mut services: Hp2LevelCoreServices = all_never();
    services.model_fast_line_check = core_line_clear;
    audible_case("core.is_audible_occlusion_default_clear", &services, obj, 0);

    services = all_never();
    services.model_fast_line_check = core_line_blocked;
    audible_case(
        "core.is_audible_occlusion_default_blocked",
        &services,
        obj,
        0,
    );

    services = all_never();
    services.level_single_line_check = core_single_line_clear;
    audible_case(
        "core.is_audible_occlusion_static_clear",
        &services,
        obj,
        OCCLUSION_STATIC_MESHES,
    );

    services = all_never();
    services.level_single_line_check = core_single_line_blocked;
    audible_case(
        "core.is_audible_occlusion_static_blocked",
        &services,
        obj,
        OCCLUSION_STATIC_MESHES,
    );
}

unsafe extern "C" fn reconcile_actor_count(_obj: Obj) -> i32 {
    4
}

unsafe extern "C" fn reconcile_actor_at(_obj: Obj, index: i32) -> Obj {
    if index == 0 {
        ptr::null_mut()
    } else {
        (index as usize + 1) as Obj
    }
}

unsafe extern "C" fn reconcile_is_player_pawn(actor: Obj) -> i32 {
    matches!(actor as usize, 3 | 4) as i32
}

unsafe extern "C" fn reconcile_pawn_player(_pawn: Obj) -> Obj {
    0x10usize as Obj
}

unsafe extern "C" fn reconcile_set_pawn_player(pawn: Obj, player: Obj) {
    let index = pawn as usize - 1;
    push_record(format!("pawn_player\t{index}\t{}", player as usize));
}

unsafe extern "C" fn reconcile_no_viewports(_obj: Obj) -> i32 {
    0
}

unsafe extern "C" fn reconcile_not_camera(_actor: Obj) -> Obj {
    ptr::null_mut()
}

unsafe extern "C" fn reconcile_is_editor() -> i32 {
    1
}

#[test]
fn core_reconcile_player_cleared_matches_fixture() {
    let _lock = core_lock();
    let mut services: Hp2LevelCoreServices = all_never();
    services.is_editor = reconcile_is_editor;
    services.actors_num = reconcile_actor_count;
    services.actors_get = reconcile_actor_at;
    services.actor_is_a_aplayer_pawn = reconcile_is_player_pawn;
    services.pawn_get_player = reconcile_pawn_player;
    services.pawn_set_player = reconcile_set_pawn_player;
    services.level_client_viewports_num = reconcile_no_viewports;
    services.actor_cast_camera = reconcile_not_camera;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario("core.reconcile_actors_player_cleared", &[], &[], || {
        unsafe { level::level_reconcile_actors(&services, &mut level as *mut ULevel as Obj) }
            .unwrap();
    });
}

struct ReconcileActor {
    id: usize,
    tag: &'static str,
    camera: bool,
}

struct ReconcileMock {
    actors: Vec<ReconcileActor>,
    viewport_actor: usize,
}

static RECONCILE: Mutex<Option<ReconcileMock>> = Mutex::new(None);
static RECONCILE_CHARS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn with_reconcile<R>(f: impl FnOnce(&mut ReconcileMock) -> R) -> R {
    f(lock_shared(&RECONCILE)
        .as_mut()
        .expect("reconcile mock installed"))
}

unsafe extern "C" fn reconcile_record_actor_count(_obj: Obj) -> i32 {
    with_reconcile(|mock| mock.actors.len() as i32)
}

unsafe extern "C" fn reconcile_record_actor_at(_obj: Obj, index: i32) -> Obj {
    with_reconcile(|mock| mock.actors[index as usize].id as Obj)
}

unsafe extern "C" fn reconcile_not_pawn(_actor: Obj) -> i32 {
    0
}

unsafe extern "C" fn reconcile_is_camera(actor: Obj) -> i32 {
    with_reconcile(|mock| {
        mock.actors
            .iter()
            .find(|entry| entry.id == actor as usize)
            .is_some_and(|entry| entry.camera) as i32
    })
}

unsafe extern "C" fn reconcile_compare_tag(actor: Obj, _viewport: Obj) -> i32 {
    with_reconcile(|mock| {
        let entry = mock
            .actors
            .iter()
            .find(|entry| entry.id == actor as usize)
            .expect("camera actor");
        if entry.tag.eq_ignore_ascii_case("Cam0") {
            0
        } else {
            1
        }
    })
}

unsafe extern "C" fn reconcile_viewport_count(_obj: Obj) -> i32 {
    1
}

unsafe extern "C" fn reconcile_viewport_at(_obj: Obj, _index: i32) -> Obj {
    0x30usize as Obj
}

unsafe extern "C" fn reconcile_viewport_actor(_viewport: Obj) -> Obj {
    with_reconcile(|mock| mock.viewport_actor as Obj)
}

unsafe extern "C" fn reconcile_match_actor(_viewport: Obj, actor: Obj) {
    with_reconcile(|mock| mock.viewport_actor = actor as usize);
    push_service(format!("viewport_match_actor\t{}", actor as usize));
}

unsafe extern "C" fn reconcile_get_name(dst: Obj, _object: Obj) {
    unsafe { fstring(dst) }.text = "Cam0".to_string();
}

unsafe extern "C" fn reconcile_fstring_chars(handle: Obj) -> *const u32 {
    let chars = unsafe { fstring(handle) }
        .text
        .chars()
        .map(|character| character as u32)
        .chain([0])
        .collect::<Vec<_>>();
    let mut buffer = lock_shared(&RECONCILE_CHARS);
    *buffer = chars;
    buffer.as_ptr()
}

unsafe extern "C" fn reconcile_debug_log(kind: i32, message: *const u32) {
    let kind = if kind == 0 { "log" } else { "error" };
    let mut text = String::new();
    let mut index = 0;
    unsafe {
        loop {
            let character = *message.add(index);
            if character == 0 {
                break;
            }
            text.push(char::from_u32(character).unwrap());
            index += 1;
        }
    }
    push_record(format!("debugf\t{kind}\t{text}"));
}

unsafe extern "C" fn reconcile_spawn_view_actor(_obj: Obj, _viewport: Obj) {
    with_reconcile(|mock| {
        mock.actors.push(ReconcileActor {
            id: 7,
            tag: "Other",
            camera: true,
        });
        mock.viewport_actor = 7;
    });
    push_record("level_spawn_view_actor\t7");
}

unsafe extern "C" fn reconcile_cast_camera(actor: Obj) -> Obj {
    if unsafe { reconcile_is_camera(actor) } != 0 {
        actor
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn reconcile_camera_viewport(view: Obj) -> Obj {
    with_reconcile(|mock| {
        if mock.viewport_actor == view as usize {
            0x30usize as Obj
        } else {
            ptr::null_mut()
        }
    })
}

unsafe extern "C" fn reconcile_destroy_actor(_obj: Obj, actor: Obj) {
    push_record(format!("level_destroy_actor\t{}", actor as usize));
}

unsafe extern "C" fn reconcile_clear_flags(view: Obj, flags: u32) {
    push_service(format!("uobject_clear_flags\t{}\t{flags}", view as usize));
}

unsafe extern "C" fn reconcile_restore_view(view: Obj, viewport: Obj) {
    push_service(format!(
        "camera_restore_view\t{}\t{:#x}",
        view as usize, viewport as usize
    ));
}

fn run_reconcile_view_case(name: &str, actors: Vec<ReconcileActor>, expected_services: &[&str]) {
    *lock_shared(&RECONCILE) = Some(ReconcileMock {
        actors,
        viewport_actor: 0,
    });
    let mut services: Hp2LevelCoreServices = all_never();
    services.is_editor = reconcile_is_editor;
    services.actors_num = reconcile_record_actor_count;
    services.actors_get = reconcile_record_actor_at;
    services.actor_is_a_aplayer_pawn = reconcile_not_pawn;
    services.actor_is_a_camera = reconcile_is_camera;
    services.actor_tag_stricmp_viewport_name = reconcile_compare_tag;
    services.level_client_viewports_num = reconcile_viewport_count;
    services.level_client_viewport = reconcile_viewport_at;
    services.viewport_get_actor = reconcile_viewport_actor;
    services.viewport_match_actor = reconcile_match_actor;
    services.fstring_new = screen_fstring_new;
    services.fstring_delete = screen_fstring_delete;
    services.fstring_chars = reconcile_fstring_chars;
    services.object_get_name = reconcile_get_name;
    services.debug_log = reconcile_debug_log;
    services.level_spawn_view_actor = reconcile_spawn_view_actor;
    services.actor_cast_camera = reconcile_cast_camera;
    services.camera_player_viewport = reconcile_camera_viewport;
    services.level_destroy_actor = reconcile_destroy_actor;
    services.uobject_clear_flags = reconcile_clear_flags;
    services.camera_restore_view = reconcile_restore_view;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(
        name,
        &["viewport_saved\t", "viewport_actor\t", "view_fields\t"],
        expected_services,
        || {
            unsafe { level::level_reconcile_actors(&services, &mut level as *mut ULevel as Obj) }
                .unwrap();
        },
    );
    *lock_shared(&RECONCILE) = None;
}

#[test]
fn core_reconcile_viewport_scenarios_match_fixture() {
    let _lock = core_lock();
    run_reconcile_view_case(
        "core.reconcile_actors_match_identical",
        vec![
            ReconcileActor {
                id: 1,
                tag: "Other",
                camera: true,
            },
            ReconcileActor {
                id: 2,
                tag: "Cam0",
                camera: true,
            },
        ],
        &[
            "viewport_match_actor\t2",
            "uobject_clear_flags\t2\t1",
            "camera_restore_view\t2\t0x30",
        ],
    );
    run_reconcile_view_case(
        "core.reconcile_actors_spawn_unmatched",
        vec![ReconcileActor {
            id: 1,
            tag: "Other",
            camera: true,
        }],
        &["uobject_clear_flags\t7\t1", "camera_restore_view\t7\t0x30"],
    );
}
/// `error` records of the `check(...)` failures the port reported, in order.
static CHECK_FAILURES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The C++ check path reports the failure and returns status 1; the port returns `Err`.
unsafe extern "C" fn reconcile_check_failed(expr: *const std::ffi::c_char, line: i32) -> i32 {
    let expr = unsafe { std::ffi::CStr::from_ptr(expr) }.to_string_lossy();
    lock_shared(&CHECK_FAILURES).push(format!(
        "error\tAssertion failed: {expr} [File:UnLevel.cpp] [Line: {line}]"
    ));
    1
}

unsafe extern "C" fn reconcile_not_editor() -> i32 {
    0
}

unsafe extern "C" fn reconcile_one_viewport(_obj: Obj) -> i32 {
    1
}

unsafe extern "C" fn reconcile_one_viewport_at(_obj: Obj, _index: i32) -> Obj {
    0x20usize as Obj
}

unsafe extern "C" fn reconcile_viewport_has_actor(_viewport: Obj) -> Obj {
    0x30usize as Obj
}

/// The `error` records the C++ check path prints for each failing `check(...)`.
/// `debugf` and `harness_abort` lines come from the C++ guard and harness, outside the port.
fn check_failure_records(name: &str) -> Vec<String> {
    fixture_block(name)
        .into_iter()
        .filter(|line| line.starts_with("error\t"))
        .map(str::to_string)
        .collect()
}

#[test]
fn core_reconcile_check_failures_match_fixture() {
    let _lock = core_lock();
    for (case, scenario) in [
        ("editor", "core.reconcile_actors_check_editor"),
        (
            "viewport_actor",
            "core.reconcile_actors_check_viewport_actor",
        ),
    ] {
        let mut services: Hp2LevelCoreServices = all_never();
        services.check_failed = reconcile_check_failed;
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        let obj = &mut level as *mut ULevel as Obj;
        if case == "editor" {
            services.is_editor = reconcile_not_editor;
        } else {
            services.is_editor = reconcile_is_editor;
            services.actors_num = reconcile_record_actor_count;
            services.level_client_viewports_num = reconcile_one_viewport;
            services.level_client_viewport = reconcile_one_viewport_at;
            services.viewport_get_actor = reconcile_viewport_has_actor;
            *lock_shared(&RECONCILE) = Some(ReconcileMock {
                actors: Vec::new(),
                viewport_actor: 0,
            });
        }
        lock_shared(&CHECK_FAILURES).clear();
        let status = unsafe { level::level_reconcile_actors(&services, obj) };
        assert!(
            status.is_err(),
            "scenario {scenario} must return the check failure"
        );
        let failures = std::mem::take(&mut *lock_shared(&CHECK_FAILURES));
        assert_eq!(
            failures,
            check_failure_records(scenario),
            "records for {scenario}"
        );
        assert_fully_wired(scenario);
        *lock_shared(&RECONCILE) = None;
    }
}
struct ArchiveMock {
    loading: bool,
    version: i32,
}

static ARCHIVE: Mutex<Option<ArchiveMock>> = Mutex::new(None);

fn with_archive<R>(f: impl FnOnce(&mut ArchiveMock) -> R) -> R {
    f(lock_shared(&ARCHIVE)
        .as_mut()
        .expect("archive mock installed"))
}

fn archive_direction() -> &'static str {
    if with_archive(|mock| mock.loading) {
        "R"
    } else {
        "W"
    }
}

unsafe extern "C" fn archive_uobject_serialize(_obj: Obj, _archive: Obj) {
    push_record("uobject_serialize");
}

unsafe extern "C" fn archive_is_loading(_archive: Obj) -> i32 {
    with_archive(|mock| mock.loading as i32)
}

unsafe extern "C" fn archive_is_saving(_archive: Obj) -> i32 {
    with_archive(|mock| (!mock.loading) as i32)
}

unsafe extern "C" fn archive_is_trans(_archive: Obj) -> i32 {
    0
}

unsafe extern "C" fn archive_version(_archive: Obj) -> i32 {
    with_archive(|mock| mock.version)
}

unsafe extern "C" fn archive_actors_count(_obj: Obj) -> i32 {
    2
}

unsafe extern "C" fn archive_count_bytes(_obj: Obj, _archive: Obj) {}

unsafe extern "C" fn archive_serialize_i32(_archive: Obj, _value: *mut i32) {
    push_record(format!(
        "archive_serialize_bytes\t4\t{}",
        archive_direction()
    ));
}

unsafe extern "C" fn archive_actors_empty(_obj: Obj, _count: i32) {}

unsafe extern "C" fn archive_actors_add(_obj: Obj, _count: i32) {}

unsafe extern "C" fn archive_slots_env_path() -> *const std::ffi::c_char {
    ptr::null()
}

unsafe extern "C" fn archive_serialize_actor_slot(_archive: Obj, _obj: Obj, _index: i32) {}

unsafe extern "C" fn archive_serialize_url(_archive: Obj, _obj: Obj) {
    push_record(format!(
        "url_serialize\t{}",
        with_archive(|mock| mock.loading as u8)
    ));
}

unsafe extern "C" fn archive_serialize_model(_archive: Obj, _slot: *mut c_void) {
    push_record(format!(
        "archive_serialize_bytes\t1\t{}",
        archive_direction()
    ));
}

unsafe extern "C" fn archive_serialize_reach_specs(_archive: Obj, _obj: Obj) {}

unsafe extern "C" fn archive_serialize_f32(_archive: Obj, _value: *mut f32) {
    push_record(format!(
        "archive_serialize_bytes\t4\t{}",
        archive_direction()
    ));
}

unsafe extern "C" fn archive_serialize_actor_ptr(_archive: Obj, _slot: *mut c_void) {}

unsafe extern "C" fn archive_serialize_text(_archive: Obj, _slot: *mut c_void) {}

unsafe extern "C" fn archive_serialize_travel_info(_archive: Obj, _obj: Obj) {
    push_record(format!(
        "archive_serialize_bytes\t1\t{}",
        archive_direction()
    ));
}

unsafe extern "C" fn archive_serialize_travel_info_legacy(_archive: Obj, _obj: Obj) {}

unsafe extern "C" fn archive_level_time(_obj: Obj) -> f32 {
    0.0
}

unsafe extern "C" fn archive_close(size: i32) {
    push_record(format!("archive_close\t{size}"));
}

fn archive_services() -> Hp2LevelCoreServices {
    let mut services: Hp2LevelCoreServices = all_never();
    services.uobject_serialize = archive_uobject_serialize;
    services.archive_is_loading = archive_is_loading;
    services.archive_is_saving = archive_is_saving;
    services.archive_is_trans = archive_is_trans;
    services.archive_ver = archive_version;
    services.actors_num = archive_actors_count;
    services.actors_count_bytes = archive_count_bytes;
    services.archive_serialize_i32 = archive_serialize_i32;
    services.actors_empty = archive_actors_empty;
    services.actors_add = archive_actors_add;
    services.actor_slot_dump_env_path = archive_slots_env_path;
    services.archive_serialize_actor_slot = archive_serialize_actor_slot;
    services.archive_serialize_url = archive_serialize_url;
    services.archive_serialize_model = archive_serialize_model;
    services.archive_serialize_reach_specs = archive_serialize_reach_specs;
    services.archive_serialize_f32 = archive_serialize_f32;
    services.archive_serialize_actor_ptr = archive_serialize_actor_ptr;
    services.archive_serialize_text_buffer = archive_serialize_text;
    services.archive_serialize_travel_info = archive_serialize_travel_info;
    services.archive_serialize_travel_info_legacy = archive_serialize_travel_info_legacy;
    services.level_time_seconds_get_float = archive_level_time;
    services
}

#[test]
fn core_base_serialize_roundtrip_matches_fixture() {
    let _lock = core_lock();
    let services = archive_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let object = &mut level as *mut ULevel as Obj;
    *lock_shared(&ARCHIVE) = Some(ArchiveMock {
        loading: false,
        version: 64,
    });
    run_scenario("core.base_serialize_roundtrip", &[], &[], || {
        unsafe { level::level_base_serialize(&services, object, ptr::null_mut()) };
        unsafe { archive_close(8) };
        with_archive(|mock| mock.loading = true);
        unsafe { level::level_base_serialize(&services, object, ptr::null_mut()) };
    });
}

fn run_level_serialize(name: &str, version: i32, loading: bool, close_size: Option<i32>) {
    let services = archive_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    *lock_shared(&ARCHIVE) = Some(ArchiveMock { loading, version });
    run_scenario(name, &[], &[], || {
        unsafe {
            level::level_serialize(&services, &mut level as *mut ULevel as Obj, ptr::null_mut())
        };
        if let Some(size) = close_size {
            unsafe { archive_close(size) };
        }
    });
}

#[test]
fn core_level_serialize_versions_match_fixture() {
    let _lock = core_lock();
    run_level_serialize("core.serialize_save_ver64", 64, false, Some(14));
    run_level_serialize("core.serialize_save_ver60", 60, false, Some(13));
    run_level_serialize("core.serialize_load_ver64", 64, true, None);
}

unsafe extern "C" fn ctor_set_flags(_obj: Obj, _flags: u32) {}

unsafe extern "C" fn ctor_get_outer(_obj: Obj) -> Obj {
    ptr::null_mut()
}

unsafe extern "C" fn ctor_model_new_root(_outer: Obj, root_outside: i32) -> Obj {
    push_record(format!("model_construct\t0\t{root_outside}"));
    0x20usize as Obj
}

unsafe extern "C" fn ctor_spawn_actor(_obj: Obj, _class: Obj) {
    push_record("level_spawn_actor\t100");
}

unsafe extern "C" fn ctor_level_info_class() -> Obj {
    0x40usize as Obj
}

unsafe extern "C" fn ctor_get_level_info(_obj: Obj) -> Obj {
    0x41usize as Obj
}

unsafe extern "C" fn ctor_spawn_brush(_obj: Obj) -> Obj {
    push_record("level_spawn_brush\t101");
    0x50usize as Obj
}

unsafe extern "C" fn ctor_actor_at(_obj: Obj, index: i32) -> Obj {
    assert_eq!(index, 1);
    0x50usize as Obj
}

unsafe extern "C" fn ctor_model_new_brush(_outer: Obj, _owner: Obj) -> Obj {
    push_record("uobject_allocate");
    push_record("model_construct\t1\t1");
    0x60usize as Obj
}

unsafe extern "C" fn ctor_set_brush(_actor: Obj, _brush: Obj) {}

fn run_ctor_case(name: &str, root_outside: i32) {
    let mut services: Hp2LevelCoreServices = all_never();
    services.uobject_set_flags = ctor_set_flags;
    services.uobject_get_outer = ctor_get_outer;
    services.model_new_root = ctor_model_new_root;
    services.level_spawn_actor = ctor_spawn_actor;
    services.alevel_info_static_class = ctor_level_info_class;
    services.level_get_level_info = ctor_get_level_info;
    services.level_spawn_brush = ctor_spawn_brush;
    services.actors_get = ctor_actor_at;
    services.model_new_brush = ctor_model_new_brush;
    services.brush_actor_set_brush = ctor_set_brush;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(name, &[], &[], || {
        push_record("uobject_allocate");
        unsafe { level::level_init(&services, &mut level as *mut ULevel as Obj, root_outside) }
            .unwrap();
    });
}

#[test]
fn core_level_constructor_scenarios_match_fixture() {
    let _lock = core_lock();
    run_ctor_case("core.init_level_ctor", 0);
    run_ctor_case("core.init_level_ctor_root", 1);
}

unsafe extern "C" fn record_model_shrink(_model: Obj) {
    push_record("model_shrink_model");
}

unsafe extern "C" fn record_reach_specs_shrink(_obj: Obj) {
    push_service("level_reach_specs_shrink");
}

unsafe extern "C" fn record_uobject_modify(_obj: Obj) {
    push_record("uobject_modify");
}

/// The shell calls `UModel::Modify()` with the default `DoTransArrays = 0` (`UnModel.h:86`).
unsafe extern "C" fn record_model_modify(_model: Obj) {
    push_record("model_modify\t0");
}

unsafe extern "C" fn record_uobject_post_load(_obj: Obj) {
    push_record("uobject_post_load");
}

unsafe extern "C" fn record_retarget_xlevels(_obj: Obj) {
    push_service("level_retarget_actor_xlevels");
}

unsafe extern "C" fn record_high_detail(_obj: Obj, value: i32) {
    HIGH_DETAIL.with(|h| h.set(value));
    push_service(format!("level_info_set_high_detail\t{value}"));
}

unsafe extern "C" fn record_event_detail_change(_game: Obj) {
    push_record("event_detail_change");
}

unsafe extern "C" fn record_app_system_time(_info: Obj) {
    push_record("app_system_time");
}

#[test]
fn core_simple_level_scenarios_match_fixture() {
    let _lock = core_lock();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let obj = &mut level as *mut ULevel as Obj;

    // Hash is null and collision is 0: nothing is reached.
    let services: Hp2LevelCoreServices = all_never();
    run_scenario("core.set_collision_noop", &[], &[], || unsafe {
        level::level_set_actor_collision(&services, obj, 0)
    });

    let mut services: Hp2LevelCoreServices = all_never();
    services.model_shrink_model = record_model_shrink;
    services.level_reach_specs_shrink = record_reach_specs_shrink;
    run_scenario(
        "core.shrink_model",
        &[],
        &["level_reach_specs_shrink"],
        || unsafe { level::level_shrink(&services, obj) },
    );

    services = all_never();
    services.uobject_modify = record_uobject_modify;
    services.model_modify = record_model_modify;
    run_scenario("core.modify_model", &[], &[], || unsafe {
        level::level_modify(&services, obj)
    });

    // The outer-match and outer-mixed cases differ only in the C++ body's `xlevel`
    // output, which is excluded below; the port's call sequence is identical for both.
    services = all_never();
    services.uobject_post_load = record_uobject_post_load;
    services.level_retarget_actor_xlevels = record_retarget_xlevels;
    run_scenario(
        "core.post_load_outer_match",
        &["xlevel\t"],
        &["level_retarget_actor_xlevels"],
        || unsafe { level::level_post_load(&services, obj) },
    );
    run_scenario(
        "core.post_load_outer_mixed",
        &["xlevel\t"],
        &["level_retarget_actor_xlevels"],
        || unsafe { level::level_post_load(&services, obj) },
    );

    services = all_never();
    services.uobject_destroy = record_uobject_destroy;
    run_scenario("core.base_destroy_no_drivers", &[], &[], || unsafe {
        level::level_base_destroy(&services, obj)
    });
    run_scenario("core.destroy_hash_none", &[], &[], || unsafe {
        level::level_destroy(&services, obj)
    });

    services = all_never();
    services.level_info_set_high_detail = record_high_detail;
    services.level_info_has_game = core_obj_bool;
    run_scenario(
        "core.detail_change_no_game",
        &[],
        &["level_info_set_high_detail\t1"],
        || unsafe {
            level::level_detail_change(&services, obj, 1);
            dump_high_detail();
        },
    );

    services.level_info_has_game = core_has_game;
    services.level_info_game_event_detail_change = record_event_detail_change;
    run_scenario(
        "core.detail_change_game",
        &[],
        &["level_info_set_high_detail\t0"],
        || unsafe {
            // The harness starts with HighDetail=1, so the dump proves the set happened.
            HIGH_DETAIL.with(|h| h.set(1));
            level::level_detail_change(&services, obj, 0);
            dump_high_detail();
        },
    );

    services = all_never();
    services.app_system_time_from_level_info = record_app_system_time;
    run_scenario("core.update_time", &["level_info_time\t"], &[], || unsafe {
        level::level_update_time(&services, obj)
    });

    services = all_never();
    services.level_has_client = core_client_available;
    services.level_client_viewports_num = core_zero_viewports;
    run_scenario("core.remember_actors_no_viewports", &[], &[], || unsafe {
        level::level_remember_actors(&services, obj)
    });

    // Engine->Client is NULL: the oracle body does nothing; only the viewport dumps remain.
    services = all_never();
    services.level_has_client = core_obj_bool;
    run_scenario(
        "core.remember_actors_no_client",
        &["viewport_saved\t", "viewport_actor\t"],
        &[],
        || unsafe { level::level_remember_actors(&services, obj) },
    );

    services = all_never();
    services.level_has_client = core_client_available;
    services.level_client_viewports_num = core_one_viewport;
    services.level_client_viewport = core_get_viewport;
    services.viewport_remember_actor = record_viewport_remember;
    run_scenario(
        "core.remember_actors_save_view",
        &["viewport_saved\t", "viewport_actor\t"],
        &["viewport_remember_actor\t0x20"],
        || unsafe { level::level_remember_actors(&services, obj) },
    );
}

#[test]
fn core_audible_occlusion_none_matches_fixture() {
    let _lock = core_lock();
    let services: Hp2LevelCoreServices = all_never();
    // Zeroed ULevel: plain data; no field is read on the OCCLUSION_None path.
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    let src: FVector = unsafe { std::mem::zeroed() };
    let lst: FVector = unsafe { std::mem::zeroed() };
    const OCCLUSION_NONE: i32 = 1;
    begin_scenario();
    let got = unsafe {
        level::level_is_audible_at(
            &services,
            &mut level as *mut ULevel as Obj,
            &src,
            &lst,
            ptr::null_mut(),
            OCCLUSION_NONE,
            &mut hit,
        )
    };
    check_scenario("core.is_audible_occlusion_none", &[], &[]);
    assert_eq!(got, fixture_return("core.is_audible_occlusion_none"));
}

#[test]
fn core_is_server_no_drivers_matches_fixture() {
    let _lock = core_lock();
    let services: Hp2LevelNetServices = all_never();
    // Zeroed ULevel: NetDriver and DemoRecDriver are both NULL.
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    begin_scenario();
    let got = unsafe { level_net::is_server(&services, &mut level as *mut ULevel as *mut c_void) };
    check_scenario("core.is_server_no_drivers", &[], &[]);
    assert_eq!(got, fixture_return("core.is_server_no_drivers"));
}

unsafe extern "C" fn record_script_cycles_clear() {
    push_service("script_cycles_clear");
}

unsafe extern "C" fn record_script_entry_tag_clear() {
    push_service("script_entry_tag_clear");
}

#[test]
fn core_init_stats_matches_fixture() {
    let _lock = core_lock();
    let mut services: Hp2LevelCoreServices = all_never();
    services.script_cycles_clear = record_script_cycles_clear;
    services.script_entry_tag_clear = record_script_entry_tag_clear;

    // Seed every counter that `level_init_stats` zeroes.
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    level.net_tick_cycles = 7;
    level.net_diff_cycles = 7;
    level.actor_tick_cycles = 7;
    level.audio_tick_cycles = 7;
    level.find_path_cycles = 7;
    level.move_cycles = 7;
    level.num_moves = 7;
    level.num_reps = 7;
    level.num_pv = 7;
    level.get_relevant_cycles = 7;
    level.num_rpc = 7;
    level.see_player = 7;
    level.spawning = 7;
    level.unused = 7;

    let obj = &mut level as *mut ULevel as Obj;
    run_scenario(
        "core.init_stats",
        &[],
        &["script_cycles_clear", "script_entry_tag_clear"],
        || unsafe { level::level_init_stats(&services, obj) },
    );

    assert_eq!(level.net_tick_cycles, 0);
    assert_eq!(level.net_diff_cycles, 0);
    assert_eq!(level.actor_tick_cycles, 0);
    assert_eq!(level.audio_tick_cycles, 0);
    assert_eq!(level.find_path_cycles, 0);
    assert_eq!(level.move_cycles, 0);
    assert_eq!(level.num_moves, 0);
    assert_eq!(level.num_reps, 0);
    assert_eq!(level.num_pv, 0);
    assert_eq!(level.get_relevant_cycles, 0);
    assert_eq!(level.num_rpc, 0);
    assert_eq!(level.see_player, 0);
    assert_eq!(level.spawning, 0);
    assert_eq!(level.unused, 0);
}

unsafe extern "C" fn gseconds_per_cycle_stub() -> f32 {
    GSECONDS_PER_CYCLE.with(Cell::get)
}

unsafe extern "C" fn script_cycles_f32_stub() -> f32 {
    SCRIPT_CYCLES.with(Cell::get)
}

/// Records the values Rust computed; the C++ format literal is checked against the fixture separately.
unsafe extern "C" fn record_stats(_result: *mut u32, vals: *const f32, num_moves: i32) {
    let v = unsafe { std::slice::from_raw_parts(vals, 9) }.to_vec();
    STATS_CALLS.with(|s| s.borrow_mut().push((v, num_moves)));
}

/// Field labels of the oracle `stats` record, in order.
fn fixture_stats_labels(name: &str) -> Vec<String> {
    fixture_stats(name)
        .split_whitespace()
        .filter_map(|t| t.split_once('=').map(|(label, _)| label.to_string()))
        .collect()
}

/// The `vals` and `num_moves` of the oracle `stats` record, in the order `vals` is passed.
fn fixture_stats_values(name: &str) -> (Vec<f32>, i32) {
    let mut vals: Vec<f32> = Vec::new();
    let mut num_moves: Option<i32> = None;
    for token in fixture_stats(name).split_whitespace() {
        if let Some(moves) = token.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
            num_moves = Some(moves.parse().expect("integer num_moves"));
        } else {
            let (_, value) = token.split_once('=').expect("label=value token");
            vals.push(value.parse().expect("float stats value"));
        }
    }
    (vals, num_moves.expect("(num_moves) token"))
}

#[test]
fn core_get_stats_matches_fixture() {
    let _lock = core_lock();
    // The oracle `stats` record fixes the label order that `vals` must follow.
    assert_eq!(
        fixture_stats_labels("core.get_stats"),
        [
            "Script", "Actor", "Path", "See", "Spawn", "Audio", "Un", "Move", "Net"
        ]
    );

    let mut services: Hp2LevelCoreServices = all_never();
    services.gseconds_per_cycle = gseconds_per_cycle_stub;
    services.script_cycles_f32 = script_cycles_f32_stub;
    services.app_sprintf_stats = record_stats;
    let mut result = [0u32; 256];

    // Case 1: the oracle scenario seeds the counters to produce its exact stats record.
    begin_scenario();
    GSECONDS_PER_CYCLE.with(|g| g.set(0.001));
    SCRIPT_CYCLES.with(|g| g.set(1.0));
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    level.actor_tick_cycles = 2;
    level.find_path_cycles = 3;
    level.see_player = 4;
    level.spawning = 5;
    level.audio_tick_cycles = 6;
    level.unused = 7;
    level.move_cycles = 8;
    level.net_tick_cycles = 10;
    level.num_moves = 9;
    unsafe {
        level::level_get_stats(
            &services,
            &mut level as *mut ULevel as Obj,
            result.as_mut_ptr(),
        )
    };
    check_scenario("core.get_stats", &["stats\t"], &[]);
    assert_eq!(
        take_stats_calls(),
        vec![fixture_stats_values("core.get_stats")]
    );
    assert_eq!(fixture_return("core.get_stats"), 0);

    // Case 2: distinct values, with no oracle record. GSecondsPerCycle = 1.0, so k = 1000
    // exactly. Script comes from the script-cycle service; the rest come from the level fields.
    begin_scenario();
    GSECONDS_PER_CYCLE.with(|g| g.set(1.0));
    SCRIPT_CYCLES.with(|g| g.set(2.0));
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    level.actor_tick_cycles = 3;
    level.find_path_cycles = 4;
    level.see_player = 5;
    level.spawning = 6;
    level.audio_tick_cycles = 7;
    level.unused = 8;
    level.move_cycles = 9;
    level.net_tick_cycles = 10;
    level.num_moves = 11;
    unsafe {
        level::level_get_stats(
            &services,
            &mut level as *mut ULevel as Obj,
            result.as_mut_ptr(),
        )
    };
    assert_eq!(
        take_stats_calls(),
        vec![(
            vec![
                2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0, 8000.0, 9000.0, 10000.0
            ],
            11
        )]
    );
    assert_fully_wired("core.get_stats (distinct values)");
}
