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
use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APlayerPawn, FArchive, FCheckResult, ULevel, UGameEngine, UViewport,
};
use hp2_engine::level::{self, Hp2LevelCoreServices, Obj};
use hp2_engine::level_net::{self, Hp2LevelNetServices};
use hp2_engine::level_spawn_actor::Hp2SpawnActorContext;

#[path = "level_spawn_actor/mock.rs"]
mod level_spawn_actor_mock;

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
#[derive(Clone, Copy)]
struct PostLoadActors {
    level: Obj,
    level_outer: Obj,
    iterator: Obj,
    iterator_index: i32,
    actors: [Obj; 2],
    actor_outers: [Obj; 2],
}

thread_local! {
    static POST_LOAD_ACTORS: RefCell<Option<PostLoadActors>> = const { RefCell::new(None) };
}

fn dump_post_load_xlevels() {
    POST_LOAD_ACTORS.with(|state| {
        let state = state.borrow();
        let state = state.as_ref().expect("post-load actors initialized");
        for (index, actor) in state.actors.iter().enumerate() {
            let xlevel = unsafe { (*actor.cast::<AActor>()).x_level };
            push_record(format!(
                "xlevel\t{}\t{}",
                index * 2,
                i32::from(xlevel == state.level)
            ));
        }
    });
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

unsafe extern "C" fn core_one_viewport(_obj: Obj) -> i32 {
    1
}

struct RememberViewportFixture {
    viewport: Box<UViewport>,
    actor: Box<APlayerPawn>,
}

thread_local! {
    static REMEMBER_VIEWPORT: RefCell<Option<RememberViewportFixture>> = const { RefCell::new(None) };
}

unsafe extern "C" fn core_get_viewport(_obj: Obj, index: i32) -> Obj {
    assert_eq!(index, 0);
    REMEMBER_VIEWPORT.with(|cell| {
        cell.borrow()
            .as_ref()
            .expect("remember viewport installed")
            .viewport
            .as_ref() as *const UViewport as Obj
    })
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
    /// Real `APlayerPawn`-shaped backing memory for the fields the port now reads/writes
    /// directly through the mirror (`ExcludeGameStates`, `bHiddenEd`,
    /// `bInCurrentGameState`, `CurrentGameState`, `GameStateMasterList`,
    /// `GameStateTokenLen`). An `AActor` prefix is also valid for any `APlayerPawn*`.
    mem: Box<APlayerPawn>,
}

struct ScreenMock {
    actors: Vec<Option<ScreenActor>>,
    current: String,
    master: String,
    editor: i32,
}

// SAFETY: the `mem` buffers are private, heap-owned engine-shaped memory, only ever
// accessed through the `SCREEN` mutex.
unsafe impl Send for ScreenMock {}

static SCREEN: Mutex<Option<ScreenMock>> = Mutex::new(None);

fn with_screen<R>(f: impl FnOnce(&mut ScreenMock) -> R) -> R {
    f(lock_shared(&SCREEN)
        .as_mut()
        .expect("screen mock installed"))
}

/// Maps a real actor `Obj` (the address of its `mem`) back to its `actors` slot.
fn screen_actor_index(actor: Obj) -> Option<usize> {
    if actor.is_null() {
        return None;
    }
    with_screen(|screen| {
        screen.actors.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|entry| entry.mem.as_ref() as *const APlayerPawn as Obj == actor)
        })
    })
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
#[repr(transparent)]
struct ScreenFStr {
    text: String,
}

unsafe fn fstring(handle: Obj) -> &'static mut ScreenFStr {
    unsafe { &mut *handle.cast::<ScreenFStr>() }
}

/// Resolves a handle's text. Real mirror addresses (the `ExcludeGameStates`,
/// `CurrentGameState`, `GameStateMasterList` fields the port now reads directly) are
/// recognized by address and resolved against the `ScreenMock`/`ScreenActor` data they
/// point into; anything else is a `ScreenFStr` scratch handle.
fn screen_handle_text(h: Obj) -> String {
    let addr = h as usize;
    with_screen(|screen| {
        let current = screen.current.clone();
        let master = screen.master.clone();
        for actor in screen.actors.iter().flatten() {
            let mem_ptr: *const APlayerPawn = actor.mem.as_ref();
            unsafe {
                if addr == ptr::addr_of!((*mem_ptr.cast::<AActor>()).exclude_game_states) as usize
                {
                    return actor.exclude.clone();
                }
                if addr == ptr::addr_of!((*mem_ptr).current_game_state) as usize {
                    return current;
                }
                if addr == ptr::addr_of!((*mem_ptr).game_state_master_list) as usize {
                    return master;
                }
            }
        }
        unsafe { fstring(h) }.text.clone()
    })
}

unsafe extern "C" fn screen_fstring_caps(dst: Obj, src: Obj) {
    let value = screen_handle_text(src).to_ascii_uppercase();
    unsafe { fstring(dst) }.text = value;
}

unsafe extern "C" fn screen_fstring_len(handle: Obj) -> i32 {
    screen_handle_text(handle).len() as i32
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
    let value = screen_handle_text(a).to_ascii_uppercase();
    let needle = screen_handle_text(b).to_ascii_uppercase();
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
            .map_or(ptr::null_mut(), |actor| {
                actor.mem.as_ref() as *const APlayerPawn as Obj
            })
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

unsafe extern "C" fn screen_actor_resolve(_actor: Obj) {
    push_record("resolve_game_state\t1");
}

unsafe extern "C" fn screen_object_name(dst: Obj, actor: Obj) {
    let index = screen_actor_index(actor).expect("non-null actor");
    let name = with_screen(|screen| screen.actors[index].as_ref().unwrap().name.clone());
    unsafe { fstring(dst) }.text = name;
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
    let mut mem: Box<APlayerPawn> = Box::new(unsafe { std::mem::zeroed() });
    mem.game_state_token_len = 10;
    mem.base.base.set_b_hidden_ed(hidden != 0);
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
        mem,
    }
}

fn screen_services() -> Hp2LevelCoreServices {
    let mut services: Hp2LevelCoreServices = checked_core_services();

    services.fstring_caps = checked_fstring_caps_screen_fstring_caps;
    services.fstring_len = checked_fstring_len_screen_fstring_len;

    services.fstring_assign_chars = checked_fstring_assign_chars_screen_fstring_assign_chars;
    services.fstring_mid = checked_fstring_mid_screen_fstring_mid;
    services.fstring_instr_chars = checked_fstring_instr_chars_screen_fstring_instr_chars;
    services.fstring_instr_fstring = checked_fstring_instr_fstring_screen_fstring_instr_fstring;
    services.fstring_eq_chars = checked_fstring_eq_chars_screen_fstring_eq_chars;
    services.fstring_eq_fstring = checked_fstring_eq_fstring_screen_fstring_eq_fstring;

    services.is_editor = checked_is_editor_screen_is_editor;
    services.object_get_name = checked_object_get_name_screen_object_name;
    services.actors_num = checked_actors_num_screen_actors_num;
    services.actors_get = checked_actors_get_screen_actor_at;
    services.actor_is_player = checked_actor_is_player_screen_actor_is_player;
    services.actor_group_chars = checked_actor_group_chars_screen_actor_group;
    services.actor_event_on_resolve_game_state =
        checked_actor_event_on_resolve_game_state_screen_actor_resolve;
    services
}

