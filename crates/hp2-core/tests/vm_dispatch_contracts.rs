use hp2_abi::engine::generated::FFrame;
use hp2_core::vm_dispatch::*;
use hp2_core::vm_frame::Hp2VmFrameInitializationServices;
use hp2_core::vm_registry::Hp2VmNativeHandler;
use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};

const FAILURE: i32 = 73;

struct Internal {
    log: Vec<&'static str>,
    fail: &'static str,
    function_flags: u32,
    flags: u32,
    remote: i32,
    probing: i32,
    code: Vec<u8>,
    pc: usize,
    result: i32,
    scratch: [u8; 1024],
    reenter: bool,
}
impl Internal {
    fn new() -> Self {
        Self {
            log: vec![],
            fail: "",
            function_flags: FUNC_SINGULAR,
            flags: 0x1000,
            remote: 0,
            probing: 1,
            code: vec![0x10, EX_RETURN, 0x11, EX_DEBUG_INFO, 0],
            pc: 0,
            result: -1,
            scratch: [0xa5; 1024],
            reenter: false,
        }
    }
    fn mark(&mut self, name: &'static str) -> i32 {
        self.log.push(name);
        if self.fail == name { FAILURE } else { 0 }
    }
}
unsafe fn internal<'a>(p: *mut c_void) -> &'a mut Internal {
    unsafe { &mut *p.cast() }
}
unsafe extern "C" fn i_function(p: *mut c_void) -> u32 {
    unsafe {
        let s = internal(p);
        s.log.push("function");
        s.function_flags
    }
}
unsafe extern "C" fn i_remote(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let s = internal(p);
        let status = s.mark("remote");
        *out = s.remote;
        status
    }
}
unsafe extern "C" fn i_probe(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let s = internal(p);
        let status = s.mark("probe");
        *out = s.probing;
        status
    }
}
unsafe extern "C" fn i_flags(p: *mut c_void) -> u32 {
    unsafe {
        let s = internal(p);
        s.log.push("flags");
        s.flags
    }
}
unsafe extern "C" fn i_set(p: *mut c_void, flags: u32) -> i32 {
    unsafe {
        let s = internal(p);
        s.flags |= flags;
        s.mark("set")
    }
}
unsafe extern "C" fn i_clear(p: *mut c_void, flags: u32) -> i32 {
    unsafe {
        let s = internal(p);
        s.flags &= !flags;
        s.mark("clear")
    }
}
unsafe extern "C" fn i_enter(p: *mut c_void) -> i32 {
    unsafe { internal(p).mark("enter") }
}
unsafe extern "C" fn i_init(p: *mut c_void) -> i32 {
    unsafe {
        let s = internal(p);
        s.scratch[..std::mem::size_of::<usize>() * 2].fill(0);
        s.mark("initialize")
    }
}
unsafe extern "C" fn i_recursion(p: *mut c_void, limit: i32) -> i32 {
    if limit != hp2_core::vm_guard::HP2_VM_RECURSE_LIMIT {
        return 99;
    }
    unsafe { internal(p).mark("critical") }
}
unsafe extern "C" fn i_opcode(p: *mut c_void) -> u8 {
    unsafe {
        let s = internal(p);
        s.code[s.pc]
    }
}
unsafe extern "C" fn i_advance(p: *mut c_void) {
    unsafe {
        let s = internal(p);
        s.pc += 1;
        s.log.push("consume-return");
    }
}
unsafe extern "C" fn i_step(p: *mut c_void, result: i32) -> i32 {
    unsafe {
        let opcode = {
            let s = internal(p);
            s.code[s.pc]
        };
        let name = match (opcode, result) {
            (0x10, 0) => "body",
            (0x11, 1) => "result",
            (EX_DEBUG_INFO, 1) => "debug",
            _ => return 99,
        };
        let status = internal(p).mark(name);
        if status != 0 {
            return status;
        }
        if opcode == 0x10 && internal(p).reenter {
            internal(p).reenter = false;
            if let Err(error) = process_internal(p, 0, &INTERNAL) {
                return error;
            }
        }
        let s = internal(p);
        s.pc += 1;
        if opcode == 0x11 {
            s.result = 42;
        }
        0
    }
}
unsafe extern "C" fn i_return(p: *mut c_void) -> i32 {
    unsafe { internal(p).mark("relevance-return") }
}
unsafe extern "C" fn i_exit(p: *mut c_void) -> i32 {
    unsafe { internal(p).mark("exit") }
}
static INTERNAL: Hp2VmInternalServices = Hp2VmInternalServices {
    function_flags: i_function,
    remote: i_remote,
    probing: i_probe,
    object_flags: i_flags,
    set_flags: i_set,
    clear_flags: i_clear,
    trace_enter: i_enter,
    initialize_buffer: i_init,
    recursion_error: i_recursion,
    opcode: i_opcode,
    advance: i_advance,
    step: i_step,
    trace_return: i_return,
    trace_exit: i_exit,
};
fn run_internal(s: &mut Internal, guarded: bool) -> Result<(), i32> {
    unsafe { process_internal((s as *mut Internal).cast(), u32::from(guarded), &INTERNAL) }
}

