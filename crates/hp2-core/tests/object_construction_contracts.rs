use hp2_abi::engine::generated::{UBoolProperty, UClass, UObject, UProperty, UStruct};
use hp2_core::object_construction::*;
use std::ffi::c_void;
use std::ptr::null_mut;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    ErrorCheck,
    Validation(i32),
    Unique,
    Conflict(i32),
    Tag(bool),
    Enter,
    Leave,
    Find,
    Allocate(i32),
    Replacing,
    NoReplace,
    Destroy,
    Reserve(i32),
    Init,
    Add(i32),
    Valid,
    Config,
    Localized,
    ConstructEnter,
    ConstructLeave,
    Constructor(u32),
}

// Every native field lives in the actual generated mirror, rather than a
// callback-only fixture representation. UClass supplies enough aligned storage
// for all views used by these allocation/replacement contracts.
#[repr(C)]
struct Object {
    abi: UClass,
    callback_context: *mut c_void,
    constructor_effect: u32,
    kind: u32,
}
impl Object {
    fn new() -> Box<Self> {
        // SAFETY: Generated ABI mirrors contain only scalars, raw pointers and
        // byte padding; all-zero storage is valid before fixture initialization.
        let mut object = Box::new(Self {
            abi: unsafe { std::mem::zeroed() },
            callback_context: null_mut(),
            constructor_effect: 0,
            kind: 0,
        });
        object.set_index(3);
        object.set_constructor(Some(checked_constructor::<11>));
        object.set_size(160);
        object.set_offset(24);
        object.set_cpp(1);
        object.set_mask(0x80000000);
        object.set_name(99);
        object
    }
    fn header(&self) -> &UObject {
        // SAFETY: Object is repr(C), and its first field is the generated UClass
        // storage with the compiler-probed UObject prefix.
        unsafe { &*(&raw const self.abi).cast::<UObject>() }
    }
    fn header_mut(&mut self) -> &mut UObject {
        // SAFETY: The exclusive fixture borrow covers the shared UObject prefix.
        unsafe { &mut *(&raw mut self.abi).cast::<UObject>() }
    }
    fn property(&self) -> &UProperty {
        // SAFETY: Generated UClass storage is larger than and aligned for this
        // generated view. No mirror reference survives a fixture mutation.
        unsafe { &*(&raw const self.abi).cast::<UProperty>() }
    }
    fn property_mut(&mut self) -> &mut UProperty {
        // SAFETY: As above, under an exclusive fixture borrow.
        unsafe { &mut *(&raw mut self.abi).cast::<UProperty>() }
    }
    fn index(&self) -> i32 {
        self.header().index
    }
    fn set_index(&mut self, value: i32) {
        self.header_mut().index = value;
    }
    fn flags(&self) -> u32 {
        self.header().object_flags
    }
    fn set_flags(&mut self, value: u32) {
        self.header_mut().object_flags = value;
    }
    fn class_flags(&self) -> u32 {
        self.abi.class_flags
    }
    fn set_class_flags(&mut self, value: u32) {
        self.abi.class_flags = value;
    }
    fn within(&self) -> *mut c_void {
        self.abi.class_within
    }
    fn set_within(&mut self, value: *mut c_void) {
        self.abi.class_within = value;
    }
    fn constructor(&self) -> Option<Hp2ClassConstructor> {
        if self.abi.class_constructor.is_null() {
            None
        } else {
            // SAFETY: Only set_constructor writes this field, using this exact
            // checked function-pointer signature.
            Some(unsafe {
                std::mem::transmute::<*mut c_void, Hp2ClassConstructor>(self.abi.class_constructor)
            })
        }
    }
    fn set_constructor(&mut self, value: Option<Hp2ClassConstructor>) {
        self.abi.class_constructor = value.map_or(null_mut(), |f| f as *mut c_void);
    }
    fn size(&self) -> i32 {
        self.abi.properties_size
    }
    fn set_size(&mut self, value: i32) {
        self.abi.properties_size = value;
    }
    fn align(&self) -> i32 {
        // SAFETY: This is the generated UStruct base view of UClass storage.
        unsafe { (*(&raw const self.abi).cast::<UStruct>()).properties_align }
    }
    fn set_align(&mut self, value: i32) {
        // SAFETY: This is the generated UStruct base view under exclusive access.
        unsafe {
            (*(&raw mut self.abi).cast::<UStruct>()).properties_align = value;
        }
    }
    fn offset(&self) -> i32 {
        self.property().offset
    }
    fn set_offset(&mut self, value: i32) {
        self.property_mut().offset = value;
    }
    fn cpp(&self) -> i32 {
        self.property().cpp_property
    }
    fn set_cpp(&mut self, value: i32) {
        self.property_mut().cpp_property = value;
    }
    fn mask(&self) -> u32 {
        // SAFETY: The generated bool-property view fits this fixture storage.
        unsafe { (*(&raw const self.abi).cast::<UBoolProperty>()).bit_mask }
    }
    fn set_mask(&mut self, value: u32) {
        // SAFETY: The generated bool-property view fits, under exclusive access.
        unsafe {
            (*(&raw mut self.abi).cast::<UBoolProperty>()).bit_mask = value;
        }
    }
    fn name(&self) -> i32 {
        i32::from_ne_bytes(self.header().name)
    }
    fn set_name(&mut self, value: i32) {
        self.header_mut().name = value.to_ne_bytes();
    }
    fn set_next(&mut self, value: *mut c_void) {
        self.header_mut().hash_next = value;
    }
    fn set_class(&mut self, value: *mut c_void) {
        self.header_mut().class = value;
    }
    fn outer(&self) -> *mut c_void {
        self.header().outer
    }
    fn set_outer(&mut self, value: *mut c_void) {
        self.header_mut().outer = value;
    }
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
}

