//! Fixture replay for the `tick` family of `UnLevTic.cpp` (`actor_tick`).
//!
//! Each fixture case applies its `in` diffs to a fresh world, runs `actor_tick`
//! through mock services that print the same `hook`/`event`/`trace` records as the
//! pinned harness, and compares the ordered record stream, the `res` line, and the
//! `out` snapshot diff against the fixture. Expected values come from the fixture
//! `res`/`hook`/`event`/`trace`/`out` lines, never from the `in` echoes.
//!
//! The `out` snapshot covers the actor fields the harness reports for this family.
//! Service slots not reached by a tick case abort via `never`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;

use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, ULevel, UNetDriver};
use hp2_engine::lev_tic::{Hp2LevTicServices, ftime_from_f32, ftime_sub};
use hp2_engine::lev_tic_actor::actor_tick as checked_actor_tick;

const KNOB_COUNT: usize = 22;
/// Index of `IsProbing` in the knob row (fixture key order).
const K_IS_PROBING: usize = 1;
/// Index of `BeginModernThirdPersonInput` in the knob row (fixture key order).
const K_BEGIN_MODERN: usize = 16;

const KIND_APAWN: i32 = 0;
const KIND_APLAYERPAWN: i32 = 1;
const KIND_ACAMERA: i32 = 3;
const KIND_AWEAPON: i32 = 4;

/// Object names in the harness snapshot order, restricted to the tick-relevant fields.
const SNAPSHOT_OBJECTS: [&str; 9] = ["A0", "A1", "A2", "A3", "A4", "A5", "PP0", "PP1", "Z0"];

#[derive(Clone, Copy)]
enum Cls {
    Object,
    Actor,
    Pawn,
    PlayerPawn,
    Camera,
    Weapon,
    Zone,
    LevelInfo,
}

thread_local! {
    static TRACE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static REGS: RefCell<Vec<(usize, String)>> = const { RefCell::new(Vec::new()) };
    static CLASSES: RefCell<HashMap<usize, Cls>> = RefCell::new(HashMap::new());
    static KNOBS: RefCell<HashMap<String, [i64; KNOB_COUNT]>> = RefCell::new(HashMap::new());
    static ZONE_LRT: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) };
    static EDITOR: Cell<i32> = const { Cell::new(0) };
    static TOKEN: Cell<u64> = const { Cell::new(0) };
    static REL: Cell<*const c_char> = const { Cell::new(std::ptr::null()) };
    static ROOT: Cell<*const c_char> = const { Cell::new(std::ptr::null()) };
    static SVC: Cell<*const Hp2LevTicServices> = const { Cell::new(std::ptr::null()) };
}

struct TickCase<'a> {
    index: usize,
    editor: i32,
    knobs: Vec<(&'a str, [i64; KNOB_COUNT])>,
    inputs: Vec<&'a str>,
    call: &'a str,
    records: Vec<&'a str>,
    result: &'a str,
    outputs: Vec<&'a str>,
}

/// Placeholder for every service slot. A call aborts the process, since panics cannot
/// unwind out of `extern "C"`.
unsafe extern "C" fn never() {
    panic!("unexpected service call: a hook not used by this scenario was reached")
}

/// Builds a services table in which every slot is `never`.
fn all_never() -> Hp2LevTicServices {
    let bytes = size_of::<Hp2LevTicServices>();
    assert_eq!(
        bytes % size_of::<usize>(),
        0,
        "services table must hold only pointer slots"
    );
    let slots: Vec<unsafe extern "C" fn()> = (0..bytes / size_of::<usize>())
        .map(|_| never as unsafe extern "C" fn())
        .collect();
    unsafe { std::ptr::read_unaligned(slots.as_ptr().cast::<Hp2LevTicServices>()) }
}

