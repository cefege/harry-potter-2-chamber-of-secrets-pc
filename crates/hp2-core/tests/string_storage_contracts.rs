use hp2_core::ansi::Tchar;
use hp2_core::array_storage::{self as array, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::string_storage::{self as string, Hp2FStringStorageServices};
use std::alloc::{Layout, alloc, dealloc};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ptr;

#[derive(Default)]
struct Env {
    allocations: RefCell<BTreeMap<usize, usize>>,
    events: RefCell<Vec<&'static str>>,
    fail_alloc: Cell<bool>,
    fail_size: Cell<bool>,
    fail_case_at: Cell<Option<u32>>,
    fail_range: Cell<bool>,
    reenter_size: Cell<*const Node>,
    tag: Cell<*const c_void>,
}
struct Node {
    data: UnsafeCell<*mut c_void>,
    num: UnsafeCell<i32>,
    max: UnsafeCell<i32>,
    env: *const Env,
}
impl Node {
    fn new(env: &Env) -> Self {
        Self {
            data: UnsafeCell::new(ptr::null_mut()),
            num: UnsafeCell::new(0),
            max: UnsafeCell::new(0),
            env,
        }
    }
    fn slots(&self) -> Hp2ArraySlots {
        Hp2ArraySlots {
            data: self.data.get(),
            num: self.num.get(),
            max: self.max.get(),
        }
    }
    fn svc(&self) -> Hp2FStringStorageServices {
        Hp2FStringStorageServices {
            context: self.env.cast_mut().cast(),
            arrays: Hp2ArrayServices {
                context: self.env.cast_mut().cast(),
                realloc: reallocate,
                checked_int_size: checked,
                free: free_buffer,
            },
            string_tag: self.env.cast(),
            array_tag: ptr::null(),
            case_char: case,
            bounds_error: bounds,
            set_tag,
            slots: object_slots,
            assign_range,
        }
    }
    fn set(&self, value: &str) {
        let chars = wide(value);
        unsafe {
            string::assign(self.slots(), None, chars.as_ptr(), false, self.svc()).unwrap();
        }
    }
    fn raw(&self) -> Vec<Tchar> {
        unsafe {
            if *self.num.get() == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts((*self.data.get()).cast(), *self.num.get() as usize)
                    .to_vec()
            }
        }
    }
    fn text(&self) -> String {
        self.raw()
            .into_iter()
            .take_while(|c| *c != 0)
            .map(|c| char::from_u32(c as u32).unwrap())
            .collect()
    }
    fn pointer(&self) -> *mut c_void {
        ptr::from_ref(self).cast_mut().cast()
    }
    fn value(&self, op: i32, a: i32, b: i32) -> Node {
        let result = Node::new(unsafe { &*self.env });
        unsafe {
            string::value(result.slots(), self.slots(), op, a, b, self.svc()).unwrap();
        }
        result
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        unsafe {
            array::release(self.slots(), self.svc().arrays).unwrap();
        }
    }
}
fn wide(s: &str) -> Vec<Tchar> {
    s.chars().map(|c| c as Tchar).chain([0]).collect()
}
fn layout(n: usize) -> Layout {
    Layout::from_size_align(n, 16).unwrap()
}
unsafe extern "C" fn reallocate(
    context: *mut c_void,
    old: *mut c_void,
    bytes: usize,
    _: *const i32,
    _: i32,
    _: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let env = &*context.cast::<Env>();
        env.events.borrow_mut().push("allocate");
        if env.fail_alloc.get() {
            return 17;
        }
        let next = if bytes == 0 {
            ptr::null_mut()
        } else {
            alloc(layout(bytes))
        };
        if !old.is_null() {
            let previous = env
                .allocations
                .borrow_mut()
                .remove(&(old as usize))
                .unwrap();
            if bytes != 0 {
                ptr::copy_nonoverlapping(old.cast::<u8>(), next, bytes.min(previous));
            }
            dealloc(old.cast(), layout(previous));
        }
        if bytes != 0 {
            env.allocations.borrow_mut().insert(next as usize, bytes);
        }
        out.write(next.cast());
        0
    }
}
unsafe extern "C" fn checked(context: *mut c_void, bytes: usize, out: *mut i32) -> i32 {
    unsafe {
        let env = &*context.cast::<Env>();
        env.events.borrow_mut().push("size");
        if env.fail_size.get() {
            return 29;
        }
        let node = env.reenter_size.replace(ptr::null());
        if !node.is_null() {
            // Move the source's allocation, while preserving its counted value.
            array::realloc((*node).slots(), 4, ptr::null(), (*node).svc().arrays).unwrap();
        }
        out.write(bytes as i32);
        0
    }
}
unsafe extern "C" fn free_buffer(context: *mut c_void, buffer: *mut c_void) -> i32 {
    unsafe {
        let env = &*context.cast::<Env>();
        env.events.borrow_mut().push("free");
        let n = env
            .allocations
            .borrow_mut()
            .remove(&(buffer as usize))
            .unwrap();
        dealloc(buffer.cast(), layout(n));
        0
    }
}
unsafe extern "C" fn case(context: *mut c_void, ch: u32, upper: i32, out: *mut u32) -> i32 {
    unsafe {
        let env = &*context.cast::<Env>();
        env.events.borrow_mut().push("case");
        if env.fail_case_at.get() == Some(ch) {
            return 37;
        }
        out.write(if upper != 0 && (97..=122).contains(&ch) {
            ch - 32
        } else if upper == 0 && (65..=90).contains(&ch) {
            ch + 32
        } else {
            ch
        });
        0
    }
}
unsafe extern "C" fn bounds(context: *mut c_void) -> i32 {
    unsafe {
        (&*context.cast::<Env>()).events.borrow_mut().push("bounds");
    }
    41
}
unsafe extern "C" fn set_tag(context: *mut c_void, tag: *const c_void) -> i32 {
    unsafe {
        (&*context.cast::<Env>()).tag.set(tag);
    }
    0
}
unsafe extern "C" fn object_slots(node: *const c_void) -> Hp2ArraySlots {
    unsafe { (&*node.cast::<Node>()).slots() }
}
unsafe extern "C" fn assign_range(
    dst: *mut c_void,
    src: *const c_void,
    start: i32,
    count: i32,
) -> i32 {
    unsafe {
        let source = &*src.cast::<Node>();
        let env = &*source.env;
        env.events.borrow_mut().push("range");
        if env.fail_range.get() {
            return 43;
        }
        let temp = source.value(3, start, count);
        let dst = &*dst.cast::<Node>();
        string::assign(
            dst.slots(),
            Some(temp.slots()),
            ptr::null(),
            false,
            source.svc(),
        )
        .err()
        .unwrap_or(0)
    }
}