struct Context {
    class: *mut c_void,
    outer: *mut c_void,
    object: *mut c_void,
    name: i32,
    registering: i32,
    package: *mut c_void,
    head: *mut c_void,
    found: *mut c_void,
    events: Vec<Event>,
    fail: Option<Event>,
    error_present: bool,
    depth: i32,
    construct_depth: i32,
    conflicts: bool,
    redirect_conflict: *mut c_void,
    replace_flags: Option<u32>,
    destructor_class_flags: Option<u32>,
    add_class_flags: Option<u32>,
    replace_property_offset: Option<i32>,
    add_constructor: Option<Hp2ClassConstructor>,
    init_observation: Option<(i32, u32, i32, *mut c_void, *mut c_void)>,
    config_observation: Option<(i32, i32, Option<Hp2ClassConstructor>)>,
    localized_observation: Option<(i32, Option<Hp2ClassConstructor>)>,
    reenter_init: bool,
    reenter_constructor: bool,
    nested: Option<Result<*mut c_void, i32>>,
    reenter_class: *mut c_void,
    reenter_object: *mut c_void,
    ctor_seen_published: bool,
}
impl Context {
    fn new(class: *mut c_void, outer: *mut c_void, object: *mut c_void) -> Self {
        Self {
            class,
            outer,
            object,
            name: 7,
            registering: 0,
            package: null_mut(),
            head: null_mut(),
            found: null_mut(),
            events: vec![],
            fail: None,
            error_present: true,
            depth: 0,
            construct_depth: 0,
            conflicts: false,
            redirect_conflict: null_mut(),
            replace_flags: None,
            destructor_class_flags: None,
            add_class_flags: None,
            replace_property_offset: None,
            add_constructor: None,
            init_observation: None,
            config_observation: None,
            localized_observation: None,
            reenter_init: false,
            reenter_constructor: false,
            nested: None,
            reenter_class: null_mut(),
            reenter_object: null_mut(),
            ctor_seen_published: false,
        }
    }
    fn ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
    fn request(&self) -> Hp2ObjectAllocationRequest {
        Hp2ObjectAllocationRequest {
            in_class: self.class,
            outer: self.outer,
            in_template: null_mut(),
            supplied: null_mut(),
            flags: 4,
        }
    }
    fn hit(&mut self, event: Event) -> i32 {
        let failed = self.fail.as_ref() == Some(&event);
        self.events.push(event);
        if failed { 71 } else { 0 }
    }
}
unsafe fn ctx<'a>(p: *mut c_void) -> &'a mut Context {
    unsafe { &mut *p.cast::<Context>() }
}
unsafe fn obj<'a>(p: *mut c_void) -> &'a mut Object {
    unsafe { &mut *p.cast::<Object>() }
}
unsafe extern "C" fn assert_error(c: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::ErrorCheck);
        if s != 0 {
            s
        } else if c.error_present {
            0
        } else {
            50
        }
    }
}
unsafe extern "C" fn assert_within(o: *mut c_void) -> i32 {
    unsafe {
        if !o.is_null() && obj(o).within().is_null() {
            51
        } else {
            0
        }
    }
}
unsafe extern "C" fn assert_constructor(o: *mut c_void) -> i32 {
    unsafe {
        if !o.is_null() && obj(o).constructor().is_none() {
            52
        } else {
            0
        }
    }
}
unsafe extern "C" fn register_count(c: *mut c_void) -> i32 {
    unsafe { ctx(c).registering }
}
unsafe extern "C" fn package_class(c: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = ctx(c).package;
        0
    }
}
unsafe extern "C" fn validation_error(
    c: *mut c_void,
    kind: i32,
    _: *mut c_void,
    _: *mut c_void,
) -> i32 {
    unsafe { ctx(c).hit(Event::Validation(kind)) }
}
unsafe extern "C" fn name(c: *mut c_void) -> i32 {
    unsafe { ctx(c).name }
}
unsafe extern "C" fn unique_name(c: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Unique);
        if s == 0 {
            c.name = 99;
        }
        s
    }
}
unsafe extern "C" fn check_conflicts(c: *mut c_void) -> i32 {
    unsafe { i32::from(ctx(c).conflicts) }
}
unsafe extern "C" fn hash_head(c: *mut c_void, _: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = ctx(c).head;
        0
    }
}
unsafe extern "C" fn conflict(c: *mut c_void, o: *mut c_void, _: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Conflict(obj(o).index()));
        if !c.redirect_conflict.is_null() {
            obj(o).set_next(c.redirect_conflict);
            c.redirect_conflict = null_mut();
        }
        s
    }
}
unsafe extern "C" fn select_tag(c: *mut c_void, _: *mut c_void, registering: i32) -> i32 {
    unsafe { ctx(c).hit(Event::Tag(registering != 0)) }
}
unsafe extern "C" fn with_tag(
    c: *mut c_void,
    r: *const Hp2ObjectAllocationRequest,
    s: *const Hp2ObjectAllocationServices,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let status = ctx(c).hit(Event::Enter);
        if status != 0 {
            return status;
        }
        ctx(c).depth += 1;
        let result = allocate_tagged(c, &*r, &*s);
        ctx(c).depth -= 1;
        ctx(c).hit(Event::Leave);
        match result {
            Ok(value) => {
                *out = value;
                0
            }
            Err(status) => status,
        }
    }
}
unsafe extern "C" fn find(
    c: *mut c_void,
    _: *mut c_void,
    _: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Find);
        if s == 0 {
            *out = c.found;
        }
        s
    }
}
unsafe extern "C" fn cast(o: *mut c_void, kind: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        *out = if !o.is_null() && obj(o).kind & (1 << kind) != 0 {
            o
        } else {
            null_mut()
        };
        0
    }
}
unsafe extern "C" fn assert_replacement(supplied: *mut c_void, o: *mut c_void) -> i32 {
    if supplied.is_null() || supplied == o {
        0
    } else {
        53
    }
}
unsafe extern "C" fn replacing(c: *mut c_void, o: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Replacing);
        if let Some(flags) = c.replace_flags {
            obj(o).set_flags(flags);
        }
        if let Some(offset) = c.replace_property_offset {
            obj(o).kind = 0;
            obj(o).set_offset(offset);
        }
        s
    }
}
unsafe extern "C" fn no_replace(c: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    unsafe {
        ctx(c).hit(Event::NoReplace);
        54
    }
}
unsafe extern "C" fn malloc(c: *mut c_void, size: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Allocate(size));
        if s == 0 {
            *out = c.object;
        }
        s
    }
}
unsafe extern "C" fn destroy(c: *mut c_void, o: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Destroy);
        if s == 0 {
            obj(o).set_index(-1);
            if let Some(flags) = c.destructor_class_flags {
                obj(c.class).set_class_flags(flags);
            }
        }
        s
    }
}
unsafe extern "C" fn reserve(c: *mut c_void, index: i32) -> i32 {
    unsafe { ctx(c).hit(Event::Reserve(index)) }
}
unsafe extern "C" fn init_properties(
    c: *mut c_void,
    o: *mut c_void,
    _: *mut c_void,
    template: *mut c_void,
) -> i32 {
    unsafe {
        let status = ctx(c).hit(Event::Init);
        if status != 0 {
            return status;
        }
        ctx(c).init_observation = Some((
            obj(o).index(),
            obj(o).flags(),
            obj(o).name(),
            obj(o).outer(),
            template,
        ));
        obj(o).set_class_flags(0x200);
        obj(o).set_constructor(Some(checked_constructor::<22>));
        obj(o).set_within(null_mut());
        obj(o).set_size(90);
        obj(o).set_align(4);
        obj(o).set_offset(0);
        obj(o).set_cpp(0);
        obj(o).set_mask(1);
        if ctx(c).reenter_init {
            let mut nested =
                Context::new(ctx(c).reenter_class, ctx(c).outer, ctx(c).reenter_object);
            nested.found = nested.object;
            let request = nested.request();
            let result = allocate(nested.ptr(), &request, &SERVICES);
            ctx(c).nested = Some(result);
        }
        0
    }
}
unsafe extern "C" fn add(c: *mut c_void, o: *mut c_void, index: i32) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Add(index));
        if s == 0 {
            obj(o).set_index(if index == -1 { 42 } else { index });
            if let Some(flags) = c.add_class_flags {
                obj(c.class).set_class_flags(flags);
            }
            if let Some(constructor) = c.add_constructor {
                obj(c.class).set_constructor(Some(constructor));
            }
        }
        s
    }
}
unsafe extern "C" fn assert_valid(c: *mut c_void, _: *mut c_void) -> i32 {
    unsafe { ctx(c).hit(Event::Valid) }
}
unsafe extern "C" fn load_config(c: *mut c_void, o: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        let s = c.hit(Event::Config);
        c.config_observation = Some((obj(o).index(), obj(o).offset(), obj(o).constructor()));
        if s == 0 {
            let flags = obj(o).class_flags() | 0x1000;
            obj(o).set_class_flags(flags);
            obj(o).set_constructor(Some(checked_constructor::<33>));
        }
        s
    }
}
unsafe extern "C" fn load_localized(c: *mut c_void, o: *mut c_void) -> i32 {
    unsafe {
        let c = ctx(c);
        c.localized_observation = Some((obj(o).offset(), obj(o).constructor()));
        c.hit(Event::Localized)
    }
}

