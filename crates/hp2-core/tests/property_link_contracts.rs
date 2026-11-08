use hp2_abi::engine::generated::{FRepRecord, UClass};
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::property_link::*;
use std::alloc::{self, Layout};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::ptr::null_mut;

type Obj = *mut c_void;
thread_local! { static EDITOR: Cell<i32> = const { Cell::new(0) }; }

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Preload(usize),
    Link(usize, usize),
    Condition(usize),
    Equal(usize, usize, i32),
    Check(usize, i32),
    Lookup(usize),
    Accept(usize),
    Missing(usize),
    Bind(usize),
    Inherit(usize, usize),
    Compare(usize, usize),
}
#[derive(Default)]
struct Environment {
    events: Vec<Event>,
    fail: Option<Event>,
    allocations: HashMap<*mut u8, Layout>,
    allocation_calls: usize,
    fail_allocation: Option<usize>,
    free_calls: usize,
    fail_free_at: Option<usize>,
    fail_free: bool,
    relocate_fields_on_compare: Option<Obj>,
}
#[repr(C)]
struct Node {
    abi: UClass,
    id: usize,
    env: *mut Environment,
    pointers: [Obj; 7],
    values: [i32; 11],
    kind: i32,
    chains: [Obj; 5],
    native_member: i32,
    linked_offset: i32,
    change_outer_on_preload: Option<Obj>,
    script: Vec<u8>,
    condition_len: i32,
    constructor: usize,
    native: bool,
    exports: bool,
    lookup_constructor: usize,
    root_class: bool,
    sort_key: i32,
}
impl Node {
    fn new(id: usize, env: *mut Environment) -> Self {
        Self {
            // SAFETY: canonical class mirrors contain only scalar/pointer fields.
            abi: unsafe { std::mem::zeroed() },
            id,
            env,
            pointers: [null_mut(); 7],
            values: [0; 11],
            kind: 0,
            chains: [null_mut(); 5],
            native_member: -1,
            linked_offset: 0,
            change_outer_on_preload: None,
            script: Vec::new(),
            condition_len: 0,
            constructor: 0,
            native: false,
            exports: false,
            lookup_constructor: 0,
            root_class: false,
            sort_key: id as i32,
        }
    }
    fn fields(&self) -> Vec<Obj> {
        // SAFETY: inspect only initialized elements, never failed-add suffixes.
        unsafe {
            (0..self.abi.net_fields.num).map(|i| {
                self.abi.net_fields.data.cast::<Obj>().add(i as usize).read_unaligned()
            }).collect()
        }
    }
    fn reps(&self) -> Vec<(Obj, i32)> {
        // SAFETY: callers inspect only successfully initialized record arrays.
        unsafe {
            (0..self.abi.class_reps.num).map(|i| {
                let record = self.abi.class_reps.data.cast::<FRepRecord>().add(i as usize);
                ((*record).property, (*record).index)
            }).collect()
        }
    }
}
// Callback identities must survive moves of the fixture and its node list.
struct Fixture {
    env: *mut Environment,
    nodes: Vec<*mut Node>,
}
impl Deref for Fixture {
    type Target = Environment;
    fn deref(&self) -> &Environment {
        // SAFETY: this fixture owns the live environment until Drop.
        unsafe { &*self.env }
    }
}
impl DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Environment {
        // SAFETY: callbacks are synchronous and no environment borrow spans one.
        unsafe { &mut *self.env }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for node in self.nodes.drain(..) {
            // SAFETY: each node is an independently owned raw Box, released once.
            unsafe { drop(Box::from_raw(node)); }
        }
        // SAFETY: release all fixture-owned allocations, including buffers with
        // native failure-prefix headers whose Num may exceed initialized data.
        unsafe {
            for (address, layout) in (*self.env).allocations.drain() {
                alloc::dealloc(address, layout);
            }
        }
        unsafe { drop(Box::from_raw(self.env)); }
    }
}
impl Fixture {
    fn new() -> Self {
        EDITOR.with(|v| v.set(0));
        Self {
            env: Box::into_raw(Box::<Environment>::default()),
            nodes: Vec::new(),
        }
    }
    fn node(&mut self, kind: i32) -> Obj {
        let node = Box::into_raw(Box::new(Node::new(self.nodes.len() + 1, self.env)));
        // SAFETY: newly allocated node, not yet exposed to callbacks.
        unsafe { (*node).kind = kind; }
        self.nodes.push(node);
        node.cast()
    }
    fn property(&mut self, root: Obj, flags: i32) -> Obj {
        let p = self.node(PROPERTY);
        unsafe {
            node(p).pointers[OUTER as usize] = root;
            node(p).pointers[OWNER_CLASS as usize] = root;
            node(p).values[FLAGS as usize] = flags;
            node(p).values[ARRAY_DIM as usize] = 1;
            node(p).values[PROPERTY_ALIGN as usize] = 4;
            node(p).values[PROPERTY_SIZE as usize] = 4;
        }
        p
    }
    fn children(&mut self, root: Obj, fields: &[Obj]) {
        unsafe {
            node(root).pointers[CHILDREN as usize] = fields.first().copied().unwrap_or(null_mut());
            for (index, &field) in fields.iter().enumerate() {
                node(field).pointers[NEXT as usize] =
                    fields.get(index + 1).copied().unwrap_or(null_mut());
            }
        }
    }
    fn condition(&mut self, p: Obj, offset: usize, bytes: &[u8]) {
        unsafe {
            node(p).values[REP_OFFSET as usize] = offset as i32;
            node(p).script = vec![0xee; offset];
            node(p).script.extend_from_slice(bytes);
            node(p).condition_len = bytes.len() as i32;
        }
    }
    fn arrays(&mut self) -> Hp2ArrayServices {
        Hp2ArrayServices {
            context: self.env.cast(),
            realloc: allocate,
            checked_int_size: checked_size,
            free: release,
        }
    }
}
unsafe fn node<'a>(p: Obj) -> &'a mut Node {
    unsafe { &mut *p.cast::<Node>() }
}
unsafe fn id(p: Obj) -> usize {
    if p.is_null() {
        0
    } else {
        unsafe { node(p).id }
    }
}
unsafe fn event(p: Obj, e: Event) -> i32 {
    unsafe {
        let env = &mut *node(p).env;
        let fail = env.fail.as_ref() == Some(&e);
        env.events.push(e);
        if fail { 77 } else { 0 }
    }
}
unsafe extern "C" fn pointer(p: Obj, selector: i32, out: *mut Obj) -> i32 {
    let selector = if selector == CLASS_SUPER {
        SUPER
    } else {
        selector
    };
    unsafe {
        *out = node(p).pointers[selector as usize];
    }
    0
}
unsafe extern "C" fn integer(p: Obj, selector: i32) -> i32 {
    unsafe { node(p).values[selector as usize] }
}
unsafe extern "C" fn set_integer(p: Obj, selector: i32, value: i32) {
    unsafe {
        node(p).values[selector as usize] = if selector == REP_INDEX {
            value as u16 as i32
        } else {
            value
        };
    }
}
unsafe extern "C" fn kind(p: Obj, mask: i32) -> i32 {
    unsafe { node(p).kind & mask }
}
unsafe extern "C" fn editor() -> i32 {
    EDITOR.with(Cell::get)
}
unsafe extern "C" fn preload(_: Obj, p: Obj) -> i32 {
    unsafe {
        if let Some(outer) = node(p).change_outer_on_preload {
            node(p).pointers[OUTER as usize] = outer;
        }
        event(p, Event::Preload(id(p)))
    }
}
unsafe extern "C" fn link(p: Obj, _: Obj, previous: Obj) -> i32 {
    unsafe {
        node(p).values[OFFSET as usize] = node(p).linked_offset;
        event(p, Event::Link(id(p), id(previous)))
    }
}
unsafe extern "C" fn native_member(_: Obj, p: Obj) -> i32 {
    unsafe { node(p).native_member }
}
unsafe extern "C" fn set_chain(root: Obj, previous: Obj, chain: i32, value: Obj) {
    unsafe {
        node(if previous.is_null() { root } else { previous }).chains[chain as usize] = value;
    }
}
unsafe extern "C" fn set_rep_owner(p: Obj, owner: Obj) {
    unsafe {
        node(p).pointers[REP_OWNER as usize] = owner;
    }
}
unsafe extern "C" fn condition_end(p: Obj, out: *mut i32) -> i32 {
    unsafe {
        *out = node(p).values[REP_OFFSET as usize] + node(p).condition_len;
        event(p, Event::Condition(id(p)))
    }
}
unsafe extern "C" fn allocate(
    context: Obj,
    data: Obj,
    bytes: usize,
    _: *const i32,
    _: i32,
    _: *const c_void,
    out: *mut Obj,
) -> i32 {
    unsafe {
        let env = &mut *context.cast::<Environment>();
        env.allocation_calls += 1;
        if env.fail_allocation == Some(env.allocation_calls) {
            return 81;
        }
        let layout = Layout::from_size_align(bytes.max(1), 16).unwrap();
        let fresh = if data.is_null() {
            alloc::alloc(layout)
        } else {
            let old = env.allocations[&data.cast::<u8>()];
            alloc::realloc(data.cast(), old, layout.size())
        };
        if fresh.is_null() {
            return 81;
        }
        env.allocations.remove(&data.cast::<u8>());
        env.allocations.insert(fresh, layout);
        out.write(fresh.cast());
    }
    0
}

