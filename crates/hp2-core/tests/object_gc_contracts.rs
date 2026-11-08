use hp2_core::object_gc::*;
use hp2_core::object_lifecycle::{Hp2ObjectLifecycleServices, RF_NATIVE, RF_UNREACHABLE};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};

#[derive(Debug, PartialEq, Eq, Clone)]
enum Event {
    Roots,
    Pending,
    Selected(i32),
    Set(i32),
    Clear(i32),
    Serialize(i32),
    Referenced(i32, i32),
    Route(i32),
    Validate(i32),
    Purge,
}
struct Object {
    id: i32,
    flags: u32,
    first: *mut c_void,
    second: *mut c_void,
    append: bool,
    fail: bool,
    route: bool,
    rewrite: bool,
    serializations: i32,
}
struct Host {
    objects: Vec<Option<Box<Object>>>,
    names: [Option<Box<u32>>; 3],
    roots: Vec<*mut c_void>,
    pending: Vec<*mut c_void>,
    counter: i32,
    state: Hp2ObjectGcMarkState,
    events: Vec<Event>,
    failure: Option<Event>,
    rewrite_slot: *mut *mut c_void,
    purged: Vec<i32>,
}
thread_local! { static ACTIVE: Cell<*mut Host> = const { Cell::new(null_mut()) }; }
fn host() -> *mut Host {
    ACTIVE.with(Cell::get)
}
unsafe fn hit(event: Event) -> i32 {
    unsafe {
        let h = host();
        let fail = (*h).failure.as_ref() == Some(&event);
        (*h).events.push(event);
        if fail { 77 } else { 0 }
    }
}
struct Fixture {
    h: Box<Host>,
    previous: *mut Host,
}
impl Fixture {
    fn new() -> Self {
        let mut h = Box::new(Host {
            objects: Vec::new(),
            names: [Some(Box::new(0)), None, Some(Box::new(RF_NATIVE))],
            roots: Vec::new(),
            pending: Vec::new(),
            counter: 99,
            state: Hp2ObjectGcMarkState {
                context: null_mut(),
                ref_obj: null_mut(),
            },
            events: Vec::new(),
            failure: None,
            rewrite_slot: null_mut(),
            purged: Vec::new(),
        });
        let pointer = &mut *h as *mut Host;
        let previous = ACTIVE.with(|active| active.replace(pointer));
        Self { h, previous }
    }
    fn add(&mut self, id: i32, flags: u32) -> *mut c_void {
        let mut object = Box::new(Object {
            id,
            flags,
            first: null_mut(),
            second: null_mut(),
            append: false,
            fail: false,
            route: true,
            rewrite: false,
            serializations: 0,
        });
        let pointer = (&mut *object as *mut Object).cast();
        self.h.objects.push(Some(object));
        pointer
    }
    fn pointer(&mut self) -> *mut c_void {
        (&mut *self.h as *mut Host).cast()
    }
    fn initialize(&mut self) -> Result<(), i32> {
        let pointer = self.pointer();
        unsafe {
            initialize_marks(
                pointer,
                addr_of_mut!((*pointer.cast::<Host>()).counter),
                &LIFE,
                &MARK,
            )
        }
    }
    fn roots(&mut self, keep: u32, required: u32) -> Result<(), i32> {
        let pointer = self.pointer();
        unsafe { serialize_roots(pointer, pointer, keep, required, &LIFE, &ROOTS) }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.previous));
    }
}
unsafe extern "C" fn index(object: *mut c_void) -> u32 {
    unsafe { (*object.cast::<Object>()).id as u32 }
}
unsafe extern "C" fn flags(object: *mut c_void) -> u32 {
    unsafe { (*object.cast::<Object>()).flags }
}
unsafe extern "C" fn set(object: *mut c_void, value: u32) -> i32 {
    unsafe {
        let object = object.cast::<Object>();
        let error = hit(Event::Set((*object).id));
        if error != 0 {
            return error;
        }
        (*object).flags |= value;
        if (*object).append {
            (*object).append = false;
            (*host()).objects.push(Some(Box::new(Object {
                id: 9,
                flags: 0,
                first: null_mut(),
                second: null_mut(),
                append: false,
                fail: false,
                route: true,
                rewrite: false,
                serializations: 0,
            })));
        }
        0
    }
}
unsafe extern "C" fn clear(object: *mut c_void, value: u32) -> i32 {
    unsafe {
        let object = object.cast::<Object>();
        let error = hit(Event::Clear((*object).id));
        if error == 0 {
            (*object).flags &= !value;
        }
        error
    }
}
unsafe extern "C" fn unused(_: *mut c_void) -> i32 {
    0
}
unsafe extern "C" fn unused_route(_: *mut c_void, _: i32) -> i32 {
    0
}
unsafe extern "C" fn unused_assert(_: i32) -> i32 {
    0
}
unsafe extern "C" fn count(_: *mut c_void) -> i32 {
    unsafe { (*host()).objects.len() as i32 }
}
unsafe extern "C" fn at(_: *mut c_void, index: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = (&mut (*host()).objects)[index as usize]
            .as_mut()
            .map_or(null_mut(), |object| (&mut **object as *mut Object).cast());
    }
    0
}
static LIFE: Hp2ObjectLifecycleServices = Hp2ObjectLifecycleServices {
    index,
    flags,
    set_flags: set,
    clear_flags: clear,
    destroy: unused,
    register: unused,
    post_load: unused,
    route_error: unused_route,
    table_len: count,
    table_at: at,
    assert_registrant: unused_assert,
};
unsafe extern "C" fn name_count() -> i32 {
    3
}
unsafe extern "C" fn name_at(index: i32) -> *mut c_void {
    unsafe {
        (&mut (*host()).names)[index as usize]
            .as_mut()
            .map_or(null_mut(), |name| (&mut **name as *mut u32).cast())
    }
}
unsafe extern "C" fn name_set(entry: *mut c_void, flags: u32) {
    unsafe {
        *entry.cast::<u32>() |= flags;
    }
}
unsafe extern "C" fn name_clear(entry: *mut c_void, flags: u32) -> i32 {
    unsafe {
        *entry.cast::<u32>() &= !flags;
    }
    0
}
unsafe extern "C" fn validate(object: *mut c_void) -> i32 {
    unsafe { hit(Event::Validate((*object.cast::<Object>()).id)) }
}
unsafe fn mark(archive: *mut c_void, slot: *mut c_void) -> Result<(), i32> {
    unsafe {
        let h = archive.cast::<Host>();
        mark_object(
            archive,
            addr_of_mut!((*h).state),
            slot,
            addr_of_mut!((*h).counter),
            1,
            &LIFE,
            &MARK,
        )
    }
}
unsafe extern "C" fn serialize(archive: *mut c_void, object: *mut c_void) -> i32 {
    unsafe {
        let object = object.cast::<Object>();
        let error = hit(Event::Serialize((*object).id));
        (*object).serializations += 1;
        if (*object).rewrite {
            (*host()).rewrite_slot.write_unaligned(null_mut());
        }
        if error != 0 {
            return error;
        }
        if (*object).fail {
            return 71;
        }
        if let Err(error) = mark(archive, addr_of_mut!((*object).first).cast()) {
            return error;
        }
        if let Err(error) = mark(archive, addr_of_mut!((*object).second).cast()) {
            return error;
        }
        if (*object).route {
            (*object).flags |= RF_DEBUG_SERIALIZE;
        }
        0
    }
}
unsafe extern "C" fn route(object: *mut c_void) -> i32 {
    unsafe {
        hit(Event::Route((*object.cast::<Object>()).id));
    }
    72
}
unsafe extern "C" fn referenced(object: *mut c_void, context: *mut c_void) -> i32 {
    unsafe {
        hit(Event::Referenced(
            (*object.cast::<Object>()).id,
            if context.is_null() {
                -1
            } else {
                (*context.cast::<Object>()).id
            },
        ))
    }
}
static MARK: Hp2ObjectGcMarkServices = Hp2ObjectGcMarkServices {
    name_count,
    name_at,
    name_set_flags: name_set,
    name_clear_flags: name_clear,
    validate,
    serialize,
    route_error: route,
    referenced,
};
unsafe extern "C" fn roots(archive: *mut c_void) -> i32 {
    unsafe {
        let error = hit(Event::Roots);
        if error != 0 {
            return error;
        }
        let h = archive.cast::<Host>();
        let mut i = 0;
        while i < (*h).roots.len() {
            let slot = (*h).roots.as_mut_ptr().add(i).cast();
            if let Err(error) = mark(archive, slot) {
                return error;
            }
            i += 1;
        }
        0
    }
}
unsafe extern "C" fn pending(archive: *mut c_void) -> i32 {
    unsafe {
        let error = hit(Event::Pending);
        if error != 0 {
            return error;
        }
        let h = archive.cast::<Host>();
        let mut i = 0;
        while i < (*h).pending.len() {
            let slot = (*h).pending.as_mut_ptr().add(i).cast();
            if let Err(error) = mark(archive, slot) {
                return error;
            }
            i += 1;
        }
        0
    }
}
unsafe extern "C" fn selected(archive: *mut c_void, mut object: *mut c_void) -> i32 {
    unsafe {
        let error = hit(Event::Selected((*object.cast::<Object>()).id));
        if error != 0 {
            return error;
        }
        mark(archive, addr_of_mut!(object).cast())
            .err()
            .unwrap_or(0)
    }
}
static ROOTS: Hp2ObjectGcRootServices = Hp2ObjectGcRootServices {
    roots,
    pending_localized: pending,
    object: selected,
};
unsafe extern "C" fn root_set(archive: *mut c_void, keep: u32, required: u32) -> i32 {
    unsafe {
        serialize_roots(archive, archive, keep, required, &LIFE, &ROOTS)
            .err()
            .unwrap_or(0)
    }
}
unsafe extern "C" fn with_archive(
    original: *mut c_void,
    slot: *mut c_void,
    keep: u32,
    ignore: i32,
    services: *const Hp2ObjectGcReferenceServices,
    result: *mut i32,
) -> i32 {
    unsafe {
        let h = host();
        (*h).state = Hp2ObjectGcMarkState {
            context: null_mut(),
            ref_obj: original,
        };
        if let Err(error) = initialize_marks(h.cast(), addr_of_mut!((*h).counter), &LIFE, &MARK) {
            return error;
        }
        match reference_marked(h.cast(), original, slot, keep, ignore, &LIFE, &*services) {
            Ok(value) => {
                *result = value;
                0
            }
            Err(error) => error,
        }
    }
}
unsafe extern "C" fn referencing(archive: *mut c_void) -> *mut c_void {
    unsafe { (*archive.cast::<Host>()).state.ref_obj }
}
static REFERENCE: Hp2ObjectGcReferenceServices = Hp2ObjectGcReferenceServices {
    with_archive,
    roots: root_set,
    referencing,
};
unsafe extern "C" fn query(slot: *mut c_void, keep: u32, ignore: i32, result: *mut i32) -> i32 {
    unsafe {
        match is_referenced(slot, keep, ignore, &REFERENCE) {
            Ok(value) => {
                *result = value;
                0
            }
            Err(error) => error,
        }
    }
}
unsafe extern "C" fn purge() -> i32 {
    unsafe {
        let error = hit(Event::Purge);
        if error != 0 {
            return error;
        }
        let h = host();
        for object in (*h).objects.iter().flatten() {
            if object.flags & RF_UNREACHABLE != 0 && object.flags & RF_NATIVE == 0 {
                (*h).purged.push(object.id);
            }
        }
        0
    }
}
static DECISION: Hp2ObjectGcDecisionServices = Hp2ObjectGcDecisionServices {
    is_referenced: query,
    purge,
};

