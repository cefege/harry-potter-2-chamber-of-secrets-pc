//! Contracts for live path maintenance and first-error boundaries.
use hp2_abi::engine::generated::{AActor, ANavigationPoint};
use hp2_engine::path::{self, Hp2PathMaintenanceServices, PathScoutThrown};
use std::{cell::RefCell, ffi::c_void, ptr};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Count(i32),
    At(i32),
    Is(*mut c_void, i32),
    Destroy(*mut c_void),
    ReachCount,
    Log(i32),
    ClearSpecs,
    ClearList,
    Status(i32, i32),
    Item(*mut c_void),
    ClearItem(*mut c_void),
    Vis(*mut c_void, i32),
}
impl Event {
    fn fallible(&self) -> bool {
        matches!(
            self,
            Self::At(_)
                | Self::Is(_, _)
                | Self::Destroy(_)
                | Self::Log(_)
                | Self::ClearSpecs
                | Self::ClearList
                | Self::Status(_, _)
        )
    }
}
struct Actor {
    pointer: *mut c_void,
    kind: i32,
    item: *mut c_void,
    visible: [*mut c_void; 16],
}
#[derive(Default)]
struct World {
    actors: Vec<*mut c_void>,
    registered: Vec<Actor>,
    events: Vec<Event>,
    fail_at: Option<usize>,
    reach_specs: i32,
    list_cleared: bool,
    // Destruction can refuse, append, or compact the live array.
    destroy_append: Option<(*mut c_void, *mut c_void)>,
    compact: bool,
    // Status can replace the current slot and change the live count.
    status_replace_append: Option<(*mut c_void, *mut c_void)>,
    status_truncate: bool,
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
    with(|w| {
        let count = w.actors.len() as i32;
        event(w, Event::Count(count));
        count
    })
}
unsafe extern "C" fn at(_: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::At(index)) {
            return 1;
        }
        unsafe {
            *out = w.actors[index as usize];
        }
        0
    })
}
unsafe extern "C" fn is_a(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
    with(|w| {
        if event(w, Event::Is(actor, kind)) {
            return 1;
        }
        let actual = w
            .registered
            .iter()
            .find(|a| a.pointer == actor)
            .unwrap()
            .kind;
        unsafe {
            *out =
                (actual == kind || (kind == path::MCLASS_NAVIGATION_POINT && actual >= 0)) as i32;
        }
        0
    })
}
unsafe extern "C" fn destroy(_: *mut c_void, actor: *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::Destroy(actor)) {
            return 1;
        }
        if let Some((trigger, appended)) = w.destroy_append {
            if actor == trigger {
                w.actors.push(appended);
                w.destroy_append = None;
            }
        }
        if w.compact {
            let index = w.actors.iter().position(|a| *a == actor).unwrap();
            w.actors.remove(index);
        }
        // Otherwise simulate DestroyActor returning false: status is still zero.
        0
    })
}
unsafe extern "C" fn reach_count(_: *mut c_void) -> i32 {
    with(|w| {
        event(w, Event::ReachCount);
        w.reach_specs
    })
}
unsafe extern "C" fn log_specs(count: i32) -> i32 {
    with(|w| event(w, Event::Log(count)) as i32)
}
unsafe extern "C" fn clear_specs(_: *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::ClearSpecs) {
            return 1;
        }
        w.reach_specs = 0;
        0
    })
}
unsafe extern "C" fn clear_list(_: *mut c_void) -> i32 {
    with(|w| {
        if event(w, Event::ClearList) {
            return 1;
        }
        w.list_cleared = true;
        0
    })
}
unsafe extern "C" fn status(index: i32, count: i32) -> i32 {
    with(|w| {
        if event(w, Event::Status(index, count)) {
            return 1;
        }
        if let Some((replacement, appended)) = w.status_replace_append.take() {
            w.actors[index as usize] = replacement;
            w.actors.push(appended);
        }
        if w.status_truncate {
            w.actors.truncate(index as usize + 1);
        }
        0
    })
}
unsafe extern "C" fn item(actor: *mut c_void) -> *mut c_void {
    with(|w| {
        event(w, Event::Item(actor));
        let a = w.registered.iter().find(|a| a.pointer == actor).unwrap();
        assert_eq!(a.kind, path::MCLASS_INVENTORY_SPOT);
        a.item
    })
}
unsafe extern "C" fn clear_item(item: *mut c_void) {
    with(|w| {
        event(w, Event::ClearItem(item));
    });
    unsafe {
        *item.cast::<*mut c_void>() = ptr::null_mut();
    }
}
unsafe extern "C" fn vis(actor: *mut c_void, index: i32, value: *mut c_void) {
    with(|w| {
        event(w, Event::Vis(actor, index));
        let a = w
            .registered
            .iter_mut()
            .find(|a| a.pointer == actor)
            .unwrap();
        assert!(a.kind >= 0);
        a.visible[index as usize] = value;
    });
}
fn services() -> Hp2PathMaintenanceServices {
    Hp2PathMaintenanceServices {
        actor_count: count,
        actor_at: at,
        actor_is_a: is_a,
        destroy_actor: destroy,
        reach_spec_count: reach_count,
        log_reach_specs: log_specs,
        clear_reach_specs: clear_specs,
        clear_navigation_point_list: clear_list,
        status_update: status,
        inventory_spot_item: item,
        clear_item_marker: clear_item,
        vis_no_reach_set: vis,
    }
}
fn register(pointer: *mut c_void, kind: i32, item: *mut c_void) {
    with(|w| {
        w.registered.push(Actor {
            pointer,
            kind,
            item,
            visible: [pointer; 16],
        })
    });
}
fn node() -> Box<ANavigationPoint> {
    let mut n: Box<ANavigationPoint> = Box::new(unsafe { std::mem::zeroed() });
    let p = (&mut *n) as *mut ANavigationPoint;
    n.base.draw_type = 7;
    n.paths = [21; 16];
    n.upstream_paths = [22; 16];
    n.pruned_paths = [23; 16];
    n.next_navigation_point = p;
    n.next_ordered = p;
    n.prev_ordered = p;
    n.start_path = p.cast();
    n.previous_path = p.cast();
    n.cost = 24;
    n.extra_cost = 25;
    n.visited_weight = 26;
    n.best_path_weight = 27;
    n
}
struct Fixture {
    nodes: Vec<Box<ANavigationPoint>>,
    ordinary: Box<AActor>,
    marker: Box<*mut c_void>,
}
impl Fixture {
    fn new() -> Self {
        // Persistent path node, persistent non-path navigation point, all four
        // temporary marker classes, and an inventory spot without an owner.
        let nodes: Vec<_> = (0..7).map(|_| node()).collect();
        let mut ordinary: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
        ordinary.draw_type = 9;
        let marker = Box::new((&*nodes[4] as *const ANavigationPoint).cast_mut().cast());
        let mut f = Self {
            nodes,
            ordinary,
            marker,
        };
        f.reset(None);
        f
    }
    fn pointer(&self, index: usize) -> *mut c_void {
        (&*self.nodes[index] as *const ANavigationPoint)
            .cast_mut()
            .cast()
    }
    fn reset(&mut self, fail_at: Option<usize>) {
        with(|w| {
            *w = World {
                reach_specs: 37,
                fail_at,
                ..World::default()
            }
        });
        *self.marker = self.pointer(4);
        for index in 0..self.nodes.len() {
            let pointer = self.pointer(index);
            // Restore only fields the algorithms mutate, keeping stable pointers.
            let n = &mut self.nodes[index];
            n.base.draw_type = 7;
            n.paths = [21; 16];
            n.upstream_paths = [22; 16];
            n.pruned_paths = [23; 16];
            n.next_navigation_point = pointer.cast();
            n.next_ordered = pointer.cast();
            n.prev_ordered = pointer.cast();
            n.start_path = pointer;
            n.previous_path = pointer;
            let kind = [1, 0, 2, 3, 4, 5, 4][index];
            let owner = if index == 4 {
                (&mut *self.marker as *mut *mut c_void).cast()
            } else {
                ptr::null_mut()
            };
            register(pointer, kind, owner);
        }
        let ordinary = (&mut *self.ordinary as *mut AActor).cast();
        register(ordinary, -1, ptr::null_mut());
        let mut actors = vec![ptr::null_mut(), ordinary];
        actors.extend((0..self.nodes.len()).map(|i| self.pointer(i)));
        with(|w| w.actors = actors);
    }
}
fn assert_reset(n: &ANavigationPoint) {
    assert!(n.next_navigation_point.is_null());
    assert!(n.next_ordered.is_null());
    assert!(n.prev_ordered.is_null());
    assert!(n.start_path.is_null());
    assert!(n.previous_path.is_null());
    assert_eq!(n.paths, [-1; 16]);
    assert_eq!(n.upstream_paths, [-1; 16]);
    assert_eq!(n.pruned_paths, [-1; 16]);
    assert_eq!(
        (n.cost, n.extra_cost, n.visited_weight, n.best_path_weight),
        (24, 25, 26, 27)
    );
}