#[test]
fn internal_rejections_short_circuit_without_touching_result_or_state() {
    for gate in 0..3 {
        let mut s = Internal::new();
        match gate {
            0 => s.remote = 1,
            1 => s.probing = 0,
            _ => s.flags |= FUNC_SINGULAR,
        }
        let flags = s.flags;
        assert_eq!(run_internal(&mut s, false), Ok(()));
        let expected: &[&str] = match gate {
            0 => &["function", "remote"],
            1 => &["function", "remote", "probe"],
            _ => &["function", "remote", "probe", "flags"],
        };
        assert_eq!(s.log, expected);
        assert_eq!((s.pc, s.result, s.flags), (0, -1, flags));
        assert_eq!(s.scratch, [0xa5; 1024]);
    }
}

#[test]
fn internal_return_debug_and_reentrant_singular_order() {
    let mut s = Internal::new();
    s.reenter = true;
    assert_eq!(run_internal(&mut s, false), Ok(()));
    assert_eq!(
        s.log,
        [
            "function",
            "remote",
            "probe",
            "flags",
            "enter",
            "set",
            "initialize",
            "body",
            "function",
            "remote",
            "probe",
            "flags",
            "consume-return",
            "result",
            "relevance-return",
            "debug",
            "exit",
            "clear"
        ]
    );
    assert_eq!((s.pc, s.result, s.flags), (4, 42, 0x1000));
    assert!(
        s.scratch[std::mem::size_of::<usize>() * 2..]
            .iter()
            .all(|b| *b == 0xa5)
    );
    let mut no_debug = Internal::new();
    no_debug.code[3] = 0;
    no_debug.function_flags = 0;
    no_debug.flags |= FUNC_SINGULAR;
    assert_eq!(run_internal(&mut no_debug, false), Ok(()));
    assert!(!no_debug.log.contains(&"debug"));
    assert_eq!(no_debug.flags, 0x1000 | FUNC_SINGULAR);
}

#[test]
fn internal_first_errors_leave_exact_prefix_and_sticky_state() {
    let success = [
        "function",
        "remote",
        "probe",
        "flags",
        "enter",
        "set",
        "initialize",
        "body",
        "consume-return",
        "result",
        "relevance-return",
        "debug",
        "exit",
        "clear",
    ];
    for failure in [
        "remote",
        "probe",
        "enter",
        "body",
        "result",
        "relevance-return",
        "debug",
        "exit",
    ] {
        let mut s = Internal::new();
        s.fail = failure;
        assert_eq!(run_internal(&mut s, false), Err(FAILURE));
        let index = success.iter().position(|name| *name == failure).unwrap();
        assert_eq!(s.log, success[..=index]);
        let entered = index >= 7;
        assert_eq!(s.flags & FUNC_SINGULAR != 0, entered);
        assert_eq!(s.result, if index > 9 { 42 } else { -1 });
    }
}

