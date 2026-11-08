use hp2_core::vm_state::*;
use std::ffi::c_void;
use std::ptr::null_mut;

const FAILURE: i32 = 73;
const OLD: i32 = 100;
const NEW: i32 = 101;
const NESTED: i32 = 102;
const LABEL: i32 = 200;
// Deliberately independent of native enum positions: the service carries IDs.
const NAME_AUTO: i32 = 9000;
const NAME_BEGIN_STATE: i32 = 9001;
const NAME_END_STATE: i32 = 9002;

struct Node {
    name: i32,
    state: bool,
    flags: u32,
    probe: u64,
    ignore: u64,
    children: *mut c_void,
    next: *mut c_void,
    parent: *mut c_void,
    table: u16,
    script: [u8; 80],
    script_len: i32,
    reads: Vec<i32>,
    is_checks: usize,
    fail_is: bool,
    fail_super: bool,
}
impl Node {
    fn new(name: i32) -> Self {
        Self {
            name,
            state: true,
            flags: 0,
            probe: 0,
            ignore: u64::MAX,
            children: null_mut(),
            next: null_mut(),
            parent: null_mut(),
            table: NO_LABEL_TABLE,
            script: [0; 80],
            script_len: 64,
            reads: vec![],
            is_checks: 0,
            fail_is: false,
            fail_super: false,
        }
    }
    fn entry(&mut self, offset: usize, name: i32, code: i32) {
        self.script[offset..offset + 4].copy_from_slice(&name.to_ne_bytes());
        self.script[offset + 4..offset + 8].copy_from_slice(&code.to_ne_bytes());
    }
}
#[derive(Clone, Copy)]
enum Action {
    None,
    Throw,
    Reenter(i32),
    ReenterThenThrow(i32),
    ReplaceFrame,
}
struct World {
    nodes: [Node; 6],
    has_frame: bool,
    class: *mut c_void,
    state: *mut c_void,
    node: *mut c_void,
    code: i32,
    latent: i32,
    mask: u64,
    flags: u32,
    end_probe: bool,
    begin_probe: bool,
    end: Action,
    begin: Action,
    fail: &'static str,
    log: Vec<&'static str>,
    notifications: Vec<(i32, u32, i32)>,
    nested_results: Vec<i32>,
    rename_old_during_find: bool,
    frame_generation: usize,
}
impl World {
    fn new() -> Box<Self> {
        let mut w = Box::new(Self {
            nodes: [
                Node::new(10),
                Node::new(OLD),
                Node::new(NEW),
                Node::new(NESTED),
                Node::new(11),
                Node::new(12),
            ],
            has_frame: true,
            class: null_mut(),
            state: null_mut(),
            node: null_mut(),
            code: 55,
            latent: 9,
            mask: u64::MAX,
            flags: RF_STATE_CHANGED,
            end_probe: true,
            begin_probe: true,
            end: Action::None,
            begin: Action::None,
            fail: "",
            log: vec![],
            notifications: vec![],
            nested_results: vec![],
            rename_old_during_find: false,
            frame_generation: 0,
        });
        w.class = w.ptr(0);
        w.state = w.ptr(1);
        w.node = w.ptr(1);
        w.nodes[0].children = w.ptr(1);
        w.nodes[1].next = w.ptr(2);
        w.nodes[2].next = w.ptr(3);
        w
    }
    fn ptr(&mut self, index: usize) -> *mut c_void {
        (&mut self.nodes[index] as *mut Node).cast()
    }
    fn mark(&mut self, event: &'static str) -> i32 {
        self.log.push(event);
        if self.fail == event { FAILURE } else { 0 }
    }
    fn transition(&mut self, name: i32) -> Result<i32, i32> {
        unsafe { goto_state((self as *mut Self).cast(), name, &SERVICES) }
    }
    fn label(&mut self, name: i32) -> Result<i32, i32> {
        unsafe { goto_label((self as *mut Self).cast(), name, &SERVICES) }
    }
}
unsafe fn world<'a>(p: *mut c_void) -> &'a mut World {
    unsafe { &mut *p.cast() }
}
unsafe fn node<'a>(p: *mut c_void) -> &'a mut Node {
    unsafe { &mut *p.cast() }
}
unsafe extern "C" fn has_frame(p: *mut c_void) -> i32 {
    unsafe {
        world(p).log.push("frame");
        i32::from(world(p).has_frame)
    }
}
unsafe extern "C" fn clear_latent(p: *mut c_void) {
    unsafe {
        world(p).log.push("latent");
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
        assert_eq!(w.latent, 0);
        let result = w.mark("find");
        if result != 0 {
            return result;
        }
        if w.rename_old_during_find {
            node(w.state).name = NEW;
        }
        *out = null_mut();
        for n in &mut w.nodes[1..4] {
            if n.name == name {
                *out = (n as *mut Node).cast();
                break;
            }
        }
        0
    }
}
unsafe extern "C" fn children(p: *mut c_void) -> *mut c_void {
    unsafe { node(p).children }
}
unsafe extern "C" fn next_field(p: *mut c_void) -> *mut c_void {
    unsafe { node(p).next }
}
unsafe extern "C" fn parent(p: *mut c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        if node(p).fail_super {
            return FAILURE;
        }
        *out = node(p).parent;
        0
    }
}
unsafe extern "C" fn is_state(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let n = node(p);
        n.is_checks += 1;
        if n.fail_is {
            return FAILURE;
        }
        *out = i32::from(n.state);
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
        let w = world(p);
        w.flags &= !flags;
        w.mark(if flags == RF_IN_END_STATE {
            "clear-end"
        } else {
            "clear-changed"
        })
    }
}
unsafe extern "C" fn set_flags(p: *mut c_void, flags: u32) -> i32 {
    unsafe {
        let w = world(p);
        w.flags |= flags;
        w.mark(if flags == RF_IN_END_STATE {
            "set-end"
        } else {
            "set-changed"
        })
    }
}
unsafe extern "C" fn probing(p: *mut c_void, name: i32, out: *mut i32) -> i32 {
    unsafe {
        let w = world(p);
        if name == NAME_END_STATE {
            *out = i32::from(w.end_probe);
            w.mark("probe-end")
        } else {
            assert_eq!(name, NAME_BEGIN_STATE);
            *out = i32::from(w.begin_probe);
            w.mark("probe-begin")
        }
    }
}
unsafe fn notify(p: *mut c_void, is_end: bool) -> i32 {
    unsafe {
        let action = {
            let w = world(p);
            w.log.push(if is_end { "end" } else { "begin" });
            w.notifications.push((node(w.state).name, w.flags, w.code));
            std::mem::replace(if is_end { &mut w.end } else { &mut w.begin }, Action::None)
        };
        // No World reference is held across recursive policy execution.
        match action {
            Action::None => 0,
            Action::Throw => FAILURE,
            Action::Reenter(name) | Action::ReenterThenThrow(name) => {
                match goto_state(p, name, &SERVICES) {
                    Ok(value) => world(p).nested_results.push(value),
                    Err(error) => return error,
                }
                if matches!(action, Action::ReenterThenThrow(_)) {
                    FAILURE
                } else {
                    0
                }
            }
            Action::ReplaceFrame => {
                let w = world(p);
                w.frame_generation += 1;
                w.state = w.ptr(3);
                w.node = w.ptr(3);
                w.code = 44;
                w.latent = 88;
                0
            }
        }
    }
}
unsafe extern "C" fn end(p: *mut c_void) -> i32 {
    unsafe { notify(p, true) }
}
unsafe extern "C" fn begin(p: *mut c_void) -> i32 {
    unsafe { notify(p, false) }
}
unsafe extern "C" fn publish(p: *mut c_void, state: *mut c_void) {
    unsafe {
        let w = world(p);
        w.log.push("publish");
        w.node = state;
        w.state = state;
        w.code = -1;
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
        world(p).log.push("mask");
    }
}
unsafe extern "C" fn table(p: *mut c_void) -> u16 {
    unsafe { node(p).table }
}
unsafe extern "C" fn script_int(p: *mut c_void, offset: i32, out: *mut i32) -> i32 {
    unsafe {
        let n = node(p);
        n.reads.push(offset);
        if offset < 0 || offset >= n.script_len {
            return FAILURE;
        }
        // The backing array is padded; only the starting index is checked,
        // exactly like Script(index) followed by the original native memcpy.
        *out = n
            .script
            .as_ptr()
            .add(offset as usize)
            .cast::<i32>()
            .read_unaligned();
        0
    }
}
unsafe extern "C" fn label_matches(name: i32, label: i32, out: *mut i32) -> i32 {
    unsafe {
        if name < 0 {
            return FAILURE;
        }
        *out = i32::from(name == label);
        0
    }
}
unsafe extern "C" fn set_node(p: *mut c_void, state: *mut c_void) {
    unsafe {
        world(p).node = state;
        world(p).log.push("node");
    }
}
unsafe extern "C" fn set_code(p: *mut c_void, state: *mut c_void, offset: i32) -> i32 {
    unsafe {
        node(state).reads.push(offset);
        if offset < 0 || offset >= node(state).script_len {
            return FAILURE;
        }
        world(p).code = offset;
        world(p).log.push("code");
        0
    }
}
unsafe extern "C" fn clear_code(p: *mut c_void) {
    unsafe {
        world(p).code = -1;
        world(p).log.push("clear-code");
    }
}
static SERVICES: Hp2VmStateServices = Hp2VmStateServices {
    auto_name: NAME_AUTO,
    begin_state_name: NAME_BEGIN_STATE,
    end_state_name: NAME_END_STATE,
    has_frame,
    clear_latent,
    frame_state,
    object_class,
    name,
    find_state,
    children,
    next_field,
    inheritance_super: parent,
    is_state,
    state_flags,
    object_flags,
    clear_flags,
    set_flags,
    probing,
    end_state: end,
    begin_state: begin,
    publish_state: publish,
    probe_mask,
    ignore_mask,
    set_probe_mask: set_mask,
    super_state: parent,
    label_table: table,
    script_int,
    label_matches,
    set_node,
    set_code,
    clear_code,
};

