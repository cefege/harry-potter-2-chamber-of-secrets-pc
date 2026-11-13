//! Real VM regression for result-aware interpreted body replacements.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{core_access::RuntimeError, ScriptPreCallDecision, ScriptScopePhase};
use std::ffi::c_void;

const ORIGINAL: i32 = 9;
const REPLACED: i32 = 0x1122_3344;
const AFTER_POST: i32 = 0x5566_7788;
const INITIAL: i32 = -31;

thread_local! {
    static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static EXPECTED_RESULT: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
    static PROBE: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static FALLBACK: Cell<bool> = const { Cell::new(false) };
    static FAIL_POST: Cell<bool> = const { Cell::new(false) };
    static FAIL_BODY: Cell<bool> = const { Cell::new(false) };
    static CHANGE_POST: Cell<bool> = const { Cell::new(false) };
    static NULL_RESULT: Cell<bool> = const { Cell::new(false) };
    static IGNORED_CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static IGNORED_MODE: Cell<bool> = const { Cell::new(false) };
    static PARAM_COPYBACK: Cell<i32> = const { Cell::new(0) };
    static LOCAL_EVENTS: RefCell<Vec<(&'static str, i32)>> = const { RefCell::new(Vec::new()) };
    static OBSERVED_NULL: RefCell<Vec<(&'static str, bool)>> = const { RefCell::new(Vec::new()) };
    static MANAGED_FAIL: Cell<bool> = const { Cell::new(false) };
    static MANAGED_CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static MANAGED_SCOPE: RefCell<Vec<(&'static str, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
}
fn record(event: &'static str) { EVENTS.with(|events| events.borrow_mut().push(event)); }

// The fixture supplies a live TextBuffer, a returning function and VM frame.
unsafe fn replacement(_: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<ScriptPreCallDecision> {
    record("body");
    // SAFETY: the VM owns this frame and both indexed records during the call.
    unsafe { assert_eq!((*frame).object, object.cast()); assert_eq!((*frame).node, function.cast()); }
    if NULL_RESULT.with(Cell::get) {
        // SAFETY: this function's return metadata is a four-byte local at the
        // validated offset; the VM owns its frame and storage for this call.
        let expected = unsafe { (*frame).locals.add(usize::from((*function).return_value_offset)).cast::<c_void>() };
        assert!(!result.is_null(), "ignored calls still have an effective return slot");
        assert_eq!(result, expected, "the effective destination is the canonical return local");
        // SAFETY: the fixture validates a live four-byte IntProperty return.
        unsafe { result.cast::<i32>().write_unaligned(ORIGINAL); }
        return Ok(ScriptPreCallDecision::Consume);
    }
    assert_eq!(result, EXPECTED_RESULT.with(Cell::get), "the callback gets the original destination");
    assert!(!result.is_null());
    if FALLBACK.with(Cell::get) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: admission in the fixture creates a four-byte IntProperty return;
    // the real LET supplies the live UTextBuffer.top i32 as its destination.
    unsafe { result.cast::<i32>().write_unaligned(REPLACED); }
    if FAIL_BODY.with(Cell::get) { return Err(RuntimeError::new("result fixture body failure")); }
    Ok(ScriptPreCallDecision::Consume)
}

fn observe(_: CoreAccess<'_>, event: hp2_core::core_access::VmObservation) -> RuntimeResult<()> {
    use hp2_core::core_access::VmObservation;
    let (label, function, result) = match event {
        VmObservation::FunctionExit { function, result, .. } => ("function-exit", function, result),
        VmObservation::ScriptCallExit { function, result, .. } => ("script-exit", function, result),
        _ => return Ok(()),
    };
    if IGNORED_MODE.with(Cell::get) && function == IGNORED_CHILD.with(Cell::get) {
        OBSERVED_NULL.with(|values| values.borrow_mut().push((label, result.is_null())));
    }
    Ok(())
}

unsafe fn ignored_parent(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let mut param = INITIAL;
    // SAFETY: the fixture holds the parent frame and child live. The child's
    // reflected parameter prefix is one initialized i32, with null result.
    let result = unsafe { access.execute_script_callback(frame, object, IGNORED_CHILD.with(Cell::get), (&raw mut param).cast(), ptr::null_mut()) };
    PARAM_COPYBACK.with(|slot| slot.set(param));
    result?;
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn managed_body(access: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<ScriptPreCallDecision> {
    // SAFETY: this fixture validates a constructed FString return slot at offset 0.
    let local = unsafe { (*frame).locals.add(usize::from((*function).return_value_offset)).cast::<c_void>() };
    assert_eq!(result, local, "ignored managed return uses the original VM local");
    assert!(!result.is_null());
    // SAFETY: the constructor-linked return is initialized, correctly sized,
    // and owned until the VM transfers the parameter prefix back to the caller.
    unsafe { access.assign_fstring(result.cast(), &[u32::from('V'), u32::from('M'), 0])?; }
    Ok(ScriptPreCallDecision::Consume)
}
unsafe fn managed_scope(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, params: *const u8, size: usize, phase: ScriptScopePhase) -> RuntimeResult<()> {
    assert_eq!(size, size_of::<TArrayRaw>());
    let label = match phase { ScriptScopePhase::Enter => "enter", ScriptScopePhase::Exit { succeeded: true } => "exit-ok", ScriptScopePhase::Exit { succeeded: false } => "exit-error" };
    // SAFETY: the return parameter is a live, initialized reflected FString.
    let units = unsafe { hp2_core::native_text::fstring_units(params.cast::<TArrayRaw>()) }.iter().map(|&v| v as u32).collect();
    MANAGED_SCOPE.with(|events| events.borrow_mut().push((label, units)));
    if label == "exit-ok" && MANAGED_FAIL.with(Cell::get) { return Err(RuntimeError::new("managed scope failure")); }
    Ok(())
}
fn managed_observe(_: CoreAccess<'_>, event: hp2_core::core_access::VmObservation) -> RuntimeResult<()> {
    if let hp2_core::core_access::VmObservation::FunctionExit { function, result, .. } = event {
        if function == MANAGED_CHILD.with(Cell::get) { OBSERVED_NULL.with(|values| values.borrow_mut().push(("function-exit", result.is_null()))); }
    }
    Ok(())
}

unsafe fn legacy(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    Ok(ScriptPreCallDecision::Continue)
}
unsafe fn pre(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<ScriptPreCallDecision> {
    record("pre");
    Ok(ScriptPreCallDecision::Continue)
}
unsafe fn scope(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, params: *const u8, size: usize, phase: ScriptScopePhase) -> RuntimeResult<()> {
    let label = match phase { ScriptScopePhase::Enter => "enter", ScriptScopePhase::Exit { succeeded: true } => "exit-ok", ScriptScopePhase::Exit { succeeded: false } => "exit-error" };
    record(label);
    if IGNORED_MODE.with(Cell::get) {
        assert_eq!(size, 4);
        // SAFETY: the fixture's returning function has one i32 parameter slot.
        let value = unsafe { params.cast::<i32>().read_unaligned() };
        LOCAL_EVENTS.with(|events| events.borrow_mut().push((label, value)));
    }
    Ok(())
}
unsafe fn post(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    record("post");
    if IGNORED_MODE.with(Cell::get) {
        assert_eq!(size, 4);
        // SAFETY: the child has a live i32 return-parameter slot.
        let value = unsafe { params.cast::<i32>().read_unaligned() };
        LOCAL_EVENTS.with(|events| events.borrow_mut().push(("post", value)));
        if FAIL_POST.with(Cell::get) { return Err(RuntimeError::new("result fixture post failure")); }
        return Ok(());
    }
    let buffer = object.cast::<UTextBuffer>();
    // SAFETY: the receiver is the fixture's live TextBuffer. The probe is a
    // synchronous interpreted event with no parameters and no retained records.
    unsafe {
        assert_eq!((*buffer).top, REPLACED, "post sees the direct return write");
        assert_eq!((*buffer).pos, INITIAL, "the four-byte result did not overwrite its neighbor");
        access.execute_event(object, PROBE.with(Cell::get), ptr::null_mut(), ptr::null_mut())?;
        assert_eq!((*buffer).pos, REPLACED, "reentrant bytecode sees the write");
        if CHANGE_POST.with(Cell::get) { (*buffer).top = AFTER_POST; }
    }
    if FAIL_POST.with(Cell::get) { return Err(RuntimeError::new("result fixture post failure")); }
    Ok(())
}

// The caller retains the owner, outer and constructed property through VM use.
unsafe fn int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: property is a newly constructed, live IntProperty fixture record.
    unsafe { (*property).array_dim = 1; (*property).element_size = 4; (*property).offset = i32::try_from(offset).unwrap(); (*property).property_flags = flags; }
    property
}
fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: the fixture retains indexed records while its script is live.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}

#[test]
fn result_body_replacement_preserves_real_destination_and_lifecycle() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("result-body-vm-{}", std::process::id()));
    assert!(!root.exists(), "use a fresh isolated artifact directory");
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
        fs::copy(source.join(from), root.join("System").join(to)).unwrap();
    }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: the owner and loaded class remain live until after all VM calls.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>();
    let top = unsafe { int_property(access, class.cast(), "ResultBodyTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let pos = unsafe { int_property(access, class.cast(), "ResultBodyPos", std::mem::offset_of!(UTextBuffer, pos), 0) };

    let mut child_code = vec![0x04, 0x1d]; // Return IntConst.
    child_code.extend(ORIGINAL.to_le_bytes());
    let child = unsafe { function(access, class, "ResultBodyChild", &mut child_code) };
    let return_property = unsafe { int_property(access, child.cast(), "ReturnValue", 0, 0x480) }; // Parm | ReturnParm.
    unsafe {
        (*child).num_parms = 1; (*child).parms_size = 4; (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = 4;
        (*child.cast::<UStruct>()).children = return_property.cast();
        assert_eq!((*return_property).array_dim, 1);
        assert_eq!((*return_property).element_size, 4);
        assert_eq!((*return_property).offset, i32::from((*child).return_value_offset));
        assert_eq!((*return_property).property_flags & 0x480, 0x480);
    }
    let mut other_code = vec![0x04, 0x1d]; other_code.extend(ORIGINAL.to_le_bytes());
    let other = unsafe { function(access, class, "ResultBodyOther", &mut other_code) };
    let other_return = unsafe { int_property(access, other.cast(), "ReturnValue", 0, 0x480) };
    unsafe { (*other).num_parms = 1; (*other).parms_size = 4; (*other).return_value_offset = 0; (*other.cast::<UStruct>()).properties_size = 4; (*other.cast::<UStruct>()).children = other_return.cast(); }
    let mut void_code = vec![0x04, 0x0b];
    let void = unsafe { function(access, class, "ResultBodyVoid", &mut void_code) };
    let mut native_code = vec![0x04, 0x0b];
    let native = unsafe { function(access, class, "ResultBodyNative", &mut native_code) };
    let native_return = unsafe { int_property(access, native.cast(), "ReturnValue", 0, 0x480) };
    // A complete returning native record isolates native rejection from void rejection.
    unsafe {
        (*native).function_flags = 0x400; (*native).num_parms = 1; (*native).parms_size = 4;
        (*native).return_value_offset = 0; (*native.cast::<UStruct>()).properties_size = 4;
        (*native.cast::<UStruct>()).children = native_return.cast();
    }

    let mut caller_code = vec![0x0f]; // LET TextBuffer.top = child().
    operand(&mut caller_code, 0x01, top.cast());
    operand(&mut caller_code, 0x1c, child.cast());
    caller_code.push(0x16);
    caller_code.push(0x0f); // Continuation proves a successful call returned.
    operand(&mut caller_code, 0x01, pos.cast());
    caller_code.extend([0x2c, 77, 0x04, 0x0b]);
    let caller = unsafe { function(access, class, "ResultBodyCaller", &mut caller_code) };
    let mut probe_code = vec![0x0f];
    operand(&mut probe_code, 0x01, pos.cast());
    operand(&mut probe_code, 0x01, top.cast());
    probe_code.extend([0x04, 0x0b]);
    let probe = unsafe { function(access, class, "ResultBodyProbe", &mut probe_code) };
    PROBE.with(|slot| slot.set(probe));
    // SAFETY: `top` and `pos` are actual four-byte data fields, not identities.
    let destination = unsafe { std::ptr::addr_of_mut!((*buffer).top).cast::<c_void>() };
    EXPECTED_RESULT.with(|slot| slot.set(destination));
    let reset = || { unsafe { (*buffer).top = INITIAL; (*buffer).pos = INITIAL; } EVENTS.with(|events| events.borrow_mut().clear()); };
    let run = || unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()) };
    reset(); run().unwrap();
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (ORIGINAL, 77), "the unmodified VM executes the original body");

    assert!(unsafe { access.install_script_result_body_replacement("result-empty", &[], replacement) }.is_err());
    assert!(unsafe { access.install_script_result_body_replacement("result-null", &[ptr::null_mut()], replacement) }.is_err());
    assert!(unsafe { access.install_script_result_body_replacement("result-void", &[void], replacement) }.is_err());
    assert!(unsafe { access.install_script_result_body_replacement("result-native", &[native], replacement) }.is_err());
    let binding = unsafe { access.install_script_result_body_replacement("result-binding", &[child, child], replacement).unwrap() };
    assert!(unsafe { access.install_script_result_body_replacement("result-binding", &[other], replacement) }.is_err(), "duplicate key");
    assert!(unsafe { access.install_script_body_replacement("result-binding", &[void], legacy) }.is_err(), "keys overlap across old and result APIs");
    assert!(unsafe { access.install_script_body_replacement("old-overlap", &[child], legacy) }.is_err(), "function overlap across APIs");
    assert!(unsafe { access.install_script_result_body_replacement("result-overlap", &[child], replacement) }.is_err());
    let pre_guard = unsafe { access.install_script_pre_observer("result-pre", &[child], pre).unwrap() };
    let scope_guard = unsafe { access.install_script_scope_observer("result-scope", &[child], scope).unwrap() };
    let post_guard = unsafe { access.install_script_post_call_hook([child, child], post).unwrap() };

    reset(); CHANGE_POST.with(|slot| slot.set(true)); run().unwrap(); CHANGE_POST.with(|slot| slot.set(false));
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (AFTER_POST, 77), "no delayed copy overwrites the post observer");
    EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre", "enter", "body", "exit-ok", "post"]));
    reset(); FAIL_POST.with(|slot| slot.set(true));
    let error = run().unwrap_err(); FAIL_POST.with(|slot| slot.set(false));
    assert_eq!(error.diagnostic(), "result fixture post failure".chars().map(u32::from).collect::<Vec<_>>());
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (REPLACED, REPLACED), "write and reentry survive the later error; caller continuation does not run");
    EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre", "enter", "body", "exit-ok", "post"]));
    reset(); FAIL_BODY.with(|slot| slot.set(true));
    let error = run().unwrap_err(); FAIL_BODY.with(|slot| slot.set(false));
    assert_eq!(error.diagnostic(), "result fixture body failure".chars().map(u32::from).collect::<Vec<_>>());
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (REPLACED, INITIAL), "body failure retains its write and stops continuation");
    EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre", "enter", "body", "exit-error"], "body error closes scope and skips post"));
    drop(post_guard);

    reset(); FALLBACK.with(|slot| slot.set(true)); run().unwrap(); FALLBACK.with(|slot| slot.set(false));
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (ORIGINAL, 77), "Continue before effects runs the real original return");
    EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre", "enter", "body", "exit-ok"]));
    drop(binding);
    IGNORED_CHILD.with(|slot| slot.set(child));
    IGNORED_MODE.with(|slot| slot.set(true)); NULL_RESULT.with(|slot| slot.set(true));
    access.set_vm_observer(Some(observe));
    let parent = unsafe { access.install_script_body_replacement("ignored-parent", &[void], ignored_parent).unwrap() };
    let ignored_post = unsafe { access.install_script_post_call_hook([child, child], post).unwrap() };
    let ignored_run = || {
        reset(); LOCAL_EVENTS.with(|events| events.borrow_mut().clear()); OBSERVED_NULL.with(|events| events.borrow_mut().clear());
        PARAM_COPYBACK.with(|slot| slot.set(INITIAL));
        let outcome = unsafe { access.execute_event(object, void, ptr::null_mut(), ptr::null_mut()) };
        (outcome, PARAM_COPYBACK.with(Cell::get), LOCAL_EVENTS.with(|e| e.borrow().clone()), OBSERVED_NULL.with(|e| e.borrow().clone()))
    };
    for fail in [false, true] {
        FAIL_POST.with(|slot| slot.set(fail));
        let (baseline, baseline_param, baseline_scope, baseline_observed) = ignored_run();
        assert_eq!(baseline.is_err(), fail);
        assert_eq!(baseline_param, ORIGINAL, "the stock return local is copied back even on post failure");
        assert_eq!(baseline_scope, vec![("enter", INITIAL), ("exit-ok", ORIGINAL), ("post", ORIGINAL)]);
        assert_eq!(baseline_observed, vec![("function-exit", true), ("script-exit", true)]);
        let result_guard = unsafe { access.install_script_result_body_replacement("ignored-child", &[child], replacement).unwrap() };
        let (actual, param, local_events, observations) = ignored_run();
        assert_eq!(actual.is_err(), fail);
        if fail { assert_eq!(actual.unwrap_err().diagnostic(), baseline.unwrap_err().diagnostic()); }
        assert_eq!(param, baseline_param, "replacement and original copy back the same result-local bytes");
        assert_eq!(local_events, baseline_scope, "scope and post see the same live result-local value");
        assert_eq!(observations, baseline_observed, "observers keep the original null result pointer");
        assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (INITIAL, INITIAL));
        drop(result_guard);
    }
    FAIL_POST.with(|slot| slot.set(false));
    drop(ignored_post); drop(parent); access.set_vm_observer(None);
    IGNORED_MODE.with(|slot| slot.set(false)); NULL_RESULT.with(|slot| slot.set(false));
    IGNORED_CHILD.with(|slot| slot.set(ptr::null_mut()));
    drop(pre_guard); drop(scope_guard);
    reset(); run().unwrap();
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (ORIGINAL, 77), "dropping the result guard restores the original body");
    assert!(EVENTS.with(|events| events.borrow().is_empty()));
    let old = unsafe { access.install_script_body_replacement("old-binding", &[child], legacy).unwrap() };
    assert!(unsafe { access.install_script_result_body_replacement("new-over-old", &[child], replacement) }.is_err());
    assert!(unsafe { access.install_script_result_body_replacement("old-binding", &[other], replacement) }.is_err());
    drop(old);
    let again = unsafe { access.install_script_result_body_replacement("result-binding", &[child], replacement).unwrap() };
    reset(); run().unwrap();
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (REPLACED, 77), "a dropped key can be installed again");
    drop(again);
    PROBE.with(|slot| slot.set(ptr::null_mut())); EXPECTED_RESULT.with(|slot| slot.set(ptr::null_mut()));
    // SAFETY: no VM call or guard remains; clear borrowed script Vec storage
    // before the owned code buffers or Core owner are destroyed. No GC is run.
    unsafe { for f in [child, other, void, native, caller, probe] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    owner.shutdown().unwrap();
}