// This is the only guarded test in this integration-test process. All other
// tests select DO_GUARD=0, so the default parallel test runner cannot race the
// production process-global counters. No inspection or counter-seeding API.
#[test]
fn recursion_guard_reports_after_increment_and_does_not_unwind_policy() {
    use hp2_core::vm_guard::{HP2_VM_RECURSE_LIMIT, reset};
    // Observe every original failure boundary through the next real threshold,
    // rather than reading a fixture-owned copy of the production counter.
    for (failure, retained) in [
        ("remote", 0),
        ("probe", 0),
        ("enter", 0),
        ("body", 1),
        ("result", 1),
        ("relevance-return", 1),
        ("debug", 1),
        ("exit", 1),
    ] {
        unsafe { reset(1) };
        let mut failed = Internal::new();
        failed.fail = failure;
        assert_eq!(run_internal(&mut failed, true), Err(FAILURE));
        for _ in 0..HP2_VM_RECURSE_LIMIT - retained {
            let mut prime = Internal::new();
            prime.fail = "body";
            assert_eq!(run_internal(&mut prime, true), Err(FAILURE));
            assert!(!prime.log.contains(&"critical"));
        }
        let mut boundary = Internal::new();
        boundary.fail = "critical";
        assert_eq!(run_internal(&mut boundary, true), Err(FAILURE));
        assert_eq!(boundary.log.last(), Some(&"critical"));
    }
    unsafe { reset(1) };
    for _ in 0..HP2_VM_RECURSE_LIMIT {
        let mut failed = Internal::new();
        failed.fail = "body";
        assert_eq!(run_internal(&mut failed, true), Err(FAILURE));
        assert!(!failed.log.contains(&"critical"));
    }
    let mut s = Internal::new();
    s.fail = "critical";
    assert_eq!(run_internal(&mut s, true), Err(FAILURE));
    assert_eq!(s.flags & FUNC_SINGULAR, FUNC_SINGULAR);
    assert_eq!((s.pc, s.result), (0, -1));
    assert_eq!(s.log.last(), Some(&"critical"));
    let mut disabled = Internal::new();
    disabled.fail = "critical";
    unsafe { reset(0) };
    assert_eq!(run_internal(&mut disabled, false), Ok(()));
    assert!(!disabled.log.contains(&"critical"));
    let mut returning_logger = Internal::new();
    assert_eq!(run_internal(&mut returning_logger, true), Ok(()));
    assert!(returning_logger.log.contains(&"critical"));
    // Returning logging decrements only its own entry, retaining prior failures.
    let mut sticky = Internal::new();
    sticky.fail = "critical";
    assert_eq!(run_internal(&mut sticky, true), Err(FAILURE));
    unsafe { reset(1) };
    let mut recovered = Internal::new();
    recovered.fail = "critical";
    assert_eq!(run_internal(&mut recovered, true), Ok(()));
    assert!(!recovered.log.contains(&"critical"));
}

