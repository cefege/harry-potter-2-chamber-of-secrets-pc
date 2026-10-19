//! Replays `fixtures/pawn_moves.txt` against `hp2_engine::pawn::{walk_move,
//! fly_move, swim_move}`.
//!
//! The fixture is generated from the ORIGINAL `UnPawn.o` by
//! `Build/pawn_fixture_harness/gen_pawn.sh`. Each record carries the subject
//! pawn inputs, the scripted results of every `ULevel::MoveActor`,
//! `ULevel::FarMoveActor` and `APawn::findWaterLine` call the original made
//! (in order), and the original's return value and final location. The replay
//! services check each call's arguments bit-for-bit and feed back the recorded
//! results, so the Rust body must issue the same call sequence.
//!
//! Record layout (hex words):
//! `fn loc3 rot3 maxStep zoneGravity3 water goal delta3 thr adj | nLog log... | ret finalLoc3`

use std::cell::RefCell;
use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, FRotator};
use hp2_engine::pawn::{Hp2PawnServices, fly_move, swim_move, walk_move};

const FIXTURE: &str = include_str!("fixtures/pawn_moves.txt");

const LEVEL: usize = 0x1000;
/// Stand-in actor for every non-goal hit.
const OTHER_ACTOR: usize = 0x3000;

/// Replay state for the current case: recorded call log and cursor.
struct Replay {
    words: Vec<u32>,
    pos: usize,
    goal: *mut AActor,
}

thread_local! {
    static REPLAY: RefCell<Replay> = const { RefCell::new(Replay { words: Vec::new(), pos: 0, goal: std::ptr::null_mut() }) };
}

fn next_word() -> u32 {
    REPLAY.with(|r| {
        let mut r = r.borrow_mut();
        let w = r.words[r.pos];
        r.pos += 1;
        w
    })
}

fn next_f() -> f32 {
    f32::from_bits(next_word())
}

fn next_vec() -> FVector {
    FVector {
        x: next_f(),
        y: next_f(),
        z: next_f(),
    }
}

