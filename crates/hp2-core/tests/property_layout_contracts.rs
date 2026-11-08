//! Deterministic Rust raw-storage contracts, not a native C++ oracle. These
//! exercise compiler-probed fields and callback ordering without C++ objects,
//! virtual dispatch, registration, or native exception/checked-size diagnostics.
use hp2_abi::engine::generated::{UArrayProperty, UBoolProperty, UFixedArrayProperty, UMapProperty,
    UProperty, UStruct, UStructProperty};
use hp2_core::property_link::{self as layout, Hp2PropertyTypeLayoutAbi as Abi,
    Hp2PropertyTypeLayoutKind as Kind, Hp2PropertyTypeLayoutServices as Services, CTOR};
use std::{cell::Cell, ffi::c_void, mem::{MaybeUninit, offset_of, size_of}, ptr};

type Object = *mut c_void;
const NATIVE: u32 = 0x1000;
// Values from native-property-layout-abi-probe.json, not inferred Rust sizes.
const ABI: Abi = Abi {
    property_alignment: 4, byte_size: 1, int_size: 4, bool_size: 4, float_size: 4,
    object_size: 8, name_size: 4, string_size: 16, array_size: 16, map_size: 32,
    object_alignment: 8, name_alignment: 4, string_alignment: 8,
    array_alignment: 8, map_alignment: 8, first_bitfield: 1,
};

// Never move initialized native bytes, form a whole-mirror reference, or drop
// native fields. Painting in place makes preservation of padding observable.
struct Storage<T>(*mut MaybeUninit<T>);
impl<T> Storage<T> {
    fn new() -> Self {
        let storage = Self(Box::into_raw(Box::new(MaybeUninit::uninit())));
        unsafe { storage.0.cast::<u8>().write_bytes(0xa5, size_of::<T>()) };
        storage
    }
    fn ptr(&self) -> *mut T { self.0.cast() }
    fn object(&self) -> Object { self.0.cast() }
    fn bytes(&self) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(self.0.cast::<u8>(), size_of::<T>()).to_vec() }
    }
}
impl<T> Drop for Storage<T> {
    fn drop(&mut self) { unsafe { drop(Box::from_raw(self.0)) }; }
}
unsafe fn base(p: Object) -> *mut UProperty {
    let p = p.cast::<UProperty>();
    unsafe {
        (*p).cpp_property = 0;
        (*p).property_flags = 0x200;
        (*p).element_size = 37;
        (*p).offset = -19;
    }
    p
}
fn untouched(before: &[u8], after: &[u8], owned: &[(usize, usize)]) {
    assert_eq!(before.len(), after.len());
    for (i, (old, new)) in before.iter().zip(after).enumerate() {
        if !owned.iter().any(|&(start, count)| (start..start + count).contains(&i)) {
            assert_eq!(old, new, "unowned byte {i}");
        }
    }
}
fn layout_slots() -> [(usize, usize); 3] {
    [(offset_of!(UProperty, element_size), 4), (offset_of!(UProperty, offset), 4),
        (offset_of!(UProperty, property_flags), 4)]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Is(Object, Kind), Size(Object), Merge(Object), Preload(Object), Link(Object), Checked(u64),
}
struct Host {
    events: Vec<Event>,
    families: Vec<(Object, u32)>,
    owner_size: i32,
    merge: i32,
    fail_at: usize,
    hook: unsafe fn(*mut Host, Event),
    root: Object,
    a: Object,
    b: Object,
    c: Object,
    nested_result: Option<Result<(), i32>>,
    observed: Vec<(i32, i32, u32)>,
    archive: Object,
    wrong_archive: bool,
}
unsafe fn idle(_: *mut Host, _: Event) {}
impl Host {
    fn new() -> Box<Self> {
        Box::new(Self {
            events: Vec::new(), families: Vec::new(), owner_size: 5, merge: 1,
            fail_at: 0, hook: idle, root: ptr::null_mut(), a: ptr::null_mut(),
            b: ptr::null_mut(), c: ptr::null_mut(), nested_result: None,
            observed: Vec::new(), archive: ptr::null_mut(), wrong_archive: false,
        })
    }
    fn family(&mut self, object: Object, kinds: &[Kind]) {
        self.families.push((object, kinds.iter().fold(0, |bits, k| bits | (1 << *k as u32))));
    }
}
thread_local! { static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) }; }
struct Active(*mut Host);
impl Drop for Active {
    fn drop(&mut self) { ACTIVE.with(|a| a.set(self.0)); }
}
fn activate(h: *mut Host) -> Active { Active(ACTIVE.with(|a| a.replace(h))) }
unsafe fn event(e: Event) -> i32 {
    let h = ACTIVE.with(Cell::get);
    unsafe {
        (*h).events.push(e);
        let index = (*h).events.len();
        let hook = (*h).hook;
        hook(h, e);
        if index == (*h).fail_at { 701 } else { 0 }
    }
}
unsafe extern "C" fn is_kind(p: Object, kind: Kind, out: *mut i32) -> i32 {
    unsafe {
        let status = event(Event::Is(p, kind));
        if status != 0 { return status; }
        let h = ACTIVE.with(Cell::get);
        out.write(i32::from((*h).families.iter().any(|&(object, bits)|
            object == p && bits & (1 << kind as u32) != 0)));
        0
    }
}
unsafe extern "C" fn owner_size(p: Object, out: *mut i32) -> i32 {
    unsafe {
        let status = event(Event::Size(p));
        if status == 0 { out.write((*ACTIVE.with(Cell::get)).owner_size); }
        status
    }
}
unsafe extern "C" fn merge(p: Object, out: *mut i32) -> i32 {
    unsafe {
        let status = event(Event::Merge(p));
        if status == 0 { out.write((*ACTIVE.with(Cell::get)).merge); }
        status
    }
}
unsafe extern "C" fn preload(ar: Object, p: Object) -> i32 {
    unsafe {
        let h = ACTIVE.with(Cell::get);
        (*h).wrong_archive |= ar != (*h).archive;
        event(Event::Preload(p))
    }
}
unsafe extern "C" fn link(p: Object, ar: Object) -> i32 {
    unsafe {
        let h = ACTIVE.with(Cell::get);
        (*h).wrong_archive |= ar != (*h).archive;
        event(Event::Link(p))
    }
}
unsafe extern "C" fn checked(value: u64, out: *mut i32) -> i32 {
    unsafe {
        let status = event(Event::Checked(value));
        if status != 0 { return status; }
        match i32::try_from(value) {
            Ok(size) => { out.write(size); 0 }
            Err(_) => 702,
        }
    }
}
const SERVICES: Services = Services {
    is_kind, owner_size, owner_merge_bools: merge, preload, link, checked_size: checked,
};
unsafe fn run(p: Object, kind: Kind, previous: Object) -> Result<(), i32> {
    unsafe {
        layout::type_layout(p.cast(), (*ACTIVE.with(Cell::get)).archive, previous,
            kind, &ABI, SERVICES)
    }
}
fn classification(p: Object) -> Vec<Event> {
    [Kind::TypeLayoutObject, Kind::TypeLayoutName, Kind::TypeLayoutString, Kind::TypeLayoutArray, Kind::TypeLayoutMap, Kind::TypeLayoutFixedArray,
        Kind::TypeLayoutStruct].map(|k| Event::Is(p, k)).to_vec()
}

