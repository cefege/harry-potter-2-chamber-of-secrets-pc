use hp2_core::array_storage::Hp2ArrayServices;
use hp2_core::map_storage::{
    self as map, Hp2MapServices, Hp2MapSlots, leaf, operation as op, query,
};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::{ffi::c_void, ptr};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pair {
    key: i32,
    value: i32,
    next: i32,
}

#[repr(C, packed(4))]
struct Packed {
    padding: i32,
    hash: *mut i32,
    count: i32,
}
#[repr(C, align(8))]
struct Aligned {
    fields: Packed,
}

struct Fixture {
    fields: UnsafeCell<Aligned>,
    pairs: RefCell<Vec<Pair>>,
    allocations: RefCell<Vec<Box<[i32]>>>,
    key: Cell<i32>,
    value: Cell<i32>,
    collision: Cell<bool>,
    fail: Cell<Option<(bool, i32, i32)>>,
    fail_alloc: Cell<bool>,
    fail_free: Cell<bool>,
    events: RefCell<Vec<(bool, i32, i32)>>,
    removed: RefCell<Vec<i32>>,
    values: RefCell<Vec<i32>>,
    diagnostics: RefCell<Vec<(i32, i32)>>,
    rehash_on_hash: Cell<bool>,
    append_on_emit: Cell<bool>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            fields: UnsafeCell::new(Aligned {
                fields: Packed {
                    padding: 0,
                    hash: ptr::null_mut(),
                    count: 8,
                },
            }),
            pairs: RefCell::new(Vec::new()),
            allocations: RefCell::new(Vec::new()),
            key: Cell::new(0),
            value: Cell::new(0),
            collision: Cell::new(false),
            fail: Cell::new(None),
            fail_alloc: Cell::new(false),
            fail_free: Cell::new(false),
            events: RefCell::new(Vec::new()),
            removed: RefCell::new(Vec::new()),
            values: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
            rehash_on_hash: Cell::new(false),
            append_on_emit: Cell::new(false),
        }
    }
    fn slots(&self) -> Hp2MapSlots {
        unsafe {
            Hp2MapSlots {
                hash: ptr::addr_of_mut!((*self.fields.get()).fields.hash),
                count: ptr::addr_of_mut!((*self.fields.get()).fields.count),
            }
        }
    }
    fn services(&self) -> Hp2MapServices {
        let context = ptr::from_ref(self).cast_mut().cast();
        Hp2MapServices {
            context,
            leaf: primitive,
            query: request,
            allocator: Hp2ArrayServices {
                context,
                realloc: allocate,
                checked_int_size: checked,
                free,
            },
            tag: ptr::null(),
        }
    }
    fn run(&self, op: i32) -> Result<i32, i32> {
        unsafe { map::run(self.slots(), self.services(), op) }
    }
    fn init(&self) {
        assert_eq!(self.run(op::REHASH), Ok(0));
    }
    fn input(&self, key: i32, value: i32) {
        self.key.set(key);
        self.value.set(value);
    }
    fn add(&self, key: i32, value: i32) -> i32 {
        self.input(key, value);
        self.run(op::ADD).unwrap()
    }
    fn count(&self) -> i32 {
        unsafe { self.slots().count.read_unaligned() }
    }
    fn hash(&self) -> *mut i32 {
        unsafe { self.slots().hash.read_unaligned() }
    }
    fn bucket(&self, i: usize) -> i32 {
        unsafe { self.hash().add(i).read_unaligned() }
    }
    fn snapshot(&self) -> Vec<(i32, i32)> {
        self.pairs
            .borrow()
            .iter()
            .map(|p| (p.key, p.value))
            .collect()
    }
    fn hash_key(&self, key: i32) -> i32 {
        if self.collision.get() { 0 } else { key }
    }
    fn event(&self, query: bool, op: i32, index: i32) -> bool {
        self.events.borrow_mut().push((query, op, index));
        self.fail.get() == Some((query, op, index))
    }
}

