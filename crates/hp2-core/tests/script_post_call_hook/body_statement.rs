//! Real-VM stock comparisons for the same-host body statement protocol.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{
    core_access::{RuntimeError, VmObservation},
    vm_owner::StepControl,
    ScriptBodyStatements, ScriptPreCallDecision,
};
use std::ffi::c_void;

const INITIAL: i32 = -31;
const RETURNED: i32 = 123;

thread_local! {
    static TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static FRAME: Cell<*mut FFrame> = const { Cell::new(ptr::null_mut()) };
    static BASE: Cell<*mut u8> = const { Cell::new(ptr::null_mut()) };
    static LENGTH: Cell<usize> = const { Cell::new(0) };
    static DEST: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static SCRATCH: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static FIRST_CHILD_RESULT: Cell<usize> = const { Cell::new(0) };
    static REPLACED: Cell<bool> = const { Cell::new(false) };
    static EXPECT_DEST: Cell<bool> = const { Cell::new(false) };
    static REDIRECT: Cell<Option<usize>> = const { Cell::new(None) };
    static FAIL_LOG: Cell<bool> = const { Cell::new(false) };
    static GAP: Cell<i32> = const { Cell::new(0) };
    static CHILD_VALUES: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static LOG_PCS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static EXIT_PC: Cell<Option<usize>> = const { Cell::new(None) };
    static EXIT_SEEN: Cell<bool> = const { Cell::new(false) };
    static CAPTURED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static CONTROLS: RefCell<Vec<StepControl>> = const { RefCell::new(Vec::new()) };
}

// Read the cursor only while the canonical invocation is active. None is Stop's
// null cursor; an in-range/one-past cursor is represented relative to the script.
unsafe fn current_pc(frame: *mut FFrame) -> Option<usize> {
    let code = unsafe { (*frame).code };
    if code.is_null() { return None; }
    let base = BASE.with(Cell::get);
    let offset = (code as usize).wrapping_sub(base as usize);
    assert!(offset <= LENGTH.with(Cell::get), "fixture cursor leaves its retained script");
    Some(offset)
}

fn output(_: CoreAccess<'_>, _: i32, text: &[u32]) -> RuntimeResult<()> {
    let message: String = text.iter().filter_map(|&unit| char::from_u32(unit)).collect();
    if !message.contains("Runaway loop detected") { return Ok(()); }
    let frame = FRAME.with(Cell::get);
    assert!(!frame.is_null(), "the diagnostic must run inside the target invocation");
    // SAFETY: the observer recorded this active frame, and all targets/offsets
    // below are fixture-owned complete bytecode statements.
    let pc = unsafe { current_pc(frame).expect("guard begins at a non-null instruction") };
    LOG_PCS.with(|pcs| pcs.borrow_mut().push(pc));
    if FAIL_LOG.with(Cell::get) { return Err(RuntimeError::new("body statement guard failure")); }
    if let Some(offset) = REDIRECT.with(Cell::get) {
        assert!(offset < LENGTH.with(Cell::get));
        // SAFETY: the test selects a complete statement in the retained script.
        unsafe { (*frame).code = BASE.with(Cell::get).add(offset); }
    }
    Ok(())
}

