//! Replays `fixtures/pawn_reach.txt` against `hp2_engine::pawn_reach::*`.
//!
//! The fixture is generated from the ORIGINAL `UnPawn.o` by
//! `Build/pawn_reach_fixture_harness/gen_pawn_reach.sh`. Each record carries the scene
//! (pawn, zones, other and goal actors, the editor flag and the call arguments), the
//! scripted results of every service call the original made (in order), and the original's
//! return value, final pawn location and velocity, and output vector. The replay services
//! check each call's arguments bit-for-bit and feed back the recorded results, so the Rust
//! body must issue the same call sequence. `IsA` and `GIsEditor` are not logged: `is_a` is
//! answered from the scene's class indices and `is_editor` from the scene word.
//!
//! Record layout (hex words):
//! `fn scene70 | nLog log... | ret loc3 vel3 out3`

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APawn, AWarpZoneMarker, AZoneInfo, FCheckResult, FPointRegion, FRotator,
};
use hp2_engine::pawn::Hp2PawnServices;
use hp2_engine::pawn_reach::{
    Hp2PawnReachServices, actor_reachable, find_best_jump, find_jump_up, fly_reachable,
    jump_landing, jump_reachable, point_reachable, reachable, suggest_jump_velocity,
    swim_reachable, walk_reachable,
};

const _: () = assert!(std::mem::size_of::<APawn>() >= std::mem::size_of::<AWarpZoneMarker>());

const FIXTURE: &str = include_str!("fixtures/pawn_reach.txt");

/// `ULevel` stand-in; the original passes `GetLevel()` (= `XLevel`) to every service.
const LEVEL: usize = 0x1000;
/// Words between the fn id and the first `|` (see `run_case` in the generator).
const SCENE_WORDS: usize = 70;
/// `TRACE_VisBlocking`.
const TRACE_VIS_BLOCKING: u32 = 0x06;
/// Number of functions the fixture covers.
const FN_COUNT: usize = 11;
const CASES_PER_FN: usize = 200;
/// Log entry kinds, in fixture order.
const KIND_COUNT: usize = 6;

/// Replay state for the current case: recorded call log, cursor and the scene objects.
struct Case {
    words: Vec<u32>,
    pos: usize,
    pawn: *mut c_void,
    other: *mut c_void,
    /// Goal actor passed to the reach call, or null when the scene has no goal.
    goal: *mut c_void,
    /// Goal object, always present (used by `is_a` even when not passed as the goal).
    goal_obj: *mut c_void,
    zones: [*mut c_void; 3],
    other_class: u32,
    goal_class: u32,
    editor: bool,
}

const EMPTY: Case = Case {
    words: Vec::new(),
    pos: 0,
    pawn: null_mut(),
    other: null_mut(),
    goal: null_mut(),
    goal_obj: null_mut(),
    zones: [null_mut(); 3],
    other_class: 4,
    goal_class: 4,
    editor: false,
};

thread_local! {
    static CASE: RefCell<Case> = const { RefCell::new(EMPTY) };
    static KIND_HITS: RefCell<[usize; KIND_COUNT]> = const { RefCell::new([0; KIND_COUNT]) };
}

fn take() -> u32 {
    CASE.with(|c| {
        let mut c = c.borrow_mut();
        let w = c.words[c.pos];
        c.pos += 1;
        w
    })
}

fn take_f() -> f32 {
    f32::from_bits(take())
}

fn take_vec() -> FVector {
    FVector {
        x: take_f(),
        y: take_f(),
        z: take_f(),
    }
}

fn bits(v: FVector) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn expect_vec(want: FVector, what: &str) {
    for w in bits(want) {
        assert_eq!(take(), w, "{what} bits");
    }
}

fn expect_kind(kind: u32) {
    assert_eq!(take(), kind, "service call kind/order");
    KIND_HITS.with(|k| k.borrow_mut()[kind as usize] += 1);
}

fn hit_actor(is_goal: bool) -> *mut c_void {
    CASE.with(|c| {
        let c = c.borrow();
        if is_goal { c.goal } else { c.other }
    })
}

