use hp2_core::object_lifecycle::*;
use std::ffi::c_void;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hook {
    Set(u32),
    Clear(u32),
    Destroy,
    Register,
    PostLoad,
    Route(i32),
}

struct Object {
    index: u32,
    flags: u32,
    calls: Vec<Hook>,
    fail: Option<Hook>,
    route: bool,
    reenter: bool,
    nested_destroy: Option<Result<i32, i32>>,
    nested_post_load: Option<Result<(), i32>>,
    hook_flags: u32,
}

impl Object {
    fn new(flags: u32) -> Self {
        Self {
            index: 0,
            flags,
            calls: vec![],
            fail: None,
            route: true,
            reenter: false,
            nested_destroy: None,
            nested_post_load: None,
            hook_flags: 0,
        }
    }
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
    fn hit(&mut self, hook: Hook) -> i32 {
        self.calls.push(hook);
        if self.fail == Some(hook) { 71 } else { 0 }
    }
}
unsafe extern "C" fn index(p: *mut c_void) -> u32 {
    unsafe { (*p.cast::<Object>()).index }
}
unsafe extern "C" fn flags(p: *mut c_void) -> u32 {
    unsafe { (*p.cast::<Object>()).flags }
}
unsafe extern "C" fn set(p: *mut c_void, value: u32) -> i32 {
    unsafe {
        let o = &mut *p.cast::<Object>();
        let status = o.hit(Hook::Set(value));
        if status == 0 {
            o.flags |= value;
        }
        status
    }
}
unsafe extern "C" fn clear(p: *mut c_void, value: u32) -> i32 {
    unsafe {
        let o = &mut *p.cast::<Object>();
        let status = o.hit(Hook::Clear(value));
        if status == 0 {
            o.flags &= !value;
        }
        status
    }
}
unsafe extern "C" fn destroy(p: *mut c_void) -> i32 {
    unsafe {
        let o = p.cast::<Object>();
        let status = (*o).hit(Hook::Destroy);
        if status != 0 {
            return status;
        }
        (*o).hook_flags = (*o).flags;
        if (*o).reenter {
            let nested = conditional_destroy(p, &LIFECYCLE);
            (*o).nested_destroy = Some(nested);
        }
        if (*o).route {
            (*o).flags |= RF_DEBUG_DESTROY;
        }
        0
    }
}
unsafe extern "C" fn register(p: *mut c_void) -> i32 {
    unsafe {
        let o = &mut *p.cast::<Object>();
        let status = o.hit(Hook::Register);
        if status == 0 {
            o.index = 7;
        }
        status
    }
}
unsafe extern "C" fn post_load(p: *mut c_void) -> i32 {
    unsafe {
        let o = p.cast::<Object>();
        let status = (*o).hit(Hook::PostLoad);
        if status != 0 {
            return status;
        }
        (*o).hook_flags = (*o).flags;
        if (*o).reenter {
            let nested = conditional_post_load(p, &LIFECYCLE);
            (*o).nested_post_load = Some(nested);
        }
        if (*o).route {
            (*o).flags |= RF_DEBUG_POST_LOAD;
        }
        0
    }
}
unsafe extern "C" fn route_error(p: *mut c_void, post: i32) -> i32 {
    unsafe {
        (*p.cast::<Object>()).hit(Hook::Route(post));
    }
    81 + post
}
unsafe extern "C" fn table_len(_: *mut c_void) -> i32 {
    unreachable!()
}
unsafe extern "C" fn table_at(_: *mut c_void, _: i32, _: *mut *mut c_void) -> i32 {
    unreachable!()
}
unsafe extern "C" fn assert_registrant(found: i32) -> i32 {
    if found != 0 { 0 } else { 73 }
}
static LIFECYCLE: Hp2ObjectLifecycleServices = Hp2ObjectLifecycleServices {
    index,
    flags,
    set_flags: set,
    clear_flags: clear,
    destroy,
    register,
    post_load,
    route_error,
    table_len,
    table_at,
    assert_registrant,
};