unsafe extern "C" fn checked_size(_: Obj, bytes: usize, out: *mut i32) -> i32 {
    match i32::try_from(bytes) {
        Ok(bytes) => {
            unsafe { out.write(bytes) };
            0
        }
        Err(_) => 81,
    }
}

unsafe extern "C" fn release(context: Obj, data: Obj) -> i32 {
    unsafe {
        let env = &mut *context.cast::<Environment>();
        env.free_calls += 1;
        if env.fail_free_at == Some(env.free_calls) {
            return 82;
        }
        let layout = env.allocations.remove(&data.cast::<u8>()).unwrap();
        alloc::dealloc(data.cast(), layout);
        if env.fail_free { 82 } else { 0 }
    }
}
unsafe extern "C" fn equal(a: Obj, b: Obj, len: i32, out: *mut i32) -> i32 {
    unsafe {
        let result = event(a, Event::Equal(id(a), id(b), len));
        if result != 0 {
            return result;
        }
        let start_a = node(a).values[REP_OFFSET as usize] as usize;
        let start_b = node(b).values[REP_OFFSET as usize] as usize;
        *out = i32::from(
            node(a).script[start_a..start_a + len as usize]
                == node(b).script[start_b..start_b + len as usize],
        );
        0
    }
}
const SERVICES: Hp2PropertyLinkServices = Hp2PropertyLinkServices {
    pointer,
    integer,
    set_integer,
    kind,
    editor,
    preload,
    link,
    native_member,
    set_chain,
    set_rep_owner,
    condition_end,
    condition_equal: equal,
};