struct Runtime {
    log: Vec<&'static str>,
    tag: i32,
    clocks: usize,
    unclocks: usize,
    live_frame: *mut FFrame,
    nested_frame_checks: usize,
}
impl Runtime {
    fn new() -> Self {
        Self {
            log: vec![],
            tag: 0,
            clocks: 0,
            unclocks: 0,
            live_frame: null_mut(),
            nested_frame_checks: 0,
        }
    }
}
struct Property {
    offset: i32,
    next: *mut Property,
    destroyed: usize,
}
struct Event {
    runtime: *mut Runtime,
    fail: &'static str,
    scriptable: i32,
    probing: i32,
    pending: i32,
    native_index: i32,
    flags: u32,
    remote: i32,
    parms: [i32; 2],
    redirect_copyback: bool,
    copyback_parameters: Option<[i32; 2]>,
    observed_locals: Option<[i32; 4]>,
    locals_size: i32,
    parameters_size: i32,
    return_offset: i32,
    frame: *mut FFrame,
    published: usize,
    code: [u8; 3],
    properties: Box<[Property; 3]>,
    child: Option<Box<Event>>,
    change_link: bool,
    change_threshold: bool,
}
impl Event {
    fn new(runtime: &mut Runtime) -> Self {
        let mut properties = Box::new([
            Property {
                offset: 0,
                next: null_mut(),
                destroyed: 0,
            },
            Property {
                offset: 8,
                next: null_mut(),
                destroyed: 0,
            },
            Property {
                offset: 12,
                next: null_mut(),
                destroyed: 0,
            },
        ]);
        properties[0].next = addr_of_mut!(properties[1]);
        properties[1].next = addr_of_mut!(properties[2]);
        Self {
            runtime,
            fail: "",
            scriptable: 1,
            probing: 1,
            pending: 0,
            native_index: 0,
            flags: 0,
            remote: 0,
            parms: [7, 8],
            redirect_copyback: false,
            copyback_parameters: None,
            observed_locals: None,
            locals_size: 16,
            parameters_size: 8,
            return_offset: 4,
            frame: null_mut(),
            published: 0,
            code: [0x10, EX_RETURN, 0],
            properties,
            child: None,
            change_link: false,
            change_threshold: false,
        }
    }
    fn mark(&mut self, name: &'static str) -> i32 {
        unsafe {
            (*self.runtime).log.push(name);
        }
        if self.fail == name { FAILURE } else { 0 }
    }
}
unsafe fn event<'a>(p: *mut c_void) -> &'a mut Event {
    unsafe { &mut *p.cast() }
}
unsafe extern "C" fn e_scriptable(p: *mut c_void) -> i32 {
    unsafe {
        let e = event(p);
        e.mark("scriptable");
        e.scriptable
    }
}
unsafe extern "C" fn e_probe(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let e = event(p);
        *out = e.probing;
        e.mark("probe")
    }
}
unsafe extern "C" fn e_pending(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let e = event(p);
        *out = e.pending;
        e.mark("pending")
    }
}
unsafe extern "C" fn e_native(p: *mut c_void) -> i32 {
    unsafe {
        let e = event(p);
        e.mark("native-index");
        e.native_index
    }
}
unsafe extern "C" fn e_flags(p: *mut c_void) -> u32 {
    unsafe {
        let e = event(p);
        e.mark("function");
        e.flags
    }
}
unsafe extern "C" fn e_remote(p: *mut c_void, out: *mut i32) -> i32 {
    unsafe {
        let e = event(p);
        *out = e.remote;
        e.mark("remote")
    }
}
unsafe extern "C" fn e_check(p: *mut c_void) -> i32 {
    unsafe { event(p).mark("check-parms") }
}
unsafe extern "C" fn e_scope(p: *mut c_void) -> i32 {
    unsafe {
        event(p).mark("scope-enter");
        let result = event_accepted(p, &EVENT).err().unwrap_or(0);
        event(p).mark("scope-exit");
        result
    }
}
unsafe extern "C" fn e_tag(p: *mut c_void) -> *mut i32 {
    unsafe { addr_of_mut!((*event(p).runtime).tag) }
}
unsafe extern "C" fn e_clock(p: *mut c_void) -> i32 {
    unsafe {
        let e = event(p);
        let result = e.mark("clock");
        if result == 0 {
            (*e.runtime).clocks += 1;
        }
        result
    }
}
unsafe extern "C" fn e_unclock(p: *mut c_void) -> i32 {
    unsafe {
        let e = event(p);
        if e.frame.is_null() {
            return 91;
        }
        let result = e.mark("unclock");
        if result == 0 {
            (*e.runtime).unclocks += 1;
        }
        result
    }
}
unsafe extern "C" fn e_locals_size(p: *mut c_void) -> i32 {
    unsafe { event(p).locals_size }
}
unsafe extern "C" fn e_parameters(p: *mut c_void) -> *mut u8 {
    unsafe {
        let e = event(p);
        if e.parameters_size == 0 {
            null_mut()
        } else {
            e.copyback_parameters.as_mut().unwrap_or(&mut e.parms).as_mut_ptr().cast()
        }
    }
}
unsafe extern "C" fn e_handler(_: *mut c_void) -> Hp2VmNativeHandler {
    e_dispatch
}
unsafe extern "C" fn e_return_offset(p: *mut c_void) -> i32 {
    unsafe { event(p).return_offset }
}
unsafe extern "C" fn e_object(p: *mut c_void) -> *mut c_void {
    p
}
unsafe extern "C" fn e_node(p: *mut c_void) -> *mut c_void {
    p
}
unsafe extern "C" fn e_object_class(_: *mut c_void) -> *mut c_void {
    null_mut()
}
unsafe extern "C" fn e_script_code(
    node: *mut c_void,
    offset: i32,
    out: *mut *mut u8,
) -> i32 {
    unsafe {
        let e = event(node);
        let status = e.mark("script-code");
        if status != 0 {
            return status;
        }
        if offset < 0 || offset as usize >= e.code.len() {
            return FAILURE;
        }
        out.write(e.code.as_mut_ptr().add(offset as usize));
        0
    }
}
unsafe extern "C" fn e_publish(p: *mut c_void, storage: *mut FFrame) {
    unsafe {
        let runtime = event(p).runtime;
        let parent = (*runtime).live_frame;
        if !parent.is_null()
            && storage != parent
            && (*parent).code == addr_of_mut!(event((*parent).object).code).cast::<u8>().add(1)
            && *(*parent).locals.cast::<[i32; 4]>() == [17, 18, 19, 20]
        {
            // Observe the suspended frame while both Rust stack frames are live.
            (*runtime).nested_frame_checks += 1;
        }
        event(p).published += 1;
        event(p).frame = storage;
        (*runtime).live_frame = storage;
    }
}
unsafe extern "C" fn e_dispatch(
    p: *mut c_void,
    raw_frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let frame = raw_frame.cast::<FFrame>();
        if frame.is_null()
            || frame != event(p).frame
            || (*frame).node != p
            || (*frame).object != p
            || !(*frame).parent.is_null()
            || (*frame).code != addr_of_mut!(event(p).code).cast::<u8>()
            || result != (*frame).locals.wrapping_offset(event(p).return_offset as isize).cast()
        {
            return 92;
        }
        if event(p).locals_size == 0 {
            return event(p).mark("dispatch");
        }
        let before = *(*frame).locals.cast::<[i32; 4]>();
        event(p).observed_locals = Some(before);
        let parameters_size = event(p).parameters_size as usize;
        let expected = if parameters_size == 0 { [0; 4] } else { [7, 8, 0, 0] };
        if before != expected
            || !std::slice::from_raw_parts(
                (*frame).locals.add(parameters_size),
                event(p).locals_size as usize - parameters_size,
            ).iter().all(|byte| *byte == 0)
        {
            return 92;
        }
        let status = event(p).mark("dispatch");
        if status != 0 {
            return status;
        }
        (*frame).code = (*frame).code.add(1);
        (*frame).locals.cast::<[i32; 4]>().write([17, 18, 19, 20]);
        if event(p).return_offset == 4 {
            result.cast::<i32>().write(18);
        }
        event(p).observed_locals = Some(*(*frame).locals.cast::<[i32; 4]>());
        let parent = (*frame).parent;
        let child = event(p)
            .child
            .as_deref_mut()
            .map(|e| e as *mut Event);
        if let Some(child) = child {
            let result = run_event(&mut *child);
            // Re-read the suspended frame after reentry rather than trusting
            // only addresses observed during initialization, including on error.
            if event(p).frame != frame
                || (*event(p).runtime).live_frame != frame
                || (*frame).node != p
                || (*frame).object != p
                || (*frame).parent != parent
                || (*frame).code != addr_of_mut!(event(p).code).cast::<u8>().add(1)
                || *(*frame).locals.cast::<[i32; 4]>() != [17, 18, 19, 20]
            {
                return 95;
            }
            event(p).observed_locals = Some(*(*frame).locals.cast::<[i32; 4]>());
            if let Err(error) = result {
                return error;
            }
        }
        if event(p).redirect_copyback {
            event(p).copyback_parameters = Some([-1; 2]);
            event(p).parameters_size = 4;
        }
        0
    }
}
unsafe extern "C" fn e_first(p: *mut c_void) -> *mut c_void {
    unsafe {
        if event(p).locals_size == 0 { null_mut() } else { addr_of_mut!(event(p).properties[0]).cast() }
    }
}
unsafe extern "C" fn e_next(p: *mut c_void) -> *mut c_void {
    unsafe { (*p.cast::<Property>()).next.cast() }
}
unsafe extern "C" fn e_offset(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<Property>()).offset }
}
unsafe extern "C" fn e_parms_size(p: *mut c_void) -> i32 {
    unsafe { event(p).parameters_size }
}
unsafe extern "C" fn e_destroy(p: *mut c_void, property: *mut c_void) -> i32 {
    unsafe {
        let e = event(p);
        let property = &mut *property.cast::<Property>();
        let expected_parameters = if e.parameters_size == 0 {
            [7, 8]
        } else if e.redirect_copyback {
            [17, -1]
        } else {
            [17, 18]
        };
        if *e.copyback_parameters.as_ref().unwrap_or(&e.parms) != expected_parameters || e.frame.is_null() {
            return 93;
        }
        e.observed_locals = Some(*(*e.frame).locals.cast::<[i32; 4]>());
        let status = e.mark(if property.offset == 8 {
            "destroy-first"
        } else {
            "destroy-second"
        });
        if status != 0 {
            return status;
        }
        property.destroyed += 1;
        (*e.frame).locals.add(property.offset as usize).cast::<i32>().write(0);
        e.observed_locals = Some(*(*e.frame).locals.cast::<[i32; 4]>());
        if e.change_link {
            property.next = null_mut();
        }
        if e.change_threshold {
            e.parameters_size = 16;
        }
        0
    }
}
static FRAME_INITIALIZATION: Hp2VmFrameInitializationServices = Hp2VmFrameInitializationServices {
    object_class: e_object_class,
    script_code: e_script_code,
};
const EVENT: Hp2VmEventServices = Hp2VmEventServices {
    scriptable: e_scriptable,
    probing: e_probe,
    pending_kill: e_pending,
    native_index: e_native,
    function_flags: e_flags,
    remote: e_remote,
    check_parameters: e_check,
    outer_scope: e_scope,
    entry_tag: e_tag,
    clock: e_clock,
    unclock: e_unclock,
    locals_size: e_locals_size,
    parameters: e_parameters,
    native_handler: e_handler,
    return_value_offset: e_return_offset,
    first_constructor: e_first,
    next_constructor: e_next,
    property_offset: e_offset,
    parameters_size: e_parms_size,
    destroy: e_destroy,
    frame_object: e_object,
    frame_node: e_node,
    publish_frame: e_publish,
    frame_initialization: &FRAME_INITIALIZATION,
};
fn run_event(e: &mut Event) -> Result<(), i32> {
    unsafe {
        let runtime = e.runtime;
        let parent = (*runtime).live_frame;
        let result = process_event((e as *mut Event).cast(), &EVENT);
        e.frame = null_mut();
        (*runtime).live_frame = parent;
        result
    }
}