#[test]
fn admission_publishes_only_owned_slots_including_self_aliases() {
    let fixed = Storage::<UFixedArrayProperty>::new();
    let array = Storage::<UArrayProperty>::new();
    let before_fixed = fixed.bytes();
    let before_array = array.bytes();
    unsafe {
        layout::publish_fixed_inner(fixed.ptr(), fixed.object(), 7);
        layout::publish_array_inner(array.ptr(), array.object());
        assert_eq!((*fixed.ptr()).inner, fixed.object());
        assert_eq!((*fixed.ptr()).count, 7);
        assert_eq!((*array.ptr()).inner, array.object());
    }
    untouched(&before_fixed, &fixed.bytes(), &[
        (offset_of!(UFixedArrayProperty, inner), size_of::<Object>()),
        (offset_of!(UFixedArrayProperty, count), 4),
    ]);
    untouched(&before_array, &array.bytes(), &[
        (offset_of!(UArrayProperty, inner), size_of::<Object>()),
    ]);
}

#[test]
fn alignment_native_short_circuits_special_families_and_element_fallbacks() {
    let property = Storage::<UProperty>::new();
    let p = unsafe { base(property.object()) };
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    unsafe {
        for (cpp, flags) in [(1, 0), (0, NATIVE)] {
            (*p).cpp_property = cpp;
            (*p).property_flags = flags;
            assert_eq!(layout::type_alignment(p, &ABI, SERVICES), Ok(4));
            assert_eq!(h.events, []);
        }
        (*p).cpp_property = 0;
        (*p).property_flags = 0;
        (*p).element_size = 2;
        let families = [Kind::TypeLayoutObject, Kind::TypeLayoutName, Kind::TypeLayoutString,
            Kind::TypeLayoutArray, Kind::TypeLayoutMap];
        for kind in families {
            h.families.clear();
            h.family(property.object(), &[kind]);
            h.events.clear();
            assert_eq!(layout::type_alignment(p, &ABI, SERVICES), Ok(4));
        }
        h.families.clear();
        for (element, expected) in [(-7, 1), (0, 1), (1, 1), (2, 2), (3, 1), (4, 4), (99, 4)] {
            h.events.clear();
            (*p).element_size = element;
            assert_eq!(layout::type_alignment(p, &ABI, SERVICES), Ok(expected));
            assert_eq!(h.events, classification(property.object()));
        }
    }
}

