//! Real interpreted LET/result-alias regression for the health callback boundary.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{core_access::RuntimeError, ScriptPreCallDecision};

const RETURN_VALUE: i32 = 0x1122_3344;
const INITIAL: i32 = -31;
thread_local! {
    static CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static PARENT: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static PROBE: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static CHILD_POSTS: Cell<u32> = const { Cell::new(0) };
    static PARENT_POSTS: Cell<u32> = const { Cell::new(0) };
    static REPLACEMENTS: Cell<u32> = const { Cell::new(0) };
}

/// The caller keeps the owned TextBuffer, functions and frame live across reentry.
unsafe fn replacement(access: CoreAccess<'_>, object: *mut UObject, _: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    REPLACEMENTS.with(|count| count.set(count.get() + 1));
    // `top` is an actual i32 data member, not a UObject identity field. This
    // is exactly the destination used by the original EX_Let instance lvalue.
    let destination = unsafe { std::ptr::addr_of_mut!((*object.cast::<UTextBuffer>()).top) };
    let mut return_parameter = 0_i32;
    // SAFETY: the function owns one four-byte reflected return parameter. The
    // result pointer and parent frame remain live during synchronous callback.
    unsafe { access.execute_script_callback(frame, object, CHILD.with(Cell::get), (&raw mut return_parameter).cast(), destination.cast())?; }
    Ok(ScriptPreCallDecision::Consume)
}

/// The installed hook is synchronous and the fixture retains all its objects.
unsafe fn post(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    if function == PARENT.with(Cell::get) {
        PARENT_POSTS.with(|count| count.set(count.get() + 1));
        return Ok(());
    }
    assert_eq!(function, CHILD.with(Cell::get));
    CHILD_POSTS.with(|count| count.set(count.get() + 1));
    // SAFETY: the fixture constructed this receiver as an owned UTextBuffer.
    let buffer = object.cast::<UTextBuffer>();
    assert_eq!(unsafe { (*buffer).top }, RETURN_VALUE, "return writes the canonical destination before the post hook");
    assert_eq!(unsafe { (*buffer).pos }, INITIAL, "the adjacent data member is not overwritten by the four-byte return");
    // A real interpreted reentrant event reads the destination through the
    // original reflected property and writes it into the other data member.
    unsafe { access.execute_event(object, PROBE.with(Cell::get), ptr::null_mut(), ptr::null_mut())?; }
    assert_eq!(unsafe { (*buffer).pos }, RETURN_VALUE, "reentrant VM code observes the return write");
    Err(RuntimeError::new("health fixture post failure"))
}

/// The caller owns the Core owner and supplies a live outer record.
unsafe fn int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    unsafe {
        (*property).array_dim = 1;
        (*property).element_size = 4;
        (*property).offset = i32::try_from(offset).unwrap();
        (*property).property_flags = flags;
    }
    property
}

fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: fixture-owned indexed records remain live for code generation.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}

#[test]
fn interpreted_let_and_replacement_keep_return_destination_on_post_failure() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("health-vm-alias-{}", std::process::id()));
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
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let buffer = object.cast::<UTextBuffer>();
    let top = unsafe { int_property(access, class.cast(), "HealthAliasTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let pos = unsafe { int_property(access, class.cast(), "HealthAliasPos", std::mem::offset_of!(UTextBuffer, pos), 0) };

    let mut child_code = vec![0x04, 0x1d]; // Return IntConst.
    child_code.extend(RETURN_VALUE.to_le_bytes());
    let child = unsafe { function(access, class, "HealthAliasReturn", &mut child_code) };
    let return_property = unsafe { int_property(access, child.cast(), "ReturnValue", 0, 0x480) }; // Parm | ReturnParm.
    unsafe {
        (*child).num_parms = 1;
        (*child).parms_size = 4;
        (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = 4;
        (*child.cast::<UStruct>()).children = return_property.cast();
    }
    let mut parent_code = vec![0x0f]; // Let InstanceVariable(top) = FinalFunction(child).
    operand(&mut parent_code, 0x01, top.cast());
    operand(&mut parent_code, 0x1c, child.cast());
    parent_code.push(0x16);
    parent_code.push(0x0f); // Reached only after the callback succeeds.
    operand(&mut parent_code, 0x01, pos.cast());
    parent_code.extend([0x2c, 77, 0x04, 0x0b]);
    let parent = unsafe { function(access, class, "HealthAliasParent", &mut parent_code) };
    let mut root_code = Vec::new();
    operand(&mut root_code, 0x1c, parent.cast());
    root_code.extend([0x16, 0x04, 0x0b]);
    let root_function = unsafe { function(access, class, "HealthAliasRoot", &mut root_code) };
    let mut probe_code = vec![0x0f];
    operand(&mut probe_code, 0x01, pos.cast());
    operand(&mut probe_code, 0x01, top.cast());
    probe_code.extend([0x04, 0x0b]);
    let probe = unsafe { function(access, class, "HealthAliasProbe", &mut probe_code) };
    CHILD.with(|slot| slot.set(child)); PARENT.with(|slot| slot.set(parent)); PROBE.with(|slot| slot.set(probe));
    let observer = unsafe { access.install_script_post_call_hook([child, parent], post).unwrap() };

    let run = || {
        unsafe { (*buffer).top = INITIAL; (*buffer).pos = INITIAL; }
        CHILD_POSTS.with(|count| count.set(0)); PARENT_POSTS.with(|count| count.set(0));
        let error = unsafe { access.execute_event(object, root_function, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
        assert_eq!(error.diagnostic(), "health fixture post failure".chars().map(u32::from).collect::<Vec<_>>());
        assert_eq!(CHILD_POSTS.with(Cell::get), 1);
        assert_eq!(PARENT_POSTS.with(Cell::get), 0, "the failed child prevents parent post and stock continuation");
        assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (RETURN_VALUE, RETURN_VALUE));
    };
    run();
    let binding = unsafe { access.install_script_body_replacement("health-alias-parent", &[parent], replacement).unwrap() };
    run();
    assert_eq!(REPLACEMENTS.with(Cell::get), 1, "the replacement ran once without stock replay");
    drop(binding); drop(observer);
    CHILD.with(|slot| slot.set(ptr::null_mut())); PARENT.with(|slot| slot.set(ptr::null_mut())); PROBE.with(|slot| slot.set(ptr::null_mut()));
    unsafe {
        for f in [child, parent, root_function, probe] { (*f.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    }
    owner.shutdown().unwrap();
}
