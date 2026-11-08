//! Boundary and lifetime contracts for the native-layout memory stack port.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_void};
use std::mem::{MaybeUninit, size_of, zeroed};
use std::ptr::null_mut;
use std::sync::Mutex;

use hp2_abi::engine::generated::{FMemStack, FTaggedMemory};
use hp2_abi::render_span::FMemMark;
use hp2_core::ansi::Tchar;
use hp2_core::mem::{self, FRAME_ALLOCATE_UNDER_PUSH, FRAME_OWN, MemServices};

// The engine's unused-chunk list is process-wide and intentionally not locked.
static STACK_TEST: Mutex<()> = Mutex::new(());
const FILE: &CStr = c"UnMem.h";

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

#[derive(Default)]
struct Allocator {
    fail: Cell<bool>,
    requests: RefCell<Vec<u32>>,
    live: RefCell<Vec<*mut c_void>>,
    errors: Cell<usize>,
    assertions: Cell<usize>,
}

impl MemServices for Allocator {
    unsafe fn alloc(&self, size: u32, _: *const c_void) -> Result<*mut c_void, i32> {
        self.requests.borrow_mut().push(size);
        if self.fail.get() {
            return Err(1);
        }
        // Real libc backing storage, as in mem_replay's recording allocator.
        let ptr = unsafe { malloc(size as usize) };
        assert!(!ptr.is_null());
        self.live.borrow_mut().push(ptr);
        Ok(ptr)
    }

    unsafe fn release(&self, ptr: *mut c_void) {
        let mut live = self.live.borrow_mut();
        let index = live.iter().position(|&entry| entry == ptr).unwrap();
        live.swap_remove(index);
        unsafe { free(ptr) };
    }

    fn raise_error(&self, _: &[Tchar]) -> i32 {
        self.errors.set(self.errors.get() + 1);
        1
    }

    fn fail_assert(&self, _: &CStr, _: &CStr, _: i32) -> i32 {
        self.assertions.set(self.assertions.get() + 1);
        1
    }
}

struct Stack {
    // Keep the native object stationary while marks hold its address.
    native: Box<FMemStack>,
    allocator: Allocator,
}

impl Stack {
    fn new(default_chunk_size: i32) -> Self {
        // The native global/zeroed construction convention initializes pointers.
        let mut native = Box::new(unsafe { zeroed::<FMemStack>() });
        mem::init(&mut native, default_chunk_size, std::ptr::null());
        Self {
            native,
            allocator: Allocator::default(),
        }
    }

    fn push(&mut self, size: i32, alignment: i32) -> Result<*mut u8, i32> {
        unsafe {
            mem::push_bytes(
                &mut self.native,
                size,
                alignment,
                true,
                FILE,
                &self.allocator,
            )
        }
    }

    fn mark(&mut self) -> FMemMark {
        let mut mark = MaybeUninit::uninit();
        unsafe {
            mem::mark_init(mark.as_mut_ptr(), &mut *self.native);
            mark.assume_init()
        }
    }

    fn pointers(&self) -> [usize; 3] {
        [
            self.native.top as usize,
            self.native.end as usize,
            self.native.top_chunk as usize,
        ]
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            mem::free_chunks(&mut self.native, null_mut());
            mem::exit(&self.native, &self.allocator, FILE.as_ptr()).unwrap();
        }
        assert!(self.allocator.live.borrow().is_empty());
    }
}