static SERVICES: Hp2ObjectAllocationServices = Hp2ObjectAllocationServices {
    assert_error,
    assert_within,
    assert_constructor,
    register_count,
    package_class,
    validation_error,
    name,
    unique_name,
    check_conflicts,
    hash_head,
    conflict,
    select_tag,
    with_tag,
    find,
    cast,
    assert_replacement,
    replacing,
    no_replace,
    allocate: malloc,
    destroy,
    reserve,
    init_properties,
    add,
    assert_valid,
    load_config,
    load_localized,
};

unsafe extern "C" fn construct_scope(
    c: *mut c_void,
    r: *const Hp2ObjectAllocationRequest,
    s: *const Hp2ObjectConstructionServices,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        ctx(c).hit(Event::ConstructEnter);
        ctx(c).construct_depth += 1;
        let result = construct_tagged(c, &*r, &*s);
        ctx(c).construct_depth -= 1;
        ctx(c).hit(Event::ConstructLeave);
        match result {
            Ok(value) => {
                *out = value;
                0
            }
            Err(status) => status,
        }
    }
}
unsafe extern "C" fn construct_allocate(
    c: *mut c_void,
    r: *const Hp2ObjectAllocationRequest,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        match allocate(c, &*r, &SERVICES) {
            Ok(value) => {
                if !value.is_null() {
                    obj(value).callback_context = c;
                }
                *out = value;
                0
            }
            Err(status) => status,
        }
    }
}
unsafe extern "C" fn checked_constructor<const EFFECT: u32>(o: *mut c_void) -> i32 {
    unsafe {
        let c = obj(o).callback_context;
        // The allocation bridge supplies a context only for this invocation.
        obj(o).callback_context = null_mut();
        obj(o).constructor_effect = EFFECT;
        ctx(c).ctor_seen_published =
            obj(o).index() != -1 && ctx(c).depth == 0 && ctx(c).construct_depth == 1;
        let status = ctx(c).hit(Event::Constructor(EFFECT));
        if status != 0 {
            return status;
        }
        if ctx(c).reenter_constructor {
            let mut nested =
                Context::new(ctx(c).reenter_class, ctx(c).outer, ctx(c).reenter_object);
            let request = nested.request();
            let result = construct(nested.ptr(), &request, &CONSTRUCTION);
            ctx(c).nested = Some(result);
        }
        0
    }
}
static CONSTRUCTION: Hp2ObjectConstructionServices = Hp2ObjectConstructionServices {
    assert_error,
    with_tag: construct_scope,
    allocate: construct_allocate,
};

