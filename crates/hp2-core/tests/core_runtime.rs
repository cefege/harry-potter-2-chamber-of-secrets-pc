use hp2_abi::engine::generated::{UFunction, UObject, UProperty, UStruct, TArrayRaw};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use hp2_core::property_value::Hp2ValueOperation;
use hp2_core::core_gc::GcLog;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

#[path = "core_runtime/health_callback_gc.rs"]
mod health_callback_gc;
#[path = "core_runtime/dynamic_array.rs"]
mod dynamic_array;
#[path = "core_runtime/console_arguments.rs"]
mod console_arguments;

static REENTER_OUTPUT: AtomicBool = AtomicBool::new(false);
static REENTRY_VEC_X: AtomicI32 = AtomicI32::new(0);
static REENTRY_WARNING_COUNT: AtomicI32 = AtomicI32::new(0);
static TRACKED_STATE_FRAME: AtomicUsize = AtomicUsize::new(0);
static RELEASED_STATE_FRAMES: AtomicUsize = AtomicUsize::new(0);
static CAPTURE_OUTPUT: AtomicBool = AtomicBool::new(false);
static CAPTURED_OUTPUT: std::sync::Mutex<Vec<(i32, Vec<u32>)>> = std::sync::Mutex::new(Vec::new());
static EXTERNAL_DESTROYS: AtomicUsize = AtomicUsize::new(0);
static EXTERNAL_FINALIZATIONS: AtomicUsize = AtomicUsize::new(0);
static EXTERNAL_REENTRANT_DELETE: AtomicBool = AtomicBool::new(false);
const EXTERNAL_PAYLOAD: u64 = 0x2110_2026_1029;
const REENTRANT_FINALIZER_PAYLOAD: u64 = 0xFA11_2026_1103;

unsafe fn external_destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: the external ABI includes this payload and Core's lifecycle prefix.
    let payload = unsafe { (*object.cast::<ExternalObjectRecord>()).payload };
    unsafe { (hp2_core::core_class_specs::NativeRecordKind::Object.operations().destroy)(access, object)?; }
    if payload == 0x2110_2026_1029 {
        EXTERNAL_DESTROYS.fetch_add(1, Ordering::SeqCst);
    }
    Ok(())
}

unsafe fn external_finalize(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let payload = unsafe { (*object.cast::<ExternalObjectRecord>()).payload };
    if payload == EXTERNAL_PAYLOAD || payload == REENTRANT_FINALIZER_PAYLOAD {
        EXTERNAL_FINALIZATIONS.fetch_add(1, Ordering::SeqCst);
    }
    if payload == REENTRANT_FINALIZER_PAYLOAD {
        EXTERNAL_REENTRANT_DELETE.store(unsafe { access.delete_object(object) }.is_ok(), Ordering::SeqCst);
        return Err(RuntimeError::new("External final-member failure"));
    }
    Ok(())
}

unsafe fn external_serialize(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    // SAFETY: the external ABI uses Core Object's complete lifecycle prefix.
    unsafe { (hp2_core::core_class_specs::NativeRecordKind::Object.operations().serialize)(access, object, archive)?; }
    // SAFETY: this class's declared static layout includes the aligned u64 tail.
    let payload = unsafe { &mut (*object.cast::<ExternalObjectRecord>()).payload };
    let mut low = *payload as u32;
    let mut high = (*payload >> 32) as u32;
    archive.serialize_u32(&mut low)?;
    archive.serialize_u32(&mut high)?;
    if archive.is_loading() {
        *payload = u64::from(low) | (u64::from(high) << 32);
    }
    Ok(())
}

static ACTIVE_REENTRY_FUNCTION: AtomicUsize = AtomicUsize::new(0);

const EXTERNAL_OBJECT_OPS: hp2_core::object_owner::ClassOps =
    hp2_core::core_class_specs::NativeRecordKind::Object.operations();
static EXTERNAL_OPS: hp2_core::object_owner::ClassOps = hp2_core::object_owner::ClassOps {
    destroy: external_destroy,
    finalize_members: external_finalize,
    serialize: external_serialize,
    ..EXTERNAL_OBJECT_OPS
};

#[repr(C)]
struct ExternalObjectRecord {
    raw: UObject,
    payload: u64,
    collision_bits: u32,
}

static EXTERNAL_CLASSES: &[hp2_core::core_class_specs::NativeClassSpec] = &[
    hp2_core::core_class_specs::NativeClassSpec {
        short_name: "ExternalObject",
        package: "Engine",
        super_name: Some("Object"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: hp2_core::core_class_specs::NativeRecordKind::Object,
        layout: Some(std::alloc::Layout::new::<ExternalObjectRecord>()),
        operations: Some(EXTERNAL_OPS),
        properties: &[
            hp2_core::core_class_specs::StartupProperty { name: "bAlignBottom", kind: hp2_core::core_class_specs::StartupPropertyKind::Bool { bit_mask: 64 }, category: "Collision", property_flags: 1, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(ExternalObjectRecord, collision_bits), array_dim: 1 },
            hp2_core::core_class_specs::StartupProperty { name: "bAlignBottomAlways", kind: hp2_core::core_class_specs::StartupPropertyKind::Bool { bit_mask: 128 }, category: "Collision", property_flags: 1, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(ExternalObjectRecord, collision_bits), array_dim: 1 },
            hp2_core::core_class_specs::StartupProperty { name: "bBlockCamera", kind: hp2_core::core_class_specs::StartupPropertyKind::Bool { bit_mask: 256 }, category: "Collision", property_flags: 1, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(ExternalObjectRecord, collision_bits), array_dim: 1 },
        ],
        defaults: &[],
    },
];
static INVALID_QUALIFIED_EDGE: &[hp2_core::core_class_specs::NativeClassSpec] = &[
    hp2_core::core_class_specs::NativeClassSpec {
        short_name: "InvalidQualifiedParent",
        package: "Engine",
        super_name: Some("Missing.Object"),
        within: "Object",
        flags: 0,
        object_flags: 4,
        config: "System",
        guid: [0; 4],
        kind: hp2_core::core_class_specs::NativeRecordKind::Object,
        layout: Some(std::alloc::Layout::new::<ExternalObjectRecord>()),
        operations: Some(EXTERNAL_OPS),
        properties: &[],
        defaults: &[],
    },
];

struct StateFrameTrackingAllocator;
unsafe impl std::alloc::GlobalAlloc for StateFrameTrackingAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        if TRACKED_STATE_FRAME.compare_exchange(pointer as usize, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            RELEASED_STATE_FRAMES.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout); }
    }
}

#[global_allocator]
static ALLOCATOR: StateFrameTrackingAllocator = StateFrameTrackingAllocator;

fn output(access: CoreAccess<'_>, category: i32, _message: &[u32]) -> RuntimeResult<()> {
    if CAPTURE_OUTPUT.load(Ordering::SeqCst) {
        CAPTURED_OUTPUT.lock().expect("output capture lock").push((category, _message.to_vec()));
    }
    if category == access.intern_str("Warning", FindName::Find)? {
        REENTRY_WARNING_COUNT.fetch_add(1, Ordering::SeqCst);
        return Ok(());
    }
    if REENTER_OUTPUT.swap(false, Ordering::SeqCst) {
        let object_class = access.native_class("Object")?;
        let object = unsafe {
            access.construct_object(object_class, access.transient_package(), 0, 0x0000_4000, std::ptr::null_mut())?
        };
        let function = find_function(access, object_class.cast(), "Vec")?;
        let mut params = params_storage(function);
        write_parameter(access, function, &mut params, "X", 21.0_f32)?;
        write_parameter(access, function, &mut params, "Y", 22.0_f32)?;
        write_parameter(access, function, &mut params, "Z", 23.0_f32)?;
        let mut result = FVector { x: 0.0, y: 0.0, z: 0.0 };
        unsafe { access.execute_event(object, function, params.as_mut_ptr().cast(), (&mut result as *mut FVector).cast())?; }
        REENTRY_VEC_X.store(result.x.to_bits() as i32, Ordering::SeqCst);
        let warning = access.intern_str("Warning", FindName::Find)?;
        access.emit_log(warning, &"nested warning".chars().map(u32::from).collect::<Vec<_>>())?;
    }
    Ok(())
}

struct EmptyMapping;
impl ArchiveMapping for EmptyMapping {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new("unexpected object index").with_context(&index.to_string()))
    }
    fn save_object(&self, _object: *mut UObject) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected object reference")) }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected name index").with_context(&index.to_string())) }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected name reference").with_context(&index.to_string())) }
    fn visit_object(&self, _slot: *mut *mut UObject) -> RuntimeResult<()> { Err(RuntimeError::new("unexpected object visit")) }
    fn visit_name(&self, _index: i32) -> RuntimeResult<()> { Err(RuntimeError::new("unexpected name visit")) }
}
struct NullMapping;
impl ArchiveMapping for NullMapping {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == 0 { Ok(std::ptr::null_mut()) } else { EmptyMapping.load_object(index) }
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { Ok(0) } else { EmptyMapping.save_object(object) }
    }
    fn load_name(&self, index: i32) -> RuntimeResult<i32> {
        if index == 0 { Ok(0) } else { EmptyMapping.load_name(index) }
    }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> {
        if index == 0 { Ok(0) } else { EmptyMapping.save_name(index) }
    }
    fn visit_object(&self, slot: *mut *mut UObject) -> RuntimeResult<()> { EmptyMapping.visit_object(slot) }
    fn visit_name(&self, index: i32) -> RuntimeResult<()> { EmptyMapping.visit_name(index) }
}


fn workspace_root() -> PathBuf {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Some(root) = std::env::var_os("HP2_CORE_DATA_ROOT") {
        return PathBuf::from(root);
    }
    workspace.join("HarryPotter2/Unreal")
}

fn isolated_data_root() -> (PathBuf, PathBuf) {
    let artifact_root = std::env::var_os("HP2_ARTIFACT_DIR").map(PathBuf::from)
        .expect("HP2_ARTIFACT_DIR must name the pre-created isolated test artifact directory");
    assert!(artifact_root.is_dir(), "HP2_ARTIFACT_DIR must exist before the consumer starts");
    let source_root = workspace_root();
    let root = artifact_root.join(format!("core-runtime-{}", std::process::id()));
    if root.exists() { fs::remove_dir_all(&root).expect("remove this test's prior isolated data root"); }
    fs::create_dir_all(root.join("System")).expect("create isolated System directory");
    fs::copy(source_root.join("System/Core.u"), root.join("System/Core.u")).expect("copy actual prototype Core.u");
    for (name, alternatives) in [
        ("System.ini", ["System.ini", "Default.ini"]),
        ("User.ini", ["User.ini", "DefUser.ini"]),
    ] {
        let candidates = alternatives.map(|file| source_root.join("System").join(file));
        let source = candidates.iter().find(|path| path.is_file())
            .expect("prototype data must contain the required default INI");
        fs::copy(source, root.join("System").join(name)).expect("copy source INI into isolated data root");
    }
    let system_ini = root.join("System/System.ini");
    (root, system_ini)
}