#[test]
fn counted_copy_pointer_identity_and_allocated_terminator_are_distinct() {
    let env = Env::default();
    let source = Node::new(&env);
    source.set("abcd");
    unsafe {
        (*source.data.get()).cast::<Tchar>().add(1).write(0);
    }
    let copy = Node::new(&env);
    unsafe {
        string::assign(
            copy.slots(),
            Some(source.slots()),
            ptr::null(),
            true,
            copy.svc(),
        )
        .unwrap();
    }
    assert_eq!(copy.raw(), [97, 0, 99, 100, 0]);
    let terminated = Node::new(&env);
    unsafe {
        string::assign(
            terminated.slots(),
            None,
            (*source.data.get()).cast(),
            true,
            source.svc(),
        )
        .unwrap();
    }
    assert_eq!(terminated.raw(), [97, 0]);
    env.events.borrow_mut().clear();
    unsafe {
        string::assign(
            source.slots(),
            Some(source.slots()),
            ptr::null(),
            false,
            source.svc(),
        )
        .unwrap();
        string::assign(
            source.slots(),
            None,
            (*source.data.get()).cast(),
            false,
            source.svc(),
        )
        .unwrap();
    }
    assert!(env.events.borrow().is_empty());
    let empty = Node::new(&env);
    unsafe {
        array::set_exact(empty.slots(), 1, 4, ptr::null(), empty.svc().arrays).unwrap();
        (*empty.data.get()).cast::<Tchar>().write(0);
    }
    let counted = empty.value(6, 0, 0);
    assert_eq!(counted.raw(), [0]);
    assert!(empty.value(8, 0, 0).raw().is_empty());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn noinit_self_assignment_does_not_read_storage() {
    unsafe extern "C" {
        fn getpagesize() -> i32;
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            descriptor: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }
    let env = Env::default();
    let initialized = Node::new(&env);
    unsafe {
        let size = getpagesize() as usize;
        let anonymous = if cfg!(target_os = "macos") {
            0x1000
        } else {
            0x20
        };
        // Inaccessible payload makes even an otherwise-unused eager read fail.
        let page = mmap(ptr::null_mut(), size, 0, 0x02 | anonymous, -1, 0);
        assert_ne!(page, usize::MAX as *mut c_void);
        let slots = Hp2ArraySlots {
            data: page.cast(),
            num: page.byte_add(size_of::<*mut c_void>()).cast(),
            max: page
                .byte_add(size_of::<*mut c_void>() + size_of::<i32>())
                .cast(),
        };
        let result = string::assign(slots, Some(slots), ptr::null(), false, initialized.svc());
        assert_eq!(munmap(page, size), 0);
        assert_eq!(result, Ok(()));
    }
    assert!(env.events.borrow().is_empty());
}

#[test]
fn unsigned_mid_and_range_termination_match_native_boundaries() {
    let env = Env::default();
    let s = Node::new(&env);
    s.set("abCλ");
    assert_eq!(s.value(3, 2, i32::MAX).text(), "Cλ");
    assert!(s.value(3, -1, 2).raw().is_empty());
    assert!(s.value(3, 2, -1).raw().is_empty());
    assert_eq!(s.value(0, -1, 0).text(), "");
    assert_eq!(s.value(0, 99, 0).text(), "abCλ");
    assert_eq!(s.value(1, 2, 0).text(), "ab");
    assert_eq!(s.value(2, 2, 0).text(), "Cλ");
    unsafe {
        (*s.data.get()).cast::<Tchar>().add(1).write(0);
    }
    assert_eq!(s.value(0, 4, 0).raw(), [97, 0, 0, 0, 0]);
    assert_eq!(s.value(2, 4, 0).raw(), [97, 0]);
}

#[test]
fn directional_search_has_empty_needle_asymmetry_and_counted_reverse() {
    let env = Env::default();
    let s = Node::new(&env);
    s.set("ababa");
    unsafe {
        assert_eq!(string::search(s.slots(), wide("").as_ptr(), false), 0);
        assert_eq!(string::search(s.slots(), wide("").as_ptr(), true), 4);
        assert_eq!(string::search(s.slots(), wide("aba").as_ptr(), false), 0);
        assert_eq!(string::search(s.slots(), wide("aba").as_ptr(), true), 2);
        assert_eq!(string::search(s.slots(), wide("ababa!").as_ptr(), true), -1);
        (*s.data.get()).cast::<Tchar>().add(1).write(0);
        assert_eq!(string::search(s.slots(), wide("aba").as_ptr(), false), -1);
        assert_eq!(string::search(s.slots(), wide("aba").as_ptr(), true), 2);
    }
    let empty = Node::new(&env);
    unsafe {
        assert_eq!(string::search(empty.slots(), wide("").as_ptr(), true), -1);
    }
}

#[test]
fn split_reloads_live_source_and_separator_after_left_assignment() {
    let env = Env::default();
    let s = Node::new(&env);
    let sep = Node::new(&env);
    let right = Node::new(&env);
    s.set("ab::tail");
    sep.set("::");
    unsafe {
        assert_eq!(
            string::split(
                s.pointer(),
                sep.pointer(),
                s.pointer(),
                right.pointer(),
                false,
                s.svc()
            ),
            Ok(true)
        );
    }
    assert_eq!(s.text(), "ab");
    assert_eq!(right.text(), "");
    s.set("long::tail");
    sep.set("::");
    unsafe {
        string::split(
            s.pointer(),
            sep.pointer(),
            sep.pointer(),
            right.pointer(),
            false,
            s.svc(),
        )
        .unwrap();
    }
    assert_eq!(sep.text(), "long");
    assert_eq!(right.text(), "il");
    s.set("ab::tail");
    sep.set("::");
    unsafe {
        string::split(
            s.pointer(),
            sep.pointer(),
            right.pointer(),
            right.pointer(),
            false,
            s.svc(),
        )
        .unwrap();
    }
    assert_eq!(right.text(), "tail");
    sep.set("missing");
    env.events.borrow_mut().clear();
    unsafe {
        assert_eq!(
            string::split(
                s.pointer(),
                sep.pointer(),
                right.pointer(),
                right.pointer(),
                false,
                s.svc()
            ),
            Ok(false)
        );
    }
    assert!(env.events.borrow().is_empty());
}

#[test]
fn append_path_padding_case_and_chr_keep_native_value_rules() {
    let env = Env::default();
    let s = Node::new(&env);
    s.set("abCλ");
    assert_eq!(s.value(4, 0, 0).text(), "ABCλ");
    assert_eq!(s.value(5, 0, 0).text(), "abcλ");
    assert_eq!(s.value(6, 6, 0).text(), "  abCλ");
    assert_eq!(s.value(7, 6, 0).text(), "abCλ  ");
    assert_eq!(s.value(6, 2, 0).text(), "abCλ");
    assert_eq!(s.value(8, 955, 0).text(), "λ");
    unsafe {
        (*s.data.get()).cast::<Tchar>().add(1).write(0);
    }
    assert_eq!(s.value(6, 6, 0).raw(), [32, 32, 97, 0]);
    assert_eq!(s.value(7, 6, 0).raw(), [97, 0, 67, 955, 32, 32, 0]);
    s.set("base");
    unsafe {
        string::path_append(
            s.slots(),
            wide("file").as_ptr(),
            wide("/").as_ptr(),
            s.svc(),
        )
        .unwrap();
        string::append(s.slots(), wide("").as_ptr(), s.svc()).unwrap();
    }
    assert_eq!(s.text(), "base/file");
    s.set("base/");
    unsafe {
        string::path_append(
            s.slots(),
            wide("file").as_ptr(),
            wide("/").as_ptr(),
            s.svc(),
        )
        .unwrap();
    }
    assert_eq!(s.text(), "base/file");
}

#[test]
fn padding_keeps_exact_return_capacity() {
    let env = Env::default();
    let source = Node::new(&env);
    source.set("ab");
    env.events.borrow_mut().clear();
    let padded = source.value(6, 4, 0);
    assert_eq!(padded.text(), "  ab");
    unsafe {
        assert_eq!((*padded.num.get(), *padded.max.get()), (5, 5));
    }
    let empty = Node::new(&env);
    env.events.borrow_mut().clear();
    unsafe {
        string::append(empty.slots(), wide("").as_ptr(), empty.svc()).unwrap();
    }
    assert!(empty.raw().is_empty());
    assert!(env.events.borrow().is_empty());
}

#[test]
fn allocation_and_checked_size_errors_preserve_publication_order() {
    let env = Env::default();
    let s = Node::new(&env);
    s.set("old");
    let old = unsafe { *s.data.get() };
    env.events.borrow_mut().clear();
    env.fail_alloc.set(true);
    unsafe {
        assert_eq!(
            string::assign(s.slots(), None, wide("longer").as_ptr(), false, s.svc()),
            Err(17)
        );
        assert_eq!((*s.num.get(), *s.max.get()), (7, 7));
        assert_eq!(*s.data.get(), old);
    }
    assert_eq!(*env.events.borrow(), ["allocate"]);
    env.fail_alloc.set(false);
    env.fail_size.set(true);
    env.events.borrow_mut().clear();
    unsafe {
        assert_eq!(
            string::assign(s.slots(), None, wide("new").as_ptr(), false, s.svc()),
            Err(29)
        );
    }
    assert_eq!(*env.events.borrow(), ["allocate", "size"]);
    env.fail_size.set(false);
}

#[test]
fn checked_size_reentry_reloads_counted_source_storage() {
    let env = Env::default();
    let source = Node::new(&env);
    source.set("moving");
    let output = Node::new(&env);
    env.reenter_size.set(&source);
    unsafe {
        string::assign(
            output.slots(),
            Some(source.slots()),
            ptr::null(),
            true,
            source.svc(),
        )
        .unwrap();
    }
    assert_eq!(output.text(), "moving");
}

#[test]
fn case_and_split_stop_at_first_captured_error() {
    let env = Env::default();
    let s = Node::new(&env);
    s.set("abc");
    let output = Node::new(&env);
    env.fail_case_at.set(Some(98));
    unsafe {
        assert_eq!(
            string::value(output.slots(), s.slots(), 4, 0, 0, s.svc()),
            Err(37)
        );
    }
    assert_eq!(output.text(), "Abc");
    assert_eq!(
        env.events.borrow().iter().filter(|e| **e == "case").count(),
        2
    );
    let sep = Node::new(&env);
    sep.set("b");
    env.fail_range.set(true);
    env.events.borrow_mut().clear();
    unsafe {
        assert_eq!(
            string::split(
                s.pointer(),
                sep.pointer(),
                output.pointer(),
                output.pointer(),
                false,
                s.svc()
            ),
            Err(43)
        );
    }
    assert_eq!(*env.events.borrow(), ["range"]);
    assert_eq!(output.text(), "Abc");
}
