use hp2_abi::engine::generated::{FFrame, TArrayRaw, UObject};
use hp2_core::ansi::Hp2AnsiServices;
use hp2_core::array_storage::Hp2ArrayServices;
use hp2_core::vm_frame::{Hp2VmFrameInitializationServices, Hp2VmFrameServices, Hp2VmNativeTraceServices};
use hp2_core::vm_object::*;
use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};
use hp2_core::vm_state::{Hp2VmStateServices, NAME_NONE};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::{c_char, c_void};
use std::ptr::{null, null_mut};

const ERROR: i32 = 73;
const AUTO: i32 = 9000;
const BEGIN: i32 = 9001;
const PROBE: i32 = 1200;
thread_local! {
    static RAW: Cell<i32> = const { Cell::new(0) };
    static DRAWS: Cell<u32> = const { Cell::new(0) };
    static RNG_FAIL: Cell<bool> = const { Cell::new(false) };
    static TRANSIENT: Cell<*mut c_void> = const { Cell::new(null_mut()) };
    static TRANSIENT_FAIL: Cell<bool> = const { Cell::new(false) };
    static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}
struct Node {
    world: *mut World,
    name: i32,
    parent: *mut c_void,
    children: *mut c_void,
    next: *mut c_void,
    property: i32,
    flags: u32,
    property_flags: u32,
    probe: u64,
    ignore: u64,
    table: u16,
    script: [i32; 4],
    enumeration: i32,
    names: Vec<i32>,
}
impl Node {
    fn new(name: i32) -> Self {
        Self {
            world: null_mut(),
            name,
            parent: null_mut(),
            children: null_mut(),
            next: null_mut(),
            property: 0,
            flags: 0,
            property_flags: 0,
            probe: 0,
            ignore: u64::MAX,
            table: u16::MAX,
            script: [0; 4],
            enumeration: 0,
            names: vec![],
        }
    }
}
struct World {
    context_base: *mut World,
    nodes: Vec<Node>,
    has_frame: i32,
    class: *mut c_void,
    state: *mut c_void,
    node: *mut c_void,
    mask: u64,
    latent: i32,
    code: i32,
    flags: u32,
    fail: &'static str,
    warnings: Vec<(u32, u32, i32)>,
    constructed: Option<(*mut c_void, *mut c_void, i32, u32)>,
    loaded: Option<(*mut c_void, i32, u32)>,
    value: i32,
    exported: usize,
    imported: usize,
    float_result: *mut f32,
    traces: Vec<(i32, u32, u32, u32, u32)>,
    change_super: *mut c_void,
    next_after_type: *mut c_void,
    end_probe: i32,
    preempt: bool,
    enum_casts: usize,
    fail_cast_at: usize,
}
// Raw callback contexts are rooted in UnsafeCell, not a temporary &mut World.
// Nodes occupy a separate allocation so a node callback can borrow its World
// without simultaneously borrowing an enclosing World.nodes array.
struct WorldOwner(Box<UnsafeCell<World>>);
impl std::ops::Deref for WorldOwner {
    type Target = World;
    fn deref(&self) -> &World {
        unsafe { &*self.0.get() }
    }
}
impl std::ops::DerefMut for WorldOwner {
    fn deref_mut(&mut self) -> &mut World {
        unsafe { &mut *self.0.get() }
    }
}
impl World {
    fn new() -> WorldOwner {
        EVENTS.with(|v| v.borrow_mut().clear());
        DRAWS.set(0);
        RAW.set(0);
        RNG_FAIL.set(false);
        TRANSIENT_FAIL.set(false);
        let mut w = WorldOwner(Box::new(UnsafeCell::new(Self {
            context_base: null_mut(),
            nodes: vec![
                Node::new(100),
                Node::new(200),
                Node::new(300),
                Node::new(400),
                Node::new(500),
            ],
            has_frame: 1,
            class: null_mut(),
            state: null_mut(),
            node: null_mut(),
            mask: 0,
            latent: 9,
            code: 55,
            flags: 0,
            fail: "",
            warnings: vec![],
            constructed: None,
            loaded: None,
            value: 42,
            exported: 0,
            imported: 0,
            float_result: null_mut(),
            traces: vec![],
            change_super: null_mut(),
            next_after_type: null_mut(),
            end_probe: 0,
            preempt: false,
            enum_casts: 0,
            fail_cast_at: 0,
        })));
        let p = w.0.get();
        unsafe {
            (*p).context_base = p;
        }
        for n in &mut w.nodes {
            n.world = p;
        }
        w.class = w.ptr(0);
        w.state = w.ptr(1);
        w.node = w.state;
        TRANSIENT.set(w.ptr(4));
        w
    }
    fn ptr(&mut self, i: usize) -> *mut c_void {
        assert!(i < self.nodes.len());
        self.nodes.as_mut_ptr().wrapping_add(i).cast()
    }
    fn context(&mut self) -> *mut c_void {
        self.context_base.cast()
    }
    fn mark(&self, event: &'static str) -> i32 {
        EVENTS.with(|v| v.borrow_mut().push(event));
        if self.fail == event { ERROR } else { 0 }
    }
}
unsafe fn world<'a>(p: *mut c_void) -> &'a mut World {
    unsafe { &mut *p.cast() }
}
unsafe fn node<'a>(p: *mut c_void) -> &'a mut Node {
    unsafe { &mut *p.cast() }
}
fn events() -> Vec<&'static str> {
    EVENTS.with(|v| v.borrow().clone())
}
unsafe extern "C" fn has_frame(p: *mut c_void) -> i32 {
    unsafe { world(p).has_frame }
}
unsafe extern "C" fn clear_latent(p: *mut c_void) {
    unsafe {
        world(p).latent = 0;
    }
}
unsafe extern "C" fn frame_state(p: *mut c_void) -> *mut c_void {
    unsafe { world(p).state }
}
unsafe extern "C" fn object_class(p: *mut c_void) -> *mut c_void {
    unsafe { world(p).class }
}
unsafe extern "C" fn name(p: *mut c_void) -> i32 {
    unsafe { node(p).name }
}
unsafe extern "C" fn find_state(p: *mut c_void, name: i32, out: *mut *mut c_void) -> i32 {
    unsafe {
        let w = world(p);
        let e = w.mark("find_state");
        if e == 0 {
            *out = if name == 300 { w.ptr(2) } else { null_mut() };
        }
        e
    }
}
unsafe extern "C" fn children(p: *mut c_void) -> *mut c_void {
    unsafe { node(p).children }
}
unsafe extern "C" fn next_field(p: *mut c_void) -> *mut c_void {
    unsafe { node(p).next }
}
unsafe extern "C" fn super_node(p: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        let w = node(p).world;
        let e = (*w).mark("super");
        let replacement = (*w).change_super;
        if !replacement.is_null() {
            node(p).parent = replacement;
            (*w).change_super = null_mut();
        }
        if e == 0 {
            *out = node(p).parent;
        }
        e
    }
}
unsafe extern "C" fn is_state(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        *out = i32::from(node(p).property == 0);
        0
    }
}
unsafe extern "C" fn state_flags(p: *mut c_void) -> u32 {
    unsafe { node(p).flags }
}
unsafe extern "C" fn object_flags(p: *mut c_void) -> u32 {
    unsafe { world(p).flags }
}
unsafe extern "C" fn clear_flags(p: *mut c_void, flags: u32) -> i32 {
    unsafe {
        world(p).flags &= !flags;
        world(p).mark("clear_flags")
    }
}
unsafe extern "C" fn set_flags(p: *mut c_void, flags: u32) -> i32 {
    unsafe {
        world(p).flags |= flags;
        world(p).mark("set_flags")
    }
}
unsafe extern "C" fn probing(p: *mut c_void, _: i32, out: *mut i32) -> i32 {
    unsafe {
        *out = world(p).end_probe;
        0
    }
}
unsafe extern "C" fn end_state(p: *mut c_void) -> i32 {
    unsafe {
        let w = world(p);
        if w.preempt {
            w.flags |= 0x1000;
            w.state = w.ptr(4);
        }
        w.mark("end")
    }
}
unsafe extern "C" fn begin_state(p: *mut c_void) -> i32 {
    unsafe { world(p).mark("begin") }
}
unsafe extern "C" fn publish_state(p: *mut c_void, state: *mut c_void) {
    unsafe {
        let w = world(p);
        w.state = state;
        w.node = state;
        w.code = 0;
    }
}
unsafe extern "C" fn probe_mask(p: *mut c_void) -> u64 {
    unsafe { node(p).probe }
}
unsafe extern "C" fn ignore_mask(p: *mut c_void) -> u64 {
    unsafe { node(p).ignore }
}
unsafe extern "C" fn set_mask(p: *mut c_void, mask: u64) {
    unsafe {
        world(p).mask = mask;
    }
}
unsafe extern "C" fn label_table(p: *mut c_void) -> u16 {
    unsafe { node(p).table }
}
unsafe extern "C" fn script_int(p: *mut c_void, offset: i32, out: *mut i32) -> i32 {
    unsafe {
        let e = (&*node(p).world).mark("script");
        if e == 0 {
            *out = node(p).script[offset as usize / 4];
        }
        e
    }
}
unsafe extern "C" fn label_matches(a: i32, b: i32, out: *mut i32) -> i32 {
    unsafe {
        *out = i32::from(a == b);
        0
    }
}
unsafe extern "C" fn set_node(p: *mut c_void, n: *mut c_void) {
    unsafe {
        world(p).node = n;
    }
}
unsafe extern "C" fn set_code(p: *mut c_void, _: *mut c_void, offset: i32) -> i32 {
    unsafe {
        let e = world(p).mark("code");
        if e == 0 {
            world(p).code = offset;
        }
        e
    }
}
unsafe extern "C" fn clear_code(p: *mut c_void) {
    unsafe {
        world(p).code = 0;
    }
}
static STATE: Hp2VmStateServices = Hp2VmStateServices {
    auto_name: AUTO,
    begin_state_name: 9002,
    end_state_name: 9003,
    has_frame,
    clear_latent,
    frame_state,
    object_class,
    name,
    find_state,
    children,
    next_field,
    inheritance_super: super_node,
    is_state,
    state_flags,
    object_flags,
    clear_flags,
    set_flags,
    probing,
    end_state,
    begin_state,
    publish_state,
    probe_mask,
    ignore_mask,
    set_probe_mask: set_mask,
    super_state: super_node,
    label_table,
    script_int,
    label_matches,
    set_node,
    set_code,
    clear_code,
};
unsafe extern "C" fn warning(p: *mut c_void, kind: u32, a: u32, b: i32) -> i32 {
    unsafe {
        world(p).warnings.push((kind, a, b));
        world(p).mark("warning")
    }
}
unsafe extern "C" fn transient(out: *mut *mut c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("transient"));
        if TRANSIENT_FAIL.get() {
            ERROR
        } else {
            *out = TRANSIENT.get();
            0
        }
    }
}
unsafe extern "C" fn construct(
    p: *mut c_void,
    class: *mut c_void,
    outer: *mut c_void,
    name: i32,
    flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let w = world(p);
        w.constructed = Some((class, outer, name, flags));
        let e = w.mark("construct");
        if e == 0 {
            *out = w.ptr(3);
        }
        e
    }
}
unsafe extern "C" fn random(out: *mut i32) -> i32 {
    unsafe {
        DRAWS.set(DRAWS.get() + 1);
        if RNG_FAIL.get() {
            ERROR
        } else {
            *out = RAW.get();
            0
        }
    }
}
unsafe extern "C" fn trace(p: *mut c_void, raw: i32, min: f32, max: f32, value: f32) -> i32 {
    unsafe {
        let w = world(p);
        w.traces.push((
            raw,
            min.to_bits(),
            max.to_bits(),
            value.to_bits(),
            (*w.float_result).to_bits(),
        ));
        w.mark("trace")
    }
}
unsafe extern "C" fn frame_mask(p: *mut c_void) -> u64 {
    unsafe { world(p).mask }
}
unsafe extern "C" fn is_property(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let w = node(p).world;
        let e = (*w).mark("type");
        let replacement = (*w).next_after_type;
        if !replacement.is_null() {
            node(p).next = replacement;
            (*w).next_after_type = null_mut();
        }
        if e == 0 {
            *out = node(p).property;
        }
        e
    }
}
unsafe extern "C" fn field_matches(p: *mut c_void, text: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let e = (&*node(p).world).mark("match");
        if e == 0 {
            *out = if node(p).name == *text.cast::<i32>() {
                -7
            } else {
                0
            };
        }
        e
    }
}
unsafe extern "C" fn field_flags(p: *mut c_void) -> u32 {
    unsafe { node(p).flags }
}
unsafe extern "C" fn property_flags(p: *mut c_void) -> u32 {
    unsafe { node(p).property_flags }
}
unsafe extern "C" fn export_text(p: *mut c_void, _: *mut c_void, scratch: *mut c_void) -> i32 {
    unsafe {
        let w = world(p);
        w.exported += 1;
        *scratch.cast::<i32>() = w.value;
        w.mark("export")
    }
}
unsafe extern "C" fn assign_text(out: *mut c_void, text: *const c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("assign"));
        *out.cast::<i32>() = *text.cast::<i32>();
        0
    }
}
unsafe extern "C" fn import_text(p: *mut c_void, _: *mut c_void, text: *const c_void) -> i32 {
    unsafe {
        let w = world(p);
        w.imported += 1;
        w.value = *text.cast::<i32>();
        w.mark("import")
    }
}
unsafe extern "C" fn is_enum(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        if p.is_null() {
            *out = 0;
            return 0;
        }
        let w = node(p).world;
        (*w).enum_casts += 1;
        let e = (*w).mark("enum_cast");
        if e != 0 {
            return e;
        }
        if (*w).enum_casts == (*w).fail_cast_at {
            return ERROR;
        }
        *out = node(p).enumeration;
        0
    }
}
unsafe extern "C" fn enum_len(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let e = (&*node(p).world).mark("enum_len");
        if e == 0 {
            *out = node(p).names.len() as i32;
        }
        e
    }
}
unsafe extern "C" fn enum_name(p: *mut c_void, i: i32, out: *mut i32) -> i32 {
    unsafe {
        let e = (&*node(p).world).mark("enum_name");
        if e == 0 {
            *out = node(p).names[i as usize];
        }
        e
    }
}
unsafe extern "C" fn load(
    class: *mut c_void,
    text: *const c_void,
    flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let w = &mut *node(class).world;
        w.loaded = Some((class, *text.cast::<i32>(), flags));
        let e = w.mark("load");
        if e == 0 {
            *out = w.ptr(4);
        }
        e
    }
}
unsafe extern "C" fn log_event(_event: i32, _text: *const c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("log_event"));
    0
}
unsafe extern "C" fn localize(
    _section: *const c_void,
    _key: *const c_void,
    _package: *const c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("localize"));
        *result.cast::<i32>() = 321;
    }
    0
}
unsafe extern "C" fn get_language(result: *mut c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("get_language"));
        *result.cast::<i32>() = 654;
    }
    0
}
unsafe extern "C" fn save_config(_object: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("save_config"));
    0
}
unsafe extern "C" fn reset_config(_class: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("reset_config"));
    0
}
unsafe extern "C" fn default_object(class: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("default_object"));
        out.write(class);
    }
    0
}
unsafe extern "C" fn mem_tag_push() -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("mem_tag_push"));
    0
}
unsafe extern "C" fn mem_tag_pop() -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("mem_tag_pop"));
    0
}
fn services() -> Hp2VmObjectServices {
    Hp2VmObjectServices {
        state: &STATE,
        script_mask: 0x125,
        public_flag: 4,
        const_flag: 2,
        load_no_warn: 0x10,
        load_quiet: 0x800,
        probe_min: PROBE,
        probe_max: PROBE + 64,
        begin_name: BEGIN,
        rand_max: i32::MAX as f32,
        script_log_name: 9500,
        warning,
        transient,
        construct,
        random,
        trace_range: trace,
        super_class: super_node,
        super_struct: super_node,
        frame_probe_mask: frame_mask,
        is_property,
        field_matches,
        field_flags,
        property_flags,
        export_text,
        assign_text,
        import_text,
        is_enum,
        enum_len,
        enum_name,
        load,
        log_event,
        localize,
        get_language,
        save_config,
        reset_config,
        mem_tag_push,
        mem_tag_pop,
        default_object,
    }
}
fn property(
    w: &mut WorldOwner,
    set: bool,
    requested: i32,
    value: i32,
    result: &mut i32,
) -> Result<(), i32> {
    let mut scratch = 0_i32;
    unsafe {
        property_text(
            w.context(),
            w.context(),
            (&requested as *const i32).cast(),
            (&value as *const i32).cast(),
            (&mut scratch as *mut i32).cast(),
            (result as *mut i32).cast(),
            set,
            &services(),
        )
    }
}
fn public_property(w: &mut World) {
    w.nodes[0].children = w.ptr(2);
    w.nodes[2].property = -1;
    w.nodes[2].flags = 4;
}