fn owner(data_root: &Path, system_ini: &Path) -> CoreOwner {
    let owner = CoreOwner::new(CoreOptions {
        data_root: data_root.to_path_buf(),
        system_ini: system_ini.to_path_buf(),
        user_ini: data_root.join("System/User.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    }).expect("initialize direct Core owner");
    owner.access().set_process_mode(CoreProcessMode {
        is_client: true,
        ..CoreProcessMode::default()
    });
    owner
}

fn find_field(access: CoreAccess<'_>, structure: *mut UStruct, name: &str) -> RuntimeResult<*mut UProperty> {
    let name = access.intern_str(name, FindName::Find)?;
    let property_class = access.native_class("Property")?;
    let field = unsafe { access.find_field(structure, name, property_class) };
    if field.is_null() { return Err(RuntimeError::new("reflected property not found").with_context(&name.to_string())); }
    Ok(field.cast())
}

fn find_function(access: CoreAccess<'_>, structure: *mut UStruct, name: &str) -> RuntimeResult<*mut UFunction> {
    let name = access.intern_str(name, FindName::Find)?;
    let class = access.native_class("Function")?;
    let field = unsafe { access.find_field(structure, name, class) };
    if field.is_null() { return Err(RuntimeError::new("reflected function not found").with_context(&name.to_string())); }
    Ok(field.cast())
}

fn params_storage(function: *mut UFunction) -> Vec<u64> {
    let size = usize::from(unsafe { (*function).parms_size });
    vec![0; size.div_ceil(std::mem::size_of::<u64>()).max(1)]
}

fn write_parameter<T: Copy>(access: CoreAccess<'_>, function: *mut UFunction, params: &mut [u64], name: &str, value: T) -> RuntimeResult<()> {
    let property = find_field(access, function.cast(), name)?;
    let offset = unsafe { (*property).offset };
    let width = unsafe { (*property).element_size };
    let size = usize::from(unsafe { (*function).parms_size });
    if offset < 0
        || unsafe { (*property).property_flags } & 0x80 == 0
        || width < std::mem::size_of::<T>() as i32
        || (offset as usize).checked_add(std::mem::size_of::<T>()).is_none_or(|end| end > size)
    {
        return Err(RuntimeError::new("invalid reflected parameter layout").with_context(name));
    }
    // SAFETY: params are u64-aligned, bounds were checked against parms_size, and T is copied.
    unsafe { params.as_mut_ptr().cast::<u8>().add(offset as usize).cast::<T>().write_unaligned(value); }
    Ok(())
}

fn run_script(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, params: &mut [u64], result: *mut c_void) -> RuntimeResult<()> {
    // SAFETY: caller supplies a live canonical receiver/function and reflected-sized aligned storage.
    unsafe { access.execute_event(object, function, params.as_mut_ptr().cast(), result) }
}

fn native_string_call(function: *mut UFunction, message: &str, tag: Option<i32>) -> Vec<u8> {
    assert!(message.is_ascii());
    let mut code = vec![0x1c]; // FinalFunction; the target is an actual Core.u function.
    code.extend(unsafe { (*function.cast::<UObject>()).index }.to_le_bytes());
    code.push(0x1f);
    code.extend(message.bytes());
    code.push(0);
    if let Some(tag) = tag {
        code.push(0x21);
        code.extend(tag.to_le_bytes());
    }
    code.extend([0x16, 0x04, 0x25]);
    code
}

fn run_probe(access: CoreAccess<'_>, object: *mut UObject, mut code: Vec<u8>, result: *mut c_void) -> RuntimeResult<()> {
    let class = access.native_class("Function")?;
    let name = access.intern_str("RuntimeNativeProbe", FindName::Add)?;
    let capture = CAPTURE_OUTPUT.swap(false, Ordering::SeqCst);
    let wrapper = unsafe { access.construct_object(class, (*object).class.cast(), name, 0x4000, std::ptr::null_mut()) };
    CAPTURE_OUTPUT.store(capture, Ordering::SeqCst);
    let wrapper = wrapper?.cast::<UFunction>();
    unsafe {
        (*wrapper).function_flags = 0;
        (*wrapper).parms_size = 0;
        (*wrapper).return_value_offset = u16::MAX;
        (*wrapper.cast::<UStruct>()).properties_size = 0;
        (*wrapper.cast::<UStruct>()).properties_align = 4;
        (*wrapper.cast::<UStruct>()).script = TArrayRaw { data: code.as_mut_ptr().cast(), num: code.len() as i32, max: code.len() as i32 };
    }
    let outcome = run_script(access, object, wrapper, &mut [], result);
    // The canonical function borrows this test's code only for the synchronous call.
    unsafe { (*wrapper.cast::<UStruct>()).script = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 }; }
    outcome
}

fn captured_output() -> Vec<(i32, Vec<u32>)> {
    std::mem::take(&mut *CAPTURED_OUTPUT.lock().expect("output capture lock"))
}

fn utf32(text: &str) -> Vec<u32> { text.chars().map(u32::from).collect() }

thread_local! {
    static SCOPE_GC_PENDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SCOPE_GC_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static SCOPE_GC_EVENTS: std::cell::RefCell<Vec<(*mut UFunction, hp2_core::ScriptScopePhase)>> = const { std::cell::RefCell::new(Vec::new()) };
}

unsafe fn select_rooted_scope_gc(access: CoreAccess<'_>, _: *mut UObject, _: *mut UFunction) -> RuntimeResult<bool> {
    if SCOPE_GC_PENDING.with(|pending| pending.replace(false)) {
        access.collect_garbage(0, 0, &|_| Ok(()))?;
        SCOPE_GC_COUNT.with(|count| count.set(count.get() + 1));
    }
    Ok(true)
}

unsafe fn record_rooted_scope_gc(_: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, _: *const u8, _: usize, phase: hp2_core::ScriptScopePhase) -> RuntimeResult<()> {
    SCOPE_GC_EVENTS.with(|events| events.borrow_mut().push((function, phase)));
    Ok(())
}

// The existing rust_core_runtime CTest uses test opt-level=1 for deep Core.u GC.
// The separate unoptimized hook-test GC overflow remains unresolved evidence.
#[test]
fn rooted_selector_gc_scope() {
    use std::{io::Write, ptr};
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("rooted-scope-gc-{}", std::process::id()));
    assert!(!root.exists(), "use a new isolated fixture");
    fs::create_dir_all(root.join("System")).unwrap();
    let source = workspace_root().join("System");
    fs::copy(source.join("Core.u"), root.join("System/Core.u")).unwrap();
    fs::copy(source.join("Default.ini"), root.join("System/System.ini")).unwrap();
    fs::copy(source.join("DefUser.ini"), root.join("System/User.ini")).unwrap();
    let exe = std::env::current_exe().expect("test executable path");
    #[cfg(target_os = "macos")]
    let stack_bytes = unsafe { libc::pthread_get_stacksize_np(libc::pthread_self()) };
    #[cfg(not(target_os = "macos"))]
    let stack_bytes = 0_usize;
    writeln!(std::io::stderr().lock(), "rooted_selector_gc_scope executable={} pthread_stack_bytes={} rust_core_runtime_registered_test_opt_level=1", exe.display(), stack_bytes).unwrap();
    let owner = owner(&root, &root.join("System/System.ini"));
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let commandlet: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &commandlet, 1).unwrap(); }
    let class = access.native_class("Object").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    let mut callee_code = vec![0x04, 0x0b];
    let mut caller_code = Vec::new();
    let make_function = |name: &str, code: &mut Vec<u8>| {
        let function = unsafe { access.construct_object(access.native_class("Function").unwrap(), class.cast(), access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UFunction>();
        unsafe {
            (*function).function_flags = 0;
            (*function).parms_size = 0;
            (*function).return_value_offset = u16::MAX;
            (*function.cast::<UStruct>()).properties_size = 0;
            (*function.cast::<UStruct>()).properties_align = 4;
            (*function.cast::<UStruct>()).script = TArrayRaw { data: code.as_mut_ptr().cast(), num: code.len() as i32, max: code.len() as i32 };
        }
        function
    };
    let callee = make_function("RootedScopeCallee", &mut callee_code);
    caller_code.push(0x1c);
    caller_code.extend(unsafe { (*callee.cast::<UObject>()).index }.to_le_bytes());
    caller_code.extend([0x16, 0x04, 0x0b]);
    let caller = make_function("RootedScopeCaller", &mut caller_code);
    unsafe { for root in [object, caller.cast::<UObject>(), callee.cast::<UObject>()] { access.add_root(root); } }
    let object_index = unsafe { (*object).index };
    let caller_index = unsafe { (*caller.cast::<UObject>()).index };
    let callee_index = unsafe { (*callee.cast::<UObject>()).index };
    let scope = unsafe { access.install_resolved_script_scope_observer("rooted-scope-gc", select_rooted_scope_gc, record_rooted_scope_gc).unwrap() };
    SCOPE_GC_EVENTS.with(|events| events.borrow_mut().clear());
    SCOPE_GC_COUNT.with(|count| count.set(0));
    SCOPE_GC_PENDING.with(|pending| pending.set(true));
    unsafe { access.execute_event(object, caller, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(SCOPE_GC_COUNT.with(std::cell::Cell::get), 1, "selector ran a real collection");
    assert_eq!(access.object_at(object_index), object);
    assert_eq!(access.object_at(caller_index), caller.cast());
    assert_eq!(access.object_at(callee_index), callee.cast());
    SCOPE_GC_EVENTS.with(|events| assert_eq!(*events.borrow(), vec![
        (caller, hp2_core::ScriptScopePhase::Enter), (callee, hp2_core::ScriptScopePhase::Enter),
        (callee, hp2_core::ScriptScopePhase::Exit { succeeded: true }), (caller, hp2_core::ScriptScopePhase::Exit { succeeded: true }),
    ]));
    drop(scope);
    unsafe { for function in [caller, callee] { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; } }
    for root in [object, caller.cast::<UObject>(), callee.cast::<UObject>()] { access.remove_root(root); }
    owner.shutdown().unwrap();
}

#[test]
fn core_runtime_acceptance() {
    let (data_root, system_ini) = isolated_data_root();
    let valid_package = fs::read(data_root.join("System/Core.u")).expect("read copied prototype package");
    let owner = owner(&data_root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });

    // A failed truncated load must not poison the canonical loader; retry the same root after restore.
    let package_path = data_root.join("System/Core.u");
    fs::write(&package_path, &valid_package[..valid_package.len() / 2]).expect("truncate isolated package copy");
    let commandlet_base = access.native_class("Commandlet").expect("native Commandlet registration");
    let simple_name = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    assert!(unsafe { access.static_load_class(commandlet_base, std::ptr::null_mut(), &simple_name, 1) }.is_err(), "truncated Core.u must fail");
    fs::write(&package_path, &valid_package).expect("restore isolated valid Core.u");
    let simple_class = unsafe { access.static_load_class(commandlet_base, std::ptr::null_mut(), &simple_name, 1) }
        .expect("fresh retry on the same owner after restoring Core.u");
    let simple = unsafe { access.construct_object(simple_class, access.transient_package(), 0, 0x0000_4000, std::ptr::null_mut()) }
        .expect("construct actual SimpleCommandlet");

    // Exercise real Vec and GetAxes with reflected offsets checked before every raw parameter write.
    let object_class = access.native_class("Object").expect("native Object registration");
    let object = unsafe { access.construct_object(object_class, access.transient_package(), 0, 0x0000_4000, std::ptr::null_mut()) }
        .expect("construct Object receiver");
    assert!(unsafe { (*object).state_frame.is_null() }, "construction does not initialize execution");
    unsafe { access.initialize_execution(object) }.expect("explicit Object execution initialization");
    let frame = unsafe { (*object).state_frame.cast::<hp2_abi::engine::generated::FStateFrame>() };
    assert!(!frame.is_null(), "explicit initialization publishes a state frame");
    assert_eq!(unsafe { (*frame).node }, object_class.cast(), "frame node is the object's class");
    assert_eq!(unsafe { (*frame).object }, object.cast(), "frame object is the canonical instance");
    assert_eq!(unsafe { (*frame).state_node }, object_class.cast(), "state node is initialized from class");
    assert_eq!(unsafe { (*frame).probe_mask }, u64::MAX, "state probe mask has native initialized value");
    assert_ne!(unsafe { (*object).object_flags } & 0x0200_0000, 0, "initialization sets RF_HasStack");
    assert!(unsafe { access.initialize_execution(std::ptr::null_mut()) }.is_err(), "null identity is rejected");
    let other_owner = self::owner(&data_root, &system_ini);
    let other_access = other_owner.access();
    // Fresh owners have not loaded Core.u's Object metadata; use a concrete Package.
    let other_object_class = other_access.native_class("Package").expect("second owner's Package class");
    let other_object = unsafe { other_access.construct_object(other_object_class, std::ptr::null_mut(), 0, 0, std::ptr::null_mut()) }
        .expect("construct second owner's Package");
    unsafe { other_access.initialize_execution(other_object) }.expect("Package inherits Object execution initialization");
    let other_frame = unsafe { (*other_object).state_frame.cast::<hp2_abi::engine::generated::FStateFrame>() };
    assert!(!other_frame.is_null());
    assert_eq!(unsafe { (*other_frame).node }, other_object_class.cast());
    unsafe { (*other_frame).probe_mask = 0x1234; }
    assert!(unsafe { access.initialize_execution(other_object) }.is_err(), "another owner's object is rejected");
    assert_eq!(unsafe { (*other_frame).probe_mask }, 0x1234, "rejection does not reinitialize foreign execution state");
    other_owner.shutdown().expect("shutdown second owner");
    let vec_function = find_function(access, object_class.cast(), "Vec").expect("actual Object.Vec function");
    assert!(unsafe { (*vec_function).parms_size } >= 12, "Vec must expose three reflected float inputs");
    let mut vec_params = params_storage(vec_function);
    write_parameter(access, vec_function, &mut vec_params, "X", 1.0_f32).expect("validate Vec.X");
    write_parameter(access, vec_function, &mut vec_params, "Y", 2.0_f32).expect("validate Vec.Y");
    write_parameter(access, vec_function, &mut vec_params, "Z", 3.0_f32).expect("validate Vec.Z");
    let mut vector = FVector { x: 0.0, y: 0.0, z: 0.0 };
    run_script(access, object, vec_function, &mut vec_params, (&mut vector as *mut FVector).cast()).expect("execute actual Object.Vec");
    assert_eq!(vector, FVector { x: 1.0, y: 2.0, z: 3.0 });

    let axes = find_function(access, object_class.cast(), "GetAxes").expect("actual Object.GetAxes native");
    let axes_size = usize::from(unsafe { (*axes).parms_size });
    let mut axes_params = params_storage(axes);
    let rotation = find_field(access, axes.cast(), "A").expect("GetAxes rotation parameter");
    let rotation_offset = unsafe { (*rotation).offset };
    let rotation_width = unsafe { (*rotation).element_size };
    let rotation_end = usize::try_from(rotation_offset)
        .ok()
        .and_then(|offset| offset.checked_add(std::mem::size_of::<hp2_abi::engine::generated::FRotator>()));
    assert!(unsafe { (*rotation).property_flags } & 0x80 != 0, "GetAxes rotation is a reflected parm");
    assert!(rotation_width >= std::mem::size_of::<hp2_abi::engine::generated::FRotator>() as i32 && rotation_end.is_some_and(|end| end <= axes_size), "GetAxes rotation offset/size fit parms_size");
    for name in ["X", "Y", "Z"] {
        let property = find_field(access, axes.cast(), name).expect("GetAxes out parameter");
        let offset = unsafe { (*property).offset };
        let width = unsafe { (*property).element_size };
        let end = usize::try_from(offset).ok().and_then(|offset| offset.checked_add(std::mem::size_of::<FVector>()));
        assert!(unsafe { (*property).property_flags } & 0x80 != 0, "{name} is a reflected out parm");
        assert!(width >= std::mem::size_of::<FVector>() as i32 && end.is_some_and(|end| end <= axes_size), "{name} offset/size must fit reflected parms");
    }
    // SAFETY: checked the reflected parameter extent and alignment of the u64-backed scratch.
    unsafe { axes_params.as_mut_ptr().cast::<u8>().add(rotation_offset as usize).cast::<hp2_abi::engine::generated::FRotator>().write_unaligned(hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 }); }
    let axes_caller = unsafe {
        access.construct_object(
            access.native_class("Function").unwrap(), object_class.cast(),
            access.intern_str("AxesFromScript", FindName::Add).unwrap(),
            0x4000, std::ptr::null_mut(),
        )
    }.expect("construct a real script caller for indexed GetAxes").cast::<UFunction>();
    let mut axes_code = vec![0x1c]; // FinalFunction.
    axes_code.extend(unsafe { (*axes.cast::<UObject>()).index }.to_le_bytes());
    for name in ["A", "X", "Y", "Z"] {
        let property = find_field(access, axes.cast(), name).unwrap();
        axes_code.push(0x00); // LocalVariable: the reflected parameter offset.
        axes_code.extend(unsafe { (*property.cast::<UObject>()).index }.to_le_bytes());
    }
    axes_code.extend([0x16, 0x04, 0x0b]); // EndFunctionParms, Return, Nothing.
    unsafe {
        (*axes_caller).function_flags = 0;
        (*axes_caller).return_value_offset = u16::MAX;
        (*axes_caller).parms_size = (*axes).parms_size;
        let caller = axes_caller.cast::<UStruct>();
        (*caller).properties_size = (*axes.cast::<UStruct>()).properties_size;
        (*caller).properties_align = (*axes.cast::<UStruct>()).properties_align;
        // The synchronous caller uses the real, POD parameter descriptors.
        (*caller).property_link = (*axes.cast::<UStruct>()).property_link;
        (*caller).script = TArrayRaw {
            data: axes_code.as_mut_ptr().cast(), num: axes_code.len() as i32, max: axes_code.len() as i32,
        };
    }
    let axes_outcome = run_script(access, object, axes_caller, &mut axes_params, std::ptr::null_mut());
    unsafe {
        (*axes_caller.cast::<UStruct>()).script =
            TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    }
    axes_outcome.expect("execute actual Object.GetAxes through source script dispatch");
    let axes_values = ["X", "Y", "Z"].map(|name| {
        let property = find_field(access, axes.cast(), name).expect("validated axis property");
        let offset = unsafe { (*property).offset } as usize;
        // SAFETY: each axis property's reflected size and bounds were checked above.
        unsafe { axes_params.as_ptr().cast::<u8>().add(offset).cast::<FVector>().read_unaligned() }
    });
    assert_eq!(axes_values, [FVector { x: 1.0, y: 0.0, z: 0.0 }, FVector { x: 0.0, y: 1.0, z: 0.0 }, FVector { x: 0.0, y: 0.0, z: 1.0 }]);

    // Invoke actual TestFunction then Main; VM logging reenters Core and executes Vec.
    let test_function = find_function(access, simple_class.cast(), "TestFunction").expect("SimpleCommandlet.TestFunction");
    let mut no_params = params_storage(test_function);
    let mut returned = 0_i32;
    run_script(access, simple, test_function, &mut no_params, std::ptr::null_mut()).expect("execute actual TestFunction into reflected return slot");
    let return_offset = usize::from(unsafe { (*test_function).return_value_offset });
    assert!(return_offset.checked_add(std::mem::size_of::<i32>()).is_some_and(|end| end <= usize::from(unsafe { (*test_function).parms_size })));
    let reflected_return = unsafe { no_params.as_ptr().cast::<u8>().add(return_offset).cast::<i32>().read_unaligned() };
    assert_eq!(reflected_return, 666);
    let main = find_function(access, simple_class.cast(), "Main").expect("SimpleCommandlet.Main");
    let mut main_params = params_storage(main);
    let in_parms = find_field(access, main.cast(), "Parms").expect("Main Parms parameter");
    let in_offset = unsafe { (*in_parms).offset };
    let in_size = unsafe { (*in_parms).element_size };
    assert!(unsafe { (*in_parms).property_flags } & 0x80 != 0, "Main Parms is a reflected parm");
    assert!(in_offset >= 0 && in_size >= std::mem::size_of::<TArrayRaw>() as i32 && (in_offset as usize + std::mem::size_of::<TArrayRaw>()) <= usize::from(unsafe { (*main).parms_size }));
    let mut empty_text: [i32; 1] = [0];
    let text = TArrayRaw { data: empty_text.as_mut_ptr().cast(), num: 1, max: 1 };
    // SAFETY: reflected Main Parms bounds/alignment were validated; TArrayRaw matches the field size.
    unsafe { main_params.as_mut_ptr().cast::<u8>().add(in_offset as usize).cast::<TArrayRaw>().write_unaligned(text); }
    let warnings_before = access.warning_count();
    let errors_before = access.error_count();
    REENTRY_VEC_X.store(0, Ordering::SeqCst);
    REENTRY_WARNING_COUNT.store(0, Ordering::SeqCst);
    REENTER_OUTPUT.store(true, Ordering::SeqCst);
    run_script(access, simple, main, &mut main_params, std::ptr::null_mut::<c_void>()).expect("execute actual SimpleCommandlet.Main");
    assert_eq!(REENTRY_VEC_X.load(Ordering::SeqCst), 21.0_f32.to_bits() as i32, "logging callback reentered Vec");
    assert_eq!(REENTRY_WARNING_COUNT.load(Ordering::SeqCst), 1, "reentrant Warning reached output once");
    assert_eq!(access.warning_count(), warnings_before + 1, "Core warning accounting includes reentrant Warning");
    assert_eq!(access.error_count(), errors_before, "successful Main introduces no Core errors");

    // Call the native Main entry, not only its script override: it must resolve
    // the receiver's real Main event and return without corrupting the caller.
    let native_main = find_function(access, commandlet_base.cast(), "Main").expect("native Commandlet.Main");
    let mut native_result = -1_i32;
    run_probe(access, simple, native_string_call(native_main, "", None), std::ptr::from_mut(&mut native_result).cast())
        .expect("native Main dispatches to the actual SimpleCommandlet override");
    assert_eq!(native_result, 0);

    let log = find_function(access, object_class.cast(), "Log").expect("actual Object.Log");
    let warn = find_function(access, object_class.cast(), "Warn").expect("actual Object.Warn");
    let tag = access.intern_str("CoreConsumerTag", FindName::Add).expect("explicit log category");
    let script_log = access.intern_str("ScriptLog", FindName::Find).expect("default script log category");
    CAPTURE_OUTPUT.store(true, Ordering::SeqCst);
    run_probe(access, simple, native_string_call(log, "tagged payload", Some(tag)), std::ptr::from_mut(&mut native_result).cast())
        .expect("Log consumes its optional explicit tag and the caller resumes");
    assert_eq!(captured_output(), vec![(tag, utf32("tagged payload"))]);
    run_probe(access, simple, native_string_call(log, "default payload", None), std::ptr::from_mut(&mut native_result).cast())
        .expect("Log without an explicit tag");
    assert_eq!(captured_output(), vec![(script_log, utf32("default payload"))]);
    let warn_code = native_string_call(warn, "diagnostic payload", None);
    let warn_offset = warn_code.len() - 2;
    let warning_count = access.warning_count();
    run_probe(access, simple, warn_code, std::ptr::from_mut(&mut native_result).cast()).expect("Warn through a real script frame");
    let diagnostics = captured_output();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].0, access.intern_str("ScriptWarning", FindName::Find).expect("frame diagnostic category"));
    let expected_tail = format!(" (Function Core.SimpleCommandlet.RuntimeNativeProbe:{warn_offset:04X}) diagnostic payload");
    assert!(diagnostics[0].1.starts_with(&utf32("SimpleCommandlet Transient.")));
    assert!(diagnostics[0].1.ends_with(&utf32(&expected_tail)));
    assert_eq!(access.warning_count(), warning_count, "frame diagnostics do not become GWarn Warning events");

    // Null class-context diagnostics carry readable source context, zero the
    // skipped expression's result, and leave the caller's instruction cursor valid.
    let mut null_context = vec![0x04, hp2_abi::engine::generated::EX_ClassContext as u8, hp2_abi::engine::generated::EX_NoObject as u8];
    null_context.extend(1_u16.to_le_bytes());
    null_context.extend([4, 0x26]); // Four-byte result; skip IntOne.
    native_result = 99;
    run_probe(access, simple, null_context, std::ptr::from_mut(&mut native_result).cast()).expect("null class-context warning");
    assert_eq!(native_result, 0);
    let diagnostics = captured_output();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].0, access.intern_str("ScriptWarning", FindName::Find).expect("frame diagnostic category"));
    assert!(diagnostics[0].1.ends_with(&utf32(" (Function Core.SimpleCommandlet.RuntimeNativeProbe:0003) Accessed null class context")));
    CAPTURE_OUTPUT.store(false, Ordering::SeqCst);

    // Valid nonzero source positions must not be interpreted as destructor links
    // when extracting a member from an actual Core.u vector temporary.
    let vector_name = access.intern_str("Vector", FindName::Find).expect("Vector name");
    let vector_struct = unsafe { access.find_field(object_class.cast(), vector_name, access.native_class("Struct").expect("Struct class")) }.cast::<UStruct>();
    assert!(!vector_struct.is_null());
    let x = find_field(access, vector_struct, "X").expect("Vector.X");
    let source_position = unsafe { ((*vector_struct).text_pos, (*vector_struct).line) };
    unsafe { (*vector_struct).text_pos = 11; (*vector_struct).line = 37; }
    let mut member_code = vec![0x04, hp2_abi::engine::generated::EX_StructMember as u8];
    member_code.extend(unsafe { (*x.cast::<UObject>()).index }.to_le_bytes());
    member_code.push(0x1c);
    member_code.extend(unsafe { (*vec_function.cast::<UObject>()).index }.to_le_bytes());
    for value in [5.0_f32, 6.0, 7.0] {
        member_code.push(0x1e);
        member_code.extend(value.to_le_bytes());
    }
    member_code.push(0x16);
    let mut member = 0.0_f32;
    let member_outcome = run_probe(access, simple, member_code, std::ptr::from_mut(&mut member).cast());
    unsafe { (*vector_struct).text_pos = source_position.0; (*vector_struct).line = source_position.1; }
    member_outcome.expect("Vector temporary member access ignores source-position metadata");
    assert_eq!(member, 5.0);

    // Corrupt one owned script opcode, observe a VM error, restore it, and call the same root again.
    let script = unsafe { (*test_function.cast::<UStruct>()).script };
    assert!(!script.data.is_null() && script.num > 0, "TestFunction owns actual loaded bytecode");
    // SAFETY: this is the mutable script body of a Rust-owned loaded function; restore immediately after invocation.
    let code = script.data.cast::<u8>();
    let original = unsafe { code.read() };
    unsafe { code.write(0xff); }
    let errors_before_invalid_opcode = access.error_count();
    let failed = run_script(access, simple, test_function, &mut no_params, (&mut returned as *mut i32).cast());
    unsafe { code.write(original); }
    assert!(failed.is_err(), "invalid bytecode opcode must be reported");
    assert_eq!(access.error_count(), errors_before_invalid_opcode, "direct VM error transport does not invent an output Error");
    run_script(access, simple, test_function, &mut no_params, (&mut returned as *mut i32).cast()).expect("valid call succeeds after restoring bytecode");
    assert_eq!(returned, 666);

    // The real HelloWorld intparm scalar archive writes wire bytes and restores mutation.
    let hello_name = "Core.HelloWorldCommandlet".chars().map(u32::from).collect::<Vec<_>>();
    let hello_class = unsafe { access.static_load_class(commandlet_base, std::ptr::null_mut(), &hello_name, 1) }
        .expect("load actual HelloWorldCommandlet");
    let hello = unsafe { access.construct_object(hello_class, access.transient_package(), 0, 0x0000_4000, std::ptr::null_mut()) }
        .expect("construct HelloWorld property owner");
    let int_property = find_field(access, hello_class.cast(), "intparm").expect("reflected intparm");
    let int_offset = unsafe { (*int_property).offset };
    let int_width = unsafe { (*int_property).element_size };
    let instance_size = unsafe { (*hello_class.cast::<UStruct>()).properties_size };
    assert!(instance_size >= 0 && int_offset >= 0 && int_width == 4 && (int_offset as usize + int_width as usize) <= instance_size as usize, "intparm offset and width fit the complete instance record");
    let scalar_address = unsafe { access.property_address(hello, int_property) }.expect("address actual intparm storage");
    unsafe { scalar_address.cast::<i32>().write(73); }
    let mapping = EmptyMapping;
    let flags = ArchiveFlags::new(100, 0, true);
    let archive = ObjectArchive::saving(flags, &mapping);
    unsafe { access.property_value(Hp2ValueOperation::ValueSerialize, int_property, scalar_address.cast(), std::ptr::null_mut(), 0, Some(&archive)) }
        .expect("serialize actual IntProperty");
    let bytes = archive.into_written_bytes().expect("collect scalar wire bytes");
    assert_eq!(bytes, 73_i32.to_le_bytes());
    unsafe { scalar_address.cast::<i32>().write(-1); }
    let load_archive = ObjectArchive::loading(hp2_core::package_archive::ExportArchive::from_slice(&bytes), flags, &mapping);
    unsafe { access.property_value(Hp2ValueOperation::ValueSerialize, int_property, scalar_address.cast(), std::ptr::null_mut(), 0, Some(&load_archive)) }
        .expect("deserialize actual IntProperty");
    assert_eq!(unsafe { scalar_address.cast::<i32>().read() }, 73);
    let mut statement = vec![0xa1, 0x01]; // AddEqual_IntInt, InstanceVariable.
    statement.extend(unsafe { (*int_property.cast::<UObject>()).index }.to_le_bytes());
    statement.push(0x1d); // IntConst.
    statement.extend(7_i32.to_le_bytes());
    statement.extend([0x16, 0x04, 0x0b]); // EndFunctionParms, Return, Nothing.
    let mut caller_result = 0x514e_2140_8ac3_907d_u64;
    run_probe(access, hello, statement, std::ptr::from_mut(&mut caller_result).cast())
        .expect("execute a compound assignment statement in a void script");
    assert_eq!(unsafe { scalar_address.cast::<i32>().read() }, 80, "statement still mutates its actual reflected lvalue");
    assert_eq!(caller_result, 0x514e_2140_8ac3_907d_u64, "statement results must not overwrite the caller's return storage");
    unsafe { scalar_address.cast::<i32>().write(73); }
    let package_class = access.native_class("Package").expect("unrelated cast target");
    for (opcode, target, candidate, expected) in [
        (0x2e, commandlet_base, hello, hello),
        (0x2e, package_class, hello, std::ptr::null_mut()),
        (0x2e, commandlet_base, std::ptr::null_mut(), std::ptr::null_mut()),
        (0x13, commandlet_base, hello_class.cast(), hello_class.cast()),
        (0x13, commandlet_base, hello, std::ptr::null_mut()),
    ] {
        let mut code = vec![0x04, opcode]; // Return followed by the cast expression.
        code.extend(unsafe { (*target.cast::<UObject>()).index }.to_le_bytes());
        if candidate.is_null() {
            code.push(0x2a); // NoObject.
        } else {
            code.push(0x20); // ObjectConst.
            code.extend(unsafe { (*candidate).index }.to_le_bytes());
        }
        let mut cast_result = std::ptr::null_mut::<UObject>();
        run_probe(access, hello, code, std::ptr::from_mut(&mut cast_result).cast())
            .expect("cast reads a literal class reference and one expression, without a parameter terminator");
        assert_eq!(cast_result, expected, "cast preserves class ancestry and rejects non-class metacast candidates");
    }

    // A newly constructed native property must copy a value before any package wire initializes it.
    let int_property_class = access.native_class("IntProperty").expect("native IntProperty registration");
    let constructed_property = unsafe {
        access.construct_object(int_property_class, hello_class.cast(), 0, 0x0000_4000, std::ptr::null_mut())
    }.expect("construct ordinary native IntProperty");
    let source = 12345_i32;
    let mut copied = -77_i32;
    unsafe {
        access.property_value(
            Hp2ValueOperation::ValueCopyComplete, constructed_property.cast(),
            std::ptr::from_mut(&mut copied).cast(),
            std::ptr::from_ref(&source).cast_mut().cast(), 0, None,
        )
    }.expect("copy complete value through an ordinary constructed property");
    assert_eq!(copied, source, "ordinary property construction must not leave complete copies as no-ops");
    let layout_name = access.intern_str("RuntimeAnonymousNativeLayout", FindName::Add).unwrap();
    let authored_offset = std::mem::size_of::<u64>();
    let layout_size = authored_offset + std::mem::size_of::<TArrayRaw>();
    let layout = unsafe { access.create_cpp_struct(object_class.cast(), layout_name, layout_size, 4) }
        .expect("construct runtime native struct");
    let anonymous = unsafe {
        access.add_cpp_property(layout, "StrProperty", 0, 0, authored_offset, 1, 0, 4, std::ptr::null_mut())
    }.expect("construct native property with an automatically generated name");
    unsafe { access.link_cpp_struct(layout) }.expect("link retains the actual constructor identity of an anonymous native property");
    assert_eq!(unsafe { (*anonymous).offset } as usize, authored_offset, "linking must preserve the native field offset");
    assert_eq!(unsafe { (*layout).properties_size } as usize, layout_size, "linking must preserve the native struct extent");

    // Exercise the public VM setter across FString growth, shrink, and empty transitions.
    let setter = find_function(access, object_class.cast(), "SetPropertyText").expect("Object.SetPropertyText");
    let string_property = find_field(access, hello_class.cast(), "strparm").expect("reflected strparm");
    let string_address = unsafe { access.property_address(hello, string_property) }.expect("actual strparm storage");
    let mut property_name = "strparm".chars().map(u32::from).chain(std::iter::once(0)).collect::<Vec<_>>();
    let name_value = TArrayRaw { data: property_name.as_mut_ptr().cast(), num: property_name.len() as i32, max: property_name.len() as i32 };
    for expected in ["a longer value before replacement", "two words", ""] {
        let expected_units = expected.chars().map(u32::from).collect::<Vec<_>>();
        let mut input = expected_units.iter().copied().chain(std::iter::once(0)).collect::<Vec<_>>();
        let input_value = TArrayRaw { data: input.as_mut_ptr().cast(), num: input.len() as i32, max: input.len() as i32 };
        let mut setter_params = params_storage(setter);
        write_parameter(access, setter, &mut setter_params, "PropName", name_value).expect("setter property name");
        write_parameter(access, setter, &mut setter_params, "PropValue", input_value).expect("setter property value");
        run_script(access, hello, setter, &mut setter_params, std::ptr::null_mut()).expect("replace actual strparm through VM");
        let actual = unsafe { string_address.cast::<TArrayRaw>().read_unaligned() };
        let count = if expected_units.is_empty() { 0 } else { expected_units.len() + 1 };
        assert_eq!(actual.num as usize, count, "FString count follows replacement, not old capacity");
        if count != 0 {
            let actual_units = unsafe { std::slice::from_raw_parts(actual.data.cast::<u32>(), count) };
            assert_eq!(&actual_units[..count - 1], expected_units);
            assert_eq!(actual_units[count - 1], 0);
        }
    }

    // Object-reference imports use pure ANY_PACKAGE lookup, in both grammar forms.
    let mut class_name = "Class".chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    let class_name_value = TArrayRaw { data: class_name.as_mut_ptr().cast(), num: class_name.len() as i32, max: class_name.len() as i32 };
    let unknown_package = "MustNotLoadForPropertyText";
    assert_eq!(access.intern_str(unknown_package, FindName::Find).expect("unknown package name"), 0);
    for (input_text, expected) in [
        ("Core.SimpleCommandlet", simple_class.cast()),
        ("Class'Core.SimpleCommandlet'", simple_class.cast()),
        ("MustNotLoadForPropertyText.MissingClass", std::ptr::null_mut()),
    ] {
        let before = access.object_count();
        let mut input = input_text.chars().map(u32::from).chain([0]).collect::<Vec<_>>();
        let input_value = TArrayRaw { data: input.as_mut_ptr().cast(), num: input.len() as i32, max: input.len() as i32 };
        let mut setter_params = params_storage(setter);
        write_parameter(access, setter, &mut setter_params, "PropName", class_name_value).expect("class property name");
        write_parameter(access, setter, &mut setter_params, "PropValue", input_value).expect("class property value");
        let imported = run_script(access, hello, setter, &mut setter_params, std::ptr::null_mut());
        // Restore identity before assertions, failure reporting, or destruction.
        let actual = unsafe { (*hello).class };
        unsafe { (*hello).class = hello_class.cast(); }
        imported.expect("import class reference through the real VM setter");
        assert_eq!(actual, expected, "class reference input {input_text}");
        assert_eq!(access.object_count(), before, "reference import must not create package objects");
    }
    assert_eq!(access.intern_str(unknown_package, FindName::Find).expect("find-only name after import"), 0,
        "unresolved property references must not intern package names");

    // Core.u can declare Normalize without an intrinsic index. Resolve its actual
    // native function and call it through the same FinalFunction path as game code.
    let normalize = find_function(access, object_class.cast(), "Normalize").expect("actual Object.Normalize");
    for (input, expected) in [
        ([65535_i32, 32768, -65537], [-1_i32, -32768, -1]),
        ([0, 32767, 65536], [0, 32767, 0]),
    ] {
        let mut code = vec![0x04, 0x1c]; // Return, FinalFunction.
        code.extend(unsafe { (*normalize.cast::<UObject>()).index }.to_le_bytes());
        code.push(0x22); // RotationConst.
        for component in input { code.extend(component.to_le_bytes()); }
        code.push(0x16);
        let mut output = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
        run_probe(access, hello, code, std::ptr::from_mut(&mut output).cast())
            .expect("execute Object.Normalize from script");
        assert_eq!([output.pitch, output.yaw, output.roll], expected);
    }

    // Exercise the real indexed RNG native through a script expression.
    let rand_range = find_function(access, object_class.cast(), "RandRange").expect("actual Object.RandRange");
    let mut rng_code = vec![0x04, 0x1c]; // Return, FinalFunction.
    rng_code.extend(unsafe { (*rand_range.cast::<UObject>()).index }.to_le_bytes());
    for value in [3.0_f32, 11.0_f32] {
        rng_code.push(0x1e); // FloatConst.
        rng_code.extend(value.to_le_bytes());
    }
    rng_code.push(0x16); // EndFunctionParms.
    unsafe { libc::srand(12345); }
    let native_draw = unsafe { libc::rand() };
    let expected = 8.0_f32.mul_add(native_draw as f32 / i32::MAX as f32, 3.0);
    hp2_core::rng_private::srand(12345);
    let (traced, count, ordinal) = hp2_core::ansi_trace::with_process_state(|state| (state.enabled, state.count, state.call_count));
    let mut random = 0.0_f32;
    run_probe(access, hello, rng_code, std::ptr::from_mut(&mut random).cast())
        .expect("execute actual RandRange from script");
    assert_eq!(random.to_bits(), expected.to_bits(), "RandRange follows the native libc draw and fused formula");
    if traced {
        hp2_core::ansi_trace::with_process_state(|state| {
            assert_eq!(state.call_count, ordinal + 1, "RandRange records exactly one global draw");
            assert_eq!(state.count, count + 1);
            let sample = &state.samples[count];
            assert_eq!(sample.value, native_draw);
            assert_eq!(sample.native.as_deref(), Some(b"Core.Object.RandRange".as_slice()));
            assert_eq!(sample.native_slot, 1033);
            assert_eq!(sample.class.as_deref(), Some(b"Core.HelloWorldCommandlet".as_slice()));
            assert_eq!(sample.function.as_deref(), Some(b"Core.Object.RandRange".as_slice()));
            assert!(sample.has_consumer && sample.has_outer_consumer);
            assert_eq!(sample.outer_function.as_deref(), Some(b"Core.Object.RandRange".as_slice()));
        });
    }
    // Empty PackageMap archives must round-trip, including both owned hash maps.
    let package_map_class = access.native_class("PackageMap").expect("native PackageMap class");
    let original_map = unsafe { access.construct_object(package_map_class, access.transient_package(), 0, 0x4000, std::ptr::null_mut()) }
        .expect("construct PackageMap for saving");
    let map_flags = ArchiveFlags::new(79, 0, false);
    let saved_map = ObjectArchive::saving(map_flags, &NullMapping);
    unsafe { access.serialize_object(original_map, &saved_map) }.expect("save fresh PackageMap");
    let map_bytes = saved_map.into_written_bytes().expect("saved PackageMap archive");
    let restored_map = unsafe { access.construct_object(package_map_class, access.transient_package(), 0, 0x4000, std::ptr::null_mut()) }
        .expect("construct PackageMap for loading");
    let loaded_map = ObjectArchive::loading(hp2_core::package_archive::ExportArchive::from_slice(&map_bytes), map_flags, &NullMapping);
    unsafe { access.serialize_object(restored_map, &loaded_map) }.expect("load empty PackageMap hash maps");
    assert_eq!(loaded_map.tell().expect("loaded map position"), map_bytes.len(), "load consumes the entire map archive");
    let resaved_map = ObjectArchive::saving(map_flags, &NullMapping);
    unsafe { access.serialize_object(restored_map, &resaved_map) }.expect("resave restored PackageMap");
    assert_eq!(resaved_map.into_written_bytes().expect("restored map archive"), map_bytes, "map wire state survives a save/load/save cycle");


    // A rooted instance survives collection; an unrooted one is destroyed and its canonical index reused.
    unsafe { access.add_root(simple); }
    let simple_index = unsafe { (*simple).index };
    let candidate_name = access.intern_str("GcCandidate", FindName::Add).expect("candidate name");
    let doomed = unsafe { access.construct_object(object_class, access.transient_package(), candidate_name, 0x0000_4000, std::ptr::null_mut()) }
        .expect("construct GC candidate");
    let doomed_index = unsafe { (*doomed).index };
    RELEASED_STATE_FRAMES.store(0, Ordering::SeqCst);
    let mut same_address = doomed;
    for generation in 1..=3 {
        let frame = unsafe { hp2_core::vm_frame::allocate_state(same_address.cast(), object_class.cast()) };
        unsafe { (*same_address).state_frame = frame.cast(); (*same_address).object_flags |= 0x0200_0000; }
        TRACKED_STATE_FRAME.store(frame as usize, Ordering::SeqCst);
        same_address = unsafe { access.construct_object(object_class, access.transient_package(), candidate_name, 0x0000_4000, std::ptr::null_mut()) }
            .expect("replace stateful canonical object identity");
        assert_eq!(same_address, doomed, "replacement retains the canonical allocation address");
        assert_eq!(unsafe { (*same_address).index }, doomed_index, "replacement retains the canonical index");
        assert_eq!(RELEASED_STATE_FRAMES.load(Ordering::SeqCst), generation, "replacement releases every owned execution frame exactly once");
        assert!(unsafe { (*same_address).state_frame.is_null() }, "replacement starts without a stale frame");
    }
    unsafe { access.add_root(same_address); }
    access.collect_garbage(0, 0, &|_event: GcLog| Ok(())).expect("collect with both instances rooted");
    assert_eq!(access.object_at(doomed_index), same_address, "rooted replacement survives collection");
    access.remove_root(same_address);
    access.collect_garbage(0, 0, &|_event: GcLog| Ok(())).expect("collect actual owned object table");
    assert_eq!(access.object_at(simple_index), simple, "rooted canonical commandlet identity survives");
    assert!(access.object_at(doomed_index).is_null(), "unrooted object is destroyed");
    // The unrooted object's FName was collectible too; a local integer is not a GC root.
    let candidate_name = access.intern_str("GcCandidate", FindName::Add).expect("re-intern collected candidate name");
    let replacement = unsafe { access.construct_object(object_class, access.transient_package(), candidate_name, 0x0000_4000, std::ptr::null_mut()) }
        .expect("construct replacement at freed canonical identity");
    assert_eq!(unsafe { (*replacement).index }, doomed_index, "free slot index reused canonically");
    access.remove_root(simple);
    owner.shutdown().expect("shutdown direct Core owner");

    let strict_owner = CoreOwner::new(CoreOptions {
        data_root: data_root.clone(), system_ini: system_ini.clone(),
        user_ini: data_root.join("System/User.ini"), command_line: utf32("-STRICT"),
        output, log_file: None,
    }).expect("initialize strict Core owner");
    let strict = strict_owner.access();
    strict.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..strict.process_mode() });
    let strict_class = unsafe { strict.static_load_class(strict.native_class("Commandlet").expect("strict Commandlet"), std::ptr::null_mut(), &simple_name, 1) }
        .expect("load actual SimpleCommandlet for strict diagnostics");
    let strict_object = unsafe { strict.construct_object(strict_class, strict.transient_package(), 0, 0x4000, std::ptr::null_mut()) }.expect("strict receiver");
    let strict_warn = find_function(strict, strict.native_class("Object").expect("strict Object").cast(), "Warn").expect("strict Warn");
    CAPTURE_OUTPUT.store(true, Ordering::SeqCst);
    let failure = run_probe(strict, strict_object, native_string_call(strict_warn, "strict diagnostic", None), std::ptr::from_mut(&mut native_result).cast())
        .expect_err("STRICT promotes the frame diagnostic to an execution error");
    assert!(failure.to_string().contains(") strict diagnostic"));
    assert!(captured_output().is_empty(), "a promoted fatal diagnostic is not also emitted as a nonfatal warning");
    CAPTURE_OUTPUT.store(false, Ordering::SeqCst);
    strict_owner.shutdown().expect("shutdown strict Core owner");
    hp2_core::ansi_trace::flush_process_state();
}

