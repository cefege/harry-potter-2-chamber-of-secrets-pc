//! Replays the independent C++ movement oracle over a typed, two-node floor world.
//! Generation/provenance: Build/pawn_native_move_fixture_harness/gen_pawn_native_move.sh.
//! Route/reach/sense/steering are real policies on both sides. Only world geometry,
//! reflection identities and explicitly declared script events are supplied by this fixture.
use hp2_abi::{FVector, engine::generated::{AActor, AGameInfo, AInventory, AInventorySpot, ALevelInfo, ANavigationPoint, APawn, APlayerReplicationInfo, FCheckResult, FPointRegion, FReachSpec, FRotator, FStateFrame, ULevel, UModel, UObject}};
use hp2_engine::{pawn::{Hp2PawnServices, PawnHitBody}, pawn_native_move::{self as movement, Hp2PawnNativeMoveServices}, pawn_reach::{self, Hp2PawnReachServices}, pawn_sense::{self, Hp2PawnSenseServices}, route::{self, Hp2RouteServices, Hp2SortedPathList}, services::{self, Hp2EngineServices}, vec};
use std::{cell::RefCell, ffi::c_void, mem::{size_of, zeroed, MaybeUninit}, ptr};
const FIXTURE: &str = include_str!("fixtures/pawn_native_move.txt");
const POOL: usize = 4;
const fn larger(a: usize, b: usize) -> usize { if a > b { a } else { b } }
const OBJ_BYTES: usize = larger(larger(size_of::<APawn>(), size_of::<AInventory>()), size_of::<AInventorySpot>());
#[repr(align(16))]
struct Blob([u8; OBJ_BYTES]);
struct Storage {
    actors: [Blob; POOL], level: ULevel, info: ALevelInfo, game: AGameInfo,
    model: UModel, frame: FStateFrame, specs: [FReachSpec; 2], list: [*mut AActor; 5],
}
#[derive(Default)]
struct World { actors: [usize; POOL], level: usize, info: usize, frame: usize, case: usize, random: u32, trace: Vec<String> }
thread_local! { static WORLD: RefCell<World> = RefCell::new(World::default()); }
fn with<T>(body: impl FnOnce(&mut World) -> T) -> T { WORLD.with(|world| body(&mut world.borrow_mut())) }
fn actor(index: usize) -> *mut AActor { with(|world| world.actors[index]) as *mut AActor }
fn identity(raw: *const c_void) -> Option<usize> { with(|world| world.actors.iter().position(|address| *address == raw as usize)) }
fn id(raw: *const c_void) -> String {
    if raw.is_null() { return "-".into(); }
    format!("p{}", identity(raw).expect("oracle object belongs to its declared pool"))
}
fn class(raw: *const c_void) -> u32 {
    match identity(raw) {
        Some(0) => 1, Some(1) => 2, Some(2) => 3,
        Some(3) => if with(|world| world.case & 1 != 0) { 1 } else { 4 },
        _ => 0,
    }
}
fn hexf(value: f32) -> String { format!("{:08x}", value.to_bits()) }
fn xyz(value: FVector) -> String { format!("{},{},{}", hexf(value.x), hexf(value.y), hexf(value.z)) }
fn trace(value: String) { with(|world| world.trace.push(value)); }
fn hit() -> FCheckResult { let mut value: FCheckResult = unsafe { zeroed() }; value.time = 1.; value.item = -1; value.normal.z = 1.; value }
fn floor_trace(end: FVector, start: FVector, extent: FVector) -> FCheckResult {
    let mut value = hit();
    if end.z < extent.z && end.z < start.z { value.time = ((start.z - extent.z) / (start.z - end.z)).clamp(0., 1.); }
    value
}
fn random() -> f32 {
    with(|world| { world.random = world.random.wrapping_mul(1664525).wrapping_add(1013904223); ((world.random >> 16) & 0x7fff) as f32 / 32768. })
}
unsafe extern "C" fn app_random(out: *mut f32) -> i32 { unsafe { out.write(random()); } 0 }
unsafe extern "C" fn far_move(_: *mut c_void, raw: *mut c_void, point: *const c_void, _: i32, no_check: i32, out: *mut i32) -> i32 {
    unsafe { let mut point = point.cast::<FVector>().read_unaligned(); let actor = raw.cast::<AActor>(); if no_check == 0 && point.z < (*actor).collision_height { point.z = (*actor).collision_height; } (*actor).location = point; out.write(1); } 0
}
unsafe extern "C" fn far_default(level: *mut c_void, raw: *mut c_void, point: *const c_void, out: *mut i32) -> i32 { unsafe { far_move(level, raw, point, 1, 0, out) } }
unsafe extern "C" fn move_actor(_: *mut c_void, raw: *mut c_void, delta: *const c_void, rotation: *const c_void, out_hit: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let actor = raw.cast::<AActor>(); let delta = delta.cast::<FVector>().read_unaligned();
        let extent = FVector { x: (*actor).collision_radius, y: (*actor).collision_radius, z: (*actor).collision_height };
        let result = floor_trace(vec::add((*actor).location, delta), (*actor).location, extent);
        (*actor).location = vec::add((*actor).location, vec::scale(delta, result.time));
        (*actor).rotation = rotation.cast::<FRotator>().read_unaligned();
        out_hit.cast::<FCheckResult>().write_unaligned(result); out.write(1);
    } 0
}
unsafe extern "C" fn water_line(_: *mut c_void, start: *const c_void, end: *mut c_void) -> i32 {
    unsafe { if (*end.cast::<FVector>()).z < 0. && (*start.cast::<FVector>()).z >= 0. { (*end.cast::<FVector>()).z = 0.; } } 0
}
unsafe extern "C" fn with_hit(context: *mut c_void, body: PawnHitBody) -> i32 { let mut value = hit(); unsafe { body(context, (&raw mut value).cast()) } }
unsafe extern "C" fn point_region(_: *mut c_void, _: *mut c_void, _: *const c_void, out: *mut c_void) -> i32 {
    let mut region: FPointRegion = unsafe { zeroed() }; region.zone = with(|world| world.info) as *mut c_void; region.zone_number = 1;
    unsafe { out.cast::<FPointRegion>().write_unaligned(region); } 0
}
unsafe extern "C" fn fast_line(_: *mut c_void, end: *const c_void, start: *const c_void, out: *mut i32) -> i32 {
    unsafe { out.write(i32::from((*end.cast::<FVector>()).z >= 0. && (*start.cast::<FVector>()).z >= 0.)); } 0
}
unsafe extern "C" fn fast_byte(model: *mut c_void, end: *const c_void, start: *const c_void, out: *mut u8) -> i32 {
    let mut value = 0; unsafe { fast_line(model, end, start, &raw mut value); out.write(value as u8); } 0
}
unsafe extern "C" fn single_line(_: *mut c_void, _: *mut c_void, out: *mut c_void, end: *const c_void, start: *const c_void, _: u32, extent: *const c_void) -> i32 {
    unsafe { out.cast::<FCheckResult>().write_unaligned(floor_trace(*end.cast::<FVector>(), *start.cast::<FVector>(), *extent.cast::<FVector>())); } 0
}
unsafe extern "C" fn reach_line(level: *mut c_void, source: *mut c_void, out: *mut c_void, end: *const c_void, start: *const c_void) -> i32 {
    let extent = vec::zero(); unsafe { single_line(level, source, out, end, start, 6, (&raw const extent).cast()) }
}
unsafe extern "C" fn reach_class(raw: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    let value = match kind { 0 => class(raw) == 1, 1 => class(raw) == 4, 2 | 3 => false, _ => panic!("invalid reach class selector") };
    unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn editor(out: *mut i32) -> i32 { unsafe { out.write(0); } 0 }
static REACH: Hp2PawnReachServices = Hp2PawnReachServices {
    pawn: Hp2PawnServices { move_actor, far_move_actor: far_default, find_water_line: water_line, with_hit },
    far_move_actor_flags: far_move, single_line_check: reach_line, point_region, fast_line_check: fast_line, is_a: reach_class, is_editor: editor,
};
unsafe extern "C" fn point_reachable(pawn: *mut c_void, point: *const c_void, know: i32, out: *mut i32) -> i32 {
    match unsafe { pawn_reach::point_reachable(&REACH, pawn.cast(), *point.cast::<FVector>(), know) } { Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => error }
}
unsafe extern "C" fn actor_reachable(pawn: *mut c_void, other: *mut c_void, know: i32, out: *mut i32) -> i32 {
    match unsafe { pawn_reach::actor_reachable(&REACH, pawn.cast(), other.cast(), know) } { Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => error }
}
unsafe extern "C" fn hit_actor(level: *mut c_void, source: *mut c_void, end: *const c_void, start: *const c_void, out: *mut *mut c_void) -> i32 {
    let mut value = hit(); unsafe { reach_line(level, source, (&raw mut value).cast(), end, start); out.write(value.actor); } 0
}
unsafe extern "C" fn nav_list(_: *mut c_void, out: *mut *mut c_void) -> i32 { unsafe { out.write((*((with(|world| world.info)) as *mut ALevelInfo)).navigation_point_list); } 0 }
unsafe extern "C" fn route_class(raw: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    let tag = class(raw); let value = match kind { 0 => tag == 2 || tag == 3, 1 => tag == 1, 2 => tag == 3, 3 => false, _ => panic!("invalid route class selector") };
    unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn mover_only(_: *mut c_void, _: *mut i32) -> i32 { panic!("authored floor world contains no Mover object") }
unsafe extern "C" fn special_cost(_: *mut c_void, _: *mut c_void, out: *mut i32) -> i32 { unsafe { out.write(7); } 0 }
unsafe extern "C" fn touch_probe(_: *mut c_void, out: *mut i32) -> i32 { unsafe { out.write(1); } 0 }
unsafe extern "C" fn desirability(_: *mut c_void, _: *mut c_void, out: *mut f32) -> i32 { unsafe { out.write(1.); } 0 }
unsafe extern "C" fn actor_name(raw: *mut c_void, out: *mut i32) -> i32 { unsafe { out.write(i32::from_ne_bytes((*raw.cast::<UObject>()).name)); } 0 }
unsafe extern "C" fn actor_class(raw: *mut c_void, out: *mut *mut c_void) -> i32 { unsafe { out.write(class(raw) as usize as *mut c_void); } 0 }
unsafe extern "C" fn log_actor(_: i32, raw: *mut c_void) -> i32 { assert!(identity(raw).is_some()); 0 }
unsafe extern "C" fn lists(count: i32, context: *mut c_void, run: unsafe extern "C" fn(*mut c_void, *mut Hp2SortedPathList, *mut Hp2SortedPathList) -> i32) -> i32 {
    let mut first = MaybeUninit::<Hp2SortedPathList>::uninit(); let mut second = MaybeUninit::<Hp2SortedPathList>::uninit();
    unsafe { ptr::addr_of_mut!((*first.as_mut_ptr()).num_points).write(0); ptr::addr_of_mut!((*second.as_mut_ptr()).num_points).write(0); run(context, first.as_mut_ptr(), if count == 2 { second.as_mut_ptr() } else { ptr::null_mut() }) }
}
static ROUTE: Hp2RouteServices = Hp2RouteServices {
    app_frand: app_random, far_move_actor: far_move, point_reachable, actor_reachable, model_fast_line_check: fast_byte,
    single_line_hit_actor: hit_actor, navigation_point_list: nav_list, actor_is_a: route_class, mover_player_only: mover_only,
    event_special_cost: special_cost, item_is_probing_touch: touch_probe, item_bot_desirability: desirability,
    actor_name_index: actor_name, actor_class, log_actor, lists,
};
unsafe extern "C" fn physics(raw: *mut c_void, value: u8) -> i32 { unsafe { (*raw.cast::<AActor>()).physics = value; } 0 }
unsafe extern "C" fn probing(_: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    let value = match kind { 0 => true, 1 => with(|world| world.case & 2 != 0), _ => panic!("invalid sense probe selector") }; unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn hear(pawn: *mut c_void, loudness: f32, noise: *mut c_void) -> i32 { trace(format!("hear({},{},{})", id(pawn), hexf(loudness), id(noise))); 0 }
unsafe extern "C" fn invisible(pawn: *mut c_void) -> i32 { trace(format!("invisible({})", id(pawn))); 0 }
unsafe extern "C" fn touch(raw: *mut c_void, other: *mut c_void) -> i32 { trace(format!("touch({},{})", id(raw), id(other))); 0 }
unsafe extern "C" fn normal(raw: *const c_void, out: *mut c_void) -> i32 { unsafe { out.cast::<FVector>().write_unaligned(hp2_core::math::vector_safe_normal(*raw.cast::<FVector>())); } 0 }
unsafe extern "C" fn rotation(raw: *const c_void, out: *mut c_void) -> i32 {
    let value = hp2_core::math::vector_rotation(unsafe { *raw.cast::<FVector>() });
    unsafe { out.cast::<FRotator>().write_unaligned(FRotator { pitch: value.pitch, yaw: value.yaw, roll: value.roll }); } 0
}
unsafe extern "C" fn rotation_vector(raw: *const c_void, out: *mut c_void) -> i32 {
    let value = unsafe { *raw.cast::<FRotator>() };
    unsafe { out.cast::<FVector>().write_unaligned(hp2_core::math::rotator_vector(hp2_core::math::FRotator { pitch: value.pitch, yaw: value.yaw, roll: value.roll }, hp2_core::math::trig_table())); } 0
}
unsafe extern "C" fn sense_class(raw: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    let value = match kind { 1 => class(raw) == 1, 2 => false, 3 => class(raw) == 4, _ => panic!("invalid sense class selector") }; unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn class_of(raw: *mut c_void, other: *mut c_void, out: *mut i32) -> i32 { unsafe { out.write(i32::from(class(raw) == class(other) || (class(raw) == 3 && class(other) == 2))); } 0 }
unsafe extern "C" fn pawn_head(_: *mut c_void, out: *mut *mut c_void) -> i32 { unsafe { out.write((*((with(|world| world.info)) as *mut ALevelInfo)).pawn_list); } 0 }
unsafe extern "C" fn team_game(_: *mut c_void, out: *mut i32) -> i32 { unsafe { let info = with(|world| world.info) as *mut ALevelInfo; out.write(i32::from((*(*info).game.cast::<AGameInfo>()).b_team_game())); } 0 }
unsafe extern "C" fn level_time(raw: *mut c_void, out: *mut f32) -> i32 { unsafe { out.write(hp2_core::time::get_float(i64::from_ne_bytes((*raw.cast::<ULevel>()).time_seconds))); } 0 }
unsafe extern "C" fn teams(a: *mut c_void, b: *mut c_void, out: *mut i32) -> i32 { assert!(!a.is_null() && !b.is_null()); unsafe { out.write(i32::from((*a.cast::<APlayerReplicationInfo>()).team != (*b.cast::<APlayerReplicationInfo>()).team)); } 0 }
unsafe extern "C" fn gravity(raw: *mut c_void, out: *mut f32) -> i32 { unsafe { out.write((*raw.cast::<ALevelInfo>()).base.zone_gravity.z); } 0 }
unsafe extern "C" fn find_toward(pawn: *mut c_void, goal: *mut c_void, single: i32, best: *mut *mut c_void, clear: i32, out: *mut i32) -> i32 {
    match unsafe { route::find_path_toward(&ROUTE, pawn.cast(), goal.cast(), single, best.cast(), clear) } { Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => error }
}
unsafe extern "C" fn special(raw: *mut c_void, _: *mut c_void, out: *mut *mut c_void) -> i32 { trace(format!("special({})", id(raw))); unsafe { out.write(raw); } 0 }
unsafe extern "C" fn can_hear(pawn: *mut c_void, location: *const c_void, loudness: f32, other: *mut c_void, out: *mut i32) -> i32 {
    match unsafe { pawn_sense::can_hear(&SENSE, pawn.cast(), *location.cast::<FVector>(), loudness, other.cast()) } { Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => error }
}
unsafe extern "C" fn sight(pawn: *mut c_void, other: *mut c_void, show: i32, out: *mut u32) -> i32 {
    match unsafe { pawn_sense::line_of_sight_to(&SENSE, pawn.cast(), other.cast(), show) } { Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => error }
}
static SENSE: Hp2PawnSenseServices = Hp2PawnSenseServices {
    fast_line_check: fast_line, single_line_check: single_line, app_frand: app_random, set_physics: physics, is_probing: probing,
    event_hear_noise: hear, event_enemy_not_visible: invisible, event_touch: touch, safe_normal: normal, vector_rotation: rotation,
    rotation_vector, is_a: sense_class, is_a_class_of: class_of, level_pawn_head: pawn_head, level_team_game: team_game,
    level_time, teams_differ: teams, zone_default_gravity_z: gravity, actor_reachable, find_path_toward: find_toward,
    event_special_handling: special, can_hear, line_of_sight_to: sight,
};
unsafe extern "C" fn move_class(raw: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    let value = match kind { 0 => class(raw) == 1, 1 => class(raw) == 4, 2 => matches!(class(raw), 2 | 3), _ => panic!("invalid movement class selector") };
    trace(format!("isA({},{kind})={}", id(raw), i32::from(value))); unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn move_probe(raw: *mut c_void, kind: u32, out: *mut i32) -> i32 {
    let value = match kind { 0 => with(|world| world.case & 2 != 0), 1 => true, _ => panic!("invalid movement probe selector") };
    trace(format!("probing({},{kind})={}", id(raw), i32::from(value))); unsafe { out.write(i32::from(value)); } 0
}
unsafe extern "C" fn alter(raw: *mut c_void) -> i32 { unsafe { (*raw.cast::<APawn>()).destination.x += 3.; } trace("alter()".into()); 0 }
unsafe extern "C" fn see(raw: *mut c_void, other: *mut c_void) -> i32 { trace(format!("seeplayer({},{})", id(raw), id(other))); 0 }
const MOVE: Hp2PawnNativeMoveServices = Hp2PawnNativeMoveServices { is_a: move_class, is_probing: move_probe, event_alter_destination: alter, event_see_player: see };
unsafe extern "C" fn engine_rotation(raw: *const c_void, out: *mut c_void) { unsafe { rotation_vector(raw, out); } }
unsafe extern "C" fn engine_rand(out: *mut i32) -> i32 { unsafe { out.write((random() * 32768.) as i32); } 0 }
unsafe extern "C" fn engine_point(pawn: *mut c_void, point: *const c_void, out: *mut i32) -> i32 { unsafe { point_reachable(pawn, point, 0, out) } }
unsafe extern "C" fn engine_line(model: *mut c_void, end: *const c_void, start: *const c_void) -> u8 { let mut value = 0; unsafe { fast_byte(model, end, start, &raw mut value); } value }
unsafe extern "C" fn wind_count() -> i32 { 0 }
unsafe extern "C" fn wind_at(_: i32) -> *mut c_void { ptr::null_mut() }
unsafe extern "C" fn wind_random() -> *mut c_void { ptr::null_mut() }
unsafe extern "C" fn define_reach(_: *mut c_void, _: *mut c_void, _: *mut c_void, _: *mut c_void, _: *mut i32) -> i32 { panic!("oracle uses authored reachspecs, not path building") }
unsafe extern "C" fn best_reach(_: *mut c_void, _: *const c_void, _: *const c_void, _: *mut c_void, _: *mut i32) -> i32 { panic!("oracle uses authored reachspecs, not path building") }
const ENGINE: Hp2EngineServices = Hp2EngineServices { app_frand: app_random, app_rand: engine_rand, far_move_actor: far_default, point_reachable: engine_point, model_fast_line_check: engine_line, rotator_vector: engine_rotation, wind_count, wind_at, wind_random, reach_define_for: define_reach, reach_find_best: best_reach };

unsafe fn setup(storage: &mut Storage, case: usize) {
    unsafe {
        ptr::write_bytes(storage, 0, 1);
        let level = &raw mut storage.level; let info = &raw mut storage.info;
        let addresses = std::array::from_fn(|i| storage.actors[i].0.as_mut_ptr() as usize);
        with(|world| { *world = World { actors: addresses, level: level as usize, info: info as usize, frame: (&raw mut storage.frame) as usize, case, random: case as u32 + 1, trace: Vec::new() }; });
        (*info).game = (&raw mut storage.game).cast(); (*info).base.zone_gravity.z = -950.; (*info).base.set_b_water_zone(case & 16 != 0); (*info).net_mode = (case % 4) as u8;
        (*level).model = (&raw mut storage.model).cast(); storage.list[0] = info.cast();
        (*level).base.actors.data = storage.list.as_mut_ptr().cast(); (*level).base.actors.array_num = 5; (*level).base.actors.array_max = 5;
        for i in 0..POOL {
            let raw = actor(i); storage.list[i+1] = raw;
            (*raw).x_level = level.cast(); (*raw).level = info.cast(); (*raw).region.zone = info.cast(); (*raw).region.zone_number = 1;
            (*raw).collision_radius = 10. + (case % 3) as f32 / 3.; (*raw).collision_height = 20. + (case % 5) as f32 / 3.;
            (*raw).location = FVector { x: (i*100 + case%4*10) as f32, y: (case%3*10) as f32, z: (*raw).collision_height };
            if class(raw.cast()) == 1 {
                let pawn = raw.cast::<APawn>(); (*pawn).foot_region.zone = info.cast(); (*pawn).head_region.zone = info.cast();
                (*pawn).set_b_can_walk(true); (*pawn).set_b_can_fly(true); (*pawn).set_b_can_swim(true);
                (*pawn).health = 100; (*pawn).sight_counter = -1.; (*pawn).hearing_threshold = 1.;
            }
        }
        let pawn = actor(0).cast::<APawn>(); (*pawn.cast::<UObject>()).state_frame = (&raw mut storage.frame).cast();
        (*pawn).base.physics = [1,3,4,10,2][case%5]; (*pawn).set_b_is_player(case&1 != 0); (*pawn).set_b_can_strafe(case&2 != 0);
        (*pawn).set_b_reduced_speed(true); (*pawn).set_b_advanced_tactics(case&4 != 0); (*pawn).set_b_shoot_special(true);
        (*pawn).special_pause = 2.; (*pawn).max_desired_speed = 1.; (*pawn).ground_speed = 200.; (*pawn).water_speed = 150.; (*pawn).air_speed = 250.;
        (*pawn).accel_rate = 512.; (*pawn).desired_speed = 0.75; (*pawn).move_timer = 1.; (*pawn).floor.z = 1.; (*pawn).base_eye_height = 12.;
        (*pawn).base.rotation = FRotator { pitch: case as i32 * 100, yaw: case as i32 * 1000, roll: 0 };
        (*pawn).destination = FVector { x: 200., y: 0., z: (*pawn).base.location.z }; (*pawn).focus = (*pawn).destination;
        (*pawn).move_target = if case%5 == 4 { ptr::null_mut() } else { actor(case%4).cast() };
        (*pawn).face_target = if case%5 == 4 { ptr::null_mut() } else { actor((case+1)%4).cast() };
        (*pawn).special_goal = if case%3 == 0 { actor(2).cast() } else { ptr::null_mut() };
        (*pawn).base.instigator = if case%3 == 0 { ptr::null_mut() } else { pawn.cast() };
        (*pawn).next_pawn = if case&1 != 0 { actor(3).cast() } else { ptr::null_mut() }; (*info).pawn_list = pawn.cast();
        let first = actor(1).cast::<ANavigationPoint>(); let second = actor(2).cast::<ANavigationPoint>();
        (*info).navigation_point_list = first.cast(); (*first).next_navigation_point = second;
        for node in [first, second] { (*node).paths = [-1;16]; (*node).upstream_paths = [-1;16]; (*node).pruned_paths = [-1;16]; (*node).extra_cost = 7; (*node).visited_weight = 10000000; }
        for (i,(start,end)) in [(first,second),(second,first)].into_iter().enumerate() {
            storage.specs[i].start = start.cast(); storage.specs[i].end = end.cast(); storage.specs[i].distance = 100;
            storage.specs[i].collision_radius = 100; storage.specs[i].collision_height = 100; storage.specs[i].reach_flags = 7;
        }
        (*level).reach_specs.data = storage.specs.as_mut_ptr().cast(); (*level).reach_specs.num = (case%3) as i32; (*level).reach_specs.max = (case%3) as i32;
        if case%3 > 0 { (*first).paths[0] = 0; (*second).upstream_paths[0] = 0; }
        if case%3 > 1 { (*second).paths[0] = 1; (*first).upstream_paths[0] = 1; }
        if case&1 == 0 { (*actor(3).cast::<AInventory>()).my_marker = second.cast(); (*second.cast::<AInventorySpot>()).marked_item = actor(3).cast(); }
    }
}
fn float(text: &str) -> f32 { f32::from_bits(u32::from_str_radix(text,16).unwrap()) }
fn vector(text: &str) -> FVector { let values: Vec<_> = text.split(',').map(float).collect(); assert_eq!(values.len(),3); FVector { x:values[0],y:values[1],z:values[2] } }
fn object(text: &str) -> *mut c_void { if text == "-" { ptr::null_mut() } else { actor(text.strip_prefix('p').unwrap().parse().unwrap()).cast() } }
fn optional<T>(text: &str, default: T, parse: impl FnOnce(&str)->T) -> T { let (present,value) = text.split_once(':').unwrap(); if present == "1" { parse(value) } else { default } }
unsafe fn execute(name: &str, args: &[&str]) -> String {
    let pawn = actor(0).cast::<APawn>();
    unsafe {
        match name {
            "actorReachable" => format!("{:08x}", movement::actor_reachable(&MOVE,pawn,object(args[0])).unwrap()),
            "findPathTo" => { let point=vector(args[0]); id(movement::find_path_to(&MOVE,pawn,(&raw const point).cast(),optional(args[1],0,|s|s.parse().unwrap()),optional(args[2],1,|s|s.parse().unwrap())).unwrap()) },
            "findPathToward" => id(movement::find_path_toward(&MOVE,pawn,object(args[0]),optional(args[1],0,|s|s.parse().unwrap()),optional(args[2],1,|s|s.parse().unwrap())).unwrap()),
            "findRandomDest" => id(movement::find_random_dest(&MOVE,pawn,optional(args[0],1,|s|s.parse().unwrap())).unwrap()),
            "makeNoise" => { movement::make_noise(&MOVE,pawn.cast(),float(args[0])).unwrap(); "-".into() },
            "moveTo" => { movement::move_to(&MOVE,pawn,vector(args[0]),optional(args[1],1.,float),501).unwrap(); "00000000".into() },
            "pollMoveTo" | "pollStrafeTo" => { movement::poll_destination(&MOVE,pawn).unwrap(); "00000000".into() },
            "moveToward" => { movement::move_toward(&MOVE,pawn,object(args[0]),optional(args[1],1.,float),503).unwrap(); "00000000".into() },
            "pollMoveToward" => { movement::poll_move_toward(&MOVE,pawn).unwrap(); "00000000".into() },
            "strafeTo" => { movement::strafe_to(&MOVE,pawn,vector(args[0]),vector(args[1]),505).unwrap(); "00000000".into() },
            "strafeFacing" => { movement::strafe_facing(&MOVE,pawn,vector(args[0]),object(args[1]),507).unwrap(); "00000000".into() },
            "pollStrafeFacing" => { movement::poll_strafe_facing(&MOVE,pawn).unwrap(); "00000000".into() },
            "turnToward" => { movement::turn_toward(&MOVE,pawn,object(args[0]),511).unwrap(); "00000000".into() },
            "pollTurnToward" => { movement::poll_turn_toward(&MOVE,pawn).unwrap(); "00000000".into() },
            "turnTo" => { movement::turn_to(&MOVE,pawn,vector(args[0]),509).unwrap(); "00000000".into() },
            "pollTurnTo" => { movement::poll_turn_to(&MOVE,pawn).unwrap(); "00000000".into() },
            "showSelf" => { movement::show_self(&MOVE,pawn).unwrap(); "-".into() },
            _ => panic!("unexpected oracle native {name}"),
        }
    }
}
fn snapshot() -> String {
    unsafe {
        let pawn=actor(0).cast::<APawn>(); let action=(*(with(|world|world.frame) as *mut FStateFrame)).latent_action;
        format!("lat={action} mt={} fa={} sg={} dest={} focus={} acc={} ds={} mtm={} red={} sp={} shoot={}",
            id((*pawn).move_target),id((*pawn).face_target),id((*pawn).special_goal),xyz((*pawn).destination),xyz((*pawn).focus),xyz((*pawn).base.acceleration),
            hexf((*pawn).desired_speed),hexf((*pawn).move_timer),u8::from((*pawn).b_reduced_speed()),hexf((*pawn).special_pause),u8::from((*pawn).b_shoot_special()))
    }
}
#[test]
fn pawn_native_move_replay_is_bit_exact() {
    unsafe { route::publish_services(&ROUTE); pawn_reach::publish_services(&REACH); pawn_sense::publish_services(&SENSE); services::publish_services(ENGINE); }
    let mut storage: Box<Storage> = Box::new(unsafe { zeroed() });
    let mut previous_case = None; let mut actual = String::new();
    for expected in FIXTURE.lines() {
        let (input,_) = expected.split_once(" ret=").expect("oracle input/output delimiter");
        let mut fields=input.split_whitespace(); let name=fields.next().unwrap();
        let case:usize=fields.next().unwrap().strip_prefix("c=").unwrap().parse().unwrap();
        if previous_case != Some(case) { unsafe { setup(&mut storage,case); } previous_case=Some(case); }
        let args:Vec<_>=fields.map(|field|field.split_once('=').unwrap().1).collect();
        with(|world|world.trace.clear());
        let result=unsafe { execute(name,&args) }; let calls=with(|world|world.trace.join(";"));
        let line=format!("{input} ret={result} {} calls={calls}",snapshot());
        actual.push_str(&line); actual.push('\n');
        assert_eq!(line,expected,"native {name}, case {case}: real composed state must match the independent C++ oracle");
    }
    let artifacts=std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"));
    std::fs::write(artifacts.join("pawn-native-move-replay.txt"),actual).unwrap();
}
