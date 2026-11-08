use hp2_abi::engine::generated::UObject;
use hp2_core::object_storage::{self as storage, Hp2ObjectStorageServices, ObjectStorage};
use std::ffi::c_void;
use std::ptr;

// Pure Rust storage uses the same compiler-probed header at offset zero; this
// does not construct or emulate a C++ object/vtable.
#[repr(C)]
struct Object {
    header: UObject,
    storage: *mut ObjectStorage,
    visit_context: *mut VisitContext,
}

fn object(table: *mut ObjectStorage, name: i32) -> Box<Object> {
    let mut header = unsafe { std::mem::zeroed::<UObject>() };
    header.index = -1;
    header.name = name.to_ne_bytes();
    Box::new(Object {
        header,
        storage: table,
        visit_context: ptr::null_mut(),
    })
}
fn pointer(object: &mut Object) -> *mut c_void {
    (object as *mut Object).cast()
}

fn bucket(object: &Object) -> i32 {
    let outer = object.header.outer.cast::<UObject>();
    let outer_index = if outer.is_null() {
        0
    } else {
        // The test retains the actual outer header through every operation.
        unsafe { (*outer).index }
    };
    storage::hash_bucket(i32::from_ne_bytes(object.header.name), outer_index)
}

unsafe extern "C" fn check(kind: i32, condition: i32) -> i32 {
    if condition != 0 { 0 } else { 10 + kind }
}
const SERVICES: Hp2ObjectStorageServices = Hp2ObjectStorageServices {
    check,
};

unsafe fn add(table: *mut ObjectStorage, object: &mut Object, index: i32) {
    unsafe {
        storage::add(table, pointer(object), index, &SERVICES).unwrap();
    }
}
unsafe fn remove(table: *mut ObjectStorage, object: &mut Object) {
    unsafe {
        storage::unhash(
            table,
            pointer(object),
            bucket(object),
            &SERVICES,
        )
        .unwrap();
        (*table).remove(object.header.index);
    }
}

#[test]
fn growth_bounds_identity_and_instance_isolation() {
    let mut first = ObjectStorage::new();
    let mut second = ObjectStorage::new();
    let mut objects = Vec::new();
    for index in 0..2050 {
        let mut value = object(&mut first, index);
        unsafe {
            add(&mut first, &mut value, -1);
        }
        assert_eq!(value.header.index, index);
        assert_eq!(first.at(index), pointer(&mut value));
        objects.push(value);
    }
    assert_eq!(first.len(), 2050);
    assert!(first.at(-1).is_null());
    assert!(first.at(i32::MAX).is_null());
    assert!(first.at(2050).is_null());
    let mut other = object(&mut second, 5);
    unsafe {
        add(&mut second, &mut other, -1);
    }
    assert_eq!(second.len(), 1);
    assert_eq!(second.at(0), pointer(&mut other));
    assert_eq!(first.at(0), pointer(&mut objects[0]));
}

#[test]
fn holes_reuse_lifo_and_replacement_keeps_address_and_index() {
    let mut table = ObjectStorage::new();
    let mut objects: Vec<_> = (0..4).map(|name| object(&mut table, name)).collect();
    for value in &mut objects {
        unsafe {
            add(&mut table, value, -1);
        }
    }
    unsafe {
        remove(&mut table, &mut objects[1]);
        remove(&mut table, &mut objects[3]);
        add(&mut table, &mut objects[3], -1);
    }
    assert_eq!(objects[3].header.index, 3);
    assert!(table.at(1).is_null());
    let original = pointer(&mut objects[0]);
    unsafe {
        remove(&mut table, &mut objects[0]);
        storage::reserve_replacement(&mut table, 0, &SERVICES).unwrap();
        add(&mut table, &mut objects[0], 0);
        add(&mut table, &mut objects[1], -1);
    }
    assert_eq!(table.at(0), original);
    assert_eq!(objects[0].header.index, 0);
    assert_eq!(objects[1].header.index, 1);
    assert_eq!(table.len(), 4);
}