#[test]
fn core_package_save_acceptance() {
    package_save_roundtrip();
}

#[test]
fn selected_object_loading_defers_unrequested_exports_until_explicit_package_load() {
    use hp2_core::core_records::TextBufferRecord;
    let (data_root, system_ini) = isolated_data_root();
    let path = data_root.join("System/LazyObjects.u");
    let source_owner = owner(&data_root, &system_ini);
    let source = source_owner.access();
    let package = unsafe {
        source.construct_object(source.native_class("Core.Package").unwrap(), std::ptr::null_mut(),
            source.intern_str("LazyObjects", FindName::Add).unwrap(), 4, std::ptr::null_mut())
    }.unwrap();
    let mut outer = package;
    for name in ["Group", "Nested"] {
        outer = unsafe {
            source.construct_object(source.native_class("Core.Package").unwrap(), outer,
                source.intern_str(name, FindName::Add).unwrap(), 4, std::ptr::null_mut())
        }.unwrap();
    }
    let class = source.native_class("Core.TextBuffer").unwrap();
    let mut selected = std::ptr::null_mut();
    for (name, text) in [("Selected", "selected payload Ω\0"), ("Unrequested", "unrequested payload\0")] {
        let object = unsafe {
            source.construct_object(class, outer, source.intern_str(name, FindName::Add).unwrap(),
                4, std::ptr::null_mut())
        }.unwrap();
        unsafe {
            (*object).object_flags &= !0x4000;
            source.assign_fstring(&mut (*object.cast::<TextBufferRecord>()).text, &utf32(text)).unwrap();
        }
        if name == "Selected" { selected = object; }
    }
    assert!(unsafe { source.save_package(package, selected, 4, &path, None) }.unwrap());
    source_owner.shutdown().unwrap();

    // StaticLoadObject's ResolveName(Create=1) permits absent intermediate
    // groups; the linker then searches by class/name across actual groups.
    for object_path in ["LazyObjects.Selected", "LazyObjects.Group.Nested.Selected", "LazyObjects.Absent.Selected", "LazyObjects.Absent.Nested.Selected"] {
        let restored_owner = owner(&data_root, &system_ini);
        let restored = restored_owner.access();
        let class = restored.native_class("Core.TextBuffer").unwrap();
        let selected = unsafe {
            restored.static_load_object(class, std::ptr::null_mut(), &utf32(object_path), 1)
        }.unwrap().cast::<TextBufferRecord>();
        let text = unsafe { (*selected).text };
        assert_eq!(unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) },
            utf32("selected payload Ω\0"));
        let package = unsafe { (*selected).base.outer.cast::<UObject>() };
        let unrequested_name = restored.intern_str("Unrequested", FindName::Find).unwrap();
        assert!(unsafe { restored.find_object(class, package, unrequested_name, true) }.is_null(),
            "selected-object loading must not construct unrelated exports");

        unsafe { restored.load_package(&path, 1) }.unwrap();
        let unrequested = unsafe { restored.find_object(class, package, unrequested_name, true) };
        assert!(!unrequested.is_null(), "explicit whole-package loading must promote the cached linker");
        let text = unsafe { (*unrequested.cast::<TextBufferRecord>()).text };
        assert_eq!(unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) },
            utf32("unrequested payload\0"));
        restored_owner.shutdown().unwrap();
    }
}

