//! Real active-body identity and external descriptor address comparisons.
use super::*;
use hp2_abi::engine::generated::{FFrame, UProperty, UTextBuffer};
use hp2_core::{
    canonical_vm_native::VmNativeSpec,
    vm_owner::StepControl,
    ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision,
};
use std::ffi::c_void;

const FIRST_INDEX: u16 = 4000;
const SECOND_INDEX: u16 = 4001;
thread_local! {
    static OUTER: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static INNER: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static OTHER_OBJECT: Cell<*mut UObject> = const { Cell::new(ptr::null_mut()) };
    static OTHER_OWNER: Cell<*const CoreOwner> = const { Cell::new(ptr::null()) };
    static RHS_OFFSET: Cell<usize> = const { Cell::new(0) };
    static INNER_SEEN: Cell<u32> = const { Cell::new(0) };
    static MASKED_SEEN: Cell<u32> = const { Cell::new(0) };
    static INVOKED: Cell<u32> = const { Cell::new(0) };
}

/// # Safety
/// Never invoked by these query-only fixtures.
unsafe fn inert(_: CoreAccess<'_>, _: *mut UObject, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
    INVOKED.with(|slot| slot.set(slot.get() + 1));
    Ok(())
}
const fn spec(name: &'static str, index: u16) -> VmNativeSpec {
    VmNativeSpec { class_name: "IdentityFixture", function_name: name, native_index: Some(index), handler: inert }
}
// Both duplicate groups have an earlier and a later entry. The second group
// reverses the distinguishing names, so winning is about position, not name.
static CATALOG: [VmNativeSpec; 5] = [
    spec("A", FIRST_INDEX), spec("A", FIRST_INDEX),
    spec("B", SECOND_INDEX), spec("A", SECOND_INDEX), spec("CoreShadow", 15),
];
static COPY: VmNativeSpec = spec("A", FIRST_INDEX);
static OTHER_AGGREGATE: [VmNativeSpec; 1] = [spec("A", FIRST_INDEX)];

/// The frame/function are borrowed only while the current callback is active.
unsafe fn matches(access: CoreAccess<'_>, frame: *mut FFrame, object: *mut UObject, function: *mut UFunction) -> bool {
    // SAFETY: the callback receives the current function with immutable script.
    let script = unsafe { (*function.cast::<UStruct>()).script };
    access.current_script_body_matches(frame, object, function, script.data.cast(), script.num as usize)
}

/// # Safety
/// The Let owner invokes this with its valid live RHS, frame and i32 destination.
unsafe fn masked_rhs(access: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame, context: *mut c_void, destination: *mut c_void) -> RuntimeResult<()> {
    // SAFETY: this callback synchronously borrows the outer frame and its node.
    let (object, function) = unsafe { ((*frame).object.cast::<UObject>(), (*frame).node.cast::<UFunction>()) };
    assert_eq!(object.cast::<c_void>(), context);
    assert!(!unsafe { matches(access, frame, object, function) }, "the outer active scope is masked during RHS evaluation");
    MASKED_SEEN.with(|slot| slot.set(slot.get() + 1));
    // SAFETY: the immutable fixture RHS is a complete i32-returning Final call;
    // the actual Let destination and receiver remain live through this call.
    unsafe { args.step_argument(frame, destination)?; }
    assert!(!unsafe { matches(access, frame, object, function) }, "the outer scope stays masked until its operation ends");
    Ok(())
}