#[test]
fn missing_frame_returns_without_touching_flags_latent_or_code() {
    for label in [false, true] {
        let mut w = World::new();
        w.has_frame = false;
        let result = if label {
            w.label(LABEL)
        } else {
            w.transition(NEW)
        };
        assert_eq!(result, Ok(0));
        assert_eq!(w.log, ["frame"]);
        assert_eq!((w.latent, w.code, w.flags), (9, 55, RF_STATE_CHANGED));
    }
}

#[test]
fn regular_transition_orders_notifications_publication_and_probe_mask() {
    let mut w = World::new();
    w.nodes[0].probe = 0x8000_0000_0000_0005;
    w.nodes[2].probe = 0x12;
    w.nodes[2].ignore = 0x8000_0000_0000_0016;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert!(w.state == w.ptr(2));
    assert!(w.node == w.ptr(2));
    assert_eq!(
        (w.latent, w.code, w.mask, w.flags),
        (0, -1, 0x8000_0000_0000_0016, RF_STATE_CHANGED)
    );
    assert_eq!(w.notifications, [(OLD, RF_IN_END_STATE, 55), (NEW, 0, -1)]);
    assert_eq!(
        w.log,
        [
            "frame",
            "latent",
            "find",
            "probe-end",
            "clear-changed",
            "set-end",
            "end",
            "clear-end",
            "publish",
            "mask",
            "probe-begin",
            "clear-changed",
            "begin",
            "set-changed"
        ]
    );
}