#[test]
fn new_masks_flags_warns_then_resolves_transient_and_constructs_full_width_handles() {
    let mut w = World::new();
    let class = w.ptr(0);
    let mut out = null_mut();
    unsafe {
        new_object(
            w.context(),
            class,
            null_mut(),
            312,
            0xffff,
            &mut out,
            &services(),
        )
        .unwrap();
    }
    assert_eq!(w.warnings, [(0, 0xffff & !0x125, 0)]);
    let outer = w.ptr(4);
    assert_eq!(w.constructed, Some((class, outer, 312, 0x125)));
    assert_eq!(out, w.ptr(3));
    assert_eq!(events(), ["warning", "transient", "construct"]);
}
#[test]
fn new_explicit_outer_skips_transient_and_accepts_null_class() {
    let mut w = World::new();
    let outer = w.ptr(2);
    let mut out = null_mut();
    unsafe {
        new_object(w.context(), null_mut(), outer, 0, 1, &mut out, &services()).unwrap();
    }
    assert_eq!(w.constructed, Some((null_mut(), outer, 0, 1)));
    assert_eq!(events(), ["construct"]);
}
#[test]
fn new_warning_error_preserves_output_and_prevents_construction() {
    let mut w = World::new();
    w.fail = "warning";
    let old = w.ptr(1);
    let mut out = old;
    assert_eq!(
        unsafe {
            new_object(
                w.context(),
                null_mut(),
                null_mut(),
                0,
                0x8000,
                &mut out,
                &services(),
            )
        },
        Err(ERROR)
    );
    assert_eq!(out, old);
    assert_eq!(events(), ["warning"]);
    assert!(w.constructed.is_none());
}
#[test]
fn new_transient_error_stops_before_construction() {
    let mut w = World::new();
    TRANSIENT_FAIL.set(true);
    let old = w.ptr(1);
    let mut out = old;
    assert_eq!(
        unsafe {
            new_object(
                w.context(),
                null_mut(),
                null_mut(),
                0,
                0,
                &mut out,
                &services(),
            )
        },
        Err(ERROR)
    );
    assert_eq!(out, old);
    assert_eq!(events(), ["transient"]);
}
#[test]
fn new_construct_error_preserves_result_after_warning() {
    let mut w = World::new();
    w.fail = "construct";
    let old = w.ptr(1);
    let mut out = old;
    assert_eq!(
        unsafe {
            new_object(
                w.context(),
                null_mut(),
                null_mut(),
                9,
                0xffff,
                &mut out,
                &services(),
            )
        },
        Err(ERROR)
    );
    assert_eq!(out, old);
    assert!(w.constructed.is_some());
    assert_eq!(events(), ["warning", "transient", "construct"]);
}
#[test]
fn range_endpoints_reversed_and_equal_bounds_consume_one_draw_and_publish_before_trace() {
    for (min, max, raw, expected) in [
        (2.0, 6.0, 0, 2.0_f32),
        (2.0, 6.0, i32::MAX, 6.0),
        (6.0, 2.0, 1 << 30, 4.0),
        (3.0, 3.0, 17, 3.0),
    ] {
        let mut w = World::new();
        RAW.set(raw);
        let mut out = 99.0;
        let result = std::ptr::addr_of_mut!(out);
        w.float_result = result;
        unsafe {
            rand_range(w.context(), min, max, result, &services()).unwrap();
        }
        assert_eq!(out.to_bits(), expected.to_bits());
        assert_eq!(DRAWS.get(), 1);
        assert_eq!(
            w.traces,
            [(
                raw,
                min.to_bits(),
                max.to_bits(),
                expected.to_bits(),
                expected.to_bits()
            )]
        );
    }
}
#[test]
fn range_rng_error_keeps_result_and_skips_trace() {
    let mut w = World::new();
    RNG_FAIL.set(true);
    let mut out = 99.0;
    let result = std::ptr::addr_of_mut!(out);
    w.float_result = result;
    assert_eq!(
        unsafe { rand_range(w.context(), 1.0, 4.0, result, &services()) },
        Err(ERROR)
    );
    assert_eq!(out, 99.0);
    assert_eq!(DRAWS.get(), 1);
    assert!(w.traces.is_empty());
}
#[test]
fn range_trace_error_keeps_published_value_without_redraw() {
    let mut w = World::new();
    w.fail = "trace";
    RAW.set(1 << 30);
    let mut out = 99.0;
    let result = std::ptr::addr_of_mut!(out);
    w.float_result = result;
    assert_eq!(
        unsafe { rand_range(w.context(), 2.0, 6.0, result, &services()) },
        Err(ERROR)
    );
    assert_eq!(out, 4.0);
    assert_eq!(DRAWS.get(), 1);
    assert_eq!(w.traces[0].4, 4.0_f32.to_bits());
}
#[test]
fn range_nan_and_infinite_bounds_are_not_clamped_or_retried() {
    let mut w = World::new();
    let mut out = 0.0;
    let result = std::ptr::addr_of_mut!(out);
    w.float_result = result;
    unsafe {
        rand_range(
            w.context(),
            f32::INFINITY,
            f32::INFINITY,
            result,
            &services(),
        )
        .unwrap();
    }
    assert!(out.is_nan());
    assert_eq!(DRAWS.get(), 1);
    assert_eq!(w.traces.len(), 1);
}
#[test]
fn child_walk_accepts_identity_and_live_inherited_parent_and_rejects_nulls() {
    let mut w = World::new();
    let a = w.ptr(0);
    let b = w.ptr(1);
    w.change_super = b;
    let mut out = 99;
    unsafe {
        class_child(a, b, &mut out, &services()).unwrap();
    }
    assert_eq!(out, 1);
    unsafe {
        class_child(a, a, &mut out, &services()).unwrap();
    }
    assert_eq!(out, 1);
    unsafe {
        class_child(a, null_mut(), &mut out, &services()).unwrap();
    }
    assert_eq!(out, 0);
    unsafe {
        class_child(null_mut(), b, &mut out, &services()).unwrap();
    }
    assert_eq!(out, 0);
}
#[test]
fn child_super_error_preserves_output_and_live_mutation() {
    let mut w = World::new();
    w.fail = "super";
    let a = w.ptr(0);
    let b = w.ptr(1);
    w.change_super = b;
    let mut out = 99;
    assert_eq!(
        unsafe { class_child(a, b, &mut out, &services()) },
        Err(ERROR)
    );
    assert_eq!(out, 99);
    assert_eq!(w.nodes[0].parent, b);
    assert_eq!(events(), ["super"]);
}
#[test]
fn goto_default_name_distinguishes_class_sentinel_and_missing_frame() {
    let mut w = World::new();
    assert_eq!(unsafe { current_state_name(w.context(), &services()) }, 200);
    w.state = w.class;
    assert_eq!(unsafe { current_state_name(w.context(), &services()) }, 0);
    w.has_frame = 0;
    assert_eq!(unsafe { current_state_name(w.context(), &services()) }, 0);
}
#[test]
fn goto_same_snapshot_skips_transition_but_uses_live_state_and_compiled_begin_label() {
    let mut w = World::new();
    w.state = w.ptr(2);
    w.nodes[2].table = 0;
    w.nodes[2].script = [BEGIN, 27, 0, 0];
    unsafe {
        goto_state(w.context(), w.context(), 200, 200, 0, &services()).unwrap();
    }
    let expected = w.ptr(2);
    assert_eq!(w.code, 27);
    assert_eq!(w.node, expected);
    assert_eq!(w.latent, 0);
    assert!(!events().contains(&"find_state"));
    assert!(w.warnings.is_empty());
}
#[test]
fn goto_explicit_missing_label_warns_after_clearing_code() {
    let mut w = World::new();
    w.fail = "warning";
    assert_eq!(
        unsafe { goto_state(w.context(), w.context(), 200, 200, 456, &services()) },
        Err(ERROR)
    );
    assert_eq!(w.code, 0);
    assert_eq!(w.warnings, [(1, 200, 456)]);
}
#[test]
fn goto_implicit_missing_begin_label_is_silent() {
    let mut w = World::new();
    unsafe {
        goto_state(w.context(), w.context(), 200, 200, 0, &services()).unwrap();
    }
    assert_eq!(w.code, 0);
    assert!(w.warnings.is_empty());
}
#[test]
fn goto_unknown_state_warns_after_state_publication_but_none_and_auto_are_silent() {
    for requested in [999, 0, AUTO] {
        let mut w = World::new();
        unsafe {
            goto_state(w.context(), w.context(), 200, requested, 321, &services()).unwrap();
        }
        assert_eq!(w.state, w.class);
        assert_eq!(w.code, 0);
        if requested == 999 {
            assert_eq!(w.warnings, [(2, 999, 321)]);
        } else {
            assert!(w.warnings.is_empty());
        }
    }
}
#[test]
fn goto_preempted_transition_never_moves_label_or_warns() {
    let mut w = World::new();
    w.end_probe = -1;
    w.preempt = true;
    unsafe {
        goto_state(w.context(), w.context(), 200, 300, 456, &services()).unwrap();
    }
    let expected = w.ptr(4);
    assert_eq!(w.state, expected);
    assert_eq!(w.code, 55);
    assert!(w.warnings.is_empty());
    assert!(!events().contains(&"script"));
}
#[test]
fn goto_find_failure_preserves_state_after_latent_clear() {
    let mut w = World::new();
    w.fail = "find_state";
    let old = w.state;
    assert_eq!(
        unsafe { goto_state(w.context(), w.context(), 200, 300, 456, &services()) },
        Err(ERROR)
    );
    assert_eq!(w.latent, 0);
    assert_eq!(w.state, old);
    assert_eq!(w.code, 55);
    assert!(w.warnings.is_empty());
}
#[test]
fn goto_label_code_failure_preserves_published_node_and_transition() {
    let mut w = World::new();
    w.nodes[2].table = 0;
    w.nodes[2].script = [456, 27, 0, 0];
    w.fail = "code";
    assert_eq!(
        unsafe { goto_state(w.context(), w.context(), 200, 300, 456, &services()) },
        Err(ERROR)
    );
    let expected = w.ptr(2);
    assert_eq!(w.state, expected);
    assert_eq!(w.node, w.state);
    assert_eq!(w.code, 0);
    assert!(w.warnings.is_empty());
}
#[test]
fn enable_masks_inherited_probe_bits_with_ignore_mask_and_preserves_other_bits() {
    let mut w = World::new();
    w.nodes[0].probe = 1_u64 << 63;
    w.nodes[1].probe = 3;
    w.nodes[1].ignore = !2;
    w.mask = 4;
    for name in [PROBE, PROBE + 1, PROBE + 63] {
        unsafe {
            probe(w.context(), w.context(), name, true, &services()).unwrap();
        }
    }
    assert_eq!(w.mask, (1_u64 << 63) | 5);
    assert!(w.warnings.is_empty());
}
#[test]
fn disable_clears_only_target_even_if_not_in_base_masks() {
    let mut w = World::new();
    w.mask = u64::MAX;
    unsafe {
        probe(w.context(), w.context(), PROBE + 63, false, &services()).unwrap();
    }
    assert_eq!(w.mask, u64::MAX >> 1);
    assert!(w.warnings.is_empty());
}
#[test]
fn probe_boundaries_and_missing_frame_warn_without_mutating() {
    for (name, frame) in [(PROBE - 1, 1), (PROBE + 64, 1), (PROBE, 0)] {
        for enable in [true, false] {
            let mut w = World::new();
            w.has_frame = frame;
            w.mask = 123;
            w.fail = "warning";
            assert_eq!(
                unsafe { probe(w.context(), w.context(), name, enable, &services()) },
                Err(ERROR)
            );
            assert_eq!(w.mask, 123);
            assert_eq!(w.warnings, [(3, name as u32, 0)]);
        }
    }
}
#[test]
fn property_get_walks_live_inherited_fields_and_honors_nonzero_type_match() {
    let mut w = World::new();
    w.nodes[0].parent = w.ptr(1);
    w.nodes[1].children = w.ptr(2);
    w.nodes[2].next = w.ptr(3);
    w.nodes[3].property = -1;
    w.nodes[3].flags = 4;
    let mut out = 99;
    property(&mut w, false, 400, 0, &mut out).unwrap();
    assert_eq!(out, 42);
    assert_eq!(w.exported, 1);
    assert_eq!(
        events(),
        ["super", "type", "type", "match", "export", "assign"]
    );
}
#[test]
fn property_search_observes_next_field_changed_by_type_callback() {
    let mut w = World::new();
    w.nodes[0].children = w.ptr(2);
    w.next_after_type = w.ptr(4);
    w.nodes[4].property = 1;
    w.nodes[4].flags = 4;
    let mut out = 99;
    property(&mut w, false, 500, 0, &mut out).unwrap();
    assert_eq!(out, 42);
    assert_eq!(w.exported, 1);
}
#[test]
fn property_first_private_match_shadows_public_parent_and_clears_get_output() {
    let mut w = World::new();
    public_property(&mut w);
    w.nodes[2].flags = 0;
    w.nodes[0].parent = w.ptr(1);
    w.nodes[1].children = w.ptr(3);
    w.nodes[3].name = 300;
    w.nodes[3].property = 1;
    w.nodes[3].flags = 4;
    let mut out = 99;
    property(&mut w, false, 300, 0, &mut out).unwrap();
    assert_eq!(out, 0);
    assert_eq!(w.exported, 0);
    assert!(!events().contains(&"super"));
}
#[test]
fn property_missing_get_clears_but_missing_set_never_changes_value() {
    let mut w = World::new();
    let mut out = 99;
    property(&mut w, false, 999, 0, &mut out).unwrap();
    assert_eq!(out, 0);
    property(&mut w, true, 999, 123, &mut out).unwrap();
    assert_eq!(w.value, 42);
    assert_eq!(w.imported, 0);
}
#[test]
fn property_set_checks_public_and_const_flags_without_importing() {
    for (flags, constant) in [(0, 0), (4, 2)] {
        let mut w = World::new();
        public_property(&mut w);
        w.nodes[2].flags = flags;
        w.nodes[2].property_flags = constant;
        let mut out = 99;
        property(&mut w, true, 300, 123, &mut out).unwrap();
        assert_eq!(w.value, 42);
        assert_eq!(w.imported, 0);
        assert_eq!(out, 99);
    }
}
#[test]
fn property_import_failure_retains_partial_mutation_and_does_not_assign_result() {
    let mut w = World::new();
    public_property(&mut w);
    w.fail = "import";
    let mut out = 99;
    assert_eq!(property(&mut w, true, 300, 123, &mut out), Err(ERROR));
    assert_eq!(w.value, 123);
    assert_eq!(out, 99);
    assert_eq!(w.imported, 1);
}
#[test]
fn property_export_failure_keeps_consumer_result_unchanged() {
    let mut w = World::new();
    public_property(&mut w);
    w.fail = "export";
    let mut out = 99;
    assert_eq!(property(&mut w, false, 300, 0, &mut out), Err(ERROR));
    assert_eq!(out, 99);
    assert_eq!(w.exported, 1);
    assert!(!events().contains(&"assign"));
}
#[test]
fn property_type_match_and_inheritance_failures_stop_at_first_error() {
    for fail in ["type", "match", "super"] {
        let mut w = World::new();
        public_property(&mut w);
        w.fail = fail;
        let mut out = 99;
        assert_eq!(property(&mut w, false, 999, 0, &mut out), Err(ERROR));
        assert_eq!(out, 99);
        assert_eq!(events().last(), Some(&fail));
        assert_eq!(w.exported, 0);
    }
}
#[test]
fn enum_bounds_null_type_and_nonzero_cast_publish_expected_names() {
    let mut w = World::new();
    w.nodes[2].enumeration = -1;
    w.nodes[2].names = vec![123, 456];
    for (index, expected) in [(-1, 0), (0, 123), (1, 456), (2, 0), (i32::MAX, 0)] {
        let mut out = 99;
        unsafe {
            get_enum(w.ptr(2), index, &mut out, &services()).unwrap();
        }
        assert_eq!(out, expected);
    }
    let mut out = 99;
    unsafe {
        get_enum(null_mut(), 0, &mut out, &services()).unwrap();
    }
    assert_eq!(out, 0);
    out = 99;
    unsafe {
        get_enum(w.ptr(0), 0, &mut out, &services()).unwrap();
    }
    assert_eq!(out, 0);
}
#[test]
fn enum_cast_and_element_errors_leave_none_already_published() {
    for fail in ["enum_cast", "enum_len", "enum_name"] {
        let mut w = World::new();
        w.nodes[2].enumeration = 1;
        w.nodes[2].names = vec![123];
        w.fail = fail;
        let mut out = 99;
        assert_eq!(
            unsafe { get_enum(w.ptr(2), 0, &mut out, &services()) },
            Err(ERROR)
        );
        assert_eq!(out, 0);
        assert_eq!(events().last(), Some(&fail));
    }
}
#[test]
fn dynamic_load_uses_compiled_flags_and_all_nonzero_boolean_values() {
    for may_fail in [0, 1, 0x8000_0000, u32::MAX] {
        let mut w = World::new();
        let class = w.ptr(0);
        let text = 123_i32;
        let mut out = null_mut();
        unsafe {
            dynamic_load(
                class,
                (&text as *const i32).cast(),
                may_fail,
                &mut out,
                &services(),
            )
            .unwrap();
        }
        assert_eq!(
            w.loaded,
            Some((class, text, if may_fail == 0 { 0x10 } else { 0x810 }))
        );
        assert_eq!(out, w.ptr(4));
    }
}
#[test]
fn dynamic_load_error_keeps_old_result_and_does_not_retry() {
    let mut w = World::new();
    w.fail = "load";
    let text = 123_i32;
    let old = w.ptr(1);
    let mut out = old;
    assert_eq!(
        unsafe {
            dynamic_load(
                w.ptr(0),
                (&text as *const i32).cast(),
                9,
                &mut out,
                &services(),
            )
        },
        Err(ERROR)
    );
    assert_eq!(out, old);
    assert_eq!(events(), ["load"]);
}
#[test]
fn state_and_class_queries_follow_live_inheritance_and_normalize_boolean_results() {
    for op in [Hp2VmObjectQuery::IsInState, Hp2VmObjectQuery::IsA] {
        let mut w = World::new();
        w.change_super = w.ptr(3);
        let mut out = 99;
        unsafe {
            query(op, w.context(), 400, &mut out, &services()).unwrap();
        }
        assert_eq!(out, 1);
        unsafe {
            query(op, w.context(), 999, &mut out, &services()).unwrap();
        }
        assert_eq!(out, 0);
    }
}
#[test]
fn get_state_name_returns_class_name_but_current_default_uses_none() {
    let mut w = World::new();
    w.state = w.class;
    let mut out = 99;
    unsafe {
        query(
            Hp2VmObjectQuery::GetStateName,
            w.context(),
            0,
            &mut out,
            &services(),
        )
        .unwrap();
    }
    assert_eq!(out, 100);
    assert_eq!(unsafe { current_state_name(w.context(), &services()) }, 0);
}
#[test]
fn missing_or_null_state_frame_returns_none_and_false_without_traversal() {
    for missing in [true, false] {
        let mut w = World::new();
        if missing {
            w.has_frame = 0;
        } else {
            w.state = null_mut();
        }
        for op in [Hp2VmObjectQuery::GetStateName, Hp2VmObjectQuery::IsInState] {
            let mut out = 99;
            unsafe {
                query(op, w.context(), 200, &mut out, &services()).unwrap();
            }
            assert_eq!(out, 0);
        }
        assert!(events().is_empty());
    }
}
#[test]
fn query_super_failure_keeps_result_and_stops_after_live_mutation() {
    for op in [Hp2VmObjectQuery::IsInState, Hp2VmObjectQuery::IsA] {
        let mut w = World::new();
        w.fail = "super";
        w.change_super = w.ptr(4);
        let mut out = 99;
        assert_eq!(
            unsafe { query(op, w.context(), 999, &mut out, &services()) },
            Err(ERROR)
        );
        assert_eq!(out, 99);
        assert_eq!(events(), ["super"]);
    }
}