#[test]
fn empty_zero_alignment_and_exact_fit_do_not_waste_a_chunk() {
    let _serial = STACK_TEST.lock().unwrap();
    let mut stack = Stack::new(96);
    let empty = stack.mark();
    assert!(empty.top.is_null());
    assert!(empty.saved_chunk.is_null());

    let zero = stack.push(0, 16).unwrap();
    assert!(!zero.is_null());
    assert_eq!(zero as usize % 16, 0);
    assert_eq!(stack.native.top, zero);
    assert_eq!(stack.allocator.requests.borrow().len(), 1);

    let first = stack.push(3, 1).unwrap();
    assert_eq!(first, zero);
    unsafe { first.write_bytes(0x51, 3) };
    let prior_top = stack.native.top;
    let aligned = stack.push(5, 16).unwrap();
    assert_eq!(aligned as usize, (prior_top as usize + 15) & !15);
    assert_eq!(stack.native.top, aligned.wrapping_add(5));
    unsafe { aligned.write_bytes(0xa2, 5) };
    let remaining = (stack.native.end as usize - stack.native.top as usize) as i32;
    let old_chunk = stack.native.top_chunk;
    let old_top = stack.native.top;
    assert_eq!(stack.push(remaining, 1).unwrap(), old_top);
    assert_eq!(stack.native.top, stack.native.end);
    assert_eq!(stack.native.top_chunk, old_chunk);
    let end = stack.native.end;
    assert_eq!(stack.push(0, 1).unwrap(), end);
    assert_eq!(stack.native.top, end);
    assert_eq!(stack.allocator.requests.borrow().len(), 1);
    assert_eq!(unsafe { std::slice::from_raw_parts(first, 3) }, &[0x51; 3]);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(aligned, 5) },
        &[0xa2; 5]
    );

    stack.push(1, 1).unwrap();
    assert_ne!(stack.native.top_chunk, old_chunk);
    assert_eq!(stack.allocator.requests.borrow().len(), 2);
    unsafe { mem::mark_pop(&empty) };
    assert_eq!(stack.pointers(), [0; 3]);
}

#[test]
fn nested_marks_restore_tops_and_reuse_first_fitting_chunk() {
    let _serial = STACK_TEST.lock().unwrap();
    let mut stack = Stack::new(64);
    let root = stack.mark();
    let retained = stack.push(8, 8).unwrap();
    unsafe { retained.write_bytes(0x6d, 8) };
    let outer = stack.mark();
    let outer_end = stack.native.end;
    stack.push(80, 8).unwrap();
    let small_chunk = stack.native.top_chunk;
    let middle = stack.mark();
    let middle_end = stack.native.end;
    stack.push(200, 8).unwrap();
    let large_chunk = stack.native.top_chunk;
    let inner = stack.mark();
    let header = size_of::<FTaggedMemory>() as u32;
    assert_eq!(
        &*stack.allocator.requests.borrow(),
        &[64, 80 + 8 + header, 200 + 8 + header]
    );

    // A same-chunk Pop must not recompute End, even if it was narrowed by a caller.
    stack.native.end = stack.native.top;
    unsafe { mem::mark_pop(&inner) };
    assert_eq!(stack.native.end, inner.top);
    assert_eq!(stack.native.top, inner.top);

    unsafe { mem::mark_pop(&middle) };
    assert_eq!(stack.native.top, middle.top);
    assert_eq!(stack.native.end, middle_end);
    assert_eq!(stack.native.top_chunk, small_chunk);
    unsafe { mem::mark_pop(&outer) };
    assert_eq!(stack.native.top, outer.top);
    assert_eq!(stack.native.end, outer_end);
    assert_eq!(stack.native.top_chunk.cast::<c_void>(), outer.saved_chunk);

    // Unused order is now small -> large: skip the too-small first entry.
    stack.push(120, 8).unwrap();
    assert_eq!(stack.native.top_chunk, large_chunk);
    assert_eq!(stack.allocator.requests.borrow().len(), 3);
    unsafe { mem::mark_pop(&outer) };
    // Order is now large -> small: choose large, not the tighter fitting small.
    stack.push(80, 8).unwrap();
    assert_eq!(stack.native.top_chunk, large_chunk);
    assert_eq!(stack.allocator.requests.borrow().len(), 3);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(retained, 8) },
        &[0x6d; 8]
    );
    unsafe { mem::mark_pop(&root) };
    assert_eq!(stack.pointers(), [0; 3]);

    // Reinitialization replaces all fields of an existing mark as well.
    let mut recaptured = outer;
    unsafe { mem::mark_init(&mut recaptured, &mut *stack.native) };
    assert_eq!(
        recaptured.mem,
        (&mut *stack.native as *mut FMemStack).cast()
    );
    assert!(recaptured.top.is_null());
    assert!(recaptured.saved_chunk.is_null());
}

