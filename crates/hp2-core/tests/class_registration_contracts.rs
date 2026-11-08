//! Registration consumer contracts over initialized raw Rust ABI fields; these
//! are not live C++ objects. Native construction is exercised by the SDK oracle.
use hp2_abi::engine::generated::{TArrayRaw, UClass};
use hp2_core::{array_storage::Hp2ArrayServices, class_metadata::{self as metadata, Hp2ClassRegistrationServices, Hp2ClassStaticConstructor}};
use std::{ffi::c_void, mem::{offset_of, size_of}, ptr};
type P = *mut c_void;

#[derive(Clone, Copy)]
#[repr(C)]
struct Defaults { owner: P, value: u32 }
#[repr(C)]
struct Class {
    raw: UClass,
    parent: P,
    constructor: Hp2ClassStaticConstructor,
    getter_constructor: Option<Hp2ClassStaticConstructor>,
    constructor_parent: Option<P>,
    constructor_parent_flags: u32,
    flags: u32,
    registered: bool,
    config_name: u32,
    defaults: Box<Defaults>,
    linked_value: Option<u32>,
    errors: [i32; 10],
    constructed: usize,
    allocated: usize,
}
struct Fixture { classes: Vec<Box<Class>> }
impl Fixture {
    fn new() -> Self { Self { classes: vec![] } }
    fn add(&mut self, parent: P, constructor: Hp2ClassStaticConstructor) -> P {
        let mut raw = unsafe { std::mem::zeroed::<UClass>() };
        raw.properties_size = size_of::<Defaults>() as i32;
        let mut class = Box::new(Class {
            raw, parent, constructor, getter_constructor: None, constructor_parent: None,
            constructor_parent_flags: 0, flags: 0, registered: false, config_name: 0,
            defaults: Box::new(Defaults { owner: ptr::null_mut(), value: 0 }),
            linked_value: None, errors: [0; 10], constructed: 0, allocated: 0,
        });
        let p = (&mut *class as *mut Class).cast();
        class.defaults.owner = p;
        self.classes.push(class);
        p
    }
}
unsafe fn class(p: P) -> &'static mut Class { unsafe { &mut *p.cast::<Class>() } }
unsafe extern "C" fn realloc(_: P, _: P, size: usize, max: *const i32, width: i32, _: *const c_void, out: *mut P) -> i32 {
    unsafe {
        let p = max.cast::<u8>().sub(offset_of!(UClass, defaults) + offset_of!(TArrayRaw, max)).cast_mut().cast();
        assert_eq!(width, 1);
        assert_eq!(size, size_of::<Defaults>());
        assert_eq!(class(p).raw.defaults.num, 0);
        class(p).allocated += 1;
        let error = class(p).errors[2];
        if error != 0 { return error; }
        // A real Rust allocation backs the raw owner fixture; it is never sent
        // to C++, and no native allocator or C++ object is fabricated here.
        class(p).defaults = Box::new(*class(p).defaults);
        out.write((&mut *class(p).defaults as *mut Defaults).cast());
        0
    }
}
unsafe extern "C" fn checked_int_size(_: P, size: usize, out: *mut i32) -> i32 { unsafe { out.write(size as i32); } 0 }
unsafe extern "C" fn free(_: P, _: P) -> i32 { 0 }
const ARRAYS: Hp2ArrayServices = Hp2ArrayServices { context: ptr::null_mut(), realloc, checked_int_size, free };
unsafe extern "C" fn array_allocator() -> *const Hp2ArrayServices { &ARRAYS }
unsafe extern "C" fn super_register(p: P) -> i32 { unsafe { class(p).registered = true; class(p).errors[0] } }
unsafe extern "C" fn restore_config_name(p: P) -> i32 { unsafe { class(p).config_name = 17; class(p).errors[1] } }
unsafe extern "C" fn initialize_default(p: P) -> i32 {
    unsafe {
        assert_eq!(class(p).raw.defaults.num, size_of::<Defaults>() as i32);
        assert_eq!(class(p).raw.defaults.data, (&mut *class(p).defaults as *mut Defaults).cast());
        class(p).defaults.value = 1;
        class(p).errors[3]
    }
}
unsafe extern "C" fn superclass(p: P) -> P { unsafe { class(p).parent } }
unsafe extern "C" fn static_constructor(p: P) -> Hp2ClassStaticConstructor { unsafe { class(p).constructor } }
unsafe extern "C" fn default_object(p: P, out: *mut P) -> i32 {
    unsafe {
        if let Some(constructor) = class(p).getter_constructor { class(p).constructor = constructor; }
        out.write(class(p).raw.defaults.data);
        class(p).errors[4]
    }
}
unsafe extern "C" fn base_constructor(default: P) -> i32 {
    unsafe {
        let d = default.cast::<Defaults>();
        (*d).value += 3;
        class((*d).owner).constructed += 1;
        class((*d).owner).errors[5]
    }
}
unsafe extern "C" fn override_constructor(default: P) -> i32 {
    unsafe {
        let d = default.cast::<Defaults>();
        (*d).value += 7;
        let p = (*d).owner;
        class(p).constructed += 1;
        if let Some(parent) = class(p).constructor_parent {
            class(p).parent = parent;
            if !parent.is_null() { class(parent).flags = class(p).constructor_parent_flags; }
        }
        class(p).errors[5]
    }
}
unsafe extern "C" fn class_flags(p: P) -> u32 { unsafe { class(p).flags } }
unsafe extern "C" fn add_class_flags(p: P, flags: u32) -> i32 { unsafe { class(p).flags |= flags; class(p).errors[6] } }
unsafe extern "C" fn link_defaults(p: P) -> i32 { unsafe { class(p).linked_value = Some(class(p).defaults.value); class(p).errors[7] } }
unsafe extern "C" fn load_config(p: P) -> i32 { unsafe { class(p).defaults.value += 10; class(p).errors[8] } }
unsafe extern "C" fn load_localized(p: P) -> i32 { unsafe { class(p).defaults.value += 20; class(p).errors[9] } }
const S: Hp2ClassRegistrationServices = Hp2ClassRegistrationServices {
    super_register, restore_config_name, array_allocator, initialize_default,
    superclass, static_constructor, default_object, class_flags, add_class_flags,
    link_defaults, load_config, load_localized,
};