#[test]
fn range_fused_cancellation_retains_the_exact_negative_residual() {
    let mut w = World::new();
    RAW.set(2_147_483_392);
    let mut out = 99.0;
    let result = std::ptr::addr_of_mut!(out);
    w.float_result = result;
    unsafe {
        rand_range(
            w.context(),
            -1.0,
            f32::from_bits(0x3400_0000),
            result,
            &services(),
        )
        .unwrap();
    }
    // (1-2^-23)*(1+2^-23)-1 = -2^-46; separate mul/add would round to zero.
    assert_eq!(out.to_bits(), 0xa880_0000);
    assert_eq!(DRAWS.get(), 1);
    assert_eq!(w.traces[0].4, 0xa880_0000);
}

#[test]
fn enum_repeated_cast_failures_keep_none_and_stop_before_element_access() {
    for attempt in [2, 3] {
        let mut w = World::new();
        w.nodes[2].enumeration = -1;
        w.nodes[2].names = vec![123];
        w.fail_cast_at = attempt;
        let mut out = 99;
        assert_eq!(
            unsafe { get_enum(w.ptr(2), 0, &mut out, &services()) },
            Err(ERROR)
        );
        assert_eq!(out, 0);
        assert_eq!(w.enum_casts, attempt);
        assert!(!events().contains(&"enum_name"));
    }
}

