//! Real-VM comparisons for the same-host Let RHS capability.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{
    core_access::{RuntimeError, VmObservation},
    vm_owner::StepControl,
    ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision,
};
use std::ffi::c_void;

// Exercise the actual Engine policy source without copying its arithmetic or
// adding a Cargo dependency from Core to Engine. This file uses only Core/ABI.
#[path = "../../../hp2-engine/src/ui_sports_style.rs"]
mod lesson_style;

const INITIAL: i32 = -31;
const VALUE: i32 = 123;

thread_local! {
    static TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    // Fixture records only. No frame pointer or current-frame borrow is stored.
    static MASK_FUNCTION: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static MASK_LOCAL: Cell<*mut UProperty> = const { Cell::new(ptr::null_mut()) };
    static MASK_PROBES: Cell<u32> = const { Cell::new(0) };
    static CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static BASE: Cell<usize> = const { Cell::new(0) };
    static SNAPSHOT_LEN: Cell<usize> = const { Cell::new(0) };
    static SNAPSHOT_BEFORE: Cell<u32> = const { Cell::new(0) };
    static SNAPSHOT_AFTER: Cell<u32> = const { Cell::new(0) };
    static SNAPSHOT_AFTER_ERROR: Cell<u32> = const { Cell::new(0) };
    static ROOT: Cell<usize> = const { Cell::new(0) };
    static RHS: Cell<usize> = const { Cell::new(0) };
    static ACTUAL_TOP: Cell<usize> = const { Cell::new(0) };
    static SCRATCH: Cell<usize> = const { Cell::new(0) };
    static DESTINATION: Cell<usize> = const { Cell::new(0) };
    static CONTEXT: Cell<usize> = const { Cell::new(0) };
    static CHILD_RESULT: Cell<usize> = const { Cell::new(0) };
    static CHILD_VALUE: Cell<Option<i32>> = const { Cell::new(None) };
    static EVALUATIONS: Cell<u32> = const { Cell::new(0) };
    static STYLE: Cell<Option<hp2_core::ScriptAssignmentValue>> = const { Cell::new(None) };
    static POSTS: Cell<u32> = const { Cell::new(0) };
    static FAIL_POST: Cell<bool> = const { Cell::new(false) };
    static FAIL_WARNING: Cell<bool> = const { Cell::new(false) };
    static SINK_CASE: Cell<bool> = const { Cell::new(false) };
    static SINK_ZERO: Cell<bool> = const { Cell::new(false) };
    static EXIT_PC: Cell<Option<usize>> = const { Cell::new(None) };
    static EXIT_SEEN: Cell<bool> = const { Cell::new(false) };
    static LOGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CONTROLS: RefCell<Vec<StepControl>> = const { RefCell::new(Vec::new()) };
}

fn output(_: CoreAccess<'_>, _: i32, text: &[u32]) -> RuntimeResult<()> {
    let message: String = text.iter().filter_map(|&unit| char::from_u32(unit)).collect();
    if message.contains("Accessed None") || message.contains("Attempt to assigned variable through None") {
        LOGS.with(|logs| logs.borrow_mut().push(message.clone()));
        if FAIL_WARNING.with(Cell::get) && message.contains("Attempt to assigned variable through None") {
            return Err(RuntimeError::new("assignment warning failure"));
        }
    }
    Ok(())
}
fn observe(_: CoreAccess<'_>, event: VmObservation) -> RuntimeResult<()> {
    match event {
        VmObservation::FunctionExit { function, frame, .. } if function == TARGET.with(Cell::get) => {
            // SAFETY: the observer borrows the active frame only for this call.
            let code = unsafe { (*frame).code };
            EXIT_PC.with(|slot| slot.set((!code.is_null()).then(|| (code as usize).wrapping_sub(BASE.with(Cell::get)))));
            EXIT_SEEN.with(|slot| slot.set(true));
        }
        VmObservation::FunctionExit { function, result, .. } if function == CHILD.with(Cell::get) => {
            assert!(!result.is_null(), "child has a reflected i32 result");
            CHILD_RESULT.with(|slot| slot.set(result as usize));
            // SAFETY: the live child result holds its completed reflected i32.
            CHILD_VALUE.with(|slot| slot.set(Some(unsafe { result.cast::<i32>().read_unaligned() })));
        }
        _ => {}
    }
    Ok(())
}
unsafe fn post(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    POSTS.with(|slot| slot.set(slot.get() + 1));
    if FAIL_POST.with(Cell::get) { return Err(RuntimeError::new("assignment child post failure")); }
    Ok(())
}