fn fixture() -> (Box<Object>, Box<Object>, Box<Object>, Context) {
    let mut class = Object::new();
    let mut outer = Object::new();
    let mut object = Object::new();
    class.set_within(outer.ptr());
    let outer_ptr = outer.ptr();
    outer.set_class(outer_ptr);
    object.set_class(class.ptr());
    object.set_outer(outer.ptr());
    object.set_index(9);
    let context = Context::new(class.ptr(), outer.ptr(), object.ptr());
    (class, outer, object, context)
}
fn run(c: &mut Context) -> Result<*mut c_void, i32> {
    let request = c.request();
    unsafe { allocate(c.ptr(), &request, &SERVICES) }
}
fn before(c: &Context, first: Event, second: Event) {
    assert!(
        c.events.iter().position(|e| e == &first).unwrap()
            < c.events.iter().position(|e| e == &second).unwrap()
    );
}

fn invoke_saved(
    constructor: Option<Hp2ClassConstructor>,
    object: &mut Object,
    context: &mut Context,
) -> i32 {
    object.callback_context = context.ptr();
    let result = unsafe { constructor.unwrap()(object.ptr()) };
    object.callback_context = null_mut();
    result
}

#[test]
fn invariant_failures_precede_nullable_validation_in_original_order() {
    let (mut class, _outer, _object, mut c) = fixture();
    c.error_present = false;
    class.set_within(null_mut());
    class.set_constructor(None);
    assert_eq!(run(&mut c), Err(50));
    c.error_present = true;
    assert_eq!(run(&mut c), Err(51));
    class.set_within(c.outer);
    assert_eq!(run(&mut c), Err(52));
    assert!(!c.events.iter().any(|e| matches!(e, Event::Validation(_))));
    c.class = null_mut();
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(0)));
}