#[test]
fn initialization_visits_live_appended_slots_and_marks_names_without_losing_flags() {
    let mut f = Fixture::new();
    let a = f.add(0, RF_NATIVE);
    f.h.objects.push(None);
    unsafe {
        (*a.cast::<Object>()).append = true;
    }
    assert_eq!(f.initialize(), Ok(()));
    assert_eq!(f.h.counter, 0);
    assert_eq!(
        unsafe { flags(a) },
        RF_NATIVE | RF_UNREACHABLE | RF_TAG_GARBAGE
    );
    assert_eq!(
        f.h.objects[2].as_ref().unwrap().flags,
        RF_UNREACHABLE | RF_TAG_GARBAGE
    );
    assert_eq!(**f.h.names[0].as_ref().unwrap(), RF_UNREACHABLE);
    assert_eq!(**f.h.names[2].as_ref().unwrap(), RF_NATIVE | RF_UNREACHABLE);
}

#[test]
fn root_and_pending_graphs_preserve_cycles_and_keep_flags_require_all_required_bits() {
    let mut f = Fixture::new();
    let root = f.add(0, 0);
    let a = f.add(1, 0);
    let b = f.add(2, 0);
    let pending = f.add(3, 0);
    let kept = f.add(4, RF_NATIVE);
    let rejected = f.add(5, RF_NATIVE);
    let unreachable = f.add(6, 0);
    unsafe {
        (*root.cast::<Object>()).first = a;
        (*a.cast::<Object>()).first = b;
        (*b.cast::<Object>()).first = a;
    }
    f.h.roots.push(root);
    f.h.pending.push(pending);
    f.initialize().unwrap();
    unsafe {
        (*rejected.cast::<Object>()).flags &= !RF_TAG_GARBAGE;
    }
    assert_eq!(f.roots(RF_NATIVE, RF_TAG_GARBAGE), Ok(()));
    for object in [root, a, b, pending, kept] {
        assert_eq!(unsafe { flags(object) } & RF_UNREACHABLE, 0);
        assert_eq!(unsafe { (*object.cast::<Object>()).serializations }, 1);
    }
    for object in [rejected, unreachable] {
        assert_ne!(unsafe { flags(object) } & RF_UNREACHABLE, 0);
    }
    assert!(!f.h.events.contains(&Event::Selected(5)));
    let roots =
        f.h.events
            .iter()
            .position(|event| *event == Event::Roots)
            .unwrap();
    let pending =
        f.h.events
            .iter()
            .position(|event| *event == Event::Pending)
            .unwrap();
    let selected =
        f.h.events
            .iter()
            .position(|event| *event == Event::Selected(4))
            .unwrap();
    assert!(roots < pending && pending < selected);
    assert!(f.h.state.context.is_null());
}