unsafe extern "C" fn bind_check(p: Obj, final_check: i32) -> i32 {
    unsafe {
        let result = event(p, Event::Check(id(p), final_check));
        if result != 0 {
            return result;
        }
        let valid = editor() != 0
            || if final_check != 0 {
                node(p).constructor != 0
            } else {
                node(p).root_class || !node(p).pointers[SUPER as usize].is_null()
            };
        if valid { 0 } else { 79 }
    }
}
unsafe extern "C" fn has_constructor(p: Obj) -> i32 {
    unsafe { i32::from(node(p).constructor != 0) }
}
unsafe extern "C" fn native(p: Obj) -> i32 {
    unsafe { i32::from(node(p).native) }
}
unsafe extern "C" fn superclass(p: Obj) -> Obj {
    unsafe { node(p).pointers[SUPER as usize] }
}
unsafe extern "C" fn lookup(p: Obj, out: *mut Obj) -> i32 {
    unsafe {
        if node(p).lookup_constructor != 0 {
            node(p).constructor = node(p).lookup_constructor;
        }
        *out = if node(p).exports { p } else { null_mut() };
        event(p, Event::Lookup(id(p)))
    }
}
unsafe extern "C" fn accept(p: Obj, _: Obj) -> i32 {
    unsafe { event(p, Event::Accept(id(p))) }
}
unsafe extern "C" fn missing(p: Obj) -> i32 {
    unsafe {
        event(p, Event::Missing(id(p)));
    }
    80
}
unsafe extern "C" fn bind_parent(p: Obj) -> i32 {
    unsafe {
        let result = event(p, Event::Bind(id(p)));
        if result != 0 {
            return result;
        }
        bind(p, &BIND).err().unwrap_or(0)
    }
}
unsafe extern "C" fn inherit(p: Obj, parent: Obj) {
    unsafe {
        node(p).constructor = node(parent).constructor;
        event(p, Event::Inherit(id(p), id(parent)));
    }
}
const BIND: Hp2ClassBindServices = Hp2ClassBindServices {
    check: bind_check,
    has_constructor,
    native,
    editor,
    superclass,
    lookup,
    accept_export: accept,
    missing_export: missing,
    bind: bind_parent,
    inherit,
};

unsafe fn seed_reps(p: Obj, values: &[(Obj, i32)]) {
    unsafe {
        let array = &raw mut (*p.cast::<Node>()).abi.class_reps;
        let slots = Hp2ArraySlots {
            data: &raw mut (*array).data,
            num: &raw mut (*array).num,
            max: &raw mut (*array).max,
        };
        let arrays = Hp2ArrayServices {
            context: (*p.cast::<Node>()).env.cast(),
            realloc: allocate,
            checked_int_size: checked_size,
            free: release,
        };
        array_storage::set(slots, values.len() as i32, std::mem::size_of::<FRepRecord>() as i32, std::ptr::null(), arrays).unwrap();
        for (i, &(property, index)) in values.iter().enumerate() {
            let record = slots.data.read_unaligned().cast::<FRepRecord>().add(i);
            (&raw mut (*record).property).write_unaligned(property);
            (&raw mut (*record).index).write_unaligned(index);
        }
    }
}
unsafe extern "C" fn compare(a: Obj, b: Obj, out: *mut i32) -> i32 {
    unsafe {
        let left = node(a).sort_key;
        let environment = node(a).env;
        if let Some(class) = (*environment).relocate_fields_on_compare.take() {
            let header = &raw mut node(class).abi.net_fields;
            let old = (*header).data;
            let bytes = (*header).max as usize * std::mem::size_of::<Obj>();
            let layout = Layout::from_size_align(bytes, 16).unwrap();
            let fresh = alloc::alloc(layout);
            assert!(!fresh.is_null());
            std::ptr::copy_nonoverlapping(old.cast::<u8>(), fresh, bytes);
            let old_layout = (*environment).allocations.remove(&old.cast::<u8>()).unwrap();
            (*header).data = fresh.cast();
            (*environment).allocations.insert(fresh, layout);
            alloc::dealloc(old.cast(), old_layout);
        }
        let right = node(b).sort_key;
        *out = left.cmp(&right) as i32;
        event(a, Event::Compare(id(a), id(b)))
    }
}
unsafe extern "C" fn sort_guard(class: Obj) -> i32 {
    unsafe {
        sort_network_fields(class.cast(), &NETWORK).err().unwrap_or(0)
    }
}
const NETWORK: Hp2ClassNetworkServices = Hp2ClassNetworkServices {
    compare,
    sort_guard,
};

