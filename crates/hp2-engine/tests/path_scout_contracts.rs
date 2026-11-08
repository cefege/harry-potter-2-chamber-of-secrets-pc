//! Consumer contracts for the active scout placement and reach-query policy.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, ULevel};
use hp2_engine::level_spawn_actor::Hp2SpawnActorContext;
use hp2_engine::path::{self, Hp2PathScoutServices, PathScoutThrown};

#[path = "pawn_moves/publish.rs"]
mod pawn_moves;
#[path = "level_spawn_actor/mock.rs"]
mod level_spawn_actor_mock;
#[path = "actor_services/mock.rs"]
mod actor_services_mock;

fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn xyz(value: FVector) -> [f32; 3] {
    [value.x, value.y, value.z]
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
    let mut pawn: Box<APawn> = Box::new(unsafe { std::mem::zeroed() });
    pawn.base.collision_height = 39.0;
    pawn.base.collision_radius = 18.0;
    pawn.max_step_height = 24.0;
    pawn.base.region.zone = ptr::from_mut(&mut *zone).cast();
    pawn.base.x_level = ptr::from_mut(&mut *level).cast();
    Scout {
        pawn,
        _zone: zone,
        _level: level,
    }
}
fn hit(time: f32, normal: FVector) -> FCheckResult {
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = time;
    hit.normal = normal;
    hit
}

#[derive(Debug, PartialEq)]
enum Event {
    ActorAt(i32),
    IsA(usize, i32),
    FindClass,
    Spawn(usize),
    Collision(usize),
    Zone(usize, bool),
    Far([f32; 3], i32, i32),
    Move([f32; 3]),
    Size(f32, f32),
    Pass2([f32; 3], [f32; 3], f32),
    /// Real `walkMove` collision move / undo move (`APawn` pawn services).
    PawnMove([f32; 3]),
    PawnFar([f32; 3]),
    Trace([f32; 3], [f32; 3], [f32; 3]),
    Extent,
    Failed(i32),
    Reach([f32; 3], u8),
}

#[derive(Default)]
struct Fixture {
    actors: Vec<*mut c_void>,
    classes: Vec<(usize, Vec<i32>)>,
    spawned: *mut c_void,
    events: Vec<Event>,
    far_results: VecDeque<i32>,
    moves: VecDeque<FCheckResult>,
    traces: VecDeque<FCheckResult>,
    /// Scripted hits for the real `walkMove`; default is floor right below.
    pawn_moves: VecDeque<FCheckResult>,
    throw_move: bool,
    reach_result: i32,
    throw_reach: bool,
}
thread_local! { static FIXTURE: RefCell<Fixture> = RefCell::new(Fixture::default()); }
fn reset() {
    FIXTURE.with(|f| *f.borrow_mut() = Fixture::default());
}
fn setup(f: impl FnOnce(&mut Fixture)) {
    FIXTURE.with(|state| f(&mut state.borrow_mut()));
}
fn events() -> Vec<Event> {
    FIXTURE.with(|f| std::mem::take(&mut f.borrow_mut().events))
}