fn observe(_: CoreAccess<'_>, event: VmObservation) -> RuntimeResult<()> {
    match event {
        VmObservation::FunctionEnter { function, frame, .. } if function == TARGET.with(Cell::get) => {
            FRAME.with(|value| value.set(frame));
        }
        VmObservation::FunctionExit { function, frame, .. } if function == TARGET.with(Cell::get) => {
            // SAFETY: FunctionExit borrows the current frame synchronously.
            EXIT_PC.with(|slot| slot.set(unsafe { current_pc(frame) }));
            EXIT_SEEN.with(|slot| slot.set(true));
            FRAME.with(|slot| slot.set(ptr::null_mut()));
        }
        VmObservation::FunctionExit { function, result, .. } if function == CHILD.with(Cell::get) => {
            assert!(!result.is_null(), "the i32 child needs a destination");
            if EXPECT_DEST.with(Cell::get) {
                assert_eq!(result, DEST.with(Cell::get), "Return's child receives the actual VOID event result");
            } else {
                FIRST_CHILD_RESULT.with(|first| {
                    if first.get() == 0 { first.set(result as usize); }
                    else { assert_eq!(first.get(), result as usize, "stock retains its statement result storage"); }
                });
                if REPLACED.with(Cell::get) {
                    assert_eq!(result, SCRATCH.with(Cell::get), "child writes the replacement's live scratch");
                }
            }
            // SAFETY: the child has an admitted reflected i32 return slot.
            CHILD_VALUES.with(|values| values.borrow_mut().push(unsafe { result.cast::<i32>().read_unaligned() }));
        }
        _ => {}
    }
    Ok(())
}

unsafe fn preload_parent(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    access.reset_script_runaway();
    let count = hp2_core::vm_guard::HP2_VM_RUNAWAY_LIMIT - GAP.with(Cell::get);
    assert!(count > 0);
    // SAFETY: the fixture serializes all guard use and keeps the parent frame
    // and target's immutable indexed records live for this synchronous call.
    for step in 0..count { unsafe { access.check_script_frame_statement(frame, step as u64)?; } }
    unsafe { access.execute_script_callback(frame, object, TARGET.with(Cell::get), ptr::null_mut(), DEST.with(Cell::get))?; }
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn replacement(access: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let mut state = ScriptBodyStatements::new();
    let scratch = state.result();
    assert_eq!((scratch as usize) % std::mem::align_of::<u128>(), 0);
    // SAFETY: ScriptBodyStatements initializes its complete FString header.
    let header = unsafe { scratch.cast::<TArrayRaw>().read_unaligned() };
    assert!(header.data.is_null());
    assert_eq!((header.num, header.max), (0, 0));
    SCRATCH.with(|slot| slot.set(scratch));
    loop {
        // SAFETY: this is the currently executing replacement, the original
        // frame and bytecode remain live, and its result slot is VM-owned.
        let Some(captured) = (unsafe { state.begin(access, frame)? }) else { break; };
        let pc = (captured as usize).wrapping_sub(BASE.with(Cell::get) as usize);
        assert!(pc < LENGTH.with(Cell::get));
        CAPTURED.with(|values| values.borrow_mut().push(pc));
        assert_eq!(state.result(), scratch, "one scratch address serves the whole body");
        // SAFETY: the already-guarded live cursor selects a complete fixture
        // statement with a valid persistent 1024-byte statement destination.
        let control = unsafe { access.execute_script_body_step(frame, state.result())? };
        CONTROLS.with(|values| values.borrow_mut().push(control));
        if !EXPECT_DEST.with(Cell::get) && CHILD_VALUES.with(|values| values.borrow().len()) == 1 {
            // SAFETY: the first completed child wrote a full i32 into scratch.
            assert_eq!(unsafe { scratch.cast::<i32>().read_unaligned() }, RETURNED, "scratch retains the child value across the intervening statement");
        }
        if control == StepControl::Return { break; }
    }
    SCRATCH.with(|slot| slot.set(ptr::null_mut()));
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: the fixture owns the newly constructed property.
    unsafe { (*property).array_dim = 1; (*property).element_size = 4; (*property).offset = i32::try_from(offset).unwrap(); (*property).property_flags = flags; }
    property
}
fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: this indexed fixture object remains alive throughout the test.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}
fn assign(code: &mut Vec<u8>, property: *mut UProperty, value: u8) {
    code.push(0x0f); operand(code, 0x01, property.cast()); code.extend([0x2c, value]);
}
fn call(code: &mut Vec<u8>, function: *mut UFunction) {
    operand(code, 0x1c, function.cast()); code.push(0x16);
}