fn tick_services() -> Hp2LevTicServices {
    let mut svc = all_never();
    svc.trace_event_tick = checked_trace_event_tick_trace_event_tick;
    svc.check_enabled = checked_check_enabled_check_enabled;
    svc.actor_is_a = checked_actor_is_a_actor_is_a;
    svc.actor_event = checked_actor_event_actor_event;
    svc.actor_is_probing = checked_actor_is_probing_actor_is_probing;
    svc.actor_process_state = checked_actor_process_state_actor_process_state;
    svc.actor_perform_physics = checked_actor_perform_physics_actor_perform_physics;
    svc.actor_move_smooth = checked_actor_move_smooth_actor_move_smooth;
    svc.actor_creature_trace_tick = checked_actor_creature_trace_tick_actor_creature_trace_tick;
    svc.player_read_input = checked_player_read_input_player_read_input;
    svc.pawn_begin_modern_tps = checked_pawn_begin_modern_tps_pawn_begin_modern_tps;
    svc.pawn_finish_modern_tps = checked_pawn_finish_modern_tps_pawn_finish_modern_tps;
    svc.pawn_apply_modern_tps = checked_pawn_apply_modern_tps_pawn_apply_modern_tps;
    svc.level_far_move_actor = checked_level_far_move_actor_level_far_move_actor;
    svc.level_zone_time_since_render =
        checked_level_zone_time_since_render_level_zone_time_since_render;
    svc.level_is_editor = checked_level_is_editor_level_is_editor;
    svc.guard_scope = fixture_guard_scope;
    svc.tick_actor_scoped = fixture_tick_actor_scoped;
    svc
}

fn trace_line(line: String) {
    TRACE.with(|t| t.borrow_mut().push(line));
}

fn name_of(ptr: *mut c_void) -> String {
    if ptr.is_null() {
        return "NULL".to_owned();
    }
    let addr = ptr as usize;
    REGS.with(|r| {
        r.borrow()
            .iter()
            .find(|entry| entry.0 == addr)
            .map(|entry| entry.1.clone())
    })
    .unwrap_or_else(|| "?".to_owned())
}

fn hook(method: &str, receiver: *mut c_void) {
    trace_line(format!("hook {method} {}", name_of(receiver)));
}

fn knob(receiver: *mut c_void, index: usize, default: i64) -> i64 {
    let name = name_of(receiver);
    KNOBS.with(|k| k.borrow().get(&name).map_or(default, |row| row[index]))
}

fn c_str(ptr: *const c_char) -> String {
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn event_name(code: i32) -> &'static str {
    match code {
        0 => "Tick",
        1 => "Timer",
        2 => "Expired",
        3 => "AnimEnd",
        4 => "PainTimer",
        5 => "SpeechTimer",
        6 => "UpdateTactics",
        7 => "UpdateEyeHeight",
        8 => "PlayerInput",
        9 => "PlayerTick",
        _ => "?",
    }
}

fn kind_matches(cls: Cls, kind: i32) -> bool {
    match kind {
        KIND_APAWN => matches!(cls, Cls::Pawn | Cls::PlayerPawn | Cls::Camera),
        KIND_APLAYERPAWN => matches!(cls, Cls::PlayerPawn | Cls::Camera),
        KIND_ACAMERA => matches!(cls, Cls::Camera),
        KIND_AWEAPON => matches!(cls, Cls::Weapon),
        _ => false,
    }
}

unsafe extern "C" fn trace_enabled() -> i32 {
    1
}

unsafe extern "C" fn trace_begin_dispatch(
    actor: *mut c_void,
    relation: *const c_char,
    root: *const c_char,
) -> u64 {
    trace_line(format!(
        "trace begin {} {} {}",
        name_of(actor),
        c_str(relation),
        c_str(root)
    ));
    TOKEN.with(|t| {
        let next = t.get() + 1;
        t.set(next);
        next
    })
}

unsafe extern "C" fn trace_event_tick(_token: u64) {
    trace_line("trace event_tick".to_owned());
}