#[test]
fn destroy_marks_before_hook_and_is_reentrant_and_idempotent() {
    let mut object = Object::new(RF_DEBUG_DESTROY | 4);
    object.reenter = true;
    assert_eq!(
        unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
        Ok(1)
    );
    assert_eq!(object.hook_flags, RF_DESTROYED | 4);
    assert_eq!(object.nested_destroy, Some(Ok(0)));
    assert_eq!(
        object.calls,
        [
            Hook::Set(RF_DESTROYED),
            Hook::Clear(RF_DEBUG_DESTROY),
            Hook::Destroy
        ]
    );
    assert_eq!(
        unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
        Ok(0)
    );
    assert_eq!(object.calls.len(), 3);
    object.index = u32::MAX;
    object.flags = 0;
    assert_eq!(
        unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
        Ok(0)
    );
    assert_eq!(object.flags, 0);
    assert_eq!(object.calls.len(), 3);
}

#[test]
fn post_load_clears_both_flags_before_reentry_and_checks_routing() {
    let mut object = Object::new(RF_NEED_POST_LOAD | RF_DEBUG_POST_LOAD | 4);
    object.reenter = true;
    assert_eq!(
        unsafe { conditional_post_load(object.ptr(), &LIFECYCLE) },
        Ok(())
    );
    assert_eq!(object.hook_flags, 4);
    assert_eq!(object.nested_post_load, Some(Ok(())));
    assert_eq!(object.flags, RF_DEBUG_POST_LOAD | 4);
    assert_eq!(
        object.calls,
        [
            Hook::Clear(RF_NEED_POST_LOAD | RF_DEBUG_POST_LOAD),
            Hook::PostLoad
        ]
    );
    assert_eq!(
        unsafe { conditional_post_load(object.ptr(), &LIFECYCLE) },
        Ok(())
    );
    assert_eq!(object.calls.len(), 2);
    object.flags |= RF_NEED_POST_LOAD;
    object.route = false;
    assert_eq!(
        unsafe { conditional_post_load(object.ptr(), &LIFECYCLE) },
        Err(82)
    );
    assert_eq!(object.flags, 4);
    assert_eq!(object.calls.last(), Some(&Hook::Route(1)));
}

#[test]
fn failed_destroy_route_keeps_destroyed_and_never_retries() {
    let mut object = Object::new(RF_DEBUG_DESTROY);
    object.route = false;
    assert_eq!(
        unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
        Err(81)
    );
    assert_eq!(object.flags, RF_DESTROYED);
    assert_eq!(object.calls.last(), Some(&Hook::Route(0)));
    let count = object.calls.len();
    assert_eq!(
        unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
        Ok(0)
    );
    assert_eq!(object.calls.len(), count);
}

#[test]
fn register_skips_registered_and_enforces_debug_membership_only_when_needed() {
    let mut object = Object::new(0);
    // A registered object must not even inspect the registrant table.
    assert_eq!(
        unsafe { conditional_register(object.ptr(), std::ptr::null_mut(), 1, &LIFECYCLE) },
        Ok(())
    );
    assert!(object.calls.is_empty());
    object.index = u32::MAX;
    let mut registrants = hp2_core::object_collections::ObjectCollection::new();
    let table = &mut registrants as *mut _;
    assert_eq!(
        unsafe { conditional_register(object.ptr(), table, 1, &LIFECYCLE) },
        Err(73)
    );
    assert!(object.calls.is_empty());
    registrants.append(std::ptr::null_mut());
    registrants.append(object.ptr());
    registrants.append(std::ptr::null_mut());
    assert_eq!(
        unsafe { conditional_register(object.ptr(), table, 1, &LIFECYCLE) },
        Ok(())
    );
    assert_eq!(object.calls, [Hook::Register]);
    object.index = u32::MAX;
    assert_eq!(
        unsafe { conditional_register(object.ptr(), std::ptr::null_mut(), 0, &LIFECYCLE) },
        Ok(())
    );
    assert_eq!(object.calls.len(), 2);
}