fn run_screen_case(name: &str, mock: ScreenMock) {
    *lock_shared(&SCREEN) = Some(mock);
    let services = screen_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(name, &[], &[], || {
        unsafe {
            with_checked_core(&services, || {
                level::level_screen_actors_by_game_state(
                    &services,
                    &mut level as *mut ULevel as Obj,
                )
            })
            .unwrap()
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
                    actor.index,
                    actor.mem.base.base.b_in_current_game_state() as i32,
                    actor.mem.base.base.b_hidden_ed() as i32
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

unsafe extern "C" fn collision_hash_new() -> Obj {
    push_record("collision_hash_new");
    0x10usize as Obj
}

unsafe extern "C" fn collision_actors_num(_obj: Obj) -> i32 {
    3
}

/// Real `AActor`-shaped backing for the collision scenarios: `actor_collide_actors` now
/// reads `bCollideActors` directly, so the two actors returned by `actors_get` need live
/// memory instead of opaque integer tokens.
struct CollisionActors {
    a: Box<AActor>,
    b: Box<AActor>,
}

// SAFETY: heap-owned engine-shaped memory, only ever accessed through the mutex below.
unsafe impl Send for CollisionActors {}

static COLLISION_ACTORS: Mutex<Option<CollisionActors>> = Mutex::new(None);

fn with_collision_actors<R>(f: impl FnOnce(&CollisionActors) -> R) -> R {
    f(lock_shared(&COLLISION_ACTORS)
        .as_ref()
        .expect("collision actors installed"))
}

fn new_colliding_actor() -> Box<AActor> {
    let mut actor: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
    actor.set_b_collide_actors(true);
    actor
}

unsafe extern "C" fn collision_actor_at(_obj: Obj, index: i32) -> Obj {
    with_collision_actors(|actors| match index {
        0 => actors.a.as_ref() as *const AActor as Obj,
        2 => actors.b.as_ref() as *const AActor as Obj,
        _ => ptr::null_mut(),
    })
}

/// Fixture actor numbering for the collision mock: actor `a` is index 0, actor `b` is index 2.
fn actor_index(actor: Obj) -> i64 {
    with_collision_actors(|actors| {
        if actor as usize == actors.a.as_ref() as *const AActor as usize {
            0
        } else if actor as usize == actors.b.as_ref() as *const AActor as usize {
            2
        } else {
            -1
        }
    })
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
    *lock_shared(&COLLISION_ACTORS) = Some(CollisionActors {
        a: new_colliding_actor(),
        b: new_colliding_actor(),
    });
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let obj = &mut level as *mut ULevel as Obj;
    let mut services: Hp2LevelCoreServices = checked_core_services();

    services.collision_hash_new = checked_collision_hash_new_collision_hash_new;
    services.actors_num = checked_actors_num_collision_actors_num;
    services.actors_get = checked_actors_get_collision_actor_at;
    services.collision_hash_add_actor = checked_collision_hash_add_actor_collision_hash_add;
    run_scenario(
        "core.set_collision_enable",
        &[],
        &[
            "app_rand_trace_phase_scope_enter\tcollision_hash",
            "app_rand_trace_phase_scope_leave\t0x30",
        ],
        || unsafe {
            with_checked_core(&services, || {
                level::level_set_actor_collision(&services, obj, 1)
            })
            .unwrap()
        },
    );

    services = checked_core_services();
    services.collision_hash_new = checked_collision_hash_new_collision_hash_new;
    services.actors_num = checked_actors_num_collision_actors_num;
    services.actors_get = checked_actors_get_collision_actor_at;
    services.collision_hash_remove_actor =
        checked_collision_hash_remove_actor_collision_hash_remove;
    services.collision_hash_delete = checked_collision_hash_delete_collision_hash_delete;
    run_scenario("core.set_collision_disable", &[], &[], || {
        let hash = unsafe { collision_hash_new() };
        unsafe { (*(obj as *mut ULevel)).hash = hash };
        unsafe {
            with_checked_core(&services, || {
                level::level_set_actor_collision(&services, obj, 0)
            })
            .unwrap()
        };
    });
    *lock_shared(&COLLISION_ACTORS) = None;
}

#[test]
fn core_destroy_hash_present_matches_fixture() {
    let _lock = core_lock();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let obj = &mut level as *mut ULevel as Obj;
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.collision_hash_new = checked_collision_hash_new_collision_hash_new;
    services.collision_hash_delete = checked_collision_hash_delete_collision_hash_delete;
    services.uobject_destroy = checked_uobject_destroy_record_uobject_destroy;
    run_scenario("core.destroy_hash_present", &[], &[], || {
        let hash = unsafe { collision_hash_new() };
        unsafe { (*(obj as *mut ULevel)).hash = hash };
        unsafe { with_checked_core(&services, || level::level_destroy(&services, obj)).unwrap() };
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
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.level_single_line_check = checked_level_single_line_check_trace_line_check;
    services.actor_is_a_aplayer_pawn = checked_actor_is_a_aplayer_pawn_trace_source_is_pawn;
    services.actor_is_owned_by = checked_actor_is_owned_by_trace_actor_owned;
    services.actor_is_a_alevel_info = checked_actor_is_a_alevel_info_trace_is_level_info;
    services.model_node_izone1_of_actor = checked_model_node_izone1_of_actor_trace_model_zone;
    services.pawn_is_zone_visible = checked_pawn_is_zone_visible_trace_zone_visible;
    services.actor_class_is_child_of = checked_actor_class_is_child_of_trace_actor_class;
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
            with_checked_core(&services, || {
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
                )
            })
            .unwrap();
        } else {
            with_checked_core(&services, || {
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
                )
            })
            .unwrap();
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
        with_checked_core(services, || {
            level::level_is_audible_at(
                services,
                obj,
                &src,
                &dst,
                ptr::null_mut(),
                occlusion,
                &mut hit,
            )
        })
        .unwrap()
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

    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.model_fast_line_check = checked_model_fast_line_check_core_line_clear;
    audible_case("core.is_audible_occlusion_default_clear", &services, obj, 0);

    services = checked_core_services();
    services.model_fast_line_check = checked_model_fast_line_check_core_line_blocked;
    audible_case(
        "core.is_audible_occlusion_default_blocked",
        &services,
        obj,
        0,
    );

    services = checked_core_services();
    services.level_single_line_check = checked_level_single_line_check_core_single_line_clear;
    audible_case(
        "core.is_audible_occlusion_static_clear",
        &services,
        obj,
        OCCLUSION_STATIC_MESHES,
    );

    services = checked_core_services();
    services.level_single_line_check = checked_level_single_line_check_core_single_line_blocked;
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

thread_local! {
    /// Real `APlayerPawn`-shaped backing for the two player-pawn actors (old fixture indices
    /// 2 and 3): `pawn_get_player`/`pawn_set_player` now read/write `Player` directly.
    static RECONCILE_PAWNS: RefCell<Option<[Box<APlayerPawn>; 2]>> = const { RefCell::new(None) };
}

fn reconcile_pawn_obj(slot: usize) -> Obj {
    RECONCILE_PAWNS.with(|cell| {
        cell.borrow().as_ref().expect("reconcile pawns installed")[slot].as_ref() as *const APlayerPawn
            as Obj
    })
}

unsafe extern "C" fn reconcile_actor_at(_obj: Obj, index: i32) -> Obj {
    match index {
        0 => ptr::null_mut(),
        1 => 0x2usize as Obj,
        2 => reconcile_pawn_obj(0),
        3 => reconcile_pawn_obj(1),
        _ => ptr::null_mut(),
    }
}

unsafe extern "C" fn reconcile_is_player_pawn(actor: Obj) -> i32 {
    RECONCILE_PAWNS.with(|cell| {
        cell.borrow()
            .as_ref()
            .expect("reconcile pawns installed")
            .iter()
            .any(|pawn| pawn.as_ref() as *const APlayerPawn as Obj == actor) as i32
    })
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
    RECONCILE_PAWNS.with(|cell| {
        *cell.borrow_mut() = Some([
            {
                let mut pawn: Box<APlayerPawn> = Box::new(unsafe { std::mem::zeroed() });
                pawn.player = 0x10usize as Obj;
                pawn
            },
            {
                let mut pawn: Box<APlayerPawn> = Box::new(unsafe { std::mem::zeroed() });
                pawn.player = 0x10usize as Obj;
                pawn
            },
        ]);
    });
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.is_editor = checked_is_editor_reconcile_is_editor;
    services.actors_num = checked_actors_num_reconcile_actor_count;
    services.actors_get = checked_actors_get_reconcile_actor_at;
    services.actor_is_a_aplayer_pawn = checked_actor_is_a_aplayer_pawn_reconcile_is_player_pawn;
    services.level_client_viewports_num = checked_level_client_viewports_num_reconcile_no_viewports;
    services.actor_cast_camera = checked_actor_cast_camera_reconcile_not_camera;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario("core.reconcile_actors_player_cleared", &[], &[], || {
        unsafe {
            with_checked_core(&services, || {
                level::level_reconcile_actors(&services, &mut level as *mut ULevel as Obj)
            })
        }
        .unwrap();
        for (index, slot) in [2, 3].into_iter().enumerate() {
            let player = unsafe { (*reconcile_pawn_obj(index).cast::<APlayerPawn>()).player };
            push_record(format!("pawn_player\t{slot}\t{}", player as usize));
        }
    });
    RECONCILE_PAWNS.with(|cell| *cell.borrow_mut() = None);
}

struct ReconcileActor {
    id: usize,
    tag: &'static str,
    camera: bool,
    mem: Box<APlayerPawn>,
}

impl ReconcileActor {
    fn new(id: usize, tag: &'static str, camera: bool) -> Self {
        ReconcileActor {
            id,
            tag,
            camera,
            mem: Box::new(unsafe { std::mem::zeroed() }),
        }
    }

    fn obj(&self) -> Obj {
        self.mem.as_ref() as *const APlayerPawn as Obj
    }
}

struct ReconcileMock {
    actors: Vec<ReconcileActor>,
}

// `APlayerPawn` carries raw pointer fields from the C++ layout, but access is always
// serialized through `RECONCILE`'s mutex, so sharing `ReconcileMock` across threads is safe.
unsafe impl Send for ReconcileMock {}

static RECONCILE: Mutex<Option<ReconcileMock>> = Mutex::new(None);
static RECONCILE_CHARS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

thread_local! {
    /// Real `UViewport`-shaped backing for the single client viewport these scenarios
    /// share. `viewport_get_actor`/`viewport_match_actor`/`camera_restore_view` are now
    /// real mirror reads/writes, so `Actor` has to live somewhere real.
    static RECONCILE_VIEWPORT: RefCell<Option<Box<UViewport>>> = const { RefCell::new(None) };
}

fn reconcile_viewport_obj() -> Obj {
    RECONCILE_VIEWPORT.with(|cell| {
        cell.borrow()
            .as_ref()
            .expect("reconcile viewport installed")
            .as_ref() as *const UViewport as Obj
    })
}

fn with_reconcile<R>(f: impl FnOnce(&mut ReconcileMock) -> R) -> R {
    f(lock_shared(&RECONCILE)
        .as_mut()
        .expect("reconcile mock installed"))
}

/// The fixture id (1, 2 or 7) for a real actor pointer, so records stay byte-identical to
/// the oracle, which predates real mirror-shaped actor backing.
fn reconcile_actor_id(actor: Obj) -> usize {
    with_reconcile(|mock| {
        mock.actors
            .iter()
            .find(|entry| entry.obj() == actor)
            .expect("known reconcile actor")
            .id
    })
}

unsafe extern "C" fn reconcile_record_actor_count(_obj: Obj) -> i32 {
    with_reconcile(|mock| mock.actors.len() as i32)
}

unsafe extern "C" fn reconcile_record_actor_at(_obj: Obj, index: i32) -> Obj {
    with_reconcile(|mock| mock.actors[index as usize].obj())
}

unsafe extern "C" fn reconcile_not_pawn(_actor: Obj) -> i32 {
    0
}

unsafe extern "C" fn reconcile_is_camera(actor: Obj) -> i32 {
    with_reconcile(|mock| {
        mock.actors
            .iter()
            .find(|entry| entry.obj() == actor)
            .is_some_and(|entry| entry.camera) as i32
    })
}

unsafe extern "C" fn reconcile_compare_tag(actor: Obj, _viewport: Obj) -> i32 {
    with_reconcile(|mock| {
        let entry = mock
            .actors
            .iter()
            .find(|entry| entry.obj() == actor)
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
    reconcile_viewport_obj()
}

unsafe extern "C" fn reconcile_get_name(dst: Obj, _object: Obj) {
    unsafe { fstring(dst) }.text = "Cam0".to_string();
}

unsafe extern "C" fn reconcile_spawn_view_actor(_obj: Obj, viewport: Obj) {
    let new_actor = with_reconcile(|mock| {
        mock.actors.push(ReconcileActor::new(7, "Other", true));
        mock.actors.last().expect("just pushed").obj()
    });
    // `SpawnViewActor` wires the new actor into the viewport as a side effect; the kept
    // leaf hides that, so the mock does it directly on the real viewport memory.
    unsafe { (*viewport.cast::<UViewport>()).actor = new_actor };
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
    let viewport = reconcile_viewport_obj();
    if unsafe { (*viewport.cast::<UViewport>()).actor } == view {
        viewport
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn reconcile_destroy_actor(_obj: Obj, actor: Obj) {
    push_record(format!(
        "level_destroy_actor\t{}",
        reconcile_actor_id(actor)
    ));
}

unsafe extern "C" fn reconcile_clear_flags(view: Obj, flags: u32) {
    push_service(format!(
        "uobject_clear_flags\t{}\t{flags}",
        reconcile_actor_id(view)
    ));
}

fn run_reconcile_view_case(name: &str, actors: Vec<ReconcileActor>, expected_services: &[&str]) {
    *lock_shared(&RECONCILE) = Some(ReconcileMock { actors });
    RECONCILE_VIEWPORT
        .with(|cell| *cell.borrow_mut() = Some(Box::new(unsafe { std::mem::zeroed() })));
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.is_editor = checked_is_editor_reconcile_is_editor;
    services.actors_num = checked_actors_num_reconcile_record_actor_count;
    services.actors_get = checked_actors_get_reconcile_record_actor_at;
    services.actor_is_a_aplayer_pawn = checked_actor_is_a_aplayer_pawn_reconcile_not_pawn;
    services.actor_is_a_camera = checked_actor_is_a_camera_reconcile_is_camera;
    services.actor_tag_stricmp_viewport_name =
        checked_actor_tag_stricmp_viewport_name_reconcile_compare_tag;
    services.level_client_viewports_num =
        checked_level_client_viewports_num_reconcile_viewport_count;
    services.level_client_viewport = checked_level_client_viewport_reconcile_viewport_at;

    services.object_get_name = checked_object_get_name_reconcile_get_name;

    services.level_spawn_view_actor = checked_level_spawn_view_actor_reconcile_spawn_view_actor;
    services.actor_cast_camera = checked_actor_cast_camera_reconcile_cast_camera;
    services.camera_player_viewport = checked_camera_player_viewport_reconcile_camera_viewport;
    services.level_destroy_actor = checked_level_destroy_actor_reconcile_destroy_actor;
    services.uobject_clear_flags = checked_uobject_clear_flags_reconcile_clear_flags;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(
        name,
        &["viewport_saved\t", "viewport_actor\t", "view_fields\t"],
        expected_services,
        || {
            unsafe {
                with_checked_core(&services, || {
                    level::level_reconcile_actors(&services, &mut level as *mut ULevel as Obj)
                })
            }
            .unwrap();
        },
    );
    *lock_shared(&RECONCILE) = None;
    RECONCILE_VIEWPORT.with(|cell| *cell.borrow_mut() = None);
}

#[test]
fn core_reconcile_viewport_scenarios_match_fixture() {
    let _lock = core_lock();
    run_reconcile_view_case(
        "core.reconcile_actors_match_identical",
        vec![
            ReconcileActor::new(1, "Other", true),
            ReconcileActor::new(2, "Cam0", true),
        ],
        &["uobject_clear_flags\t2\t1"],
    );
    run_reconcile_view_case(
        "core.reconcile_actors_spawn_unmatched",
        vec![ReconcileActor::new(1, "Other", true)],
        &["uobject_clear_flags\t7\t1"],
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
    reconcile_viewport_obj()
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
        let mut services: Hp2LevelCoreServices = checked_core_services();
        services.check_failed = reconcile_check_failed;
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        let obj = &mut level as *mut ULevel as Obj;
        if case == "editor" {
            services.is_editor = checked_is_editor_reconcile_not_editor;
        } else {
            services.is_editor = checked_is_editor_reconcile_is_editor;
            services.actors_num = checked_actors_num_reconcile_record_actor_count;
            services.level_client_viewports_num =
                checked_level_client_viewports_num_reconcile_one_viewport;
            services.level_client_viewport =
                checked_level_client_viewport_reconcile_one_viewport_at;
            *lock_shared(&RECONCILE) = Some(ReconcileMock { actors: Vec::new() });
            let mut viewport: Box<UViewport> = Box::new(unsafe { std::mem::zeroed() });
            viewport.actor = 1usize as Obj;
            RECONCILE_VIEWPORT.with(|cell| *cell.borrow_mut() = Some(viewport));
        }
        lock_shared(&CHECK_FAILURES).clear();
        let status = unsafe {
            with_checked_core(&services, || level::level_reconcile_actors(&services, obj))
        };
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
        RECONCILE_VIEWPORT.with(|cell| *cell.borrow_mut() = None);
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

/// Builds an `FArchive`-shaped buffer in `ar` and returns a pointer to it: `archive_is_loading`,
/// `archive_is_saving`, `archive_is_trans` and `archive_ver` are now direct mirror reads of
/// `ArIsLoading`/`ArIsSaving`/`ArIsTrans`/`ArVer`, not service calls, so scenarios that drive
/// those fields must back `ar: Obj` with real memory instead of a mock function pointer.
fn archive_obj(ar: &mut FArchive, loading: bool, version: i32) -> Obj {
    *ar = unsafe { std::mem::zeroed() };
    ar.ar_ver = version;
    ar.ar_is_loading = loading as i32;
    ar.ar_is_saving = (!loading) as i32;
    ar as *mut FArchive as Obj
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

unsafe extern "C" fn archive_level_time(_obj: Obj) -> f32 {
    0.0
}

unsafe extern "C" fn archive_close(size: i32) {
    push_record(format!("archive_close\t{size}"));
}

fn archive_services() -> Hp2LevelCoreServices {
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.uobject_serialize = checked_uobject_serialize_archive_uobject_serialize;

    services.actors_num = checked_actors_num_archive_actors_count;
    services.actors_count_bytes = checked_actors_count_bytes_archive_count_bytes;
    services.archive_serialize_i32 = checked_archive_serialize_i32_archive_serialize_i32;
    services.actors_empty = checked_actors_empty_archive_actors_empty;
    services.actors_add = checked_actors_add_archive_actors_add;
    services.actor_slot_dump_env_path = checked_actor_slot_dump_env_path_archive_slots_env_path;
    services.archive_serialize_actor_slot =
        checked_archive_serialize_actor_slot_archive_serialize_actor_slot;
    services.archive_serialize_url = checked_archive_serialize_url_archive_serialize_url;
    services.archive_serialize_model = checked_archive_serialize_model_archive_serialize_model;
    services.archive_serialize_reach_specs =
        checked_archive_serialize_reach_specs_archive_serialize_reach_specs;
    services.archive_serialize_f32 = checked_archive_serialize_f32_archive_serialize_f32;
    services.archive_serialize_actor_ptr =
        checked_archive_serialize_actor_ptr_archive_serialize_actor_ptr;
    services.archive_serialize_text_buffer =
        checked_archive_serialize_text_buffer_archive_serialize_text;
    services.archive_serialize_travel_info =
        checked_archive_serialize_travel_info_archive_serialize_travel_info;

    services.level_time_seconds_get_float = checked_level_time_seconds_get_float_archive_level_time;
    services
}

#[test]
fn core_base_serialize_roundtrip_matches_fixture() {
    let _lock = core_lock();
    let services = archive_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let object = &mut level as *mut ULevel as Obj;
    let mut ar: FArchive = unsafe { std::mem::zeroed() };
    *lock_shared(&ARCHIVE) = Some(ArchiveMock {
        loading: false,
        version: 64,
    });
    run_scenario("core.base_serialize_roundtrip", &[], &[], || {
        unsafe {
            with_checked_core(&services, || {
                level::level_base_serialize(&services, object, archive_obj(&mut ar, false, 64))
            })
            .unwrap()
        };
        unsafe { archive_close(8) };
        with_archive(|mock| mock.loading = true);
        unsafe {
            with_checked_core(&services, || {
                level::level_base_serialize(&services, object, archive_obj(&mut ar, true, 64))
            })
            .unwrap()
        };
    });
}

fn run_level_serialize(name: &str, version: i32, loading: bool, close_size: Option<i32>) {
    let services = archive_services();
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut ar: FArchive = unsafe { std::mem::zeroed() };
    *lock_shared(&ARCHIVE) = Some(ArchiveMock { loading, version });
    run_scenario(name, &[], &[], || {
        unsafe {
            with_checked_core(&services, || {
                level::level_serialize(
                    &services,
                    &mut level as *mut ULevel as Obj,
                    archive_obj(&mut ar, loading, version),
                )
            })
            .unwrap()
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

unsafe extern "C" fn ctor_construct(
    _ctx: *const Hp2SpawnActorContext,
    _template: usize,
    out: *mut usize,
) -> i32 {
    push_record("level_spawn_actor\t100");
    unsafe { *out = 0 };
    0
}

unsafe extern "C" fn ctor_level_info_class() -> Obj {
    0x40usize as Obj
}

unsafe extern "C" fn ctor_get_level_info(_obj: Obj) -> Obj {
    0x41usize as Obj
}

thread_local! {
    /// Real `AActor`-shaped backing for `run_ctor_case`'s default-brush actor:
    /// `brush_actor_set_brush` now writes `Brush` directly, so the handle `level_spawn_brush`
    /// and `actors_get(1)` agree on needs to be live memory, not an opaque integer.
    static CTOR_BRUSH_ACTOR: RefCell<Option<Box<AActor>>> = const { RefCell::new(None) };
}

fn ctor_brush_actor_obj() -> Obj {
    CTOR_BRUSH_ACTOR.with(|cell| {
        cell.borrow()
            .as_ref()
            .expect("ctor brush actor installed")
            .as_ref() as *const AActor as Obj
    })
}

unsafe extern "C" fn ctor_spawn_brush(_obj: Obj) -> Obj {
    push_record("level_spawn_brush\t101");
    ctor_brush_actor_obj()
}

unsafe extern "C" fn ctor_actor_at(_obj: Obj, index: i32) -> Obj {
    assert_eq!(index, 1);
    ctor_brush_actor_obj()
}

unsafe extern "C" fn ctor_model_new_brush(_outer: Obj, _owner: Obj) -> Obj {
    push_record("uobject_allocate");
    push_record("model_construct\t1\t1");
    0x60usize as Obj
}

fn run_ctor_case(name: &str, root_outside: i32) {
    CTOR_BRUSH_ACTOR.with(|cell| *cell.borrow_mut() = Some(Box::new(unsafe { std::mem::zeroed() })));
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.uobject_set_flags = checked_uobject_set_flags_ctor_set_flags;
    services.uobject_get_outer = checked_uobject_get_outer_ctor_get_outer;
    services.model_new_root = checked_model_new_root_ctor_model_new_root;
    level_spawn_actor_mock::publish(ctor_construct);
    services.alevel_info_static_class = checked_alevel_info_static_class_ctor_level_info_class;
    services.level_get_level_info = checked_level_get_level_info_ctor_get_level_info;
    services.level_spawn_brush = checked_level_spawn_brush_ctor_spawn_brush;
    services.actors_get = checked_actors_get_ctor_actor_at;
    services.model_new_brush = checked_model_new_brush_ctor_model_new_brush;
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    run_scenario(name, &[], &[], || {
        push_record("uobject_allocate");
        unsafe {
            with_checked_core(&services, || {
                level::level_init(&services, &mut level as *mut ULevel as Obj, root_outside)
            })
        }
        .unwrap();
    });
    CTOR_BRUSH_ACTOR.with(|cell| *cell.borrow_mut() = None);
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

unsafe extern "C" fn post_load_actor_iterator_get(iterator: Obj) -> Obj {
    POST_LOAD_ACTORS.with(|state| {
        let state = state.borrow();
        let state = state.as_ref().unwrap();
        assert_eq!(iterator, state.iterator);
        state
            .actors
            .get(state.iterator_index as usize)
            .copied()
            .unwrap_or(ptr::null_mut())
    })
}

unsafe extern "C" fn post_load_actor_iterator_next(iterator: Obj) {
    POST_LOAD_ACTORS.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().unwrap();
        assert_eq!(iterator, state.iterator);
        state.iterator_index += 1;
    });
}

unsafe extern "C" fn post_load_object_get_outer(obj: Obj) -> Obj {
    POST_LOAD_ACTORS.with(|state| {
        let state = state.borrow();
        let state = state.as_ref().unwrap();
        if obj == state.level {
            state.level_outer
        } else {
            let index = state.actors.iter().position(|actor| *actor == obj).unwrap();
            state.actor_outers[index]
        }
    })
}
fn run_post_load_case(name: &str, mixed: bool) {
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut actors: [AActor; 2] = unsafe { std::mem::zeroed() };
    let mut level_outer = 0u8;
    let mut other_outer = 0u8;
    let obj = &mut level as *mut ULevel as Obj;
    let actor_ptrs = actors.each_mut().map(|actor| actor as *mut AActor as Obj);
    let same_outer = &mut level_outer as *mut u8 as Obj;
    let different_outer = &mut other_outer as *mut u8 as Obj;

    let mut iterator_marker = 0u8;
    let iterator = &mut iterator_marker as *mut u8 as Obj;
    POST_LOAD_ACTORS.with(|state| {
        *state.borrow_mut() = Some(PostLoadActors {
            level: obj,
            level_outer: same_outer,
            iterator,
            iterator_index: 0,
            actors: actor_ptrs,
            actor_outers: [same_outer, if mixed { different_outer } else { same_outer }],
        });
    });

    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.uobject_post_load = checked_uobject_post_load_record_uobject_post_load;

    services.actor_iterator_get = checked_actor_iterator_get_post_load_actor_iterator_get;
    services.actor_iterator_next = checked_actor_iterator_next_post_load_actor_iterator_next;

    services.object_get_outer = checked_object_get_outer_post_load_object_get_outer;
    run_scenario(name, &[], &[], || unsafe {
        with_checked_core(&services, || level::level_post_load(&services, obj)).unwrap();
        dump_post_load_xlevels();
    });
    POST_LOAD_ACTORS.with(|state| *state.borrow_mut() = None);
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
    let mut engine: UGameEngine = unsafe { std::mem::zeroed() };
    level.base.engine = &mut engine as *mut UGameEngine as Obj;

    // Hash is null and collision is 0: nothing is reached.
    let services: Hp2LevelCoreServices = checked_core_services();
    run_scenario("core.set_collision_noop", &[], &[], || unsafe {
        with_checked_core(&services, || {
            level::level_set_actor_collision(&services, obj, 0)
        })
        .unwrap()
    });

    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.model_shrink_model = checked_model_shrink_model_record_model_shrink;
    services.level_reach_specs_shrink = checked_level_reach_specs_shrink_record_reach_specs_shrink;
    run_scenario(
        "core.shrink_model",
        &[],
        &["level_reach_specs_shrink"],
        || unsafe { with_checked_core(&services, || level::level_shrink(&services, obj)).unwrap() },
    );

    services = checked_core_services();
    services.uobject_modify = checked_uobject_modify_record_uobject_modify;
    services.model_modify = checked_model_modify_record_model_modify;
    run_scenario("core.modify_model", &[], &[], || unsafe {
        with_checked_core(&services, || level::level_modify(&services, obj)).unwrap()
    });

    run_post_load_case("core.post_load_outer_match", false);
    run_post_load_case("core.post_load_outer_mixed", true);

    services = checked_core_services();
    services.uobject_destroy = checked_uobject_destroy_record_uobject_destroy;
    run_scenario("core.base_destroy_no_drivers", &[], &[], || unsafe {
        with_checked_core(&services, || level::level_base_destroy(&services, obj)).unwrap()
    });
    run_scenario("core.destroy_hash_none", &[], &[], || unsafe {
        with_checked_core(&services, || level::level_destroy(&services, obj)).unwrap()
    });

    services = checked_core_services();
    services.level_info_set_high_detail = checked_level_info_set_high_detail_record_high_detail;
    services.level_info_has_game = checked_level_info_has_game_core_obj_bool;
    run_scenario(
        "core.detail_change_no_game",
        &[],
        &["level_info_set_high_detail\t1"],
        || unsafe {
            with_checked_core(&services, || level::level_detail_change(&services, obj, 1)).unwrap();
            dump_high_detail();
        },
    );

    services.level_info_has_game = checked_level_info_has_game_core_has_game;
    services.level_info_game_event_detail_change =
        checked_level_info_game_event_detail_change_record_event_detail_change;
    run_scenario(
        "core.detail_change_game",
        &[],
        &["level_info_set_high_detail\t0"],
        || unsafe {
            // The harness starts with HighDetail=1, so the dump proves the set happened.
            HIGH_DETAIL.with(|h| h.set(1));
            with_checked_core(&services, || level::level_detail_change(&services, obj, 0)).unwrap();
            dump_high_detail();
        },
    );

    services = checked_core_services();
    services.app_system_time_from_level_info =
        checked_app_system_time_from_level_info_record_app_system_time;
    run_scenario("core.update_time", &["level_info_time\t"], &[], || unsafe {
        with_checked_core(&services, || level::level_update_time(&services, obj)).unwrap()
    });

    services = checked_core_services();
    services.level_client_viewports_num = checked_level_client_viewports_num_core_zero_viewports;
    engine.client = 1usize as Obj;
    run_scenario("core.remember_actors_no_viewports", &[], &[], || unsafe {
        with_checked_core(&services, || level::level_remember_actors(&services, obj)).unwrap()
    });

    // Engine->Client is NULL: the oracle body does nothing; only the viewport dumps remain.
    services = checked_core_services();
    engine.client = ptr::null_mut();
    run_scenario(
        "core.remember_actors_no_client",
        &["viewport_saved\t", "viewport_actor\t"],
        &[],
        || unsafe {
            with_checked_core(&services, || level::level_remember_actors(&services, obj)).unwrap()
        },
    );

    services = checked_core_services();
    services.level_client_viewports_num = checked_level_client_viewports_num_core_one_viewport;
    services.level_client_viewport = checked_level_client_viewport_core_get_viewport;
    engine.client = 1usize as Obj;
    let mut remember_viewport: Box<UViewport> = Box::new(unsafe { std::mem::zeroed() });
    let mut remember_actor: Box<APlayerPawn> = Box::new(unsafe { std::mem::zeroed() });
    remember_viewport.actor = remember_actor.as_mut() as *mut APlayerPawn as Obj;
    REMEMBER_VIEWPORT.with(|cell| {
        *cell.borrow_mut() = Some(RememberViewportFixture {
            viewport: remember_viewport,
            actor: remember_actor,
        })
    });
    run_scenario(
        "core.remember_actors_save_view",
        &["viewport_saved\t", "viewport_actor\t"],
        &[],
        || unsafe {
            with_checked_core(&services, || level::level_remember_actors(&services, obj)).unwrap()
        },
    );
    REMEMBER_VIEWPORT.with(|cell| *cell.borrow_mut() = None);
}

#[test]
fn core_audible_occlusion_none_matches_fixture() {
    let _lock = core_lock();
    let services: Hp2LevelCoreServices = checked_core_services();
    // Zeroed ULevel: plain data; no field is read on the OCCLUSION_None path.
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    let src: FVector = unsafe { std::mem::zeroed() };
    let lst: FVector = unsafe { std::mem::zeroed() };
    const OCCLUSION_NONE: i32 = 1;
    begin_scenario();
    let got = unsafe {
        with_checked_core(&services, || {
            level::level_is_audible_at(
                &services,
                &mut level as *mut ULevel as Obj,
                &src,
                &lst,
                ptr::null_mut(),
                OCCLUSION_NONE,
                &mut hit,
            )
        })
        .unwrap()
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
    let got = unsafe { level_net::is_server(&services, &mut level as *mut ULevel as *mut c_void) }
        .unwrap();
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
    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.script_cycles_clear = checked_script_cycles_clear_record_script_cycles_clear;
    services.script_entry_tag_clear = checked_script_entry_tag_clear_record_script_entry_tag_clear;

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
        || unsafe {
            with_checked_core(&services, || level::level_init_stats(&services, obj)).unwrap()
        },
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

fn core_get_stats_matches_fixture() {
    let _lock = core_lock();
    // The oracle `stats` record fixes the label order that `vals` must follow.
    assert_eq!(
        fixture_stats_labels("core.get_stats"),
        [
            "Script", "Actor", "Path", "See", "Spawn", "Audio", "Un", "Move", "Net"
        ]
    );

    let mut services: Hp2LevelCoreServices = checked_core_services();
    services.gseconds_per_cycle = checked_gseconds_per_cycle_gseconds_per_cycle_stub;
    services.script_cycles_f32 = checked_script_cycles_f32_script_cycles_f32_stub;
    services.app_sprintf_stats = checked_app_sprintf_stats_record_stats;
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
        with_checked_core(&services, || {
            level::level_get_stats(
                &services,
                &mut level as *mut ULevel as Obj,
                result.as_mut_ptr(),
            )
        })
        .unwrap()
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
        with_checked_core(&services, || {
            level::level_get_stats(
                &services,
                &mut level as *mut ULevel as Obj,
                result.as_mut_ptr(),
            )
        })
        .unwrap()
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

// Checked ABI adapters for the existing policy oracle. Native boundary acceptance
// is provided separately by native_probe.cpp, never by this fixture table.
thread_local! {
    static ACTIVE_CORE: Cell<*const Hp2LevelCoreServices> = const { Cell::new(ptr::null()) };
}
fn with_checked_core<T>(s: &Hp2LevelCoreServices, f: impl FnOnce() -> T) -> T {
    struct Restore(*const Hp2LevelCoreServices);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_CORE.with(|s| s.set(self.0));
        }
    }
    let _restore = Restore(ACTIVE_CORE.with(|slot| slot.replace(s)));
    f()
}
unsafe fn active_core() -> &'static Hp2LevelCoreServices {
    unsafe { &*ACTIVE_CORE.with(Cell::get) }
}
fn core_status(r: Result<(), ()>) -> i32 {
    if r.is_ok() { 0 } else { 1 }
}
unsafe extern "C" fn checked_fstring_caps_screen_fstring_caps(dst: Obj, src: Obj) -> i32 {
    unsafe {
        screen_fstring_caps(dst, src);
    }
    0
}
unsafe extern "C" fn checked_fstring_len_screen_fstring_len(s: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = screen_fstring_len(s);
    }
    0
}
unsafe extern "C" fn checked_fstring_assign_chars_screen_fstring_assign_chars(
    dst: Obj,
    src: *const u32,
) -> i32 {
    unsafe {
        screen_fstring_assign_chars(dst, src);
    }
    0
}
unsafe extern "C" fn checked_fstring_mid_screen_fstring_mid(
    dst: Obj,
    src: Obj,
    start: i32,
    count: i32,
) -> i32 {
    unsafe {
        screen_fstring_mid(dst, src, start, count);
    }
    0
}
unsafe extern "C" fn checked_fstring_instr_chars_screen_fstring_instr_chars(
    s: Obj,
    sub: *const u32,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = screen_fstring_instr_chars(s, sub);
    }
    0
}
unsafe extern "C" fn checked_fstring_instr_fstring_screen_fstring_instr_fstring(
    s: Obj,
    sub: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = screen_fstring_instr_fstring(s, sub);
    }
    0
}
unsafe extern "C" fn checked_fstring_eq_chars_screen_fstring_eq_chars(
    s: Obj,
    lit: *const u32,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = screen_fstring_eq_chars(s, lit);
    }
    0
}
unsafe extern "C" fn checked_fstring_eq_fstring_screen_fstring_eq_fstring(
    a: Obj,
    b: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = screen_fstring_eq_fstring(a, b);
    }
    0
}
unsafe extern "C" fn checked_is_editor_screen_is_editor(out: *mut i32) -> i32 {
    unsafe {
        *out = screen_is_editor();
    }
    0
}
unsafe extern "C" fn checked_object_get_name_screen_object_name(dst: Obj, obj: Obj) -> i32 {
    unsafe {
        screen_object_name(dst, obj);
    }
    0
}
unsafe extern "C" fn checked_actors_num_screen_actors_num(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = screen_actors_num(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_get_screen_actor_at(obj: Obj, i: i32, out: *mut Obj) -> i32 {
    unsafe {
        *out = screen_actor_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_actor_is_player_screen_actor_is_player(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = screen_actor_is_player(actor);
    }
    0
}
unsafe extern "C" fn checked_actor_group_chars_screen_actor_group(
    actor: Obj,
    out: *mut *const u32,
) -> i32 {
    unsafe {
        *out = screen_actor_group(actor);
    }
    0
}
unsafe extern "C" fn checked_actor_event_on_resolve_game_state_screen_actor_resolve(
    actor: Obj,
) -> i32 {
    unsafe {
        screen_actor_resolve(actor);
    }
    0
}
unsafe extern "C" fn checked_collision_hash_new_collision_hash_new(out: *mut Obj) -> i32 {
    unsafe {
        *out = collision_hash_new();
    }
    0
}
unsafe extern "C" fn checked_actors_num_collision_actors_num(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = collision_actors_num(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_get_collision_actor_at(obj: Obj, i: i32, out: *mut Obj) -> i32 {
    unsafe {
        *out = collision_actor_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_collision_hash_add_actor_collision_hash_add(
    hash: Obj,
    actor: Obj,
) -> i32 {
    unsafe {
        collision_hash_add(hash, actor);
    }
    0
}
unsafe extern "C" fn checked_collision_hash_remove_actor_collision_hash_remove(
    hash: Obj,
    actor: Obj,
) -> i32 {
    unsafe {
        collision_hash_remove(hash, actor);
    }
    0
}
unsafe extern "C" fn checked_collision_hash_delete_collision_hash_delete(hash: Obj) -> i32 {
    unsafe {
        collision_hash_delete(hash);
    }
    0
}
unsafe extern "C" fn checked_uobject_destroy_record_uobject_destroy(obj: Obj) -> i32 {
    unsafe {
        record_uobject_destroy(obj);
    }
    0
}
unsafe extern "C" fn checked_level_single_line_check_trace_line_check(
    obj: Obj,
    hit: *mut c_void,
    source: Obj,
    end: *const c_void,
    start: *const c_void,
    flags: u32,
    extent: *const c_void,
    node_flags: u8,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_line_check(obj, hit, source, end, start, flags, extent, node_flags);
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_aplayer_pawn_trace_source_is_pawn(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_source_is_pawn(actor);
    }
    0
}
unsafe extern "C" fn checked_actor_is_owned_by_trace_actor_owned(
    actor: Obj,
    other: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_actor_owned(actor, other);
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_alevel_info_trace_is_level_info(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_is_level_info(actor);
    }
    0
}
unsafe extern "C" fn checked_model_node_izone1_of_actor_trace_model_zone(
    actor: Obj,
    item: i32,
    out: *mut u8,
) -> i32 {
    unsafe {
        *out = trace_model_zone(actor, item);
    }
    0
}
unsafe extern "C" fn checked_pawn_is_zone_visible_trace_zone_visible(
    pawn: Obj,
    zone: u8,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_zone_visible(pawn, zone);
    }
    0
}
unsafe extern "C" fn checked_actor_class_is_child_of_trace_actor_class(
    actor: Obj,
    parent: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = trace_actor_class(actor, parent);
    }
    0
}
unsafe extern "C" fn checked_model_fast_line_check_core_line_clear(
    model: Obj,
    end: *const c_void,
    start: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_line_clear(model, end, start);
    }
    0
}
unsafe extern "C" fn checked_model_fast_line_check_core_line_blocked(
    model: Obj,
    end: *const c_void,
    start: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_line_blocked(model, end, start);
    }
    0
}
unsafe extern "C" fn checked_level_single_line_check_core_single_line_clear(
    obj: Obj,
    hit: *mut c_void,
    source: Obj,
    end: *const c_void,
    start: *const c_void,
    flags: u32,
    extent: *const c_void,
    node_flags: u8,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_single_line_clear(obj, hit, source, end, start, flags, extent, node_flags);
    }
    0
}
unsafe extern "C" fn checked_level_single_line_check_core_single_line_blocked(
    obj: Obj,
    hit: *mut c_void,
    source: Obj,
    end: *const c_void,
    start: *const c_void,
    flags: u32,
    extent: *const c_void,
    node_flags: u8,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_single_line_blocked(obj, hit, source, end, start, flags, extent, node_flags);
    }
    0
}
unsafe extern "C" fn checked_is_editor_reconcile_is_editor(out: *mut i32) -> i32 {
    unsafe {
        *out = reconcile_is_editor();
    }
    0
}
unsafe extern "C" fn checked_actors_num_reconcile_actor_count(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = reconcile_actor_count(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_get_reconcile_actor_at(obj: Obj, i: i32, out: *mut Obj) -> i32 {
    unsafe {
        *out = reconcile_actor_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_aplayer_pawn_reconcile_is_player_pawn(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_is_player_pawn(actor);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_reconcile_no_viewports(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_no_viewports(obj);
    }
    0
}
unsafe extern "C" fn checked_actor_cast_camera_reconcile_not_camera(
    actor: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_not_camera(actor);
    }
    0
}
unsafe extern "C" fn checked_actors_num_reconcile_record_actor_count(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_record_actor_count(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_get_reconcile_record_actor_at(
    obj: Obj,
    i: i32,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_record_actor_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_aplayer_pawn_reconcile_not_pawn(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_not_pawn(actor);
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_camera_reconcile_is_camera(
    actor: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_is_camera(actor);
    }
    0
}
unsafe extern "C" fn checked_actor_tag_stricmp_viewport_name_reconcile_compare_tag(
    actor: Obj,
    viewport: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_compare_tag(actor, viewport);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_reconcile_viewport_count(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_viewport_count(obj);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewport_reconcile_viewport_at(
    obj: Obj,
    i: i32,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_viewport_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_object_get_name_reconcile_get_name(dst: Obj, obj: Obj) -> i32 {
    unsafe {
        reconcile_get_name(dst, obj);
    }
    0
}
unsafe extern "C" fn checked_level_spawn_view_actor_reconcile_spawn_view_actor(
    obj: Obj,
    viewport: Obj,
) -> i32 {
    unsafe {
        reconcile_spawn_view_actor(obj, viewport);
    }
    0
}
unsafe extern "C" fn checked_actor_cast_camera_reconcile_cast_camera(
    actor: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_cast_camera(actor);
    }
    0
}
unsafe extern "C" fn checked_camera_player_viewport_reconcile_camera_viewport(
    view: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_camera_viewport(view);
    }
    0
}
unsafe extern "C" fn checked_level_destroy_actor_reconcile_destroy_actor(
    obj: Obj,
    actor: Obj,
) -> i32 {
    unsafe {
        reconcile_destroy_actor(obj, actor);
    }
    0
}
unsafe extern "C" fn checked_uobject_clear_flags_reconcile_clear_flags(
    obj: Obj,
    flags: u32,
) -> i32 {
    unsafe {
        reconcile_clear_flags(obj, flags);
    }
    0
}
unsafe extern "C" fn checked_is_editor_reconcile_not_editor(out: *mut i32) -> i32 {
    unsafe {
        *out = reconcile_not_editor();
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_reconcile_one_viewport(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = reconcile_one_viewport(obj);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewport_reconcile_one_viewport_at(
    obj: Obj,
    i: i32,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = reconcile_one_viewport_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_uobject_serialize_archive_uobject_serialize(obj: Obj, ar: Obj) -> i32 {
    unsafe {
        archive_uobject_serialize(obj, ar);
    }
    0
}
unsafe extern "C" fn checked_actors_num_archive_actors_count(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = archive_actors_count(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_count_bytes_archive_count_bytes(obj: Obj, ar: Obj) -> i32 {
    unsafe {
        archive_count_bytes(obj, ar);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_i32_archive_serialize_i32(
    ar: Obj,
    value: *mut i32,
) -> i32 {
    unsafe {
        archive_serialize_i32(ar, value);
    }
    0
}
unsafe extern "C" fn checked_actors_empty_archive_actors_empty(obj: Obj, n: i32) -> i32 {
    unsafe {
        archive_actors_empty(obj, n);
    }
    0
}
unsafe extern "C" fn checked_actors_add_archive_actors_add(obj: Obj, n: i32) -> i32 {
    unsafe {
        archive_actors_add(obj, n);
    }
    0
}
unsafe extern "C" fn checked_actor_slot_dump_env_path_archive_slots_env_path(
    out: *mut *const c_char,
) -> i32 {
    unsafe {
        *out = archive_slots_env_path();
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_actor_slot_archive_serialize_actor_slot(
    ar: Obj,
    obj: Obj,
    i: i32,
) -> i32 {
    unsafe {
        archive_serialize_actor_slot(ar, obj, i);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_url_archive_serialize_url(ar: Obj, obj: Obj) -> i32 {
    unsafe {
        archive_serialize_url(ar, obj);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_model_archive_serialize_model(
    ar: Obj,
    slot: *mut c_void,
) -> i32 {
    unsafe {
        archive_serialize_model(ar, slot);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_reach_specs_archive_serialize_reach_specs(
    ar: Obj,
    obj: Obj,
) -> i32 {
    unsafe {
        archive_serialize_reach_specs(ar, obj);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_f32_archive_serialize_f32(
    ar: Obj,
    value: *mut f32,
) -> i32 {
    unsafe {
        archive_serialize_f32(ar, value);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_actor_ptr_archive_serialize_actor_ptr(
    ar: Obj,
    slot: *mut c_void,
) -> i32 {
    unsafe {
        archive_serialize_actor_ptr(ar, slot);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_text_buffer_archive_serialize_text(
    ar: Obj,
    slot: *mut c_void,
) -> i32 {
    unsafe {
        archive_serialize_text(ar, slot);
    }
    0
}
unsafe extern "C" fn checked_archive_serialize_travel_info_archive_serialize_travel_info(
    ar: Obj,
    obj: Obj,
) -> i32 {
    unsafe {
        archive_serialize_travel_info(ar, obj);
    }
    0
}
unsafe extern "C" fn checked_level_time_seconds_get_float_archive_level_time(
    obj: Obj,
    out: *mut f32,
) -> i32 {
    unsafe {
        *out = archive_level_time(obj);
    }
    0
}
unsafe extern "C" fn checked_uobject_set_flags_ctor_set_flags(obj: Obj, flags: u32) -> i32 {
    unsafe {
        ctor_set_flags(obj, flags);
    }
    0
}
unsafe extern "C" fn checked_uobject_get_outer_ctor_get_outer(obj: Obj, out: *mut Obj) -> i32 {
    unsafe {
        *out = ctor_get_outer(obj);
    }
    0
}
unsafe extern "C" fn checked_model_new_root_ctor_model_new_root(
    outer: Obj,
    root_outside: i32,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = ctor_model_new_root(outer, root_outside);
    }
    0
}
unsafe extern "C" fn checked_alevel_info_static_class_ctor_level_info_class(out: *mut Obj) -> i32 {
    unsafe {
        *out = ctor_level_info_class();
    }
    0
}
unsafe extern "C" fn checked_level_get_level_info_ctor_get_level_info(
    obj: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = ctor_get_level_info(obj);
    }
    0
}
unsafe extern "C" fn checked_level_spawn_brush_ctor_spawn_brush(obj: Obj, out: *mut Obj) -> i32 {
    unsafe {
        *out = ctor_spawn_brush(obj);
    }
    0
}
unsafe extern "C" fn checked_actors_get_ctor_actor_at(obj: Obj, i: i32, out: *mut Obj) -> i32 {
    unsafe {
        *out = ctor_actor_at(obj, i);
    }
    0
}
unsafe extern "C" fn checked_model_new_brush_ctor_model_new_brush(
    outer: Obj,
    owner: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = ctor_model_new_brush(outer, owner);
    }
    0
}
unsafe extern "C" fn checked_uobject_post_load_record_uobject_post_load(obj: Obj) -> i32 {
    unsafe {
        record_uobject_post_load(obj);
    }
    0
}
unsafe extern "C" fn checked_actor_iterator_get_post_load_actor_iterator_get(
    iterator: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = post_load_actor_iterator_get(iterator);
    }
    0
}
unsafe extern "C" fn checked_actor_iterator_next_post_load_actor_iterator_next(
    iterator: Obj,
) -> i32 {
    unsafe {
        post_load_actor_iterator_next(iterator);
    }
    0
}
unsafe extern "C" fn checked_object_get_outer_post_load_object_get_outer(
    obj: Obj,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = post_load_object_get_outer(obj);
    }
    0
}
unsafe extern "C" fn checked_model_shrink_model_record_model_shrink(model: Obj) -> i32 {
    unsafe {
        record_model_shrink(model);
    }
    0
}
unsafe extern "C" fn checked_level_reach_specs_shrink_record_reach_specs_shrink(obj: Obj) -> i32 {
    unsafe {
        record_reach_specs_shrink(obj);
    }
    0
}
unsafe extern "C" fn checked_uobject_modify_record_uobject_modify(obj: Obj) -> i32 {
    unsafe {
        record_uobject_modify(obj);
    }
    0
}
unsafe extern "C" fn checked_model_modify_record_model_modify(model: Obj) -> i32 {
    unsafe {
        record_model_modify(model);
    }
    0
}
unsafe extern "C" fn checked_level_info_set_high_detail_record_high_detail(
    obj: Obj,
    v: i32,
) -> i32 {
    unsafe {
        record_high_detail(obj, v);
    }
    0
}
unsafe extern "C" fn checked_level_info_has_game_core_obj_bool(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = core_obj_bool(obj);
    }
    0
}
unsafe extern "C" fn checked_level_info_has_game_core_has_game(obj: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = core_has_game(obj);
    }
    0
}
unsafe extern "C" fn checked_level_info_game_event_detail_change_record_event_detail_change(
    obj: Obj,
) -> i32 {
    unsafe {
        record_event_detail_change(obj);
    }
    0
}
unsafe extern "C" fn checked_app_system_time_from_level_info_record_app_system_time(
    info: Obj,
) -> i32 {
    unsafe {
        record_app_system_time(info);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_core_zero_viewports(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_zero_viewports(obj);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewports_num_core_one_viewport(
    obj: Obj,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = core_one_viewport(obj);
    }
    0
}
unsafe extern "C" fn checked_level_client_viewport_core_get_viewport(
    obj: Obj,
    i: i32,
    out: *mut Obj,
) -> i32 {
    unsafe {
        *out = core_get_viewport(obj, i);
    }
    0
}
unsafe extern "C" fn checked_script_cycles_clear_record_script_cycles_clear() -> i32 {
    unsafe {
        record_script_cycles_clear();
    }
    0
}
unsafe extern "C" fn checked_script_entry_tag_clear_record_script_entry_tag_clear() -> i32 {
    unsafe {
        record_script_entry_tag_clear();
    }
    0
}
unsafe extern "C" fn checked_gseconds_per_cycle_gseconds_per_cycle_stub(out: *mut f32) -> i32 {
    unsafe {
        *out = gseconds_per_cycle_stub();
    }
    0
}
unsafe extern "C" fn checked_script_cycles_f32_script_cycles_f32_stub(out: *mut f32) -> i32 {
    unsafe {
        *out = script_cycles_f32_stub();
    }
    0
}
unsafe extern "C" fn checked_app_sprintf_stats_record_stats(
    result: *mut u32,
    vals: *const f32,
    num_moves: i32,
) -> i32 {
    unsafe {
        record_stats(result, vals, num_moves);
    }
    0
}
unsafe extern "C" fn unwired_core_fstring_caps(_dst: Obj, _src: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_len(_s: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_assign_chars(_dst: Obj, _src: *const u32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_mid(
    _dst: Obj,
    _src: Obj,
    _start: i32,
    _count: i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_instr_chars(
    _s: Obj,
    _sub: *const u32,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_instr_fstring(_s: Obj, _sub: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_eq_chars(
    _s: Obj,
    _lit: *const u32,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_fstring_eq_fstring(_a: Obj, _b: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_is_editor(_out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_check_failed(_expr: *const c_char, _line: i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_object_get_name(_dst: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_tag_stricmp_viewport_name(
    _actor: Obj,
    _viewport: Obj,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_app_system_time_from_level_info(_info: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_app_sprintf_stats(
    _result: *mut u32,
    _vals: *const f32,
    _num_moves: i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_gseconds_per_cycle(_out: *mut f32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_script_cycles_f32(_out: *mut f32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_script_cycles_clear() -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_script_entry_tag_clear() -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actors_num(_obj: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actors_get(_obj: Obj, _i: i32, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actors_empty(_obj: Obj, _n: i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actors_add(_obj: Obj, _n: i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actors_count_bytes(_obj: Obj, _ar: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_alevel_info_static_class(_out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_spawn_brush(_obj: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_get_level_info(_obj: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_info_set_high_detail(_obj: Obj, _v: i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_info_has_game(_obj: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_info_game_event_detail_change(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_destroy_actor(_obj: Obj, _actor: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_spawn_view_actor(_obj: Obj, _viewport: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_single_line_check(
    _obj: Obj,
    _hit: *mut c_void,
    _source: Obj,
    _end: *const c_void,
    _start: *const c_void,
    _flags: u32,
    _extent: *const c_void,
    _node_flags: u8,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_reach_specs_shrink(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_client_viewports_num(_obj: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_client_viewport(_obj: Obj, _i: i32, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_iterator_get(_iterator: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_iterator_next(_iterator: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_object_get_outer(_obj: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_time_seconds_get_float(_obj: Obj, _out: *mut f32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_hash_new(_out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_hash_add_actor(_hash: Obj, _actor: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_hash_remove_actor(_hash: Obj, _actor: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_hash_delete(_hash: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_brush_tracker_count_bytes(_tracker: Obj, _ar: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_brush_tracker_delete(_tracker: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_net_driver_delete(_driver: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_is_player(_actor: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_group_chars(_actor: Obj, _out: *mut *const u32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_event_on_resolve_game_state(_actor: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_is_owned_by(
    _actor: Obj,
    _other: Obj,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_is_a_alevel_info(_actor: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_is_a_aplayer_pawn(_actor: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_is_a_camera(_actor: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_cast_camera(_actor: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_class_is_child_of(
    _actor: Obj,
    _parent: Obj,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_node_izone1_of_actor(
    _actor: Obj,
    _item: i32,
    _out: *mut u8,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_pawn_is_zone_visible(
    _pawn: Obj,
    _zone: u8,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_camera_player_viewport(_view: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_new_root(
    _outer: Obj,
    _root_outside: i32,
    _out: *mut Obj,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_new_brush(_outer: Obj, _owner: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_modify(_model: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_shrink_model(_model: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_model_fast_line_check(
    _model: Obj,
    _end: *const c_void,
    _start: *const c_void,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_get_outer(_obj: Obj, _out: *mut Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_set_flags(_obj: Obj, _flags: u32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_clear_flags(_obj: Obj, _flags: u32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_serialize(_obj: Obj, _ar: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_destroy(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_modify(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_uobject_post_load(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_tell(_ar: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_supports_compact_index_trace(
    _ar: Obj,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_set_compact_index_trace(
    _ar: Obj,
    _trace: *mut c_void,
    _out: *mut *mut c_void,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_i32(_ar: Obj, _value: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_f32(_ar: Obj, _value: *mut f32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_actors(_ar: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_actor_slot(
    _ar: Obj,
    _obj: Obj,
    _i: i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_url(_ar: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_net_driver(
    _ar: Obj,
    _slot: *mut c_void,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_model(_ar: Obj, _slot: *mut c_void) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_reach_specs(_ar: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_actor_ptr(_ar: Obj, _slot: *mut c_void) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_text_buffer(
    _ar: Obj,
    _slot: *mut c_void,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_travel_info(_ar: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_preload(_ar: Obj, _model: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_slot_dump_env_path(_out: *mut *const c_char) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_slot_dump_hooks_linked(_out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_slot_dump_begin(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_slot_dump_raw_actor(
    _obj: Obj,
    _slot: i32,
    _actor: Obj,
    _reference_before: i32,
    _reference_after: i32,
    _compact_index: i32,
    _captured: i32,
    _supported: i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_actor_slot_dump_end(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_screen_invalid_state(_pawn: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_exclude_caps_contains(
    _exclude: Obj,
    _state: Obj,
    _out: *mut i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_screen_invalid_token(_buffer: Obj, _token: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_debug_matched_viewport(_viewport: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_post_load_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_screen_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_start_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_collision_end_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_reconcile_identical_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_reconcile_other_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_reconcile_kill_scope(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_base_serialize(_obj: Obj, _ar: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_level_base_destroy(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_legacy_travel_scope(_ar: Obj, _obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_archive_serialize_strings(_ar: Obj, _strings: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_strings_num(_strings: Obj, _out: *mut i32) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_travel_info_reset(_obj: Obj) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}
unsafe extern "C" fn unwired_core_travel_info_set(
    _obj: Obj,
    _names: Obj,
    _items: Obj,
    _index: i32,
) -> i32 {
    UNWIRED_CALLS.with(|c| c.set(c.get() + 1));
    1
}

unsafe extern "C" fn fixture_screen_scope(obj: Obj) -> i32 {
    let mut current = ScreenFStr::default();
    let mut buffer = ScreenFStr::default();
    let mut token = ScreenFStr::default();
    unsafe {
        core_status(level::level_screen_actors_body(
            active_core(),
            obj,
            ptr::from_mut(&mut current).cast(),
            ptr::from_mut(&mut buffer).cast(),
            ptr::from_mut(&mut token).cast(),
        ))
    }
}
unsafe extern "C" fn fixture_invalid_state(pawn: Obj) -> i32 {
    push_record(if pawn.is_null() {
        "debugf\tlog\t**** Can't find a valid player!!!! "
    } else {
        "debugf\tlog\t**** Caution: The player pawn has an Invalid CurrentGameState for this package Load."
    });
    0
}
unsafe extern "C" fn fixture_exclude(exclude: Obj, state: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = screen_handle_text(exclude)
            .to_ascii_uppercase()
            .contains(&screen_handle_text(state).to_ascii_uppercase()) as i32;
    }
    0
}
unsafe extern "C" fn fixture_invalid_token(buffer: Obj, token: Obj) -> i32 {
    unsafe {
        push_record(format!(
            "debugf\terror\t*** GameState ERROR Actor: {} has an invalid GameState Token: {}!!!",
            fstring(buffer).text,
            fstring(token).text
        ));
    }
    0
}
unsafe extern "C" fn fixture_matched_viewport(viewport: Obj) -> i32 {
    let mut name = ScreenFStr::default();
    unsafe {
        reconcile_get_name(ptr::from_mut(&mut name).cast(), viewport);
    }
    push_record(format!("debugf\tlog\tMatched Viewport {}", name.text));
    0
}
unsafe extern "C" fn fixture_post_load(obj: Obj) -> i32 {
    let iterator = POST_LOAD_ACTORS.with(|slot| {
        let mut state = slot.borrow_mut();
        let state = state.as_mut().unwrap();
        state.iterator_index = 0;
        state.iterator
    });
    unsafe { core_status(level::level_post_load_body(active_core(), obj, iterator)) }
}
unsafe extern "C" fn fixture_collision_start(obj: Obj) -> i32 {
    push_service("app_rand_trace_phase_scope_enter\tcollision_hash");
    let result = unsafe { level::level_collision_start(active_core(), obj) };
    push_service("app_rand_trace_phase_scope_leave\t0x30");
    core_status(result)
}
unsafe extern "C" fn fixture_collision_end_scope(obj: Obj) -> i32 {
    unsafe { core_status(level::level_collision_end(active_core(), obj)) }
}
unsafe extern "C" fn fixture_reconcile_identical_scope(obj: Obj) -> i32 {
    unsafe { core_status(level::level_reconcile_identical(active_core(), obj)) }
}
unsafe extern "C" fn fixture_reconcile_other_scope(obj: Obj) -> i32 {
    unsafe { core_status(level::level_reconcile_other(active_core(), obj)) }
}
unsafe extern "C" fn fixture_reconcile_kill_scope(obj: Obj) -> i32 {
    unsafe { core_status(level::level_reconcile_kill(active_core(), obj)) }
}
unsafe extern "C" fn fixture_level_base_destroy(obj: Obj) -> i32 {
    unsafe { core_status(level::level_base_destroy(active_core(), obj)) }
}
unsafe extern "C" fn fixture_base_serialize(obj: Obj, ar: Obj) -> i32 {
    unsafe { core_status(level::level_base_serialize(active_core(), obj, ar)) }
}
unsafe extern "C" fn fixture_legacy(ar: Obj, obj: Obj) -> i32 {
    let mut names = 0;
    let mut items = 0;
    unsafe {
        core_status(level::level_legacy_travel(
            active_core(),
            ar,
            obj,
            ptr::from_mut(&mut names).cast(),
            ptr::from_mut(&mut items).cast(),
        ))
    }
}
unsafe extern "C" fn fixture_strings(_ar: Obj, _strings: Obj) -> i32 {
    0
}
unsafe extern "C" fn fixture_strings_num(_strings: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = 0;
    }
    0
}
unsafe extern "C" fn fixture_travel_reset(_obj: Obj) -> i32 {
    0
}
fn checked_core_services() -> Hp2LevelCoreServices {
    Hp2LevelCoreServices {
        fstring_caps: unwired_core_fstring_caps,
        fstring_len: unwired_core_fstring_len,
        fstring_assign_chars: unwired_core_fstring_assign_chars,
        fstring_mid: unwired_core_fstring_mid,
        fstring_instr_chars: unwired_core_fstring_instr_chars,
        fstring_instr_fstring: unwired_core_fstring_instr_fstring,
        fstring_eq_chars: unwired_core_fstring_eq_chars,
        fstring_eq_fstring: unwired_core_fstring_eq_fstring,
        is_editor: unwired_core_is_editor,
        check_failed: unwired_core_check_failed,
        object_get_name: unwired_core_object_get_name,
        actor_tag_stricmp_viewport_name: unwired_core_actor_tag_stricmp_viewport_name,
        app_system_time_from_level_info: unwired_core_app_system_time_from_level_info,
        app_sprintf_stats: unwired_core_app_sprintf_stats,
        gseconds_per_cycle: unwired_core_gseconds_per_cycle,
        script_cycles_f32: unwired_core_script_cycles_f32,
        script_cycles_clear: unwired_core_script_cycles_clear,
        script_entry_tag_clear: unwired_core_script_entry_tag_clear,
        actors_num: unwired_core_actors_num,
        actors_get: unwired_core_actors_get,
        actors_empty: unwired_core_actors_empty,
        actors_add: unwired_core_actors_add,
        actors_count_bytes: unwired_core_actors_count_bytes,
        alevel_info_static_class: unwired_core_alevel_info_static_class,
        level_spawn_brush: unwired_core_level_spawn_brush,
        level_get_level_info: unwired_core_level_get_level_info,
        level_info_set_high_detail: unwired_core_level_info_set_high_detail,
        level_info_has_game: unwired_core_level_info_has_game,
        level_info_game_event_detail_change: unwired_core_level_info_game_event_detail_change,
        level_destroy_actor: unwired_core_level_destroy_actor,
        level_spawn_view_actor: unwired_core_level_spawn_view_actor,
        level_single_line_check: unwired_core_level_single_line_check,
        level_reach_specs_shrink: unwired_core_level_reach_specs_shrink,
        level_client_viewports_num: unwired_core_level_client_viewports_num,
        level_client_viewport: unwired_core_level_client_viewport,
        actor_iterator_get: unwired_core_actor_iterator_get,
        actor_iterator_next: unwired_core_actor_iterator_next,
        object_get_outer: unwired_core_object_get_outer,
        level_time_seconds_get_float: unwired_core_level_time_seconds_get_float,
        collision_hash_new: unwired_core_collision_hash_new,
        collision_hash_add_actor: unwired_core_collision_hash_add_actor,
        collision_hash_remove_actor: unwired_core_collision_hash_remove_actor,
        collision_hash_delete: unwired_core_collision_hash_delete,
        brush_tracker_count_bytes: unwired_core_brush_tracker_count_bytes,
        brush_tracker_delete: unwired_core_brush_tracker_delete,
        net_driver_delete: unwired_core_net_driver_delete,
        actor_is_player: unwired_core_actor_is_player,
        actor_group_chars: unwired_core_actor_group_chars,
        actor_event_on_resolve_game_state: unwired_core_actor_event_on_resolve_game_state,
        actor_is_owned_by: unwired_core_actor_is_owned_by,
        actor_is_a_alevel_info: unwired_core_actor_is_a_alevel_info,
        actor_is_a_aplayer_pawn: unwired_core_actor_is_a_aplayer_pawn,
        actor_is_a_camera: unwired_core_actor_is_a_camera,
        actor_cast_camera: unwired_core_actor_cast_camera,
        actor_class_is_child_of: unwired_core_actor_class_is_child_of,
        model_node_izone1_of_actor: unwired_core_model_node_izone1_of_actor,
        pawn_is_zone_visible: unwired_core_pawn_is_zone_visible,
        camera_player_viewport: unwired_core_camera_player_viewport,
        model_new_root: unwired_core_model_new_root,
        model_new_brush: unwired_core_model_new_brush,
        model_modify: unwired_core_model_modify,
        model_shrink_model: unwired_core_model_shrink_model,
        model_fast_line_check: unwired_core_model_fast_line_check,
        uobject_get_outer: unwired_core_uobject_get_outer,
        uobject_set_flags: unwired_core_uobject_set_flags,
        uobject_clear_flags: unwired_core_uobject_clear_flags,
        uobject_serialize: unwired_core_uobject_serialize,
        uobject_destroy: unwired_core_uobject_destroy,
        uobject_modify: unwired_core_uobject_modify,
        uobject_post_load: unwired_core_uobject_post_load,
        archive_tell: unwired_core_archive_tell,
        archive_supports_compact_index_trace: unwired_core_archive_supports_compact_index_trace,
        archive_set_compact_index_trace: unwired_core_archive_set_compact_index_trace,
        archive_serialize_i32: unwired_core_archive_serialize_i32,
        archive_serialize_f32: unwired_core_archive_serialize_f32,
        archive_serialize_actors: unwired_core_archive_serialize_actors,
        archive_serialize_actor_slot: unwired_core_archive_serialize_actor_slot,
        archive_serialize_url: unwired_core_archive_serialize_url,
        archive_serialize_net_driver: unwired_core_archive_serialize_net_driver,
        archive_serialize_model: unwired_core_archive_serialize_model,
        archive_serialize_reach_specs: unwired_core_archive_serialize_reach_specs,
        archive_serialize_actor_ptr: unwired_core_archive_serialize_actor_ptr,
        archive_serialize_text_buffer: unwired_core_archive_serialize_text_buffer,
        archive_serialize_travel_info: unwired_core_archive_serialize_travel_info,
        archive_preload: unwired_core_archive_preload,
        actor_slot_dump_env_path: unwired_core_actor_slot_dump_env_path,
        actor_slot_dump_hooks_linked: unwired_core_actor_slot_dump_hooks_linked,
        actor_slot_dump_begin: unwired_core_actor_slot_dump_begin,
        actor_slot_dump_raw_actor: unwired_core_actor_slot_dump_raw_actor,
        actor_slot_dump_end: unwired_core_actor_slot_dump_end,
        screen_invalid_state: fixture_invalid_state,
        exclude_caps_contains: fixture_exclude,
        screen_invalid_token: fixture_invalid_token,
        debug_matched_viewport: fixture_matched_viewport,
        post_load_scope: fixture_post_load,
        screen_scope: fixture_screen_scope,
        collision_start_scope: fixture_collision_start,
        collision_end_scope: fixture_collision_end_scope,
        reconcile_identical_scope: fixture_reconcile_identical_scope,
        reconcile_other_scope: fixture_reconcile_other_scope,
        reconcile_kill_scope: fixture_reconcile_kill_scope,
        level_base_serialize: fixture_base_serialize,
        level_base_destroy: fixture_level_base_destroy,
        legacy_travel_scope: fixture_legacy,
        archive_serialize_strings: fixture_strings,
        strings_num: fixture_strings_num,
        travel_info_reset: fixture_travel_reset,
        travel_info_set: unwired_core_travel_info_set,
    }
}