#[test]
fn collisions_unlink_middle_head_tail_without_clearing_removed_link() {
    let mut table = ObjectStorage::new();
    let mut a = object(&mut table, 17);
    let mut b = object(&mut table, 4113);
    let mut c = object(&mut table, 17);
    let mut outer = object(&mut table, 0);
    outer.header.index = 4096;
    c.header.outer = pointer(&mut outer);
    unsafe {
        add(&mut table, &mut a, -1);
        add(&mut table, &mut b, -1);
        add(&mut table, &mut c, -1);
    }
    assert_eq!(table.occupied_buckets(), 1);
    assert_eq!(table.hash_head(17), pointer(&mut c));
    assert_eq!(c.header.hash_next, pointer(&mut b));
    assert_eq!(b.header.hash_next, pointer(&mut a));
    unsafe {
        storage::unhash(&mut table, pointer(&mut b), 17, &SERVICES).unwrap();
    }
    assert_eq!(c.header.hash_next, pointer(&mut a));
    assert_eq!(b.header.hash_next, pointer(&mut a)); // Unhash never clears this link.
    unsafe {
        storage::unhash(&mut table, pointer(&mut c), 17, &SERVICES).unwrap();
    }
    assert_eq!(table.hash_head(17), pointer(&mut a));
    unsafe {
        storage::unhash(&mut table, pointer(&mut a), 17, &SERVICES).unwrap();
    }
    assert!(table.hash_head(17).is_null());
    assert_eq!(table.occupied_buckets(), 0);
    assert_eq!(storage::hash_bucket(-1, 0), 4095);
}

#[test]
fn failed_checks_stop_before_later_mutations_and_keep_original_order() {
    let mut table = ObjectStorage::new();
    let mut original = object(&mut table, 8);
    let mut absent = object(&mut table, 9);
    unsafe {
        add(&mut table, &mut original, -1);
    }
    assert_eq!(
        unsafe { storage::unhash(&mut table, pointer(&mut absent), 8, &SERVICES) },
        Err(11)
    );
    assert_eq!(table.hash_head(8), pointer(&mut original));
    unsafe {
        remove(&mut table, &mut original);
    }
    assert_eq!(
        unsafe { storage::reserve_replacement(&mut table, 1, &SERVICES) },
        Err(13)
    );
    // Explicit insertion intentionally doesn't consume the free stack. A failed
    // subsequent free-slot check has already popped it, but hasn't assigned the
    // new object's index or installed it in either table.
    unsafe {
        add(&mut table, &mut original, 0);
    }
    assert_eq!(
        unsafe { storage::add(&mut table, pointer(&mut absent), -1, &SERVICES) },
        Err(10)
    );
    assert_eq!(absent.header.index, -1);
    assert_eq!(table.at(0), pointer(&mut original));
    assert!(table.hash_head(9).is_null());
    unsafe {
        add(&mut table, &mut absent, -1);
    }
    assert_eq!(absent.header.index, 1);
}

struct Scan {
    table: *mut ObjectStorage,
    created: Vec<Box<Object>>,
    seen: Vec<i32>,
    fail: bool,
    cursor: *mut i32,
}
unsafe extern "C" fn growing_predicate(
    value: *mut c_void,
    context: *mut c_void,
    found: *mut i32,
) -> i32 {
    unsafe {
        let scan = context.cast::<Scan>();
        let index = (*value.cast::<Object>()).header.index;
        if *(*scan).cursor != index {
            return 72;
        }
        (*scan).seen.push(index);
        if index == 0 {
            // Force growth/reallocation while the outer Rust scan is suspended.
            for name in 1..2050 {
                let mut value = object((*scan).table, name);
                if let Err(status) = storage::add((*scan).table, pointer(&mut value), -1, &SERVICES)
                {
                    return status;
                }
                (*scan).created.push(value);
            }
            if (*scan).fail {
                return 71;
            }
        }
        *found = i32::from(index == 2049);
        0
    }
}

