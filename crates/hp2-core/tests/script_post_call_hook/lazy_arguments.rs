//! Stock and scoped-lazy argument evaluation through real interpreted calls.
//!
//! This fixture deliberately does not collect garbage or instrument allocation.
use super::*;
use hp2_abi::engine::generated::{FFrame, UObjectProperty, UProperty, UTextBuffer};
use hp2_core::{
    core_access::{RuntimeError, VmObservation},
    vm_owner::StepControl,
    ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision,
};
use std::ffi::c_void;

const INITIAL_Y: f32 = 3.0;
const MUTATED_Y: f32 = 41.0;
const PROBE_X: f32 = 7.0;
const INITIAL_TOP: i32 = -31;
const INNER_VALUE: f32 = 13.5;

thread_local! {
    static PARENT: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static PROBE: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static INNER: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static INNER_TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static X_PROPERTY: Cell<*mut UProperty> = const { Cell::new(ptr::null_mut()) };
    static Y_PROPERTY: Cell<*mut UProperty> = const { Cell::new(ptr::null_mut()) };
    static INNER_PROPERTY: Cell<*mut UProperty> = const { Cell::new(ptr::null_mut()) };
    // Immutable fixture buffer identities, set before either event/mask.
    static OUTER_SNAPSHOT: Cell<(*const u8, usize)> = const { Cell::new((ptr::null(), 0)) };
    static INNER_SNAPSHOT: Cell<(*const u8, usize)> = const { Cell::new((ptr::null(), 0)) };
    static PROBE_SNAPSHOT: Cell<(*const u8, usize)> = const { Cell::new((ptr::null(), 0)) };
    static INNER_EVALUATIONS: Cell<u32> = const { Cell::new(0) };
    static INNER_ARGUMENT: Cell<Option<u32>> = const { Cell::new(None) };
    static INNER_TARGET_EXITS: Cell<u32> = const { Cell::new(0) };
    static PROBE_RESTORED: Cell<Option<bool>> = const { Cell::new(None) };
    static FAIL_INNER: Cell<bool> = const { Cell::new(false) };
    static SNAPSHOT_AFTER_ERROR: Cell<u32> = const { Cell::new(0) };
    static FAIL_PROBE: Cell<bool> = const { Cell::new(false) };
    static OUTER_CONTEXT: Cell<usize> = const { Cell::new(0) };
    static OUTER_FINAL: Cell<usize> = const { Cell::new(0) };
    static OUTER_END: Cell<usize> = const { Cell::new(0) };
    static INNER_FINAL: Cell<usize> = const { Cell::new(0) };
    static INNER_END: Cell<usize> = const { Cell::new(0) };
    static MISMATCH: Cell<bool> = const { Cell::new(false) };
    static FAIL_POST: Cell<bool> = const { Cell::new(false) };
    static EVALUATIONS: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
    static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static PARAMS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    static POST_PC: Cell<usize> = const { Cell::new(usize::MAX) };
    static PROBE_RESULT: Cell<usize> = const { Cell::new(0) };
    static PROBE_WRITES: Cell<Option<(u32, u32)>> = const { Cell::new(None) };
    static TARGET_RESULT: Cell<usize> = const { Cell::new(0) };
    static CANDIDATE_SCRATCH: Cell<usize> = const { Cell::new(0) };
    static SCRATCH_MATCH: Cell<bool> = const { Cell::new(false) };
    static EXIT_MATCH: Cell<bool> = const { Cell::new(false) };
    static REPLACEMENTS: Cell<u32> = const { Cell::new(0) };
    static INNER_REPLACEMENTS: Cell<u32> = const { Cell::new(0) };
}