#[test]
fn alignment_uses_live_nested_pointer_after_classification_and_stops_on_error() {
    unsafe fn replace(h: *mut Host, event: Event) {
        unsafe {
            if event == Event::Is((*h).root, Kind::TypeLayoutFixedArray) {
                (*(*h).root.cast::<UFixedArrayProperty>()).inner = (*h).b;
            }
        }
    }
    let fixed = Storage::<UFixedArrayProperty>::new();
    let a = Storage::<UProperty>::new();
    let b = Storage::<UProperty>::new();
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    unsafe {
        base(fixed.object()); base(a.object()); base(b.object());
        (*a.ptr()).element_size = 2;
        (*b.ptr()).cpp_property = 1;
        layout::publish_fixed_inner(fixed.ptr(), a.object(), 3);
        h.root = fixed.object(); h.b = b.object(); h.hook = replace;
        h.family(fixed.object(), &[Kind::TypeLayoutFixedArray]);
        assert_eq!(layout::type_alignment(fixed.ptr().cast(), &ABI, SERVICES), Ok(4));
        assert_eq!(h.events, classification(fixed.object())[..6]);
        h.events.clear(); h.fail_at = 3;
        assert_eq!(layout::type_alignment(fixed.ptr().cast(), &ABI, SERVICES), Err(701));
        assert_eq!(h.events, classification(fixed.object())[..3]);
    }
}

#[test]
fn scalar_layout_live_size_wrapping_and_failed_owner_commits_only_element() {
    unsafe fn change_size(h: *mut Host, event: Event) {
        unsafe {
            if let Event::Size(p) = event {
                (*h).observed.push(((*p.cast::<UProperty>()).element_size,
                    (*p.cast::<UProperty>()).offset, (*p.cast::<UProperty>()).property_flags));
                (*p.cast::<UProperty>()).element_size = 8;
            }
        }
    }
    for (kind, size) in [(Kind::TypeLayoutByte, 1), (Kind::TypeLayoutInt, 4), (Kind::TypeLayoutFloat, 4),
        (Kind::TypeLayoutObject, 8), (Kind::TypeLayoutName, 4), (Kind::TypeLayoutString, 16)] {
        let storage = Storage::<UProperty>::new();
        let p = unsafe { base(storage.object()) };
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.family(storage.object(), &[kind]);
        h.fail_at = 1;
        let before = storage.bytes();
        unsafe {
            assert_eq!(run(storage.object(), kind, ptr::null_mut()), Err(701));
            assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags), (size, -19, 0x200));
        }
        assert_eq!(h.events, [Event::Size(storage.object())]);
        untouched(&before, &storage.bytes(), &[(offset_of!(UProperty, element_size), 4)]);
        h.events.clear(); h.fail_at = 0; h.hook = change_size;
        h.owner_size = i32::MAX;
        unsafe {
            assert_eq!(run(storage.object(), kind, ptr::null_mut()), Ok(()));
            assert_eq!((*p).offset, i32::MIN);
            assert_eq!((*p).element_size, 8);
            assert_eq!((*p).property_flags, 0x200 | if kind == Kind::TypeLayoutString { CTOR as u32 } else { 0 });
        }
        assert_eq!(h.observed, [(size, -19, 0x200)]);
        untouched(&before, &storage.bytes(), &layout_slots());
    }
    // Distinguish a live ElementSize reread from the original scalar constant.
    let storage = Storage::<UProperty>::new();
    let p = unsafe { base(storage.object()) };
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.hook = change_size; h.owner_size = 9;
    unsafe {
        assert_eq!(run(storage.object(), Kind::TypeLayoutByte, ptr::null_mut()), Ok(()));
        assert_eq!((*p).offset, 16);
    }
}