#[test]
fn goto_success_publishes_requested_state_probe_mask_and_label() {
    let mut w = World::new();
    w.nodes[0].probe = 4;
    w.nodes[2].probe = 3;
    w.nodes[2].ignore = !2;
    w.nodes[2].table = 0;
    w.nodes[2].script = [456, 31, 0, 0];
    unsafe {
        goto_state(w.context(), w.context(), 200, 300, 456, &services()).unwrap();
    }
    let expected = w.ptr(2);
    assert_eq!(w.state, expected);
    assert_eq!(w.node, expected);
    assert_eq!(w.code, 31);
    assert_eq!(w.mask, 5);
    assert_eq!(w.latent, 0);
    assert_ne!(w.flags & 0x1000, 0);
    assert!(w.warnings.is_empty());
}

#[test]
fn goto_auto_uses_compiled_id_and_inherited_live_state_lookup() {
    let mut w = World::new();
    w.nodes[0].parent = w.ptr(3);
    w.nodes[3].children = w.ptr(2);
    w.nodes[2].flags = 2;
    w.nodes[2].table = 0;
    w.nodes[2].script = [BEGIN, 37, 0, 0];
    unsafe {
        goto_state(w.context(), w.context(), 200, AUTO, 0, &services()).unwrap();
    }
    let expected = w.ptr(2);
    assert_eq!(w.state, expected);
    assert_eq!(w.code, 37);
    assert!(!events().contains(&"find_state"));
    assert!(w.warnings.is_empty());
}

