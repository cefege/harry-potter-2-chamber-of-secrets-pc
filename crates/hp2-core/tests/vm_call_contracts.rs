use hp2_abi::engine::generated::FFrame;
use hp2_core::vm_call::{self, Hp2VmCallServices};
use hp2_core::vm_frame::Hp2VmFrameInitializationServices;
use std::{
    cell::{Cell, UnsafeCell},
    ffi::c_void,
    ptr,
};
type Object = *mut c_void;
const END: u8 = 0x16;
const DEBUG: u8 = 0x38;
thread_local! { static OBSERVER: Cell<usize> = const { Cell::new(1) }; }
struct Property {
    owner: *mut Fixture,
    id: usize,
    flags: u32,
    offset: i32,
    next: *mut Property,
    constructor: *mut Property,
}
struct Fixture {
    native: i32,
    flags: u32,
    absorbed: i32,
    fail: &'static str,
    bytes: Vec<u8>,
    code: *mut u8,
    script: [u8; 4],
    script_offset: usize,
    initialized: bool,
    initialization_status: i32,
    frame: *mut FFrame,
    parent: *mut FFrame,
    publications: usize,
    result: i32,
    nested: Object,
    reentries: usize,
    mutate_parent: bool,
    // Snapshots captured by callbacks while the owned frame is live.
    locals: [i32; 20],
    locals_bytes: i32,
    trailing: u8,
    targets: [i32; 20],
    _props: Box<[Property]>,
    properties: *mut Property,
    prop_addr: *mut u8,
    first_constructor: *mut Property,
    arguments: usize,
    aliases: bool,
    mutate_links: bool,
    swap_observer: bool,
    events: Vec<String>,
}
fn fixture(count: usize, debug: bool) -> (Box<UnsafeCell<Fixture>>, Object) {
    OBSERVER.with(|v| v.set(1));
    let mut bytes = vec![1; count];
    bytes.push(END);
    if debug {
        bytes.push(DEBUG);
    }
    bytes.push(0);
    let code = bytes.as_mut_ptr();
    let mut props = (0..17)
        .map(|id| Property {
            owner: ptr::null_mut(),
            id,
            flags: 1,
            offset: (id * 4) as i32,
            next: ptr::null_mut(),
            constructor: ptr::null_mut(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let properties = props.as_mut_ptr();
    let f = Box::new(UnsafeCell::new(Fixture {
        native: 0,
        flags: 0,
        absorbed: 0,
        fail: "",
        bytes,
        code,
        script: [0x04, 0x0b, 0, 0],
        script_offset: 0,
        initialized: false,
        initialization_status: 0,
        frame: ptr::null_mut(),
        parent: ptr::null_mut(),
        publications: 0,
        result: 0,
        nested: ptr::null_mut(),
        reentries: 0,
        mutate_parent: false,
        locals: [0; 20],
        locals_bytes: 80,
        trailing: 0,
        targets: [0; 20],
        _props: props,
        properties,
        prop_addr: ptr::null_mut(),
        first_constructor: ptr::null_mut(),
        arguments: 0,
        aliases: false,
        mutate_links: false,
        swap_observer: false,
        events: Vec::new(),
    }));
    let raw = f.get();
    unsafe {
        let properties = (*raw).properties;
        for id in 0..17 {
            (*properties.add(id)).owner = raw;
            if id < 16 {
                (*properties.add(id)).next = properties.add(id + 1);
            }
        }
        (*raw).first_constructor = properties;
    }
    (f, raw.cast())
}
unsafe fn event(f: *mut Fixture, value: String) -> i32 {
    unsafe {
        if !(*f).frame.is_null() {
            snapshot(f);
        }
        let fail = (*f).fail == value;
        (&mut (*f).events).push(value);
        if fail { 77 } else { 0 }
    }
}
unsafe extern "C" fn native(c: Object) -> i32 {
    unsafe { (*(c.cast::<Fixture>())).native }
}
unsafe extern "C" fn flags(c: Object) -> u32 {
    unsafe { (*(c.cast::<Fixture>())).flags }
}
unsafe extern "C" fn invoke(c: Object) -> i32 {
    unsafe { event(c.cast(), "native".into()) }
}
unsafe extern "C" fn remote(c: Object, out: *mut i32) -> i32 {
    unsafe {
        let f = c.cast::<Fixture>();
        *out = (*f).absorbed;
        event(f, "remote".into())
    }
}
unsafe extern "C" fn locals_size(c: Object) -> i32 {
    unsafe { (*c.cast::<Fixture>()).locals_bytes }
}
unsafe extern "C" fn frame_object(c: Object) -> Object {
    unsafe { ptr::addr_of_mut!((*c.cast::<Fixture>()).result).cast() }
}
unsafe extern "C" fn frame_node(c: Object) -> Object {
    c
}
unsafe extern "C" fn frame_parent(c: Object) -> *mut FFrame {
    unsafe { (*c.cast::<Fixture>()).parent }
}
unsafe extern "C" fn object_class(_object: Object) -> Object {
    ptr::null_mut()
}
unsafe extern "C" fn script_code(node: Object, offset: i32, out: *mut *mut u8) -> i32 {
    unsafe {
        let f = node.cast::<Fixture>();
        assert_eq!(offset, 0);
        assert!((*f).frame.is_null());
        out.write(ptr::addr_of_mut!((*f).script).cast());
        let status = (*f).initialization_status;
        (*f).initialized = status == 0;
        status
    }
}
unsafe fn assert_frame(f: *mut Fixture) {
    unsafe {
        let frame = (*f).frame;
        assert!(!frame.is_null());
        assert_eq!((*frame).node, f.cast());
        assert_eq!((*frame).object, ptr::addr_of_mut!((*f).result).cast());
        assert_eq!((*frame).code, ptr::addr_of_mut!((*f).script).cast::<u8>().add((*f).script_offset));
        assert!(!(*frame).locals.is_null());
        assert_eq!((*frame).parent, (*f).parent);
    }
}
unsafe extern "C" fn publish_frame(c: Object, frame: *mut FFrame) {
    unsafe {
        let f = c.cast::<Fixture>();
        assert!((*f).initialized);
        assert!((*f).frame.is_null());
        (*f).frame = frame;
        (*f).publications += 1;
        assert_frame(f);
    }
}
const INITIALIZATION: Hp2VmFrameInitializationServices =
    Hp2VmFrameInitializationServices { object_class, script_code };
unsafe extern "C" fn code(c: Object) -> *mut *mut u8 {
    unsafe {
        assert_frame(c.cast());
        ptr::addr_of_mut!((*c.cast::<Fixture>()).code)
    }
}
unsafe fn snapshot(f: *mut Fixture) {
    unsafe {
        assert_frame(f);
        let data = (*(*f).frame).locals;
        ptr::copy_nonoverlapping(data.cast::<i32>(), ptr::addr_of_mut!((*f).locals).cast(), 20);
        (*f).trailing = data.add((*f).locals_bytes as usize - 1).read();
    }
}
unsafe extern "C" fn address(c: Object) -> *mut *mut u8 {
    unsafe { ptr::addr_of_mut!((*c.cast::<Fixture>()).prop_addr) }
}
unsafe extern "C" fn first(c: Object) -> Object {
    unsafe { (*c.cast::<Fixture>()).properties.cast() }
}
unsafe extern "C" fn next(p: Object) -> Object {
    unsafe { (*p.cast::<Property>()).next.cast() }
}
unsafe extern "C" fn property_flags(p: Object) -> u32 {
    unsafe { (*p.cast::<Property>()).flags }
}
unsafe extern "C" fn offset(p: Object) -> i32 {
    unsafe { (*p.cast::<Property>()).offset }
}
unsafe extern "C" fn step(c: Object, dest: *mut u8) -> i32 {
    unsafe {
        let f = c.cast::<Fixture>();
        assert_frame(f);
        (*f).code = (*f).code.add(1);
        let targets = ptr::addr_of_mut!((*f).targets).cast::<i32>();
        if dest.is_null() {
            (*f).prop_addr = targets.add(19).cast();
            return event(f, "debug".into());
        }
        let index = (*f).arguments;
        (*f).arguments += 1;
        dest.cast::<i32>().write(10 + index as i32);
        (*f).prop_addr = targets.add(if (*f).aliases { 0 } else { index }).cast();
        if (*f).mutate_links && index == 0 {
            let p = (*f).properties;
            (*p).flags = 1;
            (*p).next = p.add(2);
        }
        event(f, format!("arg{index}"))
    }
}
unsafe extern "C" fn capture() -> Object {
    OBSERVER.with(|v| v.get() as Object)
}
unsafe extern "C" fn enter(c: Object, observer: Object) -> i32 {
    unsafe {
        let f = c.cast::<Fixture>();
        if (*f).swap_observer {
            OBSERVER.with(|v| v.set(2));
        }
        assert_frame(f);
        let nested = (*f).nested;
        if !nested.is_null() {
            let child = nested.cast::<Fixture>();
            (*child).parent = (*f).frame;
            let status = call(nested);
            (*child).parent = ptr::null_mut();
            assert_frame(f);
            snapshot(f);
            if let Err(status) = status {
                return status;
            }
            (*f).reentries += 1;
        }
        event(f, format!("enter{}", observer as usize))
    }
}
unsafe extern "C" fn process(c: Object) -> i32 {
    unsafe {
        let f = c.cast::<Fixture>();
        let data = (*(*f).frame).locals.cast::<i32>();
        if (*f).mutate_parent {
            let parent = (*(*f).frame).parent;
            assert!(!parent.is_null());
            let owner = (*parent).node.cast::<Fixture>();
            assert_frame(owner);
            (*parent).locals.cast::<i32>().add(19).write(700);
            (*parent).code = (*parent).code.add(1);
            (*owner).script_offset += 1;
            assert_frame(owner);
        }
        for i in 0..20 {
            let value = data.add(i);
            value.write(value.read() + 100);
        }
        if (*f).locals_bytes > 80 {
            let trailing = data.cast::<u8>().add((*f).locals_bytes as usize - 1);
            assert_eq!(trailing.read(), 0);
            trailing.write(0xa5);
        }
        (*f).result = 300;
        event(f, "process".into())
    }
}
unsafe extern "C" fn exit(c: Object, observer: Object) -> i32 {
    unsafe {
        assert_frame(c.cast());
        event(c.cast(), format!("exit{}", observer as usize))
    }
}
unsafe extern "C" fn copy(p: Object, dest: *mut u8, source: *mut u8) -> i32 {
    unsafe {
        let property = p.cast::<Property>();
        let f = (*property).owner;
        assert_frame(f);
        let status = event(f, format!("copy{}", (*property).id));
        if status == 0 {
            dest.cast::<i32>().write(source.cast::<i32>().read());
        }
        status
    }
}
unsafe extern "C" fn first_constructor(c: Object) -> Object {
    unsafe { (*c.cast::<Fixture>()).first_constructor.cast() }
}
unsafe extern "C" fn next_constructor(p: Object) -> Object {
    unsafe { (*p.cast::<Property>()).constructor.cast() }
}
unsafe extern "C" fn destroy(p: Object, _data: *mut u8) -> i32 {
    unsafe {
        let property = p.cast::<Property>();
        let id = (*property).id;
        let f = (*property).owner;
        assert_frame(f);
        if (*f).mutate_links && id == 0 {
            (*property).constructor = (*f).properties.add(2);
        }
        event(f, format!("destroy{id}"))
    }
}
const SERVICES: Hp2VmCallServices = Hp2VmCallServices {
    out_parm: 1,
    native: 0x400,
    end_parms: END,
    debug_info: DEBUG,
    native_index: native,
    function_flags: flags,
    invoke_native: invoke,
    remote,
    locals_size,
    code,
    prop_addr: address,
    first_property: first,
    next_property: next,
    property_flags,
    property_offset: offset,
    step,
    capture_observer: capture,
    observer_enter: enter,
    process_internal: process,
    observer_exit: exit,
    copy,
    first_constructor,
    next_constructor,
    destroy,
    frame_object,
    frame_node,
    frame_parent,
    publish_frame,
    frame_initialization: &INITIALIZATION,
};
unsafe fn call(c: Object) -> Result<(), i32> {
    unsafe {
        let result = vm_call::call(c, &SERVICES);
        // The published frame and locals are dead; clear without reading either.
        (*c.cast::<Fixture>()).frame = ptr::null_mut();
        result
    }
}
unsafe fn consumed(c: Object) -> usize {
    unsafe {
        let f = &*c.cast::<Fixture>();
        f.code.offset_from(f.bytes.as_ptr()) as usize
    }
}

#[test]
fn native_index_preempts_network_flags_and_unabsorbed_native_does_not_parse_arguments() {
    unsafe {
        let (_owner, c) = fixture(2, false);
        let p = c.cast::<Fixture>();
        (*p).native = 7;
        (*p).flags = 0x400;
        call(c).unwrap();
        assert_eq!((*p).events, ["native"]);
        assert_eq!(consumed(c), 0);
        assert_eq!((*p).targets, [0; 20]);
        let (_owner, c) = fixture(2, false);
        let p = c.cast::<Fixture>();
        (*p).flags = 0x400;
        call(c).unwrap();
        assert_eq!((*p).events, ["remote", "native"]);
        assert_eq!(consumed(c), 0);
    }
}
#[test]
fn failed_remote_admission_neither_parses_nor_invokes() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let p = c.cast::<Fixture>();
        (*p).flags = 0x400;
        (*p).fail = "remote";
        assert_eq!(call(c), Err(77));
        assert_eq!(consumed(c), 0);
        assert_eq!((*p).targets, [0; 20]);
        assert_eq!((*p).events, ["remote"]);
    }
}
#[test]
fn absorbed_native_still_eats_debug_and_copies_outputs_without_local_dispatch() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let p = c.cast::<Fixture>();
        (*p).flags = 0x400;
        (*p).absorbed = 2;
        call(c).unwrap();
        assert_eq!(consumed(c), 4);
        assert_eq!(&(&(*p).targets)[..2], &[10, 11]);
        assert_eq!(
            (*p).events,
            [
                "remote", "arg0", "arg1", "debug", "copy1", "copy0", "destroy0"
            ]
        );
    }
}
#[test]
fn script_captures_observer_and_output_targets_and_copies_back_in_reverse() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let p = c.cast::<Fixture>();
        (*p).aliases = true;
        (*p).swap_observer = true;
        call(c).unwrap();
        assert_eq!(consumed(c), 4);
        assert_eq!((*p).targets[0], 110);
        assert_eq!((*p).targets[19], 0);
        assert_eq!(
            (*p).events,
            [
                "arg0", "arg1", "debug", "enter1", "process", "exit1", "copy1", "copy0",
                "destroy0"
            ]
        );
    }
}
#[test]
fn argument_flags_links_and_constructor_successors_are_read_live() {
    unsafe {
        let (_owner, c) = fixture(2, false);
        let p = c.cast::<Fixture>();
        (*p).mutate_links = true;
        let properties = (*p).properties;
        (*properties).flags = 0;
        (*properties).constructor = properties.add(1);
        call(c).unwrap();
        assert_eq!(&(&(*p).targets)[..2], &[110, 111]);
        assert_eq!((*p).locals[1], 100);
        assert_eq!((*p).locals[2], 111);
        assert_eq!(
            (*p).events,
            [
                "arg0", "arg1", "enter1", "process", "exit1", "copy2", "copy0",
                "destroy0", "destroy2"
            ]
        );
    }
}
#[test]
fn failed_argument_preserves_partial_locals_and_cursor_without_copyback_or_destruction() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let p = c.cast::<Fixture>();
        (*p).fail = "arg1";
        assert_eq!(call(c), Err(77));
        assert_eq!(consumed(c), 2);
        assert_eq!(&(&(*p).locals)[..2], &[10, 11]);
        assert_eq!(&(&(*p).locals)[2..], &[0; 18]);
        assert_eq!((*p).targets, [0; 20]);
        assert_eq!((*p).events, ["arg0", "arg1"]);
    }
}
#[test]
fn failed_internal_dispatch_keeps_partial_local_values_and_skips_exit_and_cleanup() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let p = c.cast::<Fixture>();
        (*p).fail = "process";
        assert_eq!(call(c), Err(77));
        assert_eq!(consumed(c), 4);
        assert_eq!(&(&(*p).locals)[..2], &[110, 111]);
        assert_eq!((*p).targets, [0; 20]);
        assert_eq!(
            (*p).events,
            ["arg0", "arg1", "debug", "enter1", "process"]
        );
    }
}
#[test]
fn copyback_failure_preserves_completed_reverse_prefix_and_prevents_destruction() {
    unsafe {
        let (_owner, c) = fixture(2, false);
        let p = c.cast::<Fixture>();
        (*p).fail = "copy0";
        assert_eq!(call(c), Err(77));
        assert_eq!(&(&(*p).targets)[..2], &[0, 111]);
        assert_eq!((&(*p).events).last().unwrap(), "copy0");
        assert!(!(&(*p).events).iter().any(|v| v.starts_with("destroy")));
    }
}
#[test]
fn destructor_failure_does_not_undo_output_copyback_or_follow_a_later_constructor() {
    unsafe {
        let (_owner, c) = fixture(2, false);
        let p = c.cast::<Fixture>();
        (*p).mutate_links = true;
        (*p).fail = "destroy0";
        assert_eq!(call(c), Err(77));
        assert_eq!(&(&(*p).targets)[..2], &[110, 111]);
        assert_eq!((&(*p).events).last().unwrap(), "destroy0");
        assert!(!(&(*p).events).iter().any(|v| v == "destroy2"));
    }
}
#[test]
fn maximum_parameter_count_copies_the_last_slot_and_all_outputs_in_reverse() {
    unsafe {
        let (_owner, c) = fixture(16, false);
        let p = c.cast::<Fixture>();
        call(c).unwrap();
        assert_eq!(consumed(c), 17);
        assert_eq!(
            &(&(*p).targets)[..16],
            &(110..126).collect::<Vec<i32>>()[..]
        );
        let copies = (&(*p).events)
            .iter()
            .filter(|v| v.starts_with("copy"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            copies,
            (0..16)
                .rev()
                .map(|i| format!("copy{i}"))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn failed_initialization_never_publishes_or_mutates_result_arguments_or_cursor() {
    unsafe {
        let (_owner, c) = fixture(2, true);
        let f = c.cast::<Fixture>();
        (*f).initialization_status = 91;
        (*f).result = 12345;
        (*f).targets = [54321; 20];
        let cursor = (*f).code;
        assert_eq!(call(c), Err(91));
        assert!(!(*f).initialized);
        assert_eq!((*f).publications, 0);
        assert!((*f).frame.is_null());
        assert_eq!((*f).code, cursor);
        assert_eq!((*f).result, 12345);
        assert_eq!((*f).targets, [54321; 20]);
        assert_eq!((*f).arguments, 0);
        assert!((*f).events.is_empty());
    }
}

#[test]
fn rust_frames_parent_chain_and_locals_remain_live_through_nested_observer_reentry() {
    unsafe {
        let (_outer_owner, outer) = fixture(2, true);
        let (_inner_owner, inner) = fixture(1, false);
        let outer = outer.cast::<Fixture>();
        let inner = inner.cast::<Fixture>();
        let mut root = FFrame {
            node: ptr::null_mut(),
            object: ptr::null_mut(),
            code: ptr::null_mut(),
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        (*outer).parent = &raw mut root;
        (*outer).nested = inner.cast();
        (*inner).mutate_parent = true;
        (*inner).locals_bytes = 262144;
        call(outer.cast()).unwrap();
        assert_eq!((*outer).reentries, 1);
        assert_eq!((*inner).publications, 1);
        assert_eq!((*outer).publications, 1);
        assert!((*inner).frame.is_null());
        assert!((*outer).frame.is_null());
        assert_eq!((*outer).locals[19], 800);
        assert_eq!((*inner).locals[19], 100);
        assert_eq!((*inner).trailing, 0xa5);
        assert_eq!((*outer).script_offset, 1);
        assert_eq!((*inner).script_offset, 0);
        assert_eq!((*outer).result, 300);
        assert_eq!((*inner).result, 300);
        assert_eq!(consumed(outer.cast()), 4);
        assert_eq!(consumed(inner.cast()), 2);
        assert_eq!(&(&(*outer).targets)[..2], &[110, 111]);
        assert_eq!((*inner).targets[0], 110);
        assert_eq!((*outer).parent, &raw mut root);
        assert!(root.node.is_null());
        assert!(root.object.is_null());
        assert!(root.code.is_null());
        assert!(root.locals.is_null());
        assert!(root.parent.is_null());
    }
}

#[test]
fn nested_large_call_failure_preserves_live_parent_and_next_call_recovers() {
    unsafe {
        let (_outer_owner, outer) = fixture(2, false);
        let (_inner_owner, inner) = fixture(1, false);
        let outer = outer.cast::<Fixture>();
        let inner = inner.cast::<Fixture>();
        (*outer).nested = inner.cast();
        (*inner).mutate_parent = true;
        (*inner).locals_bytes = 262144;
        (*inner).fail = "process";

        assert_eq!(call(outer.cast()), Err(77));
        assert_eq!((*outer).events, ["arg0", "arg1"]);
        assert_eq!((*inner).events, ["arg0", "enter1", "process"]);
        assert_eq!(&(&(*outer).locals)[..2], &[10, 11]);
        assert_eq!((*outer).locals[19], 700);
        assert_eq!((*inner).locals[0], 110);
        assert_eq!((*inner).locals[19], 100);
        assert_eq!((*inner).trailing, 0xa5);
        assert_eq!((*outer).targets, [0; 20]);
        assert_eq!((*inner).targets, [0; 20]);
        assert_eq!((*outer).result, 0);
        assert_eq!((*inner).result, 300);
        assert_eq!(consumed(outer.cast()), 3);
        assert_eq!(consumed(inner.cast()), 2);
        assert!((*outer).frame.is_null());
        assert!((*inner).frame.is_null());

        // Restart independent invocations without reading expired frame storage.
        for f in [outer, inner] {
            (*f).fail = "";
            (*f).code = (&mut (*f).bytes).as_mut_ptr();
            (*f).arguments = 0;
            (*f).script_offset = 0;
            (*f).initialized = false;
            (*f).locals = [-1; 20];
            (*f).result = 0;
            (&mut (*f).events).clear();
        }
        call(outer.cast()).unwrap();
        assert_eq!((*outer).publications, 2);
        assert_eq!((*inner).publications, 2);
        assert_eq!((*outer).reentries, 1);
        assert_eq!((*outer).locals[19], 800);
        assert_eq!((*inner).locals[19], 100);
        assert_eq!((*inner).trailing, 0xa5);
        assert_eq!(&(&(*outer).targets)[..2], &[110, 111]);
        assert_eq!((*inner).targets[0], 110);
        assert_eq!((&(*outer).events).last().unwrap(), "destroy0");
        assert_eq!((&(*inner).events).last().unwrap(), "destroy0");
        assert!((*outer).frame.is_null());
        assert!((*inner).frame.is_null());
    }
}
