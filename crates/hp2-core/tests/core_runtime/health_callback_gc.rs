//! Rooted GC/reentry fixture for an interpreted health-style LET destination.
//! The parent `core_runtime.rs` must declare this module under its existing
//! opt1 `rust_core_runtime` selection; this file does not change registration.
use super::*;
use hp2_abi::engine::generated::{FFrame, UTextBuffer};
use hp2_core::ScriptPreCallDecision;
use std::{cell::{Cell, RefCell}, ptr};

#[path = "../support/gc_script_metadata.rs"]
mod gc_script_metadata;

const RETURN_VALUE: i32 = 0x1122_3344;
const INITIAL_TOP: i32 = -31;
const INITIAL_POS: i32 = -47;
const POST_ERROR: &str = "rooted health callback post failure";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event { ReplacementEnter, ChildPostEnter, GcStart, GcDone, ProbeStart, ProbeDone, ChildPostError, ReplacementError, ParentPost }

thread_local! {
    static CHILD: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static PARENT: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static PROBE: Cell<*mut UFunction> = const { Cell::new(ptr::null_mut()) };
    static ACTIVE_FRAME: Cell<*mut FFrame> = const { Cell::new(ptr::null_mut()) };
    static ACTIVE_DESTINATION: Cell<*mut i32> = const { Cell::new(ptr::null_mut()) };
    static ROOTS: RefCell<Vec<(usize, i32)>> = const { RefCell::new(Vec::new()) };
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static ROOTS_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static COLLECTIONS: Cell<u32> = const { Cell::new(0) };
    static METADATA_CLASSES: RefCell<Vec<gc_script_metadata::NativeClass>> = const { RefCell::new(Vec::new()) };
}

fn record(event: Event) { EVENTS.with(|events| events.borrow_mut().push(event)); }

// Read the actual metadata in this process immediately before collection. No
// preload, Core interning, callback or borrowed script mutation is permitted.
fn check_gc_metadata(access: CoreAccess<'_>, roots: &[(usize, i32)]) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    let mut evidence = String::new();
    for &(address, index) in roots {
        let object = access.object_at(index);
        if object.is_null() || object as usize != address {
            return Err(RuntimeError::new("GC metadata root is no longer indexed"));
        }
        // SAFETY: this is the exact indexed record constructed and rooted by
        // this fixture; no callback or collection runs during these reads.
        unsafe {
            if (*object).object_flags & (CORE_LINKER_CONSTANTS.need_load | CORE_LINKER_CONSTANTS.preloading) != 0
                || !(*object)._linker.is_null() || (*object)._linker_index != -1
                || !(*object).state_frame.is_null() {
                return Err(RuntimeError::new("GC metadata root has unexpected load or frame state"));
            }
        }
        evidence.push_str(&format!("ROOT index={index} address={address:#x} load_flags=0 linker=null/-1 state_frame=null\n"));
    }
    METADATA_CLASSES.with(|classes| {
        let classes = classes.borrow();
        for (root, expected) in [
            ("Function", &["Function", "Struct", "Field", "Object"][..]),
            ("IntProperty", &["IntProperty", "Property", "Field", "Object"][..]),
            ("TextBuffer", &["TextBuffer", "Object"][..]),
        ] {
            let class = classes.iter().find(|class| class.name == root)
                .ok_or_else(|| RuntimeError::new("GC metadata native identity is missing"))?;
            // SAFETY: identities came from this owner's immutable native
            // registrations before event entry. The roots and table remain
            // live; scan invokes no callback, preload, interner or Core cache.
            let report = unsafe { gc_script_metadata::scan(access, class.address, &classes) }
                .map_err(|error| RuntimeError::new(&error))?;
            if report.classes.iter().map(String::as_str).collect::<Vec<_>>().as_slice() != expected
                || !report.serializable.is_empty() {
                return Err(RuntimeError::new("GC metadata Reference property walk is not excluded"));
            }
            for line in report.lines { evidence.push_str(&line); evidence.push('\n'); }
        }
        Ok(())
    })?;
    // The RefCell borrow has ended. Retain the same-process observations; an
    // evidence-write error propagates before collection, never skips a gate.
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR")
        .ok_or_else(|| RuntimeError::new("Missing GC metadata artifact directory"))?);
    let phase = if ACTIVE_FRAME.with(Cell::get).is_null() { "stock" } else { "replacement" };
    let path = artifact.join(format!("health-gc-metadata-{}-{phase}.txt", std::process::id()));
    use std::io::Write;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| RuntimeError::new(&format!("GC metadata evidence: {error}")))?;
    file.write_all(evidence.as_bytes())
        .map_err(|error| RuntimeError::new(&format!("GC metadata evidence: {error}")))
}