#[test]
fn property_public_writable_import_changes_object_without_touching_result() {
    let mut w = World::new();
    public_property(&mut w);
    let mut out = 99;
    property(&mut w, true, 300, -123, &mut out).unwrap();
    assert_eq!(w.value, -123);
    assert_eq!(w.imported, 1);
    assert_eq!(out, 99);
    assert_eq!(events(), ["type", "match", "import"]);
}

// Frame-level operand/trace harness for execute_class_is_child_of, execute_is_a
// and execute_rand_range: wraps the plain leaves above (class_child, query,
// rand_range) with FFrame operand stepping and, for RandRange, a native trace
// scope. Mirrors vm_scalar_contracts.rs's RandInvocation harness: `frame` is
// the first field so a `*mut FFrame` doubles as a back-pointer to the struct.
#[repr(C)]
struct ObjectInvocation {
    frame: FFrame,
    code: [u8; 8],
    table: [Hp2VmNativeHandler; NATIVE_COUNT],
    operand_a: u64,
    operand_b: u64,
    operand_c: u64,
    operand_d: u64,
    events: Vec<&'static str>,
    scope_native: String,
    scope_native_slot: i32,
    scope_callsite_offset: i32,
    scope_begin_error: i32,
    scope_end_error: i32,
}
impl ObjectInvocation {
    fn new(code: [u8; 8], operand_a: u64, operand_b: u64) -> Self {
        Self::new4(code, operand_a, operand_b, 0, 0)
    }
    fn new4(code: [u8; 8], operand_a: u64, operand_b: u64, operand_c: u64, operand_d: u64) -> Self {
        Self {
            frame: FFrame {
                node: null_mut(),
                object: null_mut(),
                code: null_mut(),
                locals: null_mut(),
                parent: null_mut(),
            },
            code,
            table: [object_unexpected; NATIVE_COUNT],
            operand_a,
            operand_b,
            operand_c,
            operand_d,
            events: Vec::new(),
            scope_native: String::new(),
            scope_native_slot: 0,
            scope_callsite_offset: 0,
            scope_begin_error: 0,
            scope_end_error: 0,
        }
    }
    fn prepare(&mut self) {
        self.frame.code = self.code.as_mut_ptr();
        self.frame.node = (self as *mut Self).cast();
    }
}
unsafe extern "C" fn object_unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}
unsafe extern "C" fn object_operand_ptr_a(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("class_operand");
        result
            .cast::<*mut c_void>()
            .write((*p).operand_a as *mut c_void);
        0
    }
}
unsafe extern "C" fn object_operand_ptr_b(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("parent_operand");
        result
            .cast::<*mut c_void>()
            .write((*p).operand_b as *mut c_void);
        0
    }
}
unsafe extern "C" fn object_operand_i32(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("operand");
        result.cast::<i32>().write((*p).operand_a as i32);
        0
    }
}
unsafe extern "C" fn object_operand_i32_c(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("name_operand");
        result.cast::<i32>().write((*p).operand_c as i32);
        0
    }
}
unsafe extern "C" fn object_operand_i32_d(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("flags_operand");
        result.cast::<i32>().write((*p).operand_d as i32);
        0
    }
}
// Simulates a compiled EX_Nothing/omitted-optional-parameter opcode: the
// handler never touches `result`, so the caller's pre-seeded default survives.
unsafe extern "C" fn object_operand_absent(
    _: *mut c_void,
    frame: *mut c_void,
    _: *mut c_void,
) -> i32 {
    unsafe {
        (*frame.cast::<ObjectInvocation>())
            .events
            .push("absent_operand");
    }
    0
}
unsafe extern "C" fn object_operand_f32_min(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("min_operand");
        result
            .cast::<f32>()
            .write(f32::from_bits((*p).operand_a as u32));
        0
    }
}
unsafe extern "C" fn object_operand_f32_max(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("max_operand");
        result
            .cast::<f32>()
            .write(f32::from_bits((*p).operand_b as u32));
        0
    }
}
unsafe extern "C" fn object_object_class(_: *mut c_void) -> *mut c_void {
    panic!("object_class must not be called by callsite_offset");
}
unsafe extern "C" fn object_script_code(node: *mut c_void, offset: i32, out: *mut *mut u8) -> i32 {
    unsafe {
        let p = node.cast::<ObjectInvocation>();
        (*p).events.push("script_code");
        assert_eq!(offset, 0);
        out.write((*p).code.as_mut_ptr());
        0
    }
}
unsafe extern "C" fn object_scope_begin(
    stack: *mut c_void,
    native: *const c_char,
    native_slot: i32,
    callsite_offset: i32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let p = stack.cast::<ObjectInvocation>();
        (*p).events.push("scope_begin");
        (*p).scope_native = std::ffi::CStr::from_ptr(native)
            .to_str()
            .unwrap()
            .to_string();
        (*p).scope_native_slot = native_slot;
        (*p).scope_callsite_offset = callsite_offset;
        out.write(stack);
        (*p).scope_begin_error
    }
}
unsafe extern "C" fn object_scope_end(handle: *mut c_void) -> i32 {
    unsafe {
        let p = handle.cast::<ObjectInvocation>();
        (*p).events.push("scope_end");
        (*p).scope_end_error
    }
}
fn object_trace_services() -> Hp2VmNativeTraceServices {
    Hp2VmNativeTraceServices {
        frame: Hp2VmFrameInitializationServices {
            object_class: object_object_class,
            script_code: object_script_code,
        },
        scope_begin: object_scope_begin,
        scope_end: object_scope_end,
    }
}