/// # Safety
/// The active Let owner supplies the current fully writing typed RHS and its
/// actual destination: i32 for delegation, one byte for a selected style case.
unsafe fn evaluate(access: CoreAccess<'_>, arguments: &ScriptCallArguments<'_>, frame: *mut FFrame, context: *mut c_void, destination: *mut c_void) -> RuntimeResult<()> {
    assert!(!frame.is_null()); assert!(!context.is_null()); assert!(!destination.is_null());
    let expected = (BASE.with(Cell::get) as *const u8, SNAPSHOT_LEN.with(Cell::get));
    assert!(!expected.0.is_null() && expected.1 != 0, "fixture captured immutable body identity");
    // SAFETY: Let supplies its current live frame to this synchronous callback.
    let object = unsafe { (*frame).object.cast::<UObject>() };
    assert!(!access.current_script_body_matches(frame, object, TARGET.with(Cell::get), expected.0, expected.1), "Let body scope is masked during RHS evaluation");
    if TARGET.with(Cell::get) == MASK_FUNCTION.with(Cell::get) {
        let local = MASK_LOCAL.with(Cell::get);
        assert!(!local.is_null(), "normal fixture scalar property remains live");
        // SAFETY: the operand callback passes the owner frame only as an
        // opaque identity. Its body scope is masked, so the query rejects
        // before any frame, local storage, or property dereference.
        assert!(!unsafe { access.current_script_scalar_local_matches(
            frame, local, hp2_core::property_value::Hp2ValueKind::ValueInt, 0,
        ) }, "masked outer body cannot admit the formerly current local");
        MASK_PROBES.with(|count| count.set(count.get() + 1));
    }
    assert_eq!(arguments.body_script_snapshot(), expected, "Let capability carries captured body identity");
    SNAPSHOT_BEFORE.with(|slot| slot.set(slot.get() + 1));
    EVALUATIONS.with(|slot| slot.set(slot.get() + 1));
    CONTEXT.with(|slot| slot.set(context as usize));
    DESTINATION.with(|slot| slot.set(destination as usize));
    // The two fixture RHS values use the current frame object as their context.
    // SAFETY: the callback borrows the current live frame synchronously.
    assert_eq!(unsafe { (*frame).object }, context);
    if SINK_CASE.with(Cell::get) {
        // SAFETY: only canonical null-assignment cases set SINK_CASE. Let has
        // just installed and zeroed this complete 16-byte assignment sink. The
        // temporary slice ends before any operand callback or possible write.
        let zero = unsafe { std::slice::from_raw_parts(destination.cast::<u8>(), 16) }.iter().all(|&byte| byte == 0);
        SINK_ZERO.with(|slot| slot.set(zero));
    }
    let outcome = if let Some(evaluator) = STYLE.with(Cell::get) {
        // SAFETY: selected immutable fixture programs publish one byte;
        // conversion children write complete i32 temporaries when successful.
        unsafe { evaluator(access, arguments, frame, context, destination) }
    } else {
        // SAFETY: the current RHS is a complete Final child expression
        // returning i32, and Let supplied its actual four-byte destination.
        unsafe { arguments.step_argument(frame, destination) }
    };
    assert_eq!(arguments.body_script_snapshot(), expected, "original Let snapshot survives delegation or error");
    SNAPSHOT_AFTER.with(|slot| slot.set(slot.get() + 1));
    if outcome.is_err() { SNAPSHOT_AFTER_ERROR.with(|slot| slot.set(slot.get() + 1)); }
    outcome
}