#[test]
fn old_name_is_captured_before_lookup_and_lookup_errors_only_clear_latent() {
    let mut w = World::new();
    w.rename_old_during_find = true;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert!(w.log.contains(&"end"));
    assert!(w.log.contains(&"begin"));
    let mut w = World::new();
    w.fail = "find";
    assert_eq!(w.transition(NEW), Err(FAILURE));
    assert_eq!((w.latent, w.code, w.flags), (0, 55, RF_STATE_CHANGED));
    assert!(w.state == w.ptr(1));
}

#[test]
fn end_reentry_preempts_before_outer_publication_and_clears_end_flag() {
    let mut w = World::new();
    w.end = Action::Reenter(NESTED);
    assert_eq!(w.transition(NEW), Ok(GOTO_PREEMPTED));
    assert!(w.state == w.ptr(3));
    assert_eq!(w.nested_results, [GOTO_SUCCESS]);
    assert_eq!(w.flags, RF_STATE_CHANGED);
    assert_eq!(w.log.iter().filter(|&&x| x == "publish").count(), 1);
    assert_eq!(
        w.notifications,
        [(OLD, RF_IN_END_STATE, 55), (NESTED, RF_IN_END_STATE, -1)]
    );
}

#[test]
fn begin_reentry_preempts_after_outer_publication() {
    let mut w = World::new();
    w.begin = Action::Reenter(NESTED);
    assert_eq!(w.transition(NEW), Ok(GOTO_PREEMPTED));
    assert!(w.state == w.ptr(3));
    assert_eq!(w.nested_results, [GOTO_SUCCESS]);
    assert_eq!(w.log.iter().filter(|&&x| x == "publish").count(), 2);
    assert_eq!(w.notifications[1], (NEW, 0, -1));
    assert_eq!(w.flags, RF_STATE_CHANGED);
}