#[test]
fn layout_inherits_alignment_restores_cpp_offsets_and_overrides_native_members() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let a = f.property(root, 0);
    let nonproperty = f.node(0);
    let b = f.property(root, 0);
    let native = f.property(root, 0);
    let foreign = f.property(parent, 0);
    f.children(root, &[a, nonproperty, b, native, foreign]);
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        node(parent).values[SIZE as usize] = 13;
        node(parent).values[ALIGN as usize] = 8;
        node(nonproperty).pointers[OUTER as usize] = root;
        node(a).linked_offset = 16;
        node(a).values[PROPERTY_SIZE as usize] = 3;
        node(b).linked_offset = 100;
        node(b).values[OFFSET as usize] = 40;
        node(b).values[CPP as usize] = 1;
        node(b).values[PROPERTY_SIZE as usize] = 5;
        node(b).values[PROPERTY_ALIGN as usize] = 16;
        node(native).native_member = 2;
        node(native).linked_offset = 48;
        assert_eq!(
            layout(root, null_mut(), 1, &[8, 16, 64, 72, 80], &SERVICES),
            Ok(())
        );
        assert_eq!(node(root).values[SIZE as usize], 80);
        assert_eq!(node(root).values[ALIGN as usize], 16);
        assert_eq!(node(b).values[OFFSET as usize], 40);
        assert_eq!(node(native).values[OFFSET as usize], 64);
        assert_eq!(
            f.events,
            vec![
                Event::Preload(id(parent)),
                Event::Preload(id(a)),
                Event::Link(id(a), 0),
                Event::Preload(id(nonproperty)),
                Event::Preload(id(b)),
                Event::Link(id(b), id(a)),
                Event::Preload(id(native)),
                Event::Link(id(native), id(b)),
                Event::Preload(id(foreign))
            ]
        );
    }
}

#[test]
fn fixed_offsets_check_ownership_before_preload_and_never_recompute_layout() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let foreign_owner = f.node(0);
    let a = f.property(root, 0);
    let foreign = f.property(foreign_owner, 0);
    f.children(root, &[a, foreign]);
    unsafe {
        node(root).values[SIZE as usize] = 73;
        node(root).values[ALIGN as usize] = 16;
        node(a).values[OFFSET as usize] = 51;
        node(a).linked_offset = 9;
        node(a).change_outer_on_preload = Some(foreign_owner);
        assert_eq!(layout(root, null_mut(), 0, &[0; 5], &SERVICES), Ok(()));
        assert_eq!(node(a).values[OFFSET as usize], 51);
        assert_eq!(node(root).values[SIZE as usize], 73);
        assert_eq!(node(root).values[ALIGN as usize], 16);
        assert_eq!(
            f.events,
            vec![Event::Preload(id(a)), Event::Link(id(a), 0)]
        );
        f.events.clear();
        node(a).pointers[OUTER as usize] = root;
        assert_eq!(layout(root, null_mut(), 1, &[0; 5], &SERVICES), Ok(()));
        assert_eq!(f.events, vec![Event::Preload(id(a))]);
        assert_eq!(node(root).values[SIZE as usize], 0);
    }
}

#[test]
fn throwing_property_link_preserves_leaf_mutation_and_unaligned_partial_layout() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let a = f.property(root, 0);
    let b = f.property(root, 0);
    f.children(root, &[a, b]);
    unsafe {
        node(a).linked_offset = 0;
        node(a).values[PROPERTY_SIZE as usize] = 3;
        node(b).values[CPP as usize] = 1;
        node(b).values[OFFSET as usize] = 64;
        node(b).linked_offset = 12;
        f.fail = Some(Event::Link(id(b), id(a)));
        assert_eq!(layout(root, null_mut(), 1, &[0; 5], &SERVICES), Err(77));
        assert_eq!(node(root).values[SIZE as usize], 3);
        assert_eq!(node(b).values[OFFSET as usize], 12);
        assert_eq!(f.events.last(), Some(&Event::Link(id(b), id(a))));
    }
}

#[test]
fn chains_follow_live_inherited_iterator_order_and_clear_all_tails() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let other = f.node(0);
    let a = f.property(root, CONFIG | CTOR);
    let b = f.property(other, CTOR);
    let inherited = f.property(parent, CONFIG);
    f.children(root, &[a, b]);
    f.children(parent, &[inherited]);
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        node(a).kind |= OBJECT_PROPERTY;
        node(b).kind |= STRUCT_PROPERTY;
        node(inherited).kind |= OBJECT_PROPERTY | STRUCT_PROPERTY;
        for p in [root, a, b, inherited] {
            node(p).chains = [other; 5];
        }
        assert_eq!(reference_chain(root, REFERENCE_CHAIN, &SERVICES), Ok(()));
        assert_eq!(reference_chain(root, STRUCT_CHAIN, &SERVICES), Ok(()));
        assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Ok(()));
        assert_eq!(node(root).chains, [a, b, a, a, a]);
        assert_eq!(node(a).chains[REFERENCE_CHAIN as usize], inherited);
        assert_eq!(node(b).chains[STRUCT_CHAIN as usize], inherited);
        assert_eq!(node(a).chains[PROPERTY_CHAIN as usize], b);
        assert_eq!(node(b).chains[PROPERTY_CHAIN as usize], inherited);
        assert_eq!(node(a).chains[CONFIG_CHAIN as usize], inherited);
        assert_eq!(node(a).chains[CTOR_CHAIN as usize], b);
        assert!(node(b).chains[CTOR_CHAIN as usize].is_null());
        for chain in [REFERENCE_CHAIN, STRUCT_CHAIN, PROPERTY_CHAIN, CONFIG_CHAIN] {
            assert!(node(inherited).chains[chain as usize].is_null());
        }
        node(root).pointers[CHILDREN as usize] = null_mut();
        node(root).pointers[SUPER as usize] = null_mut();
        reference_chain(root, REFERENCE_CHAIN, &SERVICES).unwrap();
        reference_chain(root, STRUCT_CHAIN, &SERVICES).unwrap();
        cleanup_chains(root, f.arrays(), &SERVICES).unwrap();
        assert_eq!(node(root).chains, [null_mut(); 5]);
    }
}