unsafe extern "C" fn allocate(
    context: *mut c_void,
    data: *mut c_void,
    bytes: usize,
    _max: *const i32,
    _width: i32,
    _tag: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        f.events.borrow_mut().push((false, 100, bytes as i32));
        if f.fail_alloc.get() {
            return 71;
        }
        if !data.is_null() {
            return 72;
        }
        let mut allocation = vec![0; bytes / 4].into_boxed_slice();
        out.write(allocation.as_mut_ptr().cast());
        f.allocations.borrow_mut().push(allocation);
        0
    }
}
unsafe extern "C" fn checked(_: *mut c_void, bytes: usize, out: *mut i32) -> i32 {
    match i32::try_from(bytes) {
        Ok(n) => {
            unsafe {
                out.write(n);
            }
            0
        }
        Err(_) => 73,
    }
}
unsafe extern "C" fn free(context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        f.events.borrow_mut().push((false, 101, f.count()));
        if f.fail_free.get() {
            return 74;
        }
        let mut allocations = f.allocations.borrow_mut();
        if let Some(index) = allocations
            .iter()
            .position(|a| a.as_ptr().cast::<c_void>() == data)
        {
            allocations.remove(index);
            0
        } else {
            75
        }
    }
}
unsafe extern "C" fn primitive(
    context: *mut c_void,
    op: i32,
    i: i32,
    arg: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        if f.event(false, op, i) {
            return 81;
        }
        let result = match op {
            leaf::NUM => f.pairs.borrow().len() as i32,
            leaf::NEXT => f.pairs.borrow()[i as usize].next,
            leaf::LINK => {
                f.pairs.borrow_mut()[i as usize].next = arg;
                0
            }
            leaf::HASH => f.hash_key(f.pairs.borrow()[i as usize].key),
            leaf::REMOVE => {
                f.removed.borrow_mut().push(i);
                f.pairs.borrow_mut().remove(i as usize);
                0
            }
            leaf::EMPTY => {
                f.pairs.borrow_mut().clear();
                0
            }
            _ => return 82,
        };
        out.write(result);
        0
    }
}
unsafe extern "C" fn request(
    context: *mut c_void,
    op: i32,
    i: i32,
    arg: i32,
    out: *mut i32,
) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        if f.event(true, op, i) {
            return 83;
        }
        let result = match op {
            query::HASH => {
                if f.rehash_on_hash.replace(false) {
                    if let Err(code) = f.run(op::REHASH) {
                        return code;
                    }
                }
                f.hash_key(f.key.get())
            }
            query::KEY_EQ => i32::from(f.pairs.borrow()[i as usize].key == f.key.get()),
            query::VALUE_EQ => i32::from(f.pairs.borrow()[i as usize].value == f.value.get()),
            query::CONSTRUCT => {
                let mut pairs = f.pairs.borrow_mut();
                let index = pairs.len() as i32;
                pairs.push(Pair {
                    key: f.key.get(),
                    value: f.value.get(),
                    next: -99,
                });
                index
            }
            query::ASSIGN => {
                f.pairs.borrow_mut()[i as usize].value = f.value.get();
                0
            }
            query::EMIT => {
                let value = f.pairs.borrow()[i as usize].value;
                f.values.borrow_mut().push(value);
                if f.append_on_emit.replace(false) {
                    let saved = (f.key.get(), f.value.get());
                    f.value.set(999);
                    let result = f.run(op::ADD);
                    f.input(saved.0, saved.1);
                    if let Err(code) = result {
                        return code;
                    }
                }
                0
            }
            query::SUMMARY | query::BUCKET => {
                f.diagnostics.borrow_mut().push((i, arg));
                0
            }
            _ => return 84,
        };
        out.write(result);
        0
    }
}

#[test]
fn collision_growth_thresholds_and_reverse_chains() {
    let f = Fixture::new();
    f.collision.set(true);
    f.init();
    for i in 0..100 {
        assert_eq!(f.add(i, i * 10), i);
        let expected = match i + 1 {
            0..=24 => 8,
            25..=40 => 16,
            41..=72 => 32,
            _ => 64,
        };
        assert_eq!(f.count(), expected);
        assert_eq!(f.bucket(0), i);
        assert_eq!(f.pairs.borrow()[i as usize].next, i - 1);
        assert_eq!(f.allocations.borrow().len(), 1);
    }
    f.input(42, 0);
    assert_eq!(f.run(op::FIND), Ok(42));
    f.input(999, 0);
    assert_eq!(f.run(op::FIND), Ok(-1));
    assert_eq!(f.run(op::DUMP), Ok(0));
    assert_eq!(f.diagnostics.borrow()[0], (100, 64));
    assert_eq!(f.diagnostics.borrow()[1], (0, 100));
    assert!(f.diagnostics.borrow()[2..].iter().all(|&(_, n)| n == 0));
}

#[test]
fn set_duplicate_pair_uniqueness_and_multifind_append_order() {
    let f = Fixture::new();
    f.init();
    f.add(8, 1);
    f.add(8, 2);
    f.add(16, 3);
    f.add(8, 1);
    f.values.borrow_mut().push(77);
    f.input(8, 0);
    assert_eq!(f.run(op::MULTI_FIND), Ok(0));
    assert_eq!(*f.values.borrow(), [77, 1, 2, 1]);
    f.input(8, 1);
    assert_eq!(f.run(op::UNIQUE), Ok(3));
    assert_eq!(f.pairs.borrow().len(), 4);
    f.input(8, 4);
    assert_eq!(f.run(op::SET), Ok(3));
    assert_eq!(f.snapshot(), [(8, 1), (8, 2), (16, 3), (8, 4)]);
    f.input(8, 2);
    assert_eq!(f.run(op::FIND_PAIR), Ok(1));
    f.input(8, 9);
    assert_eq!(f.run(op::FIND_PAIR), Ok(-1));
    assert_eq!(f.run(op::UNIQUE), Ok(4));
    f.input(7, 6);
    assert_eq!(f.run(op::SET), Ok(5));
    assert_eq!(f.snapshot()[5], (7, 6));
}