unsafe extern "C" fn trace_end_dispatch(_token: u64, skip_reason: *const c_char) {
    trace_line(format!("trace end {}", c_str(skip_reason)));
}

unsafe extern "C" fn trace_relation_get() -> *const c_char {
    REL.with(|r| r.get())
}

unsafe extern "C" fn trace_relation_set(relation: *const c_char) -> *const c_char {
    REL.with(|r| r.replace(relation))
}

unsafe extern "C" fn trace_root_get() -> *const c_char {
    ROOT.with(|r| r.get())
}

unsafe extern "C" fn trace_root_set(admission: *const c_char) -> *const c_char {
    ROOT.with(|r| r.replace(admission))
}

unsafe extern "C" fn check_enabled() -> i32 {
    0
}

unsafe extern "C" fn tick_actor(actor: *mut c_void, delta: f32, tick_type: i32) -> i32 {
    hook("Tick", actor);
    let svc = SVC.with(|s| s.get());
    unsafe { actor_tick(&*svc, actor, delta, tick_type) }
}

unsafe extern "C" fn actor_is_a(actor: *mut c_void, kind: i32) -> i32 {
    CLASSES
        .with(|c| c.borrow().get(&(actor as usize)).copied())
        .map_or(0, |cls| i32::from(kind_matches(cls, kind)))
}

unsafe extern "C" fn actor_event(actor: *mut c_void, event: i32, _delta: f32) {
    trace_line(format!("event {} {}", event_name(event), name_of(actor)));
}

unsafe extern "C" fn actor_is_probing(actor: *mut c_void, _probe: i32) -> i32 {
    hook("IsProbing", actor);
    knob(actor, K_IS_PROBING, 0) as i32
}

unsafe extern "C" fn actor_process_state(actor: *mut c_void, _delta: f32) {
    hook("ProcessState", actor);
}

unsafe extern "C" fn actor_perform_physics(actor: *mut c_void, _delta: f32) {
    hook("performPhysics", actor);
}

unsafe extern "C" fn actor_move_smooth(actor: *mut c_void, _delta: *const f32) {
    hook("moveSmooth", actor);
}

unsafe extern "C" fn actor_creature_trace_tick(actor: *mut c_void, _delta: f32) {
    trace_line(format!("trace creature {}", name_of(actor)));
}

unsafe extern "C" fn player_read_input(player: *mut c_void, _delta: f32) {
    hook("ReadInput", player);
}

unsafe extern "C" fn pawn_begin_modern_tps(
    pawn: *mut c_void,
    raw_base_x: *mut f32,
    raw_strafe: *mut f32,
    raw_camera_yaw: *mut f32,
    raw_camera_pitch: *mut f32,
) -> i32 {
    hook("BeginModernThirdPersonInput", pawn);
    unsafe {
        *raw_base_x = 0.0;
        *raw_strafe = 0.0;
        *raw_camera_yaw = 0.0;
        *raw_camera_pitch = 0.0;
    }
    knob(pawn, K_BEGIN_MODERN, 0) as i32
}

unsafe extern "C" fn pawn_finish_modern_tps(
    pawn: *mut c_void,
    _active: i32,
    _raw_base_x: f32,
    _raw_strafe: f32,
    _raw_camera_yaw: f32,
    _raw_camera_pitch: f32,
    _delta: f32,
) {
    hook("FinishModernThirdPersonInput", pawn);
}

unsafe extern "C" fn pawn_apply_modern_tps(pawn: *mut c_void) {
    hook("ApplyModernThirdPersonMovement", pawn);
}

unsafe extern "C" fn level_far_move_actor(
    level: *mut c_void,
    _actor: *mut c_void,
    _location: *const f32,
) {
    hook("FarMoveActor", level);
}

unsafe extern "C" fn level_zone_time_since_render(level: *mut c_void, zone_number: i32) -> f32 {
    let now = i64::from_le_bytes(unsafe { (*level.cast::<ULevel>()).time_seconds });
    let lrt = ZONE_LRT.with(|z| z.get()[zone_number as usize]);
    ftime_sub(now, ftime_from_f32(lrt))
}