#[test]
fn validation_precedence_and_registration_exception_are_observable() {
    let (mut class, mut outer, _object, mut c) = fixture();
    class.set_index(-1);
    class.set_class_flags(CLASS_ABSTRACT);
    outer.set_class(null_mut());
    c.name = 0;
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(1)));
    c.registering = 1;
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(2)));
    class.set_class_flags(0);
    c.outer = null_mut();
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(3)));
    c.outer = outer.ptr();
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(4)));
    assert_eq!(c.name, 0);
    assert!(!c.events.contains(&Event::Unique));
    let outer_ptr = outer.ptr();
    outer.set_class(outer_ptr);
    assert_eq!(run(&mut c), Ok(c.object));
    assert!(c.events.contains(&Event::Tag(true)));
}

#[test]
fn within_validation_reads_the_live_abi_superclass_chain() {
    let (mut class, mut outer, _object, mut c) = fixture();
    let mut base = Object::new();
    let mut derived = Object::new();
    derived.abi.super_field = base.ptr();
    class.set_within(base.ptr());
    outer.set_class(derived.ptr());
    assert_eq!(run(&mut c), Ok(c.object));
    let mut unrelated = Object::new();
    class.set_within(unrelated.ptr());
    assert_eq!(run(&mut c), Ok(null_mut()));
    assert_eq!(c.events.last(), Some(&Event::Validation(4)));
}

#[test]
fn package_without_outer_is_valid_and_negative_registration_uses_fname_tag() {
    let (mut class, _outer, _object, mut c) = fixture();
    class.set_index(-1);
    c.registering = -1;
    c.package = class.ptr();
    c.outer = null_mut();
    assert_eq!(run(&mut c), Ok(c.object));
    assert!(c.events.contains(&Event::Tag(false)));
    assert_eq!(c.init_observation.unwrap().3, null_mut());
}

#[test]
fn failed_validation_log_propagates_instead_of_returning_null() {
    let (_class, _outer, _object, mut c) = fixture();
    c.class = null_mut();
    c.fail = Some(Event::Validation(0));
    assert_eq!(run(&mut c), Err(71));
    assert!(!c.events.contains(&Event::Enter));
}