#[test]
fn bool_merge_reentry_rereads_masks_and_supports_self_previous() {
    unsafe fn reenter(h: *mut Host, event: Event) {
        unsafe {
            if event == Event::Merge((*h).root) {
                let p = (*h).root.cast::<UProperty>();
                (*p).cpp_property = 1;
                (*p).offset = 23;
                (*(*h).root.cast::<UBoolProperty>()).bit_mask = 16;
                (*h).nested_result = Some(run((*h).root, Kind::TypeLayoutBool, ptr::null_mut()));
                (*p).cpp_property = 0;
                (*(*h).a.cast::<UProperty>()).offset = 35;
                (*(*h).a.cast::<UBoolProperty>()).bit_mask = 64;
            }
        }
    }
    for self_previous in [false, true] {
        let current = Storage::<UBoolProperty>::new();
        let other = Storage::<UBoolProperty>::new();
        let p = unsafe { base(current.object()) };
        unsafe { base(other.object()); (*current.ptr()).bit_mask = 1; (*other.ptr()).bit_mask = 2; }
        let previous = if self_previous { current.object() } else { other.object() };
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.root = current.object(); h.a = previous; h.hook = reenter;
        h.family(previous, &[Kind::TypeLayoutBool]);
        let before = current.bytes();
        unsafe {
            assert_eq!(run(current.object(), Kind::TypeLayoutBool, previous), Ok(()));
            assert_eq!(((*p).offset, (*current.ptr()).bit_mask, (*p).element_size), (35, 128, 4));
        }
        assert_eq!(h.nested_result, Some(Ok(())));
        assert_eq!(h.events, [Event::Is(previous, Kind::TypeLayoutBool), Event::Merge(current.object())]);
        untouched(&before, &current.bytes(), &[
            (offset_of!(UProperty, element_size), 4), (offset_of!(UProperty, offset), 4),
            (offset_of!(UBoolProperty, bit_mask), 4),
        ]);
    }
}

#[test]
fn bool_native_cpp_failure_and_last_bit_paths_preserve_statement_prefixes() {
    let current = Storage::<UBoolProperty>::new();
    let previous = Storage::<UBoolProperty>::new();
    let p = unsafe { base(current.object()) };
    unsafe { base(previous.object()); (*current.ptr()).bit_mask = 32; (*previous.ptr()).bit_mask = 0x8000_0000; }
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.family(previous.object(), &[Kind::TypeLayoutBool]);
    unsafe {
        (*p).cpp_property = 1;
        assert_eq!(run(current.object(), Kind::TypeLayoutBool, previous.object()), Ok(()));
        assert_eq!(((*p).offset, (*current.ptr()).bit_mask, (*p).element_size), (-19, 32, 4));
        assert_eq!(h.events, []);
        (*p).cpp_property = 0;
        (*p).property_flags = NATIVE;
        for failure in 1..=3 {
            (*p).element_size = 37;
            h.events.clear(); h.fail_at = failure;
            let before = current.bytes();
            assert_eq!(run(current.object(), Kind::TypeLayoutBool, previous.object()), Err(701));
            assert_eq!(current.bytes(), before);
            assert_eq!(h.events, [Event::Is(previous.object(), Kind::TypeLayoutBool),
                Event::Merge(current.object()), Event::Size(current.object())][..failure]);
        }
        h.events.clear(); h.fail_at = 0;
        assert_eq!(run(current.object(), Kind::TypeLayoutBool, previous.object()), Ok(()));
        assert_eq!(((*p).offset, (*current.ptr()).bit_mask, (*p).element_size), (8, 1, 4));
        // A null previous cannot merge; the current owner's policy and size
        // still determine the newly committed storage.
        h.events.clear();
        assert_eq!(run(current.object(), Kind::TypeLayoutBool, ptr::null_mut()), Ok(()));
        assert_eq!(((*p).offset, (*current.ptr()).bit_mask, (*p).element_size), (8, 1, 4));
        assert_eq!(h.events, [Event::Merge(current.object()), Event::Size(current.object())]);
    }
}

