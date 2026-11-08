//! Rust raw-storage contracts over compiler-probed fields, not live C++ objects.
//! Native typed construction/virtual dispatch is covered by the SDK consumer.
use hp2_abi::engine::generated::{
    UBoolProperty, UByteProperty, UClassProperty, UField, UObject, UObjectProperty, UProperty,
    UStruct, UStructProperty,
};
use hp2_core::native_property_offsets::{
    self as construction, Hp2NativePropertyInteger, Hp2NativePropertyServices, Hp2NativePropertyWire,
    Hp2PropertyConstructionServices,
};
use std::{cell::Cell, ffi::c_void, mem::{MaybeUninit, offset_of, size_of}, ptr};

type Object = *mut c_void;

// Keep storage in place after painting every byte, including opaque base/vptr
// slots and padding. Never form a reference to or assume_init a native mirror.
struct Storage<T>(*mut MaybeUninit<T>);
impl<T> Storage<T> {
    fn new() -> Self {
        let mut storage = Self(Box::into_raw(Box::new(MaybeUninit::uninit())));
        unsafe { storage.as_ptr().cast::<u8>().write_bytes(0xa5, size_of::<T>()) };
        storage
    }
    fn as_ptr(&mut self) -> *mut T { self.0.cast() }
    fn bytes(&mut self) -> Vec<u8> { unsafe { bytes(self.as_ptr()) } }
}
impl<T> Drop for Storage<T> {
    fn drop(&mut self) {
        // Reclaim the allocation without interpreting or dropping native bytes.
        unsafe { drop(Box::from_raw(self.0)) };
    }
}
unsafe fn bytes<T>(value: *const T) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(value.cast::<u8>(), size_of::<T>()).to_vec() }
}
fn untouched(before: &[u8], after: &[u8], owned: &[(usize, usize)]) {
    assert_eq!(before.len(), after.len());
    for (index, (&old, &new)) in before.iter().zip(after).enumerate() {
        if !owned.iter().any(|&(start, size)| (start..start + size).contains(&index)) {
            assert_eq!(old, new, "unowned byte {index} changed");
        }
    }
}

fn base_prefixes<T>() {
    let mut storage = Storage::<T>::new();
    let p = storage.as_ptr().cast::<UProperty>();
    let before = storage.bytes();
    unsafe { construction::initialize_default_data(p) };
    unsafe {
        assert_eq!(ptr::addr_of!((*p).array_dim).read_unaligned(), 1);
        assert_eq!(ptr::addr_of!((*p).offset).read_unaligned(), 0);
        assert_eq!(ptr::addr_of!((*p).cpp_property).read_unaligned(), 0);
    }
    untouched(&before, &storage.bytes(), &[
        (offset_of!(UProperty, array_dim), 4),
        (offset_of!(UProperty, offset), 4),
        (offset_of!(UProperty, cpp_property), 4),
    ]);

    // A separate CPP path must leave even Offset/CppProperty uncommitted until
    // the typed Category constructor has completed. Flags precede that leaf.
    let mut cpp = Storage::<T>::new();
    let p = cpp.as_ptr().cast::<UProperty>();
    let before = cpp.bytes();
    unsafe { construction::initialize_cpp_prefix_data(p, 0x8040_0020) };
    unsafe {
        assert_eq!(ptr::addr_of!((*p).array_dim).read_unaligned(), 1);
        assert_eq!(ptr::addr_of!((*p).property_flags).read_unaligned(), 0x8040_0020);
    }
    untouched(&before, &cpp.bytes(), &[
        (offset_of!(UProperty, array_dim), 4),
        (offset_of!(UProperty, property_flags), 4),
    ]);
}

#[test]
fn default_and_cpp_prefixes_preserve_unowned_storage_in_every_subtype() {
    base_prefixes::<UProperty>();
    base_prefixes::<UByteProperty>();
    base_prefixes::<UBoolProperty>();
    base_prefixes::<UObjectProperty>();
    base_prefixes::<UClassProperty>();
    base_prefixes::<UStructProperty>();
}