#[test]
fn lifecycle_stops_at_first_failed_leaf() {
    let sequence = [
        Hook::Set(RF_DESTROYED),
        Hook::Clear(RF_DEBUG_DESTROY),
        Hook::Destroy,
    ];
    for (position, hook) in sequence.iter().enumerate() {
        let mut object = Object::new(0);
        object.fail = Some(*hook);
        assert_eq!(
            unsafe { conditional_destroy(object.ptr(), &LIFECYCLE) },
            Err(71)
        );
        assert_eq!(object.calls, sequence[..=position]);
    }
    let sequence = [
        Hook::Clear(RF_NEED_POST_LOAD | RF_DEBUG_POST_LOAD),
        Hook::PostLoad,
    ];
    for (position, hook) in sequence.iter().enumerate() {
        let mut object = Object::new(RF_NEED_POST_LOAD | RF_DEBUG_POST_LOAD);
        object.fail = Some(*hook);
        assert_eq!(
            unsafe { conditional_post_load(object.ptr(), &LIFECYCLE) },
            Err(71)
        );
        assert_eq!(object.calls, sequence[..=position]);
    }
    let mut object = Object::new(0);
    object.index = u32::MAX;
    object.fail = Some(Hook::Register);
    assert_eq!(
        unsafe { conditional_register(object.ptr(), std::ptr::null_mut(), 0, &LIFECYCLE) },
        Err(71)
    );
    assert_eq!(object.index, u32::MAX);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Begin(i32),
    Len,
    Read(i32),
    LogDestroy(i32),
    Destroy(i32),
    Delete(i32),
    NameLen,
    NameRead(i32),
    NameDelete(i32),
    Counts(i32, i32),
}
#[derive(Default)]
struct Heap {
    objects: Vec<Box<Object>>,
    slots: Vec<*mut c_void>,
    names: Vec<Box<Object>>,
    name_slots: Vec<*mut c_void>,
    events: Vec<Event>,
    fail: Option<Event>,
    exit: i32,
    mutate_destroy: bool,
    mutate_delete: bool,
    grow_names: bool,
    replace_on_log: bool,
    logged_targets: Vec<*mut c_void>,
    destroyed_targets: Vec<*mut c_void>,
    deleted_targets: Vec<*mut c_void>,
}
impl Heap {
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
    fn add(&mut self, flags: u32) {
        let mut object = Box::new(Object::new(flags));
        self.slots.push(object.ptr());
        self.objects.push(object);
    }
    fn name(&mut self, flags: u32) {
        let mut name = Box::new(Object::new(flags));
        self.name_slots.push(name.ptr());
        self.names.push(name);
    }
    fn hit(&mut self, event: Event) -> i32 {
        self.events.push(event);
        if self.fail == Some(event) { 91 } else { 0 }
    }
    fn actions(&self) -> Vec<Event> {
        self.events
            .iter()
            .copied()
            .filter(|e| {
                matches!(
                    e,
                    Event::Begin(_)
                        | Event::Destroy(_)
                        | Event::Delete(_)
                        | Event::NameDelete(_)
                        | Event::Counts(_, _)
                )
            })
            .collect()
    }
}
unsafe extern "C" fn object_len(p: *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        h.hit(Event::Len);
        h.slots.len() as i32
    }
}
unsafe extern "C" fn object_at(p: *mut c_void, i: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::Read(i));
        if status == 0 {
            *out = h.slots[i as usize];
        }
        status
    }
}
unsafe extern "C" fn exit_purge(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<Heap>()).exit }
}
unsafe extern "C" fn log_destroy(p: *mut c_void, i: i32, object: *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::LogDestroy(i));
        h.logged_targets.push(object);
        if h.replace_on_log && i == 0 {
            h.add(RF_UNREACHABLE);
            h.slots[0] = h.slots.pop().unwrap();
        }
        status
    }
}
unsafe extern "C" fn purge_destroy(p: *mut c_void, i: i32, object: *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::Destroy(i));
        if status != 0 {
            return status;
        }
        h.destroyed_targets.push(object);
        if let Err(status) = conditional_destroy(object, &LIFECYCLE) {
            return status;
        }
        if h.mutate_destroy && i == 3 {
            h.slots[2] = std::ptr::null_mut();
            (*h.slots[1].cast::<Object>()).flags &= !RF_UNREACHABLE;
            // Lower native slots observe this change, and the deletion phase
            // obtains its own live bound, including the newly appended object.
            h.exit = 1;
            h.add(RF_UNREACHABLE);
        }
        0
    }
}
unsafe extern "C" fn delete_object(p: *mut c_void, i: i32, object: *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::Delete(i));
        if status != 0 {
            return status;
        }
        h.deleted_targets.push(object);
        h.slots[i as usize] = std::ptr::null_mut();
        if h.mutate_delete && i == 2 {
            (*h.slots[1].cast::<Object>()).flags |= RF_NATIVE;
            h.slots[0] = std::ptr::null_mut();
        }
        0
    }
}
unsafe extern "C" fn name_len(p: *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        h.hit(Event::NameLen);
        h.name_slots.len() as i32
    }
}
unsafe extern "C" fn name_at(p: *mut c_void, i: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::NameRead(i));
        if status == 0 {
            *out = h.name_slots[i as usize];
        }
        status
    }
}
unsafe extern "C" fn delete_name(p: *mut c_void, i: i32) -> i32 {
    unsafe {
        let h = &mut *p.cast::<Heap>();
        let status = h.hit(Event::NameDelete(i));
        if status != 0 {
            return status;
        }
        h.name_slots[i as usize] = std::ptr::null_mut();
        if h.grow_names && i == 0 {
            h.name_slots[1] = std::ptr::null_mut();
            (*h.name_slots[2].cast::<Object>()).flags |= RF_NATIVE;
            h.name(RF_UNREACHABLE);
        }
        0
    }
}
unsafe extern "C" fn log_begin(p: *mut c_void, no_gc: i32) -> i32 {
    unsafe { (*p.cast::<Heap>()).hit(Event::Begin(no_gc)) }
}
unsafe extern "C" fn log_counts(p: *mut c_void, before: i32, after: i32) -> i32 {
    unsafe { (*p.cast::<Heap>()).hit(Event::Counts(before, after)) }
}
static PURGE: Hp2ObjectPurgeServices = Hp2ObjectPurgeServices {
    object_len,
    object_at,
    object_flags: flags,
    exit_purge,
    log_destroy,
    destroy: purge_destroy,
    delete_object,
    name_len,
    name_at,
    name_flags: flags,
    delete_name,
    log_begin,
    log_counts,
};