#[test]
fn ignored_managed_result_matches_original_copyback_and_scope_failure() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("managed-result-vm-{}", std::process::id()));
    assert!(!root.exists());
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
        fs::copy(source.join(from), root.join("System").join(to)).unwrap();
    }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: owner, fixture classes and bytecode remain live through execution.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let mut code = vec![0x04, 0x1f, b'V', b'M', 0]; // Return StringConst.
    let child = unsafe { function(access, class, "ManagedResultChild", &mut code) };
    let property = unsafe { access.construct_object(access.native_class("StrProperty").unwrap(), child.cast(), access.intern_str("ReturnValue", FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    let width = size_of::<TArrayRaw>();
    // SAFETY: these are newly constructed live reflected fixture records.
    unsafe {
        (*property).array_dim = 1; (*property).element_size = i32::try_from(width).unwrap();
        (*property).offset = 0; (*property).property_flags = 0x480;
        (*property).constructor_link_next = ptr::null_mut();
        (*child).num_parms = 1; (*child).parms_size = u16::try_from(width).unwrap();
        (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = i32::try_from(width).unwrap();
        (*child.cast::<UStruct>()).properties_align = i32::try_from(std::mem::align_of::<TArrayRaw>()).unwrap();
        (*child.cast::<UStruct>()).children = property.cast();
        (*child.cast::<UStruct>()).constructor_link = property.cast();
        assert_eq!((*property).element_size as usize, width);
        assert_eq!((*property).property_flags & 0x480, 0x480);
    }
    MANAGED_CHILD.with(|slot| slot.set(child));
    access.set_vm_observer(Some(managed_observe));
    let scope_guard = unsafe { access.install_script_scope_observer("managed-result-scope", &[child], managed_scope).unwrap() };
    let run = || {
        MANAGED_SCOPE.with(|events| events.borrow_mut().clear()); OBSERVED_NULL.with(|events| events.borrow_mut().clear());
        let mut parameter = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        // SAFETY: the parameter is a live initialized FString of the reflected
        // width. A null incoming result selects the VM's return-local storage.
        let outcome = unsafe { access.execute_event(object, child, (&raw mut parameter).cast(), ptr::null_mut()) };
        let units = unsafe { hp2_core::native_text::fstring_units(&parameter) }.iter().map(|&v| v as u32).collect::<Vec<_>>();
        let scope = MANAGED_SCOPE.with(|events| events.borrow().clone());
        let observed = OBSERVED_NULL.with(|events| events.borrow().clone());
        // SAFETY: this copied-back parameter is the sole caller-owned FString.
        // Release it once, including after the deliberate scope-exit failure.
        unsafe { access.destroy_owned_fstring(&mut parameter).unwrap(); }
        assert!(parameter.data.is_null());
        assert_eq!(parameter.num, 0);
        (outcome, units, scope, observed)
    };
    for fail in [false, true] {
        MANAGED_FAIL.with(|slot| slot.set(fail));
        let (baseline, baseline_units, baseline_scope, baseline_observed) = run();
        assert_eq!(baseline.is_err(), fail);
        assert_eq!(baseline_units, vec![u32::from('V'), u32::from('M')]);
        assert_eq!(baseline_scope, vec![("enter", vec![]), ("exit-ok", baseline_units.clone())]);
        assert_eq!(baseline_observed, if fail { vec![] } else { vec![("function-exit", true)] });
        let guard = unsafe { access.install_script_result_body_replacement("managed-result-body", &[child], managed_body).unwrap() };
        let (actual, units, scope, observed) = run();
        assert_eq!(actual.is_err(), fail);
        if fail { assert_eq!(actual.unwrap_err().diagnostic(), baseline.unwrap_err().diagnostic()); }
        assert_eq!(units, baseline_units, "return-local FString content and copyback match");
        assert_eq!(scope, baseline_scope, "the scope sees the same managed values");
        assert_eq!(observed, baseline_observed, "the original observer pointer remains null");
        drop(guard);
    }
    MANAGED_FAIL.with(|slot| slot.set(false)); MANAGED_CHILD.with(|slot| slot.set(ptr::null_mut()));
    drop(scope_guard); access.set_vm_observer(None);
    // SAFETY: no VM calls remain; detach the borrowed bytecode before teardown.
    unsafe { (*child.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    owner.shutdown().unwrap();
}
