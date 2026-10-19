//! Replays the original-C++ oracles from `Build/levact_fixture_harness/` against
//! `hp2_engine::levact`.
//!
//! - `fixtures/levact_rotation.txt`: the four `FindDeltaFromRotation*` helpers
//!   over 2000 seeded inputs each. Each record is
//!   `<op> <origin xyz> <point xyz> <i0> <i1> <i2> -> <out xyz>`.
//! - `fixtures/levact_checks.txt`: `SinglePointCheck`, `MultiPointCheck`,
//!   `SingleLineCheck`, and `MultiLineCheck` over 1500 seeded cases each. The
//!   actor hash, the model, and the actor tables are scripted by the pure
//!   functions mirrored below. Record layout is documented in the harness.
//!
//! Floats and ints are hex bit patterns, so every comparison is bit-exact.
//! Actor fields in check records are pool indices (`-1` NULL).

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::FCheckResult;
use hp2_engine::levact::{self, LevActServices, LevelCtx, LineQuery};

const FIXTURE: &str = include_str!("fixtures/levact_rotation.txt");
const CHECK_FIXTURE: &str = include_str!("fixtures/levact_checks.txt");

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

        let ctx = LevelCtx {
            svc: &svc,
            gmem: token(GMEM_SLOT),
            hash: token(HASH_SLOT),
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