#[test]
fn purge_has_reverse_separate_phases_and_counts_already_destroyed_objects() {
    let mut h = Heap::default();
    h.add(RF_UNREACHABLE);
    h.add(RF_UNREACHABLE | RF_DESTROYED);
    h.add(0);
    h.slots.push(std::ptr::null_mut());
    h.add(RF_UNREACHABLE);
    h.name(RF_UNREACHABLE);
    h.name(0);
    h.name(RF_UNREACHABLE | RF_NATIVE);
    h.name_slots.push(std::ptr::null_mut());
    h.name(RF_UNREACHABLE);
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Ok(()));
    assert_eq!(
        h.actions(),
        [
            Event::Begin(0),
            Event::Destroy(4),
            Event::Destroy(1),
            Event::Destroy(0),
            Event::Delete(4),
            Event::Delete(1),
            Event::Delete(0),
            Event::NameDelete(0),
            Event::NameDelete(4),
            Event::Counts(4, 1)
        ]
    );
    assert!(h.objects[1].calls.is_empty());
}

#[test]
fn native_destroy_requires_exit_purge_and_native_is_never_deleted() {
    for exit in [0, 1] {
        let mut h = Heap {
            exit,
            ..Heap::default()
        };
        h.add(RF_UNREACHABLE | RF_NATIVE);
        h.add(RF_UNREACHABLE | RF_NATIVE | RF_DESTROYED);
        h.add(RF_NATIVE);
        h.name(RF_UNREACHABLE | RF_NATIVE);
        assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Ok(()));
        let expected = if exit == 0 {
            vec![Event::Begin(0), Event::Counts(3, 3)]
        } else {
            vec![
                Event::Begin(0),
                Event::Destroy(1),
                Event::Destroy(0),
                Event::Counts(3, 1),
            ]
        };
        assert_eq!(h.actions(), expected);
        assert!(h.slots.iter().all(|p| !p.is_null()));
        assert!(!h.name_slots[0].is_null());
        assert!(h.objects[1].calls.is_empty());
    }
}

#[test]
fn destroy_scan_reads_live_slots_flags_exit_and_deletion_reloads_bound() {
    let mut h = Heap {
        mutate_destroy: true,
        ..Heap::default()
    };
    h.add(RF_UNREACHABLE | RF_NATIVE);
    h.add(RF_UNREACHABLE);
    h.add(RF_UNREACHABLE);
    h.add(RF_UNREACHABLE);
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Ok(()));
    assert_eq!(
        h.actions(),
        [
            Event::Begin(0),
            Event::Destroy(3),
            Event::Destroy(0),
            Event::Delete(4),
            Event::Delete(3),
            Event::Counts(3, 1)
        ]
    );
    assert!(h.objects[1].calls.is_empty());
    assert!(h.objects[2].calls.is_empty());
    assert!(h.objects[4].calls.is_empty());
}

