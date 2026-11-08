use std::alloc::{Layout, alloc, dealloc};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::c_void;
use std::ptr;

use hp2_core::array_storage::{self as array, Hp2ArrayServices, Hp2ArraySlots};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Realloc {
        bytes: usize,
        num: i32,
        max: i32,
        data: *mut c_void,
        tag: *const c_void,
    },
    Size(usize),
    Free {
        data: *mut c_void,
        num: i32,
        max: i32,
    },
}

// Shared fixture references are safe across reentry: live fields and recording
// state use interior mutability, and no RefCell borrow crosses a callback.
struct Fixture {
    data: UnsafeCell<*mut c_void>,
    num: UnsafeCell<i32>,
    max: UnsafeCell<i32>,
    allocated: Cell<usize>,
    events: RefCell<Vec<Event>>,
    fail_realloc: Cell<bool>,
    fail_size: Cell<bool>,
    fail_free: Cell<bool>,
    shrink_on_size: Cell<bool>,
    set_on_realloc: Cell<Option<i32>>,
    append_on_size: Cell<bool>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            data: UnsafeCell::new(ptr::null_mut()),
            num: UnsafeCell::new(0),
            max: UnsafeCell::new(0),
            allocated: Cell::new(0),
            events: RefCell::new(Vec::new()),
            fail_realloc: Cell::new(false),
            fail_size: Cell::new(false),
            fail_free: Cell::new(false),
            shrink_on_size: Cell::new(false),
            set_on_realloc: Cell::new(None),
            append_on_size: Cell::new(false),
        }
    }
    fn slots(&self) -> Hp2ArraySlots {
        Hp2ArraySlots {
            data: self.data.get(),
            num: self.num.get(),
            max: self.max.get(),
        }
    }
    fn svc(&self) -> Hp2ArrayServices {
        Hp2ArrayServices {
            context: ptr::from_ref(self).cast_mut().cast(),
            realloc: reallocate,
            checked_int_size: checked_size,
            free: release,
        }
    }
    fn state(&self) -> (i32, i32) {
        unsafe { (self.num.get().read(), self.max.get().read()) }
    }
    fn data(&self) -> *mut c_void {
        unsafe { self.data.get().read() }
    }
    fn contents(&self, len: usize) -> Vec<u8> {
        if len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(self.data().cast::<u8>(), len).to_vec() }
        }
    }
    fn fill(&self, values: &[u8]) {
        unsafe {
            ptr::copy_nonoverlapping(values.as_ptr(), self.data().cast(), values.len());
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.allocated.get() != 0 {
            unsafe {
                dealloc(self.data().cast(), layout(self.allocated.get()));
            }
        }
    }
}

fn layout(bytes: usize) -> Layout {
    Layout::from_size_align(bytes, 16).unwrap()
}

unsafe extern "C" fn reallocate(
    context: *mut c_void,
    data: *mut c_void,
    bytes: usize,
    max: *const i32,
    _width: i32,
    tag: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        f.events.borrow_mut().push(Event::Realloc {
            bytes,
            num: f.num.get().read(),
            max: max.read(),
            data,
            tag,
        });
        if f.fail_realloc.get() {
            return 17;
        }
        if bytes > u32::MAX as usize {
            return 19;
        }
        if let Some(count) = f.set_on_realloc.take() {
            if let Err(code) = array::set(f.slots(), count, 1, tag, f.svc()) {
                return code;
            }
        }
        // Always move in the fixture so stale-pointer uses cannot pass by luck.
        let next = if bytes == 0 {
            ptr::null_mut()
        } else {
            alloc(layout(bytes))
        };
        if bytes != 0 && next.is_null() {
            return 23;
        }
        if f.allocated.get() != 0 {
            let len = bytes.min(f.allocated.get());
            if len != 0 {
                ptr::copy_nonoverlapping(data.cast::<u8>(), next, len);
            }
            dealloc(data.cast(), layout(f.allocated.get()));
        }
        if bytes > f.allocated.get() {
            ptr::write_bytes(next.add(f.allocated.get()), 0xa5, bytes - f.allocated.get());
        }
        f.allocated.set(bytes);
        out.write(next.cast());
        0
    }
}

unsafe extern "C" fn checked_size(context: *mut c_void, bytes: usize, out: *mut i32) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        f.events.borrow_mut().push(Event::Size(bytes));
        if f.fail_size.get() {
            return 29;
        }
        if bytes > i32::MAX as usize {
            return 31;
        }
        if f.shrink_on_size.replace(false) {
            if let Err(code) = array::shrink(f.slots(), 1, ptr::null(), f.svc()) {
                return code;
            }
        }
        if f.append_on_size.replace(false) {
            if let Err(code) = array::add(f.slots(), 1, 1, ptr::null(), f.svc()) {
                return code;
            }
        }
        out.write(bytes as i32);
        0
    }
}