#[test]
fn unaligned_eliminated_reference_is_validated_then_skipped_without_traversal() {
    // Source `FArchiveTagUsed::operator<<` (Core/Src/UnObj.cpp) copies the
    // slot into a local, nulls only that local for eliminated objects, and
    // never writes the slot back: the caller keeps its reference, the object
    // is not traversed, and it stays unreachable so the purge frees it.
    let mut f = Fixture::new();
    let object = f.add(1, RF_ELIMINATE_OBJECT);
    f.initialize().unwrap();
    let mut bytes = [0u8; 2 * std::mem::size_of::<usize>()];
    let slot = unsafe { bytes.as_mut_ptr().add(1).cast::<*mut c_void>() };
    unsafe {
        slot.write_unaligned(object);
    }
    let pointer = f.pointer();
    assert_eq!(unsafe { mark(pointer, slot.cast()) }, Ok(()));
    assert_eq!(unsafe { slot.read_unaligned() }, object);
    assert_eq!(f.h.counter, 1);
    assert_eq!(unsafe { (*object.cast::<Object>()).serializations }, 0);
    assert!(f.h.events.contains(&Event::Validate(1)));
    assert_ne!(unsafe { flags(object) } & RF_UNREACHABLE, 0);
}

#[test]
fn successful_reentrant_serialization_republishes_the_saved_slot_and_restores_context() {
    let mut f = Fixture::new();
    let object = f.add(1, 0);
    f.initialize().unwrap();
    let mut slot = object;
    f.h.rewrite_slot = addr_of_mut!(slot);
    unsafe {
        (*object.cast::<Object>()).rewrite = true;
    }
    let pointer = f.pointer();
    assert_eq!(unsafe { mark(pointer, addr_of_mut!(slot).cast()) }, Ok(()));
    assert_eq!(slot, object);
    assert!(f.h.state.context.is_null());
    assert_eq!(
        unsafe { flags(object) } & (RF_UNREACHABLE | RF_DEBUG_SERIALIZE),
        RF_DEBUG_SERIALIZE
    );
}