#[test]
fn unnamed_object_uses_unique_name_and_conflict_successor_is_live_after_logging() {
    let (_class, _outer, mut object, mut c) = fixture();
    let mut first = Object::new();
    let mut skipped = Object::new();
    let mut inserted = Object::new();
    first.set_index(10);
    skipped.set_index(11);
    inserted.set_index(12);
    first.set_outer(c.outer);
    skipped.set_outer(c.outer);
    inserted.set_outer(c.outer);
    first.set_next(skipped.ptr());
    c.head = first.ptr();
    c.redirect_conflict = inserted.ptr();
    c.name = 0;
    c.conflicts = true;
    assert_eq!(run(&mut c), Ok(object.ptr()));
    assert_eq!(object.name(), 99);
    assert!(c.events.contains(&Event::Conflict(10)));
    assert!(c.events.contains(&Event::Conflict(12)));
    assert!(!c.events.contains(&Event::Conflict(11)));
    before(&c, Event::Unique, Event::Conflict(10));
    before(&c, Event::Conflict(12), Event::Find);
}

#[test]
fn conflict_requires_name_outer_and_different_class() {
    let (_class, _outer, _object, mut c) = fixture();
    let mut first = Object::new();
    let mut second = Object::new();
    let mut third = Object::new();
    first.set_name(c.name + 1);
    first.set_outer(c.outer);
    second.set_name(c.name);
    second.set_outer(null_mut());
    third.set_name(c.name);
    third.set_outer(c.outer);
    third.set_class(c.class);
    first.set_next(second.ptr());
    second.set_next(third.ptr());
    c.head = first.ptr();
    c.conflicts = true;
    assert_eq!(run(&mut c), Ok(c.object));
    assert!(!c.events.iter().any(|e| matches!(e, Event::Conflict(_))));
}

#[test]
fn supplied_storage_skips_allocator_but_not_initialization_or_publication() {
    let (_class, _outer, mut object, mut c) = fixture();
    let mut request = c.request();
    request.supplied = object.ptr();
    request.in_template = c.outer;
    assert_eq!(
        unsafe { allocate(c.ptr(), &request, &SERVICES) },
        Ok(object.ptr())
    );
    assert!(!c.events.iter().any(|e| matches!(e, Event::Allocate(_))));
    assert_eq!(c.init_observation, Some((-1, 4, 7, c.outer, c.outer)));
    assert_eq!(object.index(), 42);
    before(&c, Event::Init, Event::Add(-1));
    before(&c, Event::Add(-1), Event::Valid);
}