#[test]
fn fixed_layout_reloads_inner_count_and_constructor_flags_after_reentry() {
    unsafe fn mutate(h: *mut Host, event: Event) {
        unsafe {
            let fixed = (*h).root.cast::<UFixedArrayProperty>();
            if event == Event::Preload((*h).a) {
                (*fixed).inner = (*h).b;
            } else if event == Event::Link((*h).b) {
                (*fixed).inner = (*h).c;
                (*fixed).count = 3;
                (*h).nested_result = Some(run((*h).c, Kind::TypeLayoutInt, ptr::null_mut()));
            } else if event == Event::Size((*h).root) {
                (*fixed).inner = (*h).a;
                (*(*h).a.cast::<UProperty>()).element_size = 2;
                (*(*h).a.cast::<UProperty>()).property_flags |= CTOR as u32;
            }
        }
    }
    let fixed = Storage::<UFixedArrayProperty>::new();
    let a = Storage::<UProperty>::new();
    let b = Storage::<UProperty>::new();
    let c = Storage::<UProperty>::new();
    let p = unsafe { base(fixed.object()) };
    unsafe { base(a.object()); base(b.object()); base(c.object());
        layout::publish_fixed_inner(fixed.ptr(), a.object(), 2); }
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.root = fixed.object(); h.a = a.object(); h.b = b.object(); h.c = c.object(); h.hook = mutate;
    h.archive = b.object(); h.family(fixed.object(), &[Kind::TypeLayoutFixedArray]);
    let before = fixed.bytes();
    unsafe {
        assert_eq!(run(fixed.object(), Kind::TypeLayoutFixedArray, ptr::null_mut()), Ok(()));
        assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags), (12, 6, 0x200 | CTOR as u32));
        assert_eq!((*fixed.ptr()).inner, a.object());
        assert_eq!((*fixed.ptr()).count, 3);
    }
    assert_eq!(h.nested_result, Some(Ok(())));
    let mut expected = vec![Event::Preload(a.object()), Event::Link(b.object()),
        Event::Size(c.object()), Event::Checked(12), Event::Size(fixed.object())];
    expected.extend_from_slice(&classification(fixed.object())[..6]);
    expected.extend(classification(a.object()));
    assert_eq!(h.events, expected);
    assert!(!h.wrong_archive, "preload/link must receive the original archive");
    let mut owned = layout_slots().to_vec();
    owned.push((offset_of!(UFixedArrayProperty, count), 4));
    untouched(&before, &fixed.bytes(), &owned);
}

#[test]
fn fixed_unsigned_product_narrowing_and_first_error_preserve_committed_prefix() {
    let fixed = Storage::<UFixedArrayProperty>::new();
    let inner = Storage::<UProperty>::new();
    let p = unsafe { base(fixed.object()) };
    unsafe { base(inner.object()); }
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    // cpp short-circuits recursive alignment, permitting this raw self-alias.
    unsafe {
        (*p).cpp_property = 1;
        layout::publish_fixed_inner(fixed.ptr(), fixed.object(), 2);
        assert_eq!(run(fixed.object(), Kind::TypeLayoutFixedArray, ptr::null_mut()), Ok(()));
        assert_eq!(((*p).element_size, (*p).offset), (74, 8));
        layout::publish_fixed_inner(fixed.ptr(), inner.object(), 3);
        (*inner.ptr()).element_size = -1;
        (*p).element_size = 37; (*p).offset = -19;
        h.events.clear();
        let before = fixed.bytes();
        assert_eq!(run(fixed.object(), Kind::TypeLayoutFixedArray, ptr::null_mut()), Err(702));
        assert_eq!(fixed.bytes(), before);
        assert_eq!(h.events, [Event::Preload(inner.object()), Event::Link(inner.object()),
            Event::Checked(u64::MAX - 2)]);
        (*inner.ptr()).element_size = i32::MAX;
        h.events.clear();
        assert_eq!(run(fixed.object(), Kind::TypeLayoutFixedArray, ptr::null_mut()), Err(702));
        assert_eq!(h.events[2], Event::Checked(6_442_450_941));
        (*inner.ptr()).element_size = 2;
        for failure in 1..=4 {
            h.events.clear(); h.fail_at = failure;
            (*p).element_size = 37; (*p).offset = -19;
            assert_eq!(run(fixed.object(), Kind::TypeLayoutFixedArray, ptr::null_mut()), Err(701));
            assert_eq!(((*p).element_size, (*p).offset), (if failure == 4 { 6 } else { 37 }, -19));
            assert_eq!(h.events, [Event::Preload(inner.object()), Event::Link(inner.object()),
                Event::Checked(6), Event::Size(fixed.object())][..failure]);
        }
    }
}

