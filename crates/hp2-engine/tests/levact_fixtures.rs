//! Replays the original-C++ oracles from `Build/levact_fixture_harness/` against
//! `hp2_engine::levact`.
//!
//! - `fixtures/levact_rotation.txt`: the four `FindDeltaFromRotation*` helpers
//!   over 2000 seeded inputs each. Each record is
//!   `<op> <origin xyz> <point xyz> <i0> <i1> <i2> -> <out xyz>`.
//! - `fixtures/levact_checks.txt`: `SinglePointCheck`, `MultiPointCheck`,
//!   `SingleLineCheck`, and `MultiLineCheck` over 1500 seeded cases each.
//! - `fixtures/levact_actor_ops.txt`: direct original-C++ calls to `MoveActor`,
//!   `CheckEncroachment`, `FindSpot`, and `SetActorZone`, replayed with the
//!   fixture's actor world and ordered service-hook answers.
//!
//! Floats and ints are hex bit patterns, so every comparison is bit-exact.
//! Actor fields in check records are pool indices (`-1` NULL).

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APawn, FCheckResult, FPointRegion, FRotator, TArrayRaw, ULevel,
};
use hp2_engine::levact::{self, LevActServices, LevelCtx, LineQuery};

const FIXTURE: &str = include_str!("fixtures/levact_rotation.txt");
const CHECK_FIXTURE: &str = include_str!("fixtures/levact_checks.txt");
const ACTOR_OPS_FIXTURE: &str = include_str!("fixtures/levact_actor_ops.txt");

fn f(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).expect("hex float"))
}

fn i(hex: &str) -> i32 {
    u32::from_str_radix(hex, 16).expect("hex int") as i32
}

fn v(xs: &[&str]) -> FVector {
    FVector {
        x: f(xs[0]),
        y: f(xs[1]),
        z: f(xs[2]),
    }
}

#[test]
fn levact_rotation_fixtures_bit_exact() {
    let mut counts = [0usize; 4];
    for line in FIXTURE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), 14, "malformed record: {line}");
        assert_eq!(t[10], "->", "malformed record: {line}");
        let origin = v(&t[1..4]);
        let point = v(&t[4..7]);
        let (i0, i1, i2) = (i(t[7]), i(t[8]), i(t[9]));
        let expect = v(&t[11..14]);
        let got = match t[0] {
            "yaw" => {
                counts[0] += 1;
                levact::find_delta_from_rotation_yaw(origin, point, i0)
            }
            "roll" => {
                counts[1] += 1;
                levact::find_delta_from_rotation_roll(origin, point, i0)
            }
            "pitch" => {
                counts[2] += 1;
                levact::find_delta_from_rotation_pitch(origin, point, i0)
            }
            "full" => {
                counts[3] += 1;
                levact::find_delta_from_rotation(origin, point, i0, i1, i2)
            }
            op => panic!("unknown op {op}"),
        };
        assert_eq!(got.x.to_bits(), expect.x.to_bits(), "x: {line}");
        assert_eq!(got.y.to_bits(), expect.y.to_bits(), "y: {line}");
        assert_eq!(got.z.to_bits(), expect.z.to_bits(), "z: {line}");
    }
    assert_eq!(counts, [2000, 2000, 2000, 2000]);
}

#[test]
fn do_multi_line_check_truth_table() {
    assert!(levact::do_multi_line_check(false, 7));
    for t in 0..=3u8 {
        assert!(levact::do_multi_line_check(true, t));
    }
    assert!(!levact::do_multi_line_check(true, 4));
}

// ---- check-function replay ----------------------------------------------------

const ARENA_CAP: usize = 1 << 12;
const PER_KIND: usize = 1500;
/// Pool slot of the level actor (`ALevelInfo`), matching the harness pool.
const POOL_LEVEL: usize = 5;
/// Pool actors are slots 0..6, the hash is slot 6, and the model's level
/// context is slot 7. Only the addresses matter; nothing is read through them.
static TOKENS: [u8; 8] = [0; 8];
const HASH_SLOT: usize = 6;
const GMEM_SLOT: usize = 7;

fn token(slot: usize) -> *mut c_void {
    TOKENS.as_ptr().wrapping_add(slot).cast_mut().cast()
}

/// Pool index of an actor handle; -1 for NULL, -2 for anything else.
fn pool_index(p: *const c_void) -> i32 {
    if p.is_null() {
        return -1;
    }
    let off = (p as usize).wrapping_sub(TOKENS.as_ptr() as usize);
    if off < 6 { off as i32 } else { -2 }
}

fn blank(time: f32) -> FCheckResult {
    FCheckResult {
        next: ptr::null_mut(),
        actor: ptr::null_mut(),
        location: FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        normal: FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        primitive: ptr::null_mut(),
        time,
        item: -1,
    }
}

/// Scripted world state: an arena for `FCheckResult` nodes (never reallocated,
/// so handed-out pointers stay valid), plus the owner chain and block flags of
/// the five plain pool actors.
struct World {
    arena: Vec<FCheckResult>,
    cursor: usize,
    owner: [i32; 5],
    blocks: [bool; 5],
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World {
        arena: vec![blank(0.0); ARENA_CAP],
        cursor: 0,
        owner: [-1; 5],
        blocks: [false; 5],
    });
}

fn reset_world(owner: [i32; 5], blocks: [bool; 5]) {
    WORLD.with(|w| {
        let mut w = w.borrow_mut();
        w.cursor = 0;
        w.owner = owner;
        w.blocks = blocks;
    });
}

fn alloc(count: usize) -> *mut FCheckResult {
    WORLD.with(|w| {
        let mut w = w.borrow_mut();
        assert!(w.cursor + count <= ARENA_CAP, "arena exhausted");
        let p = w.arena.as_mut_ptr().wrapping_add(w.cursor);
        w.cursor += count;
        p
    })
}