/// # Safety
/// Called only for the fixture's live interpreted replacement frames.
unsafe fn body(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    // SAFETY: the canonical VM owns the frame/function/immutable script here.
    let script = unsafe { (*function.cast::<UStruct>()).script };
    let base = script.data.cast::<u8>();
    let length = script.num as usize;
    assert!(access.current_script_body_matches(frame, object, function, base, length));
    assert!(!access.current_script_body_matches(ptr::null_mut(), object, function, base, length));
    assert!(!access.current_script_body_matches(frame, ptr::null_mut(), function, base, length));
    assert!(!access.current_script_body_matches(frame, object, ptr::null_mut(), base, length));
    assert!(!access.current_script_body_matches(frame, object, function, ptr::null(), length));
    assert!(!access.current_script_body_matches(frame, object, function, base, length + 1));
    // SAFETY: both fixture scripts are at least 2 bytes long, so this is a
    // distinct in-allocation pointer; the query does not dereference it.
    assert!(!access.current_script_body_matches(frame, object, function, unsafe { base.add(1) }, length));
    assert!(!access.current_script_body_matches(frame, OTHER_OBJECT.with(Cell::get), function, base, length));
    let other_function = if function == OUTER.with(Cell::get) { INNER.with(Cell::get) } else { OUTER.with(Cell::get) };
    assert!(!access.current_script_body_matches(frame, object, other_function, base, length));
    let other_owner = OTHER_OWNER.with(Cell::get);
    assert!(!other_owner.is_null());
    // SAFETY: the test owns the unmoved second CoreOwner until all callbacks
    // have returned. This compares identity and does not execute its frame.
    assert!(!unsafe { (&*other_owner).access() }.current_script_body_matches(frame, object, function, base, length));

    let mut statements = ScriptBodyStatements::new();
    if function == INNER.with(Cell::get) {
        INNER_SEEN.with(|slot| slot.set(slot.get() + 1));
        // SAFETY: the inner frame's parent is the still-live outer frame. Its
        // pointer is used only for comparison, never to execute suspended work.
        let parent = unsafe { (*frame).parent };
        assert!(!parent.is_null());
        assert!(!access.current_script_body_matches(parent, object, function, base, length));
        // The original Return writes into the effective child i32 destination.
        assert!(unsafe { statements.begin(access, frame)? }.is_none());
        return Ok(ScriptPreCallDecision::Consume);
    }
    assert_eq!(function, OUTER.with(Cell::get));
    // SAFETY: begin guards this live original Let exactly once.
    assert_eq!(unsafe { statements.begin(access, frame)? }, Some(base));
    // SAFETY: the fixture binds the immutable Let/RHS offsets and live i32 LHS.
    let control = unsafe { access.execute_script_let_with_rhs(frame, statements.result(), base, base.add(RHS_OFFSET.with(Cell::get)), masked_rhs)? };
    assert_eq!(control, StepControl::Continue);
    assert!(unsafe { matches(access, frame, object, function) }, "outer scope is restored after the masked operation");
    // SAFETY: the next original statement is complete Return Nothing.
    assert!(unsafe { statements.begin(access, frame)? }.is_none());
    Ok(ScriptPreCallDecision::Consume)
}