#[test]
fn allocation_and_size_errors_leave_stack_and_marks_unpublished() {
    let _serial = STACK_TEST.lock().unwrap();
    let mut stack = Stack::new(64);
    let root = stack.mark();
    stack.allocator.fail.set(true);
    assert_eq!(stack.push(1, 8), Err(FRAME_ALLOCATE_UNDER_PUSH));
    assert_eq!(stack.pointers(), [0; 3]);
    stack.allocator.fail.set(false);
    let data = stack.push(8, 8).unwrap();
    unsafe { data.write_bytes(0x73, 8) };
    let mark = stack.mark();
    let before = stack.pointers();
    stack.allocator.fail.set(true);
    assert_eq!(stack.push(100, 8), Err(FRAME_ALLOCATE_UNDER_PUSH));
    assert_eq!(stack.pointers(), before);
    assert_eq!(mark.top, stack.native.top);
    assert_eq!(mark.saved_chunk, stack.native.top_chunk.cast());
    let requests = stack.allocator.requests.borrow().len();
    assert_eq!(stack.push(i32::MAX, 8), Err(FRAME_OWN));
    assert_eq!(stack.pointers(), before);
    assert_eq!(stack.allocator.requests.borrow().len(), requests);
    assert_eq!(stack.allocator.errors.get(), 1);
    assert_eq!(unsafe { std::slice::from_raw_parts(data, 8) }, &[0x73; 8]);
    unsafe { mem::mark_pop(&root) };
    stack.allocator.fail.set(false);
    // Failed growth never displaced the original chunk, which is reusable.
    stack.push(8, 8).unwrap();
    assert_eq!(stack.native.top_chunk as usize, before[2]);
    assert_eq!(stack.allocator.requests.borrow().len(), requests);
}

#[test]
fn slow_checks_reject_invalid_stacks_before_growth() {
    let _serial = STACK_TEST.lock().unwrap();
    let mut stack = Stack::new(64);
    assert_eq!(stack.push(-1, 0), Err(FRAME_OWN));
    assert_eq!(stack.push(1, 3), Err(FRAME_OWN));
    assert_eq!(stack.pointers(), [0; 3]);
    assert!(stack.allocator.requests.borrow().is_empty());
    assert_eq!(stack.allocator.assertions.get(), 2);
    stack.allocator.assertions.set(0);
    stack.push(8, 8).unwrap();
    let original_end = stack.native.end;
    let original_top = stack.native.top;
    stack.native.end = null_mut();
    assert_eq!(stack.push(0, 1), Err(FRAME_OWN));
    assert_eq!(stack.native.top, original_top);
    stack.native.end = original_top.wrapping_sub(1);
    assert_eq!(stack.push(0, 1), Err(FRAME_OWN));
    assert_eq!(stack.allocator.assertions.get(), 2);
    stack.native.end = original_end;
    stack.allocator.assertions.set(0);
    let result =
        unsafe { mem::push_bytes(&mut stack.native, 0, 1, false, FILE, &stack.allocator) }.unwrap();
    assert_eq!(result, original_top);
    assert_eq!(stack.allocator.assertions.get(), 0);
}

#[test]
fn canonical_scratch_owners_never_share_cached_chunks_or_teardown() {
    use hp2_core::mem_owner::MemStackOwner;

    let left = Box::new(MemStackOwner::new(128, &[76, 101, 102, 116, 0]));
    let right = Box::new(MemStackOwner::new(128, &[82, 105, 103, 104, 116, 0]));
    let left_mark = unsafe { left.mark() };
    let right_mark = unsafe { right.mark() };
    let left_bytes = unsafe { left.push_bytes(4096, 64) }.unwrap();
    let right_bytes = unsafe { right.push_bytes(32, 16) }.unwrap();
    unsafe {
        left_bytes.write_bytes(0xa7, 4096);
        right_bytes.write_bytes(0x3c, 32);
        left.pop(&left_mark);
    }
    let right_growth = unsafe { right.push_bytes(4096, 64) }.unwrap();
    // Left still owns its cached chunk: right must allocate in its own domain.
    assert_ne!(right_growth, left_bytes);
    left.exit().unwrap();
    assert_eq!(unsafe { std::slice::from_raw_parts(right_bytes, 32) }, &[0x3c; 32]);
    unsafe { right.pop(&right_mark) };
    right.tick().unwrap();
    right.exit().unwrap();
}