unsafe extern "C" fn actor_count(_: *mut c_void) -> i32 {
    FIXTURE.with(|f| f.borrow().actors.len() as i32)
}
unsafe extern "C" fn actor_at(_: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 {
    setup(|f| {
        f.events.push(Event::ActorAt(index));
        unsafe {
            *out = f.actors[index as usize];
        }
    });
    0
}
unsafe extern "C" fn is_a(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    setup(|f| {
        f.events.push(Event::IsA(actor as usize, kind));
        unsafe {
            *out = f
                .classes
                .iter()
                .any(|(p, kinds)| *p == actor as usize && kinds.contains(&kind))
                as i32;
        }
    });
    0
}
unsafe extern "C" fn find_class(out: *mut *mut c_void) -> i32 {
    setup(|f| f.events.push(Event::FindClass));
    unsafe {
        *out = ptr::without_provenance_mut(7);
    }
    0
}
unsafe extern "C" fn spawn(ctx: *const Hp2SpawnActorContext, _: usize, out: *mut usize) -> i32 {
    setup(|f| {
        let class = unsafe { (*ctx).class };
        f.events.push(Event::Spawn(class as usize));
        unsafe {
            *out = f.spawned as usize;
        }
    });
    0
}
unsafe extern "C" fn collision(actor: *mut c_void) -> i32 {
    setup(|f| f.events.push(Event::Collision(actor as usize)));
    0
}
unsafe extern "C" fn zone(_: *mut c_void, actor: *mut c_void) -> i32 {
    setup(|f| {
        f.events.push(Event::Zone(actor as usize, unsafe {
            (*actor.cast::<AActor>()).b_collide_world()
        }))
    });
    0
}
unsafe extern "C" fn far_move(
    _: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    test: i32,
    no_check: i32,
    out: *mut i32,
) -> i32 {
    let dest = unsafe { *dest.cast::<FVector>() };
    setup(|f| {
        f.events.push(Event::Far(xyz(dest), test, no_check));
        let result = f.far_results.pop_front().unwrap_or(1);
        unsafe {
            *out = result;
            if result != 0 {
                (*actor.cast::<APawn>()).base.location = dest;
            }
        }
    });
    0
}
unsafe extern "C" fn move_actor(
    _: *mut c_void,
    actor: *mut c_void,
    delta: *const c_void,
    out: *mut c_void,
) -> i32 {
    let delta = unsafe { *delta.cast::<FVector>() };
    let mut status = 0;
    setup(|f| {
        f.events.push(Event::Move(xyz(delta)));
        if f.throw_move {
            status = 1;
            return;
        }
        let hit = f
            .moves
            .pop_front()
            .unwrap_or_else(|| hit(0.0, v(0.0, 0.0, 1.0)));
        unsafe {
            let location = &mut (*actor.cast::<APawn>()).base.location;
            location.x += delta.x * hit.time;
            location.y += delta.y * hit.time;
            location.z += delta.z * hit.time;
            *out.cast::<FCheckResult>() = hit;
        }
    });
    status
}
unsafe extern "C" fn size(actor: *mut c_void, radius: f32, height: f32) -> i32 {
    setup(|f| f.events.push(Event::Size(radius, height)));
    unsafe {
        (*actor.cast::<APawn>()).base.collision_radius = radius;
        (*actor.cast::<APawn>()).base.collision_height = height;
    }
    0
}
unsafe extern "C" fn reach(actor: *mut c_void, end: *const c_void, out: *mut i32) -> i32 {
    let mut status = 0;
    setup(|f| {
        f.events.push(Event::Reach(
            xyz(unsafe { *end.cast::<FVector>() }),
            unsafe { (*actor.cast::<APawn>()).base.physics },
        ));
        unsafe {
            *out = f.reach_result;
        }
        if f.throw_reach {
            status = 1;
        }
    });
    status
}
unsafe extern "C" fn pass2(
    _: *mut c_void,
    start: *const c_void,
    dir: *const c_void,
    turn: f32,
) -> i32 {
    setup(|f| {
        f.events.push(Event::Pass2(
            xyz(unsafe { *start.cast::<FVector>() }),
            xyz(unsafe { *dir.cast::<FVector>() }),
            turn,
        ))
    });
    0
}
unsafe extern "C" fn pawn_move(
    _: *mut c_void,
    actor: *mut c_void,
    delta: *const c_void,
    _: *const c_void,
    out: *mut c_void,
    moved: *mut i32,
) -> i32 {
    let delta = unsafe { *delta.cast::<FVector>() };
    setup(|f| {
        f.events.push(Event::PawnMove(xyz(delta)));
        let hit = f
            .pawn_moves
            .pop_front()
            .unwrap_or_else(|| hit(0.0, v(0.0, 0.0, 1.0)));
        unsafe {
            let location = &mut (*actor.cast::<APawn>()).base.location;
            location.x += delta.x * hit.time;
            location.y += delta.y * hit.time;
            location.z += delta.z * hit.time;
            *out.cast::<FCheckResult>() = hit;
            *moved = 1;
        }
    });
    0
}
unsafe extern "C" fn pawn_far(
    _: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
    out: *mut i32,
) -> i32 {
    let dest = unsafe { *dest.cast::<FVector>() };
    setup(|f| f.events.push(Event::PawnFar(xyz(dest))));
    unsafe {
        (*actor.cast::<APawn>()).base.location = dest;
        *out = 1;
    }
    0
}
unsafe extern "C" fn trace(
    _: *mut c_void,
    _: *mut c_void,
    out: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    setup(|f| {
        f.events.push(Event::Trace(
            xyz(unsafe { *end.cast::<FVector>() }),
            xyz(unsafe { *start.cast::<FVector>() }),
            xyz(unsafe { *extent.cast::<FVector>() }),
        ));
        unsafe {
            *out.cast::<FCheckResult>() = f
                .traces
                .pop_front()
                .unwrap_or_else(|| hit(1.0, v(0.0, 0.0, 0.0)));
        }
    });
    0
}
unsafe extern "C" fn extent(_: *mut c_void, out: *mut c_void) -> i32 {
    setup(|f| f.events.push(Event::Extent));
    unsafe {
        *out.cast::<FVector>() = v(18.0, 18.0, 39.0);
    }
    0
}
unsafe extern "C" fn failed(reason: i32) -> i32 {
    setup(|f| f.events.push(Event::Failed(reason)));
    0
}
fn services() -> Hp2PathScoutServices {
    pawn_moves::publish(pawn_move, pawn_far);
    level_spawn_actor_mock::publish(spawn);
    actor_services_mock::publish();
    Hp2PathScoutServices {
        actor_count,
        actor_at,
        actor_is_a: is_a,
        find_scout_class: find_class,
        set_actor_zone: zone,
        far_move_actor: far_move,
        move_actor,
        point_reachable: reach,
        single_line_check: trace,
        placement_failed: failed,
    }
}

#[test]
fn reuse_selects_last_matching_scout_including_deleted_scout_without_spawning() {
    reset();
    let mut first = pawn();
    let mut last = pawn();
    last.base.set_b_delete_me(true);
    let first_ptr = ptr::from_mut(&mut *first).cast::<c_void>();
    let last_ptr = ptr::from_mut(&mut *last).cast::<c_void>();
    setup(|f| {
        f.actors = vec![first_ptr, ptr::null_mut(), last_ptr];
        f.classes = vec![(first_ptr as usize, vec![7]), (last_ptr as usize, vec![7])];
    });
    let mut scout = ptr::null_mut();
    unsafe {
        path::get_scout(&services(), ptr::null_mut(), &mut scout).unwrap();
    }
    assert_eq!(scout, ptr::from_mut(&mut *last));
    assert_eq!(
        events(),
        vec![
            Event::ActorAt(0),
            Event::IsA(first_ptr as usize, 7),
            Event::ActorAt(1),
            Event::ActorAt(2),
            Event::IsA(last_ptr as usize, 7),
            Event::Zone(last_ptr as usize, true),
        ]
    );
}

#[test]
fn missing_scout_resolves_class_then_spawns_and_configures_collision_before_zone() {
    reset();
    let mut spawned = pawn();
    let actor = ptr::from_mut(&mut *spawned).cast::<c_void>();
    setup(|f| f.spawned = actor);
    let mut scout = ptr::without_provenance_mut(1);
    unsafe {
        path::get_scout(&services(), ptr::null_mut(), &mut scout).unwrap();
    }
    assert_eq!(scout.cast::<c_void>(), actor);
    assert_eq!(
        events(),
        vec![
            Event::FindClass,
            Event::Spawn(7),
            Event::Zone(actor as usize, true)
        ]
    );
}

#[test]
fn floor_found_on_iteration_49_succeeds_but_iteration_50_still_fails() {
    for floor_iteration in [49, 50] {
        reset();
        let mut scout = pawn();
        setup(|f| {
            for _ in 1..floor_iteration {
                f.moves.push_back(hit(1.0, v(0.0, 0.0, 0.0)));
            }
            f.moves.push_back(hit(0.0, v(0.0, 0.0, 0.7)));
        });
        let result = unsafe {
            path::find_scout_start(&services(), ptr::null_mut(), &mut *scout, v(1.0, 2.0, 3.0))
                .unwrap()
        };
        assert_eq!(result, (floor_iteration == 49) as i32);
        let calls = events();
        assert_eq!(
            calls.iter().filter(|e| matches!(e, Event::Move(_))).count(),
            floor_iteration
        );
        assert_eq!(
            calls
                .iter()
                .filter(|e| matches!(e, Event::Failed(0)))
                .count(),
            (floor_iteration == 50) as usize
        );
    }
}

#[test]
fn floor_slide_uses_remaining_time_then_two_wall_adjust_before_third_move() {
    reset();
    let mut scout = pawn();
    setup(|f| {
        f.moves.push_back(hit(0.5, v(1.0, 0.0, 0.0)));
        f.moves.push_back(hit(0.5, v(0.0, 1.0, 0.0)));
        f.moves.push_back(hit(0.0, v(0.0, 0.0, 1.0)));
    });
    assert_eq!(
        unsafe {
            path::find_scout_start(&services(), ptr::null_mut(), &mut *scout, v(0.0, 0.0, 0.0))
                .unwrap()
        },
        1
    );
    assert_eq!(
        events(),
        vec![
            Event::Far([0.0, 0.0, 0.0], 0, 0),
            Event::Move([0.0, 0.0, -50.0]),
            Event::Move([0.0, 0.0, -25.0]),
            Event::Move([0.0, 0.0, -12.5])
        ]
    );
}

#[test]
fn start_fallback_attempts_original_then_plus_twenty_but_pass2_receives_original() {
    reset();
    let mut scout = pawn();
    setup(|f| f.far_results = VecDeque::from([0, 1]));
    unsafe {
        path::test_paths_from(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            ptr::null_mut(),
            v(1.0, 2.0, 3.0),
            pass2,
        )
        .unwrap();
    }
    assert_eq!(
        events(),
        vec![
            Event::Far([1.0, 2.0, 3.0], 0, 0),
            Event::Failed(1),
            Event::Far([1.0, 2.0, 23.0], 0, 0),
            Event::Move([0.0, 0.0, -50.0]),
            Event::Pass2([1.0, 2.0, 3.0], [1.0, 0.0, 0.0], 1.0),
        ]
    );
}

#[test]
fn height_mismatch_triggers_fallback_and_two_failed_placements_skip_pass2() {
    reset();
    let mut scout = pawn();
    setup(|f| f.moves.push_back(hit(1.0, v(0.0, 0.0, 1.0))));
    unsafe {
        path::test_paths_from(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            ptr::null_mut(),
            v(0.0, 0.0, 0.0),
            pass2,
        )
        .unwrap();
    }
    let calls = events();
    assert!(calls.contains(&Event::Far([0.0, 0.0, 20.0], 0, 0)));
    assert_eq!(
        calls
            .iter()
            .filter(|e| matches!(e, Event::Pass2(..)))
            .count(),
        1
    );
    reset();
    setup(|f| f.far_results = VecDeque::from([0, 0]));
    unsafe {
        path::test_paths_from(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            ptr::null_mut(),
            v(0.0, 0.0, 0.0),
            pass2,
        )
        .unwrap();
    }
    assert_eq!(
        events(),
        vec![
            Event::Far([0.0, 0.0, 0.0], 0, 0),
            Event::Failed(1),
            Event::Far([0.0, 0.0, 20.0], 0, 0),
            Event::Failed(1)
        ]
    );
}

#[test]
fn valid_original_start_does_not_try_the_fallback() {
    reset();
    let mut scout = pawn();
    unsafe {
        path::test_paths_from(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            ptr::null_mut(),
            v(0.0, 0.0, 0.0),
            pass2,
        )
        .unwrap();
    }
    assert_eq!(
        events(),
        vec![
            Event::Far([0.0, 0.0, 0.0], 0, 0),
            Event::Move([0.0, 0.0, -50.0]),
            Event::Pass2([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 1.0)
        ]
    );
}

#[test]
fn valid_node_short_circuits_identity_deletion_and_membership_filters() {
    reset();
    let mut scout = pawn();
    let actor = ptr::from_mut(&mut scout.base);
    let svc = services();
    unsafe {
        assert_eq!(
            path::valid_node(&svc, ptr::null_mut(), ptr::null_mut()).unwrap(),
            0
        );
        assert_eq!(path::valid_node(&svc, actor.cast(), actor).unwrap(), 0);
        (*actor).set_b_delete_me(true);
        assert_eq!(path::valid_node(&svc, ptr::null_mut(), actor).unwrap(), 0);
        (*actor).set_b_delete_me(false);
    }
    assert!(events().is_empty());
    assert_eq!(
        unsafe { path::valid_node(&svc, ptr::null_mut(), actor).unwrap() },
        0
    );
    assert_eq!(events(), vec![Event::IsA(actor as usize, 0)]);
    setup(|f| f.classes = vec![(actor as usize, vec![0, 1])]);
    assert_eq!(
        unsafe { path::valid_node(&svc, ptr::null_mut(), actor).unwrap() },
        0
    );
    assert_eq!(
        events(),
        vec![Event::IsA(actor as usize, 0), Event::IsA(actor as usize, 1)]
    );
    setup(|f| f.classes = vec![(actor as usize, vec![0])]);
    assert_eq!(
        unsafe { path::valid_node(&svc, ptr::null_mut(), actor).unwrap() },
        1
    );
}

#[test]
fn reach_queries_even_after_failed_start_move_and_restores_location_not_physics() {
    reset();
    let mut scout = pawn();
    scout.base.location = v(10.0, 20.0, 30.0);
    scout.base.physics = 2;
    setup(|f| {
        f.far_results.push_back(0);
        f.reach_result = 7;
    });
    let result = unsafe {
        path::test_reach(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            v(1.0, 2.0, 3.0),
            v(4.0, 5.0, 6.0),
        )
        .unwrap()
    };
    assert_eq!(result, 7);
    assert_eq!(xyz(scout.base.location), [10.0, 20.0, 30.0]);
    assert_eq!(scout.base.physics, 1);
    assert_eq!(
        events(),
        vec![
            Event::Far([1.0, 2.0, 3.0], 0, 0),
            Event::Reach([4.0, 5.0, 6.0], 1),
            Event::Far([10.0, 20.0, 30.0], 0, 1)
        ]
    );
}

#[test]
fn walk_without_floor_restores_location_and_returns_minus_one() {
    reset();
    let mut scout = pawn();
    scout.base.location = v(1.0, 2.0, 3.0);
    setup(|f| f.pawn_moves.push_back(hit(1.0, v(0.0, 0.0, 0.0))));
    let mut local_hit = hit(0.0, v(0.0, 0.0, 1.0));
    let result = unsafe {
        path::test_walk(
            &services(),
            ptr::null_mut(),
            &mut *scout,
            v(8.0, 9.0, 10.0),
            &mut local_hit,
            4.0,
            1,
        )
        .unwrap()
    };
    assert_eq!(result, -1);
    assert_eq!(xyz(scout.base.location), [1.0, 2.0, 3.0]);
    assert_eq!(
        events(),
        vec![
            Event::PawnMove([8.0, 9.0, 0.0]),
            Event::PawnMove([0.0, 0.0, -26.0]),
            Event::Trace([9.0, 11.0, -64.0], [9.0, 11.0, 3.0], [16.0, 16.0, 1.0]),
            Event::Far([1.0, 2.0, 3.0], 0, 1)
        ]
    );
}

#[test]
fn supported_walk_keeps_movement_and_non_success_skips_floor_probe() {
    let free = || hit(1.0, v(0.0, 0.0, 0.0));
    // (scripted walkMove hits, walkMove result, scout location after TestWalk)
    let cases = [
        (vec![free()], 1, [8.0, 9.0, 0.0]),          // moved onto floor
        (vec![], 0, [0.0, 0.0, 0.0]),                // wall, step-up blocked
        (vec![free(), free()], -1, [8.0, 9.0, 0.0]), // moved, then no floor
    ];
    for (pawn_moves, walk_result, location) in cases {
        reset();
        let mut scout = pawn();
        setup(|f| {
            f.pawn_moves = pawn_moves.into();
            f.traces.push_back(hit(0.5, v(0.0, 0.0, 1.0)));
        });
        let mut local_hit = hit(1.0, v(0.0, 0.0, 0.0));
        let result = unsafe {
            path::test_walk(
                &services(),
                ptr::null_mut(),
                &mut *scout,
                v(8.0, 9.0, 10.0),
                &mut local_hit,
                4.0,
                0,
            )
            .unwrap()
        };
        assert_eq!(result, walk_result);
        assert_eq!(xyz(scout.base.location), location);
        let calls = events();
        assert_eq!(
            calls
                .iter()
                .filter(|e| matches!(e, Event::Trace(..)))
                .count(),
            (walk_result == 1) as usize
        );
        assert!(!calls.iter().any(|e| matches!(e, Event::Far(..))));
    }
}

#[test]
fn blocking_normal_prioritizes_wall_then_landing_then_ledge_and_preserves_on_miss() {
    for hit_at in [0, 1, 2, 3] {
        reset();
        let mut scout = pawn();
        scout.base.location = v(100.0, 200.0, 300.0);
        setup(|f| {
            for probe in 0..3 {
                f.traces.push_back(hit(
                    if hit_at == probe { 0.5 } else { 1.0 },
                    v(0.0, 1.0, 0.0),
                ));
            }
        });
        let mut normal = v(1.0, 0.0, 0.0);
        unsafe {
            path::find_blocking_normal(&services(), ptr::null_mut(), &mut *scout, &mut normal)
                .unwrap();
        }
        assert_eq!(
            xyz(normal),
            if hit_at == 0 || hit_at == 2 {
                [0.0, 1.0, 0.0]
            } else {
                [1.0, 0.0, 0.0]
            }
        );
        let expected = vec![
            Event::Trace(
                [84.0, 200.0, 300.0],
                [100.0, 200.0, 300.0],
                [18.0, 18.0, 39.0],
            ),
            Event::Trace(
                [84.0, 200.0, 276.0],
                [84.0, 200.0, 300.0],
                [18.0, 18.0, 39.0],
            ),
            Event::Trace(
                [100.0, 200.0, 276.0],
                [84.0, 200.0, 276.0],
                [18.0, 18.0, 39.0],
            ),
        ];
        assert_eq!(
            events(),
            expected
                .into_iter()
                .take((hit_at + 1).min(3))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn captured_exceptions_stop_before_later_moves_queries_or_restoration() {
    reset();
    let mut scout = pawn();
    setup(|f| f.throw_move = true);
    assert_eq!(
        unsafe {
            path::find_scout_start(&services(), ptr::null_mut(), &mut *scout, v(0.0, 0.0, 0.0))
        },
        Err(PathScoutThrown(1))
    );
    assert_eq!(
        events(),
        vec![
            Event::Far([0.0, 0.0, 0.0], 0, 0),
            Event::Move([0.0, 0.0, -50.0])
        ]
    );
    reset();
    setup(|f| f.throw_reach = true);
    assert_eq!(
        unsafe {
            path::test_reach(
                &services(),
                ptr::null_mut(),
                &mut *scout,
                v(1.0, 2.0, 3.0),
                v(4.0, 5.0, 6.0),
            )
        },
        Err(PathScoutThrown(1))
    );
    assert_eq!(
        events(),
        vec![
            Event::Far([1.0, 2.0, 3.0], 0, 0),
            Event::Reach([4.0, 5.0, 6.0], 1)
        ]
    );
    assert_eq!(xyz(scout.base.location), [1.0, 2.0, 3.0]);
}