unsafe extern "C" fn release(context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe {
        let f = &*context.cast::<Fixture>();
        f.events.borrow_mut().push(Event::Free {
            data,
            num: f.num.get().read(),
            max: f.max.get().read(),
        });
        if f.fail_free.get() {
            return 37;
        }
        dealloc(data.cast(), layout(f.allocated.replace(0)));
        0
    }
}

#[test]
fn native_capacity_sequence_and_preservation() {
    let f = Fixture::new();
    unsafe {
        assert_eq!(array::add(f.slots(), 3, 4, ptr::null(), f.svc()), Ok(0));
        f.fill(&[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(f.state(), (3, 36));
        array::insert_zeroed(f.slots(), 1, 2, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(
            f.contents(20),
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]
        );
        array::remove(f.slots(), 0, 1, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (4, 36));
        assert_eq!(
            array::add_zeroed(f.slots(), 70, 4, ptr::null(), f.svc()),
            Ok(4)
        );
        assert_eq!(f.state(), (74, 133));
        assert!(f.contents(296)[16..].iter().all(|v| *v == 0));
        array::remove(f.slots(), 4, 70, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (4, 4));
        let preserved = f.contents(16);
        array::set(f.slots(), 2, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (2, 4));
        array::shrink(f.slots(), 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (2, 2));
        assert_eq!(f.contents(8), preserved[..8]);
        array::set(f.slots(), 6, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (6, 6));
        array::empty(f.slots(), 10, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (0, 10));
        assert_eq!(array::add(f.slots(), 2, 4, ptr::null(), f.svc()), Ok(0));
        let events = f.events.borrow().len();
        array::remove(f.slots(), 1, 0, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.events.borrow().len(), events);
        array::empty(f.slots(), 0, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (0, 0));
        assert!(f.data().is_null());
    }
}

#[test]
fn odd_width_overlap_and_zeroing_touch_only_requested_elements() {
    let f = Fixture::new();
    unsafe {
        array::add(f.slots(), 4, 3, ptr::null(), f.svc()).unwrap();
        f.fill(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        array::insert_zeroed(f.slots(), 1, 1, 3, ptr::null(), f.svc()).unwrap();
        assert_eq!(
            f.contents(18),
            [
                1, 2, 3, 0, 0, 0, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0xa5, 0xa5, 0xa5
            ]
        );
        array::remove(f.slots(), 1, 2, 3, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.contents(9), [1, 2, 3, 7, 8, 9, 10, 11, 12]);
        array::insert(f.slots(), 3, 1, 3, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state().0, 4);
    }
}

#[test]
fn strict_shrink_ratio_slack_and_byte_thresholds() {
    for (max, before, width, after_max) in [
        (100, 37, 1, 100), // 64 slack is not enough.
        (100, 36, 1, 35),
        (300, 201, 1, 300), // Ratio equality does not shrink.
        (300, 200, 1, 199),
        (384, 257, 127, 384),
        (384, 257, 128, 256),  // Exactly 16384 unused bytes.
        (200, 138, 1024, 200), // Byte threshold still requires >64 slack.
    ] {
        let f = Fixture::new();
        unsafe {
            array::empty(f.slots(), max, width, ptr::null(), f.svc()).unwrap();
            array::set(f.slots(), before, width, ptr::null(), f.svc()).unwrap();
            array::remove(f.slots(), before - 1, 1, width, ptr::null(), f.svc()).unwrap();
        }
        assert_eq!(f.state(), (before - 1, after_max));
    }
}

#[test]
fn null_empty_and_zero_width_operations_do_not_form_invalid_ranges() {
    let f = Fixture::new();
    unsafe {
        array::insert_zeroed(f.slots(), 0, 0, 4, ptr::null(), f.svc()).unwrap();
        assert_eq!(
            array::add_zeroed(f.slots(), 0, 4, ptr::null(), f.svc()),
            Ok(0)
        );
        array::shrink(f.slots(), 4, ptr::null(), f.svc()).unwrap();
        array::release(f.slots(), f.svc()).unwrap();
        array::add_zeroed(f.slots(), 5, 0, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (5, 38));
        assert!(f.data().is_null());
        array::insert_zeroed(f.slots(), 2, 3, 0, ptr::null(), f.svc()).unwrap();
        array::remove(f.slots(), 1, 6, 0, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (2, 38));
        array::remove(f.slots(), 0, 2, 0, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (0, 0));
    }
}

#[test]
fn shrink_is_idempotent_and_final_release_frees_retained_slack() {
    let f = Fixture::new();
    unsafe {
        array::empty(f.slots(), 8, 1, ptr::null(), f.svc()).unwrap();
        let data = f.data();
        array::remove(f.slots(), 0, 0, 1, ptr::null(), f.svc()).unwrap();
        array::release(f.slots(), f.svc()).unwrap();
        assert_eq!(
            f.events.borrow().last(),
            Some(&Event::Free {
                data,
                num: 0,
                max: 8
            })
        );
        assert_eq!(f.state(), (0, 0));
        assert!(f.data().is_null());
        let events = f.events.borrow().len();
        array::release(f.slots(), f.svc()).unwrap();
        array::shrink(f.slots(), 1, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.events.borrow().len(), events);
    }
}

#[test]
fn allocation_failure_publishes_counts_not_pointer_and_stops_zeroing() {
    let f = Fixture::new();
    unsafe {
        array::set(f.slots(), 2, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[7, 9]);
        let data = f.data();
        f.events.borrow_mut().clear();
        f.fail_realloc.set(true);
        let tag = c"array-tag".as_ptr().cast();
        assert_eq!(array::add_zeroed(f.slots(), 2, 1, tag, f.svc()), Err(17));
        assert_eq!(f.state(), (4, 37));
        assert_eq!(f.data(), data);
        assert_eq!(f.contents(2), [7, 9]);
        assert_eq!(
            *f.events.borrow(),
            [Event::Realloc {
                bytes: 37,
                num: 4,
                max: 37,
                data,
                tag
            }]
        );
    }
}

#[test]
fn checked_move_failure_preserves_remove_state_but_insert_has_published_growth() {
    let f = Fixture::new();
    unsafe {
        array::add(f.slots(), 3, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[4, 5, 6]);
        f.fail_size.set(true);
        f.events.borrow_mut().clear();
        assert_eq!(
            array::remove(f.slots(), 0, 1, 1, ptr::null(), f.svc()),
            Err(29)
        );
        assert_eq!(f.state(), (3, 36));
        assert_eq!(f.contents(3), [4, 5, 6]);
        assert_eq!(
            array::insert_zeroed(f.slots(), 1, 1, 1, ptr::null(), f.svc()),
            Err(29)
        );
        assert_eq!(f.state(), (4, 36));
        assert_eq!(f.contents(3), [4, 5, 6]);
        assert_eq!(*f.events.borrow(), [Event::Size(2), Event::Size(2)]);
    }
}

#[test]
fn remove_realloc_and_empty_free_failures_keep_exact_partial_state() {
    let f = Fixture::new();
    unsafe {
        array::empty(f.slots(), 100, 1, ptr::null(), f.svc()).unwrap();
        array::set(f.slots(), 3, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[1, 2, 3]);
        f.fail_realloc.set(true);
        let data = f.data();
        assert_eq!(
            array::remove(f.slots(), 0, 1, 1, ptr::null(), f.svc()),
            Err(17)
        );
        assert_eq!(f.state(), (2, 2));
        assert_eq!(f.contents(2), [2, 3]);
        assert_eq!(array::empty(f.slots(), 0, 1, ptr::null(), f.svc()), Err(17));
        assert_eq!(f.state(), (0, 0));
        assert_eq!(f.data(), data);
        f.fail_free.set(true);
        assert_eq!(array::release(f.slots(), f.svc()), Err(37));
        assert_eq!(f.data(), data);
        f.fail_free.set(false);
        array::release(f.slots(), f.svc()).unwrap();
        assert!(f.data().is_null());
    }
}

#[test]
fn native_size_contracts_are_called_before_any_byte_access() {
    let f = Fixture::new();
    unsafe {
        assert_eq!(
            array::set(f.slots(), i32::MAX, 3, ptr::null(), f.svc()),
            Err(19)
        );
        assert_eq!(f.state(), (i32::MAX, i32::MAX));
        assert!(f.data().is_null());
        // Zero count avoids allocation but still checks the original move length.
        assert_eq!(
            array::insert(f.slots(), 0, 0, 2, ptr::null(), f.svc()),
            Err(31)
        );
        assert_eq!(
            f.events.borrow().last(),
            Some(&Event::Size(i32::MAX as usize * 2))
        );
    }
}

#[test]
fn callbacks_see_live_fields_and_reentrant_pointer_changes_are_used() {
    let f = Fixture::new();
    unsafe {
        array::add(f.slots(), 3, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[1, 2, 3]);
        let old = f.data();
        f.shrink_on_size.set(true);
        array::insert_zeroed(f.slots(), 1, 1, 1, ptr::null(), f.svc()).unwrap();
        assert_ne!(f.data(), old);
        assert_eq!(f.state(), (4, 4));
        assert_eq!(f.contents(4), [1, 0, 2, 3]);
        f.set_on_realloc.set(Some(1));
        let data = f.data();
        array::set(f.slots(), 6, 1, ptr::null(), f.svc()).unwrap();
        // The outer call must not overwrite the count changed during allocation.
        assert_eq!(f.state(), (1, 6));
        assert_eq!(
            f.events.borrow().last(),
            Some(&Event::Realloc {
                bytes: 6,
                num: 6,
                max: 6,
                data,
                tag: ptr::null(),
            })
        );
    }
}

#[test]
fn removal_decrements_live_count_after_reentrant_size_callback() {
    let f = Fixture::new();
    unsafe {
        array::add(f.slots(), 3, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[8, 9, 10]);
        f.append_on_size.set(true);
        array::remove(f.slots(), 0, 1, 1, ptr::null(), f.svc()).unwrap();
        assert_eq!(f.state(), (3, 36));
        assert_eq!(f.contents(2), [9, 10]);
    }
}

#[test]
fn exact_replacement_always_reallocates_and_preserves_prefix_and_tag() {
    let f = Fixture::new();
    let tag = c"FString".as_ptr().cast();
    unsafe {
        array::set_exact(f.slots(), 3, 2, tag, f.svc()).unwrap();
        f.fill(&[1, 2, 3, 4, 5, 6]);
        let data = f.data();
        f.events.borrow_mut().clear();
        array::set_exact(f.slots(), 3, 2, tag, f.svc()).unwrap();
        assert_ne!(f.data(), data);
        assert_eq!(f.contents(6), [1, 2, 3, 4, 5, 6]);
        assert_eq!(
            *f.events.borrow(),
            [Event::Realloc {
                bytes: 6,
                num: 3,
                max: 3,
                data,
                tag
            }]
        );
        f.fail_realloc.set(true);
        let data = f.data();
        assert_eq!(array::set_exact(f.slots(), 1, 2, tag, f.svc()), Err(17));
        assert_eq!(f.state(), (1, 1));
        assert_eq!(f.data(), data);
        assert_eq!(f.contents(6), [1, 2, 3, 4, 5, 6]);
    }
}

#[test]
fn zero_failure_occurs_after_successful_growth() {
    let f = Fixture::new();
    unsafe {
        array::set(f.slots(), 3, 1, ptr::null(), f.svc()).unwrap();
        f.fill(&[1, 2, 3]);
        f.fail_size.set(true);
        let data = f.data();
        assert_eq!(
            array::add_zeroed(f.slots(), 1, 1, ptr::null(), f.svc()),
            Err(29)
        );
        assert_eq!(f.state(), (4, 37));
        assert_ne!(f.data(), data);
        assert_eq!(f.contents(4), [1, 2, 3, 0xa5]);
    }
}

#[test]
fn shrinking_failure_publishes_exact_capacity_without_replacing_data() {
    let f = Fixture::new();
    unsafe {
        array::add(f.slots(), 2, 1, ptr::null(), f.svc()).unwrap();
        let data = f.data();
        f.fail_realloc.set(true);
        f.events.borrow_mut().clear();
        assert_eq!(array::shrink(f.slots(), 1, ptr::null(), f.svc()), Err(17));
        assert_eq!(f.state(), (2, 2));
        assert_eq!(f.data(), data);
        assert_eq!(
            *f.events.borrow(),
            [Event::Realloc {
                bytes: 2,
                num: 2,
                max: 2,
                data,
                tag: ptr::null(),
            }]
        );
    }
}

#[test]
fn four_byte_packed_native_pointer_slot_is_not_assumed_pointer_aligned() {
    #[repr(C, packed(4))]
    struct NativeFields {
        prefix: u32,
        data: *mut c_void,
        num: i32,
        max: i32,
    }
    #[repr(C, align(8))]
    struct Aligned(NativeFields);
    let mut storage = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut native = Aligned(NativeFields {
        prefix: 0,
        data: storage.as_mut_ptr().cast(),
        num: 3,
        max: 8,
    });
    let slots = Hp2ArraySlots {
        data: ptr::addr_of_mut!(native.0.data),
        num: ptr::addr_of_mut!(native.0.num),
        max: ptr::addr_of_mut!(native.0.max),
    };
    assert_eq!(slots.data as usize % 8, 4);
    let f = Fixture::new();
    unsafe {
        array::insert_zeroed(slots, 1, 1, 1, ptr::null(), f.svc()).unwrap();
        assert_eq!(&storage[..4], &[1, 0, 2, 3]);
        array::remove(slots, 0, 1, 1, ptr::null(), f.svc()).unwrap();
        assert_eq!(&storage[..3], &[0, 2, 3]);
        assert_eq!(slots.num.read(), 3);
    }
}