#[test]
fn visibility_uses_exact_draw_types_and_counts_only_path_nodes() {
    let f = Fixture::new();
    unsafe {
        assert_eq!(path::show_paths(&services(), ptr::null_mut()), Ok(1));
        assert_eq!(f.nodes[0].base.draw_type, 1);
        assert_eq!(path::hide_paths(&services(), ptr::null_mut()), Ok(1));
    }
    assert_eq!(f.nodes[0].base.draw_type, 0);
    assert_eq!(f.ordinary.draw_type, 9);
    for n in &f.nodes[1..] {
        assert_eq!(n.base.draw_type, 7);
    }
    with(|w| assert!(!w.events.iter().any(|e| matches!(e, Event::Destroy(_)))));
}

#[test]
fn removal_rechecks_live_count_and_counts_refused_destruction() {
    let f = Fixture::new();
    let mut appended = node();
    let a = (&mut *appended as *mut ANavigationPoint).cast();
    register(a, path::MCLASS_PATH_NODE, ptr::null_mut());
    with(|w| w.destroy_append = Some((f.pointer(0), a)));
    assert_eq!(
        unsafe { path::remove_paths(&services(), ptr::null_mut()) },
        Ok(2)
    );
    with(|w| {
        let destroyed: Vec<_> = w
            .events
            .iter()
            .filter_map(|e| {
                if let Event::Destroy(p) = e {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(destroyed, vec![f.pointer(0), a]);
        assert!(w.actors.contains(&f.pointer(0)) && w.actors.contains(&a));
        assert_eq!(w.events.last(), Some(&Event::Count(10)));
    });
}

#[test]
fn compacted_destruction_preserves_legacy_index_increment() {
    let f = Fixture::new();
    with(|w| {
        w.actors = vec![f.pointer(0), f.pointer(0), f.pointer(0)];
        w.compact = true;
    });
    assert_eq!(
        unsafe { path::remove_paths(&services(), ptr::null_mut()) },
        Ok(2)
    );
    with(|w| assert_eq!(w.actors, vec![f.pointer(0)]));
}

#[test]
fn undefine_preserves_order_marker_ownership_and_all_persistent_slots() {
    let f = Fixture::new();
    assert_eq!(
        unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
        Ok(())
    );
    assert_reset(&f.nodes[0]);
    assert_reset(&f.nodes[1]);
    assert!(f.marker.is_null());
    for n in &f.nodes[2..] {
        assert_eq!(n.paths, [21; 16]);
        assert!(!n.next_navigation_point.is_null());
    }
    with(|w| {
        assert_eq!(
            &w.events[..4],
            &[
                Event::ReachCount,
                Event::Log(37),
                Event::ClearSpecs,
                Event::ClearList
            ]
        );
        assert_eq!((w.reach_specs, w.list_cleared), (0, true));
        let destroyed: Vec<_> = w
            .events
            .iter()
            .filter_map(|e| {
                if let Event::Destroy(p) = e {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(destroyed, (2..7).map(|i| f.pointer(i)).collect::<Vec<_>>());
        let cleared = w
            .events
            .iter()
            .position(|e| matches!(e, Event::ClearItem(_)))
            .unwrap();
        assert_eq!(w.events[cleared - 1], Event::Item(f.pointer(4)));
        assert_eq!(
            w.events[cleared - 2],
            Event::Is(f.pointer(4), path::MCLASS_INVENTORY_SPOT)
        );
        assert_eq!(w.events[cleared + 1], Event::Destroy(f.pointer(4)));
        for i in 0..2 {
            let actor = w
                .registered
                .iter()
                .find(|a| a.pointer == f.pointer(i))
                .unwrap();
            assert_eq!(actor.visible, [ptr::null_mut(); 16]);
            let slots: Vec<_> = w
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Vis(p, slot) if *p == f.pointer(i) => Some(*slot),
                    _ => None,
                })
                .collect();
            assert_eq!(slots, (0..16).collect::<Vec<_>>());
        }
        let checks: Vec<_> = w
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Is(p, kind) if *p == f.pointer(0) => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(checks, vec![0, 2, 3, 4, 5]);
        let statuses: Vec<_> = w
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Status(i, total) => Some((*i, *total)),
                _ => None,
            })
            .collect();
        assert_eq!(statuses, (0..9).map(|i| (i, 9)).collect::<Vec<_>>());
    });
}

#[test]
fn status_mutation_is_observed_before_slot_read_and_next_count() {
    let f = Fixture::new();
    with(|w| {
        w.actors = vec![f.pointer(0)];
        w.status_replace_append = Some((f.pointer(2), f.pointer(1)));
    });
    assert_eq!(
        unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
        Ok(())
    );
    assert_eq!(f.nodes[0].paths, [21; 16]);
    assert_reset(&f.nodes[1]);
    with(|w| {
        assert!(w.events.contains(&Event::Destroy(f.pointer(2))));
        assert!(w.events.contains(&Event::Status(0, 1)));
        assert!(w.events.contains(&Event::Status(1, 2)));
        assert!(!w.events.contains(&Event::Is(f.pointer(0), 0)));
    });
}

#[test]
fn status_shrink_ends_iteration_without_snapshot_slots() {
    let f = Fixture::new();
    with(|w| {
        w.actors = vec![f.pointer(0), f.pointer(1)];
        w.status_truncate = true;
    });
    assert_eq!(
        unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
        Ok(())
    );
    assert_reset(&f.nodes[0]);
    assert_eq!(f.nodes[1].paths, [21; 16]);
    with(|w| assert!(!w.events.contains(&Event::At(1))));
}

#[test]
fn undefine_stops_at_every_fallible_boundary_without_cleanup() {
    let mut f = Fixture::new();
    assert_eq!(
        unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
        Ok(())
    );
    let baseline = with(|w| w.events.clone());
    for (index, failed) in baseline.iter().enumerate().filter(|(_, e)| e.fallible()) {
        f.reset(Some(index));
        assert_eq!(
            unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
            Err(PathScoutThrown(1))
        );
        with(|w| {
            assert_eq!(w.events, baseline[..=index], "failure at {failed:?}");
            let specs_cleared = baseline
                .iter()
                .position(|e| *e == Event::ClearSpecs)
                .unwrap()
                < index;
            let list_cleared = baseline
                .iter()
                .position(|e| *e == Event::ClearList)
                .unwrap()
                < index;
            assert_eq!(w.reach_specs, if specs_cleared { 0 } else { 37 });
            assert_eq!(w.list_cleared, list_cleared);
        });
        let owner_cleared = baseline
            .iter()
            .position(|e| matches!(e, Event::ClearItem(_)))
            .unwrap()
            < index;
        assert_eq!(f.marker.is_null(), owner_cleared);
        for node_index in 0..2 {
            let reset = baseline
                .iter()
                .position(|e| *e == Event::Vis(f.pointer(node_index), 15))
                .unwrap()
                < index;
            if reset {
                assert_reset(&f.nodes[node_index]);
            } else {
                assert_eq!(f.nodes[node_index].paths, [21; 16]);
                assert!(!f.nodes[node_index].next_navigation_point.is_null());
            }
        }
    }
}

#[test]
fn counting_operations_stop_at_every_fallible_boundary() {
    type Operation =
        unsafe fn(&Hp2PathMaintenanceServices, *mut c_void) -> Result<i32, PathScoutThrown>;
    let operations: [Operation; 3] = [path::remove_paths, path::show_paths, path::hide_paths];
    let mut f = Fixture::new();
    for operation in operations {
        f.reset(None);
        assert_eq!(unsafe { operation(&services(), ptr::null_mut()) }, Ok(1));
        let baseline = with(|w| w.events.clone());
        for (index, _) in baseline.iter().enumerate().filter(|(_, e)| e.fallible()) {
            f.reset(Some(index));
            assert_eq!(
                unsafe { operation(&services(), ptr::null_mut()) },
                Err(PathScoutThrown(1))
            );
            with(|w| assert_eq!(w.events, baseline[..=index]));
        }
    }
}

#[test]
fn empty_level_still_logs_and_clears_in_order() {
    with(|w| *w = World::default());
    unsafe {
        assert_eq!(path::remove_paths(&services(), ptr::null_mut()), Ok(0));
        assert_eq!(path::show_paths(&services(), ptr::null_mut()), Ok(0));
        assert_eq!(path::hide_paths(&services(), ptr::null_mut()), Ok(0));
    }
    with(|w| w.events.clear());
    assert_eq!(
        unsafe { path::undefine_paths(&services(), ptr::null_mut()) },
        Ok(())
    );
    with(|w| {
        assert_eq!(
            w.events,
            vec![
                Event::ReachCount,
                Event::Log(0),
                Event::ClearSpecs,
                Event::ClearList,
                Event::Count(0),
            ]
        )
    });
}