fn event(value: &'static str) { EVENTS.with(|events| events.borrow_mut().push(value)); }

fn output_lazy(_: CoreAccess<'_>, _: i32, units: &[u32]) -> RuntimeResult<()> {
    let text: String = units.iter().filter_map(|&unit| char::from_u32(unit)).collect();
    if text.contains("Accessed None") { WARNINGS.with(|warnings| warnings.borrow_mut().push(text)); }
    Ok(())
}

fn observe(_: CoreAccess<'_>, observation: VmObservation) -> RuntimeResult<()> {
    match observation {
        VmObservation::FunctionEnter { function, frame, .. } if function == TARGET.with(Cell::get) => {
            // SAFETY: the target's live frame has the still-active parent frame.
            let parent = unsafe { (*frame).parent };
            assert!(!parent.is_null());
            assert_eq!(unsafe { (*parent).node }, PARENT.with(Cell::get).cast());
            let pc = unsafe { ((*parent).code as usize).wrapping_sub((*(*parent).node.cast::<UStruct>()).script.data as usize) };
            POST_PC.with(|slot| slot.set(pc));
        }
        VmObservation::FunctionExit { function, result, .. } if function == PROBE.with(Cell::get) => {
            assert!(!result.is_null(), "ProbeX has a real float result destination");
            PROBE_RESULT.with(|slot| slot.set(result as usize));
        }
        VmObservation::FunctionExit { function, result, .. } if function == TARGET.with(Cell::get) => {
            assert!(!result.is_null(), "the void statement still has a VM scratch result");
            TARGET_RESULT.with(|slot| slot.set(result as usize));
            let scratch = CANDIDATE_SCRATCH.with(Cell::get);
            if scratch != 0 { SCRATCH_MATCH.with(|slot| slot.set(result as usize == scratch)); }
        }
        VmObservation::ScriptCallExit { function, .. } if function == INNER_TARGET.with(Cell::get) => {
            INNER_TARGET_EXITS.with(|count| count.set(count.get() + 1));
        }
        VmObservation::ScriptCallExit { function, result, .. } if function == TARGET.with(Cell::get) => {
            EXIT_MATCH.with(|slot| slot.set(result as usize == TARGET_RESULT.with(Cell::get) && !result.is_null()));
        }
        _ => {}
    }
    Ok(())
}

/// # Safety
/// The post hook receives the live canonical Float2 parameter storage.
unsafe fn target_post(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    assert_eq!(function, TARGET.with(Cell::get));
    assert_eq!(size, 8);
    assert!(!params.is_null());
    // SAFETY: the reflected target has two four-byte floats at offsets 0 and 4.
    let values = unsafe { (params.cast::<f32>().read_unaligned().to_bits(), params.add(4).cast::<f32>().read_unaligned().to_bits()) };
    PARAMS.with(|all| all.borrow_mut().push(values));
    event("target-post");
    if FAIL_POST.with(Cell::get) { return Err(RuntimeError::new("lazy arguments post failure")); }
    Ok(())
}

/// # Safety
/// Core calls this only with the borrowed, currently active argument executor.
unsafe fn evaluate(access: CoreAccess<'_>, args: &ScriptCallArguments, frame: *mut FFrame, property: *mut UProperty, result: *mut c_void) -> RuntimeResult<()> {
    let inner = property == INNER_PROPERTY.with(Cell::get);
    let expected = if inner { INNER_SNAPSHOT.with(Cell::get) } else { OUTER_SNAPSHOT.with(Cell::get) };
    let function = if inner { INNER.with(Cell::get) } else { PARENT.with(Cell::get) };
    assert!(!expected.0.is_null() && expected.1 != 0, "fixture captured a real immutable body snapshot");
    // SAFETY: the argument executor borrows this same live current frame.
    let object = unsafe { (*frame).object.cast::<UObject>() };
    assert!(!access.current_script_body_matches(frame, object, function, expected.0, expected.1), "the current body scope is masked during argument evaluation");
    assert_eq!(args.body_script_snapshot(), expected, "active masked callback gets its original body identity");
    assert!(!result.is_null());
    let index = if inner {
        INNER_EVALUATIONS.with(|count| count.set(count.get() + 1));
        None
    } else {
        let index = if property == X_PROPERTY.with(Cell::get) { 0 } else {
            assert_eq!(property, Y_PROPERTY.with(Cell::get), "only the two reflected Float2 arguments are selected");
            1
        };
        // SAFETY: the callback borrows the current active frame. Compute the
        // metric against the pre-captured original base, not a live header.
        let pc = unsafe { ((*frame).code as usize).wrapping_sub(expected.0 as usize) };
        EVALUATIONS.with(|values| values.borrow_mut().push((index, pc)));
        Some(index)
    };
    // SAFETY: step exactly the current complete float expression into the
    // actual reflected destination. The capability remains lexical here.
    let stepped = unsafe { args.step_argument(frame, result) };
    assert_eq!(args.body_script_snapshot(), expected, "snapshot survives step, nested reentry, or error");
    if stepped.is_err() { SNAPSHOT_AFTER_ERROR.with(|count| count.set(count.get() + 1)); }
    stepped?;
    if index == Some(0) {
        assert_eq!(result as usize, PROBE_RESULT.with(Cell::get), "ProbeX writes the exact first-argument destination");
    }
    if inner {
        // SAFETY: INNER_PROPERTY is the real four-byte FloatProperty argument.
        let bits = unsafe { result.cast::<f32>().read_unaligned() }.to_bits();
        assert_eq!(bits, INNER_VALUE.to_bits());
        INNER_ARGUMENT.with(|slot| slot.set(Some(bits)));
    }
    Ok(())
}

/// # Safety
/// The records and bytecode remain live for this synchronous body replacement.
unsafe fn lazy_body(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let inner = function == INNER.with(Cell::get);
    if inner { INNER_REPLACEMENTS.with(|count| count.set(count.get() + 1)); }
    else { assert_eq!(function, PARENT.with(Cell::get)); REPLACEMENTS.with(|count| count.set(count.get() + 1)); }
    let mut statements = ScriptBodyStatements::new();
    // SAFETY: Core supplies this live frame and its script belongs to the fixture.
    let script = unsafe { (*function.cast::<UStruct>()).script };
    let expected_snapshot = if inner { INNER_SNAPSHOT.with(Cell::get) } else { OUTER_SNAPSHOT.with(Cell::get) };
    assert!(access.current_script_body_matches(frame, object, function, expected_snapshot.0, expected_snapshot.1), "expected fixture snapshot matches the active unmasked owner");
    if !inner {
        // The first two original statements initialize the real caller locals.
        for _ in 0..2 {
            // SAFETY: both current positions are complete original statements.
            unsafe { statements.before(access, frame)?; access.execute_script_frame_step(frame, statements.result())?; }
        }
        assert_eq!(unsafe { (*frame).code as usize - script.data as usize }, OUTER_CONTEXT.with(Cell::get));
    }
    // SAFETY: the cursor is at the original Context statement in this frame.
    unsafe { statements.before(access, frame)?; }
    let final_offset = if inner { INNER_FINAL.with(Cell::get) } else { OUTER_FINAL.with(Cell::get) };
    let end_offset = if inner { INNER_END.with(Cell::get) } else { OUTER_END.with(Cell::get) };
    let expected = if inner || (!inner && MISMATCH.with(Cell::get)) { INNER_TARGET.with(Cell::get) } else { TARGET.with(Cell::get) };
    if !inner { CANDIDATE_SCRATCH.with(|slot| slot.set(statements.result() as usize)); }
    // SAFETY: this is the matching active replacement frame, at the original
    // Context; the selected Final opcode and target are retained fixture records.
    let call = unsafe { access.execute_script_context_call_with_arguments(frame, statements.result(), script.data.cast::<u8>().add(final_offset), expected, evaluate) };
    if !inner { CANDIDATE_SCRATCH.with(|slot| slot.set(0)); }
    assert_eq!(call?, StepControl::Continue);
    // The successful Context must leave the cursor at its original next statement.
    assert_eq!(unsafe { (*frame).code as usize - script.data as usize }, end_offset);
    if inner && FAIL_INNER.with(Cell::get) {
        // This occurs after the selected body completed the actual inner call,
        // including the real FloatConst argument and target body.
        event("inner-after-call");
        return Err(RuntimeError::new("lazy inner selected body failure"));
    }
    // Return is a top-level statement; Nothing is its nested return expression.
    // Keep both actual cursor updates and the returned control, without forcing
    // the cursor to an expected end or adding a nested statement guard.
    unsafe {
        statements.before(access, frame)?;
        assert_eq!(access.execute_script_context_call_with_arguments(frame, statements.result(), script.data.cast::<u8>().add(final_offset), expected, evaluate)?, StepControl::Return);
        assert_eq!((*frame).code as usize - script.data as usize, end_offset + 1);
        assert_eq!(access.execute_script_context_call_with_arguments(frame, ptr::null_mut(), script.data.cast::<u8>().add(final_offset), expected, evaluate)?, StepControl::Continue);
        assert_eq!((*frame).code as usize - script.data as usize, script.num as usize);
    }
    Ok(ScriptPreCallDecision::Consume)
}

/// # Safety
/// Core calls this on the live ProbeX frame with its reflected float destination.
unsafe fn probe_body(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<ScriptPreCallDecision> {
    assert!(!result.is_null());
    // SAFETY: ProbeX is a nested Final call in the still-active caller frame.
    let parent = unsafe { (*frame).parent };
    assert!(!parent.is_null());
    assert_eq!(unsafe { (*parent).node }, PARENT.with(Cell::get).cast());
    assert_eq!(unsafe { (*parent).object }, object.cast());
    event("probe-enter");
    // Verify the Probe's own pre-captured original identity, not the suspended
    // outer frame. This scope must return on both nested Ok and nested Err.
    let probe_snapshot = PROBE_SNAPSHOT.with(Cell::get);
    assert_eq!(function, PROBE.with(Cell::get));
    assert!(access.current_script_body_matches(frame, object, function, probe_snapshot.0, probe_snapshot.1), "Probe scope matches before nested entry");
    let nested = unsafe { access.execute_script_callback(frame, object, INNER.with(Cell::get), ptr::null_mut(), ptr::null_mut()) };
    let restored = access.current_script_body_matches(frame, object, function, probe_snapshot.0, probe_snapshot.1);
    PROBE_RESTORED.with(|slot| slot.set(Some(restored)));
    assert!(restored, "Probe scope must be restored before propagating nested Ok or Err");
    // Do not inspect the unwritten Probe result or reacquire outer scope.
    nested?;
    event("probe-after-inner");
    // SAFETY: the parent has a reflected four-byte FloatProperty local at 0;
    // both frames and the float result remain live during synchronous reentry.
    unsafe { (*parent).locals.cast::<f32>().write_unaligned(MUTATED_Y); result.cast::<f32>().write_unaligned(PROBE_X); }
    // Observe only copied scalar bits; no live frame or destination escapes.
    PROBE_WRITES.with(|slot| slot.set(Some(unsafe { ((*parent).locals.cast::<f32>().read_unaligned().to_bits(), result.cast::<f32>().read_unaligned().to_bits()) })));
    event("probe-mutated-y");
    if FAIL_PROBE.with(Cell::get) { return Err(RuntimeError::new("lazy probe injected failure")); }
    Ok(ScriptPreCallDecision::Consume)
}

/// # Safety
/// `outer` is a live fixture record owned by `access`.
unsafe fn property(access: CoreAccess<'_>, outer: *mut UObject, class: &str, name: &str, offset: usize, size: usize, flags: u32) -> *mut UProperty {
    let value = unsafe { access.construct_object(access.native_class(class).unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    unsafe { (*value).array_dim = 1; (*value).element_size = size as i32; (*value).offset = offset as i32; (*value).property_flags = flags; }
    value
}

fn operand(code: &mut Vec<u8>, opcode: u8, record: *mut UObject) {
    code.push(opcode);
    // SAFETY: fixture-owned indexed records are retained for the entire test.
    code.extend(unsafe { (*record).index }.to_le_bytes());
}

fn context_prefix(code: &mut Vec<u8>) -> usize {
    code.push(0x19); // EX_Context; caller emits its object expression.
    code.len()
}

// Detach borrowed script buffers even if a later assertion unwinds.
struct DetachScripts([*mut UFunction; 5]);
impl Drop for DetachScripts {
    fn drop(&mut self) {
        // SAFETY: each function belongs to the still-live owner, and the
        // fixture releases its callback guards before the owner is shut down.
        unsafe { for &function in &self.0 { if !function.is_null() { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } } }
    }
}

// Declared after DetachScripts and before observer installation or any
// binding. On unwind, later binding guards drop before this cleanup; detach
// and backing buffers then drop while the Core owner remains live.
struct Cleanup<'a>(CoreAccess<'a>);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        self.0.clear_vm_observer(observe);
        PARENT.with(|v| v.set(ptr::null_mut())); TARGET.with(|v| v.set(ptr::null_mut())); PROBE.with(|v| v.set(ptr::null_mut()));
        INNER.with(|v| v.set(ptr::null_mut())); INNER_TARGET.with(|v| v.set(ptr::null_mut()));
        X_PROPERTY.with(|v| v.set(ptr::null_mut())); Y_PROPERTY.with(|v| v.set(ptr::null_mut())); INNER_PROPERTY.with(|v| v.set(ptr::null_mut()));
        OUTER_SNAPSHOT.with(|v| v.set((ptr::null(), 0))); INNER_SNAPSHOT.with(|v| v.set((ptr::null(), 0))); PROBE_SNAPSHOT.with(|v| v.set((ptr::null(), 0)));
        OUTER_CONTEXT.with(|v| v.set(0)); OUTER_FINAL.with(|v| v.set(0)); OUTER_END.with(|v| v.set(0));
        INNER_FINAL.with(|v| v.set(0)); INNER_END.with(|v| v.set(0));
        PROBE_RESULT.with(|v| v.set(0)); TARGET_RESULT.with(|v| v.set(0)); CANDIDATE_SCRATCH.with(|v| v.set(0)); POST_PC.with(|v| v.set(usize::MAX));
        MISMATCH.with(|v| v.set(false)); FAIL_POST.with(|v| v.set(false)); FAIL_PROBE.with(|v| v.set(false)); FAIL_INNER.with(|v| v.set(false));
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    error: Option<Vec<u32>>,
    error_context: Vec<Vec<u32>>,
    warnings: Vec<String>,
    events: Vec<&'static str>,
    params: Vec<(u32, u32)>,
    inner_argument: Option<u32>,
    inner_target_exits: u32,
    probe_restored: Option<bool>,
    probe_writes: Option<(u32, u32)>,
    top: i32,
    post_pc: usize,
    exit_match: bool,
}

#[test]
fn stock_and_scoped_lazy_arguments_preserve_reentry_context_and_failure() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"));
    let root = artifact.join(format!("lazy-arguments-{}", std::process::id()));
    assert!(!root.exists());
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] { fs::copy(source.join(from), root.join("System").join(to)).unwrap(); }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output: output_lazy, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>();
    let top = unsafe { property(access, class.cast(), "IntProperty", "LazyArgumentsTop", std::mem::offset_of!(UTextBuffer, top), 4, 0) };

    // Every code buffer precedes its detach guard. Register each borrowed
    // script immediately, including setup-error/unwind paths before all records
    // have been constructed. No collection occurs in this fixture.
    let mut target_code = vec![0x0f]; // Let instance top = 77.
    let mut inner_target_code = vec![0x04, 0x0b];
    let mut inner_code = Vec::new();
    let mut probe_code = vec![0x04, 0x1e];
    let mut parent_code = Vec::new();
    let mut detach = DetachScripts([ptr::null_mut(); 5]);
    let cleanup = Cleanup(access);
    operand(&mut target_code, 0x01, top.cast());
    target_code.extend([0x2c, 77, 0x04, 0x0b]);
    let target = unsafe { function(access, class, "LazyFloat2", &mut target_code) };
    detach.0[1] = target;
    let x = unsafe { property(access, target.cast(), "FloatProperty", "LazyX", 0, 4, 0x80) };
    let y = unsafe { property(access, target.cast(), "FloatProperty", "LazyY", 4, 4, 0x80) };
    // SAFETY: these are fixture-owned reflected parameter records in child order.
    unsafe {
        (*x.cast::<UField>()).next = y.cast();
        (*target.cast::<UStruct>()).children = x.cast();
        (*target.cast::<UStruct>()).properties_size = 8;
        (*target).num_parms = 2;
        (*target).parms_size = 8;
    }
    let inner_target = unsafe { function(access, class, "LazyInnerTarget", &mut inner_target_code) };
    detach.0[4] = inner_target;
    let inner_input = unsafe { property(access, inner_target.cast(), "FloatProperty", "LazyInnerInput", 0, 4, 0x80) };
    // SAFETY: one unmanaged float input spans the complete aligned parameter area.
    unsafe {
        (*inner_target.cast::<UStruct>()).children = inner_input.cast();
        (*inner_target.cast::<UStruct>()).properties_size = 4;
        (*inner_target.cast::<UStruct>()).properties_align = 4;
        (*inner_target).num_parms = 1; (*inner_target).parms_size = 4;
    }
    context_prefix(&mut inner_code);
    inner_code.push(0x17); // Self.
    let inner_skip = inner_code.len();
    inner_code.extend([0, 0, 0]); // skip and void result width.
    let inner_final = inner_code.len();
    operand(&mut inner_code, 0x1c, inner_target.cast());
    inner_code.push(0x1e); // complete FloatConst input to the real child parameter.
    inner_code.extend(INNER_VALUE.to_le_bytes());
    inner_code.push(0x16);
    let inner_end = inner_code.len();
    inner_code[inner_skip..inner_skip + 2].copy_from_slice(&(u16::try_from(inner_end - inner_final).unwrap()).to_le_bytes());
    inner_code.extend([0x04, 0x0b]);
    let inner = unsafe { function(access, class, "LazyInner", &mut inner_code) };
    detach.0[3] = inner;

    // Return FloatConst; replacement supplies the same layout.
    probe_code.extend(PROBE_X.to_le_bytes());
    let probe = unsafe { function(access, class, "LazyProbeX", &mut probe_code) };
    detach.0[2] = probe;
    let probe_return = unsafe { property(access, probe.cast(), "FloatProperty", "ReturnValue", 0, 4, 0x480) };
    unsafe {
        (*probe.cast::<UStruct>()).children = probe_return.cast();
        (*probe.cast::<UStruct>()).properties_size = 4;
        (*probe).num_parms = 1;
        (*probe).parms_size = 4;
        (*probe).return_value_offset = 0;
    }
    // Construct the parent first so its local properties have the correct outer.
    let parent = unsafe { function(access, class, "LazyParent", &mut parent_code) };
    detach.0[0] = parent;
    let local_y = unsafe { property(access, parent.cast(), "FloatProperty", "LazyLaterY", 0, 4, 0) };
    let local_object = unsafe { property(access, parent.cast(), "ObjectProperty", "LazyContextObject", 8, std::mem::size_of::<*mut UObject>(), 0) };
    unsafe {
        (*local_object.cast::<UObjectProperty>()).property_class = class.cast();
        (*local_y.cast::<UField>()).next = local_object.cast();
        (*parent.cast::<UStruct>()).children = local_y.cast();
        (*parent.cast::<UStruct>()).properties_size = 16;
        // This synthetic frame stores an aligned native-width object pointer.
        // Request its alignment explicitly; this is not retail layout evidence.
        (*parent.cast::<UStruct>()).properties_align = std::mem::align_of::<*mut UObject>() as i32;
    }
    parent_code.push(0x0f); // Let local Y = initial float.
    operand(&mut parent_code, 0x00, local_y.cast());
    parent_code.push(0x1e);
    parent_code.extend(INITIAL_Y.to_le_bytes());
    parent_code.push(0x0f); // Let local context object = Self or None.
    operand(&mut parent_code, 0x00, local_object.cast());
    let selector = parent_code.len();
    parent_code.push(0x17);
    let outer_context = parent_code.len();
    context_prefix(&mut parent_code);
    operand(&mut parent_code, 0x00, local_object.cast());
    let outer_skip = parent_code.len();
    parent_code.extend([0, 0, 0]);
    let outer_final = parent_code.len();
    operand(&mut parent_code, 0x1c, target.cast());
    operand(&mut parent_code, 0x1c, probe.cast()); // First argument: ProbeX().
    parent_code.push(0x16);
    operand(&mut parent_code, 0x00, local_y.cast()); // Later argument: caller's Y.
    parent_code.push(0x16);
    let outer_end = parent_code.len();
    parent_code[outer_skip..outer_skip + 2].copy_from_slice(&(u16::try_from(outer_end - outer_final).unwrap()).to_le_bytes());
    parent_code.extend([0x04, 0x0b]);
    // SAFETY: all bytecode Vecs stay live and unresized through calls; the
    // initial empty parent script is replaced only after construction ends.
    let parent_script = parent_code.as_mut_ptr();
    unsafe { (*parent.cast::<UStruct>()).script = TArrayRaw { data: parent_script.cast(), num: parent_code.len() as i32, max: parent_code.len() as i32 }; }

    // A missing scope rejects before any frame/cursor/result access. This is an
    // explicitly supported negative call, not a suspended matching invocation.
    let mut outside_locals = [0_u64; 2];
    let mut outside = FFrame { node: parent.cast(), object: object.cast(), code: unsafe { parent_script.add(outer_context) }, locals: outside_locals.as_mut_ptr().cast(), parent: ptr::null_mut() };
    let initial_code = outside.code;
    let mut outside_scratch = ScriptBodyStatements::new();
    let rejected = unsafe { access.execute_script_context_call_with_arguments(&raw mut outside, outside_scratch.result(), parent_script.add(outer_final), target, evaluate) };
    assert!(rejected.is_err(), "there is no active replacement scope outside the call");
    assert_eq!(outside.code, initial_code);
    assert!(EVALUATIONS.with(|values| values.borrow().is_empty()));

    PARENT.with(|slot| slot.set(parent)); TARGET.with(|slot| slot.set(target)); PROBE.with(|slot| slot.set(probe));
    INNER.with(|slot| slot.set(inner)); INNER_TARGET.with(|slot| slot.set(inner_target));
    X_PROPERTY.with(|slot| slot.set(x)); Y_PROPERTY.with(|slot| slot.set(y)); INNER_PROPERTY.with(|slot| slot.set(inner_input));
    let outer_snapshot = (parent_code.as_ptr(), parent_code.len());
    let inner_snapshot = (inner_code.as_ptr(), inner_code.len());
    let probe_snapshot = (probe_code.as_ptr(), probe_code.len());
    assert!(!outer_snapshot.0.is_null() && !inner_snapshot.0.is_null() && !probe_snapshot.0.is_null());
    assert_ne!(outer_snapshot.0, inner_snapshot.0, "nested body has a distinct live script allocation");
    OUTER_SNAPSHOT.with(|slot| slot.set(outer_snapshot)); INNER_SNAPSHOT.with(|slot| slot.set(inner_snapshot)); PROBE_SNAPSHOT.with(|slot| slot.set(probe_snapshot));
    OUTER_CONTEXT.with(|slot| slot.set(outer_context)); OUTER_FINAL.with(|slot| slot.set(outer_final)); OUTER_END.with(|slot| slot.set(outer_end));
    INNER_FINAL.with(|slot| slot.set(inner_final)); INNER_END.with(|slot| slot.set(inner_end));
    access.set_vm_observer(Some(observe));
    let post = unsafe { access.install_script_post_call_hook([target, target], target_post).unwrap() };
    let probe_binding = unsafe { access.install_script_result_body_replacement("lazy-probe-x", &[probe], probe_body).unwrap() };
    let inner_binding = unsafe { access.install_script_body_replacement("lazy-inner", &[inner], lazy_body).unwrap() };

    // No Vec is resized after this point. Only the fixed-size Self/None opcode
    // changes between independent, fully completed calls.
    let run = |none: bool, fail: bool, fail_probe: bool, fail_inner: bool| -> Outcome {
        unsafe { parent_script.add(selector).write(if none { 0x2a } else { 0x17 }); (*buffer).top = INITIAL_TOP; }
        FAIL_POST.with(|slot| slot.set(fail)); FAIL_PROBE.with(|slot| slot.set(fail_probe)); FAIL_INNER.with(|slot| slot.set(fail_inner));
        INNER_EVALUATIONS.with(|slot| slot.set(0)); SNAPSHOT_AFTER_ERROR.with(|slot| slot.set(0));
        INNER_ARGUMENT.with(|slot| slot.set(None)); INNER_TARGET_EXITS.with(|slot| slot.set(0));
        PROBE_RESTORED.with(|slot| slot.set(None));
        EVALUATIONS.with(|values| values.borrow_mut().clear()); EVENTS.with(|values| values.borrow_mut().clear());
        WARNINGS.with(|values| values.borrow_mut().clear()); PARAMS.with(|values| values.borrow_mut().clear());
        PROBE_RESULT.with(|slot| slot.set(0)); PROBE_WRITES.with(|slot| slot.set(None));
        TARGET_RESULT.with(|slot| slot.set(0)); POST_PC.with(|slot| slot.set(usize::MAX));
        SCRATCH_MATCH.with(|slot| slot.set(false)); EXIT_MATCH.with(|slot| slot.set(false));
        REPLACEMENTS.with(|slot| slot.set(0)); INNER_REPLACEMENTS.with(|slot| slot.set(0));
        let result = unsafe { access.execute_event(object, parent, ptr::null_mut(), ptr::null_mut()) };
        let (error, error_context) = match result { Ok(()) => (None, Vec::new()), Err(error) => (Some(error.diagnostic().to_vec()), error.context().to_vec()) };
        Outcome { error, error_context, warnings: WARNINGS.with(|values| values.borrow().clone()), events: EVENTS.with(|values| values.borrow().clone()), params: PARAMS.with(|values| values.borrow().clone()), inner_argument: INNER_ARGUMENT.with(Cell::get), inner_target_exits: INNER_TARGET_EXITS.with(Cell::get), probe_restored: PROBE_RESTORED.with(Cell::get), probe_writes: PROBE_WRITES.with(Cell::get), top: unsafe { (*buffer).top }, post_pc: POST_PC.with(Cell::get), exit_match: EXIT_MATCH.with(Cell::get) }
    };
    // "stock" is the OUTER canonical baseline only: the inner and Probe
    // replacements are installed in both baseline and candidate. It is not
    // a wholly stock nested-execution proof.
    let stock = run(false, false, false, false);
    assert_eq!(stock.probe_restored, Some(true));
    assert_eq!(stock.inner_argument, Some(INNER_VALUE.to_bits()));
    assert_eq!(stock.inner_target_exits, 1);
    assert_eq!(stock.params, [(PROBE_X.to_bits(), MUTATED_Y.to_bits())]);
    assert_eq!(stock.events, ["probe-enter", "probe-after-inner", "probe-mutated-y", "target-post"]);
    assert_eq!(stock.top, 77);
    assert_eq!(stock.post_pc, outer_end);
    assert!(stock.exit_match);
    assert_eq!(INNER_REPLACEMENTS.with(Cell::get), 1);
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1);
    let stock_error = run(false, true, false, false);
    assert_eq!(stock_error.error.as_deref(), Some("lazy arguments post failure".chars().map(u32::from).collect::<Vec<_>>().as_slice()));
    assert_eq!(stock_error.params, stock.params);
    assert_eq!(stock_error.top, 77, "the target effect remains after the later post error");
    let stock_none = run(true, false, false, false);
    assert_eq!(stock_none.top, INITIAL_TOP);
    assert!(stock_none.params.is_empty());
    assert!(!stock_none.warnings.is_empty(), "the real null Context reports its warning");
    // Independent reset and real nested child first-effects, then injected Err.
    let stock_probe_error = run(false, false, true, false);
    assert_eq!(stock_probe_error.error.as_deref(), Some("lazy probe injected failure".chars().map(u32::from).collect::<Vec<_>>().as_slice()));
    assert_eq!(stock_probe_error.events, ["probe-enter", "probe-after-inner", "probe-mutated-y"]);
    assert!(stock_probe_error.params.is_empty());
    assert_eq!(stock_probe_error.probe_writes, Some((MUTATED_Y.to_bits(), PROBE_X.to_bits())));
    assert_eq!(stock_probe_error.top, INITIAL_TOP);
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1);
    assert_eq!(SNAPSHOT_AFTER_ERROR.with(Cell::get), 0, "stock outer call has no selected argument callback");
    let stock_inner_error = run(false, false, false, true);
    assert_eq!(stock_inner_error.error.as_deref(), Some("lazy inner selected body failure".chars().map(u32::from).collect::<Vec<_>>().as_slice()));
    assert_eq!(stock_inner_error.events, ["probe-enter", "inner-after-call"]);
    assert_eq!(stock_inner_error.inner_argument, Some(INNER_VALUE.to_bits()));
    assert_eq!(stock_inner_error.inner_target_exits, 1, "inner target call completed before selected body failed");
    assert_eq!(stock_inner_error.probe_restored, Some(true), "Probe scope restored on nested Err");
    assert_eq!(stock_inner_error.probe_writes, None, "Probe result and later parent mutation did not occur");
    assert!(stock_inner_error.params.is_empty());
    assert_eq!(stock_inner_error.top, INITIAL_TOP);
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1);
    assert_eq!(SNAPSHOT_AFTER_ERROR.with(Cell::get), 0);

    let outer_binding = unsafe { access.install_script_body_replacement("lazy-parent", &[parent], lazy_body).unwrap() };
    MISMATCH.with(|slot| slot.set(false));
    let candidate = run(false, false, false, false);
    assert_eq!(candidate, stock);
    assert_eq!(REPLACEMENTS.with(Cell::get), 1);
    assert_eq!(INNER_REPLACEMENTS.with(Cell::get), 1);
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1, "nested callback observed its own snapshot");
    let positions = EVALUATIONS.with(|values| values.borrow().clone());
    assert_eq!(positions.len(), 2);
    assert_eq!((positions[0].0, positions[1].0), (0, 1));
    assert!(positions[0].1 < positions[1].1 && positions[1].1 < outer_end);
    assert!(SCRATCH_MATCH.with(Cell::get), "target observes the replacement's same live statement scratch");
    assert_eq!(run(false, true, false, false), stock_error);
    assert_eq!(run(true, false, false, false), stock_none);
    assert!(EVALUATIONS.with(|values| values.borrow().is_empty()), "null Context cannot evaluate arguments");
    let candidate_probe_error = run(false, false, true, false);
    assert_eq!(candidate_probe_error, stock_probe_error, "child error retains diagnostic, context and first effects");
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1);
    assert_eq!(SNAPSHOT_AFTER_ERROR.with(Cell::get), 1, "outer callback checked snapshot after step returned Err");
    assert_eq!(EVALUATIONS.with(|values| values.borrow().len()), 1, "the second outer argument was not stepped after error");
    let candidate_inner_error = run(false, false, false, true);
    assert_eq!(candidate_inner_error, stock_inner_error, "inner selected-body error preserves full diagnostic, context and initialized partial effects");
    assert_eq!(candidate_inner_error.probe_restored, Some(true));
    assert_eq!(INNER_EVALUATIONS.with(Cell::get), 1);
    assert_eq!(SNAPSHOT_AFTER_ERROR.with(Cell::get), 1, "outer getter checked after nested selected-body Err");
    assert_eq!(EVALUATIONS.with(|values| values.borrow().len()), 1, "no later outer argument after inner failure");
    MISMATCH.with(|slot| slot.set(true));
    assert_eq!(run(false, false, false, false), stock, "a different expected target delegates to the normal VM");
    assert!(EVALUATIONS.with(|values| values.borrow().is_empty()), "unmatched target cannot use the evaluator");
    MISMATCH.with(|slot| slot.set(false));
    drop(outer_binding);
    assert_eq!(run(false, false, false, false), stock, "dropping the body restores interpreted execution");
    assert_eq!(REPLACEMENTS.with(Cell::get), 0);
    assert!(EVALUATIONS.with(|values| values.borrow().is_empty()));
    drop(inner_binding); drop(probe_binding); drop(post);
    // Binding guards precede observer/TLS cleanup; scripts are still attached.
    drop(cleanup);
    drop(detach);
    owner.shutdown().unwrap();
}