#[test]
fn equivalent_replication_conditions_reassign_earlier_owners_to_last_match() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let a = f.property(root, NET);
    let different = f.property(root, NET);
    let longer = f.property(root, NET);
    let nonnet = f.property(root, 0);
    let b = f.property(parent, NET);
    f.children(root, &[a, different, longer, nonnet]);
    f.children(parent, &[b]);
    f.condition(a, 2, &[1, 2]);
    f.condition(different, 0, &[1, 3]);
    f.condition(longer, 1, &[1, 2, 3]);
    f.condition(b, 5, &[1, 2]);
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        cleanup_chains(root, f.arrays(), &SERVICES).unwrap();
        assert_eq!(node(a).pointers[REP_OWNER as usize], b);
        assert_eq!(node(b).pointers[REP_OWNER as usize], b);
        assert_eq!(node(different).pointers[REP_OWNER as usize], different);
        assert_eq!(node(longer).pointers[REP_OWNER as usize], longer);
        assert!(node(nonnet).pointers[REP_OWNER as usize].is_null());
        assert!(f.allocations.is_empty());
        assert!(f.events.contains(&Event::Equal(id(b), id(a), 2)));
        assert!(
            !f
                .events
                .iter()
                .any(|e| matches!(e, Event::Equal(x, _, _) if *x == id(longer)))
        );
        f.events.clear();
        EDITOR.with(|v| v.set(1));
        node(a).pointers[REP_OWNER as usize] = nonnet;
        cleanup_chains(root, f.arrays(), &SERVICES).unwrap();
        assert!(f.events.is_empty());
        assert_eq!(node(a).pointers[REP_OWNER as usize], nonnet);
        assert_eq!(node(root).chains[PROPERTY_CHAIN as usize], a);
    }
}

#[test]
fn condition_error_sets_current_owner_but_does_not_append_or_reset_tails() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let sentinel = f.node(0);
    let a = f.property(root, NET | CONFIG | CTOR);
    let b = f.property(root, NET | CONFIG | CTOR);
    f.children(root, &[a, b]);
    f.condition(a, 0, &[4]);
    f.condition(b, 0, &[4]);
    unsafe {
        node(a).chains = [sentinel; 5];
        f.fail = Some(Event::Condition(id(b)));
        assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Err(77));
        assert_eq!(node(b).pointers[REP_OWNER as usize], b);
        assert_eq!(node(a).pointers[REP_OWNER as usize], a);
        for chain in [PROPERTY_CHAIN, CONFIG_CHAIN, CTOR_CHAIN] {
            assert_eq!(node(root).chains[chain as usize], a);
            assert_eq!(node(a).chains[chain as usize], sentinel);
        }
        assert!(f.allocations.is_empty());
        assert_eq!(f.events.last(), Some(&Event::Condition(id(b))));
    }
}

#[test]
fn condition_map_allocation_errors_release_storage_without_finishing_chains() {
    // Initial buckets, first pair storage, bucket growth, then pair growth.
    for (allocation, completed) in [(1, 0), (2, 0), (3, 24), (4, 33)] {
        let mut f = Fixture::new();
        let root = f.node(0);
        let sentinel = f.node(0);
        let fields: Vec<_> = (0..40)
            .map(|_| f.property(root, NET | CONFIG | CTOR))
            .collect();
        f.children(root, &fields);
        for &p in &fields {
            f.condition(p, 0, &[7]);
        }
        f.fail_allocation = Some(allocation);
        unsafe {
            node(root).chains = [sentinel; 5];
            for &p in &fields {
                node(p).chains = [sentinel; 5];
            }
            assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Err(81));
            assert!(f.allocations.is_empty(), "allocation {allocation}");
            for chain in [PROPERTY_CHAIN, CONFIG_CHAIN, CTOR_CHAIN] {
                let head = if completed == 0 { sentinel } else { fields[0] };
                assert_eq!(node(root).chains[chain as usize], head);
                if completed != 0 {
                    assert_eq!(node(fields[completed - 1]).chains[chain as usize], sentinel);
                }
            }
            if allocation == 1 {
                assert!(f.events.is_empty());
                assert!(node(fields[0]).pointers[REP_OWNER as usize].is_null());
            } else {
                assert_eq!(node(fields[completed]).pointers[REP_OWNER as usize], fields[completed]);
                assert_eq!(f.events.last(), Some(&Event::Condition(id(fields[completed]))));
                if completed != 0 {
                    for &p in &fields[..completed] {
                        assert_eq!(node(p).pointers[REP_OWNER as usize], fields[completed - 1]);
                    }
                }
            }
        }
    }
}