#[test]
fn missing_safe_replace_import_uses_metadata_only_nested_package_root() {
    use hp2_abi::engine::generated::{CORE_LINKER_CONSTANTS, UStruct};
    use hp2_core::core_records::TextBufferRecord;

    let (data_root, system_ini) = isolated_data_root();
    let producer_path = data_root.join("System/OptionalResource.u");
    let consumer_path = data_root.join("System/OptionalConsumer.u");
    let source_owner = owner(&data_root, &system_ini);
    let source = source_owner.access();
    let constants = CORE_LINKER_CONSTANTS;
    let package_class = source.native_class("Package").unwrap();
    let producer = unsafe {
        source.construct_object(
            package_class,
            std::ptr::null_mut(),
            source.intern_str("OptionalResource", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    let nested = unsafe {
        source.construct_object(
            package_class,
            producer,
            source.intern_str("Nested", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    let text_buffer_class = source.native_class("TextBuffer").unwrap();
    let optional = unsafe {
        source.construct_object(
            text_buffer_class,
            nested,
            source.intern_str("Optional", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    unsafe {
        source.assign_fstring(
            &mut (*optional.cast::<TextBufferRecord>()).text,
            &utf32("optional payload\0"),
        )
    }
    .unwrap();
    let sibling = unsafe {
        source.construct_object(
            text_buffer_class,
            nested,
            source.intern_str("Retained", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    unsafe {
        source.assign_fstring(
            &mut (*sibling.cast::<TextBufferRecord>()).text,
            &utf32("retained payload\0"),
        )
    }
    .unwrap();

    let consumer = unsafe {
        source.construct_object(
            package_class,
            std::ptr::null_mut(),
            source.intern_str("OptionalConsumer", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    let consumer_struct = unsafe {
        source.construct_object(
            source.native_class("Struct").unwrap(),
            consumer,
            source.intern_str("ConsumerStruct", FindName::Add).unwrap(),
            constants.public_flag,
            std::ptr::null_mut(),
        )
    }
    .unwrap();
    unsafe {
        (*consumer_struct.cast::<UStruct>()).script_text = optional.cast();
    }
    assert!(unsafe {
        source.save_package(
            consumer,
            consumer_struct,
            constants.public_flag,
            &consumer_path,
            None,
        )
    }
    .unwrap());

    unsafe { (*optional).object_flags &= !constants.public_flag; }
    assert!(unsafe {
        source.save_package(
            producer,
            producer,
            constants.public_flag,
            &producer_path,
            None,
        )
    }
    .unwrap());
    source_owner.shutdown().unwrap();

    let safe_owner = owner(&data_root, &system_ini);
    let safe = safe_owner.access();
    let text_buffer_class = safe.native_class("TextBuffer").unwrap();
    unsafe { (*text_buffer_class).class_flags |= constants.safe_replace; }
    let loaded = unsafe {
        safe.static_load_object(
            safe.native_class("Struct").unwrap(),
            std::ptr::null_mut(),
            &utf32("OptionalConsumer.ConsumerStruct"),
            1,
        )
    }
    .unwrap()
    .cast::<UStruct>();
    assert!(!loaded.is_null());
    assert!(
        unsafe { (*loaded).script_text.is_null() },
        "a missing SafeReplace TextBuffer import must resolve to null"
    );

    safe_owner.shutdown().unwrap();

    let strict_owner = owner(&data_root, &system_ini);
    let strict = strict_owner.access();
    let text_buffer_class = strict.native_class("TextBuffer").unwrap();
    unsafe { (*text_buffer_class).class_flags &= !constants.safe_replace; }
    let error = unsafe {
        strict.static_load_object(
            strict.native_class("Struct").unwrap(),
            std::ptr::null_mut(),
            &utf32("OptionalConsumer.ConsumerStruct"),
            1,
        )
    }
    .expect_err("a missing non-SafeReplace resource remains an import error");
    assert!(error.to_string().contains("Unresolved package import"), "{error}");
    strict_owner.shutdown().unwrap();
}

fn package_save_roundtrip() {
    use hp2_core::package_archive::OwnedPackage;
    let (data_root, system_ini) = isolated_data_root();
    let saved_path = data_root.join("System/SavedCore.u");
    let source_owner = owner(&data_root, &system_ini);
    let access = source_owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    let commandlet = access.native_class("Commandlet").expect("Commandlet class");
    let hello_class = unsafe {
        access.static_load_class(commandlet, std::ptr::null_mut(), &utf32("Core.HelloWorldCommandlet"), 1)
    }.expect("load prototype HelloWorld class");
    let package = unsafe {
        access.construct_object(access.native_class("Package").expect("Package class"), std::ptr::null_mut(),
            access.intern_str("SavedCore", FindName::Add).expect("package name"), 4, std::ptr::null_mut())
    }.expect("construct saved package");
    let hello = unsafe {
        access.construct_object(hello_class, package,
            access.intern_str("SavedHello", FindName::Add).expect("instance name"), 4, std::ptr::null_mut())
    }.expect("construct saved commandlet");
    let int_property = find_field(access, hello_class.cast(), "intparm").expect("intparm");
    let text_property = find_field(access, hello_class.cast(), "strparm").expect("strparm");
    let mut text = utf32("persisted Ω text\0");
    let text_value = TArrayRaw { data: text.as_mut_ptr().cast(), num: text.len() as i32, max: text.len() as i32 };
    unsafe {
        // Commandlet classes are transient; explicitly opt this fixture instance into persistence.
        (*hello).object_flags &= !0x4000;
        access.property_address(hello, int_property).expect("int address").cast::<i32>().write(73);
        access.property_value(Hp2ValueOperation::ValueCopyComplete, text_property,
            access.property_address(hello, text_property).expect("text address").cast(),
            std::ptr::from_ref(&text_value).cast_mut().cast(), 0, None)
    }.expect("set saved string");
    let structure = unsafe {
        access.construct_object(access.native_class("Struct").expect("Struct class"), package,
            access.intern_str("SavedStruct", FindName::Add).expect("struct name"), 4, std::ptr::null_mut())
    }.expect("construct saved struct").cast::<UStruct>();
    let peer = unsafe {
        access.construct_object(access.native_class("IntProperty").expect("IntProperty class"), structure.cast(),
            access.intern_str("SavedPeer", FindName::Add).expect("peer name"), 4, std::ptr::null_mut())
    }.expect("construct referenced peer");
    let friendly_name = access.intern_str("PersistentFriendlyName", FindName::Add).expect("friendly name");
    unsafe {
        (*structure).friendly_name = friendly_name.to_ne_bytes();
        (*structure).properties_align = 4;
        (*structure.cast::<hp2_abi::engine::generated::UField>()).next = peer.cast();
    }
    assert!(unsafe { access.save_package(package, hello, 4, &saved_path, None) }.expect("save Rust-owned graph"));
    let original = OwnedPackage::open(&saved_path).expect("parse saved package");
    assert!(!data_root.join("System/Save.tmp").exists(), "successful commit retires temporary file");
    let saved_friendly = original.names().iter().find(|name| name.text == utf32("PersistentFriendlyName")).expect("saved friendly name");
    assert_eq!(saved_friendly.flags & 0x70000, 0x70000, "referenced name retains all export load contexts");

    let missing = data_root.join("missing/Failed.u");
    assert!(unsafe { access.save_package(package, hello, 4, &missing, None) }.is_err());
    assert!(!missing.exists() && !data_root.join("missing/Save.tmp").exists(), "create failure publishes nothing");
    let blocked = data_root.join("System/Blocked.u");
    fs::create_dir(&blocked).expect("make destination unreplaceable");
    assert!(!unsafe { access.save_package(package, hello, 4, &blocked, None) }.expect("commit failure returns false"));
    assert!(blocked.is_dir() && !data_root.join("System/Save.tmp").exists(), "commit failure preserves destination and removes temp");
    unsafe { (*structure).friendly_name = i32::MAX.to_ne_bytes(); }
    let invalid = data_root.join("System/InvalidName.u");
    assert!(unsafe { access.save_package(package, hello, 4, &invalid, None) }.is_err(), "invalid referenced names fail traversal");
    assert!(!invalid.exists() && !data_root.join("System/Save.tmp").exists(), "traversal failure removes incomplete archive");
    unsafe { (*structure).friendly_name = friendly_name.to_ne_bytes(); }
    let private_field = unsafe {
        access.construct_object(access.native_class("Struct").expect("private Struct class"), access.transient_package(),
            access.intern_str("PrivateDependency", FindName::Add).expect("private dependency name"), 0, std::ptr::null_mut())
    }.expect("construct private external reference");
    unsafe { (*structure.cast::<hp2_abi::engine::generated::UField>()).next = private_field.cast(); }
    let original_bytes = fs::read(&saved_path).expect("retain committed package");
    unsafe { access.save_package(package, hello, 4, &saved_path, None) }.expect_err("external private dependency rejects save");
    assert_eq!(fs::read(&saved_path).expect("preserved package"), original_bytes, "failed traversal preserves committed package");
    assert!(!data_root.join("System/Save.tmp").exists(), "private-import failure retires temporary archive");
    let transient_dependency = unsafe {
        access.construct_object(access.native_class("Struct").expect("transient Struct class"),
            access.transient_package(),
            access.intern_str("TransientDependency", FindName::Add).expect("transient dependency name"),
            0x4000, std::ptr::null_mut())
    }.expect("construct transient private reference");
    unsafe { (*structure.cast::<hp2_abi::engine::generated::UField>()).next = transient_dependency.cast(); }
    let transient_reference_path = data_root.join("System/TransientReference.u");
    assert!(unsafe { access.save_package(package, hello, 4, &transient_reference_path, None) }
        .expect("transient private object references serialize as null"));
    let transient_saved = OwnedPackage::open(&transient_reference_path).expect("parse transient-reference package");
    assert!(!transient_saved.imports().iter().any(|import|
        import.object_name == access.intern_str("TransientDependency", FindName::Find).expect("transient name")),
        "transient private dependency is not emitted as a package import");
    unsafe { (*structure.cast::<hp2_abi::engine::generated::UField>()).next = peer.cast(); }
    source_owner.shutdown().expect("close saving owner");

    let restored_owner = owner(&data_root, &system_ini);
    let restored = restored_owner.access();
    restored.set_process_mode(CoreProcessMode { is_scriptable: true, ..restored.process_mode() });
    let transient_structure = unsafe {
        restored.static_load_object(restored.native_class("Struct").expect("transient-reference Struct"),
            std::ptr::null_mut(), &utf32("TransientReference.SavedStruct"), 1)
    }.expect("reload transient-reference package").cast::<hp2_abi::engine::generated::UField>();
    assert!(unsafe { (*transient_structure).next.is_null() },
        "an omitted transient private reference reloads as null, not an unrelated import");
    let loaded = unsafe {
        restored.static_load_object(restored.native_class("Commandlet").expect("restored Commandlet"),
            std::ptr::null_mut(), &utf32("SavedCore.SavedHello"), 1)
    }.expect("reload saved commandlet in fresh owner");
    assert!(!loaded.is_null());
    let loaded_class = unsafe { (*loaded).class.cast() };
    let int_property = find_field(restored, loaded_class, "intparm").expect("restored intparm");
    let text_property = find_field(restored, loaded_class, "strparm").expect("restored strparm");
    assert_eq!(unsafe { restored.property_address(loaded, int_property).expect("restored scalar").cast::<i32>().read() }, 73);
    let text = unsafe { restored.property_address(loaded, text_property).expect("restored text").cast::<TArrayRaw>().read() };
    assert!(text.num > 0 && !text.data.is_null(), "restored string has terminated storage");
    let text = unsafe { std::slice::from_raw_parts(text.data.cast::<u32>(), text.num as usize) };
    assert_eq!(text, utf32("persisted Ω text\0"));
    let loaded_structure = unsafe {
        restored.static_load_object(restored.native_class("Struct").expect("restored Struct"),
            std::ptr::null_mut(), &utf32("SavedCore.SavedStruct"), 1)
    }.expect("reload saved struct").cast::<UStruct>();
    let loaded_peer = unsafe { (*loaded_structure.cast::<hp2_abi::engine::generated::UField>()).next.cast::<UObject>() };
    assert!(!loaded_peer.is_null(), "mapped object reference survives fresh-owner reload");
    assert_eq!(restored.name_text(i32::from_ne_bytes(unsafe { (*loaded_peer).name })).expect("peer name"), utf32("SavedPeer"));
    assert_eq!(restored.name_text(i32::from_ne_bytes(unsafe { (*loaded_structure).friendly_name })).expect("mapped friendly name"), utf32("PersistentFriendlyName"));
    let package = unsafe { (*loaded).outer.cast::<UObject>() };
    unsafe { (*loaded_structure.cast::<hp2_abi::engine::generated::UField>()).next = std::ptr::null_mut(); }
    unsafe {
        (*loaded).object_flags &= !0x4000;
        (*loaded_peer).object_flags |= 0x4000;
        restored.construct_object(restored.native_class("Object").expect("restored Object"), package,
            restored.intern_str("AddedLater", FindName::Add).expect("new export name"), 4, std::ptr::null_mut())
    }.expect("append new conform candidate");
    assert!(unsafe { restored.save_package(package, loaded, 4, &saved_path, Some(&original)) }.expect("save conform graph"));
    let conformed = OwnedPackage::open(&saved_path).expect("parse conformed package");
    assert_eq!(conformed.summary().guid, original.summary().guid, "conform retains package identity");
    assert_eq!(conformed.summary().generations.len(), original.summary().generations.len() + 1);
    for (index, old) in original.exports().iter().enumerate() {
        let old_name = &original.names()[old.object_name as usize].text;
        let current = &conformed.exports()[index];
        if old_name == &utf32("SavedPeer") {
            assert_eq!((current.serial_size, current.serial_offset), (0, 0), "deleted conform slot stays empty");
        } else {
            assert_eq!(&conformed.names()[current.object_name as usize].text, old_name, "matched conform slot retains identity");
        }
    }
    restored_owner.shutdown().expect("close conform owner");
    let final_owner = owner(&data_root, &system_ini);
    let final_access = final_owner.access();
    let added = unsafe {
        final_access.static_load_object(final_access.native_class("Object").expect("final Object"),
            std::ptr::null_mut(), &utf32("SavedCore.AddedLater"), 1)
    }.expect("load appended export from conformed package");
    assert!(!added.is_null());
    final_owner.shutdown().expect("close fresh conform consumer");
}

#[test]
fn external_native_class_uses_canonical_package_and_layout() {
    let (data_root, system_ini) = isolated_data_root();
    std::fs::copy(
        std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
            .expect("prototype data root").join("System/Engine.u"),
        data_root.join("System/Engine.u"),
    ).expect("copy the real external class package");
    let saved_path = data_root.join("System/SavedEngine.u");
    let user_ini = data_root.join("System/User.ini");
    EXTERNAL_DESTROYS.store(0, Ordering::SeqCst);
    EXTERNAL_FINALIZATIONS.store(0, Ordering::SeqCst);
    EXTERNAL_REENTRANT_DELETE.store(false, Ordering::SeqCst);
    let rejected = unsafe {
        CoreOwner::with_native_classes(CoreOptions {
            data_root: data_root.clone(),
            system_ini: system_ini.clone(),
            user_ini: user_ini.clone(),
            command_line: Vec::new(),
            output,
            log_file: None,
        }, INVALID_QUALIFIED_EDGE)
    };
    assert!(rejected.is_err(), "qualified missing parents must not fall back to Core names");
    let owner = unsafe {
        CoreOwner::with_native_classes(CoreOptions {
            data_root: data_root.clone(),
            system_ini: system_ini.clone(),
            user_ini: user_ini.clone(),
            command_line: Vec::new(),
            output,
            log_file: None,
        }, EXTERNAL_CLASSES)
    }.expect("initialize external native class in Core's canonical object graph");
    let access = owner.access();
    let class = access.native_class("Engine.ExternalObject").expect("qualified Engine class");
    let superclass = access.native_class("Object").expect("Core Object superclass");
    assert_eq!(unsafe { (*class.cast::<hp2_abi::engine::generated::UField>()).super_field }, superclass.cast());
    assert!(access.native_class("ExternalObject").is_err(), "unqualified lookup remains Core-only");
    let package = unsafe {
        access.construct_object(access.native_class("Package").expect("Package class"), std::ptr::null_mut(),
            access.intern_str("SavedEngine", FindName::Add).expect("package name"), 4, std::ptr::null_mut())
    }.expect("construct saved package");
    let object = unsafe {
        access.construct_object(class, package,
            access.intern_str("ExternalInstance", FindName::Add).expect("instance name"),
            0x4000, std::ptr::null_mut())
    }.expect("construct external reflected object");
    assert!(unsafe { hp2_core::object_model::is_a(object.cast(), superclass.cast()) });
    let external = object.cast::<ExternalObjectRecord>();
    unsafe {
        (*object).object_flags &= !0x4000;
        (*external).payload = 0x2110_2026_1029;
        (*external).collision_bits = 0x8000_0000 | 64 | 256;
    }
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    let property = find_field(access, class.cast(), "bAlignBottom").expect("actual packed bool property");
    let property_index = unsafe { (*property.cast::<UObject>()).index };
    for (framed, literal, expected_bits, expected_return) in [
        (false, 0x27, 0x8000_0000 | 64 | 128 | 256, 1_u32),
        (true, 0x28, 0x8000_0000 | 128 | 256, 0_u32),
    ] {
        if framed {
            unsafe {
                access.initialize_execution(object).expect("initialize live receiver state");
                (*(*object).state_frame.cast::<hp2_abi::engine::generated::FStateFrame>()).probe_mask = 0;
            }
        }
        unsafe { (*external).collision_bits = 0x8000_0000 | 128 | 256 | 64; }
        let mut code = vec![0x14, 0x2d, 0x01]; // LetBool, BoolVariable, InstanceVariable.
        code.extend(property_index.to_le_bytes());
        code.extend([literal, 0x04, 0x2d, 0x01]); // RHS, Return, BoolVariable, InstanceVariable.
        code.extend(property_index.to_le_bytes());
        let mut returned = u32::MAX;
        run_probe(access, object, code, std::ptr::from_mut(&mut returned).cast())
            .expect("packed bool assignment and return consume exactly one property reference each");
        assert_eq!(unsafe { (*external).collision_bits }, expected_bits, "assignment preserves adjacent bools and unrelated bits");
        assert_eq!(returned, expected_return, "return resumes at the following bool expression");
        let defaults = unsafe { access.default_object(class) }.expect("live native class defaults").cast::<ExternalObjectRecord>();
        let saved_bits = unsafe { (*defaults).collision_bits };
        unsafe { (*defaults).collision_bits = expected_bits ^ 64; }
        let mut context_code = vec![0x04, 0x19, 0x20]; // Return, Context, ObjectConst.
        context_code.extend(unsafe { (*object).index }.to_le_bytes());
        context_code.extend([6, 0, 4, 0x2d, 0x01]); // Null skip/size, BoolVariable, InstanceVariable.
        context_code.extend(property_index.to_le_bytes());
        returned = u32::MAX;
        let outcome = run_probe(access, defaults.cast(), context_code, std::ptr::from_mut(&mut returned).cast());
        unsafe { (*defaults).collision_bits = saved_bits; }
        outcome.expect("nested context evaluates on the expression receiver, not the frame receiver");
        assert_eq!(returned, expected_return, "context reads the target instance even when the frame has the opposite bit");
    }
    unsafe { (*external).collision_bits = 0x8000_0000 | 64 | 256; }
    assert!(unsafe { access.save_package(package, object, 4, &saved_path, None) }.expect("save external Rust object"));

    let replacement_name = access.intern_str("ReplaceFinalizer", FindName::Add).expect("replacement name");
    let replaced = unsafe {
        access.construct_object(class, package, replacement_name, 0, std::ptr::null_mut())
    }.expect("construct replacement source");
    unsafe { (*replaced.cast::<ExternalObjectRecord>()).payload = EXTERNAL_PAYLOAD; }
    let replacement = unsafe {
        access.construct_object(class, package, replacement_name, 0, std::ptr::null_mut())
    }.expect("replace same-name canonical source");
    assert_eq!(replacement, replaced, "same-name replacement retains the owned allocation");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 1,
        "replacement finalizes the prior member tail exactly once");
    unsafe { (*replacement.cast::<ExternalObjectRecord>()).payload = 0; }
    owner.shutdown().expect("destroy source through supplied Rust operations");
    assert_eq!(EXTERNAL_DESTROYS.load(Ordering::SeqCst), 2, "source and replaced instances are destroyed exactly once");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 2,
        "shutdown finalizes source members after replacement finalized its prior allocation");

    let restored_owner = unsafe {
        CoreOwner::with_native_classes(CoreOptions {
            data_root,
            system_ini,
            user_ini,
            command_line: Vec::new(),
            output,
            log_file: None,
        }, EXTERNAL_CLASSES)
    }.expect("initialize matching external catalog in fresh owner");
    let restored = restored_owner.access();
    restored.set_process_mode(CoreProcessMode { is_client: true, ..restored.process_mode() });
    let restored_class = restored.native_class("Engine.ExternalObject").expect("restored qualified class");
    let metadata_flags = 1 | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.verify
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.no_verify;
    unsafe { restored.load_package(&saved_path.with_file_name("Engine.u"), metadata_flags) }
        .expect("open external namespace package without constructing unrelated classes");
    let loaded = unsafe {
        restored.static_load_object(restored.native_class("Object").expect("Object load base"),
            std::ptr::null_mut(), &utf32("SavedEngine.ExternalInstance"),
            metadata_flags)
    }.expect("load external class instance in fresh owner");
    let loaded_package = unsafe { (*loaded).outer.cast::<UObject>() };
    assert_ne!(
        unsafe { (*loaded_package).object_flags }
            & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.public_flag,
        0,
        "StaticLoadObject's canonical package namespace must retain CreatePackage's RF_Public",
    );
    assert_eq!(unsafe { (*loaded).class }, restored_class.cast(), "load resolves the canonical external class");
    assert_eq!(unsafe { (*loaded.cast::<ExternalObjectRecord>()).payload }, 0x2110_2026_1029);
    assert_eq!(unsafe { (*loaded.cast::<ExternalObjectRecord>()).collision_bits }, 64 | 256,
        "native bool masks sharing an offset must round-trip independently without serializing unrelated bits");
    let loaded_index = unsafe { (*loaded).index };
    let gc_package = unsafe { (*loaded).outer.cast::<UObject>() };
    assert!(unsafe { restored.conditional_destroy(loaded) }.expect("explicit class teardown"));
    assert_eq!(EXTERNAL_DESTROYS.load(Ordering::SeqCst), 3, "explicit teardown reaches the actual supplied class operation");
    assert!(!unsafe { restored.conditional_destroy(loaded) }.expect("repeat conditional teardown"),
        "conditional teardown is exactly once");
    assert_eq!(restored.object_at(loaded_index), loaded, "teardown retains identity until release");
    unsafe { restored.delete_object(loaded) }.expect("release already-destroyed canonical allocation");
    assert!(restored.object_at(loaded_index).is_null(), "explicit delete removes the canonical identity slot");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 3,
        "direct deletion finalizes members once after an earlier explicit Destroy");

    unsafe { restored.add_root(gc_package); }
    let gc_name = restored.intern_str("GcFinalizer", FindName::Add).expect("GC fixture name");
    let gc_object = unsafe {
        restored.construct_object(restored_class, gc_package, gc_name, 0, std::ptr::null_mut())
    }.expect("construct unrooted finalizer fixture");
    unsafe { (*gc_object.cast::<ExternalObjectRecord>()).payload = EXTERNAL_PAYLOAD; }
    let gc_index = unsafe { (*gc_object).index };
    restored.collect_garbage(0, 0, &|_event: GcLog| Ok(())).expect("collect finalizer fixture");
    assert!(restored.object_at(gc_index).is_null(), "GC deletes the unrooted canonical object");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 4,
        "GC runs the final member callback once");
    restored.remove_root(gc_package);

    let failed_constructor_slot = std::cell::Cell::new(std::ptr::null_mut());
    let failed_name = restored.intern_str("FailedConstructorFinalizer", FindName::Add).expect("failed constructor name");
    let failed_constructor = unsafe {
        restored.construct_object_with_constructor(
            restored_class, restored.transient_package(), failed_name, 0, std::ptr::null_mut(),
            |_, object| {
                unsafe { (*object.cast::<ExternalObjectRecord>()).payload = EXTERNAL_PAYLOAD; }
                failed_constructor_slot.set(object);
                Err(RuntimeError::new("Injected external constructor failure"))
            },
        )
    };
    assert!(failed_constructor.is_err(), "the failing constructor remains an observable allocation error");
    let failed_object = failed_constructor_slot.get();
    let failed_index = unsafe { (*failed_object).index };
    unsafe { restored.delete_object(failed_object) }.expect("delete partially constructed external object");
    assert!(restored.object_at(failed_index).is_null());
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 5,
        "partially constructed members finalize once on direct deletion");

    let reentrant_name = restored.intern_str("ReentrantFinalizer", FindName::Add).expect("reentrant finalizer name");
    let reentrant = unsafe {
        restored.construct_object(restored_class, restored.transient_package(), reentrant_name, 0, std::ptr::null_mut())
    }.expect("construct reentrant finalizer fixture");
    unsafe { (*reentrant.cast::<ExternalObjectRecord>()).payload = REENTRANT_FINALIZER_PAYLOAD; }
    let reentrant_index = unsafe { (*reentrant).index };
    assert!(unsafe { restored.delete_object(reentrant) }.is_err(),
        "the first finalizer error is returned after deletion still releases the slot");
    assert!(EXTERNAL_REENTRANT_DELETE.load(Ordering::SeqCst),
        "same-object reentrant deletion observes the in-progress finalizer without retrying it");
    assert!(restored.object_at(reentrant_index).is_null(), "finalizer failure does not leak the object allocation");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 6,
        "a failed reentrant finalizer is invoked exactly once");

    restored_owner.shutdown().expect("destroy remaining class and package storage");
    assert_eq!(EXTERNAL_DESTROYS.load(Ordering::SeqCst), 5, "delete and shutdown do not re-enter released class destructors");
    assert_eq!(EXTERNAL_FINALIZATIONS.load(Ordering::SeqCst), 6,
        "shutdown does not repeat final-member destruction for GC, replacement, delete, or errors");
}

#[test]
fn registered_canonical_natives_preserve_the_catalog_and_support_reentry() {
    use hp2_core::canonical_vm_native::{finish_arguments, VmNativeSpec};
    use hp2_core::core_records::TextBufferRecord;
    use hp2_core::array_storage::{self, Hp2ArraySlots};
    use hp2_abi::engine::generated::FFrame;

    fn execute_from_source(access: CoreAccess<'_>, object: *mut UObject, target: *mut UFunction, name: &str) -> RuntimeResult<()> {
        let wrapper_object = unsafe {
            access.construct_object(
                access.native_class("Function")?,
                (*object).class.cast(),
                access.intern_str(name, FindName::Add)?,
                0x4000,
                std::ptr::null_mut(),
            )?
        };
        let wrapper = wrapper_object.cast::<UFunction>();
        let structure = wrapper_object.cast::<UStruct>();
        let mut code = vec![0x1c];
        code.extend(unsafe { (*target.cast::<UObject>()).index }.to_le_bytes());
        code.extend([0x16, 0x08, 0x25]);
        unsafe {
            (*wrapper).i_native = 0;
            (*wrapper).function_flags = 0;
            (*wrapper).return_value_offset = u16::MAX;
            (*wrapper).parms_size = 0;
            (*structure).properties_size = 0;
            (*structure).properties_align = 1;
            let script = std::ptr::addr_of_mut!((*structure).script).cast::<TArrayRaw>();
            array_storage::set(Hp2ArraySlots {
                data: std::ptr::addr_of_mut!((*script).data),
                num: std::ptr::addr_of_mut!((*script).num),
                max: std::ptr::addr_of_mut!((*script).max),
            }, code.len() as i32, 1, std::ptr::null(), array_storage::system_allocator())
                .expect("allocate owned wrapper script");
            std::ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast::<u8>(), code.len());
            (*wrapper_object).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
            access.conditional_post_load(wrapper_object)?;
            access.execute_event(object, wrapper, std::ptr::null_mut(), std::ptr::null_mut())
        }
    }

    unsafe fn increment_position(
        access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame,
        _: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe { finish_arguments(access, frame)?; }
        let buffer = object.cast::<TextBufferRecord>();
        let position = unsafe { (*buffer).pos + 1 };
        unsafe { (*buffer).pos = position; }
        if position == 1 {
            let function = ACTIVE_REENTRY_FUNCTION.load(Ordering::SeqCst) as *mut UFunction;
            assert!(!function.is_null(), "reentry target is an active reflected function");
            execute_from_source(access, object, function, "NestedNativeCall")?;
        }
        Ok(())
    }

    static NATIVES: &[VmNativeSpec] = &[
        VmNativeSpec {
            class_name: "Core.TextBuffer", function_name: "IncrementPosition",
            native_index: Some(4094), handler: increment_position,
        },
        VmNativeSpec {
            class_name: "Core.TextBuffer", function_name: "NamedIncrementPosition",
            native_index: None, handler: increment_position,
        },
    ];
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifact directory"));
    let root = artifact.join(format!("core-external-native-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).expect("create isolated no-data runtime");
    let system_ini = root.join("System/Default.ini");
    fs::write(&system_ini, "").expect("write isolated config");
    fs::write(root.join("System/User.ini"), "").expect("write isolated user config");
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    access.register_vm_natives(NATIVES).expect("register actual Rust native handlers");
    assert!(access.register_vm_natives(&[]).is_err(), "refused replacement must retain the first catalog");
    let receiver_class = access.native_class("TextBuffer").expect("TextBuffer class");
    let receiver = unsafe {
        access.construct_object(receiver_class, access.transient_package(),
            access.intern_str("CanonicalNativeReceiver", FindName::Add).expect("receiver name"),
            0x4000, std::ptr::null_mut())
    }.expect("construct canonical receiver").cast::<TextBufferRecord>();
    let function_class = access.native_class("Function").expect("Function class");
    for (name, native_index, singular, code, expected) in [
        ("IncrementPosition", Some(4094), false, &[22][..], 2),
        ("CallIndexedNative", None, false, &[111, 254, 22, 4, 11][..], 2),
        ("NamedIncrementPosition", Some(0), false, &[22][..], 2),
        ("SingularCallIndexedNative", None, true, &[111, 254, 22, 4, 11][..], 1),
    ] {
        let object = unsafe {
            access.construct_object(function_class, receiver_class.cast(),
                access.intern_str(name, FindName::Add).expect("function name"),
                0x4000, std::ptr::null_mut())
        }.expect("construct canonical function");
        let function = object.cast::<UFunction>();
        let structure = object.cast::<UStruct>();
        unsafe {
            (*function).i_native = native_index.unwrap_or(0);
            (*function).function_flags = if native_index.is_some() { 0x400 } else { 0 };
            if singular {
                (*function).function_flags |= hp2_core::vm_dispatch::FUNC_SINGULAR;
            }
            (*function).return_value_offset = u16::MAX;
            (*function).parms_size = 0;
            (*structure).properties_size = 0;
            (*structure).properties_align = 1;
            let script = std::ptr::addr_of_mut!((*structure).script).cast::<TArrayRaw>();
            let slots = Hp2ArraySlots {
                data: std::ptr::addr_of_mut!((*script).data),
                num: std::ptr::addr_of_mut!((*script).num),
                max: std::ptr::addr_of_mut!((*script).max),
            };
            array_storage::set(slots, code.len() as i32, 1, std::ptr::null(),
                array_storage::system_allocator()).expect("own real executable bytecode");
            std::ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast::<u8>(), code.len());
            (*object).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
            access.conditional_post_load(object).expect("bind reflected native identity");
            (*receiver).pos = 0;
            if native_index == Some(4094) {
                access.execute_event(receiver.cast(), function, std::ptr::null_mut(), std::ptr::null_mut())
                    .expect("indexed native ProcessEvent route is rejected without failing");
                assert_eq!((*receiver).pos, 0,
                    "indexed native ProcessEvent rejection does not invoke its registered handler");
            }
            (*receiver).pos = 0;
            ACTIVE_REENTRY_FUNCTION.store(function as usize, Ordering::SeqCst);
            execute_from_source(access, receiver.cast(), function, "NativeSourceCall")
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!((*receiver).pos, expected, "{name} must preserve source reentry admission");
            if singular {
                execute_from_source(access, receiver.cast(), function, "SecondSingularSourceCall")
                    .expect("a completed singular call must permit a later independent call");
                assert_eq!((*receiver).pos, 2, "the successful singular call clears its object flag");
            }
        }
    }
    owner.shutdown().expect("destroy canonical receiver and executable functions");
}

#[test]
fn canonical_obj_list_counts_unchanged_strings_and_retained_capacity() {
    use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_records::SystemRecord};
    let artifacts = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"));
    let root = artifacts.join("canonical-console-memory");
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/Default.ini"), "").unwrap();
    fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }).unwrap();
    let access = owner.access();
    let command = "SET Core.System SavePath CanonicalMemoryCountSmoke".chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    assert!(unsafe { access.execute_static_command(command.as_ptr(), &mut |_| Ok(())) }.unwrap());
    let class = access.native_class("Core.System").unwrap();
    let name = access.intern_str("CountMemTarget", FindName::Add).unwrap();
    let system = unsafe {
        access.construct_object(class, access.transient_package(), name, 0, std::ptr::null_mut())
    }.unwrap().cast::<SystemRecord>();
    let mut expected = (std::mem::size_of::<SystemRecord>(), std::mem::size_of::<SystemRecord>());
    unsafe {
        // Reserve real storage without changing the inherited default string.
        // Num and Max must differ, so counting only live text cannot pass.
        let raw = std::ptr::addr_of_mut!((*system).save_path);
        let old_num = array_storage::add(Hp2ArraySlots {
            data: std::ptr::addr_of_mut!((*raw).data),
            num: std::ptr::addr_of_mut!((*raw).num),
            max: std::ptr::addr_of_mut!((*raw).max),
        }, 1, 4, std::ptr::null(), array_storage::system_allocator()).unwrap();
        (*raw).num = old_num;
        for string in [(*system).save_path, (*system).save_slot_path, (*system).map_path, (*system).cache_path, (*system).cache_ext] {
            expected.0 += string.num as usize * 4;
            expected.1 += string.max as usize * 4;
        }
        let paths = (*system).paths;
        expected.0 += paths.num as usize * std::mem::size_of::<TArrayRaw>();
        expected.1 += paths.max as usize * std::mem::size_of::<TArrayRaw>();
        for index in 0..paths.num as usize {
            let string = paths.data.cast::<TArrayRaw>().add(index).read();
            expected.0 += string.num as usize * 4;
            expected.1 += string.max as usize * 4;
        }
        expected.0 += (*system).suppress.num as usize * 4;
        expected.1 += (*system).suppress.max as usize * 4;
    }
    let mut lines = Vec::new();
    let command = "OBJ LIST CLASS=Core.System".chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    assert!(unsafe { access.execute_static_command(command.as_ptr(), &mut |line| {
        lines.push(line.iter().map(|&unit| char::from_u32(unit).unwrap()).collect::<String>());
        Ok(())
    }) }.unwrap());
    fs::write(root.join("report.txt"), format!(
        "data_profile=data-none\nexpected_num={}\nexpected_max={}\n{}\n",
        expected.0, expected.1, lines.join("\n"),
    )).unwrap();
    let row = lines.iter().find(|line| line.contains("CountMemTarget")).expect("actual object row");
    let mut columns = row.split_whitespace().rev();
    let max: usize = columns.next().unwrap().parse().unwrap();
    let num: usize = columns.next().unwrap().parse().unwrap();
    assert_eq!((num, max), expected, "OBJ LIST must count live buffers, not saving output");
    owner.shutdown().unwrap();
}

#[test]
fn canonical_foreach_evaluates_arguments_once_and_preserves_enclosing_control() {
    use hp2_abi::engine::generated::FFrame;
    use hp2_core::{
        array_storage::{self, Hp2ArraySlots},
        canonical_vm_native::{finish_arguments, run_iterator, step_argument, VmNativeSpec},
        core_records::TextBufferRecord,
    };

    unsafe fn count(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            finish_arguments(access, frame)?;
            (*object.cast::<TextBufferRecord>()).top += 1;
            result.cast::<i32>().write_unaligned(3);
        }
        Ok(())
    }
    unsafe fn advance(access: CoreAccess<'_>, object: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            finish_arguments(access, frame)?;
            (*object.cast::<TextBufferRecord>()).pos += 1;
        }
        Ok(())
    }
    unsafe fn range(access: CoreAccess<'_>, _: *mut UObject, frame: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> {
        let mut count = 0_i32;
        unsafe {
            step_argument(access, frame, (&raw mut count).cast())?;
            finish_arguments(access, frame)?;
        }
        let mut next = 0;
        unsafe { run_iterator(access, frame, || {
            let yielded = next < count;
            next += 1;
            Ok(yielded)
        }) }
    }
    static NATIVES: &[VmNativeSpec] = &[
        VmNativeSpec { class_name: "Core.TextBuffer", function_name: "Count", native_index: Some(4092), handler: count },
        VmNativeSpec { class_name: "Core.TextBuffer", function_name: "Advance", native_index: Some(4093), handler: advance },
        VmNativeSpec { class_name: "Core.TextBuffer", function_name: "Range", native_index: Some(4094), handler: range },
    ];
    fn iteration(code: &mut Vec<u8>, count: &[u8], body: impl FnOnce(&mut Vec<u8>)) -> usize {
        code.extend([0x2f, 0x6f, 0xfe]); // Iterator marker, Range.
        code.extend(count);
        code.push(0x16);
        let end_word = code.len();
        code.extend([0, 0]);
        body(code);
        code.push(0x31);
        let pop = code.len();
        code.push(0x30);
        code[end_word..end_word + 2].copy_from_slice(&(pop as u16).to_le_bytes());
        pop
    }
    fn return_int(code: &mut Vec<u8>, value: i32) {
        code.extend([0x04, 0x1d]);
        code.extend(value.to_le_bytes());
    }
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifact directory"))
        .join(format!("canonical-foreach-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/Default.ini"), "").unwrap();
    fs::write(root.join("System/User.ini"), "").unwrap();
    let owner = owner(&root, &root.join("System/Default.ini"));
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    access.register_vm_natives(NATIVES).unwrap();
    let receiver_class = access.native_class("TextBuffer").unwrap();
    let receiver = unsafe { access.construct_object(receiver_class, access.transient_package(), 0, 0x4000, std::ptr::null_mut()) }
        .unwrap().cast::<TextBufferRecord>();
    let mut cases = Vec::new();
    let count = [0x6f, 0xfc, 0x16];
    let advance = [0x6f, 0xfd, 0x16];
    let mut code = Vec::new();
    iteration(&mut code, &count, |body| body.extend(advance));
    return_int(&mut code, 7);
    cases.push(("Exhaustion", code, 3, 1, 7));
    let mut code = Vec::new();
    iteration(&mut code, &[0x25], |body| body.extend(advance)); // IntZero.
    return_int(&mut code, 7);
    cases.push(("Empty", code, 0, 0, 7));
    let mut code = Vec::new();
    let mut break_word = 0;
    let pop = iteration(&mut code, &count, |body| {
        body.extend(advance);
        body.push(0x06); // Source break jumps to the iterator's Pop.
        break_word = body.len();
        body.extend([0, 0]);
    });
    code[break_word..break_word + 2].copy_from_slice(&(pop as u16).to_le_bytes());
    return_int(&mut code, 7);
    cases.push(("Break", code, 1, 1, 7));
    let mut code = Vec::new();
    iteration(&mut code, &count, |body| {
        iteration(body, &[0x2c, 2], |inner| inner.extend(advance)); // IntConstByte(2).
    });
    return_int(&mut code, 7);
    cases.push(("Nested", code, 6, 1, 7));
    let mut code = Vec::new();
    iteration(&mut code, &count, |body| {
        iteration(body, &[0x2c, 2], |inner| {
            inner.extend(advance);
            return_int(inner, 42);
        });
    });
    return_int(&mut code, 7);
    cases.push(("NestedReturn", code, 1, 1, 42));
    let mut code = Vec::new();
    iteration(&mut code, &count, |body| { body.extend(advance); body.push(0x08); });
    return_int(&mut code, 7);
    cases.push(("Stop", code, 1, 1, -99));
    for (name, code, expected_position, expected_evaluations, expected_return) in cases {
        let function = unsafe { access.construct_object(
            access.native_class("Function").unwrap(), receiver_class.cast(),
            access.intern_str(name, FindName::Add).unwrap(), 0x4000, std::ptr::null_mut(),
        ) }.unwrap().cast::<UFunction>();
        unsafe {
            (*function).i_native = 0;
            (*function).function_flags = 0;
            (*function).return_value_offset = u16::MAX;
            (*function).parms_size = 0;
            let structure = function.cast::<UStruct>();
            (*structure).properties_size = 0;
            (*structure).properties_align = 1;
            let script = std::ptr::addr_of_mut!((*structure).script).cast::<TArrayRaw>();
            array_storage::set(Hp2ArraySlots {
                data: std::ptr::addr_of_mut!((*script).data), num: std::ptr::addr_of_mut!((*script).num),
                max: std::ptr::addr_of_mut!((*script).max),
            }, code.len() as i32, 1, std::ptr::null(), array_storage::system_allocator()).unwrap();
            std::ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast(), code.len());
            (*receiver).top = 0;
            (*receiver).pos = 0;
            let mut returned = -99_i32;
            access.execute_event(receiver.cast(), function, std::ptr::null_mut(), (&raw mut returned).cast())
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!((*receiver).pos, expected_position, "{name}: body control");
            assert_eq!((*receiver).top, expected_evaluations, "{name}: argument side effects");
            assert_eq!(returned, expected_return, "{name}: enclosing return destination");
        }
    }
    owner.shutdown().unwrap();
}


#[test]
fn probe_mask_preserves_ordinary_names_and_frameless_receivers() {
    use hp2_abi::engine::generated::FStateFrame;
    let (root, system_ini) = isolated_data_root();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    let object = unsafe {
        access.construct_object(
            access.native_class("TextBuffer").expect("concrete TextBuffer class"),
            access.transient_package(),
            access.intern_str("ProbePolicyObject", FindName::Add).expect("object name"),
            0x4000,
            std::ptr::null_mut(),
        )
    }.expect("construct live probe receiver");
    let ordinary = access.intern_str("OrdinaryProbePolicyEvent", FindName::Add).unwrap();
    let spawned = access.intern_str("Spawned", FindName::Find).unwrap();
    unsafe {
        assert!(access.is_probing(object, ordinary).unwrap());
        assert!(access.is_probing(object, spawned).unwrap(),
            "frameless receivers accept source probe names");
        access.initialize_execution(object).expect("initialize live state frame");
        let frame = (*object).state_frame.cast::<FStateFrame>();
        (*frame).probe_mask = 0;
        assert!(access.is_probing(object, ordinary).unwrap(),
            "a disabled probe mask must not suppress ordinary events");
        assert!(!access.is_probing(object, spawned).unwrap());
        (*frame).probe_mask = 1;
        assert!(access.is_probing(object, spawned).unwrap(),
            "the source Spawned probe occupies the first mask bit");
    }
    owner.shutdown().expect("destroy live probe records");
}

#[test]
fn application_system_validates_paths_and_applies_only_configured_name_flags() {
    let (root, system_ini) = isolated_data_root();
    for (path_count, configured, valid) in [(5, true, false), (6, true, true), (6, false, true)] {
        let mut configuration = String::from("[Core.System]\n");
        for directory in ["System", "Maps", "Textures", "Sounds", "Music", "Save"].into_iter().take(path_count) {
            configuration.push_str(&format!("Paths=../{directory}/*.*\n"));
        }
        if configured {
            configuration.push_str("Suppress=ScriptWarning\nSuppress=CustomStartupCategory\n");
        }
        fs::write(&system_ini, configuration).unwrap();
        let owner = self::owner(&root, &system_ini);
        let access = owner.access();
        let script_warning = access.intern_str("ScriptWarning", FindName::Find).unwrap();
        let custom = access.intern_str("CustomStartupCategory", FindName::Add).unwrap();
        assert!(!access.name_suppressed(script_warning));
        assert!(!access.name_suppressed(custom));
        let initialized = access.initialize_application_system();
        if valid {
            assert_eq!(initialized.unwrap(), access.system().unwrap(),
                "application init uses the one rooted canonical System");
        } else {
            assert!(initialized.is_err());
        }
        assert_eq!(access.name_suppressed(script_warning), valid && configured);
        assert_eq!(access.name_suppressed(custom), valid && configured);
        assert!(!access.name_suppressed(access.intern_str("Log", FindName::Find).unwrap()),
            "unconfigured categories retain their original flags");
        owner.shutdown().unwrap();
    }
}

#[test]
fn new_expression_preserves_the_following_statement_without_a_parameter_terminator() {
    let (root, system_ini) = isolated_data_root();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    let class = access.native_class("TextBuffer").unwrap();
    let receiver = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0x4000, std::ptr::null_mut())
    }.unwrap();
    let name = access.intern_str("ScriptConstructedBuffer", FindName::Add).unwrap();
    let mut code = vec![0x11, 0x2a, 0x21]; // New, NoObject, NameConst.
    code.extend(name.to_le_bytes());
    code.extend([0x25, 0x20]); // IntZero flags, ObjectConst class.
    code.extend(unsafe { (*class.cast::<UObject>()).index }.to_le_bytes());
    code.extend([0x04, 0x1d]); // Return, IntConst. New has no EndFunctionParms.
    code.extend(42_i32.to_le_bytes());
    let mut returned = -1_i32;
    run_probe(access, receiver, code, std::ptr::from_mut(&mut returned).cast())
        .expect("execute source New grammar followed by an independent return");
    assert_eq!(returned, 42, "object construction must not consume the following Return token");
    let path = "Transient.ScriptConstructedBuffer".chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    let created = unsafe { access.find_object_by_name(class, &path) }.unwrap().unwrap();
    assert_ne!(created, receiver);
    assert_eq!(unsafe { (*created).class }, class.cast());
    assert_eq!(unsafe { (*created).outer }, access.transient_package().cast());
    owner.shutdown().unwrap();
}

#[test]
fn goto_state_without_a_label_starts_begin_code_not_begin_state_event() {
    use hp2_abi::engine::generated::{FStateFrame, UField, UState};
    use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_records::TextBufferRecord};
    let (root, system_ini) = isolated_data_root();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    unsafe { access.load_package(&root.join("System/Core.u"), 1) }.unwrap();
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe {
        access.construct_object(class, access.transient_package(),
            access.intern_str("StateCodeReceiver", FindName::Add).unwrap(), 0x4000, std::ptr::null_mut())
    }.unwrap().cast::<TextBufferRecord>();
    let state_name = access.intern_str("StateStartup", FindName::Add).unwrap();
    let state = unsafe {
        access.construct_object(access.native_class("Core.State").unwrap(), class.cast(),
            state_name, 0x4000, std::ptr::null_mut())
    }.unwrap().cast::<UState>();
    let begin = access.intern_str("Begin", FindName::Add).unwrap();
    let begin_state = access.intern_str("BeginState", FindName::Add).unwrap();
    let mut code = vec![0x1d];
    code.extend(97_i32.to_le_bytes());
    code.extend([0x08, 0x1d]);
    code.extend(13_i32.to_le_bytes());
    code.push(0x08);
    code.push(0x0c); // LabelTable; its entries follow the opcode.
    let table_offset = code.len() as u16;
    code.extend(begin.to_le_bytes());
    code.extend(0_i32.to_le_bytes());
    code.extend(begin_state.to_le_bytes());
    code.extend(6_i32.to_le_bytes());
    code.extend(0_i32.to_le_bytes());
    code.extend(0_i32.to_le_bytes());
    unsafe {
        (*state.cast::<UField>()).next = (*class.cast::<UStruct>()).children;
        (*class.cast::<UStruct>()).children = state.cast();
        (*state).label_table_offset = table_offset;
        (*state).probe_mask = u64::MAX;
        (*state).ignore_mask = u64::MAX;
        let script = std::ptr::addr_of_mut!((*state.cast::<UStruct>()).script);
        array_storage::set(Hp2ArraySlots {
            data: &raw mut (*script).data, num: &raw mut (*script).num, max: &raw mut (*script).max,
        }, code.len() as i32, 1, std::ptr::null(), array_storage::system_allocator()).unwrap();
        std::ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast::<u8>(), code.len());
        access.initialize_execution(object.cast()).unwrap();
    }
    let mut enter = vec![113, 0x21];
    enter.extend(state_name.to_le_bytes());
    enter.extend([0x16, 0x04, 0x0b]);
    run_probe(access, object.cast(), enter, std::ptr::null_mut()).unwrap();
    let frame = unsafe { (*object.cast::<UObject>()).state_frame.cast::<FStateFrame>() };
    assert_eq!(unsafe { (*frame).state_node }, state.cast());
    assert_eq!(unsafe { (*frame).code }, unsafe { (*state.cast::<UStruct>()).script.data.cast() },
        "an omitted label selects the state's Begin code");
    let mut result = 0_i32;
    unsafe { access.execute_state_frame_step(frame.cast(), (&raw mut result).cast()) }.unwrap();
    assert_eq!(result, 97, "the Begin entry executes instead of the distinct BeginState label");
    owner.shutdown().unwrap();
}

