use hp2_abi::engine::generated::FFrame;
use hp2_core::ansi::{Hp2AnsiServices, Tchar};
use hp2_core::vm_frame::{self, Hp2VmFrameServices};
use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::ptr::{addr_of_mut, null, null_mut};

unsafe extern "C" fn ansi_assertion(_: *const c_void, _: *const c_void, _: i32) -> i32 {
    1
}
unsafe extern "C" fn ansi_conversion(_: *const c_void, _: *mut *const c_void) -> i32 {
    1
}
unsafe extern "C" fn ansi_size_error() -> i32 {
    1
}
const ANSI: Hp2AnsiServices = Hp2AnsiServices {
    fail_assert: ansi_assertion,
    to_ansi: ansi_conversion,
    size_exceeds_int_contract: ansi_size_error,
};

#[repr(C)]
struct Frame {
    frame: FFrame,
    env: *mut State,
}
struct Node {
    env: *mut State,
    name: &'static str,
    line: i32,
}
struct State {
    buffer: [Tchar; 1024],
    frames: [Frame; 3],
    nodes: [Node; 3],
    strict: i32,
    logs: Vec<&'static str>,
    formatted: Vec<&'static str>,
    status: i32,
    fail_buffer: bool,
    fail_node: *mut Node,
    change_parent: bool,
    change_node: bool,
    dispatches: Vec<(u32, usize, u8)>,
    native_table: [Option<Hp2VmNativeHandler>; NATIVE_COUNT],
    context: *mut c_void,
    redirect: *mut u8,
}
struct Fixture(Box<UnsafeCell<State>>);
impl Fixture {
    fn new() -> Self {
        let frames = std::array::from_fn(|_| Frame {
            frame: FFrame {
                node: null_mut(),
                object: 0x100usize as *mut c_void,
                code: null_mut(),
                locals: null_mut(),
                parent: null_mut(),
            },
            env: null_mut(),
        });
        let fixture = Self(Box::new(UnsafeCell::new(State {
            buffer: [0; 1024],
            frames,
            nodes: [
                Node {
                    env: null_mut(),
                    name: "Child",
                    line: 17,
                },
                Node {
                    env: null_mut(),
                    name: "Parent",
                    line: 23,
                },
                Node {
                    env: null_mut(),
                    name: "Grand",
                    line: 42,
                },
            ],
            strict: 0,
            logs: Vec::new(),
            formatted: Vec::new(),
            status: 0,
            fail_buffer: false,
            fail_node: null_mut(),
            change_parent: false,
            change_node: false,
            dispatches: Vec::new(),
            native_table: [Some(undefined); NATIVE_COUNT],
            context: null_mut(),
            redirect: null_mut(),
        })));
        unsafe {
            let state = fixture.0.get();
            (*state).native_table[0x81] = Some(positive);
            (*state).native_table[0x82] = Some(reenter);
            (*state).native_table[0xff] = Some(negative);
            for i in 0..3 {
                (*state).frames[i].env = state;
                (*state).nodes[i].env = state;
                (*state).frames[i].frame.node = addr_of_mut!((*state).nodes[i]).cast();
                if i < 2 {
                    (*state).frames[i].frame.parent = addr_of_mut!((*state).frames[i + 1].frame);
                }
            }
            (*state).frames[1].frame.object = 0x200usize as *mut c_void;
            (*state).frames[2].frame.object = 0x300usize as *mut c_void;
        }
        fixture
    }
    fn state(&self) -> *mut State {
        self.0.get()
    }
    fn root(&self) -> *mut FFrame {
        unsafe { addr_of_mut!((*self.state()).frames[0].frame) }
    }
    fn services(&self) -> Hp2VmFrameServices {
        Hp2VmFrameServices {
            critical_name: 91,
            strict: unsafe { addr_of_mut!((*self.state()).strict) },
            ansi: &ANSI,
            buffer,
            outer,
            format_description,
            error,
            warning,
            native_table: unsafe { addr_of_mut!((*self.state()).native_table).cast() },
            logf,
        }
    }
    fn text(&self) -> String {
        unsafe {
            (*self.state())
                .buffer
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| char::from_u32(c as u32).unwrap())
                .collect()
        }
    }
    fn describe(&self) -> Result<(), i32> {
        unsafe { vm_frame::describe(self.root(), &self.services()).map(|_| ()) }
    }
    fn step(&self, context: *mut c_void, result: *mut c_void) -> Result<(), i32> {
        unsafe { vm_frame::step(self.root(), context, result, &self.services()) }
    }
}