#[test]
fn execute_class_is_child_of_steps_class_then_parent_and_publishes_the_child_check() {
    for (class, parent, expected) in [
        (0x10usize as *mut c_void, 0x10usize as *mut c_void, 1u32),
        (null_mut(), 0x20usize as *mut c_void, 0u32),
    ] {
        let mut invocation = ObjectInvocation::new(
            [0x80, 0x81, 0x16, 0x42, 0, 0, 0, 0],
            class as u64,
            parent as u64,
        );
        invocation.table[0x80] = object_operand_ptr_a;
        invocation.table[0x81] = object_operand_ptr_b;
        invocation.prepare();
        let mut result = 0xAAAA_AAAA_u32;
        let p = &raw mut invocation;
        let status = unsafe {
            execute_class_is_child_of(
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &services(),
            )
        };
        assert_eq!(status, Ok(()));
        assert_eq!(result, expected);
        assert_eq!(invocation.events, ["class_operand", "parent_operand"]);
        assert_eq!(
            invocation.frame.code,
            invocation.code.as_mut_ptr().wrapping_add(3)
        );
    }
}

// IsA's native leaf context is `FVmStateContext { Object; RequestedName }`, not
// a World: its first field is the handler's own `object` argument. Reuse the
// existing STATE leaf table (name/super_state walk live World nodes exactly
// like state_and_class_queries_follow_live_inheritance...), only object_class
// differs, since its receiver is this raw context rather than a World/Node.
fn context_state() -> Hp2VmStateServices {
    Hp2VmStateServices {
        auto_name: AUTO,
        begin_state_name: 9002,
        end_state_name: 9003,
        has_frame,
        clear_latent,
        frame_state,
        object_class: context_object_class,
        name,
        find_state,
        children,
        next_field,
        inheritance_super: super_node,
        is_state,
        state_flags,
        object_flags,
        clear_flags,
        set_flags,
        probing,
        end_state,
        begin_state,
        publish_state,
        probe_mask,
        ignore_mask,
        set_probe_mask: set_mask,
        super_state: super_node,
        label_table,
        script_int,
        label_matches,
        set_node,
        set_code,
        clear_code,
    }
}
thread_local! {
    static ISA_OBSERVED_OBJECT: Cell<*mut c_void> = const { Cell::new(null_mut()) };
    static ISA_NODE: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}
unsafe extern "C" fn context_object_class(p: *mut c_void) -> *mut c_void {
    // Record the observed identity directly instead of continuing traversal on
    // it: a regression that fed frame.object through here would otherwise
    // dereference a decoy pointer deep in the World/Node walk and abort the
    // whole test binary with a segfault instead of failing this assertion.
    let observed = unsafe { p.cast::<*mut c_void>().read() };
    ISA_OBSERVED_OBJECT.with(|c| c.set(observed));
    ISA_NODE.with(|c| c.get())
}

#[test]
fn execute_is_a_steps_the_class_name_operand_and_uses_the_handler_object_not_frame_object() {
    let mut w = World::new();
    let node = w.ptr(0);
    ISA_NODE.with(|c| c.set(node));
    let state_mock = context_state();
    let s = Hp2VmObjectServices {
        state: &state_mock,
        ..services()
    };
    for (class_name, expected) in [(100, 1), (999, 0)] {
        let mut invocation =
            ObjectInvocation::new([0x80, 0x16, 0x42, 0, 0, 0, 0, 0], class_name as u64, 0);
        invocation.table[0x80] = object_operand_i32;
        invocation.prepare();
        // Stack.Object: a decoy that must never reach the query as identity.
        invocation.frame.object = 0x11usize as *mut c_void;
        let object = 0x55usize as *mut c_void;
        let mut result = 0x1234_5678_i32;
        let p = &raw mut invocation;
        let status = unsafe {
            execute_is_a(
                object,
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &s,
            )
        };
        assert_eq!(status, Ok(()));
        assert_eq!(result, expected);
        assert_eq!(invocation.events, ["operand"]);
        assert_eq!(
            invocation.frame.code,
            invocation.code.as_mut_ptr().wrapping_add(2)
        );
        assert_eq!(ISA_OBSERVED_OBJECT.with(|c| c.get()), object);
        assert_ne!(object, invocation.frame.object);
    }
}

#[derive(Default)]
struct RangeCtx {
    raw: i32,
    rand_error: i32,
    trace_error: i32,
    traced: Option<(usize, i32, u32, u32, u32)>,
    events: Vec<&'static str>,
}
thread_local! {
    static RANGE_CTX: RefCell<RangeCtx> = RefCell::new(RangeCtx::default());
}
unsafe extern "C" fn context_random(out: *mut i32) -> i32 {
    RANGE_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.events.push("rand");
        unsafe { out.write(c.raw) };
        c.rand_error
    })
}
unsafe extern "C" fn context_trace_range(
    p: *mut c_void,
    raw: i32,
    min: f32,
    max: f32,
    value: f32,
) -> i32 {
    RANGE_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.events.push("trace");
        // FVmObjectContext's first field is FVmStateContext.Object: the
        // handler's own `object` argument, not Stack.Object.
        let object = unsafe { p.cast::<*mut c_void>().read() as usize };
        c.traced = Some((object, raw, min.to_bits(), max.to_bits(), value.to_bits()));
        c.trace_error
    })
}
fn range_services() -> Hp2VmObjectServices {
    Hp2VmObjectServices {
        random: context_random,
        trace_range: context_trace_range,
        rand_max: i32::MAX as f32,
        ..services()
    }
}

#[test]
fn execute_rand_range_offsets_before_operands_and_orders_draw_before_end_errors() {
    for (rand_error, trace_error, end_error, ctx_events, status) in [
        (0, 0, 0, vec!["rand", "trace"], Ok(())),
        (7, 0, 0, vec!["rand"], Err(7)),
        (0, 0, 9, vec!["rand", "trace"], Err(9)),
        // Both the leaf draw and the scope close fail: the draw error (the
        // same `rand` leaf as above) still wins, and scope_end still runs.
        (7, 0, 9, vec!["rand"], Err(7)),
        // The draw itself succeeds but its own trace_range leaf fails after
        // the result is already published; the scope still closes.
        (0, 11, 0, vec!["rand", "trace"], Err(11)),
        (0, 11, 9, vec!["rand", "trace"], Err(11)),
    ] {
        let mut invocation = ObjectInvocation::new(
            [0x80, 0x81, 0x16, 0x42, 0, 0, 0, 0],
            (2.0f32).to_bits() as u64,
            (6.0f32).to_bits() as u64,
        );
        invocation.table[0x80] = object_operand_f32_min;
        invocation.table[0x81] = object_operand_f32_max;
        invocation.scope_end_error = end_error;
        invocation.prepare();
        // Stack.Object: a decoy that must never be read as the leaf identity.
        invocation.frame.object = 0x11usize as *mut c_void;
        RANGE_CTX.with(|c| {
            *c.borrow_mut() = RangeCtx {
                raw: 123,
                rand_error,
                trace_error,
                ..Default::default()
            }
        });
        let trace = object_trace_services();
        let s = range_services();
        let object = 0x55usize as *mut c_void;
        let mut result = 99.0_f32;
        let p = &raw mut invocation;
        let status_got = unsafe {
            execute_rand_range(
                object,
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &s,
                &trace,
            )
        };
        assert_eq!(status_got, status);
        assert_eq!(
            invocation.events,
            [
                "script_code",
                "min_operand",
                "max_operand",
                "scope_begin",
                "scope_end",
            ]
        );
        assert_eq!(
            invocation.frame.code,
            invocation.code.as_mut_ptr().wrapping_add(3)
        );
        assert_eq!(invocation.scope_native, "Core.Object.RandRange");
        assert_eq!(invocation.scope_native_slot, 1033);
        assert_eq!(invocation.scope_callsite_offset, 0);
        RANGE_CTX.with(|c| assert_eq!(c.borrow().events, ctx_events));
        if rand_error == 0 {
            assert_ne!(result.to_bits(), 99.0_f32.to_bits());
            RANGE_CTX.with(|c| {
                assert_eq!(c.borrow().traced.unwrap().0, object as usize);
            });
        } else {
            assert_eq!(result, 99.0);
        }
    }
}

// Frame-level coverage for execute_new: Outer/Name/Flags/Class are optional,
// defaulting to `GetIndex()!=INDEX_NONE ? this : NULL`/None/0/NULL, and New
// has no P_FINISH in the native opcode stream.
thread_local! {
    static NEW_CONSTRUCTED: Cell<Option<(*mut c_void, *mut c_void, i32, u32)>> =
        const { Cell::new(None) };
}
unsafe extern "C" fn new_frame_construct(
    _context: *mut c_void,
    class: *mut c_void,
    outer: *mut c_void,
    name: i32,
    flags: u32,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        NEW_CONSTRUCTED.with(|c| c.set(Some((class, outer, name, flags))));
        EVENTS.with(|v| v.borrow_mut().push("construct"));
        out.write(0x9000usize as *mut c_void);
    }
    0
}
unsafe extern "C" fn new_frame_warning(_: *mut c_void, _: u32, _: u32, _: i32) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("warning"));
    0
}
unsafe extern "C" fn new_frame_transient(out: *mut *mut c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("transient"));
        out.write(0x7000usize as *mut c_void);
    }
    0
}
fn new_frame_services() -> Hp2VmObjectServices {
    Hp2VmObjectServices {
        construct: new_frame_construct,
        warning: new_frame_warning,
        transient: new_frame_transient,
        script_mask: u32::MAX,
        ..services()
    }
}
unsafe extern "C" fn new_outer_operand(_: *mut c_void, frame: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("outer_operand");
        result
            .cast::<*mut c_void>()
            .write((*p).operand_a as *mut c_void);
        0
    }
}
unsafe extern "C" fn new_class_operand(_: *mut c_void, frame: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        let p = frame.cast::<ObjectInvocation>();
        (*p).events.push("class_operand");
        result
            .cast::<*mut c_void>()
            .write((*p).operand_b as *mut c_void);
        0
    }
}