#[test]
fn dynamic_and_map_live_pointer_replacement_and_link_reentry() {
    unsafe fn mutate(h: *mut Host, event: Event) {
        unsafe {
            if event == Event::Preload((*h).a) {
                // Inner and Key are accessed through their own generated type.
                if (*h).families.iter().any(|&(p, bits)| p == (*h).root && bits & (1 << Kind::TypeLayoutMap as u32) != 0) {
                    (*(*h).root.cast::<UMapProperty>()).key = (*h).b;
                } else {
                    (*(*h).root.cast::<UArrayProperty>()).inner = (*h).b;
                }
            } else if event == Event::Link((*h).b) {
                (*h).nested_result = Some(run((*h).b, Kind::TypeLayoutByte, ptr::null_mut()));
                if (*h).families.iter().any(|&(p, bits)| p == (*h).root && bits & (1 << Kind::TypeLayoutMap as u32) != 0) {
                    (*(*h).root.cast::<UMapProperty>()).value = (*h).c;
                }
            } else if event == Event::Preload((*h).c) {
                (*(*h).root.cast::<UMapProperty>()).value = (*h).a;
            }
        }
    }
    let array = Storage::<UArrayProperty>::new();
    let map = Storage::<UMapProperty>::new();
    let a = Storage::<UProperty>::new();
    let b = Storage::<UProperty>::new();
    let c = Storage::<UProperty>::new();
    unsafe { base(a.object()); base(b.object()); base(c.object()); }
    for (root, kind, size) in [(array.object(), Kind::TypeLayoutArray, 16), (map.object(), Kind::TypeLayoutMap, 32)] {
        let p = unsafe { base(root) };
        unsafe {
            if kind == Kind::TypeLayoutArray { layout::publish_array_inner(array.ptr(), a.object()); }
            else { (*map.ptr()).key = a.object(); (*map.ptr()).value = b.object(); }
        }
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.root = root; h.a = a.object(); h.b = b.object(); h.c = c.object(); h.hook = mutate;
        h.archive = c.object(); h.family(root, &[kind]);
        unsafe {
            assert_eq!(run(root, kind, ptr::null_mut()), Ok(()));
            assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags), (size, 8, 0x200 | CTOR as u32));
        }
        let mut expected = vec![Event::Preload(a.object()), Event::Link(b.object()), Event::Size(b.object())];
        if kind == Kind::TypeLayoutMap { expected.extend([Event::Preload(c.object()), Event::Link(a.object())]); }
        expected.push(Event::Size(root));
        expected.extend_from_slice(&classification(root)[..if kind == Kind::TypeLayoutArray { 4 } else { 5 }]);
        assert_eq!(h.events, expected);
        assert_eq!(h.nested_result, Some(Ok(())));
        assert!(!h.wrong_archive, "nested calls retain archive identity");
    }
}

#[test]
fn dynamic_and_map_first_errors_commit_size_only_after_all_nested_calls() {
    let array = Storage::<UArrayProperty>::new();
    let map = Storage::<UMapProperty>::new();
    let inner = Storage::<UProperty>::new();
    unsafe { base(inner.object()); }
    for (root, kind, size, nested) in [(array.object(), Kind::TypeLayoutArray, 16, 2),
        (map.object(), Kind::TypeLayoutMap, 32, 4)] {
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.family(root, &[kind]);
        unsafe {
            if kind == Kind::TypeLayoutArray { layout::publish_array_inner(array.ptr(), root); }
            else { (*map.ptr()).key = root; (*map.ptr()).value = inner.object(); }
        }
        let mut expected = vec![Event::Preload(root), Event::Link(root)];
        if kind == Kind::TypeLayoutMap { expected.extend([Event::Preload(inner.object()), Event::Link(inner.object())]); }
        expected.push(Event::Size(root));
        expected.extend_from_slice(&classification(root)[..if kind == Kind::TypeLayoutArray { 4 } else { 5 }]);
        for failure in 1..=expected.len() {
            let p = unsafe { base(root) };
            let before = if kind == Kind::TypeLayoutArray { array.bytes() } else { map.bytes() };
            h.events.clear(); h.fail_at = failure;
            unsafe {
                assert_eq!(run(root, kind, ptr::null_mut()), Err(701));
                assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags),
                    (if failure > nested { size } else { 37 }, -19, 0x200));
            }
            assert_eq!(h.events, expected[..failure]);
            let after = if kind == Kind::TypeLayoutArray { array.bytes() } else { map.bytes() };
            untouched(&before, &after, &[(offset_of!(UProperty, element_size), 4)]);
        }
    }
}