#[test]
fn throwing_serialize_keeps_context_flags_and_callback_slot_mutation_without_copyback() {
    let mut f = Fixture::new();
    let object = f.add(1, 0);
    f.initialize().unwrap();
    let mut slot = object;
    f.h.rewrite_slot = addr_of_mut!(slot);
    unsafe {
        (*object.cast::<Object>()).rewrite = true;
        (*object.cast::<Object>()).fail = true;
    }
    let pointer = f.pointer();
    assert_eq!(unsafe { mark(pointer, addr_of_mut!(slot).cast()) }, Err(71));
    assert!(slot.is_null());
    assert_eq!(f.h.state.context, object);
    assert_eq!(
        unsafe { flags(object) } & (RF_UNREACHABLE | RF_DEBUG_SERIALIZE),
        0
    );
    assert_eq!(f.h.counter, 1);
}

#[test]
fn missing_super_serialize_route_leaves_claimed_object_context_and_stops_before_restore() {
    let mut f = Fixture::new();
    let object = f.add(1, 0);
    f.initialize().unwrap();
    unsafe {
        (*object.cast::<Object>()).route = false;
    }
    let pointer = f.pointer();
    let mut slot = object;
    assert_eq!(unsafe { mark(pointer, addr_of_mut!(slot).cast()) }, Err(72));
    assert_eq!(f.h.state.context, object);
    assert_eq!(slot, object);
    assert_eq!(unsafe { flags(object) } & RF_UNREACHABLE, 0);
    assert_eq!(f.h.events.last(), Some(&Event::Route(1)));
}