#[test]
fn iterator_observes_reentrant_growth_without_borrowing_vector_across_predicate() {
    let mut table = ObjectStorage::new();
    let mut first = object(&mut table, 0);
    unsafe {
        add(&mut table, &mut first, -1);
    }
    let mut index = -1;
    let mut scan = Scan {
        table: &mut table,
        created: Vec::new(),
        seen: Vec::new(),
        fail: false,
        cursor: &raw mut index,
    };
    assert_eq!(
        unsafe {
            storage::next_index(
                &mut table,
                &raw mut index,
                (&raw mut scan).cast(),
                growing_predicate,
            )
        },
        Ok(())
    );
    assert_eq!(index, 2049);
    assert_eq!(scan.seen, (0..2050).collect::<Vec<_>>());
    assert_eq!(
        unsafe {
            storage::next_index(
                &mut table,
                &raw mut index,
                (&raw mut scan).cast(),
                growing_predicate,
            )
        },
        Ok(())
    );
    assert_eq!(index, 2050);
}

#[test]
fn iterator_error_preserves_reentrant_mutations_and_stops_scan() {
    let mut table = ObjectStorage::new();
    let mut first = object(&mut table, 0);
    unsafe {
        add(&mut table, &mut first, -1);
    }
    let mut index = -1;
    let mut scan = Scan {
        table: &mut table,
        created: Vec::new(),
        seen: Vec::new(),
        fail: true,
        cursor: &raw mut index,
    };
    assert_eq!(
        unsafe {
            storage::next_index(
                &mut table,
                &raw mut index,
                (&raw mut scan).cast(),
                growing_predicate,
            )
        },
        Err(71)
    );
    assert_eq!(index, 0);
    assert_eq!(scan.seen, vec![0]);
    assert_eq!(table.len(), 2050);
    assert_eq!(table.at(2049), pointer(&mut scan.created[2048]));
}

unsafe extern "C" fn index_is_even(value: *mut c_void, _: *mut c_void, found: *mut i32) -> i32 {
    unsafe {
        *found = i32::from((*value.cast::<Object>()).header.index % 2 == 0);
    }
    0
}

#[test]
fn iterator_skips_holes_and_reads_replaced_slots_live() {
    let mut table = ObjectStorage::new();
    let mut values: Vec<_> = (0..5).map(|name| object(&mut table, name)).collect();
    for value in &mut values {
        unsafe {
            add(&mut table, value, -1);
        }
    }
    unsafe {
        remove(&mut table, &mut values[2]);
    }
    let mut index = 0;
    assert_eq!(
        unsafe { storage::next_index(&mut table, &raw mut index, ptr::null_mut(), index_is_even) },
        Ok(())
    );
    assert_eq!(index, 4);
    unsafe {
        add(&mut table, &mut values[2], -1);
    }
    index = 0;
    assert_eq!(
        unsafe { storage::next_index(&mut table, &raw mut index, ptr::null_mut(), index_is_even) },
        Ok(())
    );
    assert_eq!(index, 2);
}