#[test]
fn execute_new_decodes_outer_name_flags_class_in_order_without_p_finish() {
    let mut object = unsafe { std::mem::zeroed::<UObject>() };
    object.index = 7;
    let object_ptr = (&raw mut object).cast::<c_void>();
    let outer_ptr = 0x111usize as *mut c_void;
    let class_ptr = 0x222usize as *mut c_void;
    let mut invocation = ObjectInvocation::new4(
        [0x80, 0x81, 0x82, 0x83, 0, 0, 0, 0],
        outer_ptr as u64,
        class_ptr as u64,
        99,
        0x20,
    );
    invocation.table[0x80] = new_outer_operand;
    invocation.table[0x81] = object_operand_i32_c;
    invocation.table[0x82] = object_operand_i32_d;
    invocation.table[0x83] = new_class_operand;
    invocation.prepare();
    let s = new_frame_services();
    let mut result: *mut c_void = null_mut();
    let p = &raw mut invocation;
    let status = unsafe {
        execute_new(
            object_ptr,
            std::ptr::addr_of_mut!((*p).frame),
            &raw mut result,
            std::ptr::addr_of!((*p).table).cast(),
            &s,
        )
    };
    assert_eq!(status, Ok(()));
    assert_eq!(result, 0x9000usize as *mut c_void);
    assert_eq!(
        invocation.events,
        ["outer_operand", "name_operand", "flags_operand", "class_operand"]
    );
    // No P_FINISH: the cursor stops immediately after the 4th operand opcode.
    assert_eq!(
        invocation.frame.code,
        invocation.code.as_mut_ptr().wrapping_add(4)
    );
    assert_eq!(
        NEW_CONSTRUCTED.with(|c| c.get()),
        Some((class_ptr, outer_ptr, 99, 0x20))
    );
}

#[test]
fn execute_new_outer_defaults_from_index_and_other_operands_default_when_omitted() {
    for (index, expect_transient) in [(7, false), (-1, true)] {
        NEW_CONSTRUCTED.with(|c| c.set(None));
        EVENTS.with(|v| v.borrow_mut().clear());
        let mut object = unsafe { std::mem::zeroed::<UObject>() };
        object.index = index;
        let object_ptr = (&raw mut object).cast::<c_void>();
        let mut invocation = ObjectInvocation::new4([0x80, 0x81, 0x82, 0x83, 0, 0, 0, 0], 0, 0, 0, 0);
        invocation.table[0x80] = object_operand_absent;
        invocation.table[0x81] = object_operand_absent;
        invocation.table[0x82] = object_operand_absent;
        invocation.table[0x83] = object_operand_absent;
        invocation.prepare();
        let s = new_frame_services();
        let mut result: *mut c_void = null_mut();
        let p = &raw mut invocation;
        let status = unsafe {
            execute_new(
                object_ptr,
                std::ptr::addr_of_mut!((*p).frame),
                &raw mut result,
                std::ptr::addr_of!((*p).table).cast(),
                &s,
            )
        };
        assert_eq!(status, Ok(()));
        let expected_outer = if expect_transient {
            0x7000usize as *mut c_void
        } else {
            object_ptr
        };
        assert_eq!(
            NEW_CONSTRUCTED.with(|c| c.get()),
            Some((null_mut(), expected_outer, NAME_NONE, 0))
        );
        assert_eq!(events().contains(&"transient"), expect_transient);
    }
}

// Frame-level ordering for execute_enable/execute_disable: Enable probes
// before P_FINISH, Disable probes after, matching the legacy evaluation
// order. An out-of-[probe_min,probe_max) name routes probe() through its
// single `warning` call, so no World/state scaffolding is needed.
unsafe extern "C" fn probe_order_name_operand(
    _: *mut c_void,
    _: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("name_operand"));
        result.cast::<i32>().write(PROBE - 1);
    }
    0
}
unsafe extern "C" fn probe_order_finish_marker(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("finish"));
    0
}
unsafe extern "C" fn probe_order_warning(_: *mut c_void, _: u32, _: u32, _: i32) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("probe"));
    0
}
fn probe_order_services() -> Hp2VmObjectServices {
    Hp2VmObjectServices {
        warning: probe_order_warning,
        ..services()
    }
}

#[test]
fn execute_enable_probes_before_p_finish_and_disable_probes_after() {
    for (enable, expected_order) in [
        (true, vec!["name_operand", "probe", "finish"]),
        (false, vec!["name_operand", "finish", "probe"]),
    ] {
        EVENTS.with(|v| v.borrow_mut().clear());
        let mut invocation = ObjectInvocation::new([0x80, 0x16, 0x38, 0x42, 0, 0, 0, 0], 0, 0);
        invocation.table[0x80] = probe_order_name_operand;
        invocation.table[0x38] = probe_order_finish_marker;
        invocation.prepare();
        let s = probe_order_services();
        let object = 0x55usize as *mut c_void;
        let p = &raw mut invocation;
        let status = unsafe {
            if enable {
                execute_enable(
                    object,
                    std::ptr::addr_of_mut!((*p).frame),
                    std::ptr::addr_of!((*p).table).cast(),
                    &s,
                )
            } else {
                execute_disable(
                    object,
                    std::ptr::addr_of_mut!((*p).frame),
                    std::ptr::addr_of!((*p).table).cast(),
                    &s,
                )
            }
        };
        assert_eq!(status, Ok(()));
        assert_eq!(events(), expected_order);
    }
}

// Frame-level coverage for execute_goto_state: S defaults to the current
// state name, computed via has_frame before any operand is decoded; L
// defaults to None. A has_frame stub that always reports "missing frame"
// lets both current_state_name and goto_label short-circuit without needing
// World/Node scaffolding, while still proving the call order and that the
// omitted-label case stays silent (matching
// goto_implicit_missing_begin_label_is_silent for a live state).
unsafe extern "C" fn goto_default_has_frame(_: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("has_frame"));
    0
}
unsafe extern "C" fn goto_default_absent_operand(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("operand_absent"));
    0
}

#[test]
fn execute_goto_state_defaults_s_to_current_computed_before_decode_and_l_to_none() {
    EVENTS.with(|v| v.borrow_mut().clear());
    let state_table = Hp2VmStateServices {
        auto_name: AUTO,
        begin_state_name: 9002,
        end_state_name: 9003,
        has_frame: goto_default_has_frame,
        clear_latent,
        frame_state,
        object_class,
        name,
        find_state,
        children,
        next_field,
        inheritance_super: super_node,
        is_state,
        state_flags,
        object_flags,
        clear_flags,
        set_flags,
        probing,
        end_state,
        begin_state,
        publish_state,
        probe_mask,
        ignore_mask,
        set_probe_mask: set_mask,
        super_state: super_node,
        label_table,
        script_int,
        label_matches,
        set_node,
        set_code,
        clear_code,
    };
    let s = Hp2VmObjectServices {
        state: &state_table,
        begin_name: BEGIN,
        ..services()
    };
    let mut invocation = ObjectInvocation::new([0x80, 0x81, 0x16, 0x42, 0, 0, 0, 0], 0, 0);
    invocation.table[0x80] = goto_default_absent_operand; // S omitted
    invocation.table[0x81] = goto_default_absent_operand; // L omitted
    invocation.prepare();
    let object = 0x55usize as *mut c_void;
    let p = &raw mut invocation;
    let status = unsafe {
        execute_goto_state(
            object,
            std::ptr::addr_of_mut!((*p).frame),
            std::ptr::addr_of!((*p).table).cast(),
            &s,
        )
    };
    assert_eq!(status, Ok(()));
    // has_frame(#1) = current_state_name, read before any operand decode;
    // both operands are omitted and keep their seeded defaults (current,
    // None); has_frame(#2) is goto_label's own missing-frame short-circuit.
    // Since the omitted L stayed None, the missing-label case is silent.
    assert_eq!(
        events(),
        ["has_frame", "operand_absent", "operand_absent", "has_frame"]
    );
    assert_eq!(
        invocation.frame.code,
        invocation.code.as_mut_ptr().wrapping_add(3)
    );
}