#[test]
fn reference_query_reports_owner_or_ignores_and_restores_the_actual_reference_slot() {
    let mut f = Fixture::new();
    let owner = f.add(0, 0);
    let target = f.add(1, 0);
    unsafe {
        (*owner.cast::<Object>()).first = target;
    }
    f.h.roots.push(owner);
    let mut query = target;
    assert_eq!(
        unsafe { is_referenced(addr_of_mut!(query).cast(), RF_NATIVE, 0, &REFERENCE) },
        Ok(1)
    );
    assert_eq!(query, owner);
    assert_eq!(f.h.state.ref_obj, owner);
    let slot = unsafe { addr_of_mut!((*owner.cast::<Object>()).first).cast() };
    assert_eq!(
        unsafe { is_referenced(slot, RF_NATIVE, 1, &REFERENCE) },
        Ok(0)
    );
    assert_eq!(unsafe { (*owner.cast::<Object>()).first }, target);
    assert_ne!(unsafe { flags(target) } & RF_UNREACHABLE, 0);
}

#[test]
fn failed_ignored_query_does_not_restore_a_slot_or_run_later_root_phases() {
    let mut f = Fixture::new();
    let owner = f.add(0, 0);
    let target = f.add(1, 0);
    unsafe {
        (*owner.cast::<Object>()).first = target;
    }
    f.h.roots.push(owner);
    f.h.failure = Some(Event::Roots);
    let slot = unsafe { addr_of_mut!((*owner.cast::<Object>()).first).cast() };
    assert_eq!(
        unsafe { is_referenced(slot, RF_NATIVE, 1, &REFERENCE) },
        Err(77)
    );
    assert!(unsafe { (*owner.cast::<Object>()).first }.is_null());
    assert_eq!(unsafe { flags(target) } & RF_TAG_GARBAGE, 0);
    assert!(!f.h.events.contains(&Event::Pending));
}

#[test]
fn attempt_delete_preserves_native_and_referenced_objects_but_purges_unreachable_cycle() {
    let mut f = Fixture::new();
    let owner = f.add(0, RF_NATIVE);
    let a = f.add(1, 0);
    let b = f.add(2, 0);
    unsafe {
        (*owner.cast::<Object>()).first = a;
        (*a.cast::<Object>()).first = b;
        (*b.cast::<Object>()).first = a;
    }
    let mut candidate = owner;
    assert_eq!(
        unsafe {
            attempt_delete(
                addr_of_mut!(candidate).cast(),
                RF_NATIVE,
                1,
                &LIFE,
                &DECISION,
            )
        },
        Ok(0)
    );
    assert!(f.h.events.is_empty());
    candidate = a;
    assert_eq!(
        unsafe {
            attempt_delete(
                addr_of_mut!(candidate).cast(),
                RF_NATIVE,
                1,
                &LIFE,
                &DECISION,
            )
        },
        Ok(0)
    );
    assert_eq!(candidate, a);
    assert!(f.h.purged.is_empty());
    unsafe {
        (*owner.cast::<Object>()).first = null_mut();
    }
    assert_eq!(
        unsafe {
            attempt_delete(
                addr_of_mut!(candidate).cast(),
                RF_NATIVE,
                1,
                &LIFE,
                &DECISION,
            )
        },
        Ok(1)
    );
    assert_eq!(f.h.purged, vec![1, 2]);
    assert_eq!(unsafe { flags(owner) } & RF_UNREACHABLE, 0);
}

#[test]
fn collector_stops_after_marking_failure_without_purging_any_graph() {
    let mut f = Fixture::new();
    let root = f.add(0, 0);
    let target = f.add(1, 0);
    f.h.roots.push(root);
    f.initialize().unwrap();
    f.h.failure = Some(Event::Pending);
    let pointer = f.pointer();
    assert_eq!(
        unsafe { collect(pointer, RF_NATIVE, root_set, purge) },
        Err(77)
    );
    assert_eq!(unsafe { flags(root) } & RF_UNREACHABLE, 0);
    assert_ne!(unsafe { flags(target) } & RF_UNREACHABLE, 0);
    assert!(f.h.purged.is_empty());
    assert!(!f.h.events.contains(&Event::Purge));
}