#[test]
fn next_index_is_a_walks_the_class_chain_and_skips_non_matches() {
    #[repr(C)]
    struct FakeClass {
        base: UObject,
        super_field: *mut c_void,
    }
    fn new_fake() -> Box<FakeClass> {
        Box::new(FakeClass {
            base: unsafe { std::mem::zeroed() },
            super_field: ptr::null_mut(),
        })
    }
    fn class_ptr(class: &mut FakeClass) -> *mut c_void {
        (class as *mut FakeClass).cast()
    }

    let mut table = ObjectStorage::new();
    let mut base_class = new_fake();
    let mut mid_class = new_fake();
    mid_class.super_field = class_ptr(&mut base_class);
    let mut leaf_class = new_fake();
    leaf_class.super_field = class_ptr(&mut mid_class);
    let mut other_class = new_fake();
    let mid_ptr = class_ptr(&mut mid_class);

    // index 0: class chain leaf -> mid matches; index 1: unrelated class; index
    // 2: class itself is the target.
    let mut leaf_obj = object(&mut table, 1);
    leaf_obj.header.class = class_ptr(&mut leaf_class);
    let mut other_obj = object(&mut table, 2);
    other_obj.header.class = class_ptr(&mut other_class);
    let mut mid_obj = object(&mut table, 3);
    mid_obj.header.class = mid_ptr;
    unsafe {
        add(&mut table, &mut leaf_obj, -1);
        add(&mut table, &mut other_obj, -1);
        add(&mut table, &mut mid_obj, -1);
    }

    let mut index = -1;
    assert_eq!(
        unsafe { storage::next_index_is_a(&mut table, &raw mut index, mid_ptr) },
        Ok(())
    );
    assert_eq!(index, 0, "leaf_obj matches through the super_field chain");

    assert_eq!(
        unsafe { storage::next_index_is_a(&mut table, &raw mut index, mid_ptr) },
        Ok(())
    );
    assert_eq!(index, 2, "other_obj at index 1 is skipped, mid_obj matches directly");

    assert_eq!(
        unsafe { storage::next_index_is_a(&mut table, &raw mut index, mid_ptr) },
        Ok(())
    );
    assert_eq!(index, 3, "exhausted with no further match, stops at len");
}

#[test]
fn release_resets_slots_free_stack_and_hash_for_next_lifetime() {
    let mut table = ObjectStorage::new();
    let mut a = object(&mut table, 3);
    let mut b = object(&mut table, 5);
    unsafe {
        add(&mut table, &mut a, -1);
        add(&mut table, &mut b, -1);
        remove(&mut table, &mut a);
    }
    table.release();
    assert_eq!(table.len(), 0);
    assert_eq!(table.occupied_buckets(), 0);
    unsafe {
        add(&mut table, &mut a, -1);
    }
    assert_eq!(a.header.index, 0);
    assert!(a.header.hash_next.is_null());
    assert_eq!(table.len(), 1);
}

struct VisitContext {
    seen: Vec<i32>,
    appended: Box<Object>,
    fail: bool,
}

unsafe extern "C" fn visiting(value: *mut c_void) -> i32 {
    unsafe {
        let value = value.cast::<Object>();
        let context = (*value).visit_context;
        let name = i32::from_ne_bytes((*value).header.name);
        (*context).seen.push(name);
        if name == 0 {
            let appended = (&mut *(*context).appended as *mut Object).cast();
            if let Err(status) = storage::add((*value).storage, appended, -1, &SERVICES) {
                return status;
            }
            if (*context).fail {
                return 73;
            }
        }
        0
    }
}

#[test]
fn shutdown_visit_uses_live_bound_and_stops_on_first_error() {
    for fail in [false, true] {
        let mut table = ObjectStorage::new();
        let mut first = object(&mut table, 0);
        let mut hole = object(&mut table, 1);
        unsafe {
            add(&mut table, &mut first, -1);
            add(&mut table, &mut hole, -1);
            remove(&mut table, &mut hole);
            storage::reserve_replacement(&mut table, 1, &SERVICES).unwrap();
        }
        let mut context = VisitContext {
            seen: Vec::new(),
            appended: object(&mut table, 2),
            fail,
        };
        let context_ptr = &mut context as *mut VisitContext;
        first.visit_context = context_ptr;
        context.appended.visit_context = context_ptr;
        let result = unsafe { storage::visit(&mut table, visiting) };
        assert_eq!(result, if fail { Err(73) } else { Ok(()) });
        assert_eq!(context.seen, if fail { vec![0] } else { vec![0, 2] });
        assert!(table.at(1).is_null());
        assert_eq!(table.at(2), pointer(&mut context.appended));
    }
}