#[test]
fn reentrant_fallback_does_not_preempt_and_live_replaced_frame_is_published() {
    let mut w = World::new();
    w.end = Action::Reenter(NAME_NONE);
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert_eq!(w.nested_results, [GOTO_NOT_FOUND]);
    assert!(w.state == w.ptr(2));
    let mut w = World::new();
    w.end = Action::ReplaceFrame;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert_eq!(w.frame_generation, 1);
    assert!(w.state == w.ptr(2));
    assert_eq!(w.latent, 88);
    assert_eq!(w.code, -1);
}

#[test]
fn thrown_notifications_preserve_sticky_end_and_published_begin_state() {
    let mut w = World::new();
    w.end = Action::Throw;
    assert_eq!(w.transition(NEW), Err(FAILURE));
    assert_eq!(w.flags, RF_IN_END_STATE);
    assert!(w.state == w.ptr(1));
    assert_eq!(w.code, 55);
    assert_eq!(w.log.last(), Some(&"end"));
    let mut w = World::new();
    w.begin = Action::Throw;
    assert_eq!(w.transition(NEW), Err(FAILURE));
    assert_eq!(w.flags, 0);
    assert!(w.state == w.ptr(2));
    assert_eq!(w.code, -1);
    assert_eq!(w.log.last(), Some(&"begin"));
    let mut w = World::new();
    w.end = Action::ReenterThenThrow(NESTED);
    assert_eq!(w.transition(NEW), Err(FAILURE));
    assert!(w.state == w.ptr(3));
    assert_eq!(w.flags, RF_IN_END_STATE | RF_STATE_CHANGED);
}

#[test]
fn throwing_flag_operations_preserve_partial_mutations_and_stop_immediately() {
    for (fail, flags, published) in [
        ("clear-changed", 0, false),
        ("set-end", RF_IN_END_STATE, false),
        ("clear-end", 0, false),
        ("set-changed", RF_STATE_CHANGED, true),
    ] {
        let mut w = World::new();
        w.fail = fail;
        assert_eq!(w.transition(NEW), Err(FAILURE));
        assert_eq!(w.flags, flags);
        assert_eq!(w.log.last(), Some(&fail));
        assert!(w.state == w.ptr(if published { 2 } else { 1 }));
    }
}

#[test]
fn same_name_probe_gates_in_end_gate_and_fallback_keep_original_success_flags() {
    let mut w = World::new();
    assert_eq!(w.transition(OLD), Ok(GOTO_SUCCESS));
    assert!(w.notifications.is_empty());
    assert!(!w.log.contains(&"probe-end"));
    assert!(!w.log.contains(&"probe-begin"));
    assert_eq!(w.code, -1);
    let mut w = World::new();
    w.end_probe = false;
    w.begin_probe = false;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert!(w.notifications.is_empty());
    let mut w = World::new();
    w.flags |= RF_IN_END_STATE;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert!(!w.log.contains(&"end"));
    assert!(w.log.contains(&"probe-end"));
    assert_eq!(w.flags, RF_IN_END_STATE | RF_STATE_CHANGED);
    for requested in [NAME_NONE, 999, NAME_AUTO] {
        let mut w = World::new();
        w.end_probe = false;
        w.nodes[0].probe = 0x37;
        w.nodes[0].ignore = 0x15;
        assert_eq!(w.transition(requested), Ok(GOTO_NOT_FOUND));
        assert!(w.state == w.ptr(0));
        assert!(w.node == w.ptr(0));
        assert_eq!((w.code, w.mask, w.flags), (-1, 0x15, RF_STATE_CHANGED));
        assert!(!w.log.contains(&"probe-begin"));
    }
    let mut w = World::new();
    w.state = w.class;
    assert_eq!(w.transition(NEW), Ok(GOTO_SUCCESS));
    assert!(!w.log.contains(&"probe-end"));
    assert_eq!(w.notifications.len(), 1);
}