#[test]
fn root_and_override_construct_but_inherited_handler_is_suppressed() {
    let mut f = Fixture::new();
    let root = f.add(ptr::null_mut(), base_constructor);
    let inherited = f.add(root, base_constructor);
    let overridden = f.add(root, override_constructor);
    unsafe {
        class(root).flags = 0x31;
        class(inherited).flags = 0x80;
        class(overridden).flags = 0x80;
        for p in [root, inherited, overridden] { metadata::register_class(p, 0xf, &S).unwrap(); }
        assert_eq!(class(root).defaults.value, 34);
        assert_eq!(class(root).linked_value, Some(4));
        assert_eq!(class(inherited).defaults.value, 31);
        assert_eq!(class(inherited).linked_value, Some(1));
        assert_eq!(class(inherited).constructed, 0);
        assert_eq!(class(overridden).defaults.value, 38);
        assert_eq!(class(overridden).linked_value, Some(8));
        assert_eq!(class(root).flags, 0x31);
        assert_eq!(class(inherited).flags, 0x81);
        assert_eq!(class(overridden).flags, 0x81);
    }
}

#[test]
fn checked_default_getter_replaces_handler_before_direct_dispatch() {
    let mut f = Fixture::new();
    let root = f.add(ptr::null_mut(), base_constructor);
    unsafe {
        class(root).getter_constructor = Some(override_constructor);
        metadata::register_class(root, 0xf, &S).unwrap();
        assert_eq!(class(root).linked_value, Some(8));
        assert_eq!(class(root).defaults.value, 38);
        assert_eq!(class(root).constructed, 1);
    }
}

#[test]
fn constructor_reentry_changes_live_parent_and_flags_before_inheritance() {
    for (initial_parent, final_parent) in [(true, true), (true, false), (false, true)] {
        let mut f = Fixture::new();
        let old = f.add(ptr::null_mut(), override_constructor);
        let new = f.add(ptr::null_mut(), base_constructor);
        let p = f.add(if initial_parent { old } else { ptr::null_mut() }, base_constructor);
        unsafe {
            class(old).flags = 1;
            class(new).flags = 4;
            class(p).flags = 0x80;
            class(p).getter_constructor = Some(override_constructor);
            class(p).constructor_parent = Some(if final_parent { new } else { ptr::null_mut() });
            class(p).constructor_parent_flags = 0x32;
            metadata::register_class(p, 0xf, &S).unwrap();
            assert_eq!(class(p).linked_value, Some(8));
            assert_eq!(class(p).defaults.value, 38);
            assert_eq!(class(p).flags, if final_parent { 0x82 } else { 0x80 });
            assert_eq!(class(old).flags, 1);
            assert_eq!(class(new).flags, if final_parent { 0x32 } else { 4 });
        }
    }
}

#[test]
fn first_error_preserves_partial_state_and_registration_recovers() {
    for failed in 0..10 {
        let mut f = Fixture::new();
        let parent = f.add(ptr::null_mut(), base_constructor);
        let p = f.add(parent, override_constructor);
        unsafe {
            class(parent).flags = 0x31;
            class(p).flags = 0x80;
            for later in failed..10 { class(p).errors[later] = 101 + later as i32; }
            assert_eq!(metadata::register_class(p, 0xf, &S), Err(101 + failed as i32));
            assert!(class(p).registered);
            assert_eq!(class(p).config_name, if failed >= 1 { 17 } else { 0 });
            assert_eq!(class(p).raw.defaults.max, if failed >= 2 { size_of::<Defaults>() as i32 } else { 0 });
            assert_eq!(class(p).raw.defaults.num, if failed >= 3 { size_of::<Defaults>() as i32 } else { 0 });
            assert_eq!(class(p).raw.defaults.data.is_null(), failed <= 2);
            let expected = match failed { 0..=2 => 0, 3..=4 => 1, 5..=7 => 8, 8 => 18, _ => 38 };
            assert_eq!(class(p).defaults.value, expected);
            assert_eq!(class(p).flags, if failed >= 6 { 0x81 } else { 0x80 });
            assert_eq!(class(p).linked_value, if failed >= 7 { Some(8) } else { None });
            assert_eq!(class(parent).allocated, 0);
            class(p).errors = [0; 10];
            metadata::register_class(p, 0xf, &S).unwrap();
            assert_eq!(class(p).defaults.value, 38);
            assert_eq!(class(p).linked_value, Some(8));
            let healthy = f.add(parent, override_constructor);
            metadata::register_class(healthy, 0xf, &S).unwrap();
            assert_eq!(class(healthy).defaults.value, 38);
        }
    }
}