// Declare this after every borrowed code buffer, before any attachment. Its
// registration capacity is reserved before scripts can point into the buffers.
struct DetachScripts(Vec<*mut UFunction>);
impl Drop for DetachScripts {
    fn drop(&mut self) {
        // SAFETY: the owner and all registered records remain live until this
        // guard detaches scripts, including partial setup and VM unwinding.
        unsafe { for &function in &self.0 { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    error: Option<(Vec<u32>, Vec<Vec<u32>>)>,
    top: i32,
    pos: i32,
    result: i32,
    logs: Vec<usize>,
    child_values: Vec<i32>,
    exit_seen: bool,
    exit_pc: Option<usize>,
}

#[test]
fn body_statement_matches_stock_return_redirects_stop_scratch_and_errors() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("body-statement-vm-{}", std::process::id()));
    assert!(!root.exists(), "use a fresh artifact directory");
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
        fs::copy(source.join(from), root.join("System").join(to)).unwrap();
    }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: owner retains the class and records throughout all calls.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>();
    let top = unsafe { int_property(access, class.cast(), "BodyStmtTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let pos = unsafe { int_property(access, class.cast(), "BodyStmtPos", std::mem::offset_of!(UTextBuffer, pos), 0) };
    let mut child_code = Vec::new();
    let mut parent_code = vec![0x04, 0x0b];
    let mut return_code = vec![0x04];
    let mut nothing_code = vec![0x04, 0x0b];
    let mut redirect_code = Vec::new();
    let mut stop_code = vec![hp2_abi::engine::generated::EX_Stop as u8];
    let mut scratch_code = Vec::new();
    let mut detach = DetachScripts(Vec::with_capacity(7));
    assign(&mut child_code, pos, 7);
    child_code.extend([0x04, 0x2c, RETURNED as u8]);
    let child = unsafe { function(access, class, "BodyStmtChild", &mut child_code) };
    detach.0.push(child);
    let child_return = unsafe { int_property(access, child.cast(), "ReturnValue", 0, 0x480) };
    // SAFETY: the reflected child owns a single correctly sized i32 return.
    unsafe {
        (*child).num_parms = 1; (*child).parms_size = 4; (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = 4; (*child.cast::<UStruct>()).children = child_return.cast();
    }
    CHILD.with(|slot| slot.set(child));
    let parent = unsafe { function(access, class, "BodyStmtParent", &mut parent_code) };
    detach.0.push(parent);
    let parent_binding = unsafe { access.install_script_body_replacement("body-stmt-preload", &[parent], preload_parent).unwrap() };
    access.set_vm_observer(Some(observe));

    // All source buffers remain stable while their corresponding functions run.
    call(&mut return_code, child);
    return_code.push(0x38); return_code.extend(100_i32.to_le_bytes());
    return_code.extend(42_i32.to_le_bytes()); return_code.extend(3_i32.to_le_bytes());
    return_code.extend(b"Fixture\0");
    let after_debug = return_code.len(); assign(&mut return_code, top, 99);
    return_code.extend([0x04, 0x0b]);
    let captured_return = unsafe { function(access, class, "BodyStmtReturn", &mut return_code) };
    detach.0.push(captured_return);
    let nothing_after = nothing_code.len(); assign(&mut nothing_code, top, 99);
    nothing_code.extend([0x04, 0x0b]);
    let nothing_return = unsafe { function(access, class, "BodyStmtNothing", &mut nothing_code) };
    detach.0.push(nothing_return);

    assign(&mut redirect_code, top, 11);
    let live_return = redirect_code.len(); redirect_code.push(0x04); call(&mut redirect_code, child);
    assign(&mut redirect_code, top, 99); redirect_code.extend([0x04, 0x0b]);
    let redirected_return = unsafe { function(access, class, "BodyStmtRedirect", &mut redirect_code) };
    detach.0.push(redirected_return);

    assign(&mut stop_code, top, 99); stop_code.extend([0x04, 0x0b]);
    let stop = unsafe { function(access, class, "BodyStmtStop", &mut stop_code) };
    detach.0.push(stop);

    call(&mut scratch_code, child);
    assign(&mut scratch_code, top, 11); call(&mut scratch_code, child);
    scratch_code.extend([0x04, 0x0b]);
    let scratch = unsafe { function(access, class, "BodyStmtScratch", &mut scratch_code) };
    detach.0.push(scratch);

    let run = |target: *mut UFunction, code: &mut Vec<u8>, replaced: bool, redirect: Option<usize>, fail: bool, gap: i32, expect_dest: bool| {
        TARGET.with(|slot| slot.set(target)); BASE.with(|slot| slot.set(code.as_mut_ptr())); LENGTH.with(|slot| slot.set(code.len()));
        REPLACED.with(|slot| slot.set(replaced)); EXPECT_DEST.with(|slot| slot.set(expect_dest));
        REDIRECT.with(|slot| slot.set(redirect)); FAIL_LOG.with(|slot| slot.set(fail)); GAP.with(|slot| slot.set(gap));
        FRAME.with(|slot| slot.set(ptr::null_mut())); SCRATCH.with(|slot| slot.set(ptr::null_mut())); FIRST_CHILD_RESULT.with(|slot| slot.set(0));
        CHILD_VALUES.with(|slot| slot.borrow_mut().clear()); LOG_PCS.with(|slot| slot.borrow_mut().clear()); CAPTURED.with(|slot| slot.borrow_mut().clear()); CONTROLS.with(|slot| slot.borrow_mut().clear());
        EXIT_SEEN.with(|slot| slot.set(false)); EXIT_PC.with(|slot| slot.set(None));
        // SAFETY: the fields belong to the fixture's live TextBuffer.
        unsafe { (*buffer).top = INITIAL; (*buffer).pos = INITIAL; }
        let mut result = INITIAL;
        DEST.with(|slot| slot.set((&raw mut result).cast()));
        let binding = if replaced { Some(unsafe { access.install_script_body_replacement("body-stmt-test", &[target], replacement).unwrap() }) } else { None };
        // SAFETY: all records, immutable bytecode and result storage remain live.
        let outcome = unsafe { access.execute_event(object, parent, ptr::null_mut(), ptr::null_mut()) };
        let error = outcome.err().map(|error| (error.diagnostic().to_vec(), error.context().to_vec()));
        drop(binding);
        access.reset_script_runaway(); FRAME.with(|slot| slot.set(ptr::null_mut())); DEST.with(|slot| slot.set(ptr::null_mut())); SCRATCH.with(|slot| slot.set(ptr::null_mut())); FIRST_CHILD_RESULT.with(|slot| slot.set(0));
        Outcome { error, top: unsafe { (*buffer).top }, pos: unsafe { (*buffer).pos }, result, logs: LOG_PCS.with(|slot| slot.borrow().clone()), child_values: CHILD_VALUES.with(|slot| slot.borrow().clone()), exit_seen: EXIT_SEEN.with(Cell::get), exit_pc: EXIT_PC.with(Cell::get) }
    };

    let stock = run(captured_return, &mut return_code, false, Some(after_debug), false, 0, true);
    assert_eq!((stock.top, stock.pos, stock.result), (INITIAL, 7, RETURNED));
    assert_eq!(stock.logs, vec![0]); assert_eq!(stock.exit_pc, Some(after_debug)); assert!(stock.exit_seen); assert!(stock.error.is_none());
    let actual = run(captured_return, &mut return_code, true, Some(after_debug), false, 0, true);
    assert_eq!(actual, stock, "captured Return overrides the guard redirect and consumes the debug tail");
    assert!(CAPTURED.with(|v| v.borrow().is_empty()), "captured Return is finished by begin");

    let stock = run(nothing_return, &mut nothing_code, false, Some(nothing_after), false, 0, false);
    assert_eq!((stock.top, stock.pos, stock.result), (INITIAL, INITIAL, INITIAL));
    assert_eq!(stock.logs, vec![0]); assert_eq!(stock.exit_pc, Some(nothing_after)); assert!(stock.error.is_none());
    let actual = run(nothing_return, &mut nothing_code, true, Some(nothing_after), false, 0, false);
    assert_eq!(actual, stock, "VOID Return Nothing follows the captured cursor without a debug tail");

    let stock = run(redirected_return, &mut redirect_code, false, Some(live_return), false, 0, false);
    assert_eq!((stock.top, stock.pos, stock.result), (INITIAL, INITIAL, INITIAL));
    assert_eq!(stock.logs, vec![0]); assert_eq!(stock.exit_pc, Some(live_return + 1)); assert!(stock.child_values.is_empty()); assert!(stock.error.is_none());
    let actual = run(redirected_return, &mut redirect_code, true, Some(live_return), false, 0, false);
    assert_eq!(actual, stock, "a redirected live Return does not evaluate its operand");
    assert_eq!(CAPTURED.with(|v| v.borrow().clone()), vec![0]);
    assert_eq!(CONTROLS.with(|v| v.borrow().clone()), vec![StepControl::Return]);

    let stock = run(stop, &mut stop_code, false, None, false, 0, false);
    assert_eq!((stock.top, stock.pos), (INITIAL, INITIAL)); assert!(stock.exit_seen); assert_eq!(stock.exit_pc, None); assert!(stock.error.is_none());
    let actual = run(stop, &mut stop_code, true, None, false, 0, false);
    assert_eq!(actual, stock, "Stop retains its null cursor and prevents following statements");
    assert_eq!(CONTROLS.with(|v| v.borrow().clone()), vec![StepControl::Return]);

    let stock = run(scratch, &mut scratch_code, false, None, false, 100, false);
    assert_eq!(stock.child_values, vec![RETURNED, RETURNED]); assert_eq!((stock.top, stock.pos), (11, 7)); assert!(stock.error.is_none());
    let actual = run(scratch, &mut scratch_code, true, None, false, 100, false);
    assert_eq!(actual, stock, "standalone calls reuse the persistent scratch across a statement");
    assert_eq!(CONTROLS.with(|v| v.borrow().len()), 3);

    let stock = run(redirected_return, &mut redirect_code, false, None, true, 0, false);
    assert!(stock.error.is_some()); assert_eq!(stock.logs, vec![0]); assert!(!stock.exit_seen); assert_eq!((stock.top, stock.pos), (INITIAL, INITIAL));
    let actual = run(redirected_return, &mut redirect_code, true, None, true, 0, false);
    assert_eq!(actual, stock, "guard errors retain first error, PC and partial effects");
    assert!(CAPTURED.with(|v| v.borrow().is_empty()), "failed guard produces no already-guarded statement");

    let stock = run(redirected_return, &mut redirect_code, false, None, true, 1, false);
    assert!(stock.error.is_some()); assert_eq!(stock.logs, vec![live_return]);
    assert_eq!((stock.top, stock.pos), (11, INITIAL), "the first assignment survives the failing Return guard");
    let actual = run(redirected_return, &mut redirect_code, true, None, true, 1, false);
    assert_eq!(actual, stock, "a later guard failure preserves earlier effects and cursor");
    assert_eq!(CAPTURED.with(|v| v.borrow().clone()), vec![0]);

    drop(parent_binding); access.set_vm_observer(None); access.reset_script_runaway();
    TARGET.with(|v| v.set(ptr::null_mut())); CHILD.with(|v| v.set(ptr::null_mut())); BASE.with(|v| v.set(ptr::null_mut()));
    // Calls/bindings end before the guard detaches every borrowed script.
    drop(detach);
    owner.shutdown().unwrap();
}