unsafe extern "C" fn level_is_editor() -> i32 {
    EDITOR.with(|e| e.get())
}

fn alloc<T>(value: T) -> usize {
    Box::into_raw(Box::new(value)) as usize
}

fn zeroed<T>() -> T {
    unsafe { std::mem::zeroed() }
}

fn f32_hex(value: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(value, 16).unwrap())
}

fn fbits(value: f32) -> String {
    format!("{:08x}", value.to_bits())
}

fn flag(value: bool) -> String {
    u8::from(value).to_string()
}

/// Test-side world: every named object is a leaked allocation registered for mock
/// name lookups.
struct World {
    objects: HashMap<String, usize>,
}

impl World {
    fn new() -> Self {
        let mut w = World {
            objects: HashMap::new(),
        };
        let li = alloc(zeroed::<ALevelInfo>());
        let lev = alloc(zeroed::<ULevel>());
        w.add("LI", li, Cls::LevelInfo);
        w.add("LEV", lev, Cls::Object);
        w.add("PLAYER", alloc(0u8), Cls::Object);
        w.add("MESH", alloc(0u8), Cls::Object);
        w.add("DEMOREC", alloc(zeroed::<UNetDriver>()), Cls::Object);
        for name in ["A0", "A1", "A2", "A3", "A5"] {
            w.add(name, alloc(zeroed::<AActor>()), Cls::Actor);
        }
        w.add("A4", alloc(zeroed::<AActor>()), Cls::Weapon);
        w.add("Z0", alloc(zeroed::<AActor>()), Cls::Zone);
        w.add("PP0", alloc(zeroed::<APlayerPawn>()), Cls::PlayerPawn);
        w.add("PP1", alloc(zeroed::<APlayerPawn>()), Cls::Camera);

        let li_ptr = w.at::<ALevelInfo>("LI");
        let lev_ptr = w.at::<ULevel>("LEV");
        unsafe {
            // 100 seconds as a raw FTime (seconds * 2^32).
            (*lev_ptr).time_seconds = 429_496_729_600i64.to_le_bytes();
            (*lev_ptr).base.demo_rec_driver = w.ptr("DEMOREC");
            (*li_ptr).net_mode = 0;
            for name in ["A0", "A1", "A2", "A3", "A4", "A5", "Z0", "PP0", "PP1"] {
                let actor = w.at::<AActor>(name);
                (*actor).x_level = lev_ptr.cast();
                (*actor).level = li_ptr.cast();
                (*actor).role = 4;
                (*actor).remote_role = 2;
            }
        }
        w
    }

    fn add(&mut self, name: &str, addr: usize, cls: Cls) {
        self.objects.insert(name.to_owned(), addr);
        REGS.with(|r| r.borrow_mut().push((addr, name.to_owned())));
        CLASSES.with(|c| c.borrow_mut().insert(addr, cls));
    }

    fn get(&self, name: &str) -> usize {
        *self
            .objects
            .get(name)
            .unwrap_or_else(|| panic!("unknown fixture object {name}"))
    }

    fn at<T>(&self, name: &str) -> *mut T {
        self.get(name) as *mut T
    }

    fn ptr(&self, name: &str) -> *mut c_void {
        if name == "NULL" {
            std::ptr::null_mut()
        } else {
            self.get(name) as *mut c_void
        }
    }
}

fn set_class(addr: usize, value: &str) {
    let cls = match value {
        "AActor" => Cls::Actor,
        "APawn" => Cls::Pawn,
        "APlayerPawn" => Cls::PlayerPawn,
        "ACamera" => Cls::Camera,
        "AWeapon" => Cls::Weapon,
        "AZoneInfo" => Cls::Zone,
        "ALevelInfo" => Cls::LevelInfo,
        other => panic!("unknown fixture class {other}"),
    };
    CLASSES.with(|c| c.borrow_mut().insert(addr, cls));
}