/// # Safety
/// This callback is installed on fixture functions with live admitted scripts.
unsafe fn replacement(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    // Before the argument owner masks the scope, verify the identity captured
    // from the fixture's unchanged code allocation before execute_event.
    assert!(access.current_script_body_matches(frame, object, function, BASE.with(Cell::get) as *const u8, SNAPSHOT_LEN.with(Cell::get)), "pre-mask original body identity");
    let mask_local = MASK_LOCAL.with(Cell::get);
    let check_mask = function == MASK_FUNCTION.with(Cell::get);
    if check_mask {
        // SAFETY: this is the current replacement; the fixture retains its
        // live canonical local, aligned owner allocation, and metadata.
        assert!(unsafe { access.current_script_scalar_local_matches(
            frame, mask_local, hp2_core::property_value::Hp2ValueKind::ValueInt, 0,
        ) }, "the exact local is admitted immediately before masking");
    }
    let mut state = ScriptBodyStatements::new();
    let scratch = state.result();
    SCRATCH.with(|slot| slot.set(scratch as usize));
    loop {
        // SAFETY: this is the current active replacement and its frame/snapshot.
        let Some(captured) = (unsafe { state.begin(access, frame)? }) else { break; };
        let control = if captured as usize == BASE.with(Cell::get) {
            // SAFETY: this already-guarded live root and any redirected operand
            // are complete typed fixture instructions. Expected addresses are
            // identities in the retained immutable script, not forced cursors.
            unsafe { access.execute_script_let_with_rhs(frame, state.result(), ROOT.with(Cell::get) as *const u8, RHS.with(Cell::get) as *const u8, evaluate)? }
        } else {
            // SAFETY: begin has already guarded this complete live statement.
            unsafe { access.execute_script_body_step(frame, state.result())? }
        };
        if check_mask {
            // SAFETY: the selected operation has returned and restored this
            // replacement scope; the same local records remain live.
            assert!(unsafe { access.current_script_scalar_local_matches(
                frame, mask_local, hp2_core::property_value::Hp2ValueKind::ValueInt, 0,
            ) }, "the same local is admitted again after the masked operation");
        }
        CONTROLS.with(|values| values.borrow_mut().push(control));
        if control == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: this is the fixture's newly constructed i32 property.
    unsafe { (*property).array_dim = 1; (*property).element_size = 4; (*property).offset = i32::try_from(offset).unwrap(); (*property).property_flags = flags; }
    property
}
fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: the indexed property/function stays live throughout all calls.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}
fn call(code: &mut Vec<u8>, child: *mut UFunction) { operand(code, 0x1c, child.cast()); code.push(0x16); }

struct DetachScripts(Vec<*mut UFunction>);
impl Drop for DetachScripts {
    fn drop(&mut self) {
        // SAFETY: this guard is dropped before the Core owner and before every
        // predeclared backing buffer, including on partial setup or unwind.
        unsafe { for &function in &self.0 { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    }
}

// Construct after DetachScripts, before any binding or observer install.
// Unwinding drops binding guards first, then this cleanup, then scripts.
struct Cleanup<'a>(CoreAccess<'a>);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        self.0.clear_vm_observer(observe);
        TARGET.with(|v| v.set(ptr::null_mut())); CHILD.with(|v| v.set(ptr::null_mut()));
        MASK_FUNCTION.with(|v| v.set(ptr::null_mut())); MASK_LOCAL.with(|v| v.set(ptr::null_mut())); MASK_PROBES.with(|v| v.set(0));
        BASE.with(|v| v.set(0)); SNAPSHOT_LEN.with(|v| v.set(0)); ROOT.with(|v| v.set(0)); RHS.with(|v| v.set(0));
        ACTUAL_TOP.with(|v| v.set(0)); SCRATCH.with(|v| v.set(0)); DESTINATION.with(|v| v.set(0)); CONTEXT.with(|v| v.set(0)); CHILD_RESULT.with(|v| v.set(0));
        STYLE.with(|v| v.set(None)); FAIL_POST.with(|v| v.set(false)); FAIL_WARNING.with(|v| v.set(false)); SINK_CASE.with(|v| v.set(false));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    error: Option<(Vec<u32>, Vec<Vec<u32>>)>,
    top: i32,
    pos: i32,
    logs: Vec<String>,
    posts: u32,
    child_value: Option<i32>,
    child_is_top: bool,
    exit_seen: bool,
    exit_pc: Option<usize>,
}