#[test]
fn condition_map_growth_keeps_all_conditions_and_releases_every_allocation() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let fields: Vec<_> = (0..80).map(|_| f.property(root, NET)).collect();
    f.children(root, &fields);
    for (offset, &p) in fields.iter().enumerate() {
        f.condition(p, offset, &[9]);
    }
    unsafe {
        cleanup_chains(root, f.arrays(), &SERVICES).unwrap();
        for &p in &fields {
            assert_eq!(node(p).pointers[REP_OWNER as usize], *fields.last().unwrap());
        }
        assert!(node(*fields.last().unwrap()).chains[PROPERTY_CHAIN as usize].is_null());
    }
    assert!(f.allocations.is_empty());
    assert!(f.allocation_calls > 4);
}

#[test]
fn condition_map_growth_free_error_reclaims_unpublished_buckets_and_keeps_partial_chains() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let sentinel = f.node(0);
    let fields: Vec<_> = (0..25)
        .map(|_| f.property(root, NET | CONFIG | CTOR))
        .collect();
    f.children(root, &fields);
    for &p in &fields {
        f.condition(p, 0, &[7]);
    }
    f.fail_free_at = Some(1);
    unsafe {
        for &p in &fields {
            node(p).chains = [sentinel; 5];
        }
        assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Err(82));
        assert!(f.allocations.is_empty());
        assert_eq!(f.allocation_calls, 3);
        assert_eq!(f.free_calls, 4);
        assert_eq!(f.events.last(), Some(&Event::Condition(id(fields[24]))));
        for &p in &fields[..24] {
            assert_eq!(node(p).pointers[REP_OWNER as usize], fields[23]);
        }
        assert_eq!(node(fields[24]).pointers[REP_OWNER as usize], fields[24]);
        for chain in [PROPERTY_CHAIN, CONFIG_CHAIN, CTOR_CHAIN] {
            assert_eq!(node(root).chains[chain as usize], fields[0]);
            assert_eq!(node(fields[23]).chains[chain as usize], sentinel);
        }
    }
}

#[test]
fn condition_error_remains_first_when_temporary_storage_cleanup_also_errors() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let a = f.property(root, NET);
    let b = f.property(root, NET);
    f.children(root, &[a, b]);
    f.condition(a, 0, &[1]);
    f.condition(b, 0, &[1]);
    unsafe {
        f.fail = Some(Event::Condition(id(b)));
        f.fail_free = true;
        assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Err(77));
    }
    assert!(f.allocations.is_empty());
    assert_eq!(f.free_calls, 2);
}


#[test]
fn binding_rereads_lookup_side_effects_and_inherits_only_when_still_missing() {
    let mut f = Fixture::new();
    let parent = f.node(0);
    let child = f.node(0);
    unsafe {
        node(parent).root_class = true;
        node(parent).native = true;
        node(parent).exports = true;
        node(parent).lookup_constructor = 42;
        node(child).pointers[SUPER as usize] = parent;
        bind(child, &BIND).unwrap();
        assert_eq!(node(child).constructor, 42);
        assert!(f.events.contains(&Event::Lookup(id(parent))));
        assert!(
            f
                .events
                .contains(&Event::Inherit(id(child), id(parent)))
        );
        f.events.clear();
        node(child).constructor = 0;
        node(child).native = true;
        node(child).exports = true;
        node(child).lookup_constructor = 99;
        bind(child, &BIND).unwrap();
        assert_eq!(node(child).constructor, 99);
        assert_eq!(
            f.events,
            vec![
                Event::Check(id(child), 0),
                Event::Lookup(id(child)),
                Event::Accept(id(child)),
                Event::Check(id(child), 1)
            ]
        );
    }
}

#[test]
fn missing_native_export_errors_before_inheritance_but_editor_continues() {
    let mut f = Fixture::new();
    let parent = f.node(0);
    let child = f.node(0);
    unsafe {
        node(parent).root_class = true;
        node(parent).constructor = 7;
        node(child).pointers[SUPER as usize] = parent;
        node(child).native = true;
        assert_eq!(bind(child, &BIND), Err(80));
        assert_eq!(node(child).constructor, 0);
        assert_eq!(f.events.last(), Some(&Event::Missing(id(child))));
        f.events.clear();
        EDITOR.with(|v| v.set(1));
        bind(child, &BIND).unwrap();
        assert_eq!(node(child).constructor, 7);
        assert!(!f.events.contains(&Event::Missing(id(child))));
        // GetDllExport can install a constructor even while returning no export.
        node(child).constructor = 0;
        node(child).lookup_constructor = 91;
        f.events.clear();
        bind(child, &BIND).unwrap();
        assert_eq!(node(child).constructor, 91);
        assert!(!f.events.contains(&Event::Bind(id(parent))));
        node(child).constructor = 0;
        f.fail = Some(Event::Lookup(id(child)));
        assert_eq!(bind(child, &BIND), Err(77));
        assert_eq!(node(child).constructor, 91);
        assert_eq!(f.events.last(), Some(&Event::Lookup(id(child))));
    }
}

