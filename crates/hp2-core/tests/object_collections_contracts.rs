use hp2_core::object_collections::{self, Hp2ObjectCollectionArchiveServices, ObjectCollection};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};

fn object(id: usize) -> *mut c_void {
    id as *mut c_void
}
fn values(collection: &ObjectCollection) -> Vec<*mut c_void> {
    (0..collection.len()).map(|i| collection.at(i)).collect()
}

#[test]
fn duplicates_are_appended_and_remove_all_preserves_survivor_order() {
    let mut store = ObjectCollection::new();
    for id in [1, 2, 1, 3, 2, 1] {
        store.append(object(id));
    }
    store.remove_all(object(1));
    assert_eq!(values(&store), [object(2), object(3), object(2)]);
    store.remove_all(object(9));
    assert_eq!(values(&store), [object(2), object(3), object(2)]);
    store.remove_all(object(2));
    store.remove_all(object(3));
    assert_eq!((store.len(), store.capacity()), (0, 0));
    store.append(object(4));
    store.release();
    assert_eq!((store.len(), store.capacity()), (0, 0));
}

struct Registration {
    store: ObjectCollection,
    visited: Vec<*mut c_void>,
    fail: *mut c_void,
    nested: bool,
}
unsafe extern "C" fn register(context: *mut c_void, entry: *mut c_void) -> i32 {
    unsafe {
        let state = context.cast::<Registration>();
        (*state).visited.push(entry);
        if entry == (*state).fail {
            return 71;
        }
        if entry == object(1) && !(*state).nested {
            (*state).nested = true;
            // Force allocation while the outer production drain is suspended.
            for id in 2..130 {
                (*state).store.append(object(id));
            }
        }
        0
    }
}

#[test]
fn registrant_live_drain_observes_hook_growth_and_releases_backing() {
    let state = UnsafeCell::new(Registration {
        store: ObjectCollection::new(),
        visited: vec![],
        fail: null_mut(),
        nested: false,
    });
    unsafe {
        let p = state.get();
        (*p).store.append(object(1));
        assert_eq!(
            object_collections::drain(addr_of_mut!((*p).store), p.cast(), register),
            Ok(())
        );
        assert_eq!((*p).visited, (1..130).map(object).collect::<Vec<_>>());
        assert_eq!(((*p).store.len(), (*p).store.capacity()), (0, 0));
    }
}

#[test]
fn registrant_failure_keeps_live_partial_storage_without_visiting_followers() {
    let state = UnsafeCell::new(Registration {
        store: ObjectCollection::new(),
        visited: vec![],
        fail: object(2),
        nested: false,
    });
    unsafe {
        let p = state.get();
        (*p).store.append(object(1));
        assert_eq!(
            object_collections::drain(addr_of_mut!((*p).store), p.cast(), register),
            Err(71)
        );
        assert_eq!((*p).visited, [object(1), object(2)]);
        assert_eq!((*p).store.len(), 129);
        assert!((*p).store.capacity() >= 129);
        assert_eq!((*p).store.at(128), object(129));
    }
}

unsafe extern "C" fn reorder_suffix(data: *mut *mut c_void, count: i32) -> i32 {
    unsafe {
        assert_eq!(count, 3);
        assert_eq!(*data, object(2));
        assert_eq!(*data.add(2), object(4));
        // A native primitive controls exact output/tie order, not a Rust sort.
        std::ptr::swap(data, data.add(2));
    }
    0
}
unsafe extern "C" fn failed_sort(data: *mut *mut c_void, _: i32) -> i32 {
    unsafe {
        *data = object(8);
    }
    83
}

#[test]
fn loaded_suffix_uses_in_place_native_order_and_retains_partial_error_writes() {
    let mut store = ObjectCollection::new();
    for id in 1..5 {
        store.append(object(id));
    }
    unsafe {
        assert_eq!(
            object_collections::sort_suffix(&mut store, 1, reorder_suffix),
            Ok(())
        );
        assert_eq!(values(&store), [object(1), object(4), object(3), object(2)]);
        assert_eq!(
            object_collections::sort_suffix(&mut store, 2, failed_sort),
            Err(83)
        );
    }
    assert_eq!(values(&store), [object(1), object(4), object(8), object(2)]);
    // A subsequent preload can grow storage after the sort span has ended.
    for id in 5..200 {
        store.append(object(id));
    }
    assert_eq!(store.at(2), object(8));
}