#[test]
fn struct_preloads_only_and_rereads_replaced_structure_for_size_alignment_and_ctor() {
    unsafe fn mutate(h: *mut Host, event: Event) {
        unsafe {
            let p = (*h).root.cast::<UStructProperty>();
            if event == Event::Preload((*h).a) {
                (*p).r#struct = (*h).b;
            } else if event == Event::Size((*h).root) {
                (*p).r#struct = (*h).c;
                (*h).nested_result = Some(run((*h).archive, Kind::TypeLayoutByte, ptr::null_mut()));
            } else if event == Event::Is((*h).root, Kind::TypeLayoutStruct) {
                (*(*h).c.cast::<UStruct>()).properties_align = 16;
                (*(*h).c.cast::<UStruct>()).constructor_link = (*h).root;
            }
        }
    }
    let property = Storage::<UStructProperty>::new();
    let a = Storage::<UStruct>::new();
    let b = Storage::<UStruct>::new();
    let c = Storage::<UStruct>::new();
    let nested = Storage::<UProperty>::new();
    let p = unsafe { base(property.object()) };
    unsafe {
        base(nested.object());
        (*property.ptr()).r#struct = a.object();
        (*b.ptr()).properties_size = 26;
        (*c.ptr()).properties_align = 2;
        (*c.ptr()).constructor_link = ptr::null_mut();
    }
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.root = property.object(); h.a = a.object(); h.b = b.object(); h.c = c.object(); h.hook = mutate;
    h.archive = nested.object();
    h.family(property.object(), &[Kind::TypeLayoutStruct]);
    unsafe {
        assert_eq!(run(property.object(), Kind::TypeLayoutStruct, ptr::null_mut()), Ok(()));
        assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags), (26, 16, 0x200 | CTOR as u32));
    }
    let mut expected = vec![Event::Preload(a.object()), Event::Size(property.object()), Event::Size(nested.object())];
    expected.extend(classification(property.object()));
    assert_eq!(h.events, expected);
    assert_eq!(h.nested_result, Some(Ok(())));
}

#[test]
fn ctor_flags_are_live_native_suppressed_and_never_cleared() {
    let string = Storage::<UProperty>::new();
    let fixed = Storage::<UFixedArrayProperty>::new();
    let array = Storage::<UArrayProperty>::new();
    let map = Storage::<UMapProperty>::new();
    let structure = Storage::<UStructProperty>::new();
    let inner = Storage::<UProperty>::new();
    let descriptor = Storage::<UStruct>::new();
    unsafe {
        base(inner.object()); (*inner.ptr()).element_size = 2;
        (*inner.ptr()).property_flags = CTOR as u32;
        layout::publish_fixed_inner(fixed.ptr(), inner.object(), 2);
        layout::publish_array_inner(array.ptr(), inner.object());
        (*map.ptr()).key = inner.object(); (*map.ptr()).value = inner.object();
        (*structure.ptr()).r#struct = descriptor.object();
        (*descriptor.ptr()).properties_size = 9;
        (*descriptor.ptr()).properties_align = 2;
        (*descriptor.ptr()).constructor_link = inner.object();
    }
    for (root, kind) in [(string.object(), Kind::TypeLayoutString), (fixed.object(), Kind::TypeLayoutFixedArray),
        (array.object(), Kind::TypeLayoutArray), (map.object(), Kind::TypeLayoutMap), (structure.object(), Kind::TypeLayoutStruct)] {
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.family(root, &[kind]);
        for flags in [0x200, 0x200 | NATIVE, 0x200 | NATIVE | CTOR as u32] {
            let p = unsafe { base(root) };
            unsafe {
                (*p).property_flags = flags;
                assert_eq!(run(root, kind, ptr::null_mut()), Ok(()));
                assert_eq!((*p).property_flags,
                    flags | if flags & NATIVE == 0 { CTOR as u32 } else { 0 });
            }
        }
    }
    // Absence of a nested constructor does not clear an existing bit, nor
    // introduce one when the bit was not already present.
    unsafe { (*inner.ptr()).property_flags = 0; (*descriptor.ptr()).constructor_link = ptr::null_mut(); }
    for (root, kind) in [(fixed.object(), Kind::TypeLayoutFixedArray), (structure.object(), Kind::TypeLayoutStruct)] {
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.family(root, &[kind]);
        for flags in [0x200, 0x200 | CTOR as u32] {
            let p = unsafe { base(root) };
            unsafe {
                (*p).property_flags = flags;
                assert_eq!(run(root, kind, ptr::null_mut()), Ok(()));
                assert_eq!((*p).property_flags, flags);
            }
        }
    }
}