#[test]
fn script_vector_shifts_preserve_source_rotation_direction() {
    let (root, system_ini) = isolated_data_root();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    let class = access.native_class("TextBuffer").unwrap();
    let receiver = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0x4000, std::ptr::null_mut())
    }.unwrap();

    // A camera's negative-X offset rotates in opposite directions for << and
    // >>. Exercise the real high-native bytecode, not just the math helper.
    for (native, expected_y) in [(19_u8, 128.0_f32), (20_u8, -128.0_f32)] {
        let mut code = vec![0x04, 0x61, native, 0x23]; // Return, high native, VectorConst.
        for value in [-128.0_f32, 0.0, 55.0] {
            code.extend(value.to_le_bytes());
        }
        code.push(0x22); // RotationConst: one quarter-turn around yaw.
        for value in [0_i32, 16384, 0] {
            code.extend(value.to_le_bytes());
        }
        code.push(0x16);
        let mut result = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
        run_probe(access, receiver, code, std::ptr::from_mut(&mut result).cast()).unwrap();
        assert!(result.x.abs() < 0.0001);
        assert!((result.y - expected_y).abs() < 0.0001,
            "script native {} returned Y={} instead of {}", 256 + u32::from(native), result.y, expected_y);
        assert_eq!(result.z, 55.0);
    }
    owner.shutdown().unwrap();
}