struct Archive {
    roots: ObjectCollection,
    loading: bool,
    wire_count: i32,
    seen_count: i32,
    bytes: (usize, usize),
    seen: Vec<*mut c_void>,
    visits: i32,
    writes_visible: bool,
    fail_count: bool,
    fail_object: i32,
    grow: bool,
    replacement: *mut c_void,
}
impl Archive {
    fn new(loading: bool) -> Self {
        Self {
            roots: ObjectCollection::new(),
            loading,
            wire_count: 3,
            seen_count: -1,
            bytes: (0, 0),
            seen: vec![],
            visits: 0,
            writes_visible: true,
            fail_count: false,
            fail_object: -1,
            grow: false,
            replacement: null_mut(),
        }
    }
}
unsafe extern "C" fn count_bytes(context: *mut c_void, used: usize, allocated: usize) -> i32 {
    unsafe {
        (*context.cast::<Archive>()).bytes = (used, allocated);
    }
    0
}
unsafe extern "C" fn is_loading(context: *mut c_void) -> i32 {
    unsafe { i32::from((*context.cast::<Archive>()).loading) }
}
unsafe extern "C" fn count(context: *mut c_void, count: *mut i32) -> i32 {
    unsafe {
        let p = context.cast::<Archive>();
        (*p).seen_count = *count;
        if (*p).loading {
            *count = (*p).wire_count;
        }
        if (*p).fail_count {
            return 61;
        }
    }
    0
}
unsafe extern "C" fn archive_object(context: *mut c_void, value: *mut *mut c_void) -> i32 {
    unsafe {
        let p = context.cast::<Archive>();
        let index = (*p).visits;
        (*p).visits += 1;
        if !(*p).loading {
            (*p).seen.push(*value);
        }
        if (*p).loading {
            // The slot is visible before the archive leaf is entered.
            assert_eq!((*p).roots.len(), index + 1);
            *value = object(100 + index as usize);
        } else if !(*p).replacement.is_null() {
            *value = (*p).replacement;
            if (*p).roots.at(index) != *value {
                (*p).writes_visible = false;
            }
        }
        if (*p).grow {
            (*p).grow = false;
            for id in 2..130 {
                (*p).roots.append(object(id));
            }
        }
        if index == (*p).fail_object {
            return 67;
        }
    }
    0
}
const ARCHIVE: Hp2ObjectCollectionArchiveServices = Hp2ObjectCollectionArchiveServices {
    count_bytes,
    is_loading,
    count,
    object: archive_object,
};
unsafe fn serialize(p: *mut Archive) -> Result<(), i32> {
    unsafe { object_collections::serialize(addr_of_mut!((*p).roots), p.cast(), &ARCHIVE) }
}

#[test]
fn root_reference_visit_uses_live_order_and_reacquires_slots_after_growth() {
    let state = UnsafeCell::new(Archive::new(false));
    unsafe {
        let p = state.get();
        (*p).roots.append(object(1));
        (*p).grow = true;
        (*p).replacement = object(999);
        let capacity = (*p).roots.capacity();
        assert_eq!(serialize(p), Ok(()));
        assert_eq!((*p).seen_count, 1);
        assert_eq!(
            (*p).bytes,
            (
                std::mem::size_of::<*mut c_void>(),
                capacity * std::mem::size_of::<*mut c_void>()
            )
        );
        assert_eq!((*p).seen, (1..130).map(object).collect::<Vec<_>>());
        assert_eq!((*p).roots.at(0), object(999));
        assert_eq!((*p).roots.at(128), object(999));
        assert!(
            (*p).writes_visible,
            "root pointer writes must be visible before callback reentry"
        );
    }
}

#[test]
fn root_loading_failure_publishes_written_pointer_and_only_started_slots() {
    let state = UnsafeCell::new(Archive::new(true));
    unsafe {
        let p = state.get();
        (*p).roots.append(object(9));
        (*p).fail_object = 1;
        assert_eq!(serialize(p), Err(67));
        assert_eq!(values(&(*p).roots), [object(100), object(101)]);
        assert_eq!((*p).visits, 2);
    }
}

#[test]
fn root_count_failure_preserves_old_storage_and_empty_load_releases_backing() {
    let state = UnsafeCell::new(Archive::new(true));
    unsafe {
        let p = state.get();
        (*p).roots.append(object(9));
        (*p).fail_count = true;
        let capacity = (*p).roots.capacity();
        assert_eq!(serialize(p), Err(61));
        assert_eq!(values(&(*p).roots), [object(9)]);
        assert_eq!((*p).roots.capacity(), capacity);
        assert!((*p).seen.is_empty());
        (*p).fail_count = false;
        (*p).wire_count = 0;
        assert_eq!(serialize(p), Ok(()));
        assert_eq!(((*p).roots.len(), (*p).roots.capacity()), (0, 0));
    }
}

#[test]
fn root_reference_failure_keeps_pointer_written_before_error() {
    let state = UnsafeCell::new(Archive::new(false));
    unsafe {
        let p = state.get();
        (*p).roots.append(object(1));
        (*p).roots.append(object(2));
        (*p).replacement = object(7);
        (*p).fail_object = 0;
        assert_eq!(serialize(p), Err(67));
        assert_eq!(values(&(*p).roots), [object(7), object(2)]);
        assert_eq!((*p).seen, [object(1)]);
    }
}