// Frame-level coverage for execute_static_save_config: `Class->GetDefaultObject()
// ->SaveConfig()` must go through the checked GetDefaultObject() leaf, not a raw
// Class-field read standing in for the default object. default_object here
// writes a sentinel distinct from the Class pointer it receives, so a
// regression to the old raw-read bug (SaveConfig(Class) instead of
// SaveConfig(GetDefaultObject())) would fail the pointer-identity assertion.
thread_local! {
    static STATIC_SAVE_DEFAULT_OBJECT_CLASS: Cell<*mut c_void> = const { Cell::new(null_mut()) };
    static STATIC_SAVE_SAVE_CONFIG_OBJECT: Cell<*mut c_void> = const { Cell::new(null_mut()) };
    static STATIC_SAVE_DEFAULT_OBJECT_STATUS: Cell<i32> = const { Cell::new(0) };
}
unsafe extern "C" fn static_save_default_object(class: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        STATIC_SAVE_DEFAULT_OBJECT_CLASS.with(|c| c.set(class));
        EVENTS.with(|v| v.borrow_mut().push("default_object"));
        out.write(0x9999usize as *mut c_void);
    }
    STATIC_SAVE_DEFAULT_OBJECT_STATUS.with(|c| c.get())
}
unsafe extern "C" fn static_save_save_config(object: *mut c_void) -> i32 {
    STATIC_SAVE_SAVE_CONFIG_OBJECT.with(|c| c.set(object));
    EVENTS.with(|v| v.borrow_mut().push("save_config"));
    0
}
fn static_save_services() -> Hp2VmObjectServices {
    Hp2VmObjectServices {
        default_object: static_save_default_object,
        save_config: static_save_save_config,
        ..services()
    }
}

#[test]
fn execute_static_save_config_routes_through_get_default_object_not_a_raw_class_read() {
    for (default_object_status, expect_save_config) in [(0, true), (ERROR, false)] {
        EVENTS.with(|v| v.borrow_mut().clear());
        STATIC_SAVE_DEFAULT_OBJECT_CLASS.with(|c| c.set(null_mut()));
        STATIC_SAVE_SAVE_CONFIG_OBJECT.with(|c| c.set(null_mut()));
        STATIC_SAVE_DEFAULT_OBJECT_STATUS.with(|c| c.set(default_object_status));
        let mut object = unsafe { std::mem::zeroed::<UObject>() };
        let class_ptr = 0x1234usize as *mut c_void;
        object.class = class_ptr;
        let object_ptr = (&raw mut object).cast::<c_void>();
        let mut invocation = ObjectInvocation::new([0xAA, 0x42, 0, 0, 0, 0, 0, 0], 0, 0);
        invocation.prepare();
        let s = static_save_services();
        let p = &raw mut invocation;
        let status = unsafe {
            execute_static_save_config(
                object_ptr,
                std::ptr::addr_of_mut!((*p).frame),
                std::ptr::addr_of!((*p).table).cast(),
                &s,
            )
        };
        assert_eq!(
            STATIC_SAVE_DEFAULT_OBJECT_CLASS.with(|c| c.get()),
            class_ptr
        );
        if expect_save_config {
            assert_eq!(status, Ok(()));
            assert_eq!(events(), ["default_object", "save_config"]);
            // SaveConfig must receive exactly the pointer default_object wrote
            // (0x9999), never the raw Class pointer (0x1234) it was handed.
            assert_eq!(
                STATIC_SAVE_SAVE_CONFIG_OBJECT.with(|c| c.get()),
                0x9999usize as *mut c_void
            );
        } else {
            // A default_object failure short-circuits before save_config runs.
            assert_eq!(status, Err(ERROR));
            assert_eq!(events(), ["default_object"]);
        }
    }
}

// Frame-level coverage for execute_warn: `Stack.Logf(TEXT("%s"), *S)` must
// reach the composite `logf` leaf exactly once, never `error`/`warning` (those
// belong to the unrelated FFrame::Serialize port used by other callers). The
// old bug called vm_frame::serialize directly, bypassing FOutputDevice::Logf's
// SafeSuppressed(NAME_Log) check and 4096-char truncation entirely. Argument
// storage (S) must be released on both the success and the logf-error path.
thread_local! {
    static WARN_LOGF_CALLS: RefCell<Vec<(*mut c_void, *const c_void)>> =
        const { RefCell::new(Vec::new()) };
    static WARN_LOGF_STATUS: Cell<i32> = const { Cell::new(0) };
}
static WARN_TEXT_BYTE: u8 = 0;
unsafe extern "C" fn warn_text_operand(_: *mut c_void, _: *mut c_void, result: *mut c_void) -> i32 {
    unsafe {
        EVENTS.with(|v| v.borrow_mut().push("text_operand"));
        result.cast::<TArrayRaw>().write(TArrayRaw {
            data: (&raw const WARN_TEXT_BYTE).cast::<c_void>().cast_mut(),
            num: 1,
            max: 1,
        });
    }
    0
}
unsafe extern "C" fn warn_logf(frame: *mut c_void, text: *const c_void) -> i32 {
    WARN_LOGF_CALLS.with(|c| c.borrow_mut().push((frame, text)));
    EVENTS.with(|v| v.borrow_mut().push("logf"));
    WARN_LOGF_STATUS.with(|c| c.get())
}
unsafe extern "C" fn warn_error(_: *const c_void, _: *const c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("error_unexpected"));
    0
}
unsafe extern "C" fn warn_warning(_: *const c_void, _: *const c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("warning_unexpected"));
    0
}
unsafe extern "C" fn warn_buffer(_: *const c_void, _: *mut *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("buffer_unexpected"));
    0
}
unsafe extern "C" fn warn_outer(_: *mut c_void) -> *mut c_void {
    EVENTS.with(|v| v.borrow_mut().push("outer_unexpected"));
    null_mut()
}
unsafe extern "C" fn warn_format_description(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("format_description_unexpected"));
    0
}
unsafe extern "C" fn warn_free(_: *mut c_void, _: *mut c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("free"));
    0
}
unsafe extern "C" fn warn_realloc_unexpected(
    _: *mut c_void,
    _: *mut c_void,
    _: usize,
    _: *const i32,
    _: i32,
    _: *const c_void,
    _: *mut *mut c_void,
) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("realloc_unexpected"));
    0
}
unsafe extern "C" fn warn_checked_int_size_unexpected(_: *mut c_void, _: usize, _: *mut i32) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("checked_int_size_unexpected"));
    0
}
const WARN_ANSI: Hp2AnsiServices = Hp2AnsiServices {
    fail_assert: ansi_fail_assert_unexpected,
    to_ansi: ansi_to_ansi_unexpected,
    size_exceeds_int_contract: ansi_size_exceeds_unexpected,
};
unsafe extern "C" fn ansi_fail_assert_unexpected(_: *const c_void, _: *const c_void, _: i32) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("ansi_unexpected"));
    0
}
unsafe extern "C" fn ansi_to_ansi_unexpected(_: *const c_void, _: *mut *const c_void) -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("ansi_unexpected"));
    0
}
unsafe extern "C" fn ansi_size_exceeds_unexpected() -> i32 {
    EVENTS.with(|v| v.borrow_mut().push("ansi_unexpected"));
    0
}
static WARN_STRICT: i32 = 0;
fn warn_frame_services() -> Hp2VmFrameServices {
    Hp2VmFrameServices {
        critical_name: 91,
        strict: &raw const WARN_STRICT,
        ansi: &WARN_ANSI,
        buffer: warn_buffer,
        outer: warn_outer,
        format_description: warn_format_description,
        error: warn_error,
        warning: warn_warning,
        native_table: null(),
        logf: warn_logf,
    }
}
fn warn_arrays() -> Hp2ArrayServices {
    Hp2ArrayServices {
        context: null_mut(),
        realloc: warn_realloc_unexpected,
        checked_int_size: warn_checked_int_size_unexpected,
        free: warn_free,
    }
}

#[test]
fn execute_warn_calls_logf_once_and_releases_storage_on_every_path() {
    for (logf_status, expect_ok) in [(0, true), (ERROR, false)] {
        EVENTS.with(|v| v.borrow_mut().clear());
        WARN_LOGF_CALLS.with(|c| c.borrow_mut().clear());
        WARN_LOGF_STATUS.with(|c| c.set(logf_status));
        let mut invocation = ObjectInvocation::new([0x80, 0x16, 0x42, 0, 0, 0, 0, 0], 0, 0);
        invocation.table[0x80] = warn_text_operand;
        invocation.prepare();
        let frame_services = warn_frame_services();
        let p = &raw mut invocation;
        let frame_ptr = unsafe { std::ptr::addr_of_mut!((*p).frame) };
        let status = unsafe {
            execute_warn(
                frame_ptr,
                std::ptr::addr_of!((*p).table).cast(),
                warn_arrays(),
                &frame_services,
            )
        };
        assert_eq!(status, if expect_ok { Ok(()) } else { Err(ERROR) });
        // Exactly one logf call, decoded after the text operand; error/warning/
        // the other frame/array leaves never run; storage (non-null `data`) is
        // always released via `free`, success or failure.
        assert_eq!(events(), ["text_operand", "logf", "free"]);
        let calls = WARN_LOGF_CALLS.with(|c| c.borrow().clone());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, frame_ptr.cast::<c_void>());
        assert_eq!(calls[0].1, (&raw const WARN_TEXT_BYTE).cast::<c_void>());
    }
}