fn set_zone_lrt(index: usize, value: &str) {
    ZONE_LRT.with(|z| {
        let mut lrt = z.get();
        lrt[index] = f32_hex(value);
        z.set(lrt);
    });
}

/// Applies one `in <OBJ> <Field> <Value>` line to the test world.
fn apply_input(w: &World, line: &str) {
    let mut fields = line.split_whitespace();
    assert_eq!(fields.next(), Some("in"), "input line: {line}");
    let object = fields.next().unwrap();
    let field = fields.next().unwrap();
    let value = fields.next().unwrap();
    unsafe {
        match (object, field) {
            ("LI", "NetMode") => (*w.at::<ALevelInfo>("LI")).net_mode = value.parse().unwrap(),
            ("LEV", "Ticked") => (*w.at::<ULevel>("LEV")).ticked = value.parse().unwrap(),
            ("LEV", "TimeSeconds") => {
                (*w.at::<ULevel>("LEV")).time_seconds = value.parse::<i64>().unwrap().to_le_bytes()
            }
            ("LEV", "bInSpecialPauseMode") => {
                (*w.at::<ULevel>("LEV")).b_in_special_pause_mode = value.parse().unwrap()
            }
            ("MODEL", "Zones0.LastRenderTime") => set_zone_lrt(0, value),
            ("MODEL", "Zones1.LastRenderTime") => set_zone_lrt(1, value),
            (actor, "Class") => set_class(w.get(actor), value),
            _ => apply_actor_input(w, object, field, value),
        }
    }
}

/// Applies an actor-field `in` line. Caller guarantees an unsafe context.
fn apply_actor_input(w: &World, object: &str, field: &str, value: &str) {
    let a = w.at::<AActor>(object);
    let pp = w.at::<APlayerPawn>(object);
    unsafe {
        match field {
            "Owner" => (*a).owner = w.ptr(value),
            "TickParent" => (*a).tick_parent = w.ptr(value),
            "TickParent2" => (*a).tick_parent2 = w.ptr(value),
            "Mesh" => (*a).mesh = w.ptr(value),
            "Player" => (*pp).player = w.ptr(value),
            "Weapon" => (*pp).base.weapon = w.ptr(value),
            "Physics" => (*a).physics = value.parse().unwrap(),
            "Role" => (*a).role = value.parse().unwrap(),
            "RemoteRole" => (*a).remote_role = value.parse().unwrap(),
            "AnimBone" => (*a).anim_bone = value.parse().unwrap(),
            "ZoneNumber" => (*a).region.zone_number = value.parse().unwrap(),
            "TweenRate" => (*a).tween_rate = f32_hex(value),
            "TweenAlpha" => (*a).tween_alpha = f32_hex(value),
            "AnimRate" => (*a).anim_rate = f32_hex(value),
            "AnimFrame" => (*a).anim_frame = f32_hex(value),
            "AnimLast" => (*a).anim_last = f32_hex(value),
            "TimerRate" => (*a).timer_rate = f32_hex(value),
            "TimerCounter" => (*a).timer_counter = f32_hex(value),
            "bTicked" => (*a).set_b_ticked(value == "1"),
            "bCanMoveInSpecialPause" => (*a).set_b_can_move_in_special_pause(value == "1"),
            "bStasis" => (*a).set_b_stasis(value == "1"),
            "bForceStasis" => (*a).set_b_force_stasis(value == "1"),
            "bAnimTransient" => (*a).set_b_anim_transient(value == "1"),
            "bTimerLoop" => (*a).set_b_timer_loop(value == "1"),
            "bIsPawn" => (*a).set_b_is_pawn(value == "1"),
            other => panic!("unexpected fixture input {object} {other}"),
        }
    }
}