unsafe extern "C" fn move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    delta: *const c_void,
    rot: *const c_void,
    hit: *mut c_void,
) -> i32 {
    expect_kind(0);
    assert_eq!(level as usize, LEVEL);
    let (delta, rot) = unsafe { (*delta.cast::<FVector>(), *rot.cast::<FRotator>()) };
    expect_vec(delta, "MoveActor delta");
    assert_eq!(take(), rot.pitch as u32);
    assert_eq!(take(), rot.yaw as u32);
    assert_eq!(take(), rot.roll as u32);
    // Test=1, IgnorePawns=1, bIgnoreBases=0, bNoFail=0 pinned by the C++ shell.
    assert_eq!([take(), take(), take(), take()], [1, 1, 0, 0]);
    let ret = take() as i32;
    let time = take_f();
    let normal = take_vec();
    let is_goal = take() != 0;
    let new_loc = take_vec();
    unsafe {
        (*actor.cast::<AActor>()).location = new_loc;
        let hit = hit.cast::<FCheckResult>();
        (*hit).time = time;
        (*hit).normal = normal;
        (*hit).actor = hit_actor(is_goal);
    }
    ret
}

unsafe extern "C" fn far_move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
) -> i32 {
    expect_kind(1);
    assert_eq!(level as usize, LEVEL);
    let dest = unsafe { *dest.cast::<FVector>() };
    expect_vec(dest, "FarMoveActor dest");
    // Test=1, noCheck=1 pinned by the C++ shell.
    assert_eq!([take(), take()], [1, 1]);
    let ret = take() as i32;
    unsafe { (*actor.cast::<AActor>()).location = dest };
    ret
}

unsafe extern "C" fn find_water_line(pawn: *mut c_void, start: *const c_void, end: *mut c_void) {
    expect_kind(2);
    let _ = pawn;
    let start = unsafe { *start.cast::<FVector>() };
    let end = end.cast::<FVector>();
    expect_vec(start, "findWaterLine start");
    let end_in = unsafe { *end };
    expect_vec(end_in, "findWaterLine end-in");
    unsafe { *end = take_vec() };
}

unsafe extern "C" fn far_move_actor_flags(
    level: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    test: i32,
    no_check: i32,
) -> i32 {
    expect_kind(1);
    assert_eq!(level as usize, LEVEL);
    let dest = unsafe { *dest.cast::<FVector>() };
    expect_vec(dest, "FarMoveActor dest");
    assert_eq!(take(), test as u32, "FarMoveActor test");
    assert_eq!(take(), no_check as u32, "FarMoveActor noCheck");
    let ret = take() as i32;
    unsafe { (*actor.cast::<AActor>()).location = dest };
    ret
}

unsafe extern "C" fn single_line_check(
    level: *mut c_void,
    source: *mut c_void,
    hit: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) {
    expect_kind(3);
    let _ = source;
    assert_eq!(level as usize, LEVEL);
    let (end, start) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    expect_vec(end, "SingleLineCheck end");
    expect_vec(start, "SingleLineCheck start");
    assert_eq!(take(), TRACE_VIS_BLOCKING, "SingleLineCheck trace flags");
    // Extent is the default zero vector; NodeFlags is 0.
    assert_eq!(
        [take(), take(), take()],
        [0, 0, 0],
        "SingleLineCheck extent"
    );
    assert_eq!(take(), 0, "SingleLineCheck nodeFlags");
    // The original ignores the return value (`GetLevel()->SingleLineCheck(...)` is a statement).
    let _ret = take();
    let time = take_f();
    let normal = take_vec();
    let actor_kind = take();
    let actor = match actor_kind {
        0 => null_mut(),
        1 => {
            let g = hit_actor(true);
            assert!(!g.is_null(), "goal hit without a goal");
            g
        }
        2 => hit_actor(false),
        _ => panic!("bad SingleLineCheck actor kind {actor_kind}"),
    };
    unsafe {
        let hit = hit.cast::<FCheckResult>();
        (*hit).time = time;
        (*hit).normal = normal;
        (*hit).actor = actor;
    }
}