#[test]
fn canonical_localization_reads_nested_native_package_paths() {
    let (root, system_ini) = isolated_data_root();
    let directory = root.join("System/Cutscenes");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("RuntimeIntro.int"),
        "[thread_0]\nline_0=CAMERA FadeIn Time=1\n").unwrap();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    let text = access.localize_text(
        &utf32("thread_0"), &utf32("line_0"),
        Some(&utf32("Cutscenes\\RuntimeIntro")), None, false,
    ).unwrap();
    assert_eq!(text, utf32("CAMERA FadeIn Time=1"));
    owner.shutdown().unwrap();
}

#[test]
fn script_string_switch_matches_native_case_insensitive_keywords() {
    let (root, system_ini) = isolated_data_root();
    let owner = owner(&root, &system_ini);
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_scriptable: true, ..access.process_mode() });
    let class = access.native_class("TextBuffer").unwrap();
    let receiver = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0x4000, std::ptr::null_mut())
    }.unwrap();

    for (keyword, expected) in [("cApTuRe", 41_i32), ("unrecognized", -1_i32)] {
        let mut code = vec![0x05, 0, 0x1f]; // Switch FString, StringConst.
        code.extend(keyword.bytes().chain([0]));
        code.push(0x0a); // Case.
        let next_case = code.len();
        code.extend([0, 0, 0x1f]);
        code.extend(b"CAPTURE\0");
        code.extend([0x04, 0x1d]); // Return IntConst from the matched branch.
        code.extend(41_i32.to_le_bytes());
        let default_offset = code.len() as u16;
        code[next_case..next_case + 2].copy_from_slice(&default_offset.to_le_bytes());
        code.extend([0x0a, 0xff, 0xff, 0x04, 0x1d]);
        code.extend((-1_i32).to_le_bytes());
        let mut result = 0_i32;
        run_probe(access, receiver, code, std::ptr::from_mut(&mut result).cast()).unwrap();
        assert_eq!(result, expected, "cutscene keyword {keyword}");
    }
    owner.shutdown().unwrap();
}