#[test]
fn event_rejection_order_and_remote_absorption() {
    let prefix = [
        "scriptable",
        "probe",
        "pending",
        "native-index",
        "function",
        "remote",
    ];
    for gate in 0..5 {
        let mut runtime = Runtime::new();
        let mut e = Event::new(&mut runtime);
        match gate {
            0 => e.scriptable = 0,
            1 => e.probing = 0,
            2 => e.pending = 1,
            3 => e.native_index = 1,
            _ => {
                e.flags = FUNC_NATIVE;
                e.remote = 1;
            }
        }
        assert_eq!(run_event(&mut e), Ok(()));
        assert_eq!(runtime.log, prefix[..if gate == 4 { 6 } else { gate + 1 }]);
        assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (0, 0, 0));
        assert_eq!(e.parms, [7, 8]);
        assert_eq!(e.observed_locals, None);
    }
}

#[test]
fn event_copyback_precedes_local_only_cleanup_with_live_links() {
    for native in [false, true] {
        let mut runtime = Runtime::new();
        let mut e = Event::new(&mut runtime);
        if native {
            e.flags = FUNC_NATIVE;
        }
        assert_eq!(run_event(&mut e), Ok(()));
        assert_eq!(e.parms, [17, 18]);
        assert_eq!(e.observed_locals, Some([17, 18, 0, 0]));
        assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 1, 1]);
        assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (0, 1, 1));
        assert_eq!(runtime.log.contains(&"remote"), native);
    }
    let mut runtime = Runtime::new();
    let mut e = Event::new(&mut runtime);
    e.change_link = true;
    assert_eq!(run_event(&mut e), Ok(()));
    assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 1, 0]);
    assert_eq!(e.observed_locals, Some([17, 18, 0, 20]));
}