#[test]
fn network_expands_owned_arrays_inherits_reps_filters_functions_and_stops_at_owner_boundary() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let inherited = f.property(parent, NET);
    let a = f.property(root, NET);
    let zero = f.property(root, NET);
    let foreign_outer = f.property(parent, NET);
    let function = f.node(FUNCTION);
    let override_function = f.node(FUNCTION);
    let not_visited = f.property(root, NET);
    f.children(
        root,
        &[
            a,
            zero,
            foreign_outer,
            function,
            override_function,
            inherited,
            not_visited,
        ],
    );
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        seed_reps(parent, &[(inherited, 0), (inherited, 1)]);
        node(a).values[ARRAY_DIM as usize] = 2;
        node(zero).values[ARRAY_DIM as usize] = 0;
        node(foreign_outer).pointers[OWNER_CLASS as usize] = root;
        node(foreign_outer).values[REP_INDEX as usize] = 63;
        for p in [function, override_function] {
            node(p).pointers[OWNER_CLASS as usize] = root;
            node(p).values[FUNCTION_FLAGS as usize] = 0x40;
        }
        node(override_function).pointers[SUPER_FUNCTION as usize] = function;
        node(function).sort_key = -1;
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        assert_eq!(node(root).fields(), vec![function, a, zero, foreign_outer]);
        assert_eq!(
            node(root).reps(),
            vec![(inherited, 0), (inherited, 1), (a, 0), (a, 1)]
        );
        assert_eq!(node(a).values[REP_INDEX as usize], 2);
        assert_eq!(node(zero).values[REP_INDEX as usize], 4);
        assert_eq!(node(foreign_outer).values[REP_INDEX as usize], 63);
        assert!(!node(root).fields().contains(&not_visited));
        let fields = node(root).abi.net_fields;
        assert_eq!(fields.max, fields.num);
        f.events.clear();
        EDITOR.with(|v| v.set(1));
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        assert!(f.events.is_empty());
        assert_eq!(node(root).reps().len(), 4);
    }
}

#[test]
fn network_rep_failure_retains_field_index_and_initialized_record_prefix() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let p = f.property(root, NET);
    f.children(root, &[p]);
    unsafe {
        node(p).values[ARRAY_DIM as usize] = 40;
        // Empty fields, empty reps, first field, first reps, then reps growth.
        f.fail_allocation = Some(5);
        assert_eq!(network(root, f.arrays(), &SERVICES, &NETWORK), Err(81));
        assert_eq!(node(root).fields(), vec![p]);
        assert_eq!(node(p).values[REP_INDEX as usize], 0);
        assert_eq!(node(root).abi.class_reps.num, 34);
        assert_eq!(node(root).abi.class_reps.max, 78);
        let data = node(root).abi.class_reps.data.cast::<FRepRecord>();
        for i in 0..33 {
            assert_eq!((*data.add(i)).property, p);
            assert_eq!((*data.add(i)).index, i as i32);
        }
        assert_eq!(node(root).abi.net_fields.max, 33);
    }
}

#[test]
fn native_unstable_sort_preserves_tie_order_and_partition_error_mutations() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let mut fields = Vec::new();
    for _ in 0..9 {
        fields.push(f.property(root, NET));
    }
    f.children(root, &fields);
    unsafe {
        for &p in &fields {
            node(p).sort_key = 0;
        }
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        // Both partition scans cross all equal elements: only the pivot moves.
        let mut expected = fields.clone();
        expected.swap(0, 4);
        assert_eq!(node(root).fields(), expected);
        f.events.clear();
        f.fail = Some(Event::Compare(id(fields[1]), id(fields[4])));
        assert_eq!(network(root, f.arrays(), &SERVICES, &NETWORK), Err(77));
        let mut expected = fields.clone();
        expected.swap(0, 4);
        assert_eq!(node(root).fields(), expected);
        assert_eq!(node(root).reps().len(), 9);
        assert_eq!(
            f.events.last(),
            Some(&Event::Compare(id(fields[1]), id(fields[4])))
        );
    }
}

#[test]
fn network_sort_orders_partition_sizes_and_preserves_small_tie_behavior() {
    for count in [0, 1, 2, 8, 9, 17, 64] {
        let mut f = Fixture::new();
        let root = f.node(0);
        let mut fields = Vec::new();
        for index in 0..count {
            let p = f.property(root, NET);
            unsafe {
                node(p).sort_key = ((index * 37 + 11) % 67) as i32;
            }
            fields.push(p);
        }
        f.children(root, &fields);
        unsafe {
            network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
            let keys: Vec<_> = node(root)
                .fields()
                .iter()
                .map(|&p| node(p).sort_key)
                .collect();
            assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
            let mut expected = fields.clone();
            expected.sort_by_key(|&p| node(p).sort_key);
            assert_eq!(node(root).fields(), expected);
        }
    }
    let mut f = Fixture::new();
    let root = f.node(0);
    let a = f.property(root, NET);
    let b = f.property(root, NET);
    let c = f.property(root, NET);
    f.children(root, &[a, b, c]);
    unsafe {
        for p in [a, b, c] {
            node(p).sort_key = 0;
        }
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        assert_eq!(node(root).fields(), vec![b, c, a]);
    }
}

