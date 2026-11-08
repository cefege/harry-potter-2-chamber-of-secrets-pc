//! Full builder phase transitions and failure-state contracts.
#[path = "actor_services/mock.rs"]
mod actor_services_mock;

use hp2_abi::engine::generated::{AActor, ANavigationPoint, APawn, FReachSpec, ULevel};
use hp2_engine::level_spawn_actor::Hp2SpawnActorContext;
use hp2_engine::path::{
    self, Hp2PathBuildServices, Hp2PathCreationServices, Hp2PathDefinitionServices,
    Hp2PathMaintenanceServices, Hp2PathScoutServices, PathScoutThrown,
};
use std::{cell::RefCell, ffi::c_void, ptr};

#[path = "level_spawn_actor/mock.rs"]
mod level_spawn_actor_mock;

#[derive(Clone, Debug, PartialEq)]
enum Event {
    At(i32),
    Is(*mut c_void, i32),
    Destroy(*mut c_void),
    ClearSpecs,
    ClearList,
    Status,
    Collision,
    Zone,
    Size,
    Class,
    Spawn,
    Name,
    Same,
    Add,
    Elapsed(f32),
}
#[derive(Default)]
struct World {
    actors: Vec<*mut c_void>,
    nodes: Vec<*mut c_void>,
    automatic: Vec<*mut c_void>,
    scout: *mut c_void,
    events: Vec<Event>,
    fail_at: Option<usize>,
    append_on_destroy: Option<*mut c_void>,
    level: *mut ULevel,
    specs: Vec<FReachSpec>,
    clocks: usize,
    configured: bool,
}
thread_local! { static WORLD: RefCell<World> = RefCell::new(World::default()); }
fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}
fn event(w: &mut World, e: Event) -> bool {
    w.events.push(e);
    w.fail_at == Some(w.events.len() - 1)
}
unsafe extern "C" fn count(_: *mut c_void) -> i32 {
    with(|w| w.actors.len() as i32)
}
unsafe extern "C" fn at(_: *mut c_void, i: i32, out: *mut *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::At(i)) {
            return 1;
        }
        unsafe {
            *out = w.actors[i as usize];
        }
        0
    })
}
unsafe extern "C" fn scout_is(a: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Is(a, kind)) {
            return 1;
        }
        unsafe {
            *out = match kind {
                0 => w.nodes.contains(&a),
                7 | 9 => a == w.scout,
                _ => false,
            } as i32;
        }
        0
    })
}
unsafe extern "C" fn maintenance_is(a: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Is(a, kind)) {
            return 1;
        }
        unsafe {
            *out = (matches!(kind, 0 | 1) && w.nodes.contains(&a)) as i32;
        }
        0
    })
}
unsafe extern "C" fn destroy(_: *mut c_void, a: *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::Destroy(a)) {
            return 1;
        }
        unsafe {
            (*a.cast::<AActor>()).set_b_delete_me(true);
        }
        for slot in &mut w.actors {
            if *slot == a {
                *slot = ptr::null_mut();
            }
        }
        if a != w.scout {
            if let Some(added) = w.append_on_destroy.take() {
                w.actors.push(added);
            }
        }
        0
    })
}
unsafe extern "C" fn automatic(a: *mut c_void) -> i32 {
    with(|w| w.automatic.contains(&a) as i32)
}
unsafe extern "C" fn clear_specs(_: *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::ClearSpecs) {
            return 1;
        }
        w.specs.clear();
        0
    })
}
unsafe extern "C" fn clear_list(_: *mut c_void) -> i32 {
    with(|w| event(w, Event::ClearList) as i32)
}
unsafe extern "C" fn status(_: i32, _: i32) -> i32 {
    with(|w| event(w, Event::Status) as i32)
}
unsafe extern "C" fn reach_count(_: *mut c_void) -> i32 {
    with(|w| w.specs.len() as i32)
}
unsafe extern "C" fn log_count(_: i32) -> i32 {
    0
}
unsafe extern "C" fn item(_: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn clear_item(_: *mut c_void) {}
unsafe extern "C" fn vis(_: *mut c_void, _: i32, _: *mut c_void) {}
unsafe extern "C" fn class(out: *mut *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::Class) {
            return 1;
        }
        unsafe {
            *out = ptr::without_provenance_mut(1);
        }
        0
    })
}
unsafe extern "C" fn spawn(_: *const Hp2SpawnActorContext, _: usize, out: *mut usize) -> i32 {
    with(|w| {
        if event(w, Event::Spawn) {
            return 1;
        }
        unsafe {
            *out = w.scout as usize;
        }
        w.actors.push(w.scout);
        0
    })
}
unsafe extern "C" fn zone(_: *mut c_void, _: *mut c_void) -> i32 {
    with(|w| event(w, Event::Zone) as i32)
}
unsafe extern "C" fn size(a: *mut c_void, _: f32, _: f32) -> i32 {
    with(|w| {
        if event(w, Event::Size) {
            return 1;
        }
        let p = unsafe { &*a.cast::<APawn>() };
        w.configured = p.jump_z == -1.0
            && p.ground_speed == 320.0
            && p.max_step_height == 24.0
            && p.base.b_collide_world();
        0
    })
}
unsafe extern "C" fn name(_: *mut c_void, _: i32, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Name) {
            return 1;
        }
        unsafe {
            *out = 0;
        }
        0
    })
}
unsafe extern "C" fn same(_: *mut c_void, _: *mut c_void, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Same) {
            return 1;
        }
        unsafe {
            *out = 1;
        }
        0
    })
}
unsafe extern "C" fn add(_: *mut c_void, spec: *const c_void, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Add) {
            return 1;
        }
        unsafe {
            *out = w.specs.len() as i32;
            // Read assigned fields, not the native FReachSpec padding.
            let source = spec.cast::<FReachSpec>();
            let mut value: FReachSpec = std::mem::zeroed();
            value.distance = (*source).distance;
            value.start = (*source).start;
            value.end = (*source).end;
            value.collision_radius = (*source).collision_radius;
            value.collision_height = (*source).collision_height;
            value.reach_flags = (*source).reach_flags;
            value.b_pruned = (*source).b_pruned;
            w.specs.push(value);
            (*w.level).reach_specs.data = w.specs.as_mut_ptr().cast();
        }
        0
    })
}
unsafe extern "C" fn seconds() -> f32 {
    with(|w| {
        w.clocks += 1;
        if w.clocks == 1 { 12.5 } else { 15.75 }
    })
}
unsafe extern "C" fn elapsed(t: f32) -> i32 {
    with(|w| event(w, Event::Elapsed(t)) as i32)
}
// These movement/spawn leaves are deliberately rejected: this fixture has one
// persistent node, so no traversal, placement, averaging or intermediate path is needed.
unsafe extern "C" fn unused_spawn(_: *mut c_void, _: *const c_void, _: *mut *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn unused_far(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: i32,
    _: i32,
    _: *mut i32,
) -> i32 {
    1
}
unsafe extern "C" fn unused_move(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: *mut c_void,
) -> i32 {
    1
}
unsafe extern "C" fn unused_reach(_: *mut c_void, _: *const c_void, _: *mut i32) -> i32 {
    1
}
unsafe extern "C" fn unused_trace(
    _: *mut c_void,
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    _: *const c_void,
) -> i32 {
    1
}
unsafe extern "C" fn unused_line(
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    _: *mut i32,
) -> i32 {
    1
}
unsafe extern "C" fn unused_vector(_: *mut c_void, _: *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn unused_diagnostic(
    _: i32,
    _: *mut c_void,
    _: *mut c_void,
    _: f32,
    _: f32,
    _: f32,
) -> i32 {
    1
}
unsafe extern "C" fn unused_mark(_: *mut c_void) {}
unsafe extern "C" fn unused_flag(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn unused_reason(_: i32) -> i32 {
    1
}
fn scout_services() -> Hp2PathScoutServices {
    level_spawn_actor_mock::publish(spawn);
    actor_services_mock::publish();
    Hp2PathScoutServices {
        actor_count: count,
        actor_at: at,
        actor_is_a: scout_is,
        find_scout_class: class,
        set_actor_zone: zone,
        far_move_actor: unused_far,
        move_actor: unused_move,
        point_reachable: unused_reach,
        single_line_check: unused_trace,
        placement_failed: unused_reason,
    }
}
fn maintenance() -> Hp2PathMaintenanceServices {
    Hp2PathMaintenanceServices {
        actor_count: count,
        actor_at: at,
        actor_is_a: maintenance_is,
        destroy_actor: destroy,
        reach_spec_count: reach_count,
        log_reach_specs: log_count,
        clear_reach_specs: clear_specs,
        clear_navigation_point_list: clear_list,
        status_update: status,
        inventory_spot_item: item,
        clear_item_marker: clear_item,
        vis_no_reach_set: vis,
    }
}
fn creation() -> Hp2PathCreationServices {
    Hp2PathCreationServices {
        spawn_path: unused_spawn,
        destroy_actor: destroy,
        auto_built: automatic,
        set_auto_built: unused_mark,
        one_way: unused_flag,
        rotation_vector: unused_vector,
        fast_line: unused_line,
        diagnostic: unused_diagnostic,
    }
}
fn definition() -> Hp2PathDefinitionServices {
    Hp2PathDefinitionServices {
        actor_count: count,
        actor_at: at,
        actor_is_a: scout_is,
        clear_navigation_point_list: clear_list,
        actor_class_name_is: name,
        actor_same_class: same,
        reach_spec_add: add,
    }
}
fn build() -> Hp2PathBuildServices {
    Hp2PathBuildServices {
        seconds,
        log_elapsed: elapsed,
    }
}
struct Fixture {
    level: Box<ULevel>,
    nodes: [Box<ANavigationPoint>; 3],
    scout: Box<APawn>,
}
impl Fixture {
    fn new() -> Self {
        let mut f = Self {
            level: Box::new(unsafe { std::mem::zeroed() }),
            nodes: std::array::from_fn(|_| Box::new(unsafe { std::mem::zeroed() })),
            scout: Box::new(unsafe { std::mem::zeroed() }),
        };
        for (i, n) in f.nodes.iter_mut().enumerate() {
            n.paths.fill(-1);
            n.base.location.x = i as f32 * 100.0;
        }
        f.scout.jump_z = 999.0;
        f.scout.ground_speed = 1.0;
        f.scout.max_step_height = 2.0;
        f.reset();
        f
    }
    fn reset(&mut self) {
        let n = self
            .nodes
            .each_mut()
            .map(|n| ptr::from_mut(&mut **n).cast::<c_void>());
        let level_ptr: *mut c_void = ptr::from_mut(&mut *self.level).cast();
        for node in &mut self.nodes {
            node.base.set_b_delete_me(false);
            node.paths.fill(-1);
            node.base.x_level = level_ptr;
        }
        self.scout.base.set_b_delete_me(false);
        self.scout.base.x_level = level_ptr;
        with(|w| {
            *w = World {
                actors: vec![n[0], n[1]],
                nodes: n.to_vec(),
                automatic: vec![n[0], n[2]],
                scout: ptr::from_mut(&mut *self.scout).cast(),
                level: ptr::from_mut(&mut *self.level),
                specs: Vec::with_capacity(16),
                ..World::default()
            }
        });
    }
    fn run(&mut self) -> Result<i32, PathScoutThrown> {
        let mut scout_slot = ptr::null_mut();
        unsafe {
            path::build_paths(
                &scout_services(),
                &maintenance(),
                &creation(),
                &definition(),
                &build(),
                ptr::from_mut(&mut *self.level).cast(),
                &mut scout_slot,
                99,
            )
        }
    }
}
#[test]
fn build_clears_old_automatic_nodes_preserves_manual_nodes_and_configures_scout() {
    let mut f = Fixture::new();
    assert_eq!(f.run(), Ok(0));
    assert!(f.nodes[0].base.b_delete_me());
    assert!(!f.nodes[1].base.b_delete_me());
    assert!(f.scout.base.b_delete_me());
    assert_eq!(f.nodes[1].paths, [-1; 16]);
    assert!(
        f.scout.jump_z == -1.0
            && f.scout.ground_speed == 320.0
            && f.scout.max_step_height == 24.0
            && f.scout.base.b_collide_world()
    );
    with(|w| {
        let phases: Vec<_> = w
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::ClearSpecs | Event::ClearList | Event::Destroy(_) | Event::Elapsed(_)
                )
            })
            .cloned()
            .collect();
        assert_eq!(
            phases,
            vec![
                Event::Destroy(ptr::from_mut(&mut *f.nodes[0]).cast()),
                Event::ClearSpecs,
                Event::ClearList,
                Event::ClearList,
                Event::Destroy(w.scout),
                Event::ClearSpecs,
                Event::ClearList,
                Event::ClearList,
                Event::Elapsed(3.25)
            ]
        );
    });
}
#[test]
fn initial_removal_observes_appended_auto_built_slots() {
    let mut f = Fixture::new();
    with(|w| w.append_on_destroy = Some(ptr::from_mut(&mut *f.nodes[2]).cast()));
    assert_eq!(f.run(), Ok(0));
    assert!(f.nodes[2].base.b_delete_me());
    let events = with(|w| w.events.clone());
    let clear = events.iter().position(|e| *e == Event::ClearSpecs).unwrap();
    assert_eq!(
        events[..clear]
            .iter()
            .filter(|e| matches!(e, Event::Destroy(_)))
            .count(),
        2
    );
}
#[test]
fn every_throwing_phase_stops_without_later_cleanup_or_final_logging() {
    let mut f = Fixture::new();
    assert_eq!(f.run(), Ok(0));
    let baseline = with(|w| w.events.clone());
    for index in 0..baseline.len() {
        f.reset();
        with(|w| w.fail_at = Some(index));
        assert_eq!(f.run(), Err(PathScoutThrown(1)), "failure at {index}");
        assert_eq!(
            with(|w| w.events.clone()),
            baseline[..=index],
            "later work after {index}"
        );
    }
}
#[test]
fn graph_allocation_failure_keeps_only_previously_published_specs() {
    let mut f = Fixture::new();
    with(|w| {
        w.automatic.clear();
        w.events.clear();
    });
    assert_eq!(
        unsafe { path::define_paths(&definition(), &mut *f.level) },
        Ok(())
    );
    let fail_at = with(|w| {
        w.events
            .iter()
            .enumerate()
            .filter(|(_, e)| **e == Event::Add)
            .nth(1)
            .unwrap()
            .0
    });
    f.reset();
    with(|w| {
        w.automatic.clear();
        w.fail_at = Some(fail_at);
    });
    assert_eq!(
        unsafe { path::define_paths(&definition(), &mut *f.level) },
        Err(PathScoutThrown(1))
    );
    with(|w| {
        assert_eq!(w.specs.len(), 1);
        assert_eq!(w.events.last(), Some(&Event::Add));
    });
    assert_eq!(f.nodes[0].paths[0], 0);
    assert_eq!(f.nodes[1].paths, [-1; 16]);
}