/// The rooted receiver and frame remain live until the synchronous call ends.
unsafe fn replacement(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    assert!(ROOTS_ACTIVE.with(Cell::get), "roots remain installed during replacement");
    assert_eq!(function, PARENT.with(Cell::get));
    assert!(!frame.is_null(), "replacement receives a live parent frame");
    assert_eq!(unsafe { (*frame).object }, object.cast(), "parent frame belongs to receiver");
    record(Event::ReplacementEnter);
    // SAFETY: the receiver is the rooted UTextBuffer constructed by this test.
    // `top` is the original four-byte EX_Let lvalue, not an identity field.
    let destination = unsafe { ptr::addr_of_mut!((*object.cast::<UTextBuffer>()).top) };
    let mut return_parameter = 0_i32;
    ACTIVE_FRAME.with(|slot| slot.set(frame));
    ACTIVE_DESTINATION.with(|slot| slot.set(destination));
    // SAFETY: the child and its reflected return record are rooted. Both the
    // parent frame and the actual destination remain live through reentry.
    let result = unsafe { access.execute_script_callback(frame, object, CHILD.with(Cell::get), (&raw mut return_parameter).cast(), destination.cast()) };
    ACTIVE_FRAME.with(|slot| slot.set(ptr::null_mut()));
    ACTIVE_DESTINATION.with(|slot| slot.set(ptr::null_mut()));
    if result.is_err() { record(Event::ReplacementError); }
    result?;
    Ok(ScriptPreCallDecision::Consume)
}

/// Both paths call this after the child wrote the original LET result address.
unsafe fn post(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, _: *const u8, _: usize) -> RuntimeResult<()> {
    if function == PARENT.with(Cell::get) {
        record(Event::ParentPost);
        return Ok(());
    }
    assert_eq!(function, CHILD.with(Cell::get));
    assert!(ROOTS_ACTIVE.with(Cell::get), "roots remain installed in child post hook");
    record(Event::ChildPostEnter);
    // SAFETY: this is the rooted TextBuffer receiver, whose fields are valid.
    let buffer = object.cast::<UTextBuffer>();
    assert_eq!(unsafe { (*buffer).top }, RETURN_VALUE, "the original destination is already written");
    assert_eq!(unsafe { (*buffer).pos }, INITIAL_POS, "the adjacent field is untouched");
    // Do not hold the RefCell borrow across collection or reentrant script.
    let roots = ROOTS.with(|records| records.borrow().clone());
    record(Event::GcStart);
    check_gc_metadata(access, &roots)?;
    access.collect_garbage(0, 0, &|_| Ok(()))?;
    COLLECTIONS.with(|count| count.set(count.get() + 1));
    record(Event::GcDone);
    for (address, index) in roots {
        assert_eq!(access.object_at(index) as usize, address, "each constructed root remains indexed after GC");
    }
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (RETURN_VALUE, INITIAL_POS));
    let frame = ACTIVE_FRAME.with(Cell::get);
    if !frame.is_null() {
        // The replacement call is still synchronous; its parent frame and
        // exact result pointer must remain valid through the collection.
        assert_eq!(unsafe { (*frame).object }, object.cast());
        assert_eq!(unsafe { (*frame).node }, PARENT.with(Cell::get).cast());
        assert_eq!(ACTIVE_DESTINATION.with(Cell::get), unsafe { ptr::addr_of_mut!((*buffer).top) });
        assert_eq!(unsafe { ACTIVE_DESTINATION.with(Cell::get).read_unaligned() }, RETURN_VALUE);
    }
    record(Event::ProbeStart);
    // SAFETY: probe, property records and receiver are rooted; it executes
    // synchronously while the parent's result destination is still live.
    unsafe { access.execute_event(object, PROBE.with(Cell::get), ptr::null_mut(), ptr::null_mut())?; }
    record(Event::ProbeDone);
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (RETURN_VALUE, RETURN_VALUE), "real script reentry reads the written destination");
    record(Event::ChildPostError);
    Err(RuntimeError::new(POST_ERROR))
}