unsafe extern "C" fn buffer(frame: *const c_void, out: *mut *mut c_void) -> i32 {
    unsafe {
        let state = (*frame.cast::<Frame>()).env;
        out.write(addr_of_mut!((*state).buffer).cast());
        if (*state).fail_buffer {
            (*state).status
        } else {
            0
        }
    }
}
unsafe extern "C" fn outer(object: *mut c_void) -> *mut c_void {
    object
}
unsafe fn append(out: *mut Tchar, text: &str) {
    unsafe {
        for (i, c) in text.chars().enumerate() {
            out.add(i).write(c as Tchar);
        }
        out.add(text.chars().count()).write(0);
    }
}
unsafe extern "C" fn format_description(
    out: *mut c_void,
    node: *mut c_void,
    outer: *mut c_void,
) -> i32 {
    unsafe {
        let node = node.cast::<Node>();
        let state = (*node).env;
        (&mut *addr_of_mut!((*state).formatted)).push((*node).name);
        if node == (*state).fail_node {
            append(out.cast(), "partial");
            return (*state).status;
        }
        let prefix = if outer as usize == 0x100 {
            ""
        } else {
            "Relocated/"
        };
        append(
            out.cast(),
            &format!("{}{}({})", prefix, (*node).name, (*node).line),
        );
        if node == addr_of_mut!((*state).nodes[0]) {
            if (*state).change_parent {
                (*state).frames[0].frame.parent = addr_of_mut!((*state).frames[2].frame);
                (*state).frames[0].frame.object = 0x400usize as *mut c_void;
            }
            if (*state).change_node {
                (*state).frames[1].frame.node = addr_of_mut!((*state).nodes[2]).cast();
            }
        }
        0
    }
}
unsafe extern "C" fn error(frame: *const c_void, _: *const c_void) -> i32 {
    unsafe {
        let state = (*frame.cast::<Frame>()).env;
        (&mut *addr_of_mut!((*state).logs)).push("fatal");
        (*state).status
    }
}
unsafe extern "C" fn warning(frame: *const c_void, _: *const c_void) -> i32 {
    unsafe {
        let state = (*frame.cast::<Frame>()).env;
        (&mut *addr_of_mut!((*state).logs)).push("warning");
        (*state).status
    }
}
unsafe extern "C" fn logf(frame: *mut c_void, _: *const c_void) -> i32 {
    unsafe {
        let state = (*frame.cast::<Frame>()).env;
        (&mut *addr_of_mut!((*state).logs)).push("logf");
        (*state).status
    }
}
unsafe extern "C" fn undefined(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    67
}
unsafe extern "C" fn positive(
    context: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe { invoke_operand(context, frame, result, 0x81) }
}
unsafe extern "C" fn negative(
    context: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe { invoke_operand(context, frame, result, 0xff) }
}
unsafe fn invoke_operand(
    context: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
    opcode: u32,
) -> i32 {
    unsafe {
        let frame = frame.cast::<Frame>();
        let state = (*frame).env;
        (*state).context = context;
        let operand = (*frame).frame.code.read();
        (&mut *addr_of_mut!((*state).dispatches)).push((opcode, (*frame).frame.code as usize, operand));
        (*frame).frame.code = (*frame).frame.code.add(1);
        let value = if opcode == 0x81 {
            u32::from(operand)
        } else {
            (-(operand as i32)) as u32
        };
        if !result.is_null() {
            result.cast::<u32>().write_unaligned(value);
        }
        if !(*state).redirect.is_null() {
            (*frame).frame.code = (*state).redirect;
        }
        (*state).status
    }
}