#[test]
fn nested_events_keep_distinct_live_rust_frames_and_clock_only_outermost() {
    let mut runtime = Runtime::new();
    let mut outer = Event::new(&mut runtime);
    let mut child = Event::new(&mut runtime);
    child.locals_size = 262_144;
    child.child = Some(Box::new(Event::new(&mut runtime)));
    outer.child = Some(Box::new(child));
    assert_eq!(run_event(&mut outer), Ok(()));
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (0, 1, 1));
    let child = outer.child.as_ref().unwrap();
    assert_eq!((outer.parms, child.parms), ([17, 18], [17, 18]));
    let grandchild = child.child.as_ref().unwrap();
    assert_eq!(grandchild.parms, [17, 18]);
    for e in [&outer, child.as_ref(), grandchild.as_ref()] {
        assert!(e.frame.is_null());
        assert_eq!(e.observed_locals, Some([17, 18, 0, 0]));
        assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 1, 1]);
    }
    assert!(runtime.live_frame.is_null());
    assert_eq!(runtime.nested_frame_checks, 2);
    assert_eq!(runtime.log.iter().filter(|s| **s == "dispatch").count(), 3);
    assert_eq!(runtime.log.iter().filter(|s| **s == "destroy-first").count(), 3);
}

#[test]
fn event_failures_stop_at_first_error_without_timer_or_local_cleanup() {
    let mut success_runtime = Runtime::new();
    let mut success = Event::new(&mut success_runtime);
    assert_eq!(run_event(&mut success), Ok(()));
    for failure in [
        "probe",
        "pending",
        "check-parms",
        "clock",
        "script-code",
        "dispatch",
        "destroy-first",
        "destroy-second",
        "unclock",
    ] {
        let mut runtime = Runtime::new();
        let mut e = Event::new(&mut runtime);
        e.fail = failure;
        assert_eq!(run_event(&mut e), Err(FAILURE));
        let stop = success_runtime
            .log
            .iter()
            .position(|name| *name == failure)
            .unwrap();
        let mut prefix = success_runtime.log[..=stop].to_vec();
        let index = |name| success_runtime.log.iter().position(|step| *step == name).unwrap();
        if stop >= index("scope-enter") {
            prefix.push("scope-exit");
        }
        assert_eq!(runtime.log, prefix, "{failure}");
        assert_eq!(
            runtime.tag,
            i32::from(stop >= index("clock") && failure != "unclock")
        );
        assert_eq!(runtime.unclocks, 0);
        assert_eq!(e.parms, if stop > index("dispatch") { [17, 18] } else { [7, 8] });
        assert_eq!(e.properties[0].destroyed, 0);
        assert_eq!(e.properties[1].destroyed, usize::from(stop > index("destroy-first")));
        assert_eq!(e.properties[2].destroyed, usize::from(stop > index("destroy-second")));
        assert!(e.frame.is_null());
        assert!(runtime.live_frame.is_null());
    }
}