/// The owner and outer remain live; each returned record is rooted immediately.
unsafe fn make_function(access: CoreAccess<'_>, outer: *mut UObject, name: &str) -> *mut UFunction {
    let function = unsafe { access.construct_object(access.native_class("Function").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UFunction>();
    unsafe {
        access.add_root(function.cast());
        (*function).function_flags = 0;
        (*function).parms_size = 0;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_size = 0;
        (*function.cast::<UStruct>()).properties_align = 4;
    }
    function
}

/// The owner and outer remain live; the caller supplies the exact ABI offset.
unsafe fn make_int_property(access: CoreAccess<'_>, outer: *mut UObject, name: &str, offset: usize, flags: u32) -> *mut UProperty {
    let property = unsafe { access.construct_object(access.native_class("IntProperty").unwrap(), outer, access.intern_str(name, FindName::Add).unwrap(), 0x4000, ptr::null_mut()).unwrap() }.cast::<UProperty>();
    unsafe {
        access.add_root(property.cast());
        (*property).array_dim = 1;
        (*property).element_size = 4;
        (*property).offset = i32::try_from(offset).unwrap();
        (*property).property_flags = flags;
    }
    property
}

fn operand(code: &mut Vec<u8>, opcode: u8, object: *mut UObject) {
    code.push(opcode);
    // SAFETY: the caller supplies an indexed record rooted before execution.
    code.extend(unsafe { (*object).index }.to_le_bytes());
}

/// Detach all borrowed bytecode before buffers or the owner can be dropped.
/// This guard is declared after the code Vecs and before scripts are attached.
struct FixtureCleanup<'a> { access: CoreAccess<'a>, functions: Vec<*mut UFunction>, roots: Vec<*mut UObject> }
impl Drop for FixtureCleanup<'_> {
    fn drop(&mut self) {
        for &function in &self.functions {
            // SAFETY: the explicit roots are still installed at this point.
            unsafe { (*function.cast::<UStruct>()).script = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
        }
        for &object in &self.roots { self.access.remove_root(object); }
        ROOTS_ACTIVE.with(|active| active.set(false));
        ROOTS.with(|roots| roots.borrow_mut().clear());
        CHILD.with(|slot| slot.set(ptr::null_mut()));
        PARENT.with(|slot| slot.set(ptr::null_mut()));
        PROBE.with(|slot| slot.set(ptr::null_mut()));
        METADATA_CLASSES.with(|classes| classes.borrow_mut().clear());
        ACTIVE_FRAME.with(|slot| slot.set(ptr::null_mut()));
        ACTIVE_DESTINATION.with(|slot| slot.set(ptr::null_mut()));
    }
}

#[test]
fn rooted_health_let_callback_gc_reentry() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifact.join(format!("health-callback-gc-{}", std::process::id()));
    assert!(!root.exists(), "use a fresh isolated artifact directory");
    fs::create_dir_all(root.join("System")).unwrap();
    let source = workspace_root().join("System");
    for (from, to) in [("Core.u", "Core.u"), ("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
        fs::copy(source.join(from), root.join("System").join(to)).unwrap();
    }
    let owner = owner(&root, &root.join("System/System.ini"));
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    let name: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
    // SAFETY: this loads the actual package class used by the rooted GC template.
    unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &name, 1).unwrap(); }
    let class = access.native_class("Core.TextBuffer").unwrap();
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0x4000, ptr::null_mut()).unwrap() };
    unsafe { access.add_root(object); }
    let buffer = object.cast::<UTextBuffer>();
    assert_eq!(std::mem::offset_of!(UTextBuffer, top) - std::mem::offset_of!(UTextBuffer, pos), 4, "the neighbor is exactly one i32 away");

    // These Vecs are declared before the cleanup guard. They remain alive while
    // it detaches every borrowed script on normal return or panic.
    let mut child_code = Vec::new();
    let mut parent_code = Vec::new();
    let mut root_code = Vec::new();
    let mut probe_code = Vec::new();
    let top = unsafe { make_int_property(access, class.cast(), "RootedHealthTop", std::mem::offset_of!(UTextBuffer, top), 0) };
    let pos = unsafe { make_int_property(access, class.cast(), "RootedHealthPos", std::mem::offset_of!(UTextBuffer, pos), 0) };
    let child = unsafe { make_function(access, class.cast(), "RootedHealthReturn") };
    let ret = unsafe { make_int_property(access, child.cast(), "ReturnValue", 0, 0x480) };
    let parent = unsafe { make_function(access, class.cast(), "RootedHealthParent") };
    let root_function = unsafe { make_function(access, class.cast(), "RootedHealthRoot") };
    let probe = unsafe { make_function(access, class.cast(), "RootedHealthProbe") };
    let roots = vec![object, top.cast(), pos.cast(), child.cast(), ret.cast(), parent.cast(), root_function.cast(), probe.cast()];
    ROOTS.with(|records| *records.borrow_mut() = roots.iter().map(|&record| (record as usize, unsafe { (*record).index })).collect());
    ROOTS_ACTIVE.with(|active| active.set(true));
    let cleanup = FixtureCleanup { access, functions: vec![child, parent, root_function, probe], roots };
    CHILD.with(|slot| slot.set(child)); PARENT.with(|slot| slot.set(parent)); PROBE.with(|slot| slot.set(probe));
    // Resolve caches only during setup, before any script event. The identity
    // list contains borrowed pointers, all owned by this still-live Core owner.
    let metadata_classes = hp2_core::core_class_specs::NATIVE_CLASSES.iter().map(|spec| {
        gc_script_metadata::NativeClass { address: access.native_class(spec.short_name).unwrap(),
            name: spec.short_name, kind: spec.kind }
    }).collect();
    METADATA_CLASSES.with(|classes| *classes.borrow_mut() = metadata_classes);

    unsafe {
        (*child).num_parms = 1;
        (*child).parms_size = 4;
        (*child).return_value_offset = 0;
        (*child.cast::<UStruct>()).properties_size = 4;
        (*child.cast::<UStruct>()).children = ret.cast();
    }
    child_code.extend([0x04, 0x1d]); // Return IntConst.
    child_code.extend(RETURN_VALUE.to_le_bytes());
    parent_code.push(0x0f); // Let InstanceVariable(top) = FinalFunction(child).
    operand(&mut parent_code, 0x01, top.cast());
    operand(&mut parent_code, 0x1c, child.cast());
    parent_code.push(0x16);
    parent_code.push(0x0f); // Must not run after the post-call error.
    operand(&mut parent_code, 0x01, pos.cast());
    parent_code.extend([0x2c, 77, 0x04, 0x0b]);
    operand(&mut root_code, 0x1c, parent.cast());
    root_code.extend([0x16, 0x04, 0x0b]);
    probe_code.push(0x0f); // Let InstanceVariable(pos) = InstanceVariable(top).
    operand(&mut probe_code, 0x01, pos.cast());
    operand(&mut probe_code, 0x01, top.cast());
    probe_code.extend([0x04, 0x0b]);
    for (function, code) in [(child, &mut child_code), (parent, &mut parent_code), (root_function, &mut root_code), (probe, &mut probe_code)] {
        // SAFETY: these Vecs do not move or reallocate before cleanup detaches
        // their script pointers, and all functions remain explicitly rooted.
        unsafe { (*function.cast::<UStruct>()).script = TArrayRaw { data: code.as_mut_ptr().cast(), num: code.len() as i32, max: code.len() as i32 }; }
    }
    let observer = unsafe { access.install_script_post_call_hook([child, parent], post).unwrap() };
    let run = |expected: &[Event]| {
        unsafe { (*buffer).top = INITIAL_TOP; (*buffer).pos = INITIAL_POS; }
        EVENTS.with(|events| events.borrow_mut().clear());
        COLLECTIONS.with(|count| count.set(0));
        // SAFETY: every constructed record and every borrowed script is live.
        let error = unsafe { access.execute_event(object, root_function, ptr::null_mut(), ptr::null_mut()) }.unwrap_err();
        assert_eq!(error.diagnostic(), POST_ERROR.chars().map(u32::from).collect::<Vec<_>>());
        assert_eq!(COLLECTIONS.with(Cell::get), 1, "one real synchronous GC per path");
        assert_eq!(EVENTS.with(|events| events.borrow().clone()).as_slice(), expected);
        assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (RETURN_VALUE, RETURN_VALUE), "post error retains the write and probe effect");
        ROOTS.with(|records| for &(address, index) in records.borrow().iter() {
            assert_eq!(access.object_at(index) as usize, address, "root remains indexed after the call");
        });
        assert!(ROOTS_ACTIVE.with(Cell::get));
    };
    let baseline = [Event::ChildPostEnter, Event::GcStart, Event::GcDone, Event::ProbeStart, Event::ProbeDone, Event::ChildPostError];
    run(&baseline);
    let binding = unsafe { access.install_script_body_replacement("rooted-health-gc-parent", &[parent], replacement).unwrap() };
    let replaced = [Event::ReplacementEnter, Event::ChildPostEnter, Event::GcStart, Event::GcDone, Event::ProbeStart, Event::ProbeDone, Event::ChildPostError, Event::ReplacementError];
    run(&replaced);

    // Guards remain installed throughout both calls. After both calls return,
    // remove them and prove a fresh call no longer invokes either hook.
    assert!(ACTIVE_FRAME.with(Cell::get).is_null());
    assert!(ACTIVE_DESTINATION.with(Cell::get).is_null());
    drop(binding);
    drop(observer);
    assert!(ROOTS_ACTIVE.with(Cell::get));
    EVENTS.with(|events| events.borrow_mut().clear());
    COLLECTIONS.with(|count| count.set(0));
    unsafe { (*buffer).top = INITIAL_TOP; (*buffer).pos = INITIAL_POS; }
    unsafe { access.execute_event(object, root_function, ptr::null_mut(), ptr::null_mut()).unwrap(); }
    assert_eq!(unsafe { ((*buffer).top, (*buffer).pos) }, (RETURN_VALUE, 77));
    assert!(EVENTS.with(|events| events.borrow().is_empty()), "dropped hooks do not run");
    assert_eq!(COLLECTIONS.with(Cell::get), 0);
    drop(cleanup);
    assert!(!ROOTS_ACTIVE.with(Cell::get), "roots are removed only after execution");
    assert!(ROOTS.with(|records| records.borrow().is_empty()));
    owner.shutdown().unwrap();
}