#[test]
fn bool_disabled_merge_ignores_previous_and_owner_reentry_keeps_live_writes() {
    unsafe fn reenter(h: *mut Host, event: Event) {
        unsafe {
            if event == Event::Size((*h).root) {
                let p = (*h).root.cast::<UProperty>();
                (*p).cpp_property = 1;
                (*p).offset = 47;
                (*(*h).root.cast::<UBoolProperty>()).bit_mask = 256;
                (*h).nested_result = Some(run((*h).root, Kind::TypeLayoutBool, ptr::null_mut()));
                (*p).cpp_property = 0;
            }
        }
    }
    for fail in [false, true] {
        let current = Storage::<UBoolProperty>::new();
        let previous = Storage::<UBoolProperty>::new();
        let p = unsafe { base(current.object()) };
        unsafe {
            base(previous.object());
            (*current.ptr()).bit_mask = 1;
            (*previous.ptr()).bit_mask = 64;
            (*previous.ptr().cast::<UProperty>()).offset = 99;
        }
        let mut h = Host::new();
        let _active = activate(&raw mut *h);
        h.root = current.object(); h.merge = 0; h.hook = reenter;
        h.family(previous.object(), &[Kind::TypeLayoutBool]);
        h.fail_at = if fail { 3 } else { 0 };
        unsafe {
            assert_eq!(run(current.object(), Kind::TypeLayoutBool, previous.object()),
                if fail { Err(701) } else { Ok(()) });
            // Inner completed writes survive outer owner-size failure. On
            // success the outer assignment resumes after the reentrant call.
            assert_eq!(((*p).offset, (*current.ptr()).bit_mask, (*p).element_size),
                if fail { (47, 256, 4) } else { (8, 1, 4) });
        }
        assert_eq!(h.nested_result, Some(Ok(())));
        assert_eq!(h.events, [Event::Is(previous.object(), Kind::TypeLayoutBool),
            Event::Merge(current.object()), Event::Size(current.object())]);
    }
}

#[test]
fn struct_first_error_preserves_preload_and_element_commit_boundaries() {
    let property = Storage::<UStructProperty>::new();
    let descriptor = Storage::<UStruct>::new();
    unsafe {
        (*property.ptr()).r#struct = descriptor.object();
        (*descriptor.ptr()).properties_size = 13;
        (*descriptor.ptr()).properties_align = 8;
        (*descriptor.ptr()).constructor_link = property.object();
    }
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.family(property.object(), &[Kind::TypeLayoutStruct]);
    let mut expected = vec![Event::Preload(descriptor.object()), Event::Size(property.object())];
    expected.extend(classification(property.object()));
    for failure in 1..=expected.len() {
        let p = unsafe { base(property.object()) };
        let before = property.bytes();
        h.events.clear(); h.fail_at = failure;
        unsafe {
            assert_eq!(run(property.object(), Kind::TypeLayoutStruct, ptr::null_mut()), Err(701));
            assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags),
                (if failure == 1 { 37 } else { 13 }, -19, 0x200));
        }
        assert_eq!(h.events, expected[..failure]);
        untouched(&before, &property.bytes(), &[(offset_of!(UProperty, element_size), 4)]);
    }
}

#[test]
fn owner_callback_changes_native_suppression_before_alignment_and_ctor_write() {
    unsafe fn make_native(h: *mut Host, event: Event) {
        unsafe {
            if event == Event::Size((*h).root) {
                (*(*h).root.cast::<UProperty>()).property_flags |= NATIVE;
            }
        }
    }
    let string = Storage::<UProperty>::new();
    let p = unsafe { base(string.object()) };
    let mut h = Host::new();
    let _active = activate(&raw mut *h);
    h.root = string.object(); h.hook = make_native;
    unsafe {
        assert_eq!(run(string.object(), Kind::TypeLayoutString, ptr::null_mut()), Ok(()));
        assert_eq!(((*p).element_size, (*p).offset, (*p).property_flags),
            (16, 8, 0x200 | NATIVE));
    }
    assert_eq!(h.events, [Event::Size(string.object())]);
}