#[test]
fn initializer_failure_does_not_publish_or_touch_parameters_body_or_destructors() {
    let mut runtime = Runtime::new();
    let mut e = Event::new(&mut runtime);
    e.fail = "script-code";
    assert_eq!(run_event(&mut e), Err(FAILURE));
    assert_eq!(e.published, 0);
    assert!(e.frame.is_null());
    assert!(runtime.live_frame.is_null());
    assert_eq!(e.parms, [7, 8]);
    assert_eq!(e.observed_locals, None);
    assert_eq!(e.code, [0x10, EX_RETURN, 0]);
    assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 0, 0]);
    for phase in ["dispatch", "destroy-first", "destroy-second"] {
        assert!(!runtime.log.contains(&phase));
    }
    // Native scopes unwind, but failure does not repair VM tags or timers.
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (1, 1, 0));
}

#[test]
fn nested_initializer_failure_preserves_suspended_frame_and_locals() {
    let mut runtime = Runtime::new();
    let mut outer = Event::new(&mut runtime);
    let mut child = Event::new(&mut runtime);
    child.fail = "script-code";
    outer.child = Some(Box::new(child));
    assert_eq!(run_event(&mut outer), Err(FAILURE));
    let child = outer.child.as_ref().unwrap();
    assert_eq!((outer.published, child.published), (1, 0));
    assert_eq!(outer.observed_locals, Some([17, 18, 19, 20]));
    assert_eq!(child.observed_locals, None);
    assert_eq!((outer.parms, child.parms), ([7, 8], [7, 8]));
    for e in [&outer, child.as_ref()] {
        assert!(e.frame.is_null());
        assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 0, 0]);
    }
    assert!(runtime.live_frame.is_null());
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (2, 1, 0));
}