unsafe extern "C" fn point_region(
    level: *mut c_void,
    actor: *mut c_void,
    point: *const c_void,
    out: *mut c_void,
) {
    expect_kind(4);
    let _ = actor;
    assert_eq!(level as usize, LEVEL);
    let point = unsafe { *point.cast::<FVector>() };
    expect_vec(point, "PointRegion point");
    let k = take() as usize;
    assert!(k < 3, "PointRegion zone index {k}");
    let zone = CASE.with(|c| c.borrow().zones[k]);
    let mut region: FPointRegion = unsafe { std::mem::zeroed() };
    region.zone = zone;
    unsafe { *out.cast::<FPointRegion>() = region };
}

unsafe extern "C" fn fast_line_check(
    level: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) -> i32 {
    expect_kind(5);
    assert_eq!(level as usize, LEVEL);
    let (end, start) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    expect_vec(end, "FastLineCheck end");
    expect_vec(start, "FastLineCheck start");
    take() as i32
}

unsafe extern "C" fn is_a(object: *mut c_void, kind: i32) -> i32 {
    assert!((0..=3).contains(&kind), "IsA kind {kind}");
    CASE.with(|c| {
        let c = c.borrow();
        // Scene class indices: 0 APawn, 1 AInventory, 2 ATrigger, 3 AWarpZoneMarker, 4 none.
        let class = if object == c.pawn {
            0
        } else if object == c.other {
            c.other_class
        } else if object == c.goal_obj {
            c.goal_class
        } else {
            4
        };
        i32::from(class == kind as u32)
    })
}

unsafe extern "C" fn is_editor() -> i32 {
    CASE.with(|c| i32::from(c.borrow().editor))
}

fn services() -> Hp2PawnReachServices {
    Hp2PawnReachServices {
        pawn: Hp2PawnServices {
            move_actor,
            far_move_actor,
            find_water_line,
        },
        far_move_actor_flags,
        single_line_check,
        point_region,
        fast_line_check,
        is_a,
        is_editor,
    }
}

fn hex(tok: &str) -> u32 {
    u32::from_str_radix(tok, 16).unwrap_or_else(|_| panic!("bad hex word {tok}"))
}

fn fvec(t: &[&str]) -> FVector {
    FVector {
        x: f32::from_bits(hex(t[0])),
        y: f32::from_bits(hex(t[1])),
        z: f32::from_bits(hex(t[2])),
    }
}

/// Sequential reader over the scene words, in the generator's `run_case` order.
struct Scene {
    w: Vec<u32>,
    pos: usize,
}

