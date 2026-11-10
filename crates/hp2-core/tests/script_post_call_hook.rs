//! Real VM calls exercise the optional scoped post-call hook.
use hp2_abi::engine::generated::{TArrayRaw, UFunction, UObject, UStruct};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{cell::Cell, fs, path::PathBuf, ptr};

thread_local! {
    static COUNT: Cell<u32> = const { Cell::new(0) };
    static OBSERVER_COUNT: Cell<u32> = const { Cell::new(0) };
    static PRE_COUNT: Cell<u32> = const { Cell::new(0) };
    static PRE_CONSUME: Cell<bool> = const { Cell::new(false) };
    static SCOPES: Cell<u32> = const { Cell::new(0) };
    static REENTER: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
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
unsafe fn extra_observed(_: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    OBSERVER_COUNT.with(|count| count.set(count.get() + 1));
    Ok(())
}
unsafe fn function(access: CoreAccess<'_>, class: *mut hp2_abi::engine::generated::UClass, name: &str, code: &mut Vec<u8>) -> *mut UFunction {
    let f = unsafe { access.construct_object(access.native_class("Function").unwrap(), class.cast(), access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UFunction>();
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
    unsafe {
        for f in [callee, other, caller, unmatched, unmatched_caller, bad] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    }
    owner.shutdown().unwrap();
}
