//! Real VM calls exercise the optional scoped post-call hook.
#[path = "script_post_call_hook/health_boundary.rs"]
mod health_boundary;
#[path = "script_post_call_hook/result_body.rs"]
mod result_body;
#[path = "script_post_call_hook/guard_statement.rs"]
mod guard_statement;
#[path = "script_post_call_hook/lazy_arguments.rs"]
mod lazy_arguments;
#[path = "script_post_call_hook/body_statement.rs"]
mod body_statement;
#[path = "script_post_call_hook/snapshot_host.rs"]
mod snapshot_host;
#[path = "script_post_call_hook/assignment_value.rs"]
mod assignment_value;
#[path = "script_post_call_hook/body_identity.rs"]
mod body_identity;
use hp2_abi::engine::generated::{FStateFrame, TArrayRaw, UField, UFunction, UObject, UState, UStruct};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{cell::{Cell, RefCell}, fs, path::PathBuf, ptr};

thread_local! {
    static BODY_EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static BODY_CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static BODY_PARENT: Cell<*mut hp2_abi::engine::generated::FFrame> = const { Cell::new(ptr::null_mut()) };
    static BODY_NATIVE: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static BODY_FAIL: Cell<bool> = const { Cell::new(false) };
    static BODY_CONSUME: Cell<bool> = const { Cell::new(false) };
    static BODY_FALLBACK: Cell<bool> = const { Cell::new(false) };
    static FRAME_WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static FINAL_TARGET: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static CALLBACK_STATE: Cell<*mut UState> = const { Cell::new(ptr::null_mut()) };
    static COUNT: Cell<u32> = const { Cell::new(0) };
    static OBSERVER_COUNT: Cell<u32> = const { Cell::new(0) };
    static PRE_COUNT: Cell<u32> = const { Cell::new(0) };
    static PRE_CONSUME: Cell<bool> = const { Cell::new(false) };
    static SCOPES: Cell<u32> = const { Cell::new(0) };
    static REENTER: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static RESOLVED_EVENTS: RefCell<Vec<(*mut UFunction, hp2_core::ScriptScopePhase)>> = const { RefCell::new(Vec::new()) };
    static SELECT_FAIL: Cell<bool> = const { Cell::new(false) };
    static FAIL_FUNCTION: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static SKIP_FUNCTION: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static EXIT_FAIL: Cell<bool> = const { Cell::new(false) };
    static ENTER_FAIL: Cell<bool> = const { Cell::new(false) };
    static EXPECTED_RECEIVER: Cell<*mut UObject> = const { Cell::new(ptr::null_mut()) };
}
fn body_event(event: &'static str) { BODY_EVENTS.with(|events| events.borrow_mut().push(event)); }
unsafe fn replacement_pre(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    body_event(if function == BODY_CHILD.with(Cell::get) { "pre-child" } else { "pre-parent" });
    Ok(if BODY_CONSUME.with(Cell::get) { hp2_core::ScriptPreCallDecision::Consume } else { hp2_core::ScriptPreCallDecision::Continue })
}
unsafe fn replacement_scope(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, _: *const u8, _: usize, phase: hp2_core::ScriptScopePhase) -> RuntimeResult<()> {
    let child = function == BODY_CHILD.with(Cell::get);
    body_event(match (child, phase) {
        (false, hp2_core::ScriptScopePhase::Enter) => "enter-parent",
        (true, hp2_core::ScriptScopePhase::Enter) => "enter-child",
        (false, hp2_core::ScriptScopePhase::Exit { succeeded: true }) => "exit-parent-ok",
        (true, hp2_core::ScriptScopePhase::Exit { succeeded: true }) => "exit-child-ok",
        (false, hp2_core::ScriptScopePhase::Exit { succeeded: false }) => "exit-parent-error",
        (true, hp2_core::ScriptScopePhase::Exit { succeeded: false }) => "exit-child-error",
    });
    Ok(())
}
unsafe fn replacement_post(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    body_event(if function == BODY_CHILD.with(Cell::get) { "post-child" } else { "post-parent" });
    Ok(())
}
unsafe fn replacement_body(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut hp2_abi::engine::generated::FFrame) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    if BODY_FALLBACK.with(Cell::get) { return Ok(hp2_core::ScriptPreCallDecision::Continue); }
    assert!(!frame.is_null());
    // SAFETY: the replacement callback borrows the canonical VM's live frame.
    unsafe {
        assert_eq!((*frame).object, object.cast());
        assert_eq!((*frame).node, function.cast());
    }
    let child = BODY_CHILD.with(Cell::get);
    if function == child {
        // SAFETY: the synchronous parent replacement is still on the stack.
        assert_eq!(unsafe { (*frame).parent }, BODY_PARENT.with(Cell::get));
        body_event("body-child");
    } else {
        body_event("body-parent");
        let native = BODY_NATIVE.with(Cell::get);
        // A native binding needs bytecode arguments, not this reflected buffer.
        // SAFETY: the current frame and native fixture record are live; the
        // API must reject the unsupported target before executing its body.
        assert!(unsafe { access.execute_script_callback(frame, object, native, ptr::null_mut(), ptr::null_mut()) }.is_err());
        BODY_PARENT.with(|slot| slot.set(frame));
        // SAFETY: both functions have empty reflected parameter layouts and
        // all fixture records remain stable throughout synchronous reentry.
        let result = unsafe { access.execute_script_callback(frame, object, child, ptr::null_mut(), ptr::null_mut()) };
        BODY_PARENT.with(|slot| slot.set(ptr::null_mut()));
        result?;
        body_event("resume-parent");
        if BODY_FAIL.with(Cell::get) {
            return Err(hp2_core::core_access::RuntimeError::new("replacement body failed"));
        }
    }
    Ok(hp2_core::ScriptPreCallDecision::Consume)
}
unsafe fn final_body(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut hp2_abi::engine::generated::FFrame) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    // SAFETY: the fixture supplies a live canonical target with no parameters,
    // and the replacement borrows the live parent frame for synchronous entry.
    unsafe { access.execute_script_callback(frame, object, FINAL_TARGET.with(Cell::get), ptr::null_mut(), ptr::null_mut())?; }
    Ok(hp2_core::ScriptPreCallDecision::Consume)
}
unsafe fn change_callback_state(_: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    // SAFETY: this fixture owns the receiver's initialized state frame and the
    // live state belongs to its class. Only the active-state selection changes.
    unsafe { (*(*object).state_frame.cast::<FStateFrame>()).state_node = CALLBACK_STATE.with(Cell::get).cast(); }
    Ok(())
}
unsafe fn state_reentry_body(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut hp2_abi::engine::generated::FFrame) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    let first = FINAL_TARGET.with(Cell::get);
    // SAFETY: both reflected functions have no parameters; all fixture records
    // and the canonical parent frame remain live throughout callback reentry.
    unsafe { access.execute_script_callback(frame, object, first, ptr::null_mut(), ptr::null_mut())?; }
    let name = i32::from_ne_bytes(unsafe { (*first.cast::<UObject>()).name });
    let after = unsafe { access.find_object_function_by_name(object, name, false)? };
    assert_ne!(after, first, "the first callback changes the next virtual target");
    unsafe { access.execute_script_callback(frame, object, after, ptr::null_mut(), ptr::null_mut())?; }
    Ok(hp2_core::ScriptPreCallDecision::Consume)
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn stage(name: &str) {
    use std::io::Write;
    writeln!(std::io::stderr().lock(), "script-scope-test stage={name}").expect("write test stage");
}
unsafe fn observed(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, _: *const u8, size: usize) -> RuntimeResult<()> {
    assert_eq!(size, 0);
    COUNT.with(|count| count.set(count.get() + 1));
    let caller = REENTER.with(|slot| slot.replace(ptr::null_mut()));
    if !caller.is_null() {
        unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut())?; }
    }
    Ok(())
}
unsafe fn pre_observed(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    PRE_COUNT.with(|count| count.set(count.get() + 1));
    Ok(if PRE_CONSUME.with(Cell::get) { hp2_core::ScriptPreCallDecision::Consume } else { hp2_core::ScriptPreCallDecision::Continue })
}
unsafe fn scope_observed(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize, phase: hp2_core::ScriptScopePhase) -> RuntimeResult<()> {
    SCOPES.with(|count| count.set(count.get() | match phase { hp2_core::ScriptScopePhase::Enter => 1, hp2_core::ScriptScopePhase::Exit { succeeded: true } => 2, hp2_core::ScriptScopePhase::Exit { succeeded: false } => 4 }));
    Ok(())
}
unsafe fn select_resolved(_: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction) -> RuntimeResult<bool> {
    let expected = EXPECTED_RECEIVER.with(Cell::get);
    if !expected.is_null() { assert_eq!(receiver, expected); }
    if SELECT_FAIL.with(Cell::get) || FAIL_FUNCTION.with(Cell::get) == function { return Err(hp2_core::core_access::RuntimeError::new("selector failed")); }
    Ok(SKIP_FUNCTION.with(Cell::get) != function)
}
unsafe fn reject_resolved(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction) -> RuntimeResult<bool> {
    Err(hp2_core::core_access::RuntimeError::new("second selector failed"))
}
unsafe fn resolved_observed(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, _: *const u8, _: usize, phase: hp2_core::ScriptScopePhase) -> RuntimeResult<()> {
    RESOLVED_EVENTS.with(|events| events.borrow_mut().push((function, phase)));
    if phase == hp2_core::ScriptScopePhase::Enter && ENTER_FAIL.with(Cell::get) {
        return Err(hp2_core::core_access::RuntimeError::new("enter failed"));
    }
    if matches!(phase, hp2_core::ScriptScopePhase::Exit { .. }) && EXIT_FAIL.with(Cell::get) {
        return Err(hp2_core::core_access::RuntimeError::new("exit failed"));
    }
    Ok(())
}
unsafe fn extra_observed(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    OBSERVER_COUNT.with(|count| count.set(count.get() + 1));
    Ok(())
}
unsafe fn function(access: CoreAccess<'_>, class: *mut hp2_abi::engine::generated::UClass, name: &str, code: &mut Vec<u8>) -> *mut UFunction {
    unsafe { function_in(access, class.cast(), name, code) }
}
unsafe fn function_in(access: CoreAccess<'_>, outer: *mut UObject, name: &str, code: &mut Vec<u8>) -> *mut UFunction {
    let f = unsafe { access.construct_object(access.native_class("Function").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UFunction>();
    unsafe {
        (*f).function_flags = 0;
        (*f).parms_size = 0;
        (*f).return_value_offset = u16::MAX;
        (*f.cast::<UStruct>()).properties_size = 0;
        (*f.cast::<UStruct>()).properties_align = 4;
        (*f.cast::<UStruct>()).script = TArrayRaw { data: code.as_mut_ptr().cast(), num: code.len() as i32, max: code.len() as i32 };
    }
    f
}
#[test]
fn real_vm_post_call_is_filtered_reentrant_and_scoped() {
    stage("fixture-start");
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("script-hook-{}", std::process::id()));
    assert!(!root.exists());
    fs::create_dir_all(root.join("System")).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    fs::copy(source.join("Core.u"), root.join("System/Core.u")).unwrap();
    fs::copy(source.join("Default.ini"), root.join("System/System.ini")).unwrap();
    fs::copy(source.join("DefUser.ini"), root.join("System/User.ini")).unwrap();
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"), user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let class = access.native_class("Object").unwrap();
    let text: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &text, 1).unwrap(); }
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let mut callee_code = vec![0x04, 0x0b]; // Return Nothing.
    let mut other_code = vec![0x04, 0x0b];
    let callee = unsafe { function(access, class, "HookCallee", &mut callee_code) };
    let other = unsafe { function(access, class, "HookOther", &mut other_code) };
    let mut caller_code = vec![0x1c]; // FinalFunction.
    caller_code.extend(unsafe { (*callee.cast::<UObject>()).index }.to_le_bytes());
    caller_code.extend([0x16, 0x04, 0x0b]);
    let caller = unsafe { function(access, class, "HookCaller", &mut caller_code) };
    let mut unmatched_code = vec![0x04, 0x0b];
    let unmatched = unsafe { function(access, class, "HookUnmatched", &mut unmatched_code) };
    let mut unmatched_caller_code = vec![0x1c];
    unmatched_caller_code.extend(unsafe { (*unmatched.cast::<UObject>()).index }.to_le_bytes());
    unmatched_caller_code.extend([0x16, 0x04, 0x0b]);
    let unmatched_caller = unsafe { function(access, class, "HookUnmatchedCaller", &mut unmatched_caller_code) };
    let run = || unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()).unwrap() };
    stage("baseline-hooks-start");
    let guard = unsafe { access.install_script_post_call_hook([callee, other], observed).unwrap() };
    let extra = unsafe { access.install_script_observer("test-extra", &[callee], extra_observed).unwrap() };
    let pre_coexist = unsafe { access.install_script_pre_observer("coexist-pre", &[callee], pre_observed).unwrap() };
    assert!(unsafe { access.install_script_observer("test-extra", &[callee], extra_observed) }.is_err());
    assert!(unsafe { access.install_script_post_call_hook([callee, other], observed) }.is_err());
    unsafe { access.execute_event(object, unmatched_caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(COUNT.with(Cell::get), 0, "other script functions are not observed");
    REENTER.with(|slot| slot.set(caller));
    run();
    assert_eq!(COUNT.with(Cell::get), 1, "recursive call is suppressed");
    run();
    assert_eq!(COUNT.with(Cell::get), 2, "guard resets after reentry");
    assert!(OBSERVER_COUNT.with(Cell::get) >= 2, "observer coexists with the FPS-style hook");
    assert!(PRE_COUNT.with(Cell::get) >= 2, "pre hook coexists with both post hooks");
    drop(pre_coexist);
    PRE_COUNT.with(|count| count.set(0));
    let previous = OBSERVER_COUNT.with(Cell::get);
    unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(OBSERVER_COUNT.with(Cell::get), previous + 1, "observer receives a root event");
    drop(extra);
    drop(guard);
    run();
    assert_eq!(OBSERVER_COUNT.with(Cell::get), previous + 1, "observer guard detaches");
    assert_eq!(COUNT.with(Cell::get), 2, "dropping guard detaches hook");
    let pre = unsafe { access.install_script_pre_observer("test-pre", &[callee], pre_observed).unwrap() };
    assert!(unsafe { access.install_script_pre_observer("test-pre", &[callee], pre_observed) }.is_err());
    let post = unsafe { access.install_script_observer("test-post", &[callee], extra_observed).unwrap() };
    let before = OBSERVER_COUNT.with(Cell::get);
    run();
    assert_eq!(PRE_COUNT.with(Cell::get), 1, "pre hook sees a nested script call");
    assert_eq!(OBSERVER_COUNT.with(Cell::get), before + 1, "continuing reaches post hook");
    PRE_CONSUME.with(|slot| slot.set(true));
    run();
    unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(PRE_COUNT.with(Cell::get), 3, "pre hook sees nested and root calls");
    assert_eq!(OBSERVER_COUNT.with(Cell::get), before + 1, "consumed calls do not reach post hooks");
    drop(pre);
    run();
    assert_eq!(PRE_COUNT.with(Cell::get), 3, "pre guard detaches");
    assert_eq!(OBSERVER_COUNT.with(Cell::get), before + 2, "post hook remains installed");
    drop(post);
    let mut bad_code = vec![0xff];
    let bad = unsafe { function(access, class, "HookBad", &mut bad_code) };
    let scopes = unsafe { access.install_script_scope_observer("test-scope", &[callee, bad], scope_observed).unwrap() };
    run();
    assert_eq!(SCOPES.with(Cell::get), 3, "scope wraps a successful bytecode call");
    assert!(unsafe { access.execute_event(object, bad, ptr::null_mut(), ptr::null_mut()) }.is_err());
    assert_eq!(SCOPES.with(Cell::get), 7, "scope exits on script error");
    drop(scopes);
    stage("resolved-hooks-start");
    let resolved = unsafe { access.install_resolved_script_scope_observer("test-resolved", select_resolved, resolved_observed).unwrap() };
    assert!(unsafe { access.install_resolved_script_scope_observer("test-resolved", select_resolved, resolved_observed) }.is_err());
    run();
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (caller, hp2_core::ScriptScopePhase::Enter), (callee, hp2_core::ScriptScopePhase::Enter),
        (callee, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ]));
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    let skipped_post = unsafe { access.install_script_observer("skipped-post", &[callee], extra_observed).unwrap() };
    let before_skipped = OBSERVER_COUNT.with(Cell::get);
    SKIP_FUNCTION.with(|value| value.set(callee));
    run();
    SKIP_FUNCTION.with(|value| value.set(ptr::null_mut()));
    assert_eq!(OBSERVER_COUNT.with(Cell::get), before_skipped + 1, "unselected callee executes");
    drop(skipped_post);
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![(caller, hp2_core::ScriptScopePhase::Enter), (caller, hp2_core::ScriptScopePhase::Exit { succeeded: true })], "unselected nested body still executes"));
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    assert!(unsafe { access.execute_event(object, bad, ptr::null_mut(), ptr::null_mut()) }.is_err());
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![(bad, hp2_core::ScriptScopePhase::Enter), (bad, hp2_core::ScriptScopePhase::Exit { succeeded: false })]));
    SELECT_FAIL.with(|value| value.set(true));
    assert!(unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()) }.unwrap_err().to_string().contains("selector failed"));
    SELECT_FAIL.with(|value| value.set(false));
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    FAIL_FUNCTION.with(|value| value.set(callee));
    assert!(unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()) }.is_err());
    FAIL_FUNCTION.with(|value| value.set(ptr::null_mut()));
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![(caller, hp2_core::ScriptScopePhase::Enter), (caller, hp2_core::ScriptScopePhase::Exit { succeeded: false })]));
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    stage("resolved-reentry-start");
    let reenter_hook = unsafe { access.install_script_post_call_hook([callee, callee], observed).unwrap() };
    REENTER.with(|slot| slot.set(caller));
    run();
    drop(reenter_hook);
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (caller, hp2_core::ScriptScopePhase::Enter), (callee, hp2_core::ScriptScopePhase::Enter),
        (callee, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
        (caller, hp2_core::ScriptScopePhase::Enter), (callee, hp2_core::ScriptScopePhase::Enter),
        (callee, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
        (caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ]));
    stage("error-cases-start");
    ENTER_FAIL.with(|value| value.set(true));
    let error = unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
    assert_eq!(error.diagnostic(), "enter failed".chars().map(u32::from).collect::<Vec<_>>());
    ENTER_FAIL.with(|value| value.set(false));
    let body_error = unsafe { access.execute_event(object, bad, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
    EXIT_FAIL.with(|value| value.set(true));
    let error = unsafe { access.execute_event(object, bad, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
    assert_eq!(error.diagnostic(), body_error.diagnostic(), "script failure remains primary");
    let exit_failed: Vec<u32> = "exit failed".chars().map(u32::from).collect();
    assert!(error.context().iter().any(|part| part.windows(exit_failed.len()).any(|window| window == exit_failed)), "{error}");
    let second = unsafe { access.install_resolved_script_scope_observer("reject-resolved", reject_resolved, resolved_observed).unwrap() };
    let error = unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
    assert_eq!(error.diagnostic(), "second selector failed".chars().map(u32::from).collect::<Vec<_>>());
    assert!(error.context().iter().any(|part| part.windows(exit_failed.len()).any(|window| window == exit_failed)), "{error}");
    drop(second);
    EXIT_FAIL.with(|value| value.set(false));

    // Real virtual VM dispatch must select the subclass and then active-state
    // overrides, rather than a fixed pointer for the base function.
    stage("virtual-fixture-start");
    let subclass = access.native_class("Core.TextBuffer").unwrap();
    assert!(unsafe { hp2_core::object_model::is_a(subclass.cast(), access.native_class("Core.Class").unwrap().cast()) });
    let receiver = unsafe { access.construct_object(subclass, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    assert!(unsafe { hp2_core::object_model::is_a(receiver.cast(), class.cast()) });
    let state = unsafe { access.construct_object(access.native_class("Core.State").unwrap(), subclass.cast(), access.intern_str("ScopeTestState", FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UState>();
    let draw_name = access.intern_str("ScopeDrawItem", FindName::Add).unwrap();
    let mut base_code = vec![0x04, 0x0b];
    let mut subclass_code = vec![0x04, 0x0b];
    let mut state_code = vec![0x04, 0x0b];
    let base_draw = unsafe { function(access, class, "ScopeDrawItem", &mut base_code) };
    let subclass_draw = unsafe { function(access, subclass, "ScopeDrawItem", &mut subclass_code) };
    let state_draw = unsafe { function_in(access, state.cast(), "ScopeDrawItem", &mut state_code) };
    let base_children = unsafe { (*class.cast::<UStruct>()).children };
    let subclass_children = unsafe { (*subclass.cast::<UStruct>()).children };
    unsafe {
        (*base_draw.cast::<UField>()).next = base_children;
        (*class.cast::<UStruct>()).children = base_draw.cast();
        (*subclass_draw.cast::<UField>()).next = subclass_children;
        (*subclass.cast::<UStruct>()).children = subclass_draw.cast();
        (*state.cast::<UStruct>()).children = state_draw.cast();
        (*state).probe_mask = u64::MAX;
        (*state).ignore_mask = u64::MAX;
        access.initialize_execution(receiver).unwrap();
    }
    let mut virtual_code = vec![0x1b]; // EX_VirtualFunction, name, EndFunctionParms.
    virtual_code.extend(draw_name.to_le_bytes());
    virtual_code.extend([0x16, 0x04, 0x0b]);
    let virtual_caller = unsafe { function(access, subclass, "ScopeVirtualCaller", &mut virtual_code) };
    let fixed = unsafe { access.install_script_scope_observer("fixed-subclass", &[subclass_draw], scope_observed).unwrap() };
    SCOPES.with(|value| value.set(0));
    EXPECTED_RECEIVER.with(|value| value.set(receiver));
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    stage("subclass-dispatch-start");
    unsafe { access.execute_event(receiver, virtual_caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (virtual_caller, hp2_core::ScriptScopePhase::Enter), (subclass_draw, hp2_core::ScriptScopePhase::Enter),
        (subclass_draw, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (virtual_caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ], "virtual dispatch selects the subclass body"));
    assert_eq!(SCOPES.with(Cell::get), 3, "fixed selector still matches its exact function");
    SCOPES.with(|value| value.set(0));
    let frame = unsafe { (*receiver).state_frame.cast::<FStateFrame>() };
    assert!(!frame.is_null());
    unsafe { (*frame).state_node = state.cast(); }
    stage("state-dispatch-start");
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    unsafe { access.execute_event(receiver, virtual_caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (virtual_caller, hp2_core::ScriptScopePhase::Enter), (state_draw, hp2_core::ScriptScopePhase::Enter),
        (state_draw, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (virtual_caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ], "virtual dispatch selects the active-state body"));
    assert_eq!(SCOPES.with(Cell::get), 0, "fixed selector does not match an active-state override");
    FINAL_TARGET.with(|slot| slot.set(base_draw));
    let final_binding = unsafe { access.install_script_body_replacement("final-target", &[virtual_caller], final_body).unwrap() };
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    unsafe { access.execute_event(receiver, virtual_caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (virtual_caller, hp2_core::ScriptScopePhase::Enter), (base_draw, hp2_core::ScriptScopePhase::Enter),
        (base_draw, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (virtual_caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ], "an exact final target bypasses both same-name subclass and state overrides"));
    drop(final_binding);
    unsafe { (*frame).state_node = subclass.cast(); }
    FINAL_TARGET.with(|slot| slot.set(subclass_draw));
    CALLBACK_STATE.with(|slot| slot.set(state));
    let transition = unsafe { access.install_script_post_call_hook([subclass_draw, subclass_draw], change_callback_state).unwrap() };
    let reentry_binding = unsafe { access.install_script_body_replacement("state-reentry", &[virtual_caller], state_reentry_body).unwrap() };
    RESOLVED_EVENTS.with(|events| events.borrow_mut().clear());
    unsafe { access.execute_event(receiver, virtual_caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    RESOLVED_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (virtual_caller, hp2_core::ScriptScopePhase::Enter), (subclass_draw, hp2_core::ScriptScopePhase::Enter),
        (subclass_draw, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (state_draw, hp2_core::ScriptScopePhase::Enter),
        (state_draw, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (virtual_caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ], "post-callback state changes affect the next lookup inside the replacement"));
    drop(reentry_binding);
    drop(transition);
    CALLBACK_STATE.with(|slot| slot.set(ptr::null_mut()));
    FINAL_TARGET.with(|slot| slot.set(ptr::null_mut()));
    drop(fixed);
    EXPECTED_RECEIVER.with(|value| value.set(ptr::null_mut()));
    unsafe {
        (*frame).state_node = subclass.cast();
        (*class.cast::<UStruct>()).children = base_children;
        (*subclass.cast::<UStruct>()).children = subclass_children;
        (*state.cast::<UStruct>()).children = ptr::null_mut();
        for f in [base_draw, subclass_draw, state_draw, virtual_caller] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    }
    // Rooted selector-GC coverage is in core_runtime::rooted_selector_gc_scope.
    // The original unoptimized GC overflow remains documented evidence.
    drop(resolved);
    stage("rust-body-replacement-start");
    BODY_CHILD.with(|slot| slot.set(other));
    let mut native_code = vec![0x04, 0x0b];
    let native_target = unsafe { function(access, class, "BodyNativeTarget", &mut native_code) };
    unsafe { (*native_target).function_flags = 0x400; }
    BODY_NATIVE.with(|slot| slot.set(native_target));
    assert!(unsafe { access.install_script_body_replacement("native-body", &[native_target], replacement_body) }.is_err());
    // Invalid original code proves that a replacement, rather than bytecode,
    // executes. Restoring the binding must expose the original failure again.
    callee_code[0] = 0xff;
    let body = unsafe { access.install_script_body_replacement("test-body", &[callee, other], replacement_body).unwrap() };
    assert!(unsafe { access.install_script_body_replacement("test-body", &[bad], replacement_body) }.is_err());
    assert!(unsafe { access.install_script_body_replacement("overlap-body", &[callee], replacement_body) }.is_err());
    assert!(unsafe { access.install_script_body_replacement("empty-body", &[], replacement_body) }.is_err());
    let body_pre = unsafe { access.install_script_pre_observer("body-pre", &[callee, other], replacement_pre).unwrap() };
    let body_scope = unsafe { access.install_script_scope_observer("body-scope", &[callee, other], replacement_scope).unwrap() };
    let body_post = unsafe { access.install_script_post_call_hook([callee, other], replacement_post).unwrap() };
    BODY_EVENTS.with(|events| events.borrow_mut().clear());
    run();
    let successful_body = vec!["pre-parent", "enter-parent", "body-parent", "pre-child", "enter-child", "body-child", "exit-child-ok", "post-child", "resume-parent", "exit-parent-ok", "post-parent"];
    BODY_EVENTS.with(|events| assert_eq!(*events.borrow(), successful_body, "replacement and nested callback retain canonical ordering"));
    BODY_EVENTS.with(|events| events.borrow_mut().clear());
    BODY_FAIL.with(|slot| slot.set(true));
    let error = unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
    assert_eq!(error.diagnostic(), "replacement body failed".chars().map(u32::from).collect::<Vec<_>>());
    BODY_EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre-parent", "enter-parent", "body-parent", "pre-child", "enter-child", "body-child", "exit-child-ok", "post-child", "resume-parent", "exit-parent-error"], "failed replacement keeps completed child side effects but skips parent post-call hook"));
    BODY_FAIL.with(|slot| slot.set(false));
    BODY_CONSUME.with(|slot| slot.set(true));
    BODY_EVENTS.with(|events| events.borrow_mut().clear());
    run();
    BODY_EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre-parent"], "consuming pre-observer takes precedence over replacement"));
    BODY_CONSUME.with(|slot| slot.set(false));
    BODY_FALLBACK.with(|slot| slot.set(true));
    BODY_EVENTS.with(|events| events.borrow_mut().clear());
    assert!(unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()) }.is_err(), "preflight fallback executes the original invalid body");
    BODY_EVENTS.with(|events| assert_eq!(*events.borrow(), vec!["pre-parent", "enter-parent", "exit-parent-error"]));
    BODY_FALLBACK.with(|slot| slot.set(false));
    BODY_EVENTS.with(|events| events.borrow_mut().clear());
    unsafe { access.execute_event(object, callee, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    BODY_EVENTS.with(|events| assert_eq!(*events.borrow(), successful_body[..successful_body.len() - 1], "root event still has no script post-call hook, but its nested callback does"));
    drop(body_pre);
    drop(body_scope);
    drop(body_post);
    drop(body);
    assert!(unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()) }.is_err(), "dropping replacement restores original invalid bytecode");
    callee_code[0] = 0x04;
    run();
    BODY_NATIVE.with(|slot| slot.set(ptr::null_mut()));
    unsafe { (*native_target.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    stage("rust-body-replacement-complete");
    unsafe {
        for f in [callee, other, caller, unmatched, unmatched_caller, bad] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    }
    stage("shutdown-start");
    owner.shutdown().unwrap();
    stage("complete");
}

fn warning_output(_: CoreAccess<'_>, _: i32, message: &[u32]) -> RuntimeResult<()> {
    let text: String = message.iter().filter_map(|&unit| char::from_u32(unit)).collect();
    if text.contains("Accessed None: BodyNullProp") {
        FRAME_WARNINGS.with(|warnings| warnings.borrow_mut().push(text));
    }
    Ok(())
}

unsafe fn warning_body(access: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, frame: *mut hp2_abi::engine::generated::FFrame) -> RuntimeResult<hp2_core::ScriptPreCallDecision> {
    // SAFETY: these are live canonical records borrowed by the replacement.
    let script = unsafe { (*function.cast::<UStruct>()).script };
    assert_eq!(script.num, 30);
    for offset in [0, 14] {
        // These are runtime bytecode positions, not compact archive offsets.
        // Both expressions are EX_Context(InstanceVariable), skip=5, width=4.
        // SAFETY: the fixture owns the complete immutable 30-byte script and
        // the canonical frame/locals remain live during this synchronous step.
        unsafe { (*frame).code = script.data.cast::<u8>().add(offset); }
        let mut value = -1_i32;
        unsafe { access.execute_script_frame_step(frame, (&raw mut value).cast())?; }
        assert_eq!(value, 0, "a null context zeroes the result");
    }
    Ok(hp2_core::ScriptPreCallDecision::Consume)
}

#[test]
fn replacement_preserves_null_context_positions_and_strict_failure() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for strict in [false, true] {
        let root = artifact.join(format!("body-warnings-{}-{strict}", std::process::id()));
        assert!(!root.exists());
        fs::create_dir_all(root.join("System")).unwrap();
        for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
            fs::copy(source.join(from), root.join("System").join(to)).unwrap();
        }
        let owner = CoreOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/System.ini"),
            user_ini: root.join("System/User.ini"),
            command_line: if strict { "-STRICT".chars().map(u32::from).collect() } else { Vec::new() },
            output: warning_output, log_file: None,
        }).unwrap();
        let access = owner.access();
        access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
        let concrete_name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
        // Load a concrete class, rather than instantiating the abstract native
        // Object bootstrap descriptor before its package metadata is loaded.
        let concrete = unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &concrete_name, 1).unwrap() };
        let class = access.native_class("Object").unwrap();
        // SAFETY: all fixture records remain owned by this Core owner until
        // after both VM runs and binding teardown.
        let object = unsafe { access.construct_object(concrete, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
        let property = unsafe { access.construct_object(access.native_class("ObjectProperty").unwrap(), class.cast(), access.intern_str("BodyNullProp", FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<hp2_abi::engine::generated::UProperty>();
        let outer = unsafe { (*object).outer };
        unsafe {
            (*object).outer = ptr::null_mut();
            (*property).array_dim = 1;
            (*property).element_size = size_of::<*mut UObject>() as i32;
            (*property).offset = i32::try_from(std::mem::offset_of!(UObject, outer)).unwrap();
            (*property.cast::<hp2_abi::engine::generated::UObjectProperty>()).property_class = class.cast();
        }
        let mut expression = vec![0x19, 0x01];
        expression.extend(unsafe { (*property.cast::<UObject>()).index }.to_le_bytes());
        expression.extend([5, 0, 4, 0x01]);
        expression.extend(unsafe { (*property.cast::<UObject>()).index }.to_le_bytes());
        assert_eq!(expression.len(), 14);
        let mut code = expression.repeat(2);
        code.extend([0x04, 0x0b]);
        let target = unsafe { function(access, class, "BodyWarningTarget", &mut code) };
        FRAME_WARNINGS.with(|warnings| warnings.borrow_mut().clear());
        let baseline = unsafe { access.execute_event(object, target, ptr::null_mut(), ptr::null_mut()) };
        let expected = FRAME_WARNINGS.with(|warnings| warnings.borrow().clone());
        if strict {
            assert!(baseline.as_ref().unwrap_err().to_string().contains(":0006)"));
            assert!(expected.is_empty(), "strict mode fails before logging or the second expression");
        } else {
            baseline.as_ref().unwrap();
            assert_eq!(expected.len(), 2);
            assert!(expected[0].contains(":0006)"), "{}", expected[0]);
            assert!(expected[1].contains(":0014)"), "{}", expected[1]);
        }
        FRAME_WARNINGS.with(|warnings| warnings.borrow_mut().clear());
        let replacement = unsafe { access.install_script_body_replacement("body-warning", &[target], warning_body).unwrap() };
        let actual = unsafe { access.execute_event(object, target, ptr::null_mut(), ptr::null_mut()) };
        assert_eq!(FRAME_WARNINGS.with(|warnings| warnings.borrow().clone()), expected);
        if strict {
            assert_eq!(actual.unwrap_err().diagnostic(), baseline.unwrap_err().diagnostic(), "strict replacement preserves the first failing context exactly");
        } else {
            actual.unwrap();
        }
        drop(replacement);
        unsafe {
            (*object).outer = outer;
            (*target.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        }
        owner.shutdown().unwrap();
    }
}