impl Scene {
    fn u(&mut self) -> u32 {
        let x = self.w[self.pos];
        self.pos += 1;
        x
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn v(&mut self) -> FVector {
        FVector {
            x: self.f(),
            y: self.f(),
            z: self.f(),
        }
    }
}

#[test]
fn pawn_reach_fixture_replay_bit_exact() {
    let mut per_fn = [0usize; FN_COUNT];
    // Return codes seen per (function, return): every listed return class must appear.
    let mut seen = BTreeMap::<(usize, i32), usize>::new();
    let mut total = 0usize;
    for line in FIXTURE.lines().filter(|l| !l.trim().is_empty()) {
        let toks: Vec<&str> = line.split_whitespace().filter(|t| *t != "|").collect();
        let f = hex(toks[0]) as usize;
        let mut scene = Scene {
            w: toks[1..=SCENE_WORDS].iter().map(|t| hex(t)).collect(),
            pos: 0,
        };
        let n = hex(toks[1 + SCENE_WORDS]) as usize;
        let log_at = 2 + SCENE_WORDS;
        let log: Vec<u32> = toks[log_at..log_at + n].iter().map(|t| hex(t)).collect();
        let ret_at = log_at + n;
        assert_eq!(toks.len(), ret_at + 10, "record width in case {total}");
        let ret_want = hex(toks[ret_at]) as i32;
        let loc_want = fvec(&toks[ret_at + 1..ret_at + 4]);
        let vel_want = fvec(&toks[ret_at + 4..ret_at + 7]);
        let out_want = fvec(&toks[ret_at + 7..ret_at + 10]);

        // Scene: pawn.
        let p_loc = scene.v();
        let p_vel = scene.v();
        let p_rot = FRotator {
            pitch: scene.i(),
            yaw: scene.i(),
            roll: scene.i(),
        };
        let p_cr = scene.f();
        let p_ch = scene.f();
        let p_phys = scene.u() as u8;
        let p_cj = scene.u() != 0;
        let p_cw = scene.u() != 0;
        let p_cs = scene.u() != 0;
        let p_cf = scene.u() != 0;
        let p_msh = scene.f();
        let p_gs = scene.f();
        let p_jz = scene.f();
        let p_mr = scene.f();
        let p_bev = scene.f();
        let rz = scene.u() as usize;
        let fz = scene.u() as usize;
        let zn = scene.u() as u8;
        // Scene: zones.
        let mut zone_vals = Vec::with_capacity(3);
        for _ in 0..3 {
            let g = scene.v();
            let water = scene.u() != 0;
            let pain = scene.u() != 0;
            let friction = scene.f();
            zone_vals.push((g, water, pain, friction));
        }
        // Scene: other actor.
        let o_loc = scene.v();
        let o_cr = scene.f();
        let o_ch = scene.f();
        let o_block = scene.u() != 0;
        let o_cls = scene.u();
        let o_zone = scene.u() as usize;
        // Scene: goal actor.
        let goal_on = scene.u() != 0;
        let g_loc = scene.v();
        let g_cr = scene.f();
        let g_ch = scene.f();
        let g_block = scene.u() != 0;
        let mw = scene.i();
        let g_cls = scene.u();
        let g_zone = scene.u() as usize;
        let editor = scene.u() != 0;
        // Scene: arguments.
        let v1 = scene.v();
        let v2 = scene.v();
        let thr = scene.f();
        let rf = scene.i();
        let flag = scene.i();
        assert_eq!(scene.pos, SCENE_WORDS, "scene width in case {total}");

        // Zones (AZoneInfo, zeroed except the scene fields).
        let mut zones: [AZoneInfo; 3] = unsafe { std::mem::zeroed() };
        for (z, (g, water, pain, friction)) in zones.iter_mut().zip(zone_vals) {
            z.zone_gravity = g;
            z.set_b_water_zone(water);
            z.set_b_pain_zone(pain);
            z.zone_fluid_friction = friction;
        }
        let zone_ptrs: [*mut c_void; 3] =
            std::array::from_fn(|i| (&mut zones[i] as *mut AZoneInfo).cast());

        // Pawn.
        let mut pawn: APawn = unsafe { std::mem::zeroed() };
        pawn.base.location = p_loc;
        pawn.base.velocity = p_vel;
        pawn.base.rotation = p_rot;
        pawn.base.collision_radius = p_cr;
        pawn.base.collision_height = p_ch;
        pawn.base.physics = p_phys;
        pawn.set_b_can_jump(p_cj);
        pawn.set_b_can_walk(p_cw);
        pawn.set_b_can_swim(p_cs);
        pawn.set_b_can_fly(p_cf);
        pawn.max_step_height = p_msh;
        pawn.ground_speed = p_gs;
        pawn.jump_z = p_jz;
        pawn.melee_range = p_mr;
        pawn.base_eye_height = p_bev;
        pawn.base.x_level = LEVEL as *mut c_void;
        pawn.base.region.zone_number = zn;
        pawn.base.region.zone = zone_ptrs[rz];
        pawn.foot_region.zone = zone_ptrs[fz];

        // Other actor (APawn-sized so a pawn-class other can read its foot region).
        let mut other: APawn = unsafe { std::mem::zeroed() };
        other.base.location = o_loc;
        other.base.collision_radius = o_cr;
        other.base.collision_height = o_ch;
        other.base.set_b_block_actors(o_block);
        other.base.region.zone = zone_ptrs[o_zone];
        other.foot_region.zone = zone_ptrs[o_zone];

        // Goal object (APawn-sized, viewed as AWarpZoneMarker for markedWarpZone).
        let mut goal_obj: APawn = unsafe { std::mem::zeroed() };
        goal_obj.base.location = g_loc;
        goal_obj.base.collision_radius = g_cr;
        goal_obj.base.collision_height = g_ch;
        goal_obj.base.set_b_block_actors(g_block);
        goal_obj.base.region.zone = zone_ptrs[g_zone];
        goal_obj.foot_region.zone = zone_ptrs[g_zone];
        let marked = if mw < 0 {
            null_mut()
        } else {
            zone_ptrs[mw as usize]
        };
        unsafe {
            (*(&mut goal_obj as *mut APawn).cast::<AWarpZoneMarker>()).marked_warp_zone = marked;
        }

        let pawn_p = &mut pawn as *mut APawn;
        let other_p = &mut other as *mut APawn as *mut AActor;
        let goal_p = &mut goal_obj as *mut APawn as *mut AActor;
        CASE.with(|c| {
            *c.borrow_mut() = Case {
                words: log,
                pos: 0,
                pawn: pawn_p.cast(),
                other: other_p.cast(),
                goal: if goal_on { goal_p.cast() } else { null_mut() },
                goal_obj: goal_p.cast(),
                zones: zone_ptrs,
                other_class: o_cls,
                goal_class: g_cls,
                editor,
            };
        });

        // Call the Rust body. `out` starts as v2 (landing / velocity in-out).
        let svc = services();
        let goal_arg = if goal_on { goal_p } else { null_mut() };
        let mut out = v2;
        let ret = unsafe {
            match f {
                0 => actor_reachable(&svc, pawn_p, other_p, flag),
                1 => point_reachable(&svc, pawn_p, v1, flag),
                2 => reachable(&svc, pawn_p, v1, thr, goal_arg),
                3 => walk_reachable(&svc, pawn_p, v1, thr, rf, goal_arg),
                4 => fly_reachable(&svc, pawn_p, v1, thr, rf, goal_arg),
                5 => swim_reachable(&svc, pawn_p, v1, thr, rf, goal_arg),
                6 => jump_reachable(&svc, pawn_p, v1, thr, rf, goal_arg),
                7 => {
                    jump_landing(&svc, pawn_p, v1, &mut out, flag);
                    0
                }
                8 => find_jump_up(&svc, pawn_p, v2),
                9 => {
                    suggest_jump_velocity(&svc, pawn_p, v1, &mut out);
                    0
                }
                10 => find_best_jump(&svc, pawn_p, v1, v2, &mut out, flag),
                _ => panic!("bad fn id {f}"),
            }
        };
        let consumed = CASE.with(|c| c.borrow().pos);
        assert_eq!(consumed, n, "call count mismatch in case {total} (fn {f})");
        assert_eq!(ret, ret_want, "return value in case {total} (fn {f})");
        assert_eq!(
            bits(pawn.base.location),
            bits(loc_want),
            "final location in case {total} (fn {f})"
        );
        assert_eq!(
            bits(pawn.base.velocity),
            bits(vel_want),
            "final velocity in case {total} (fn {f})"
        );
        assert_eq!(
            bits(out),
            bits(out_want),
            "output vector in case {total} (fn {f})"
        );
        per_fn[f] += 1;
        *seen.entry((f, ret)).or_default() += 1;
        total += 1;
    }
    assert_eq!(total, FN_COUNT * CASES_PER_FN);
    assert_eq!(per_fn, [CASES_PER_FN; FN_COUNT]);
    // Return classes each function must reach (from the generator's histogram).
    let want: [(usize, &[i32]); 11] = [
        (0, &[0, 1, 2, 3, 4, 9]),
        (1, &[0, 1, 2, 3, 4]),
        (2, &[0, 1, 2, 3, 4, 9]),
        (3, &[0, 1, 3, 4, 5, 7, 9, 11, 13, 15]),
        (4, &[0, 2, 3, 6, 7, 10, 11, 14, 15]),
        (5, &[0, 1, 4, 5, 6, 7, 12, 13, 14, 15]),
        (6, &[0, 4, 9, 11, 13, 15]),
        (7, &[0]),
        (8, &[0, 1, -1]),
        (9, &[0]),
        (10, &[0, 1]),
    ];
    for (f, rets) in want {
        for &r in rets {
            assert!(seen.contains_key(&(f, r)), "fn {f} never returned {r}");
        }
    }
    let kinds = KIND_HITS.with(|k| *k.borrow());
    for (k, hits) in kinds.iter().enumerate() {
        assert!(*hits > 0, "log kind {k} never replayed");
    }
}