unsafe extern "C" fn reenter(
    context: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let state = (*frame.cast::<Frame>()).env;
        // Replace a handler while the outer native is active. Nested Step must
        // load the current table entry, not a cached table or borrowed slice.
        addr_of_mut!((*state).native_table)
            .cast::<Option<Hp2VmNativeHandler>>()
            .add(0xff)
            .write(Some(negative));
        let services = context.cast::<Hp2VmFrameServices>();
        if let Err(status) = vm_frame::step(frame.cast(), context, result, &*services) {
            return status;
        }
        match vm_frame::step(frame.cast(), context, result, &*services) {
            Ok(()) => 0,
            Err(status) => status,
        }
    }
}

#[test]
fn description_uses_root_outer_for_the_entire_parent_chain() {
    let f = Fixture::new();
    f.describe().unwrap();
    assert_eq!(f.text(), "Child(17)<Parent(23)<Grand(42)");
}

#[test]
fn skipped_root_and_intermediate_nodes_preserve_leading_separator() {
    let f = Fixture::new();
    unsafe {
        (*f.root()).node = null_mut();
        (*f.state()).frames[1].frame.node = null_mut();
    }
    f.describe().unwrap();
    assert_eq!(f.text(), "<Grand(42)");
}

#[test]
fn null_object_clears_old_scratch_without_formatting_parents() {
    let f = Fixture::new();
    unsafe {
        append(addr_of_mut!((*f.state()).buffer).cast(), "stale");
        (*f.root()).object = null_mut();
    }
    f.describe().unwrap();
    assert_eq!(f.text(), "");
    assert!(unsafe { &(*f.state()).formatted }.is_empty());
}

#[test]
fn description_reloads_parent_and_root_object_after_formatting() {
    let f = Fixture::new();
    unsafe {
        (*f.state()).change_parent = true;
    }
    f.describe().unwrap();
    assert_eq!(f.text(), "Child(17)<Relocated/Grand(42)");
    assert_eq!(unsafe { &(*f.state()).formatted }, &["Child", "Grand"]);
}

#[test]
fn description_observes_next_node_changed_by_prior_formatting() {
    let f = Fixture::new();
    unsafe {
        (*f.state()).change_node = true;
        (*f.state()).frames[1].frame.parent = null_mut();
    }
    f.describe().unwrap();
    assert_eq!(f.text(), "Child(17)<Grand(42)");
}

#[test]
fn format_error_preserves_partial_scratch_and_stops_before_later_frames() {
    let f = Fixture::new();
    unsafe {
        (*f.state()).fail_node = addr_of_mut!((*f.state()).nodes[1]);
        (*f.state()).status = 73;
    }
    assert_eq!(f.describe(), Err(73));
    assert_eq!(f.text(), "Child(17)<partial");
    assert_eq!(unsafe { &(*f.state()).formatted }, &["Child", "Parent"]);
}

#[test]
fn buffer_error_does_not_clear_previous_scratch_or_traverse_frames() {
    let f = Fixture::new();
    unsafe {
        append(addr_of_mut!((*f.state()).buffer).cast(), "stale");
        (*f.state()).status = 79;
        (*f.state()).fail_buffer = true;
    }
    assert_eq!(f.describe(), Err(79));
    assert_eq!(f.text(), "stale");
    assert!(unsafe { &(*f.state()).formatted }.is_empty());
}

#[test]
fn utf32_description_places_separator_after_unicode_scalars() {
    let f = Fixture::new();
    unsafe {
        (*f.state()).nodes[0].name = "巫師Δ";
        (*f.state()).frames[1].frame.parent = null_mut();
    }
    f.describe().unwrap();
    assert_eq!(f.text(), "巫師Δ(17)<Parent(23)");
}