/// Snapshot rows `(object, field, value)` in harness order.
fn snapshot(w: &World) -> Vec<(&'static str, &'static str, String)> {
    let mut rows = Vec::new();
    for name in SNAPSHOT_OBJECTS {
        let a = w.at::<AActor>(name);
        let mut fields: Vec<(&'static str, String)> = Vec::new();
        unsafe {
            fields.push(("Physics", (*a).physics.to_string()));
            fields.push(("Role", (*a).role.to_string()));
            fields.push(("RemoteRole", (*a).remote_role.to_string()));
            fields.push(("AnimBone", (*a).anim_bone.to_string()));
            fields.push(("ZoneNumber", (*a).region.zone_number.to_string()));
            fields.push(("TweenRate", fbits((*a).tween_rate)));
            fields.push(("TweenAlpha", fbits((*a).tween_alpha)));
            fields.push(("AnimRate", fbits((*a).anim_rate)));
            fields.push(("AnimFrame", fbits((*a).anim_frame)));
            fields.push(("AnimLast", fbits((*a).anim_last)));
            fields.push(("TimerRate", fbits((*a).timer_rate)));
            fields.push(("TimerCounter", fbits((*a).timer_counter)));
            fields.push(("LifeSpan", fbits((*a).life_span)));
            fields.push(("bTicked", flag((*a).b_ticked())));
            fields.push((
                "bCanMoveInSpecialPause",
                flag((*a).b_can_move_in_special_pause()),
            ));
            fields.push(("bStasis", flag((*a).b_stasis())));
            fields.push(("bForceStasis", flag((*a).b_force_stasis())));
            fields.push(("bAnimTransient", flag((*a).b_anim_transient())));
            fields.push(("bTimerLoop", flag((*a).b_timer_loop())));
            fields.push(("bIsPawn", flag((*a).b_is_pawn())));
        }
        rows.extend(
            fields
                .into_iter()
                .map(|(field, value)| (name, field, value)),
        );
    }
    rows
}

fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key} in {text}"))
}

fn parse_tick_cases(fixture: &str) -> Vec<TickCase<'_>> {
    let mut cases = Vec::new();
    let mut active: Option<TickCase<'_>> = None;
    for line in fixture.lines() {
        if let Some(header) = line.strip_prefix("case ") {
            if let Some(case) = active.take() {
                cases.push(case);
            }
            let mut fields = header.split_whitespace();
            if fields.next() == Some("tick") {
                active = Some(TickCase {
                    index: fields.next().unwrap().parse().unwrap(),
                    editor: 0,
                    knobs: Vec::new(),
                    inputs: Vec::new(),
                    call: "",
                    records: Vec::new(),
                    result: "",
                    outputs: Vec::new(),
                });
            }
        } else if let Some(case) = active.as_mut() {
            if let Some(value) = line.strip_prefix("editor ") {
                case.editor = value.parse().unwrap();
            } else if let Some(row) = line.strip_prefix("knob ") {
                let mut fields = row.split_whitespace();
                let name = fields.next().unwrap();
                let values: Vec<i64> = fields.map(|v| v.parse().unwrap()).collect();
                assert_eq!(values.len(), KNOB_COUNT, "knob row {name}");
                let mut row = [0i64; KNOB_COUNT];
                row.copy_from_slice(&values);
                case.knobs.push((name, row));
            } else if line.starts_with("in ") {
                case.inputs.push(line);
            } else if let Some(call) = line.strip_prefix("call tick ") {
                case.call = call;
            } else if let Some(result) = line.strip_prefix("res tick ") {
                case.result = result;
            } else if let Some(output) = line.strip_prefix("out ") {
                case.outputs.push(output);
            } else if !case.call.is_empty() && case.result.is_empty() {
                case.records.push(line);
            }
        }
    }
    if let Some(case) = active {
        cases.push(case);
    }
    cases
}