#[test]
fn reverse_removal_preserves_survivors_duplicates_and_relax_threshold() {
    let f = Fixture::new();
    f.init();
    for i in 0..73 {
        f.add(i % 2, i % 3);
    }
    assert_eq!(f.count(), 64);
    f.input(0, 1);
    let before = f.snapshot();
    let expected: Vec<_> = before.iter().copied().filter(|&p| p != (0, 1)).collect();
    assert_eq!(f.run(op::REMOVE_PAIR), Ok(12));
    assert_eq!(f.snapshot(), expected);
    assert!(f.removed.borrow().windows(2).all(|w| w[0] > w[1]));
    f.input(0, 0);
    assert_eq!(f.run(op::REMOVE), Ok(25));
    assert_eq!(f.count(), 64); // 64 <= 36*2+8.
    f.input(1, 0);
    assert_eq!(f.run(op::REMOVE), Ok(36));
    assert_eq!(f.count(), 8);
    assert!(f.snapshot().is_empty());
    let old = f.hash();
    assert_eq!(f.run(op::REMOVE), Ok(0));
    assert_eq!(f.hash(), old); // No matches do not rehash.
}

#[test]
fn relax_equality_boundary_and_archive_rehash_do_not_autogrow() {
    let f = Fixture::new();
    f.init();
    for i in 0..50 {
        f.pairs.borrow_mut().push(Pair {
            key: i,
            value: i,
            next: 0,
        });
    }
    assert_eq!(f.run(op::REHASH), Ok(0));
    assert_eq!(f.count(), 8); // Loading into a default map only rebuilds chains.
    unsafe {
        f.slots().count.write_unaligned(32);
    }
    f.pairs.borrow_mut().truncate(12);
    assert_eq!(f.run(op::RELAX), Ok(0));
    assert_eq!(f.count(), 32); // 32 == 12*2+8, strict inequality.
    f.pairs.borrow_mut().truncate(11);
    assert_eq!(f.run(op::RELAX), Ok(0));
    assert_eq!(f.count(), 16);
    assert_eq!(f.run(op::EMPTY), Ok(0));
    assert_eq!(f.count(), 8);
    assert!((0..8).all(|i| f.bucket(i) == -1));
    assert_eq!(f.run(op::RELEASE), Ok(0));
    assert!(f.hash().is_null());
    assert_eq!(f.count(), 0);
    assert!(f.allocations.borrow().is_empty());
}

#[test]
fn allocation_failure_keeps_published_growth_count_and_inserted_pair() {
    let f = Fixture::new();
    f.init();
    for i in 0..24 {
        f.add(i, i);
    }
    let old = f.hash();
    f.fail_alloc.set(true);
    f.input(24, 24);
    assert_eq!(f.run(op::ADD), Err(71));
    assert_eq!(f.count(), 16);
    assert_eq!(f.hash(), old);
    assert_eq!(f.pairs.borrow().len(), 25);
    assert_eq!(f.bucket(0), 24);
    assert_eq!(f.events.borrow().last(), Some(&(false, 100, 64)));
}

#[test]
fn hash_failure_retains_partial_links_and_stops_without_free_or_publication() {
    let f = Fixture::new();
    f.init();
    f.add(0, 0);
    f.add(8, 1);
    f.add(16, 2);
    let old = f.hash();
    unsafe {
        f.slots().count.write_unaligned(16);
    }
    f.fail.set(Some((false, leaf::HASH, 2)));
    assert_eq!(f.run(op::REHASH), Err(81));
    assert_eq!(f.hash(), old);
    assert_eq!(f.pairs.borrow()[1].next, -1); // Rewritten into bucket 8 before failure.
    assert_eq!(f.allocations.borrow().len(), 2); // Native throw left unpublished table.
    assert_eq!(f.events.borrow().last(), Some(&(false, leaf::HASH, 2)));
}

#[test]
fn free_failure_does_not_publish_new_hash_or_clear_release_slots() {
    let f = Fixture::new();
    f.init();
    f.add(1, 7);
    let old = f.hash();
    f.fail_free.set(true);
    assert_eq!(f.run(op::REHASH), Err(74));
    assert_eq!(f.hash(), old);
    assert_eq!(f.allocations.borrow().len(), 2);
    assert_eq!(f.run(op::RELEASE), Err(74));
    assert_eq!(f.hash(), old);
    assert_eq!(f.count(), 8);
}

