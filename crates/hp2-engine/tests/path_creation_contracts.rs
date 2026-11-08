//! Contracts for path placement, live actor-array mutation and wall traversal.
use hp2_abi::{
    FVector,
    engine::generated::{AActor, ANavigationPoint, APawn, AZoneInfo, FCheckResult, ULevel},
};
use hp2_engine::level_spawn_actor::Hp2SpawnActorContext;
use hp2_engine::path::{self, Hp2PathCreationServices, Hp2PathScoutServices, PathScoutThrown};
use std::{
    cell::RefCell,
    ffi::c_void,
    ops::{Deref, DerefMut},
    ptr,
};

#[path = "pawn_moves/publish.rs"]
mod pawn_moves;
#[path = "level_spawn_actor/mock.rs"]
mod level_spawn_actor_mock;
#[path = "actor_services/mock.rs"]
mod actor_services_mock;

fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn xyz(v: FVector) -> [f32; 3] {
    [v.x, v.y, v.z]
}
fn node(x: f32) -> Box<ANavigationPoint> {
    let mut n: Box<ANavigationPoint> = Box::new(unsafe { std::mem::zeroed() });
    n.base.location = v(x, 0.0, 0.0);
    n
}
/// Scout pawn plus the dry zero-gravity zone the real `walkMove` reads.
struct Scout {
    pawn: Box<APawn>,
    _zone: Box<AZoneInfo>,
    _level: Box<ULevel>,
}
impl Deref for Scout {
    type Target = APawn;
    fn deref(&self) -> &APawn {
        &self.pawn
    }
}
impl DerefMut for Scout {
    fn deref_mut(&mut self) -> &mut APawn {
        &mut self.pawn
    }
}
fn pawn() -> Scout {
    let mut zone: Box<AZoneInfo> = Box::new(unsafe { std::mem::zeroed() });
    let mut level: Box<ULevel> = Box::new(unsafe { std::mem::zeroed() });
    let mut p: Box<APawn> = Box::new(unsafe { std::mem::zeroed() });
    p.base.collision_height = 39.0;
    p.base.collision_radius = 18.0;
    p.max_step_height = 24.0;
    p.base.region.zone = ptr::from_mut(&mut *zone).cast();
    p.base.x_level = ptr::from_mut(&mut *level).cast();
    Scout {
        pawn: p,
        _zone: zone,
        _level: level,
    }
}
#[derive(Default)]
struct World {
    actors: Vec<*mut c_void>,
    pawns: Vec<*mut c_void>,
    ordinary_actors: Vec<*mut c_void>,
    auto_queries: Vec<*mut c_void>,
    auto: Vec<*mut c_void>,
    spawned: *mut c_void,
    spawn_count: usize,
    spawn_spots: Vec<[f32; 3]>,
    destroyed: Vec<*mut c_void>,
    /// Requested direction of every real `walkMove` (its first horizontal move).
    walks: Vec<[f32; 3]>,
    traces: usize,
    /// Floor is open in every direction after the initial wall probe; otherwise
    /// every walk is blocked by a wall with a low ceiling.
    open_after_probe: bool,
    walk_open: bool,
    stepping_up: bool,
    fail_walk: bool,
    fail_destroy: bool,
    fail_log: Option<i32>,
    logs: Vec<(i32, f32)>,
}
thread_local! { static WORLD: RefCell<World> = RefCell::new(World::default()); }
fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|s| f(&mut s.borrow_mut()))
}
fn reset() {
    with(|w| *w = World::default());
}
unsafe extern "C" fn count(_: *mut c_void) -> i32 {
    with(|w| w.actors.len() as i32)
}
unsafe extern "C" fn at(_: *mut c_void, i: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = with(|w| w.actors[i as usize]);
    }
    0
}
unsafe extern "C" fn is_a(a: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    unsafe {
        *out = with(|w| match kind {
            0 => !w.pawns.contains(&a) && !w.ordinary_actors.contains(&a),
            9 => w.pawns.contains(&a),
            _ => false,
        }) as i32;
    }
    0
}
unsafe extern "C" fn class(_: *mut *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn scout_spawn(_: *const Hp2SpawnActorContext, _: usize, _: *mut usize) -> i32 {
    1
}
unsafe extern "C" fn zone(_: *mut c_void, _: *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn far(
    _: *mut c_void,
    a: *mut c_void,
    dest: *const c_void,
    _: i32,
    _: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        (*a.cast::<AActor>()).location = *dest.cast::<FVector>();
        *out = 1;
    }
    0
}
unsafe extern "C" fn move_actor(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: *mut c_void,
) -> i32 {
    1
}
unsafe extern "C" fn reach(_: *mut c_void, _: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = 1;
    }
    0
}
/// Scripted collision for the real `walkMove`: down moves land at once on flat
/// floor, up moves hit the ceiling, and a horizontal move either covers its full
/// delta or stops at a wall. The step-up retry (a horizontal move right after
/// an up move) belongs to the same walk.
unsafe extern "C" fn pawn_move(
    _: *mut c_void,
    a: *mut c_void,
    delta: *const c_void,
    _: *const c_void,
    hit: *mut c_void,
    out: *mut i32,
) -> i32 {
    let delta = unsafe { *delta.cast::<FVector>() };
    with(|w| {
        let horizontal = delta.z == 0.0;
        if horizontal && !w.stepping_up {
            w.walks.push(xyz(delta));
            if w.fail_walk {
                return 1;
            }
            w.walk_open = w.open_after_probe && w.walks.len() > 1;
        }
        w.stepping_up = delta.z > 0.0;
        unsafe {
            let hit = &mut *hit.cast::<FCheckResult>();
            if horizontal && w.walk_open {
                let a = &mut *a.cast::<AActor>();
                a.location.x += delta.x;
                a.location.y += delta.y;
                hit.time = 1.0;
            } else {
                hit.time = 0.0;
                hit.normal = if horizontal {
                    v(-1.0, 0.0, 0.0)
                } else {
                    v(0.0, 0.0, -delta.z.signum())
                };
            }
            *out = 1;
        }
        0
    })
}
unsafe extern "C" fn pawn_far(
    _: *mut c_void,
    a: *mut c_void,
    dest: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        (*a.cast::<AActor>()).location = *dest.cast::<FVector>();
        *out = 1;
    }
    0
}
unsafe extern "C" fn trace(
    _: *mut c_void,
    _: *mut c_void,
    hit: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    _: *const c_void,
) -> i32 {
    with(|w| w.traces += 1);
    unsafe {
        (*hit.cast::<FCheckResult>()).time = 0.0;
        (*hit.cast::<FCheckResult>()).normal = v(1.0, 0.0, 0.0);
    }
    0
}
unsafe extern "C" fn failed(_: i32) -> i32 {
    1
}
unsafe extern "C" fn spawn(_: *mut c_void, spot: *const c_void, out: *mut *mut c_void) -> i32 {
    with(|w| {
        let spot = unsafe { *spot.cast::<FVector>() };
        w.spawn_count += 1;
        w.spawn_spots.push(xyz(spot));
        unsafe {
            *out = w.spawned;
            if !w.spawned.is_null() {
                (*w.spawned.cast::<AActor>()).location = spot;
                w.actors.push(w.spawned);
            }
        }
    });
    0
}
unsafe extern "C" fn destroy(_: *mut c_void, a: *mut c_void) -> i32 {
    with(|w| {
        w.destroyed.push(a);
        if w.fail_destroy {
            return 1;
        }
        for entry in &mut w.actors {
            if *entry == a {
                *entry = ptr::null_mut();
            }
        }
        unsafe {
            (*a.cast::<AActor>()).set_b_delete_me(true);
        }
        0
    })
}
unsafe extern "C" fn auto(a: *mut c_void) -> i32 {
    with(|w| {
        w.auto_queries.push(a);
        w.auto.contains(&a) as i32
    })
}
unsafe extern "C" fn set_auto(a: *mut c_void) {
    with(|w| w.auto.push(a));
}
unsafe extern "C" fn one_way(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn rotation(_: *mut c_void, _: *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn line(
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    out: *mut i32,
) -> i32 {
    unsafe {
        *out = 1;
    }
    0
}
unsafe extern "C" fn log(code: i32, _: *mut c_void, _: *mut c_void, x: f32, _: f32, _: f32) -> i32 {
    with(|w| {
        w.logs.push((code, x));
        (w.fail_log == Some(code)) as i32
    })
}
fn services() -> Hp2PathScoutServices {
    pawn_moves::publish(pawn_move, pawn_far);
    level_spawn_actor_mock::publish(scout_spawn);
    actor_services_mock::publish();
    Hp2PathScoutServices {
        actor_count: count,
        actor_at: at,
        actor_is_a: is_a,
        find_scout_class: class,
        set_actor_zone: zone,
        far_move_actor: far,
        move_actor,
        point_reachable: reach,
        single_line_check: trace,
        placement_failed: failed,
    }
}
fn creation() -> Hp2PathCreationServices {
    Hp2PathCreationServices {
        spawn_path: spawn,
        destroy_actor: destroy,
        auto_built: auto,
        set_auto_built: set_auto,
        one_way,
        rotation_vector: rotation,
        fast_line: line,
        diagnostic: log,
    }
}

#[test]
fn placement_adjusts_height_and_only_initializes_owned_lists() {
    reset();
    let mut scout = pawn();
    let mut added = node(0.0);
    added.pruned_paths.fill(73);
    added.set_b_end_point(true);
    with(|w| w.spawned = ptr::from_mut(&mut *added).cast());
    let result = unsafe {
        path::new_path(
            &creation(),
            ptr::null_mut(),
            &mut *scout,
            v(10.0, 20.0, 30.0),
        )
    }
    .unwrap();
    assert_eq!(result, ptr::from_mut(&mut *added));
    assert_eq!(xyz(added.base.location), [10.0, 20.0, 39.0]);
    assert_eq!(added.paths, [-1; 16]);
    assert_eq!(added.upstream_paths, [-1; 16]);
    assert_eq!(added.pruned_paths, [73; 16]);
    assert!(added.b_end_point());
    assert!(with(|w| w.auto.contains(&result.cast())));
    scout.base.collision_height = 60.0;
    unsafe { path::new_path(&creation(), ptr::null_mut(), &mut *scout, v(0.0, 0.0, 5.0)) }.unwrap();
    assert_eq!(added.base.location.z, 5.0);
}

#[test]
fn spawn_diagnostic_failure_stops_before_marking_and_initialization() {
    reset();
    let mut scout = pawn();
    let mut added = node(0.0);
    added.paths.fill(27);
    with(|w| {
        w.spawned = ptr::from_mut(&mut *added).cast();
        w.fail_log = Some(1);
    });
    assert_eq!(
        unsafe { path::new_path(&creation(), ptr::null_mut(), &mut *scout, v(0.0, 0.0, 0.0)) },
        Err(PathScoutThrown(1))
    );
    assert_eq!(added.paths, [27; 16]);
    assert!(with(|w| w.auto.is_empty()));
}

#[test]
fn merge_keeps_manual_node_moves_average_and_restores_pawn_collision() {
    reset();
    let mut scout = pawn();
    let mut automatic = node(0.0);
    let mut manual = node(100.0);
    automatic.visited_weight = 42;
    automatic.set_b_end_point(true);
    let a = ptr::from_mut(&mut *automatic).cast();
    let m = ptr::from_mut(&mut *manual).cast();
    let p = ptr::from_mut(&mut *scout).cast();
    with(|w| {
        w.actors = vec![a, m, p];
        w.auto.push(a);
        w.pawns.push(p);
    });
    let count =
        unsafe { path::create_paths(&services(), &creation(), ptr::null_mut(), &mut *scout, 0) }
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(manual.base.location.x, 50.0);
    assert!(automatic.base.b_delete_me());
    assert_eq!(automatic.visited_weight, 1);
    assert!(!automatic.b_end_point());
    assert_eq!(with(|w| w.destroyed.clone()), vec![a]);
    assert!(unsafe {
        (*p.cast::<AActor>()).b_collide_actors()
            && (*p.cast::<AActor>()).b_block_actors()
            && (*p.cast::<AActor>()).b_block_players()
    });
}

#[test]
fn failed_destroy_stops_before_average_and_collision_restore() {
    reset();
    let mut scout = pawn();
    let mut automatic = node(0.0);
    let mut manual = node(100.0);
    let a = ptr::from_mut(&mut *automatic).cast();
    let m = ptr::from_mut(&mut *manual).cast();
    with(|w| {
        w.actors = vec![a, m];
        w.auto.push(a);
        w.fail_destroy = true;
    });
    assert_eq!(
        unsafe { path::create_paths(&services(), &creation(), ptr::null_mut(), &mut *scout, 0) },
        Err(PathScoutThrown(1))
    );
    assert_eq!(manual.base.location.x, 100.0);
    assert!(
        !automatic.base.b_collide_actors()
            && !automatic.base.b_block_actors()
            && !automatic.base.b_block_players()
            && !manual.base.b_collide_actors()
            && !manual.base.b_block_actors()
            && !manual.base.b_block_players()
    );
    assert!(!with(|w| w.logs.iter().any(|(code, _)| *code == 6)));
}

#[test]
fn intermediate_spawn_is_seen_by_live_nested_actor_iteration() {
    reset();
    let mut scout = pawn();
    let mut first = node(0.0);
    let mut second = node(800.0);
    let mut middle = node(0.0);
    with(|w| {
        w.actors = vec![
            ptr::from_mut(&mut *first).cast(),
            ptr::from_mut(&mut *second).cast(),
        ];
        w.spawned = ptr::from_mut(&mut *middle).cast();
    });
    assert_eq!(
        unsafe { path::create_paths(&services(), &creation(), ptr::null_mut(), &mut *scout, 99) }
            .unwrap(),
        1
    );
    assert_eq!(xyz(middle.base.location), [400.0, 0.0, 9.0]);
    assert_eq!(with(|w| w.spawn_count), 1);
    assert!(with(|w| w.logs.iter().any(|(code, _)| *code == 9)));
    assert_eq!(middle.paths, [-1; 16]);
}

#[test]
fn right_turn_traversal_spawns_once_and_stops_after_101_turns() {
    reset();
    let mut scout = pawn();
    let mut added = node(0.0);
    with(|w| {
        w.spawned = ptr::from_mut(&mut *added).cast();
        w.open_after_probe = true;
    });
    unsafe {
        path::pass2_from(
            &services(),
            &creation(),
            ptr::null_mut(),
            &mut *scout,
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            1.0,
        )
    }
    .unwrap();
    assert_eq!(with(|w| w.spawn_count), 1);
    assert_eq!(with(|w| w.walks.len()), 102); // initial wall probe, then 101 right turns
    let directions = with(|w| w.walks[1..5].to_vec());
    assert_eq!(
        directions,
        vec![
            [16.0, 0.0, 0.0],
            [0.0, 16.0, 0.0],
            [-16.0, 0.0, 0.0],
            [-16.0, 0.0, 0.0]
        ]
    );
    assert_eq!(with(|w| w.logs.last().copied()), Some((17, 3000.0)));
}

#[test]
fn blocked_traversal_stops_at_total_step_limit_without_spawning() {
    reset();
    let mut scout = pawn();
    unsafe {
        path::pass2_from(
            &services(),
            &creation(),
            ptr::null_mut(),
            &mut *scout,
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            -1.0,
        )
    }
    .unwrap();
    assert_eq!(with(|w| w.walks.len()), 403); // initial probe + two walks per iteration
    assert_eq!(with(|w| w.spawn_count), 0);
    assert_eq!(with(|w| w.logs.last().copied()), Some((17, 2000.0)));
}

#[test]
fn failed_initial_walk_preserves_mark_reset_and_stops_all_tracing() {
    reset();
    let mut scout = pawn();
    let mut anchor = node(0.0);
    anchor.visited_weight = 9;
    anchor.set_b_end_point(true);
    with(|w| {
        w.actors.push(ptr::from_mut(&mut *anchor).cast());
        w.fail_walk = true;
    });
    assert_eq!(
        unsafe {
            path::pass2_from(
                &services(),
                &creation(),
                ptr::null_mut(),
                &mut *scout,
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                1.0,
            )
        },
        Err(PathScoutThrown(1))
    );
    assert_eq!(anchor.visited_weight, 1);
    assert!(!anchor.b_end_point());
    assert_eq!(
        with(|w| (w.walks.len(), w.traces, w.spawn_count)),
        (1, 0, 0)
    );
}

#[test]
fn null_spawn_is_a_normal_failure_without_initialization() {
    reset();
    let mut scout = pawn();
    let result =
        unsafe { path::new_path(&creation(), ptr::null_mut(), &mut *scout, v(1.0, 2.0, 3.0)) }
            .unwrap();
    assert!(result.is_null());
    assert!(with(|w| w.auto.is_empty()));
    assert_eq!(with(|w| w.logs.clone()), vec![(0, 0.0)]);
}

#[test]
fn ordinary_actor_never_reads_navigation_only_auto_built_field() {
    reset();
    let mut scout = pawn();
    let mut ordinary: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
    let actor = ptr::from_mut(&mut *ordinary).cast();
    with(|w| {
        w.actors.push(actor);
        w.ordinary_actors.push(actor);
    });
    assert_eq!(
        unsafe { path::create_paths(&services(), &creation(), ptr::null_mut(), &mut *scout, 0,) }
            .unwrap(),
        0
    );
    assert!(with(|w| w.auto_queries.is_empty()));
    assert!(with(|w| w.destroyed.is_empty()));
    assert!(
        !ordinary.b_collide_actors() && !ordinary.b_block_actors() && !ordinary.b_block_players()
    );
}