#[test]
fn assignment_value_delegation_matches_stock_and_respects_live_destinations() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("assignment-value-vm-{}", std::process::id()));
    assert!(!root.exists(), "use a fresh isolated artifact directory");
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] { fs::copy(source.join(from), root.join("System").join(to)).unwrap(); }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: the owner retains the class and records throughout the fixture.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>();
    let top = unsafe { int_property(access, class.cast(), "AssignmentValueTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let mut child_code = vec![0x04, 0x2c, VALUE as u8];
    let mut normal_code = Vec::new();
    let mut null_code = Vec::new();
    let mut stop_code = vec![hp2_abi::engine::generated::EX_Stop as u8, 0x04, 0x0b];
    let mut style_codes: [Vec<u8>; 12] = std::array::from_fn(|_| Vec::new());
    let mut detach = DetachScripts(Vec::with_capacity(4 + style_codes.len()));
    let cleanup = Cleanup(access);
    let child = unsafe { function(access, class, "AssignmentValueChild", &mut child_code) };
    detach.0.push(child);
    let return_property = unsafe { int_property(access, child.cast(), "ReturnValue", 0, 0x480) };
    // SAFETY: this child has one live reflected four-byte return parameter.
    unsafe { (*child).num_parms = 1; (*child).parms_size = 4; (*child).return_value_offset = 0; (*child.cast::<UStruct>()).properties_size = 4; (*child.cast::<UStruct>()).children = return_property.cast(); }
    CHILD.with(|slot| slot.set(child));
    normal_code.push(0x0f); operand(&mut normal_code, 0x01, top.cast());
    let normal_rhs = normal_code.len(); call(&mut normal_code, child);
    let normal_return = normal_code.len(); normal_code.extend([0x04, 0x0b, 0x2c, 88]);
    let normal = unsafe { function(access, class, "AssignmentValueNormal", &mut normal_code) };
    detach.0.push(normal);
    // Give this existing synthetic script one real function-owned scalar.
    // Its bytecode does not read it; both stock and replacement use the same
    // reflected layout. Only address/metadata admission is tested.
    let mask_local = unsafe { int_property(access, normal.cast(), "AssignmentMaskLocal", 0, 0) };
    // SAFETY: normal and mask_local are fixture-owned live canonical records.
    unsafe {
        (*mask_local.cast::<UField>()).next = ptr::null_mut();
        (*normal.cast::<UStruct>()).children = mask_local.cast();
        (*normal.cast::<UStruct>()).properties_size = 4;
        (*normal.cast::<UStruct>()).properties_align = 4;
    }
    MASK_FUNCTION.with(|slot| slot.set(normal)); MASK_LOCAL.with(|slot| slot.set(mask_local));
    // Let Context(None, InstanceVariable(top)) = child(). Its 5-byte skip is
    // the complete encoded InstanceVariable; width=4 is the i32 LHS shape.
    null_code.extend([0x0f, 0x19, 0x2a, 5, 0, 4]); operand(&mut null_code, 0x01, top.cast());
    let null_rhs = null_code.len(); call(&mut null_code, child);
    let null_return = null_code.len(); null_code.extend([0x04, 0x0b]);
    let null = unsafe { function(access, class, "AssignmentValueNull", &mut null_code) };
    detach.0.push(null);
    let stop = unsafe { function(access, class, "AssignmentValueStop", &mut stop_code) };
    detach.0.push(stop);
    let byte = unsafe { access.construct_object(access.native_class("ByteProperty").unwrap(), class.cast(), access.intern_str("AssignmentByte", FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: the same live four-byte top field permits a one-byte property at
    // its start. Both paths must retain all three neighboring bytes.
    unsafe { (*byte).array_dim = 1; (*byte).element_size = 1; (*byte).offset = std::mem::offset_of!(UTextBuffer, top) as i32; }
    let mut style_functions = Vec::with_capacity(style_codes.len());
    let mut style_rhs = Vec::with_capacity(style_codes.len());
    for (index, code) in style_codes.iter_mut().enumerate() {
        code.push(0x0f);
        if index >= 10 {
            // The complete one-byte property expression is skipped on None.
            code.extend([0x19, 0x2a, 5, 0, 1]);
        }
        operand(code, 0x01, byte.cast());
        style_rhs.push(code.len());
        match index {
            0 | 8 | 10 => code.extend([0x3e, 0x2c, 255]),
            1 => code.extend([0x3e, 0x2c, 0]),
            2 => code.extend([0x3e, 0x2c, 128]),
            3 | 11 => code.extend([0x24, 0]),
            4 => code.extend([0x24, 255]),
            5 => code.extend([0x24, 128]),
            6 => { code.push(0x3e); call(code, child); }
            7 => code.extend([0x24, 111]),
            9 => code.extend([0x3e, 0x28]), // fully writing Bool False -> i32 -> byte
            _ => unreachable!("all fixture programs enumerated"),
        }
        code.extend([0x04, 0x0b]);
        let function = unsafe { function(access, class, &format!("AssignmentStyle{index}"), code) };
        detach.0.push(function);
        style_functions.push(function);
    }
    let post_guard = unsafe { access.install_script_post_call_hook([child, child], post).unwrap() };
    access.set_vm_observer(Some(observe));

    let run = |function: *mut UFunction, code: &mut Vec<u8>, replaced: bool, root_offset: usize, rhs_offset: usize, sink: bool, fail_post: bool, fail_warning: bool| {
        assert!(root_offset < code.len() && rhs_offset < code.len());
        let base = code.as_mut_ptr() as usize;
        TARGET.with(|slot| slot.set(function)); BASE.with(|slot| slot.set(base)); SNAPSHOT_LEN.with(|slot| slot.set(code.len()));
        SNAPSHOT_BEFORE.with(|slot| slot.set(0)); SNAPSHOT_AFTER.with(|slot| slot.set(0)); SNAPSHOT_AFTER_ERROR.with(|slot| slot.set(0));
        MASK_PROBES.with(|slot| slot.set(0));
        ROOT.with(|slot| slot.set(base + root_offset)); RHS.with(|slot| slot.set(base + rhs_offset));
        // SAFETY: the fixture owns this live TextBuffer and its data fields.
        unsafe { (*buffer).top = INITIAL; (*buffer).pos = INITIAL; ACTUAL_TOP.with(|slot| slot.set((&raw mut (*buffer).top) as usize)); }
        SCRATCH.with(|slot| slot.set(0)); DESTINATION.with(|slot| slot.set(0)); CONTEXT.with(|slot| slot.set(0)); CHILD_RESULT.with(|slot| slot.set(0));
        CHILD_VALUE.with(|slot| slot.set(None)); EVALUATIONS.with(|slot| slot.set(0)); POSTS.with(|slot| slot.set(0)); SINK_ZERO.with(|slot| slot.set(false));
        FAIL_POST.with(|slot| slot.set(fail_post)); FAIL_WARNING.with(|slot| slot.set(fail_warning)); SINK_CASE.with(|slot| slot.set(sink));
        EXIT_SEEN.with(|slot| slot.set(false)); EXIT_PC.with(|slot| slot.set(None)); LOGS.with(|slot| slot.borrow_mut().clear()); CONTROLS.with(|slot| slot.borrow_mut().clear());
        let binding = if replaced { Some(unsafe { access.install_script_body_replacement("assignment-value-fixture", &[function], replacement).unwrap() }) } else { None };
        // SAFETY: the fixture records, script and object remain live for the call.
        let result = unsafe { access.execute_event(object, function, ptr::null_mut(), ptr::null_mut()) };
        let error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec()));
        drop(binding); access.reset_script_runaway();
        Outcome { error, top: unsafe { (*buffer).top }, pos: unsafe { (*buffer).pos }, logs: LOGS.with(|slot| slot.borrow().clone()), posts: POSTS.with(Cell::get), child_value: CHILD_VALUE.with(Cell::get), child_is_top: CHILD_RESULT.with(Cell::get) != 0 && CHILD_RESULT.with(Cell::get) == ACTUAL_TOP.with(Cell::get), exit_seen: EXIT_SEEN.with(Cell::get), exit_pc: EXIT_PC.with(Cell::get) }
    };
    let stock = run(normal, &mut normal_code, false, 0, normal_rhs, false, false, false);
    assert_eq!((stock.top, stock.pos, stock.posts, stock.child_value), (VALUE, INITIAL, 1, Some(VALUE))); assert!(stock.child_is_top); assert!(stock.error.is_none());
    let actual = run(normal, &mut normal_code, true, 0, normal_rhs, false, false, false);
    assert_eq!(actual, stock, "canonical selected RHS uses the actual LHS and preserves stock behavior");
    assert_eq!(EVALUATIONS.with(Cell::get), 1); assert_eq!(DESTINATION.with(Cell::get), ACTUAL_TOP.with(Cell::get));
    assert_eq!((SNAPSHOT_BEFORE.with(Cell::get), SNAPSHOT_AFTER.with(Cell::get)), (1, 1));
    assert_eq!(MASK_PROBES.with(Cell::get), 1, "the real masked RHS probed this current scalar once");
    assert_eq!(CHILD_RESULT.with(Cell::get), DESTINATION.with(Cell::get)); assert_ne!(SCRATCH.with(Cell::get), DESTINATION.with(Cell::get));
    assert_eq!(CONTEXT.with(Cell::get), object as usize);

    let stock = run(null, &mut null_code, false, 0, null_rhs, true, false, false);
    assert_eq!((stock.top, stock.pos, stock.posts, stock.child_value), (INITIAL, INITIAL, 1, Some(VALUE))); assert!(!stock.child_is_top); assert_eq!(stock.logs.len(), 2); assert!(stock.error.is_none());
    let actual = run(null, &mut null_code, true, 0, null_rhs, true, false, false);
    assert_eq!(actual, stock, "null LHS preserves both warnings and canonical sink effects");
    assert_eq!(EVALUATIONS.with(Cell::get), 1); assert!(SINK_ZERO.with(Cell::get)); assert_eq!(DESTINATION.with(Cell::get), CHILD_RESULT.with(Cell::get));
    assert_ne!(DESTINATION.with(Cell::get), ACTUAL_TOP.with(Cell::get)); assert_ne!(DESTINATION.with(Cell::get), SCRATCH.with(Cell::get));

    let stock = run(normal, &mut normal_code, false, 0, normal_rhs, false, false, false);
    let actual = run(normal, &mut normal_code, true, 0, normal_return + 2, false, false, false);
    assert_eq!(actual, stock, "mismatched RHS uses the live canonical expression exactly once"); assert_eq!(EVALUATIONS.with(Cell::get), 0);
    let actual = run(normal, &mut normal_code, true, normal_return, normal_rhs, false, false, false);
    assert_eq!(actual, stock, "mismatched root dispatches the consumed real Let exactly once"); assert_eq!(EVALUATIONS.with(Cell::get), 0);
    let stock = run(stop, &mut stop_code, false, 1, 1, false, false, false);
    let actual = run(stop, &mut stop_code, true, 1, 1, false, false, false);
    assert_eq!(actual, stock); assert!(actual.exit_seen); assert_eq!(actual.exit_pc, None); assert_eq!(CONTROLS.with(|slot| slot.borrow().clone()), vec![StepControl::Return]);

    let stock = run(normal, &mut normal_code, false, 0, normal_rhs, false, true, false);
    assert!(stock.error.is_some()); assert_eq!((stock.top, stock.pos, stock.posts), (VALUE, INITIAL, 1));
    let actual = run(normal, &mut normal_code, true, 0, normal_rhs, false, true, false);
    assert_eq!(actual, stock, "selected RHS propagates the stock post error and partial write");
    assert_eq!((SNAPSHOT_BEFORE.with(Cell::get), SNAPSHOT_AFTER.with(Cell::get), SNAPSHOT_AFTER_ERROR.with(Cell::get)), (1, 1, 1));
    let contexts = &actual.error.as_ref().unwrap().1;
    let strings: Vec<String> = contexts.iter().map(|c| c.iter().filter_map(|&u| char::from_u32(u)).collect()).collect();
    assert_eq!(strings.iter().filter(|c| c.contains("intUObjectexecLet") && c.contains("(15)")).count(), 1);
    assert_eq!(strings.iter().filter(|c| c.as_str() == "stepping Core VM bytecode").count(), 1);

    let stock = run(null, &mut null_code, false, 0, null_rhs, true, false, true);
    assert!(stock.error.is_some()); assert_eq!((stock.top, stock.pos, stock.posts), (INITIAL, INITIAL, 0)); assert_eq!(stock.logs.len(), 2);
    let actual = run(null, &mut null_code, true, 0, null_rhs, true, false, true);
    assert_eq!(actual, stock, "warning failure stops before RHS or sink use"); assert_eq!(EVALUATIONS.with(Cell::get), 0);
    assert_eq!(null_return, null_rhs + 6);

    for (index, code) in style_codes.iter_mut().enumerate() {
        let blue = matches!(index, 3..=5 | 8 | 11);
        STYLE.with(|slot| slot.set(Some(if blue { lesson_style::lesson_blue_rhs } else { lesson_style::lesson_yellow_rhs })));
        let sink = index >= 10;
        let stock = run(style_functions[index], code, false, 0, style_rhs[index], sink, false, false);
        let actual = run(style_functions[index], code, true, 0, style_rhs[index], sink, false, false);
        assert_eq!(actual, stock, "actual Rust RGB policy, immutable program {index}");
        let value = match index { 0 | 4 | 8 | 10 => 255, 1 | 3 | 9 | 11 => 0, 2 | 5 => 128, 6 => VALUE as u8, 7 => 111, _ => unreachable!() };
        let mut bytes = INITIAL.to_ne_bytes();
        if !sink { bytes[0] = value; }
        assert_eq!(actual.top, i32::from_ne_bytes(bytes), "only the selected byte changes in program {index}");
        assert_eq!(EVALUATIONS.with(Cell::get), 1);
        assert_eq!((SNAPSHOT_BEFORE.with(Cell::get), SNAPSHOT_AFTER.with(Cell::get), SNAPSHOT_AFTER_ERROR.with(Cell::get)), (1, 1, 0));
        if sink { assert!(SINK_ZERO.with(Cell::get)); assert_eq!(actual.logs.len(), 2); }
    }
    // Child post errors occur after its i32 temporary write, but before the
    // enclosing conversion can publish a byte. Preserve native62/15/step order.
    STYLE.with(|slot| slot.set(Some(lesson_style::lesson_yellow_rhs)));
    let stock = run(style_functions[6], &mut style_codes[6], false, 0, style_rhs[6], false, true, false);
    let actual = run(style_functions[6], &mut style_codes[6], true, 0, style_rhs[6], false, true, false);
    assert_eq!(actual, stock, "actual Rust conversion keeps the first child error");
    assert_eq!((SNAPSHOT_BEFORE.with(Cell::get), SNAPSHOT_AFTER.with(Cell::get), SNAPSHOT_AFTER_ERROR.with(Cell::get)), (1, 1, 1));
    assert!(actual.error.is_some()); assert_eq!(actual.top, INITIAL);
    let contexts: Vec<String> = actual.error.as_ref().unwrap().1.iter().map(|c| c.iter().filter_map(|&u| char::from_u32(u)).collect()).collect();
    assert_eq!(contexts.iter().filter(|c| c.contains("intUObjectexecIntToByte") && c.contains("(62)")).count(), 1);
    assert_eq!(contexts.iter().filter(|c| c.contains("intUObjectexecLet") && c.contains("(15)")).count(), 1);
    STYLE.with(|slot| slot.set(None));
    // SAFETY: an absent scope must reject this opaque nonmatching address
    // without reading the frame, code or destination.
    let rejected = unsafe { access.execute_script_let_with_rhs(1_usize as *mut FFrame, ptr::null_mut(), ptr::null(), ptr::null(), evaluate) }.unwrap_err();
    assert_eq!(rejected.diagnostic(), "No matching active Core replacement frame".chars().map(u32::from).collect::<Vec<_>>());

    drop(post_guard); access.reset_script_runaway();
    // Observer and all fixture identities are cleared before scripts detach.
    drop(cleanup);
    drop(detach);
    owner.shutdown().unwrap();
}