#[test]
fn auto_uses_first_state_field_in_inheritance_order_and_scans_successor() {
    let mut w = World::new();
    w.nodes[1].state = false;
    w.nodes[1].flags = STATE_AUTO;
    w.nodes[2].flags = STATE_AUTO;
    w.nodes[3].flags = STATE_AUTO;
    assert_eq!(w.transition(NAME_AUTO), Ok(GOTO_SUCCESS));
    assert!(w.state == w.ptr(2));
    assert!(!w.log.contains(&"find"));
    assert_eq!(w.nodes[1].is_checks, 1);
    assert_eq!(w.nodes[3].is_checks, 1);
    let mut w = World::new();
    w.nodes[0].parent = w.ptr(4);
    w.nodes[4].children = w.ptr(5);
    w.nodes[5].flags = STATE_AUTO;
    assert_eq!(w.transition(NAME_AUTO), Ok(GOTO_SUCCESS));
    assert!(w.state == w.ptr(5));
    assert_eq!(w.notifications.last().unwrap().0, 12);
    let mut w = World::new();
    w.nodes[2].flags = STATE_AUTO;
    w.nodes[3].fail_is = true;
    assert_eq!(w.transition(NAME_AUTO), Err(FAILURE));
    assert!(w.state == w.ptr(1));
    assert_eq!(w.code, 55);
    assert_eq!(w.latent, 0);
    assert!(w.notifications.is_empty());
}

#[test]
fn inherited_labels_read_unaligned_entries_and_sentinel_without_changing_state() {
    let mut w = World::new();
    w.nodes[1].table = 1;
    w.nodes[1].entry(1, LABEL + 1, 10);
    w.nodes[1].entry(9, NAME_NONE, -123);
    w.nodes[1].parent = w.ptr(4); // Absent table is skipped.
    w.nodes[4].parent = w.ptr(5);
    w.nodes[5].table = 3;
    w.nodes[5].entry(3, LABEL, 31);
    assert_eq!(w.label(LABEL), Ok(1));
    assert!(w.state == w.ptr(1));
    assert!(w.node == w.ptr(5));
    assert_eq!(
        (w.code, w.latent, w.flags, w.mask),
        (31, 0, RF_STATE_CHANGED, u64::MAX)
    );
    assert_eq!(w.nodes[1].reads, [1, 5, 9, 13]);
    assert!(w.nodes[4].reads.is_empty());
    assert_eq!(w.nodes[5].reads, [3, 7, 31]);
}

#[test]
fn none_and_missing_labels_clear_only_code_and_latent() {
    for requested in [NAME_NONE, LABEL] {
        let mut w = World::new();
        w.nodes[1].table = 1;
        w.nodes[1].entry(1, NAME_NONE, 0);
        assert_eq!(w.label(requested), Ok(0));
        assert!(w.node == w.ptr(1));
        assert!(w.state == w.ptr(1));
        assert_eq!((w.code, w.latent, w.flags), (-1, 0, RF_STATE_CHANGED));
        if requested == NAME_NONE {
            assert!(w.nodes[1].reads.is_empty());
        } else {
            assert_eq!(w.nodes[1].reads, [1, 5]);
        }
    }
}

#[test]
fn label_checked_access_errors_preserve_code_and_match_publishes_node_first() {
    for logical_len in [1, 5] {
        let mut w = World::new();
        w.nodes[1].table = 1;
        w.nodes[1].entry(1, NAME_NONE, 0);
        w.nodes[1].script_len = logical_len;
        assert_eq!(w.label(LABEL), Err(FAILURE));
        assert_eq!((w.code, w.latent), (55, 0));
        assert!(w.node == w.ptr(1));
        assert_eq!(
            w.nodes[1].reads,
            if logical_len == 1 {
                vec![1]
            } else {
                vec![1, 5]
            }
        );
    }
    let mut w = World::new();
    w.nodes[1].parent = w.ptr(5);
    w.nodes[5].table = 1;
    w.nodes[5].entry(1, LABEL, -1);
    assert_eq!(w.label(LABEL), Err(FAILURE));
    assert!(w.node == w.ptr(5));
    assert!(w.state == w.ptr(1));
    assert_eq!(w.code, 55);
    assert_eq!(w.log.last(), Some(&"node"));
    let mut w = World::new();
    w.nodes[1].table = 1;
    w.nodes[1].entry(1, -10, 20);
    assert_eq!(w.label(LABEL), Err(FAILURE));
    assert_eq!(w.code, 55);
    assert!(w.node == w.ptr(1));
    let mut w = World::new();
    w.nodes[1].fail_super = true;
    assert_eq!(w.label(LABEL), Err(FAILURE));
    assert_eq!(w.code, 55);
}

#[test]
fn script_int_retains_start_index_check_instead_of_adding_span_validation() {
    let mut w = World::new();
    w.nodes[1].table = 1;
    w.nodes[1].entry(1, NAME_NONE, 1234);
    w.nodes[1].script_len = 6;
    assert_eq!(w.label(LABEL), Ok(0));
    assert_eq!(w.nodes[1].reads, [1, 5]);
}