#[test]
fn delete_scan_and_ascending_name_scan_observe_live_mutation() {
    let mut h = Heap {
        mutate_delete: true,
        grow_names: true,
        ..Heap::default()
    };
    for _ in 0..3 {
        h.add(RF_UNREACHABLE);
        h.name(RF_UNREACHABLE);
    }
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Ok(()));
    assert_eq!(
        h.actions(),
        [
            Event::Begin(0),
            Event::Destroy(2),
            Event::Destroy(1),
            Event::Destroy(0),
            Event::Delete(2),
            Event::NameDelete(0),
            Event::NameDelete(3),
            Event::Counts(3, 0)
        ]
    );
    assert!(!h.slots[1].is_null());
    assert!(!h.name_slots[2].is_null());
}

#[test]
fn no_gc_only_logs_even_with_populated_tables() {
    let mut h = Heap::default();
    h.add(RF_UNREACHABLE);
    h.name(RF_UNREACHABLE);
    assert_eq!(unsafe { purge_garbage(h.ptr(), 1, &PURGE) }, Ok(()));
    assert_eq!(h.events, [Event::Begin(1)]);
    assert!(h.objects[0].calls.is_empty());
    h.fail = Some(Event::Begin(1));
    assert_eq!(unsafe { purge_garbage(h.ptr(), 1, &PURGE) }, Err(91));
    assert_eq!(h.events, [Event::Begin(1), Event::Begin(1)]);
}

#[test]
fn purge_first_error_stops_without_later_phases_or_summary() {
    fn fixture() -> Heap {
        let mut h = Heap::default();
        h.add(RF_UNREACHABLE);
        h.add(RF_UNREACHABLE);
        h.name(RF_UNREACHABLE);
        h.name(RF_UNREACHABLE);
        h
    }
    let mut reference = fixture();
    assert_eq!(unsafe { purge_garbage(reference.ptr(), 0, &PURGE) }, Ok(()));
    for failure in [
        Event::Begin(0),
        Event::Read(1),
        Event::LogDestroy(1),
        Event::LogDestroy(0),
        Event::Destroy(1),
        Event::Destroy(0),
        Event::Delete(1),
        Event::Delete(0),
        Event::NameRead(0),
        Event::NameDelete(0),
        Event::NameRead(1),
        Event::NameDelete(1),
        Event::Counts(2, 0),
    ] {
        let mut h = fixture();
        h.fail = Some(failure);
        assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Err(91));
        let position = reference.events.iter().position(|e| *e == failure).unwrap();
        assert_eq!(h.events, reference.events[..=position], "{failure:?}");
    }
    let mut h = fixture();
    h.objects[1].route = false;
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Err(81));
    assert_eq!(h.actions(), [Event::Begin(0), Event::Destroy(1)]);
    assert!(h.objects[0].calls.is_empty());
}

#[test]
fn purge_reloads_target_after_reentrant_logging_and_passes_live_pointer_to_leaves() {
    let mut h = Heap {
        replace_on_log: true,
        ..Heap::default()
    };
    h.add(RF_UNREACHABLE);
    let original = h.slots[0];
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Ok(()));
    let replacement = h.objects[1].ptr();
    assert_eq!(h.logged_targets, [original]);
    assert_eq!(h.destroyed_targets, [replacement]);
    assert_eq!(h.deleted_targets, [replacement]);
    assert!(h.objects[0].calls.is_empty());
    assert_ne!(h.objects[1].flags & RF_DESTROYED, 0);
}

#[test]
fn purge_failed_log_retains_reentrant_replacement_without_destroying_either_target() {
    let mut h = Heap {
        replace_on_log: true,
        fail: Some(Event::LogDestroy(0)),
        ..Heap::default()
    };
    h.add(RF_UNREACHABLE);
    let original = h.slots[0];
    assert_eq!(unsafe { purge_garbage(h.ptr(), 0, &PURGE) }, Err(91));
    assert_eq!(h.logged_targets, [original]);
    assert_eq!(h.slots[0], h.objects[1].ptr());
    assert!(h.destroyed_targets.is_empty());
    assert!(h.deleted_targets.is_empty());
    assert!(h.objects.iter().all(|object| object.calls.is_empty()));
    assert_eq!(h.events.last(), Some(&Event::LogDestroy(0)));
}