#[test]
fn nested_failure_leaves_both_entry_tags_and_outer_copyback_untouched() {
    let mut runtime = Runtime::new();
    let mut outer = Event::new(&mut runtime);
    let mut child = Event::new(&mut runtime);
    child.fail = "destroy-first";
    child.locals_size = 262_144;
    outer.child = Some(Box::new(child));
    assert_eq!(run_event(&mut outer), Err(FAILURE));
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (2, 1, 0));
    assert_eq!(outer.parms, [7, 8]);
    assert_eq!(outer.child.as_ref().unwrap().parms, [17, 18]);
    assert_eq!(outer.properties.each_ref().map(|p| p.destroyed), [0, 0, 0]);
}

#[test]
fn event_remote_failure_stops_before_parameter_check_and_local_calls_skip_remote() {
    let mut runtime = Runtime::new();
    let mut native = Event::new(&mut runtime);
    native.flags = FUNC_NATIVE;
    native.fail = "remote";
    assert_eq!(run_event(&mut native), Err(FAILURE));
    assert_eq!(
        runtime.log,
        [
            "scriptable",
            "probe",
            "pending",
            "native-index",
            "function",
            "remote"
        ]
    );
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (0, 0, 0));
    assert_eq!(native.parms, [7, 8]);
    assert_eq!(native.observed_locals, None);

    let mut local_runtime = Runtime::new();
    let mut local = Event::new(&mut local_runtime);
    local.remote = 1;
    local.fail = "remote";
    assert_eq!(run_event(&mut local), Ok(()));
    assert!(!local_runtime.log.contains(&"remote"));
    assert_eq!(local.parms, [17, 18]);
}

#[test]
fn event_copyback_reloads_parameter_pointer_and_size_after_body() {
    let mut runtime = Runtime::new();
    let mut e = Event::new(&mut runtime);
    e.redirect_copyback = true;
    assert_eq!(run_event(&mut e), Ok(()));
    assert_eq!(e.parms, [7, 8]);
    assert_eq!(e.copyback_parameters, Some([17, -1]));
    assert_eq!(e.observed_locals, Some([17, 18, 0, 0]));
    assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 1, 1]);
}

#[test]
fn event_destructors_reload_live_parameter_threshold() {
    let mut runtime = Runtime::new();
    let mut e = Event::new(&mut runtime);
    e.change_threshold = true;
    assert_eq!(run_event(&mut e), Ok(()));
    assert_eq!(e.parms, [17, 18]);
    assert_eq!(e.observed_locals, Some([17, 18, 0, 20]));
    assert_eq!(e.properties.each_ref().map(|p| p.destroyed), [0, 1, 0]);
}

#[test]
fn event_void_result_sentinel_and_null_zero_size_parameters_are_not_dereferenced() {
    for (size, parameters_size) in [(0, 0), (16, 0), (16, 8)] {
        let mut runtime = Runtime::new();
        let mut e = Event::new(&mut runtime);
        e.locals_size = size;
        e.parameters_size = parameters_size;
        e.return_offset = i32::MAX;
        assert_eq!(run_event(&mut e), Ok(()));
        assert_eq!(e.parms, if parameters_size == 0 { [7, 8] } else { [17, 18] });
        assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (0, 1, 1));
        assert!(e.frame.is_null());
        assert!(runtime.live_frame.is_null());
    }
}

#[test]
fn event_storage_recovers_after_nested_failure_without_repairing_sticky_tags() {
    let mut runtime = Runtime::new();
    let mut failed = Event::new(&mut runtime);
    let mut child = Event::new(&mut runtime);
    child.locals_size = 262_144;
    child.fail = "dispatch";
    failed.child = Some(Box::new(child));
    assert_eq!(run_event(&mut failed), Err(FAILURE));
    assert_eq!(failed.observed_locals, Some([17, 18, 19, 20]));
    assert_eq!(failed.child.as_ref().unwrap().observed_locals, Some([7, 8, 0, 0]));
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (2, 1, 0));

    let mut recovered = Event::new(&mut runtime);
    recovered.locals_size = 262_144;
    assert_eq!(run_event(&mut recovered), Ok(()));
    assert_eq!(recovered.parms, [17, 18]);
    assert_eq!(recovered.observed_locals, Some([17, 18, 0, 0]));
    assert_eq!((runtime.tag, runtime.clocks, runtime.unclocks), (2, 1, 0));
    assert!(recovered.frame.is_null());
    assert!(runtime.live_frame.is_null());
}