#[test]
fn replacement_retains_address_index_live_keep_flags_and_class_snapshot() {
    let (_class, _outer, mut object, mut c) = fixture();
    c.found = object.ptr();
    object.kind = 1;
    object.set_class_flags(CLASS_ABSTRACT | CLASS_TRANSIENT | 0x40);
    object.set_within(c.outer);
    object.set_size(208);
    object.set_align(16);
    c.replace_flags = Some(RF_KEEP | 0x80004 | 0x10);
    c.destructor_class_flags = Some(CLASS_TRANSIENT);
    c.add_class_flags = Some(CLASS_PER_OBJECT_CONFIG);
    let mut request = c.request();
    request.flags = 0;
    assert_eq!(
        unsafe { allocate(c.ptr(), &request, &SERVICES) },
        Ok(object.ptr())
    );
    assert_eq!(object.index(), 9);
    assert_eq!(object.flags(), RF_KEEP | RF_TRANSIENT);
    assert_eq!(object.within(), c.outer);
    let restored_constructor = object.constructor();
    assert_eq!((object.size(), object.align()), (208, 16));
    assert_eq!(object.class_flags(), CLASS_ABSTRACT | 0x200 | 0x1000);
    let (index, offset, configured_constructor) = c.config_observation.unwrap();
    assert_eq!((index, offset), (9, 0));
    assert_eq!(invoke_saved(configured_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 22);
    let (localized_offset, localized_constructor) = c.localized_observation.unwrap();
    assert_eq!(localized_offset, 0);
    assert_eq!(invoke_saved(localized_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 33);
    assert_eq!(invoke_saved(restored_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 11);
    before(&c, Event::Destroy, Event::Reserve(9));
    before(&c, Event::Reserve(9), Event::Init);
    before(&c, Event::Valid, Event::Config);
    before(&c, Event::Config, Event::Localized);
    before(&c, Event::Localized, Event::Leave);
}

#[test]
fn bool_property_snapshot_is_restored_after_configuration() {
    let (mut class, _outer, mut object, mut c) = fixture();
    class.set_class_flags(CLASS_PER_OBJECT_CONFIG);
    c.found = object.ptr();
    object.kind = 6;
    c.replace_property_offset = Some(83);
    assert_eq!(run(&mut c), Ok(object.ptr()));
    let (index, offset, configured_constructor) = c.config_observation.unwrap();
    assert_eq!((index, offset), (9, 0));
    assert_eq!(invoke_saved(configured_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 22);
    assert_eq!(
        (object.offset(), object.cpp(), object.mask()),
        (83, 1, 0x80000000)
    );
    assert_eq!(invoke_saved(object.constructor(), &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 33);
}

#[test]
fn wrong_replacement_storage_precedes_logging_and_class_mismatch_precedes_destroy() {
    let (_class, _outer, mut object, mut c) = fixture();
    c.found = object.ptr();
    let mut request = c.request();
    request.supplied = c.outer;
    assert_eq!(unsafe { allocate(c.ptr(), &request, &SERVICES) }, Err(53));
    assert!(!c.events.contains(&Event::Replacing));
    object.set_class(null_mut());
    assert_eq!(run(&mut c), Err(54));
    assert!(!c.events.contains(&Event::Destroy));
    assert_eq!(object.index(), 9);
    before(&c, Event::Replacing, Event::NoReplace);
}

#[test]
fn destruction_and_reservation_errors_preserve_partial_state_and_close_tag() {
    for fail in [Event::Destroy, Event::Reserve(9)] {
        let (_class, _outer, mut object, mut c) = fixture();
        c.found = object.ptr();
        c.fail = Some(fail.clone());
        assert_eq!(run(&mut c), Err(71));
        assert_eq!(object.index(), if fail == Event::Destroy { 9 } else { -1 });
        assert!(!c.events.contains(&Event::Init));
        assert_eq!(c.depth, 0);
        assert_eq!(c.events.last(), Some(&Event::Leave));
        if fail == Event::Destroy {
            assert!(!c.events.contains(&Event::Reserve(9)));
        }
    }
}

#[test]
fn initialization_publication_validity_and_config_failures_stop_at_visible_boundary() {
    for fail in [
        Event::Init,
        Event::Add(9),
        Event::Valid,
        Event::Config,
        Event::Localized,
    ] {
        let (mut class, _outer, mut object, mut c) = fixture();
        class.set_class_flags(CLASS_PER_OBJECT_CONFIG);
        object.kind = 6;
        c.found = object.ptr();
        c.fail = Some(fail.clone());
        assert_eq!(run(&mut c), Err(71));
        assert_eq!(
            object.index(),
            if matches!(fail, Event::Init | Event::Add(_)) {
                -1
            } else {
                9
            }
        );
        if fail != Event::Init {
            assert_eq!((object.offset(), object.cpp(), object.mask()), (0, 0, 1));
        }
        if matches!(fail, Event::Init | Event::Add(_) | Event::Valid) {
            assert!(!c.events.contains(&Event::Config));
        }
        if fail == Event::Config {
            assert!(!c.events.contains(&Event::Localized));
        }
        assert_eq!(c.depth, 0);
    }
}

#[test]
fn initialization_can_reenter_without_overwriting_outer_replacement_snapshot() {
    let (_class, _outer, mut object, mut c) = fixture();
    let mut nested_class = Object::new();
    let mut nested_object = Object::new();
    nested_class.set_within(c.outer);
    nested_object.kind = 1;
    nested_object.set_class(nested_class.ptr());
    nested_object.set_within(c.outer);
    nested_object.set_constructor(Some(checked_constructor::<33>));
    object.kind = 1;
    object.set_within(c.outer);
    object.set_constructor(Some(checked_constructor::<71>));
    c.found = object.ptr();
    c.reenter_init = true;
    c.reenter_class = nested_class.ptr();
    c.reenter_object = nested_object.ptr();
    assert_eq!(run(&mut c), Ok(object.ptr()));
    assert_eq!(c.nested, Some(Ok(nested_object.ptr())));
    assert_eq!(invoke_saved(object.constructor(), &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 71);
    assert_eq!(object.index(), 9);
    assert_eq!(nested_object.index(), 3);
    assert_eq!(
        invoke_saved(nested_object.constructor(), &mut nested_object, &mut c),
        0
    );
    assert_eq!(nested_object.constructor_effect, 33);
}

#[test]
fn constructor_runs_only_for_published_success_under_outer_tag() {
    let (mut class, _outer, object, mut c) = fixture();
    class.set_constructor(Some(checked_constructor::<11>));
    c.add_constructor = Some(checked_constructor::<88>);
    let request = c.request();
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Ok(c.object)
    );
    assert!(c.ctor_seen_published);
    assert_eq!(object.constructor_effect, 88);
    before(&c, Event::Leave, Event::Constructor(88));
    before(&c, Event::Constructor(88), Event::ConstructLeave);
    assert_eq!((c.depth, c.construct_depth), (0, 0));
    c.events.clear();
    class.set_class_flags(CLASS_ABSTRACT);
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Ok(null_mut())
    );
    assert!(!c.events.iter().any(|e| matches!(e, Event::Constructor(_))));
    assert_eq!(c.events.last(), Some(&Event::ConstructLeave));
}

#[test]
fn constructor_failure_retains_publication_and_unwinds_both_tag_boundaries() {
    let (mut class, _outer, mut object, mut c) = fixture();
    class.set_constructor(Some(checked_constructor::<88>));
    c.fail = Some(Event::Constructor(88));
    let request = c.request();
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Err(71)
    );
    assert_eq!(object.index(), 42);
    assert_eq!(object.constructor_effect, 88);
    assert_eq!((c.depth, c.construct_depth), (0, 0));
    assert_eq!(c.events.last(), Some(&Event::ConstructLeave));
    c.events.clear();
    c.fail = Some(Event::Init);
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Err(71)
    );
    assert_eq!(object.index(), -1);
    assert!(!c.events.iter().any(|e| matches!(e, Event::Constructor(_))));
    before(&c, Event::Leave, Event::ConstructLeave);
    c.events.clear();
    c.fail = None;
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Ok(object.ptr())
    );
    assert_eq!(object.index(), 42);
    assert_eq!(object.constructor_effect, 88);
    assert!(c.ctor_seen_published);
    assert_eq!((c.depth, c.construct_depth), (0, 0));
}

#[test]
fn replacement_restores_nullable_class_constructor_after_configuration() {
    let (mut class, _outer, mut object, mut c) = fixture();
    class.set_class_flags(CLASS_PER_OBJECT_CONFIG);
    object.kind = 1;
    object.set_within(c.outer);
    object.set_constructor(None);
    c.found = object.ptr();
    assert_eq!(run(&mut c), Ok(object.ptr()));
    assert_eq!(object.index(), 9);
    assert_eq!(object.within(), c.outer);
    assert!(object.constructor().is_none());
    let (index, offset, configured_constructor) = c.config_observation.unwrap();
    assert_eq!((index, offset), (9, 0));
    assert_eq!(invoke_saved(configured_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 22);
    let (localized_offset, localized_constructor) = c.localized_observation.unwrap();
    assert_eq!(localized_offset, 0);
    assert_eq!(invoke_saved(localized_constructor, &mut object, &mut c), 0);
    assert_eq!(object.constructor_effect, 33);
    before(&c, Event::Localized, Event::Leave);
}

#[test]
fn checked_constructor_can_reenter_with_independent_published_objects() {
    let (_class, _outer, object, mut c) = fixture();
    let mut nested_class = Object::new();
    let mut nested_object = Object::new();
    nested_class.set_within(c.outer);
    nested_class.set_constructor(Some(checked_constructor::<33>));
    c.reenter_constructor = true;
    c.reenter_class = nested_class.ptr();
    c.reenter_object = nested_object.ptr();
    let request = c.request();
    assert_eq!(
        unsafe { construct(c.ptr(), &request, &CONSTRUCTION) },
        Ok(c.object)
    );
    assert_eq!(c.nested, Some(Ok(nested_object.ptr())));
    assert_eq!((object.index(), nested_object.index()), (42, 42));
    assert_eq!(
        (object.constructor_effect, nested_object.constructor_effect),
        (11, 33)
    );
    assert!(c.ctor_seen_published);
    assert_eq!((c.depth, c.construct_depth), (0, 0));
    assert!(object.callback_context.is_null());
    assert!(nested_object.callback_context.is_null());
}