#[test]
fn query_errors_stop_before_assignment_and_remove_rehash() {
    let f = Fixture::new();
    f.init();
    f.add(1, 10);
    f.add(1, 20);
    f.add(1, 30);
    f.input(1, 90);
    f.fail.set(Some((true, query::ASSIGN, 2)));
    assert_eq!(f.run(op::SET), Err(83));
    assert_eq!(f.snapshot(), [(1, 10), (1, 20), (1, 30)]);
    assert_eq!(f.events.borrow().last(), Some(&(true, query::ASSIGN, 2)));
    f.fail.set(Some((true, query::KEY_EQ, 1)));
    let old = f.hash();
    assert_eq!(f.run(op::REMOVE), Err(83));
    assert_eq!(f.snapshot(), [(1, 10), (1, 20)]);
    assert_eq!(f.hash(), old); // Removal's Relax is never reached on a throw.
    assert_eq!(f.events.borrow().last(), Some(&(true, query::KEY_EQ, 1)));
}

#[test]
fn callback_reentry_reloads_hash_and_pair_storage_without_extending_walk() {
    let f = Fixture::new();
    f.init();
    for i in 0..24 {
        f.add(7, i);
    }
    f.input(7, 0);
    f.rehash_on_hash.set(true);
    assert_eq!(f.run(op::FIND), Ok(23));
    f.append_on_emit.set(true);
    assert_eq!(f.run(op::MULTI_FIND), Ok(0));
    assert_eq!(f.count(), 16); // Nested append crossed the growth threshold.
    assert_eq!(*f.values.borrow(), (0..24).rev().collect::<Vec<_>>());
    assert_eq!(f.snapshot().last(), Some(&(7, 999)));
    assert_eq!(f.allocations.borrow().len(), 1);
}

#[test]
fn packed_pointer_slot_and_short_circuit_pair_comparison() {
    let f = Fixture::new();
    f.init();
    if std::mem::align_of::<*mut i32>() == 8 {
        assert_eq!(f.slots().hash as usize % 8, 4);
    }
    f.collision.set(true);
    f.add(1, 4);
    f.add(2, 5);
    f.input(1, 4);
    f.fail.set(Some((true, query::VALUE_EQ, 1)));
    assert_eq!(f.run(op::FIND_PAIR), Ok(0)); // Wrong key must skip value comparison.
}

#[test]
fn construction_hash_and_output_failures_stop_at_the_first_primitive() {
    let f = Fixture::new();
    f.init();
    f.add(3, 30);
    let old = f.hash();
    f.input(3, 40);
    f.fail.set(Some((true, query::CONSTRUCT, 0)));
    assert_eq!(f.run(op::ADD), Err(83));
    assert_eq!(f.snapshot(), [(3, 30)]);
    assert_eq!(f.hash(), old);
    assert_eq!(f.events.borrow().last(), Some(&(true, query::CONSTRUCT, 0)));
    f.fail.set(Some((true, query::HASH, 0)));
    assert_eq!(f.run(op::FIND), Err(83));
    assert_eq!(f.events.borrow().last(), Some(&(true, query::HASH, 0)));
    f.fail.set(None);
    f.add(3, 40);
    f.fail.set(Some((true, query::EMIT, 0)));
    assert_eq!(f.run(op::MULTI_FIND), Err(83));
    assert_eq!(*f.values.borrow(), [40]);
    assert_eq!(f.events.borrow().last(), Some(&(true, query::EMIT, 0)));
    f.fail.set(Some((true, query::BUCKET, 2)));
    assert_eq!(f.run(op::DUMP), Err(83));
    assert_eq!(*f.diagnostics.borrow(), [(2, 8), (0, 0), (1, 0)]);
    assert_eq!(f.events.borrow().last(), Some(&(true, query::BUCKET, 2)));
}

#[test]
fn hash_failure_after_construction_keeps_the_pair_unlinked() {
    let f = Fixture::new();
    f.init();
    f.add(3, 30);
    let old = f.hash();
    f.input(3, 40);
    f.fail.set(Some((false, leaf::HASH, 1)));
    assert_eq!(f.run(op::ADD), Err(81));
    assert_eq!(f.snapshot(), [(3, 30), (3, 40)]);
    assert_eq!(f.pairs.borrow()[1].next, -99);
    assert_eq!(f.bucket(3), 0);
    assert_eq!(f.hash(), old);
    assert_eq!(f.events.borrow().last(), Some(&(false, leaf::HASH, 1)));
}