unsafe fn int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    // SAFETY: this is the fixture's new i32 property.
    unsafe { (*property).array_dim = 1; (*property).element_size = 4; (*property).offset = i32::try_from(offset).unwrap(); (*property).property_flags = flags; }
    property
}
fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: indexed fixture records are live throughout execution.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}
struct DetachScripts(Vec<*mut UFunction>);
impl Drop for DetachScripts {
    fn drop(&mut self) {
        // SAFETY: all registered records and predeclared buffers outlive this
        // guard, including on partially completed setup and unwind.
        unsafe { for &function in &self.0 { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    }
}

#[test]
fn active_identity_and_external_descriptor_identity_are_owner_local() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("body-identity-vm-{}", std::process::id()));
    assert!(!root.exists());
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal/System");
    for name in ["first", "second"] {
        let dir = root.join(name).join("System"); fs::create_dir_all(&dir).unwrap();
        for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] { fs::copy(source.join(from), dir.join(to)).unwrap(); }
    }
    let options = |name: &str| { let data = root.join(name); CoreOptions { data_root: data.clone(), system_ini: data.join("System/System.ini"), user_ini: data.join("System/User.ini"), command_line: Vec::new(), output, log_file: None } };
    let owner = CoreOwner::new(options("first")).unwrap();
    let other_owner = CoreOwner::new(options("second")).unwrap();
    let access = owner.access(); let other = other_owner.access();
    let unicode = "BodyIdentityÉ𝄞";
    let name = access.intern_str(unicode, FindName::Add).unwrap();
    assert_eq!(access.name_text(name).unwrap(), unicode.chars().map(u32::from).collect::<Vec<_>>());
    assert!(access.name_is(name, unicode).unwrap(), "exact Unicode scalar sequence matches");
    for mismatch in ["bodyIdentityÉ𝄞", "BodyIdentityé𝄞", "BodyIdentityE\u{301}𝄞", "BodyIdentityÉ", "BodyIdentityÉ𝄞\0"] {
        assert!(!access.name_is(name, mismatch).unwrap(), "name comparison must not fold case, normalize, truncate or accept extra units: {mismatch:?}");
    }
    for invalid in [-1, i32::MAX] {
        let actual = access.name_is(invalid, unicode).unwrap_err();
        let reference = access.name_text(invalid).unwrap_err();
        assert_eq!(actual.diagnostic(), reference.diagnostic(), "invalid-index diagnostic matches name_text");
        assert_eq!(actual.context(), reference.context(), "invalid-index context matches name_text");
    }
    assert!(!hp2_core::core_native_catalog::iter().any(|s| s.opcode == Some(i32::from(FIRST_INDEX)) || s.opcode == Some(i32::from(SECOND_INDEX))), "fixture native indices must not collide with Core");
    assert!(!access.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[0]));
    assert!(!other.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[0]));
    access.register_vm_natives(&CATALOG).unwrap();
    assert!(access.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[0]));
    assert!(!other.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[0]), "an unregistered owner remains absent");
    other.register_vm_natives(&CATALOG).unwrap();
    assert!(other.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[0]), "same installed descriptor can match in both owners");
    for a in [access, other] {
        assert!(!a.effective_external_vm_native_is(FIRST_INDEX, &CATALOG[1]));
        assert!(!a.effective_external_vm_native_is(FIRST_INDEX, &COPY));
        assert!(!a.effective_external_vm_native_is(FIRST_INDEX, &OTHER_AGGREGATE[0]));
        assert!(a.effective_external_vm_native_is(SECOND_INDEX, &CATALOG[2]));
        assert!(!a.effective_external_vm_native_is(SECOND_INDEX, &CATALOG[3]));
        assert!(!a.effective_external_vm_native_is(SECOND_INDEX, &CATALOG[0]));
        assert!(!a.effective_external_vm_native_is(15, &CATALOG[4]), "Core Let takes precedence over an external entry");
        assert!(a.register_vm_natives(&CATALOG).is_err(), "registry installation is one-shot");
    }
    assert_eq!(INVOKED.with(Cell::get), 0, "a query never invokes a handler");
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let class_name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: owner and class records remain live for all VM calls.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &class_name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let other_object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    assert_ne!(object, other_object, "identity mismatch needs distinct live objects");
    let top = unsafe { int_property(access, class.cast(), "IdentityTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let mut child_code = vec![0x04, 0x2c, 123];
    let mut outer_code = Vec::new();
    let mut detach = DetachScripts(Vec::with_capacity(2));
    let child = unsafe { function(access, class, "BodyIdentityInner", &mut child_code) };
    detach.0.push(child);
    let result = unsafe { int_property(access, child.cast(), "ReturnValue", 0, 0x480) };
    // SAFETY: the child owns an i32 reflected result parameter.
    unsafe { (*child).num_parms = 1; (*child).parms_size = 4; (*child).return_value_offset = 0; (*child.cast::<UStruct>()).properties_size = 4; (*child.cast::<UStruct>()).children = result.cast(); }
    outer_code.push(0x0f); operand(&mut outer_code, 0x01, top.cast());
    let rhs = outer_code.len(); operand(&mut outer_code, 0x1c, child.cast()); outer_code.extend([0x16, 0x04, 0x0b]);
    let outer = unsafe { function(access, class, "BodyIdentityOuter", &mut outer_code) };
    detach.0.push(outer);
    OUTER.with(|v| v.set(outer)); INNER.with(|v| v.set(child)); OTHER_OBJECT.with(|v| v.set(other_object)); RHS_OFFSET.with(|v| v.set(rhs));
    OTHER_OWNER.with(|v| v.set(&raw const other_owner)); INNER_SEEN.with(|v| v.set(0)); MASKED_SEEN.with(|v| v.set(0));
    // This allocated frame slot is used for pointer comparison only; the query
    // must not read its uninitialized contents when there is no active scope.
    let mut inactive_frame = std::mem::MaybeUninit::<FFrame>::uninit();
    assert!(!access.current_script_body_matches(inactive_frame.as_mut_ptr(), object, outer, outer_code.as_ptr(), outer_code.len()), "no scope before event");
    let binding = unsafe { access.install_script_body_replacement("body-identity", &[outer, child], body).unwrap() };
    // SAFETY: both live reflected scripts and their object survive the event.
    unsafe { access.execute_event(object, outer, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(unsafe { (*object.cast::<UTextBuffer>()).top }, 123);
    assert_eq!(INNER_SEEN.with(Cell::get), 1); assert_eq!(MASKED_SEEN.with(Cell::get), 1);
    assert!(!access.current_script_body_matches(inactive_frame.as_mut_ptr(), object, outer, outer_code.as_ptr(), outer_code.len()), "no scope after event");
    assert_eq!(INVOKED.with(Cell::get), 0);
    drop(binding); OTHER_OWNER.with(|v| v.set(ptr::null()));
    OUTER.with(|v| v.set(ptr::null_mut())); INNER.with(|v| v.set(ptr::null_mut())); OTHER_OBJECT.with(|v| v.set(ptr::null_mut()));
    drop(detach); other_owner.shutdown().unwrap(); owner.shutdown().unwrap();
}