#[test]
fn critical_event_short_circuits_strict_read_and_propagates_fatal_error() {
    let f = Fixture::new();
    let mut s = f.services();
    s.strict = null();
    unsafe {
        (*f.state()).status = 83;
    }
    assert_eq!(
        unsafe { vm_frame::serialize(f.root().cast(), null(), 91, &s) },
        Err(83)
    );
    assert_eq!(unsafe { &(*f.state()).logs }, &["fatal"]);
}

#[test]
fn noncritical_severity_observes_current_nonzero_strict_flag() {
    let f = Fixture::new();
    let s = f.services();
    for strict in [0, -7, 0] {
        unsafe {
            (*f.state()).strict = strict;
            vm_frame::serialize(f.root().cast(), null(), 12, &s).unwrap();
        }
    }
    assert_eq!(
        unsafe { &(*f.state()).logs },
        &["warning", "fatal", "warning"]
    );
}

#[test]
fn opcode_is_consumed_before_dispatch_and_result_alias_cannot_change_selection() {
    let f = Fixture::new();
    let mut code = [0x81u8, 0x42, 0, 0, 0, 0];
    let code_pointer = code.as_mut_ptr();
    unsafe {
        (*f.root()).code = code_pointer;
    }
    let mut context = 17u32;
    let context_pointer = (&raw mut context).cast();
    f.step(context_pointer, code_pointer.cast()).unwrap();
    assert_eq!(unsafe { (*f.state()).context }, context_pointer);
    assert_eq!(u32::from_ne_bytes(code[..4].try_into().unwrap()), 0x42);
    assert_eq!(
        unsafe { &(*f.state()).dispatches },
        &[(0x81, unsafe { code.as_ptr().add(1) } as usize, 0x42)]
    );
    assert_eq!(unsafe { (*f.root()).code }, unsafe {
        code.as_mut_ptr().add(2)
    });
}

#[test]
fn dispatch_failure_keeps_consumption_output_and_reentrant_cursor_redirection() {
    let f = Fixture::new();
    let mut code = [0xffu8, 0x55];
    let mut redirected = [0x11u8, 0x22];
    let mut result = [0xa5u8; 6];
    unsafe {
        (*f.root()).code = code.as_mut_ptr();
        (*f.state()).redirect = redirected.as_mut_ptr();
        (*f.state()).status = 89;
    }
    assert_eq!(
        f.step(null_mut(), unsafe { result.as_mut_ptr().add(1) }.cast()),
        Err(89)
    );
    assert_eq!(result[0], 0xa5);
    assert_eq!(result[5], 0xa5);
    assert_eq!(
        u32::from_ne_bytes(result[1..5].try_into().unwrap()),
        (-85i32) as u32
    );
    assert_eq!(unsafe { (*f.root()).code }, redirected.as_mut_ptr());
    assert_eq!(
        unsafe { &(*f.state()).dispatches },
        &[(255, unsafe { code.as_ptr().add(1) } as usize, 0x55)]
    );
}

#[test]
fn unregistered_opcode_invokes_the_initialized_undefined_handler() {
    let f = Fixture::new();
    let mut code = [0x33u8, 0x44];
    unsafe {
        (*f.root()).code = code.as_mut_ptr();
    }
    assert_eq!(f.step(null_mut(), null_mut()), Err(67));
    assert_eq!(unsafe { (*f.root()).code }, unsafe { code.as_mut_ptr().add(1) });
    assert!(unsafe { &(*f.state()).dispatches }.is_empty());
}

#[test]
fn reentry_observes_live_table_and_stops_at_the_first_native_failure() {
    let f = Fixture::new();
    let mut code = [0x82u8, 0xff, 0x55, 0x81, 0x42];
    let mut result = 0u32;
    let mut services = f.services();
    unsafe {
        (*f.root()).code = code.as_mut_ptr();
        (*f.state()).native_table[0xff] = Some(positive);
        (*f.state()).status = 89;
    }
    assert_eq!(
        f.step((&raw mut services).cast(), (&raw mut result).cast()),
        Err(89)
    );
    assert_eq!(result, (-85i32) as u32);
    assert_eq!(unsafe { (*f.root()).code }, unsafe { code.as_mut_ptr().add(3) });
    assert_eq!(
        unsafe { &(*f.state()).dispatches },
        &[(0xff, unsafe { code.as_ptr().add(2) } as usize, 0x55)]
    );
}