#[test]
fn subtype_initializers_preserve_bases_links_padding_and_alias_inputs() {
    unsafe {
        let mut byte = Storage::<UByteProperty>::new();
        let p = byte.as_ptr();
        for value in [p.cast(), ptr::null_mut()] {
            let before = byte.bytes();
            construction::initialize_byte_data(p, value);
            assert_eq!(ptr::addr_of!((*p).r#enum).read_unaligned(), value);
            untouched(&before, &byte.bytes(), &[(offset_of!(UByteProperty, r#enum), size_of::<Object>())]);
        }

        let mut boolean = Storage::<UBoolProperty>::new();
        let p = boolean.as_ptr();
        let before = boolean.bytes();
        construction::initialize_bool_data(p);
        assert_eq!(ptr::addr_of!((*p).bit_mask).read_unaligned(), 1);
        untouched(&before, &boolean.bytes(), &[(offset_of!(UBoolProperty, bit_mask), 4)]);

        let mut object = Storage::<UObjectProperty>::new();
        let p = object.as_ptr();
        let before = object.bytes();
        construction::initialize_object_data(p, p.cast());
        assert_eq!(ptr::addr_of!((*p).property_class).read_unaligned(), p.cast());
        untouched(&before, &object.bytes(), &[(offset_of!(UObjectProperty, property_class), size_of::<Object>())]);

        let mut class = Storage::<UClassProperty>::new();
        let p = class.as_ptr();
        // The base class input and metaclass input are independent, including
        // when the latter aliases the property's storage.
        construction::initialize_object_data(p.cast(), object.as_ptr().cast());
        let before = class.bytes();
        construction::initialize_class_data(p, p.cast());
        assert_eq!(ptr::addr_of!((*p).meta_class).read_unaligned(), p.cast());
        assert_eq!(ptr::addr_of!((*p.cast::<UObjectProperty>()).property_class).read_unaligned(), object.as_ptr().cast());
        untouched(&before, &class.bytes(), &[(offset_of!(UClassProperty, meta_class), size_of::<Object>())]);

        let mut structure = Storage::<UStructProperty>::new();
        let p = structure.as_ptr();
        for value in [p.cast(), ptr::null_mut()] {
            let before = structure.bytes();
            construction::initialize_struct_data(p, value);
            assert_eq!(ptr::addr_of!((*p).r#struct).read_unaligned(), value);
            untouched(&before, &structure.bytes(), &[(offset_of!(UStructProperty, r#struct), size_of::<Object>())]);
        }
    }
}

#[test]
fn publication_captures_old_head_before_replacing_it_and_preserves_other_bytes() {
    unsafe {
        let mut owner = Storage::<UStruct>::new();
        let mut first = Storage::<UProperty>::new();
        let mut second = Storage::<UProperty>::new();
        let o = owner.as_ptr();
        let a = first.as_ptr().cast::<UField>();
        let b = second.as_ptr().cast::<UField>();
        ptr::addr_of_mut!((*o).children).write_unaligned(ptr::null_mut());
        let owner_before = owner.bytes();
        let first_before = first.bytes();
        construction::add_cpp_property(o, a);
        assert_eq!(ptr::addr_of!((*o).children).read_unaligned(), a.cast());
        assert!(ptr::addr_of!((*a).next).read_unaligned().is_null());
        untouched(&owner_before, &owner.bytes(), &[(offset_of!(UStruct, children), size_of::<Object>())]);
        untouched(&first_before, &first.bytes(), &[(offset_of!(UField, next), size_of::<Object>())]);
        let first_published = first.bytes();
        let second_before = second.bytes();
        construction::add_cpp_property(o, b);
        assert_eq!(ptr::addr_of!((*o).children).read_unaligned(), b.cast());
        assert_eq!(ptr::addr_of!((*b).next).read_unaligned(), a.cast());
        assert_eq!(first.bytes(), first_published);
        untouched(&second_before, &second.bytes(), &[(offset_of!(UField, next), size_of::<Object>())]);
        // No deduplication/special casing: repeating the same input publishes
        // the self-link that the original two native stores would produce.
        construction::add_cpp_property(o, b);
        assert_eq!(ptr::addr_of!((*b).next).read_unaligned(), b.cast());
        assert_eq!(ptr::addr_of!((*o).children).read_unaligned(), b.cast());
    }
}

#[derive(Debug, PartialEq)]
enum Event {
    Add(Object, Vec<u8>),
    Key(Object, Object, i32),
    Capacity,
}
struct Host {
    events: Vec<Event>,
    replacement: Object,
    mutate: bool,
    add_status: i32,
    key_status: i32,
    capacity_status: i32,
    nested: *mut UProperty,
    nested_result: Option<Result<(), i32>>,
    services: Hp2PropertyConstructionServices,
}
thread_local! { static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) }; }
struct Active(*mut Host);
impl Drop for Active {
    fn drop(&mut self) { ACTIVE.with(|active| active.set(self.0)); }
}
const SERVICES: Hp2PropertyConstructionServices = Hp2PropertyConstructionServices {
    add_cpp_property: add,
    registration_key: key,
    capacity_assert: capacity,
};
unsafe fn property_name(p: Object) -> i32 {
    unsafe { i32::from_ne_bytes(ptr::addr_of!((*p.cast::<UObject>()).name).read_unaligned()) }
}
unsafe fn set_identity(p: *mut UProperty, owner: Object, name: i32) {
    unsafe {
        ptr::addr_of_mut!((*p.cast::<UObject>()).outer).write_unaligned(owner);
        ptr::addr_of_mut!((*p.cast::<UObject>()).name).write_unaligned(name.to_ne_bytes());
    }
}
unsafe extern "C" fn poison_key(_: Object, _: *mut Object, _: *mut i32) -> i32 { 819 }
unsafe extern "C" fn add(p: Object) -> i32 {
    let h = ACTIVE.with(Cell::get);
    unsafe {
        (*h).events.push(Event::Add(p, bytes(p.cast::<UProperty>())));
        let owner = ptr::addr_of!((*p.cast::<UObject>()).outer).read_unaligned();
        construction::add_cpp_property(owner.cast(), p.cast());
        if (*h).mutate {
            ptr::addr_of_mut!((*p.cast::<UProperty>()).offset).write_unaligned(-777);
            set_identity(p.cast(), (*h).replacement, property_name(p) + 1000);
        }
        // Changing caller-owned table storage must not affect this invocation.
        (*h).services.registration_key = poison_key;
        let nested = (*h).nested;
        (*h).nested = ptr::null_mut();
        if !nested.is_null() {
            let result = construction::finish_cpp_construction(nested, -91, SERVICES);
            (*h).nested_result = Some(result);
        }
        (*h).add_status
    }
}
unsafe extern "C" fn key(p: Object, owner: *mut Object, name: *mut i32) -> i32 {
    let h = ACTIVE.with(Cell::get);
    unsafe {
        let current_owner = ptr::addr_of!((*p.cast::<UObject>()).outer).read_unaligned();
        let current_name = property_name(p);
        (*h).events.push(Event::Key(p, current_owner, current_name));
        owner.write(current_owner);
        name.write(current_name);
        (*h).key_status
    }
}
unsafe extern "C" fn capacity() -> i32 {
    let h = ACTIVE.with(Cell::get);
    unsafe {
        (*h).events.push(Event::Capacity);
        (*h).capacity_status
    }
}

// Observe registration through its actual archive consumer, not a registry
// inspection/reset API. This archive supplies no-op wire leaves deliberately:
// only the real raw property's identity and offset publication are relevant.
unsafe extern "C" fn integer(p: Object, _: Object, field: Hp2NativePropertyInteger, out: *mut i32) -> i32 {
    unsafe {
        let property = p.cast::<UProperty>();
        out.write(match field {
            Hp2NativePropertyInteger::Offset => ptr::addr_of!((*property).offset).read_unaligned(),
            Hp2NativePropertyInteger::CppProperty => ptr::addr_of!((*property).cpp_property).read_unaligned(),
            Hp2NativePropertyInteger::Flags => 0,
            Hp2NativePropertyInteger::Loading => 1,
            Hp2NativePropertyInteger::Version => 100,
            Hp2NativePropertyInteger::Name => property_name(p),
        });
    }
    0
}
unsafe extern "C" fn owner(p: Object, out: *mut Object) -> i32 {
    unsafe { out.write(ptr::addr_of!((*p.cast::<UObject>()).outer).read_unaligned()) };
    0
}
unsafe extern "C" fn archive(_: Object, _: Object, _: Hp2NativePropertyWire) -> i32 { 0 }
unsafe extern "C" fn flags(_: Object, _: u32) -> i32 { 0 }
unsafe extern "C" fn publish(p: Object, offset: i32, cpp: i32) -> i32 {
    unsafe {
        ptr::addr_of_mut!((*p.cast::<UProperty>()).offset).write_unaligned(offset);
        ptr::addr_of_mut!((*p.cast::<UProperty>()).cpp_property).write_unaligned(cpp);
    }
    0
}
unsafe fn registered_offset(p: *mut UProperty) -> (i32, i32) {
    unsafe {
        // Eliminate saved-CPP-offset fallback so absence cannot look registered.
        ptr::addr_of_mut!((*p).offset).write_unaligned(456);
        ptr::addr_of_mut!((*p).cpp_property).write_unaligned(0);
        let before = bytes(p);
        let services = Hp2NativePropertyServices { integer, owner, archive, set_flags: flags, publish };
        assert_eq!(construction::serialize(p.cast(), ptr::null_mut(), &services), Ok(()));
        untouched(&before, &bytes(p), &[
            (offset_of!(UProperty, offset), 4), (offset_of!(UProperty, cpp_property), 4),
        ]);
        (ptr::addr_of!((*p).offset).read_unaligned(), ptr::addr_of!((*p).cpp_property).read_unaligned())
    }
}

// This is the only registry-mutating test in this integration-test executable.
// The existing registry unit test runs in a different process, with separate
// statics. No reset/test-only API or lock shared with production is needed.
#[test]
fn checked_finish_preserves_prefixes_reentry_original_offsets_and_capacity_order() {
    let mut owner_storage = Storage::<UStruct>::new();
    let mut replacement_storage = Storage::<UStruct>::new();
    let owner = owner_storage.as_ptr();
    let replacement = replacement_storage.as_ptr();
    let mut host = Box::new(Host {
        events: vec![], replacement: replacement.cast(), mutate: true,
        add_status: 0, key_status: 0, capacity_status: -93,
        nested: ptr::null_mut(), nested_result: None, services: SERVICES,
    });
    let _active = Active(ACTIVE.with(|active| active.replace(&mut *host)));
    unsafe {
        ptr::addr_of_mut!((*owner).children).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*replacement).children).write_unaligned(ptr::null_mut());

        // A callback changes the live offset and identity, but registration
        // uses the original offset with the freshly obtained owner/name.
        let mut property = Storage::<UProperty>::new();
        let p = property.as_ptr();
        set_identity(p, owner.cast(), 11);
        construction::initialize_cpp_prefix_data(p, 0x8040_0020);
        ptr::addr_of_mut!((*p).category).write_unaligned([1, 2, 3, 4]);
        let before = property.bytes();
        assert_eq!(construction::finish_cpp_construction(p, -37, host.services), Ok(()));
        assert_eq!(host.events.len(), 2);
        let Event::Add(seen, prefix) = &host.events[0] else { panic!("missing publication leaf") };
        assert_eq!(*seen, p.cast());
        untouched(&before, prefix, &[
            (offset_of!(UProperty, offset), 4), (offset_of!(UProperty, cpp_property), 4),
        ]);
        assert_eq!(&prefix[offset_of!(UProperty, offset)..offset_of!(UProperty, offset) + 4], &(-37i32).to_ne_bytes());
        assert_eq!(&prefix[offset_of!(UProperty, cpp_property)..offset_of!(UProperty, cpp_property) + 4], &1i32.to_ne_bytes());
        assert_eq!(host.events[1], Event::Key(p.cast(), replacement.cast(), 1011));
        assert_eq!(ptr::addr_of!((*p).offset).read_unaligned(), -777);
        assert_eq!(ptr::addr_of!((*owner).children).read_unaligned(), p.cast());
        untouched(&before, &property.bytes(), &[
            (offset_of!(UProperty, offset), 4), (offset_of!(UProperty, cpp_property), 4),
            (offset_of!(UField, next), size_of::<Object>()),
            (offset_of!(UObject, outer), size_of::<Object>()), (offset_of!(UObject, name), 4),
        ]);
        assert_eq!(registered_offset(p), (-37, 1));
        // A second construction at the same key is append-only/first-match.
        host.mutate = false;
        host.events.clear();
        assert_eq!(construction::finish_cpp_construction(p, -88, SERVICES), Ok(()));
        assert_eq!(registered_offset(p), (-37, 1));

        // Reentrant registration under the same key commits before the outer
        // entry. No borrowed property/registry/table survives the callback.
        let mut nested = Storage::<UProperty>::new();
        let n = nested.as_ptr();
        set_identity(p, owner.cast(), 12);
        set_identity(n, owner.cast(), 12);
        host.nested = n;
        host.events.clear();
        host.services = SERVICES;
        assert_eq!(construction::finish_cpp_construction(p, -52, host.services), Ok(()));
        assert_eq!(host.nested_result, Some(Ok(())));
        assert!(matches!(&host.events[..], [Event::Add(a, _), Event::Add(b, _), Event::Key(c, _, 12), Event::Key(d, _, 12)] if *a == p.cast() && *b == n.cast() && *c == n.cast() && *d == p.cast()));
        assert_eq!(ptr::addr_of!((*owner).children).read_unaligned(), n.cast());
        assert_eq!(ptr::addr_of!((*n.cast::<UField>()).next).read_unaligned(), p.cast());
        assert_eq!(registered_offset(p), (-91, 1));
        assert_eq!(registered_offset(n), (-91, 1));

        // First error stops the pipeline, without undoing either the finish
        // prefix or the publication leaf's committed mutations.
        for (name, add_status, key_status) in [(21, -41, 73), (22, 0, 53)] {
            host.events.clear();
            host.mutate = true;
            host.add_status = add_status;
            host.key_status = key_status;
            set_identity(p, owner.cast(), name);
            let before = property.bytes();
            let expected = if add_status != 0 { add_status } else { key_status };
            assert_eq!(construction::finish_cpp_construction(p, -61, SERVICES), Err(expected));
            assert_eq!(ptr::addr_of!((*p).offset).read_unaligned(), -777);
            assert_eq!(ptr::addr_of!((*p).cpp_property).read_unaligned(), 1);
            assert_eq!(ptr::addr_of!((*owner).children).read_unaligned(), p.cast());
            assert_eq!(host.events.len(), if add_status != 0 { 1 } else { 2 });
            untouched(&before, &property.bytes(), &[
                (offset_of!(UProperty, offset), 4), (offset_of!(UProperty, cpp_property), 4),
                (offset_of!(UField, next), size_of::<Object>()),
                (offset_of!(UObject, outer), size_of::<Object>()), (offset_of!(UObject, name), 4),
            ]);
            assert_eq!(registered_offset(p), (0, 0));
        }

        // Capacity is deliberately exhausted last. No test depends on resetting
        // this pre-appInit append-only registry or running in a particular order.
        let mut appended = 0;
        while construction::register(owner.cast(), -3000, 19) {
            appended += 1;
            assert!(appended <= 16384, "registry did not bound its storage");
        }
        host.mutate = false;
        for (name, add_status, key_status, capacity_status, expected, count) in [
            (31, -71, 72, -93, Err(-71), 1),
            (32, 0, 72, -93, Err(72), 2),
            (33, 0, 0, -93, Err(-93), 3),
            (34, 0, 0, 0, Ok(()), 3),
        ] {
            host.events.clear();
            host.add_status = add_status;
            host.key_status = key_status;
            host.capacity_status = capacity_status;
            set_identity(p, owner.cast(), name);
            assert_eq!(construction::finish_cpp_construction(p, -107, SERVICES), expected);
            assert_eq!(host.events.len(), count);
            assert!(matches!(host.events.first(), Some(Event::Add(_, _))));
            if count >= 2 { assert_eq!(host.events[1], Event::Key(p.cast(), owner.cast(), name)); }
            if count == 3 { assert_eq!(host.events[2], Event::Capacity); }
            assert_eq!(ptr::addr_of!((*owner).children).read_unaligned(), p.cast());
            assert_eq!(ptr::addr_of!((*p).offset).read_unaligned(), -107);
            assert_eq!(ptr::addr_of!((*p).cpp_property).read_unaligned(), 1);
            assert_eq!(registered_offset(p), (0, 0));
        }
    }
}