/// `hmix`: the harness's 64-bit mixer over exact bit patterns.
fn hmix(h: u64, w: u64) -> u64 {
    let mut z = h ^ w
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(h << 6)
        .wrapping_add(h >> 2);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hvec(h: u64, v: FVector) -> u64 {
    hmix(
        hmix(hmix(h, u64::from(v.x.to_bits())), u64::from(v.y.to_bits())),
        u64::from(v.z.to_bits()),
    )
}

fn draw(h: u64, k: u64) -> u64 {
    hmix(h, 0x1000 + k)
}

fn coord5(u: u64) -> f32 {
    ((u % 5) as i64 - 2) as f32
}

fn bit01(u: u64) -> f32 {
    (u % 2) as f32
}

/// Fill a hit's scripted fields (not Actor, Next, or Primitive).
fn fill_hit(r: &mut FCheckResult, h: u64) {
    r.time = (draw(h, 1) % 1024) as f32 / 1024.0;
    r.location = FVector {
        x: coord5(draw(h, 2)),
        y: coord5(draw(h, 3)),
        z: coord5(draw(h, 4)),
    };
    r.normal = FVector {
        x: bit01(draw(h, 5)),
        y: bit01(draw(h, 6)),
        z: bit01(draw(h, 7)),
    };
    r.item = (draw(h, 8) % 1000) as i32 - 500;
}

/// Hash-returned list: `n = h % 4` nodes, built in reverse so the head is node 0.
fn build_list(h: u64) -> *mut FCheckResult {
    let n = (h % 4) as usize;
    let mut head: *mut FCheckResult = ptr::null_mut();
    for i in (0..n).rev() {
        let hi = hmix(h, 0x100 + i as u64);
        let node = alloc(1);
        unsafe {
            *node = blank(1.0);
            fill_hit(&mut *node, hi);
            (*node).actor = token(draw(hi, 9) as usize % 5);
            (*node).next = head.cast();
        }
        head = node;
    }
    head
}

/// Pool class kinds: slots 3, 4, 5 are AMover, AZoneInfo, ALevelInfo; 0..2 are
/// plain AActor, which match none of the three kinds.
fn class_kind(k: i32) -> i32 {
    match k {
        3 => levact::ACTOR_KIND_MOVER,
        4 => levact::ACTOR_KIND_ZONE_INFO,
        5 => levact::ACTOR_KIND_LEVEL_INFO,
        _ => -1,
    }
}

// Scripted services, matching the harness hooks and `Hp2LevActServices` thunks.

unsafe extern "C" fn svc_alloc(_mem: *mut c_void, count: i32) -> *mut FCheckResult {
    alloc(count as usize)
}

unsafe extern "C" fn svc_model_point(
    _level: *mut c_void,
    hit: *mut FCheckResult,
    location: *const FVector,
    extent: *const FVector,
) -> i32 {
    unsafe {
        let h = hvec(hvec(hmix(0x11, 0), *location), *extent);
        if h.is_multiple_of(3) {
            return 1;
        }
        fill_hit(&mut *hit, hmix(h, 0x12));
        0
    }
}

unsafe extern "C" fn svc_model_line(
    _level: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> i32 {
    unsafe {
        let h = hmix(
            hvec(hvec(hvec(hmix(0x21, 0), *end), *start), *extent),
            u64::from(flags),
        );
        if h.is_multiple_of(3) {
            return 1;
        }
        fill_hit(&mut *hit, hmix(h, 0x22));
        0
    }
}

unsafe extern "C" fn svc_hash_point(
    _hash: *mut c_void,
    _mem: *mut c_void,
    location: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> *mut FCheckResult {
    unsafe {
        let h = hmix(
            hvec(hvec(hmix(0x31, 0), *location), *extent),
            u64::from(flags),
        );
        build_list(h)
    }
}

unsafe extern "C" fn svc_hash_line(
    _hash: *mut c_void,
    _mem: *mut c_void,
    end: *const FVector,
    start: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> *mut FCheckResult {
    unsafe {
        let h = hmix(
            hvec(hvec(hvec(hmix(0x41, 0), *end), *start), *extent),
            u64::from(flags),
        );
        build_list(h)
    }
}

unsafe extern "C" fn svc_is_a(actor: *const c_void, kind: i32) -> i32 {
    i32::from(class_kind(pool_index(actor)) == kind)
}

unsafe extern "C" fn svc_owned(actor: *const c_void, other: *const c_void) -> i32 {
    let (source, target) = (pool_index(actor), pool_index(other));
    WORLD.with(|w| {
        let w = w.borrow();
        // `AActor::IsOwnedBy`: walk the Owner chain from `this`.
        let mut a = source;
        while a >= 0 {
            if a == target {
                return 1;
            }
            a = if a < 5 { w.owner[a as usize] } else { -1 };
        }
        0
    })
}

unsafe extern "C" fn svc_blocked(actor: *const c_void, other: *const c_void) -> i32 {
    let (source, target) = (pool_index(actor), pool_index(other));
    WORLD.with(|w| {
        let w = w.borrow();
        let blocks = |k: i32| (0..5).contains(&k) && w.blocks[k as usize];
        // `AActor::IsBlockedBy` reduces to `Other.bBlockActors && this.bBlockActors`
        // for these pool actors (no pawn, projectile, or brush classes).
        i32::from(blocks(source) && blocks(target))
    })
}

unsafe extern "C" fn svc_mem_save(_mem: *mut c_void, _buffer: *mut c_void) {}
unsafe extern "C" fn svc_mem_pop(_buffer: *mut c_void) {}
unsafe extern "C" fn svc_cycles() -> u32 {
    100
}
unsafe extern "C" fn svc_editor() -> i32 {
    0
}
unsafe extern "C" fn svc_void_pair(_a: *mut c_void, _b: *mut c_void) {}
static ACTOR_OP_TRIG: std::sync::LazyLock<Vec<f32>> = std::sync::LazyLock::new(|| {
    (0..16384i32)
        .map(|index| {
            let arg = (index as f32) * 2.0f32 * std::f32::consts::PI / 16384.0f32;
            (arg as f64).sin() as f32
        })
        .collect()
});
unsafe extern "C" fn svc_sin_tab(angle: i32) -> f32 {
    ACTOR_OP_TRIG[((angle >> 2) & 16383) as usize]
}
unsafe extern "C" fn svc_cos_tab(angle: i32) -> f32 {
    ACTOR_OP_TRIG[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

unsafe extern "C" fn svc_point_region(
    _model: *mut c_void,
    _zone: *mut c_void,
    _location: *const FVector,
    out: *mut FPointRegion,
) {
    unsafe {
        let region = &mut *out;
        region.zone = ptr::null_mut();
        region.i_leaf = -1;
        region.zone_number = 0;
    }
}
unsafe extern "C" fn svc_encroachment(
    _hash: *mut c_void,
    _mem: *mut c_void,
    _actor: *mut c_void,
    _location: *const FVector,
    _rotation: *const FRotator,
    _flags: u32,
) -> *mut FCheckResult {
    ptr::null_mut()
}
unsafe extern "C" fn svc_collision_box(_actor: *mut c_void, min: *mut FVector, max: *mut FVector) {
    unsafe {
        min.write(FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        max.write(FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    }
}
unsafe extern "C" fn svc_cylinder_extent(_actor: *mut c_void, out: *mut FVector) {
    unsafe {
        out.write(FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        })
    }
}
unsafe extern "C" fn svc_set_base(actor: *mut c_void, base: *mut c_void) {
    unsafe { ptr::addr_of_mut!((*actor.cast::<AActor>()).base).write_unaligned(base) }
}
unsafe extern "C" fn svc_move_smooth(_actor: *mut c_void, _delta: *const FVector) -> i32 {
    0
}
unsafe extern "C" fn svc_event_encroaching(_actor: *mut c_void, _other: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn svc_zone_event(_actor: *mut c_void, _zone: *mut c_void) {}
unsafe extern "C" fn svc_zone_leaving(_zone: *mut c_void, _actor: *mut c_void) {}
unsafe extern "C" fn svc_net_mode(_level: *mut c_void) -> u8 {
    0
}
unsafe extern "C" fn svc_tracker(_tracker: *mut c_void, _actor: *mut c_void) {}
unsafe extern "C" fn svc_touch_end(_actor: *mut c_void, _other: *mut c_void, _no_notify: i32) {}
unsafe extern "C" fn svc_overlap(_actor: *mut c_void, _other: *mut c_void) -> i32 {
    1
}

fn services() -> LevActServices {
    LevActServices {
        alloc_check_results: svc_alloc,
        model_point_check: svc_model_point,
        model_line_check: svc_model_line,
        hash_actor_point_check: svc_hash_point,
        hash_actor_line_check: svc_hash_line,
        actor_is_a: svc_is_a,
        actor_is_owned_by: svc_owned,
        actor_is_blocked_by: svc_blocked,
        mem_mark_save: svc_mem_save,
        mem_mark_pop: svc_mem_pop,
        app_cycles: svc_cycles,
        is_editor: svc_editor,
        bump_observer: svc_void_pair,
        sin_tab: svc_sin_tab,
        cos_tab: svc_cos_tab,
        model_point_region: svc_point_region,
        hash_add_actor: svc_void_pair,
        hash_remove_actor: svc_void_pair,
        hash_actor_encroachment_check: svc_encroachment,
        tracker_update: svc_tracker,
        actor_begin_touch: svc_void_pair,
        actor_end_touch: svc_touch_end,
        actor_is_overlapping: svc_overlap,
        actor_collision_box: svc_collision_box,
        actor_cylinder_extent: svc_cylinder_extent,
        actor_set_base: svc_set_base,
        actor_move_smooth: svc_move_smooth,
        event_bump: svc_void_pair,
        event_encroaching_on: svc_event_encroaching,
        event_encroached_by: svc_void_pair,
        event_zone_change: svc_zone_event,
        zone_event_actor_leaving: svc_zone_leaving,
        zone_event_actor_entered: svc_zone_leaving,
        pawn_event_foot_zone_change: svc_zone_event,
        pawn_event_head_zone_change: svc_zone_event,
        pri_set_player_zone: svc_zone_event,
        level_net_mode: svc_net_mode,
    }
}

/// `FCheckResult Hit(3.f)` with the sentinel fields the harness writes.
fn sentinel_hit() -> FCheckResult {
    let mut h = blank(3.0);
    h.location = FVector {
        x: 7.0,
        y: 8.0,
        z: 9.0,
    };
    h.normal = FVector {
        x: -7.0,
        y: -8.0,
        z: -9.0,
    };
    h.item = -3;
    h
}

/// Checks one hit against `[time, loc xyz, normal xyz, actor, item]` (9 tokens).
fn expect_hit(h: &FCheckResult, t: &[&str], line: &str) {
    assert_eq!(t.len(), 9, "hit tokens: {line}");
    let floats = [
        h.time,
        h.location.x,
        h.location.y,
        h.location.z,
        h.normal.x,
        h.normal.y,
        h.normal.z,
    ];
    for (k, got) in floats.iter().enumerate() {
        assert_eq!(got.to_bits(), f(t[k]).to_bits(), "float {k}: {line}");
    }
    assert_eq!(
        pool_index(h.actor),
        t[7].parse::<i32>().unwrap(),
        "actor: {line}"
    );
    assert_eq!(h.item, t[8].parse::<i32>().unwrap(), "item: {line}");
}

/// Checks a hit list: `<count>` then `count` hits of 9 tokens each.
fn expect_list(mut p: *mut FCheckResult, out: &[&str], line: &str) {
    let want: usize = out[0].parse().expect("count");
    assert_eq!(out.len(), 1 + 9 * want, "list length: {line}");
    let mut got = 0usize;
    while !p.is_null() {
        assert!(got < want, "extra hit: {line}");
        let slot = &out[1 + 9 * got..1 + 9 * got + 9];
        unsafe {
            expect_hit(&*p, slot, line);
            p = (*p).next.cast();
        }
        got += 1;
    }
    assert_eq!(got, want, "hit count: {line}");
}

#[test]
fn levact_check_fixtures_bit_exact() {
    let svc = services();
    let mut counts = [0usize; 4];
    for line in CHECK_FIXTURE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert!(t.len() >= 29 && t[28] == "->", "malformed record: {line}");
        let kind: usize = t[0].parse().expect("kind");
        counts[kind] += 1;

        let loc = v(&t[1..4]);
        let end = v(&t[4..7]);
        let start = v(&t[7..10]);
        let ext = v(&t[10..13]);
        let flags = u32::from_str_radix(t[13], 16).expect("flags");
        let node = u32::from_str_radix(t[14], 16).expect("node");
        let check_actors = t[15] == "1";
        let with_level = t[16] == "1";
        let src: i32 = t[17].parse().expect("src");
        let owner: [i32; 5] = std::array::from_fn(|k| t[18 + k].parse().expect("owner"));
        let blocks: [bool; 5] = std::array::from_fn(|k| t[23 + k] == "1");
        reset_world(owner, blocks);
        let mut ulevel: ULevel = unsafe { std::mem::zeroed() };
        let ctx = LevelCtx {
            svc: &svc,
            gmem: token(GMEM_SLOT),
            hash: token(HASH_SLOT),
            ulevel: &mut ulevel,
        };
        let level = if with_level {
            token(POOL_LEVEL)
        } else {
            ptr::null_mut()
        };
        let out = &t[29..];
        match kind {
            0 => {
                assert_eq!(out.len(), 10, "sp length: {line}");
                let mut hit = sentinel_hit();
                let ret = unsafe {
                    levact::single_point_check(ctx, &mut hit, level, loc, ext, flags, check_actors)
                };
                assert_eq!(
                    i32::from(ret),
                    out[0].parse::<i32>().unwrap(),
                    "ret: {line}"
                );
                expect_hit(&hit, &out[1..10], line);
            }
            1 => {
                let head = unsafe {
                    levact::multi_point_check(ctx, ctx.gmem, level, loc, ext, flags, check_actors)
                };
                expect_list(head, out, line);
            }
            2 => {
                assert_eq!(out.len(), 10, "sl length: {line}");
                let mut hit = sentinel_hit();
                let source = if src >= 0 {
                    token(src as usize)
                } else {
                    ptr::null_mut()
                };
                let seg = LineQuery {
                    end,
                    start,
                    extent: ext,
                };
                let ret = unsafe {
                    levact::single_line_check(
                        ctx,
                        &mut hit,
                        source,
                        token(POOL_LEVEL),
                        seg,
                        flags,
                        node as u8,
                    )
                };
                assert_eq!(
                    i32::from(ret),
                    out[0].parse::<i32>().unwrap(),
                    "ret: {line}"
                );
                expect_hit(&hit, &out[1..10], line);
            }
            3 => {
                let seg = LineQuery {
                    end,
                    start,
                    extent: ext,
                };
                let head = unsafe {
                    levact::multi_line_check(ctx, ctx.gmem, level, seg, check_actors, node)
                };
                expect_list(head, out, line);
            }
            _ => panic!("unknown kind: {line}"),
        }
    }
    assert_eq!(counts, [PER_KIND; 4]);
}

#[derive(Clone, Copy)]
struct ActorRow {
    class: i32,
    owner: i32,
    base: i32,
    region: [i32; 3],
    flags: u32,
    tag: u32,
    location: FVector,
    old_location: FVector,
    just_teleported: bool,
    rotation: [i32; 3],
    velocity: FVector,
    collision: [f32; 3],
    standing: u8,
    foot: [i32; 3],
    head: [i32; 3],
    eye_height: f32,
}

fn actor_row(t: &[&str], at: usize) -> (ActorRow, usize) {
    let row = ActorRow {
        class: t[at].parse().expect("actor class"),
        owner: t[at + 1].parse().expect("actor owner"),
        base: t[at + 2].parse().expect("actor base"),
        region: [
            t[at + 3].parse().expect("region zone"),
            t[at + 4].parse().expect("region leaf"),
            u8::from_str_radix(t[at + 5], 16).expect("region number") as i32,
        ],
        flags: u32::from_str_radix(t[at + 6], 16).expect("actor flags"),
        tag: u32::from_str_radix(t[at + 7], 16).expect("actor tag"),
        location: v(&t[at + 8..at + 11]),
        old_location: v(&t[at + 11..at + 14]),
        just_teleported: t[at + 14] == "1",
        rotation: [
            t[at + 15].parse().expect("yaw"),
            t[at + 16].parse().expect("roll"),
            t[at + 17].parse().expect("pitch"),
        ],
        velocity: v(&t[at + 18..at + 21]),
        collision: [f(t[at + 21]), f(t[at + 22]), f(t[at + 23])],
        standing: t[at + 24].parse().expect("standing count"),
        foot: [
            t[at + 25].parse().expect("foot zone"),
            t[at + 26].parse().expect("foot leaf"),
            u8::from_str_radix(t[at + 27], 16).expect("foot number") as i32,
        ],
        head: [
            t[at + 28].parse().expect("head zone"),
            t[at + 29].parse().expect("head leaf"),
            u8::from_str_radix(t[at + 30], 16).expect("head number") as i32,
        ],
        eye_height: f(t[at + 31]),
    };
    (row, at + 32)
}

fn actor_handle(ptrs: &[*mut c_void], index: i32) -> *mut c_void {
    if index == -1 {
        ptr::null_mut()
    } else if index >= 0 && (index as usize) < ptrs.len() {
        ptrs[index as usize]
    } else {
        token((index as usize) % TOKENS.len())
    }
}

fn point_region(ptrs: &[*mut c_void], zone: [i32; 3]) -> FPointRegion {
    let mut region: FPointRegion = unsafe { std::mem::zeroed() };
    region.zone = actor_handle(ptrs, zone[0]);
    region.i_leaf = zone[1];
    region.zone_number = zone[2] as u8;
    region
}

fn init_actor(row: ActorRow, actor: &mut APawn, ptrs: &[*mut c_void]) {
    let a = &mut actor.base;
    a.owner = actor_handle(ptrs, row.owner);
    a.base = actor_handle(ptrs, row.base);
    a.region = point_region(ptrs, row.region);
    a.tag = row.tag.to_ne_bytes();
    a.location = row.location;
    a.old_location = row.old_location;
    a.set_b_just_teleported(row.just_teleported);
    a.rotation = FRotator {
        pitch: row.rotation[2],
        yaw: row.rotation[0],
        roll: row.rotation[1],
    };
    a.velocity = row.velocity;
    a.collision_radius = row.collision[0];
    a.collision_width = row.collision[1];
    a.collision_height = row.collision[2];
    a.standing_count = row.standing;
    a.set_b_static(row.flags & 1 != 0);
    a.set_b_movable(row.flags & 2 != 0);
    a.set_b_collide_actors(row.flags & 4 != 0);
    a.set_b_collide_world(row.flags & 8 != 0);
    a.set_b_block_actors(row.flags & 16 != 0);
    a.set_b_block_players(row.flags & 32 != 0);
    a.set_b_is_pawn(row.flags & 64 != 0);
    a.set_b_delete_me(row.flags & 256 != 0);
    if row.flags & 128 != 0 {
        a.brush = token(0);
    }
    actor.foot_region = point_region(ptrs, row.foot);
    actor.head_region = point_region(ptrs, row.head);
    actor.eye_height = row.eye_height;
}

fn ptr_index(ptrs: &[*mut c_void], ptr: *mut c_void) -> i32 {
    if ptr.is_null() {
        -1
    } else {
        ptrs.iter().position(|p| *p == ptr).map_or(-2, |i| i as i32)
    }
}

fn region_row(region: FPointRegion, ptrs: &[*mut c_void]) -> [i32; 3] {
    [
        ptr_index(ptrs, region.zone),
        region.i_leaf,
        i32::from(region.zone_number),
    ]
}

fn actor_flags(actor: &AActor) -> u32 {
    u32::from(actor.b_static())
        | (u32::from(actor.b_movable()) << 1)
        | (u32::from(actor.b_collide_actors()) << 2)
        | (u32::from(actor.b_collide_world()) << 3)
        | (u32::from(actor.b_block_actors()) << 4)
        | (u32::from(actor.b_block_players()) << 5)
        | (u32::from(actor.b_is_pawn()) << 6)
        | (u32::from(actor.b_delete_me()) << 8)
        | (u32::from(actor.b_just_teleported()) << 9)
}

fn assert_actor_row(row: ActorRow, actor: &APawn, ptrs: &[*mut c_void], line: &str) {
    let a = &actor.base;
    assert_eq!(ptr_index(ptrs, a.owner), row.owner, "owner: {line}");
    assert_eq!(ptr_index(ptrs, a.base), row.base, "base: {line}");
    assert_eq!(region_row(a.region, ptrs), row.region, "region: {line}");
    assert_eq!(
        actor_flags(a) | (row.flags & 128),
        row.flags,
        "flags: {line}"
    );
    assert_eq!(u32::from_ne_bytes(a.tag), row.tag, "tag: {line}");
    assert_eq!(
        a.location.x.to_bits(),
        row.location.x.to_bits(),
        "location.x: {line}"
    );
    assert_eq!(
        a.location.y.to_bits(),
        row.location.y.to_bits(),
        "location.y: {line}"
    );
    assert_eq!(
        a.location.z.to_bits(),
        row.location.z.to_bits(),
        "location.z: {line}"
    );
    assert_eq!(
        a.old_location.x.to_bits(),
        row.old_location.x.to_bits(),
        "old_location.x: {line}"
    );
    assert_eq!(
        a.old_location.y.to_bits(),
        row.old_location.y.to_bits(),
        "old_location.y: {line}"
    );
    assert_eq!(
        a.old_location.z.to_bits(),
        row.old_location.z.to_bits(),
        "old_location.z: {line}"
    );
    assert_eq!(
        a.b_just_teleported(),
        row.just_teleported,
        "just_teleported: {line}"
    );
    assert_eq!(
        [a.rotation.yaw, a.rotation.roll, a.rotation.pitch],
        row.rotation,
        "rotation: {line}"
    );
    assert_eq!(
        a.velocity.x.to_bits(),
        row.velocity.x.to_bits(),
        "velocity.x: {line}"
    );
    assert_eq!(
        a.velocity.y.to_bits(),
        row.velocity.y.to_bits(),
        "velocity.y: {line}"
    );
    assert_eq!(
        a.velocity.z.to_bits(),
        row.velocity.z.to_bits(),
        "velocity.z: {line}"
    );
    assert_eq!(
        a.collision_radius.to_bits(),
        row.collision[0].to_bits(),
        "radius: {line}"
    );
    assert_eq!(
        a.collision_width.to_bits(),
        row.collision[1].to_bits(),
        "width: {line}"
    );
    assert_eq!(
        a.collision_height.to_bits(),
        row.collision[2].to_bits(),
        "height: {line}"
    );
    assert_eq!(a.standing_count, row.standing, "standing: {line}");
    assert_eq!(
        region_row(actor.foot_region, ptrs),
        row.foot,
        "foot region: {line}"
    );
    assert_eq!(
        region_row(actor.head_region, ptrs),
        row.head,
        "head region: {line}"
    );
    assert_eq!(
        actor.eye_height.to_bits(),
        row.eye_height.to_bits(),
        "eye height: {line}"
    );
}

struct ActorOpWorld {
    rows: Vec<ActorRow>,
    actors: Vec<*mut c_void>,
    level_info: i32,
    hooks: Vec<Vec<String>>,
    next_hook: usize,
}

thread_local! {
    static ACTOR_OP_WORLD: RefCell<Option<ActorOpWorld>> = const { RefCell::new(None) };
}

fn actor_op_world<R>(f: impl FnOnce(&mut ActorOpWorld) -> R) -> R {
    ACTOR_OP_WORLD.with(|slot| f(slot.borrow_mut().as_mut().expect("active actor-op world")))
}

fn next_actor_hook(name: &str) -> Vec<String> {
    actor_op_world(|world| {
        let row = world
            .hooks
            .get(world.next_hook)
            .unwrap_or_else(|| panic!("unexpected hook {name}"));
        assert_eq!(
            row.first().map(String::as_str),
            Some(name),
            "hook order at {}",
            world.next_hook
        );
        world.next_hook += 1;
        row.clone()
    })
}

fn hook_f(row: &[String], at: usize, actual: f32) {
    assert_eq!(f(&row[at]).to_bits(), actual.to_bits(), "hook float arg");
}

fn hook_vec(row: &[String], at: usize, actual: FVector) {
    hook_f(row, at, actual.x);
    hook_f(row, at + 1, actual.y);
    hook_f(row, at + 2, actual.z);
}

fn hook_actor(row: &[String], at: usize, actual: *mut c_void) {
    let got = actor_op_world(|world| ptr_index(&world.actors, actual));
    assert_eq!(row[at].parse::<i32>().unwrap(), got, "hook actor");
}

fn hit_from_tokens(row: &[String], at: usize, actors: &[*mut c_void]) -> (FCheckResult, usize) {
    let mut hit = blank(f(&row[at]));
    hit.location = v(&[&row[at + 1], &row[at + 2], &row[at + 3]]);
    hit.normal = v(&[&row[at + 4], &row[at + 5], &row[at + 6]]);
    let actor: i32 = row[at + 7].parse().expect("hit actor");
    hit.actor = actor_handle(actors, actor);
    hit.item = row[at + 8].parse().expect("hit item");
    (hit, at + 9)
}

fn expect_actor_hit(hit: &FCheckResult, t: &[&str], actors: &[*mut c_void], line: &str) {
    assert_eq!(t.len(), 9, "actor hit length: {line}");
    let expected = [
        f(t[0]).to_bits(),
        f(t[1]).to_bits(),
        f(t[2]).to_bits(),
        f(t[3]).to_bits(),
        f(t[4]).to_bits(),
        f(t[5]).to_bits(),
        f(t[6]).to_bits(),
    ];
    let actual = [
        hit.time.to_bits(),
        hit.location.x.to_bits(),
        hit.location.y.to_bits(),
        hit.location.z.to_bits(),
        hit.normal.x.to_bits(),
        hit.normal.y.to_bits(),
        hit.normal.z.to_bits(),
    ];
    assert_eq!(actual, expected, "actor hit values: {line}");
    assert_eq!(
        ptr_index(actors, hit.actor),
        t[7].parse::<i32>().unwrap(),
        "actor hit actor: {line}"
    );
    assert_eq!(
        hit.item,
        t[8].parse::<i32>().unwrap(),
        "actor hit item: {line}"
    );
}

fn op_actor_kind(actor: *const c_void) -> (usize, ActorRow) {
    actor_op_world(|world| {
        let index = ptr_index(&world.actors, actor.cast_mut());
        assert!(index >= 0, "unknown actor handle");
        (index as usize, world.rows[index as usize])
    })
}

unsafe extern "C" fn op_is_a(actor: *const c_void, kind: i32) -> i32 {
    let (_, row) = op_actor_kind(actor);
    i32::from(match kind {
        levact::ACTOR_KIND_LEVEL_INFO => row.class == 3,
        levact::ACTOR_KIND_MOVER => row.class == 1,
        levact::ACTOR_KIND_ZONE_INFO => row.class == 2,
        levact::ACTOR_KIND_BRUSH => row.class == 5 || row.flags & 128 != 0,
        // Harness `actor_class` reports 4 for the APlayerPawn (APawn child) subject.
        levact::ACTOR_KIND_PAWN => row.class == 4,
        levact::ACTOR_KIND_DECORATION => false,
        _ => false,
    })
}

unsafe extern "C" fn op_owned(actor: *const c_void, other: *const c_void) -> i32 {
    let target = actor_op_world(|world| ptr_index(&world.actors, other.cast_mut()));
    let mut index = actor_op_world(|world| ptr_index(&world.actors, actor.cast_mut()));
    while index >= 0 {
        if index == target {
            return 1;
        }
        index = actor_op_world(|world| world.rows[index as usize].owner);
    }
    0
}

unsafe extern "C" fn op_blocked(actor: *const c_void, other: *const c_void) -> i32 {
    let (_, source) = op_actor_kind(actor);
    let (_, target) = op_actor_kind(other);
    i32::from(source.flags & 16 != 0 && target.flags & 16 != 0)
}

unsafe extern "C" fn op_model_point(
    level: *mut c_void,
    hit: *mut FCheckResult,
    location: *const FVector,
    extent: *const FVector,
) -> i32 {
    let row = next_actor_hook("PC");
    let location = unsafe { *location };
    let extent = unsafe { *extent };
    hook_vec(&row, 1, location);
    hook_vec(&row, 4, extent);
    let flags = u32::from_str_radix(&row[7], 16).unwrap();
    assert_eq!(flags, 0, "point-check flags");
    assert_eq!(
        actor_op_world(|world| ptr_index(&world.actors, level)),
        actor_op_world(|world| world.level_info),
        "PointCheck LevelInfo context"
    );
    let answer: i32 = row[9].parse().unwrap();
    let (scripted, end) = actor_op_world(|world| hit_from_tokens(&row, 10, &world.actors));
    assert_eq!(end, row.len(), "PC row arity");
    unsafe { hit.write(scripted) };
    answer
}

unsafe extern "C" fn op_model_line(
    level: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> i32 {
    let row = next_actor_hook("LC");
    hook_vec(&row, 1, unsafe { *end });
    hook_vec(&row, 4, unsafe { *start });
    hook_vec(&row, 7, unsafe { *extent });
    assert_eq!(
        u32::from_str_radix(&row[10], 16).unwrap(),
        flags,
        "line-check flags"
    );
    assert_eq!(
        actor_op_world(|world| ptr_index(&world.actors, level)),
        actor_op_world(|world| world.level_info),
        "LineCheck LevelInfo context"
    );
    let answer: i32 = row[12].parse().unwrap();
    let (scripted, next) = actor_op_world(|world| hit_from_tokens(&row, 13, &world.actors));
    assert_eq!(next, row.len(), "LC row arity");
    unsafe { hit.write(scripted) };
    answer
}

fn hook_hit_list(
    row: &[String],
    at: usize,
    count: usize,
    actors: &[*mut c_void],
) -> *mut FCheckResult {
    if count == 0 {
        assert_eq!(at, row.len(), "empty hit-list row arity");
        return ptr::null_mut();
    }
    assert_eq!(row.len(), at + count * 9, "hit-list row arity");
    let head = alloc(count);
    for n in 0..count {
        let (mut hit, next) = hit_from_tokens(row, at + n * 9, actors);
        hit.next = if n + 1 == count {
            ptr::null_mut()
        } else {
            unsafe { head.add(n + 1).cast() }
        };
        unsafe { head.add(n).write(hit) };
        debug_assert_eq!(next, at + (n + 1) * 9);
    }
    head
}

unsafe extern "C" fn op_hash_point(
    _hash: *mut c_void,
    _mem: *mut c_void,
    location: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> *mut FCheckResult {
    let row = next_actor_hook("AP");
    hook_vec(&row, 1, unsafe { *location });
    hook_vec(&row, 4, unsafe { *extent });
    assert_eq!(u32::from_str_radix(&row[7], 16).unwrap(), flags, "AP flags");
    let count: usize = row[8].parse().unwrap();
    actor_op_world(|world| hook_hit_list(&row, 9, count, &world.actors))
}

unsafe extern "C" fn op_hash_line(
    _hash: *mut c_void,
    _mem: *mut c_void,
    end: *const FVector,
    start: *const FVector,
    extent: *const FVector,
    flags: u32,
) -> *mut FCheckResult {
    let row = next_actor_hook("AL");
    hook_vec(&row, 1, unsafe { *end });
    hook_vec(&row, 4, unsafe { *start });
    hook_vec(&row, 7, unsafe { *extent });
    assert_eq!(
        u32::from_str_radix(&row[10], 16).unwrap(),
        flags,
        "AL flags"
    );
    let count: usize = row[11].parse().unwrap();
    actor_op_world(|world| hook_hit_list(&row, 12, count, &world.actors))
}

unsafe extern "C" fn op_hash_encroachment(
    _hash: *mut c_void,
    _mem: *mut c_void,
    actor: *mut c_void,
    location: *const FVector,
    rotation: *const FRotator,
    flags: u32,
) -> *mut FCheckResult {
    let row = next_actor_hook("EC");
    hook_actor(&row, 1, actor);
    hook_vec(&row, 2, unsafe { *location });
    let rotation = unsafe { *rotation };
    assert_eq!(
        [
            row[5].parse::<i32>().unwrap(),
            row[6].parse::<i32>().unwrap(),
            row[7].parse::<i32>().unwrap(),
        ],
        [rotation.yaw, rotation.roll, rotation.pitch],
        "EC rotation"
    );
    assert_eq!(u32::from_str_radix(&row[8], 16).unwrap(), flags, "EC flags");
    let count: usize = row[9].parse().unwrap();
    actor_op_world(|world| hook_hit_list(&row, 10, count, &world.actors))
}

unsafe extern "C" fn op_model_region(
    _model: *mut c_void,
    zone: *mut c_void,
    location: *const FVector,
    out: *mut FPointRegion,
) {
    let row = next_actor_hook("ZP");
    hook_actor(&row, 1, zone);
    hook_vec(&row, 2, unsafe { *location });
    let region = point_region(
        &actor_op_world(|world| world.actors.clone()),
        [
            row[5].parse().unwrap(),
            row[6].parse().unwrap(),
            u8::from_str_radix(&row[7], 16).unwrap() as i32,
        ],
    );
    unsafe { out.write(region) };
}

unsafe extern "C" fn op_collision_box(actor: *mut c_void, min: *mut FVector, max: *mut FVector) {
    let gp = next_actor_hook("GP");
    hook_actor(&gp, 1, actor);
    assert_eq!(gp.len(), 2, "GP row arity");
    let row = next_actor_hook("CB");
    hook_actor(&row, 1, actor);
    assert_eq!(row[2], "1", "collision box world flag");
    let minimum = v(&[&row[3], &row[4], &row[5]]);
    let maximum = v(&[&row[6], &row[7], &row[8]]);
    assert_eq!(row.len(), 9, "CB row arity");
    unsafe {
        min.write(minimum);
        max.write(maximum);
    }
}

unsafe extern "C" fn op_encroaching(actor: *mut c_void, other: *mut c_void) -> i32 {
    let row = next_actor_hook("ER");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, other);
    assert_eq!(row.len(), 4, "ER row arity");
    row[3].parse().unwrap()
}

unsafe extern "C" fn op_move_smooth(actor: *mut c_void, delta: *const FVector) -> i32 {
    let row = next_actor_hook("MS");
    hook_actor(&row, 1, actor);
    hook_vec(&row, 2, unsafe { *delta });
    let result: i32 = row[5].parse().unwrap();
    let location = v(&[&row[6], &row[7], &row[8]]);
    unsafe {
        ptr::addr_of_mut!((*actor.cast::<AActor>()).location).write_unaligned(location);
    }
    assert_eq!(row.len(), 9, "MS row arity");
    result
}

unsafe extern "C" fn op_begin_touch(actor: *mut c_void, other: *mut c_void) {
    let row = next_actor_hook("PB");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, other);
    assert_eq!(row.len(), 3, "PB row arity");
}

unsafe extern "C" fn op_end_touch(actor: *mut c_void, other: *mut c_void, no_notify: i32) {
    let row = next_actor_hook("ET");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, other);
    assert_eq!(row[3].parse::<i32>().unwrap(), no_notify);
    assert_eq!(row.len(), 4, "ET row arity");
}

unsafe extern "C" fn op_zone_change(actor: *mut c_void, zone: *mut c_void) {
    let row = next_actor_hook("ZC");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, zone);
    assert_eq!(row.len(), 3, "ZC row arity");
}

unsafe extern "C" fn op_zone_leaving(zone: *mut c_void, actor: *mut c_void) {
    let row = next_actor_hook("ZL");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, zone);
    assert_eq!(row.len(), 3, "ZL row arity");
}

unsafe extern "C" fn op_zone_entered(zone: *mut c_void, actor: *mut c_void) {
    let row = next_actor_hook("ZE");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, zone);
    assert_eq!(row.len(), 3, "ZE row arity");
}

unsafe extern "C" fn op_foot_zone(actor: *mut c_void, zone: *mut c_void) {
    let row = next_actor_hook("ZF");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, zone);
    assert_eq!(row.len(), 3, "ZF row arity");
}

unsafe extern "C" fn op_head_zone(actor: *mut c_void, zone: *mut c_void) {
    let row = next_actor_hook("ZH");
    hook_actor(&row, 1, actor);
    hook_actor(&row, 2, zone);
    assert_eq!(row.len(), 3, "ZH row arity");
}

unsafe extern "C" fn op_hash_add(hash: *mut c_void, actor: *mut c_void) {
    assert_eq!(hash, token(HASH_SLOT), "Hash::AddActor hash");
    let row = next_actor_hook("HA");
    hook_actor(&row, 1, actor);
    assert_eq!(row.len(), 2, "HA row arity");
}

unsafe extern "C" fn op_hash_remove(hash: *mut c_void, actor: *mut c_void) {
    assert_eq!(hash, token(HASH_SLOT), "Hash::RemoveActor hash");
    let row = next_actor_hook("HR");
    hook_actor(&row, 1, actor);
    assert_eq!(row[2], "0", "Hash::RemoveActor thorough argument");
    assert_eq!(row.len(), 3, "HR row arity");
}

/// `AActor::GetCylinderExtent` for the harness actors: their storage is zeroed, so
/// `CollideType == CT_AlignedCylinder` with no Mesh, giving `(Radius, Radius, Height)`.
unsafe extern "C" fn op_cylinder_extent(actor: *mut c_void, out: *mut FVector) {
    let a = actor.cast::<AActor>();
    unsafe {
        let radius = ptr::addr_of!((*a).collision_radius).read_unaligned();
        let height = ptr::addr_of!((*a).collision_height).read_unaligned();
        out.write(FVector {
            x: radius,
            y: radius,
            z: height,
        });
    }
}

fn actor_op_services() -> LevActServices {
    let mut svc = services();
    svc.model_point_check = op_model_point;
    svc.actor_cylinder_extent = op_cylinder_extent;
    svc.model_line_check = op_model_line;
    svc.hash_actor_point_check = op_hash_point;
    svc.hash_actor_line_check = op_hash_line;
    svc.actor_is_a = op_is_a;
    svc.actor_is_owned_by = op_owned;
    svc.actor_is_blocked_by = op_blocked;
    svc.model_point_region = op_model_region;
    svc.hash_actor_encroachment_check = op_hash_encroachment;
    svc.hash_add_actor = op_hash_add;
    svc.hash_remove_actor = op_hash_remove;
    svc.actor_begin_touch = op_begin_touch;
    svc.actor_end_touch = op_end_touch;
    svc.actor_collision_box = op_collision_box;
    svc.actor_move_smooth = op_move_smooth;
    svc.event_encroaching_on = op_encroaching;
    svc.event_zone_change = op_zone_change;
    svc.zone_event_actor_leaving = op_zone_leaving;
    svc.zone_event_actor_entered = op_zone_entered;
    svc.pawn_event_foot_zone_change = op_foot_zone;
    svc.pawn_event_head_zone_change = op_head_zone;
    svc
}

fn actor_hook_rows(t: &[&str], mut at: usize, count: usize, line: &str) -> Vec<Vec<String>> {
    let mut hooks = Vec::with_capacity(count);
    for _ in 0..count {
        let name = t[at];
        let len = match name {
            "PC" => 19,
            "LC" => 22,
            "AP" => 9 + 9 * t[at + 8].parse::<usize>().expect("AP count"),
            "AL" => 12 + 9 * t[at + 11].parse::<usize>().expect("AL count"),
            "EC" => 10 + 9 * t[at + 9].parse::<usize>().expect("EC count"),
            "HA" => 2,
            "HR" => 3,
            "GP" => 2,
            "CB" => 9,
            "ER" => 4,
            "MS" => 9,
            "PB" => 3,
            "ET" => 4,
            "ZP" => 8,
            "ZL" | "ZC" | "ZE" | "ZF" | "ZH" => 3,
            _ => panic!("unknown actor-op hook {name}: {line}"),
        };
        hooks.push(t[at..at + len].iter().map(|s| (*s).to_string()).collect());
        at += len;
    }
    assert_eq!(at, t.len(), "trailing actor-op tokens: {line}");
    hooks
}

#[test]
fn levact_actor_ops_fixtures_bit_exact() {
    let svc = actor_op_services();
    let mut branch_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut op_counts = [0usize; 5];

    for line in ACTOR_OPS_FIXTURE.lines() {
        let mut t: Vec<&str> = line.split_whitespace().collect();
        let far_actor =
            (t[0] == "far_move").then(|| t.remove(3).parse::<i32>().expect("far_move actor index"));
        let arrow = t
            .iter()
            .position(|word| *word == "->")
            .expect("record arrow");
        let bar = t
            .iter()
            .position(|word| *word == "|")
            .expect("hook separator");
        assert!(arrow < bar, "malformed actor-op record: {line}");
        let op = t[0];
        let op_index = match op {
            "move_actor" => 0,
            "check_encroachment" => 1,
            "find_spot" => 2,
            "set_actor_zone" => 3,
            "far_move" => 4,
            _ => panic!("unknown actor operation {op}: {line}"),
        };
        op_counts[op_index] += 1;
        *branch_counts.entry(format!("{op}/{}", t[2])).or_default() += 1;

        let actor_count: usize = t[3].parse().expect("actor count");
        assert_eq!(actor_count, 6, "actor pool width: {line}");
        let level_idx: i32 = t[4].parse().expect("level actor index");
        assert!((0..6).contains(&level_idx), "level actor index: {line}");
        let level_actor_count: usize = t[5].parse().expect("level actor array count");
        let level_actor_indices: Vec<i32> = (0..level_actor_count)
            .map(|i| t[6 + i].parse().expect("level actor array index"))
            .collect();
        assert_eq!(
            level_actor_indices.first().copied(),
            Some(level_idx),
            "level actor must be Actors(0): {line}"
        );
        assert!(
            level_actor_indices
                .iter()
                .all(|index| (0..6).contains(index)),
            "invalid level actor array index: {line}"
        );
        let mut initial_rows = Vec::with_capacity(actor_count);
        let mut at = 6 + level_actor_count;
        for _ in 0..actor_count {
            let (row, next) = actor_row(&t, at);
            initial_rows.push(row);
            at = next;
        }
        assert_eq!(
            initial_rows[level_idx as usize].class, 3,
            "level actor class: {line}"
        );
        let op_inputs = &t[at..arrow];
        let result_at = arrow + 1;
        let expected_hook_count: usize = t[bar + 1].parse().expect("hook count");
        let hooks = actor_hook_rows(&t, bar + 2, expected_hook_count, line);
        let mvcenter = op == "move_actor" && t[2] == "mvcenter";
        let mvcenter_hooks = if mvcenter { Some(hooks.clone()) } else { None };

        let slot_count = 6;
        let mut objects: Vec<APawn> = std::iter::repeat_with(|| unsafe { std::mem::zeroed() })
            .take(slot_count)
            .collect();
        let actor_ptrs: Vec<*mut c_void> = objects
            .iter_mut()
            .map(|a| ptr::addr_of_mut!(a.base).cast())
            .collect();
        let world_rows = initial_rows.clone();
        for (index, row) in world_rows.iter().copied().enumerate() {
            init_actor(row, &mut objects[index], &actor_ptrs);
        }
        ACTOR_OP_WORLD.with(|world| {
            *world.borrow_mut() = Some(ActorOpWorld {
                rows: world_rows,
                actors: actor_ptrs.clone(),
                level_info: level_idx,
                hooks,
                next_hook: 0,
            });
        });
        reset_world([-1; 5], [false; 5]);

        let mut level_actors: Vec<*mut c_void> = level_actor_indices
            .iter()
            .map(|index| actor_handle(&actor_ptrs, *index))
            .collect();
        let mut ulevel: ULevel = unsafe { std::mem::zeroed() };
        unsafe {
            ptr::addr_of_mut!(ulevel.actors)
                .cast::<TArrayRaw>()
                .write_unaligned(TArrayRaw {
                    data: level_actors.as_mut_ptr().cast(),
                    num: level_actors.len() as i32,
                    max: level_actors.len() as i32,
                });
        }
        ulevel.model = token(2);
        ulevel.brush_tracker = token(3);
        let ctx = LevelCtx {
            svc: &svc,
            gmem: token(GMEM_SLOT),
            hash: token(HASH_SLOT),
            ulevel: &mut ulevel,
        };
        let mut final_hit = sentinel_hit();
        match op {
            "move_actor" => {
                let actor_idx: i32 = op_inputs[0].parse().unwrap();
                let delta = v(&op_inputs[1..4]);
                let new_rotation = FRotator {
                    yaw: op_inputs[4].parse().unwrap(),
                    roll: op_inputs[5].parse().unwrap(),
                    pitch: op_inputs[6].parse().unwrap(),
                };
                let (mut hit, _) = hit_from_tokens(
                    &op_inputs
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<_>>(),
                    7,
                    &actor_ptrs,
                );
                let test = op_inputs[16] == "1";
                let ignore_pawns = op_inputs[17] == "1";
                let ignore_bases = op_inputs[18] == "1";
                let no_fail = op_inputs[19] == "1";
                let moved = unsafe {
                    levact::move_actor(
                        ctx,
                        actor_handle(&actor_ptrs, actor_idx),
                        delta,
                        new_rotation,
                        &mut hit,
                        test,
                        ignore_pawns,
                        ignore_bases,
                        no_fail,
                    )
                };
                final_hit = hit;
                assert_eq!(
                    moved,
                    t[result_at].parse::<i32>().unwrap() != 0,
                    "MoveActor return: {line}"
                );
                expect_actor_hit(
                    &final_hit,
                    &t[result_at + 1..result_at + 10],
                    &actor_ptrs,
                    line,
                );
            }
            "check_encroachment" => {
                let actor_idx: i32 = op_inputs[0].parse().unwrap();
                let location = v(&op_inputs[1..4]);
                let rotation = FRotator {
                    yaw: op_inputs[4].parse().unwrap(),
                    roll: op_inputs[5].parse().unwrap(),
                    pitch: op_inputs[6].parse().unwrap(),
                };
                let touch_notify = op_inputs[7] == "1";
                let encroached = unsafe {
                    levact::check_encroachment(
                        ctx,
                        actor_handle(&actor_ptrs, actor_idx),
                        location,
                        rotation,
                        touch_notify,
                    )
                };
                assert_eq!(
                    encroached,
                    t[result_at].parse::<i32>().unwrap() != 0,
                    "CheckEncroachment return: {line}"
                );
            }
            "find_spot" => {
                let extent = v(&op_inputs[0..3]);
                let mut location = v(&op_inputs[3..6]);
                let check_actors = op_inputs[6] == "1";
                let assume_fit = op_inputs[7] == "1";
                let found = unsafe {
                    levact::find_spot(ctx, extent, &mut location, check_actors, assume_fit)
                };
                assert_eq!(
                    found,
                    t[result_at].parse::<i32>().unwrap() != 0,
                    "FindSpot return: {line}"
                );
                let expected_location = v(&t[result_at + 1..result_at + 4]);
                assert_eq!(
                    location.x.to_bits(),
                    expected_location.x.to_bits(),
                    "FindSpot x: {line}"
                );
                assert_eq!(
                    location.y.to_bits(),
                    expected_location.y.to_bits(),
                    "FindSpot y: {line}"
                );
                assert_eq!(
                    location.z.to_bits(),
                    expected_location.z.to_bits(),
                    "FindSpot z: {line}"
                );
            }
            "set_actor_zone" => {
                let actor_idx: i32 = op_inputs[0].parse().unwrap();
                unsafe {
                    levact::set_actor_zone(
                        ctx,
                        actor_handle(&actor_ptrs, actor_idx),
                        op_inputs[1] == "1",
                        op_inputs[2] == "1",
                    )
                };
                assert_eq!(t[result_at], "-", "SetActorZone result: {line}");
            }
            "far_move" => {
                let moved = far_actor.expect("far_move actor index");
                assert!((0..6).contains(&moved), "far_move actor index: {line}");
                let dest = v(&op_inputs[0..3]);
                let test = op_inputs[3] == "1";
                let no_check = op_inputs[4] == "1";
                let moved_ok = unsafe {
                    levact::far_move_actor(
                        ctx,
                        actor_handle(&actor_ptrs, moved),
                        dest,
                        test,
                        no_check,
                    )
                };
                assert_eq!(
                    moved_ok,
                    t[result_at].parse::<i32>().unwrap() != 0,
                    "FarMoveActor return: {line}"
                );
            }
            _ => unreachable!("validated actor operation"),
        }
        let output_count_at = match op {
            "move_actor" => result_at + 10,
            "check_encroachment" => result_at + 1,
            "find_spot" => result_at + 4,
            "set_actor_zone" => result_at + 1,
            "far_move" => result_at + 1,
            _ => unreachable!(),
        };
        let final_count: usize = t[output_count_at].parse().expect("final actor count");
        assert_eq!(final_count, actor_count, "final actor count: {line}");
        let mut final_at = output_count_at + 1;
        for (index, initial) in initial_rows.iter().copied().enumerate() {
            let (expected, next) = actor_row(&t, final_at);
            assert_eq!(expected.class, initial.class, "actor class: {line}");
            assert_actor_row(expected, &objects[index], &actor_ptrs, line);
            final_at = next;
        }
        assert_eq!(final_at, bar, "state/hook boundary: {line}");
        if let Some(hooks) = mvcenter_hooks {
            assert!(hooks.len() >= 3, "mvcenter hook count: {line}");
            assert_eq!(
                [
                    hooks[0][0].as_str(),
                    hooks[1][0].as_str(),
                    hooks[2][0].as_str()
                ],
                ["GP", "CB", "LC"],
                "mvcenter callback order: {line}"
            );
            let actor_idx: usize = op_inputs[0].parse().unwrap();
            let trace_start = v(&[&hooks[2][4], &hooks[2][5], &hooks[2][6]]);
            let actor_start = initial_rows[actor_idx].location;
            assert_ne!(
                [
                    trace_start.x.to_bits(),
                    trace_start.y.to_bits(),
                    trace_start.z.to_bits()
                ],
                [
                    actor_start.x.to_bits(),
                    actor_start.y.to_bits(),
                    actor_start.z.to_bits()
                ],
                "mvcenter must trace from primitive center, not actor location: {line}"
            );
            assert_eq!(
                hooks.len(),
                expected_hook_count,
                "mvcenter event count: {line}"
            );
            assert_eq!(
                final_hit.time.to_bits(),
                f(t[result_at + 1]).to_bits(),
                "mvcenter Hit.Time: {line}"
            );
        }
        ACTOR_OP_WORLD.with(|world| {
            let world = world.borrow();
            let world = world.as_ref().unwrap();
            assert_eq!(
                world.next_hook,
                world.hooks.len(),
                "unconsumed hooks: {line}"
            );
        });
    }

    assert_eq!(
        op_counts,
        [7, 2, 4, 5, 2],
        "expected 7 MoveActor, 2 CheckEncroachment, 4 FindSpot, 5 SetActorZone, 2 FarMoveActor records; observed branches {branch_counts:?}"
    );
    assert_eq!(
        op_counts.iter().sum::<usize>(),
        20,
        "total actor-op fixture records"
    );
    let required_branches = [
        "move_actor/static",
        "move_actor/zero_same",
        "move_actor/zero_rotate",
        "move_actor/no_collision",
        "move_actor/mvcenter",
        "move_actor/self_hit",
        "move_actor/based_move",
        "check_encroachment/skip",
        "check_encroachment/nonblocking",
        "find_spot/zero_fit",
        "find_spot/zero_block",
        "find_spot/assume_fit",
        "find_spot/adjust_attempt",
        "set_actor_zone/delete",
        "set_actor_zone/level_actor",
        "set_actor_zone/zone_same",
        "set_actor_zone/pawn_regions",
        "set_actor_zone/force_refresh_same",
        "far_move/checked",
        "far_move/no_check",
    ];
    for branch in required_branches {
        assert_eq!(
            branch_counts.get(branch).copied().unwrap_or(0),
            1,
            "expected exactly one record for branch {branch}; observed {branch_counts:?}"
        );
    }
}
