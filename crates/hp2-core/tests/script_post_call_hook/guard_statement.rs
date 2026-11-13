//! Real VM comparison of original and Rust-driven top-level statement guards.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{core_access::{RuntimeError, VmObservation}, ScriptBodyStatements, ScriptPreCallDecision};

thread_local! {
    static TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static TARGET_FRAME: Cell<*mut FFrame> = const { Cell::new(ptr::null_mut()) };
    static BUFFER: Cell<*mut UTextBuffer> = const { Cell::new(ptr::null_mut()) };
    static PRELOAD_GAP: Cell<i32> = const { Cell::new(0) };
    static FAIL_LOG: Cell<bool> = const { Cell::new(false) };
    static CHILD_POSTS: Cell<u32> = const { Cell::new(0) };
    static SELECTED_PC: Cell<usize> = const { Cell::new(0) };
    static ELSE_PC: Cell<usize> = const { Cell::new(0) };
    static CALL_PC: Cell<usize> = const { Cell::new(0) };
    static JUMP_PC: Cell<usize> = const { Cell::new(0) };
    static COMMON_PC: Cell<usize> = const { Cell::new(0) };
    static RETURN_PC: Cell<usize> = const { Cell::new(0) };
    static CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static CHILD_RETURN_PC: Cell<usize> = const { Cell::new(0) };
    static EXPECTED_SCRATCH: Cell<*mut std::ffi::c_void> = const { Cell::new(ptr::null_mut()) };
    static CHILD_WRITES: Cell<u32> = const { Cell::new(0) };
    static CHILD_RESULTS: RefCell<Vec<(bool, i32)>> = const { RefCell::new(Vec::new()) };
    static LOGS: RefCell<Vec<(String, i32, i32, usize)>> = const { RefCell::new(Vec::new()) };
}

fn output_guard(_: CoreAccess<'_>, _: i32, units: &[u32]) -> RuntimeResult<()> {
    let message: String = units.iter().filter_map(|&unit| char::from_u32(unit)).collect();
    if !message.contains("Runaway loop detected") { return Ok(()); }
    let frame = TARGET_FRAME.with(Cell::get);
    let buffer = BUFFER.with(Cell::get);
    assert!(!frame.is_null()); assert!(!buffer.is_null());
    // SAFETY: the target frame is active during synchronous output; the
    // fixture retains the real TextBuffer and immutable script throughout it.
    let (top, pos, pc) = unsafe {
        let script = (*(*frame).node.cast::<UStruct>()).script;
        ((*buffer).top, (*buffer).pos, ((*frame).code as usize).wrapping_sub(script.data as usize))
    };
    LOGS.with(|logs| logs.borrow_mut().push((message, top, pos, pc)));
    if FAIL_LOG.with(Cell::get) { return Err(RuntimeError::new("runaway output failure")); }
    Ok(())
}
fn observe(_: CoreAccess<'_>, observation: VmObservation) -> RuntimeResult<()> {
    match observation {
        VmObservation::FunctionEnter { function, frame, .. } if function == TARGET.with(Cell::get) => TARGET_FRAME.with(|slot| slot.set(frame)),
        VmObservation::FunctionExit { function, result, .. } if function == CHILD.with(Cell::get) => {
            assert!(!result.is_null(), "standalone returning calls receive the statement scratch");
            // SAFETY: the child has a reflected four-byte integer return and
            // this synchronous observation precedes return-storage teardown.
            let value = unsafe { result.cast::<i32>().read_unaligned() };
            CHILD_RESULTS.with(|values| values.borrow_mut().push((true, value)));
        }
        _ => {}
    }
    Ok(())
}
unsafe fn child_post(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    CHILD_POSTS.with(|posts| posts.set(posts.get() + 1));
    Ok(())
}