#[test]
fn condition_comparison_error_retains_completed_owner_reassignments_without_finishing_chains() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let sentinel = f.node(0);
    let a = f.property(root, NET);
    let b = f.property(root, NET);
    let c = f.property(root, NET);
    f.children(root, &[a, b, c]);
    for p in [a, b, c] {
        f.condition(p, 0, &[17]);
    }
    unsafe {
        node(b).chains[PROPERTY_CHAIN as usize] = sentinel;
        f.fail = Some(Event::Equal(id(c), id(b), 1));
        assert_eq!(cleanup_chains(root, f.arrays(), &SERVICES), Err(77));
        assert_eq!(node(a).pointers[REP_OWNER as usize], c);
        assert_eq!(node(b).pointers[REP_OWNER as usize], b);
        assert_eq!(node(c).pointers[REP_OWNER as usize], c);
        assert_eq!(node(a).chains[PROPERTY_CHAIN as usize], b);
        assert_eq!(node(b).chains[PROPERTY_CHAIN as usize], sentinel);
        assert!(f.allocations.is_empty());
        assert_eq!(f.events.last(), Some(&Event::Equal(id(c), id(b), 1)));
    }
}

#[test]
fn rep_index_uses_native_word_width_while_record_count_and_array_bounds_remain_ints() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let inherited = f.property(parent, NET);
    let p = f.property(root, NET);
    let negative_dim = f.property(root, NET);
    f.children(root, &[p, negative_dim]);
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        seed_reps(parent, &vec![(inherited, 0); 65536]);
        node(negative_dim).values[ARRAY_DIM as usize] = -1;
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        assert_eq!(node(p).values[REP_INDEX as usize], 0);
        assert_eq!(node(negative_dim).values[REP_INDEX as usize], 1);
        assert_eq!(node(root).reps().len(), 65537);
        assert_eq!(node(root).reps().last(), Some(&(p, 0)));
        assert_eq!(node(root).fields(), vec![p, negative_dim]);
    }
}

#[test]
fn binding_failure_stops_before_parent_inheritance_and_final_validation() {
    let mut f = Fixture::new();
    let parent = f.node(0);
    let child = f.node(0);
    unsafe {
        node(parent).constructor = 37;
        node(child).pointers[SUPER as usize] = parent;
        f.fail = Some(Event::Check(id(child), 0));
        assert_eq!(bind(child, &BIND), Err(77));
        assert_eq!(f.events, vec![Event::Check(id(child), 0)]);
        f.events.clear();
        f.fail = Some(Event::Bind(id(parent)));
        assert_eq!(bind(child, &BIND), Err(77));
        assert_eq!(node(child).constructor, 0);
        assert_eq!(
            f.events,
            vec![Event::Check(id(child), 0), Event::Bind(id(parent))]
        );
    }
}

#[test]
fn network_inherited_copy_allocation_failure_preserves_old_buffer_and_empty_prefix() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let parent = f.node(0);
    let p = f.property(root, NET);
    f.children(root, &[p]);
    unsafe {
        node(root).pointers[SUPER as usize] = parent;
        seed_reps(parent, &[(parent, 0), (parent, 1)]);
        seed_reps(root, &[(root, 9)]);
        let original = node(root).abi.class_reps.data;
        f.allocation_calls = 0;
        f.fail_allocation = Some(2);
        assert_eq!(network(root, f.arrays(), &SERVICES, &NETWORK), Err(81));
        assert_eq!(node(root).abi.net_fields.num, 0);
        assert_eq!(node(root).abi.class_reps.data, original);
        assert_eq!((node(root).abi.class_reps.num, node(root).abi.class_reps.max), (0, 2));
        let previous = original.cast::<FRepRecord>();
        let property = std::ptr::addr_of!((*previous).property).read_unaligned();
        let index = std::ptr::addr_of!((*previous).index).read_unaligned();
        assert_eq!((property, index), (root, 9));
        assert_eq!(node(p).values[REP_INDEX as usize], 0);
        assert!(f.events.is_empty());
    }
}

#[test]
fn network_field_growth_and_shrink_failures_preserve_native_count_prefixes() {
    for failure in [3, 5] {
        let mut f = Fixture::new();
        let root = f.node(0);
        let p = f.property(root, NET);
        f.children(root, &[p]);
        f.fail_allocation = Some(failure);
        unsafe {
            assert_eq!(network(root, f.arrays(), &SERVICES, &NETWORK), Err(81));
            assert_eq!(node(root).abi.net_fields.num, 1);
            if failure == 3 {
                assert_eq!(node(root).abi.net_fields.max, 33);
                assert_eq!(node(root).abi.class_reps.num, 0);
            } else {
                assert_eq!(node(root).abi.net_fields.max, 1);
                assert_eq!(node(root).fields(), vec![p]);
                assert_eq!(node(root).reps(), vec![(p, 0)]);
            }
            assert!(f.events.is_empty());
        }
    }
}

#[test]
fn network_sort_reloads_canonical_backing_after_comparison_reentry() {
    let mut f = Fixture::new();
    let root = f.node(0);
    let a = f.property(root, NET);
    let b = f.property(root, NET);
    let c = f.property(root, NET);
    f.children(root, &[c, a, b]);
    f.relocate_fields_on_compare = Some(root);
    unsafe {
        network(root, f.arrays(), &SERVICES, &NETWORK).unwrap();
        assert!(f.relocate_fields_on_compare.is_none());
        assert_eq!(node(root).fields(), vec![a, b, c]);
        assert_eq!(node(root).reps(), vec![(c, 0), (a, 0), (b, 0)]);
    }
}