#[test]
fn successful_reentry_continues_from_the_live_cursor() {
    let f = Fixture::new();
    let mut code = [0x82u8, 0xff, 0x55, 0x81, 0x42];
    let mut result = 0u32;
    let mut services = f.services();
    unsafe {
        (*f.root()).code = code.as_mut_ptr();
    }
    f.step((&raw mut services).cast(), (&raw mut result).cast()).unwrap();
    assert_eq!(result, 0x42);
    assert_eq!(unsafe { (*f.root()).code }, unsafe { code.as_mut_ptr().add(5) });
    assert_eq!(
        unsafe { &(*f.state()).dispatches },
        &[
            (0xff, unsafe { code.as_ptr().add(2) } as usize, 0x55),
            (0x81, unsafe { code.as_ptr().add(4) } as usize, 0x42),
        ]
    );
}

#[test]
fn initialization_preserves_early_field_mutation_and_first_failure_during_reentry() {
    unsafe extern "C" fn object_class(object: *mut c_void) -> *mut c_void {
        object
    }
    unsafe extern "C" fn script_code(node: *mut c_void, offset: i32, out: *mut *mut u8) -> i32 {
        unsafe {
            let state = (*(node.cast::<Node>())).env;
            let root = addr_of_mut!((*state).frames[0].frame);
            assert_eq!((*root).node, node);
            assert_eq!((*root).object, addr_of_mut!((*state).frames[2].frame).cast());
            (*root).node = addr_of_mut!((*state).nodes[2]).cast();
            (*root).object = addr_of_mut!((*state).nodes[1]).cast();
            vm_frame::initialize_object(
                addr_of_mut!((*state).frames[1].frame),
                node,
                &INITIALIZATION,
            );
            out.write(addr_of_mut!((*state).buffer).cast::<u8>().offset(offset as isize));
            (*state).status
        }
    }
    const INITIALIZATION: vm_frame::Hp2VmFrameInitializationServices =
        vm_frame::Hp2VmFrameInitializationServices { object_class, script_code };

    for failure in [0, 73] {
        let fixture = Fixture::new();
        unsafe {
            let state = fixture.state();
            let root = fixture.root();
            let node = addr_of_mut!((*state).nodes[0]).cast();
            let object = addr_of_mut!((*state).frames[2].frame).cast();
            let old_code = addr_of_mut!((*state).buffer).cast::<u8>().add(1);
            let old_locals = old_code.add(1);
            let locals = old_code.add(7);
            let old_parent = (*root).parent;
            let parent = addr_of_mut!((*state).frames[2].frame);
            (*state).status = failure;
            (*root).code = old_code;
            (*root).locals = old_locals;
            let result = vm_frame::initialize(root, object, node, 3, locals, parent, &INITIALIZATION);
            assert_eq!(result, if failure == 0 { Ok(()) } else { Err(failure) });
            assert_eq!((*root).node, addr_of_mut!((*state).nodes[2]).cast());
            assert_eq!((*root).object, addr_of_mut!((*state).nodes[1]).cast());
            let nested = addr_of_mut!((*state).frames[1].frame);
            assert_eq!((*nested).node, node);
            assert_eq!((*nested).object, node);
            assert!((*nested).code.is_null() && (*nested).locals.is_null() && (*nested).parent.is_null());
            assert_eq!((*root).code, if failure == 0 { old_code.add(2) } else { old_code });
            assert_eq!((*root).locals, if failure == 0 { locals } else { old_locals });
            assert_eq!((*root).parent, if failure == 0 { parent } else { old_parent });
        }
    }
}