fn bits(v: FVector) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn expect_kind(kind: u32) {
    assert_eq!(next_word(), kind, "service call kind/order");
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
    let delta = unsafe { *delta.cast::<FVector>() };
    let rot = unsafe { *rot.cast::<FRotator>() };
    let hit = hit.cast::<FCheckResult>();
    let want = bits(delta);
    for w in want {
        assert_eq!(next_word(), w, "MoveActor delta bits");
    }
    assert_eq!(next_word(), rot.pitch as u32);
    assert_eq!(next_word(), rot.yaw as u32);
    assert_eq!(next_word(), rot.roll as u32);
    // Test=1, IgnorePawns=1, bIgnoreBases=0, bNoFail=0 pinned by the C++ shell.
    assert_eq!(
        [next_word(), next_word(), next_word(), next_word()],
        [1, 1, 0, 0]
    );
    let ret = next_word() as i32;
    let time = next_f();
    let normal = next_vec();
    let is_goal = next_word() != 0;
    let new_loc = next_vec();
    let a = actor.cast::<AActor>();
    unsafe {
        (*a).location = new_loc;
        (*hit).time = time;
        (*hit).normal = normal;
        (*hit).actor = if is_goal {
            REPLAY.with(|r| r.borrow().goal).cast()
        } else {
            OTHER_ACTOR as *mut c_void
        };
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
    for w in bits(dest) {
        assert_eq!(next_word(), w, "FarMoveActor dest bits");
    }
    // Test=1, noCheck=1 pinned by the C++ shell.
    assert_eq!([next_word(), next_word()], [1, 1]);
    let ret = next_word() as i32;
    unsafe { (*actor.cast::<AActor>()).location = dest };
    ret
}

unsafe extern "C" fn find_water_line(pawn: *mut c_void, start: *const c_void, end: *mut c_void) {
    expect_kind(2);
    let _ = pawn;
    let start = unsafe { *start.cast::<FVector>() };
    let end = end.cast::<FVector>();
    for w in bits(start) {
        assert_eq!(next_word(), w, "findWaterLine start bits");
    }
    let end_in = unsafe { *end };
    for w in bits(end_in) {
        assert_eq!(next_word(), w, "findWaterLine end-in bits");
    }
    unsafe { *end = next_vec() };
}

fn services() -> Hp2PawnServices {
    Hp2PawnServices {
        move_actor,
        far_move_actor,
        find_water_line,
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

#[test]
fn pawn_move_fixtures_replay_bit_exact() {
    let mut per_fn = [0usize; 3];
    // Return codes seen per (function, return), so the fixture must reach every exit path.
    let mut seen = std::collections::BTreeMap::<(usize, i32), usize>::new();
    let mut total = 0usize;
    for line in FIXTURE.lines().filter(|l| !l.trim().is_empty()) {
        let toks: Vec<&str> = line.split_whitespace().filter(|t| *t != "|").collect();
        let f = hex(toks[0]) as usize;
        let loc = fvec(&toks[1..4]);
        let rot = FRotator {
            pitch: hex(toks[4]) as i32,
            yaw: hex(toks[5]) as i32,
            roll: hex(toks[6]) as i32,
        };
        let max_step = f32::from_bits(hex(toks[7]));
        let zg = fvec(&toks[8..11]);
        let water = hex(toks[11]) != 0;
        let goal_on = hex(toks[12]) != 0;
        let delta = fvec(&toks[13..16]);
        let thr = f32::from_bits(hex(toks[16]));
        let adj = hex(toks[17]) as i32;
        let n = hex(toks[18]) as usize;
        let log: Vec<u32> = toks[19..19 + n].iter().map(|t| hex(t)).collect();
        let ret_want = hex(toks[19 + n]) as i32;
        let end_loc = fvec(&toks[20 + n..23 + n]);
        assert_eq!(toks.len(), 23 + n, "record width");

        let mut zone: AZoneInfo = unsafe { std::mem::zeroed() };
        zone.zone_gravity = zg;
        zone.set_b_water_zone(water);
        let mut pawn: APawn = unsafe { std::mem::zeroed() };
        pawn.base.location = loc;
        pawn.base.rotation = rot;
        pawn.base.x_level = LEVEL as *mut c_void;
        pawn.base.region.zone = (&mut zone as *mut AZoneInfo).cast();
        pawn.max_step_height = max_step;
        let mut goal_obj: AActor = unsafe { std::mem::zeroed() };
        let goal = if goal_on {
            &mut goal_obj as *mut AActor
        } else {
            std::ptr::null_mut()
        };
        REPLAY.with(|r| {
            *r.borrow_mut() = Replay {
                words: log,
                pos: 0,
                goal,
            };
        });
        let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
        hit.time = 1.0;
        let svc = services();
        let ret = unsafe {
            match f {
                0 => walk_move(&svc, &mut pawn, delta, &mut hit, goal, thr, adj),
                1 => fly_move(&svc, &mut pawn, delta, goal, thr, adj),
                2 => swim_move(&svc, &mut pawn, delta, goal, thr, adj),
                _ => panic!("bad fn id {f}"),
            }
        };
        let consumed = REPLAY.with(|r| r.borrow().pos);
        assert_eq!(consumed, n, "call count mismatch in case {total}");
        assert_eq!(ret, ret_want, "return value in case {total} (fn {f})");
        assert_eq!(
            bits(pawn.base.location),
            bits(end_loc),
            "final location in case {total} (fn {f})"
        );
        per_fn[f] += 1;
        *seen.entry((f, ret)).or_default() += 1;
        total += 1;
    }
    assert_eq!(per_fn, [800, 800, 800]);
    for ret in [-1, 0, 1, 5] {
        assert!(seen.contains_key(&(0, ret)), "walk never returned {ret}");
    }
    for f in [1, 2] {
        for ret in [0, 1, 5] {
            assert!(seen.contains_key(&(f, ret)), "fn {f} never returned {ret}");
        }
    }
    assert_eq!(total, 2400);
}