fn reset_state(case: &TickCase<'_>) {
    TRACE.with(|t| t.borrow_mut().clear());
    REGS.with(|r| r.borrow_mut().clear());
    CLASSES.with(|c| c.borrow_mut().clear());
    KNOBS.with(|k| {
        *k.borrow_mut() = case
            .knobs
            .iter()
            .map(|(name, row)| (name.to_string(), *row))
            .collect();
    });
    ZONE_LRT.with(|z| z.set([0.0; 2]));
    EDITOR.with(|e| e.set(case.editor));
    TOKEN.with(|t| t.set(0));
    REL.with(|r| r.set(c"direct".as_ptr()));
    ROOT.with(|r| r.set(c"dynamic".as_ptr()));
}

fn replay(case: &TickCase<'_>) {
    reset_state(case);
    let w = World::new();
    for input in case.inputs.iter().copied() {
        apply_input(&w, input);
    }

    let call = case.call;
    let dt = u32::from_str_radix(field(call, "dt="), 16).unwrap();
    let mode: i32 = field(call, "mode=").parse().unwrap();
    let editor: i32 = field(call, "editor=").parse().unwrap();
    assert_eq!(editor, case.editor, "case {} editor", case.index);
    let actor_name = field(call, "actor=");
    let actor = w.ptr(actor_name);

    let before = snapshot(&w);
    let svc = tick_services();
    SVC.with(|s| s.set(&svc));
    let ret = unsafe { actor_tick(&svc, actor, f32::from_bits(dt), mode) };
    let after = snapshot(&w);

    let (b_ticked, loc) = unsafe {
        let a = w.at::<AActor>(actor_name);
        (
            (*a).b_ticked(),
            ((*a).location.x, (*a).location.y, (*a).location.z),
        )
    };
    let result = format!(
        "ret={ret} bTicked={} loc={:.3},{:.3},{:.3}",
        u8::from(b_ticked),
        loc.0,
        loc.1,
        loc.2
    );
    assert_eq!(result, case.result, "case {} result", case.index);

    let records = TRACE.with(|t| t.borrow().clone());
    assert_eq!(
        records, case.records,
        "case {} hook/event/trace records",
        case.index
    );

    let mut outputs = Vec::new();
    for (pre, post) in before.iter().zip(after.iter()) {
        if pre.2 != post.2 {
            outputs.push(format!("{} {} {}", post.0, post.1, post.2));
        }
    }
    assert_eq!(outputs, case.outputs, "case {} out", case.index);
}

#[test]
fn lev_tic_tick_fixture_replays_records_results_and_outputs() {
    const FIXTURE: &str = include_str!("fixtures/lev_tic.txt");
    let cases = parse_tick_cases(FIXTURE);
    assert_eq!(cases.len(), 60, "tick family case count");
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(case.index, index);
        replay(case);
    }
}