/// This parent keeps the live target and its own frame stable through reentry.
unsafe fn preload_parent(a: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    a.reset_script_runaway();
    let count = hp2_core::vm_guard::HP2_VM_RUNAWAY_LIMIT - PRELOAD_GAP.with(Cell::get);
    assert!(count > 0);
    // SAFETY: the canonical parent frame points to its live original Return;
    // all tests using the process-wide guard must run on one serialized thread.
    for step in 0..count {
        unsafe { a.check_script_frame_statement(frame, step as u64)?; }
    }
    // SAFETY: the target has no parameters and its indexed records remain live.
    unsafe { a.execute_script_callback(frame, object, TARGET.with(Cell::get), ptr::null_mut(), ptr::null_mut())?; }
    Ok(ScriptPreCallDecision::Consume)
}
unsafe fn cursor(frame: *mut FFrame, offset: usize) {
    // SAFETY: all offsets are checked fixture statement boundaries.
    unsafe { let script = (*(*frame).node.cast::<UStruct>()).script; assert!(offset < script.num as usize); (*frame).code = script.data.cast::<u8>().add(offset); }
}
unsafe fn statement(a: CoreAccess<'_>, frame: *mut FFrame, state: &mut ScriptBodyStatements, offset: usize) -> RuntimeResult<()> {
    unsafe { cursor(frame, offset); state.before(a, frame) }
}
unsafe fn step(a: CoreAccess<'_>, frame: *mut FFrame, state: &mut ScriptBodyStatements, offset: usize) -> RuntimeResult<()> {
    unsafe { statement(a, frame, state, offset)?; a.execute_script_frame_step(frame, state.result()) }
}
unsafe fn guarded_body(a: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let mut state = ScriptBodyStatements::new();
    let scratch = state.result();
    assert_eq!((scratch as usize) % std::mem::align_of::<u128>(), 0, "statement scratch retains VM alignment");
    // SAFETY: new() initializes the complete FString header in its scratch area.
    let header = unsafe { scratch.cast::<TArrayRaw>().read_unaligned() };
    assert!(header.data.is_null()); assert_eq!((header.num, header.max), (0, 0));
    // SAFETY: offsets identify statements in the fixture's immutable script;
    // one state persists over the entire body and through nested VM reentry.
    unsafe {
        step(a, frame, &mut state, 0)?;
        let branch = SELECTED_PC.with(Cell::get) - 4;
        step(a, frame, &mut state, branch)?;
        let script = (*(*frame).node.cast::<UStruct>()).script;
        let pc = ((*frame).code as usize).wrapping_sub(script.data as usize);
        if pc == SELECTED_PC.with(Cell::get) {
            // Replace one original assignment directly, after its original guard.
            statement(a, frame, &mut state, pc)?;
            (*object.cast::<UTextBuffer>()).top = 2;
            EXPECTED_SCRATCH.with(|slot| slot.set(state.result()));
            step(a, frame, &mut state, CALL_PC.with(Cell::get))?;
            assert_eq!(state.result().cast::<i32>().read_unaligned(), 123, "the standalone child wrote the live statement scratch");
            step(a, frame, &mut state, JUMP_PC.with(Cell::get))?;
            assert_eq!(state.result().cast::<i32>().read_unaligned(), 123, "scratch persists across the following jump");
        } else {
            assert_eq!(pc, ELSE_PC.with(Cell::get));
            step(a, frame, &mut state, pc)?;
        }
        step(a, frame, &mut state, COMMON_PC.with(Cell::get))?;
        statement(a, frame, &mut state, RETURN_PC.with(Cell::get))?;
        (*frame).code = script.data.cast::<u8>().add(script.num as usize);
    }
    Ok(ScriptPreCallDecision::Consume)
}
unsafe fn child_result_body(a: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, frame: *mut FFrame, result: *mut std::ffi::c_void) -> RuntimeResult<ScriptPreCallDecision> {
    assert!(!result.is_null(), "a standalone function result is not a null slot");
    assert_eq!(result, EXPECTED_SCRATCH.with(Cell::get), "the nested VM callback writes the same persistent statement scratch");
    let mut state = ScriptBodyStatements::new();
    // SAFETY: the child frame and return layout are validated fixture records.
    unsafe {
        step(a, frame, &mut state, 0)?;
        statement(a, frame, &mut state, CHILD_RETURN_PC.with(Cell::get))?;
        result.cast::<i32>().write_unaligned(123);
        let script = (*(*frame).node.cast::<UStruct>()).script;
        (*frame).code = script.data.cast::<u8>().add(script.num as usize);
    }
    CHILD_WRITES.with(|count| count.set(count.get() + 1));
    Ok(ScriptPreCallDecision::Consume)
}
unsafe fn int_property(a: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize) -> *mut UProperty {
    let property = unsafe { a.construct_object(a.native_class("IntProperty").unwrap(), outer, a.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: the fixture owns the newly constructed IntProperty.
    unsafe { (*property).array_dim = 1; (*property).element_size = 4; (*property).offset = i32::try_from(offset).unwrap(); (*property).property_flags = 0; }
    property
}
fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: the indexed fixture record is live through script execution.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}
fn assign(code: &mut Vec<u8>, property: *mut UProperty, value: u8) {
    code.push(0x0f); operand(code, 0x01, property.cast()); code.extend([0x2c, value]);
}

#[test]
fn statement_guard_matches_stock_branches_nested_reentry_and_failure() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("guard-statement-vm-{}", std::process::id()));
    assert!(!root.exists()); fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] { fs::copy(source.join(from), root.join("System").join(to)).unwrap(); }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output: output_guard, log_file: None }).unwrap();
    let a = owner.access();
    a.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: Core owns these records and the fixture retains every script Vec.
    unsafe { a.static_load_class(a.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = a.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { a.construct_object(class, a.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>(); BUFFER.with(|slot| slot.set(buffer));
    let top = unsafe { int_property(a, class.cast(), "GuardStatementTop", std::mem::offset_of!(UTextBuffer, top)) };
    let pos = unsafe { int_property(a, class.cast(), "GuardStatementPos", std::mem::offset_of!(UTextBuffer, pos)) };
    let mut child_code = Vec::new(); assign(&mut child_code, pos, 55);
    let child_return = child_code.len(); child_code.extend([0x04, 0x2c, 123]); // Return IntConstByte.
    let child = unsafe { function(a, class, "GuardStatementChild", &mut child_code) };
    let result_property = unsafe { int_property(a, child.cast(), "ReturnValue", 0) };
    // SAFETY: these live reflected records describe exactly one i32 return.
    unsafe {
        (*result_property).property_flags = 0x480; // Parm | ReturnParm.
        (*child).num_parms = 1; (*child).parms_size = 4; (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = 4; (*child.cast::<UStruct>()).children = result_property.cast();
        assert_eq!((*result_property).element_size, 4);
    }
    CHILD.with(|slot| slot.set(child)); CHILD_RETURN_PC.with(|slot| slot.set(child_return));
    let mut code = Vec::new(); assign(&mut code, top, 1);
    let branch = code.len(); code.extend([0x07, 0, 0, 0x27]); // JumpIfNot, True.
    let condition = branch + 3;
    let selected = code.len(); assign(&mut code, top, 2);
    let call = code.len(); operand(&mut code, 0x1c, child.cast()); code.push(0x16);
    let jump = code.len(); code.extend([0x06, 0, 0]);
    let otherwise = code.len(); assign(&mut code, top, 3);
    let common = code.len(); assign(&mut code, pos, 4);
    let return_pc = code.len(); code.extend([0x04, 0x0b]);
    code[branch + 1..branch + 3].copy_from_slice(&u16::try_from(otherwise).unwrap().to_le_bytes());
    code[jump + 1..jump + 3].copy_from_slice(&u16::try_from(common).unwrap().to_le_bytes());
    let target = unsafe { function(a, class, "GuardStatementTarget", &mut code) };
    let mut parent_code = vec![0x04, 0x0b];
    let parent = unsafe { function(a, class, "GuardStatementParent", &mut parent_code) };
    TARGET.with(|slot| slot.set(target)); SELECTED_PC.with(|v| v.set(selected)); ELSE_PC.with(|v| v.set(otherwise));
    CALL_PC.with(|v| v.set(call)); JUMP_PC.with(|v| v.set(jump)); COMMON_PC.with(|v| v.set(common)); RETURN_PC.with(|v| v.set(return_pc));
    let parent_guard = unsafe { a.install_script_body_replacement("guard-preload-parent", &[parent], preload_parent).unwrap() };
    let post_guard = unsafe { a.install_script_post_call_hook([child, child], child_post).unwrap() };
    a.set_vm_observer(Some(observe));
    for selected_branch in [true, false] {
        code[condition] = if selected_branch { 0x27 } else { 0x28 };
        // The direct Core `execute_flow` branch path currently has no extra
        // jump guard. Place the threshold at the child assignment or common
        // assignment. The stock run remains the parity oracle if that changes.
        PRELOAD_GAP.with(|slot| slot.set(if selected_branch { 4 } else { 3 }));
        for fail in [false, true] {
            FAIL_LOG.with(|slot| slot.set(fail));
            let run = || {
                // SAFETY: `top` and `pos` are owned data fields, not identities.
                unsafe { (*buffer).top = -7; (*buffer).pos = -7; }
                LOGS.with(|logs| logs.borrow_mut().clear()); CHILD_POSTS.with(|v| v.set(0)); TARGET_FRAME.with(|v| v.set(ptr::null_mut()));
                EXPECTED_SCRATCH.with(|v| v.set(ptr::null_mut())); CHILD_WRITES.with(|v| v.set(0)); CHILD_RESULTS.with(|v| v.borrow_mut().clear());
                let outcome = unsafe { a.execute_event(object, parent, ptr::null_mut(), ptr::null_mut()) };
                let diagnostic = outcome.err().map(|error| (error.diagnostic().to_vec(), error.context().to_vec()));
                let values = unsafe { ((*buffer).top, (*buffer).pos) };
                let logs = LOGS.with(|logs| logs.borrow().clone());
                let posts = CHILD_POSTS.with(Cell::get);
                let child_results = CHILD_RESULTS.with(|results| results.borrow().clone());
                // The process-global guard must not leak across fixture runs.
                a.reset_script_runaway();
                (diagnostic, values, logs, posts, child_results)
            };
            let baseline = run();
            assert_eq!(baseline.0.is_some(), fail, "stock error on output failure");
            assert_eq!(baseline.2.len(), 1, "stock threshold produces exactly one log");
            assert_eq!(baseline.2[0].1, if selected_branch { 2 } else { 3 });
            assert_eq!(baseline.2[0].2, -7, "the log occurs before the selected child/common assignment");
            assert_eq!(baseline.2[0].3, if selected_branch { jump } else { common }, "the log observes the original target cursor");
            assert_eq!(baseline.3, u32::from(selected_branch && !fail), "nested callback post runs only after successful child execution");
            assert_eq!(baseline.4, if selected_branch && !fail { vec![(true, 123)] } else { vec![] }, "stock child returns into non-null statement scratch");
            if fail { assert_eq!(baseline.1, (if selected_branch { 2 } else { 3 }, -7), "failed guard preserves partial effects and prevents continuation"); }
            else { assert_eq!(baseline.1, (if selected_branch { 2 } else { 3 }, 4)); }
            let replacement = unsafe { a.install_script_body_replacement("guard-result-body", &[target], guarded_body).unwrap() };
            let child_binding = unsafe { a.install_script_result_body_replacement("guard-child-result", &[child], child_result_body).unwrap() };
            let actual = run();
            assert_eq!(actual, baseline, "replacement preserves logs, PCs, partial effects, nested posts, scratch result and errors: selected={selected_branch}, fail={fail}");
            assert_eq!(CHILD_WRITES.with(Cell::get), u32::from(selected_branch && !fail), "the result-aware child writes the same non-null scratch on success");
            drop(child_binding); drop(replacement);
        }
    }
    drop(post_guard); drop(parent_guard); a.set_vm_observer(None);
    a.reset_script_runaway(); TARGET_FRAME.with(|v| v.set(ptr::null_mut())); TARGET.with(|v| v.set(ptr::null_mut())); BUFFER.with(|v| v.set(ptr::null_mut()));
    CHILD.with(|v| v.set(ptr::null_mut())); EXPECTED_SCRATCH.with(|v| v.set(ptr::null_mut()));
    // SAFETY: calls and guards have ended; detach scripts before Vec teardown.
    unsafe { for f in [target, parent, child] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    owner.shutdown().unwrap();
}