// Checked adapters for the existing deterministic fixture callbacks.
unsafe extern "C" fn checked_trace_event_tick_trace_event_tick(arg0: u64) -> i32 {
    unsafe {
        trace_event_tick(arg0);
    }
    0
}
unsafe extern "C" fn checked_check_enabled_check_enabled(out: *mut i32) -> i32 {
    unsafe {
        out.write(check_enabled());
    }
    0
}
unsafe extern "C" fn checked_actor_is_a_actor_is_a(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(actor_is_a(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_event_actor_event(
    arg0: *mut c_void,
    arg1: i32,
    arg2: f32,
) -> i32 {
    unsafe {
        actor_event(arg0, arg1, arg2);
    }
    0
}
unsafe extern "C" fn checked_actor_is_probing_actor_is_probing(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(actor_is_probing(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_actor_process_state_actor_process_state(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        actor_process_state(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_actor_perform_physics_actor_perform_physics(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        actor_perform_physics(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_actor_move_smooth_actor_move_smooth(
    arg0: *mut c_void,
    arg1: *const f32,
) -> i32 {
    unsafe {
        actor_move_smooth(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_actor_creature_trace_tick_actor_creature_trace_tick(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        actor_creature_trace_tick(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_player_read_input_player_read_input(
    arg0: *mut c_void,
    arg1: f32,
) -> i32 {
    unsafe {
        player_read_input(arg0, arg1);
    }
    0
}
unsafe extern "C" fn checked_pawn_begin_modern_tps_pawn_begin_modern_tps(
    arg0: *mut c_void,
    arg1: *mut f32,
    arg2: *mut f32,
    arg3: *mut f32,
    arg4: *mut f32,
    out: *mut i32,
) -> i32 {
    unsafe {
        out.write(pawn_begin_modern_tps(arg0, arg1, arg2, arg3, arg4));
    }
    0
}
unsafe extern "C" fn checked_pawn_finish_modern_tps_pawn_finish_modern_tps(
    arg0: *mut c_void,
    arg1: i32,
    arg2: f32,
    arg3: f32,
    arg4: f32,
    arg5: f32,
    arg6: f32,
) -> i32 {
    unsafe {
        pawn_finish_modern_tps(arg0, arg1, arg2, arg3, arg4, arg5, arg6);
    }
    0
}
unsafe extern "C" fn checked_pawn_apply_modern_tps_pawn_apply_modern_tps(arg0: *mut c_void) -> i32 {
    unsafe {
        pawn_apply_modern_tps(arg0);
    }
    0
}
unsafe extern "C" fn checked_level_far_move_actor_level_far_move_actor(
    arg0: *mut c_void,
    arg1: *mut c_void,
    arg2: *const f32,
) -> i32 {
    unsafe {
        level_far_move_actor(arg0, arg1, arg2);
    }
    0
}
unsafe extern "C" fn checked_level_zone_time_since_render_level_zone_time_since_render(
    arg0: *mut c_void,
    arg1: i32,
    out: *mut f32,
) -> i32 {
    unsafe {
        out.write(level_zone_time_since_render(arg0, arg1));
    }
    0
}
unsafe extern "C" fn checked_level_is_editor_level_is_editor(out: *mut i32) -> i32 {
    unsafe {
        out.write(level_is_editor());
    }
    0
}
unsafe extern "C" fn fixture_guard_scope(
    _scope: i32,
    context: *mut c_void,
    continuation: unsafe extern "C" fn(*mut c_void) -> i32,
) -> i32 {
    unsafe { continuation(context) }
}
unsafe fn actor_tick(
    svc: &Hp2LevTicServices,
    actor: *mut c_void,
    delta: f32,
    tick_type: i32,
) -> i32 {
    let mut dispatch = hp2_engine::lev_tic::Hp2TickDispatch {
        token: u64::MAX,
        skip_reason: c"skip_no_event".as_ptr(),
        event_tick_dispatched: 0,
    };
    if unsafe { trace_enabled() } != 0 {
        dispatch.token =
            unsafe { trace_begin_dispatch(actor, trace_relation_get(), trace_root_get()) };
    }
    let result = unsafe { checked_actor_tick(svc, actor, delta, tick_type, &mut dispatch) };
    if dispatch.token != u64::MAX {
        unsafe {
            trace_end_dispatch(
                dispatch.token,
                if dispatch.event_tick_dispatched != 0 {
                    c"dispatched".as_ptr()
                } else {
                    dispatch.skip_reason
                },
            )
        };
    }
    result.unwrap()
}
unsafe extern "C" fn fixture_tick_actor_scoped(
    actor: *mut c_void,
    delta: f32,
    tick_type: i32,
    relation: *const c_char,
    root: *const c_char,
    out: *mut i32,
) -> i32 {
    unsafe {
        let previous_relation = if relation.is_null() {
            None
        } else {
            Some(trace_relation_set(relation))
        };
        let previous_root = trace_root_set(root);
        out.write(tick_actor(actor, delta, tick_type));
        trace_root_set(previous_root);
        if let Some(previous) = previous_relation {
            trace_relation_set(previous);
        }
    }
    0
}
