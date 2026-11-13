//! Synthetic owned Function locals, not Engine or retail property metadata.
//! Fully-writing X/Y trees, allocation witnesses and current-statement fallback.
//! No FString, rendering or reentry qualification.
use super::*;
use hp2_abi::engine::generated::UProperty;
use hp2_core::{ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision, vm_owner::StepControl};
use std::{ffi::c_void, mem::offset_of};

const SITES: [(usize, usize); 2] = [(0x183, 0x189), (0x1a2, 0x1a8)];
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Params { x: i32, y: i32, scale: f32, x_extent: f32, y_extent: f32 }
#[derive(Clone, Copy)]
struct Selection { function: *mut UFunction, receiver: *mut UObject, base: *const u8, len: usize, selected: bool }
thread_local! {
    static SELECTION: Cell<Option<Selection>> = const { Cell::new(None) };
    static ORDER: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static ROOTS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}
struct SelectionGuard;
impl Drop for SelectionGuard {
    fn drop(&mut self) {
        SELECTION.with(|s| s.set(None));
        ORDER.with(|v| v.borrow_mut().clear());
        ROOTS.with(|v| v.borrow_mut().clear());
    }
}

// SAFETY: only the currently executing Function/frame is supplied by Core;
// its synthetic program and scalar local records are owned for the event.
unsafe fn replacement(a: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let s = SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("hotkey fixture selection absent"))?;
    if function != s.function || object != s.receiver || frame.is_null()
        || !a.current_script_body_matches(frame, object, function, s.base, s.len)
    { return Err(RuntimeError::new("hotkey fixture current body mismatch")); }
    let mut state = ScriptBodyStatements::new();
    while unsafe { state.begin(a, frame)? }.is_some() {
        // SAFETY: Core's current, guarded body frame remains live here.
        let live = unsafe { (*frame).code.cast_const() };
        let at = SITES.iter().position(|(site, _)| live == s.base.wrapping_add(*site));
        if let Some(i) = at { ORDER.with(|v| v.borrow_mut().push(SITES[i].0)); }
        let step = if s.selected && at == Some(0) {
            // SAFETY: immutable exact fixture tree and fully-writing continuations.
            unsafe { a.execute_script_let_with_rhs(frame, state.result(), s.base.add(0x183), s.base.add(0x189), trace_x)? }
        } else if s.selected && at == Some(1) {
            // SAFETY: same bounded fixture contract for Y.
            unsafe { a.execute_script_let_with_rhs(frame, state.result(), s.base.add(0x1a2), s.base.add(0x1a8), trace_y)? }
        } else {
            // Both comparison arms use the same current-host statement loop;
            // the canonical arm delegates every statement to Core.
            unsafe { a.execute_script_body_step(frame, state.result())? }
        };
        if step == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

// Only offsets are copied; the current frame/destination/scope never escape.
unsafe fn trace(a: CoreAccess<'_>, args: &ScriptCallArguments<'_>, f: *mut FFrame, object: *mut c_void, dst: *mut c_void, i: usize) -> RuntimeResult<()> {
    let s = SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("hotkey trace selection absent"))?;
    if args.body_script_snapshot() != (s.base, s.len) || unsafe { (*f).code.cast_const() } != s.base.wrapping_add(SITES[i].1) {
        return Err(RuntimeError::new("hotkey fixture actual RHS mismatch"));
    }
    ROOTS.with(|v| v.borrow_mut().push(SITES[i].1));
    let eval: hp2_core::ScriptAssignmentValue = if i == 0 { super::hotkey_rhs::x_offset } else { super::hotkey_rhs::y_offset };
    unsafe { eval(a, args, f, object, dst) }
}
unsafe fn trace_x(a: CoreAccess<'_>, s: &ScriptCallArguments<'_>, f: *mut FFrame, o: *mut c_void, d: *mut c_void) -> RuntimeResult<()> { unsafe { trace(a, s, f, o, d, 0) } }
unsafe fn trace_y(a: CoreAccess<'_>, s: &ScriptCallArguments<'_>, f: *mut FFrame, o: *mut c_void, d: *mut c_void) -> RuntimeResult<()> { unsafe { trace(a, s, f, o, d, 1) } }

// Only fixture-created records enter this owner-scoped guard. Rooting does
// not prevent explicit destruction; this closed case permits no such callback.
struct RootedFixture { fx: Option<Fixture>, objects: Vec<*mut UObject> }
impl RootedFixture {
    fn new(label: &str) -> Self { Self { fx: Some(Fixture::new(label, Native::Tile, Case::StyleNone, false)), objects: Vec::new() } }
    fn fixture(&self) -> &Fixture { self.fx.as_ref().expect("live fixture") }
    fn root(&mut self, object: *mut UObject) {
        // Each object has just been constructed under a previously vacant name.
        // Publish the cleanup obligation before adding the root.
        self.objects.push(object);
        unsafe { self.fixture().a().add_root(object); }
    }
    fn finish(mut self) -> RuntimeResult<()> {
        let fx = self.fx.as_mut().expect("live fixture");
        fx.verify();
        fx.detach(); // no event, binding or selection may still be active
        for object in self.objects.drain(..).rev() { fx.a().remove_root(object); }
        self.fx.take().expect("live fixture").finish()
    }
}
impl Drop for RootedFixture {
    fn drop(&mut self) {
        if let Some(fx) = self.fx.as_mut() {
            // Replacement binding and selection guards must drop first.
            fx.detach();
            for object in self.objects.drain(..).rev() { fx.a().remove_root(object); }
        }
        // Fixture's own Drop runs the checked/best-effort owner shutdown path.
    }
}

// Check vacancy before the owner constructor can select a replacement slot.
unsafe fn vacant(a: CoreAccess<'_>, class: *mut UClass, outer: *mut UObject, name: &str) {
    let n = a.intern_str(name, FindName::Add).expect("fixture unique name");
    assert!(unsafe { a.find_object(class, outer, n, false) }.is_null(), "fixture name must be unused in this outer");
}

// Equivalent bounded class/default checks to Fixture::class, extended only to
// the two actual Core scalar-property layouts. Nothing is fabricated.
unsafe fn scalar_class(a: CoreAccess<'_>, kind: &str) -> *mut UClass {
    assert!(kind == "Core.IntProperty" || kind == "Core.FloatProperty");
    let chain = [
        (kind, size_of::<UProperty>(), align_of::<UProperty>()),
        ("Core.Property", size_of::<UProperty>(), align_of::<UProperty>()),
        ("Core.Field", size_of::<UField>(), align_of::<UField>()),
        ("Core.Object", size_of::<UObject>(), align_of::<UObject>()),
    ];
    let meta = a.native_class("Core.Class").expect("class metaclass");
    let package_class = a.native_class("Core.Package").expect("package class");
    let bad = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.preloading
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load
        | hp2_core::object_lifecycle::RF_UNREACHABLE
        | hp2_core::object_lifecycle::RF_DESTROYED
        | hp2_core::object_lifecycle::RF_DEBUG_DESTROY;
    let mut selected = ptr::null_mut();
    for (i, &(name, bytes, alignment)) in chain.iter().enumerate() {
        let cls = a.native_class(name).expect("registered Core class");
        unsafe { indexed(a, cls.cast()); }
        if i == 0 { selected = cls; }
        let record = cls.cast::<UObject>();
        assert_eq!(unsafe { (*record).class.cast::<UClass>() }, meta);
        let (package_name, short) = name.split_once('.').expect("qualified class");
        assert_eq!(a.name_text(i32::from_ne_bytes(unsafe { (*record).name })).expect("class name"), utf32(short));
        let package = unsafe { (*record).outer.cast::<UObject>() };
        unsafe { indexed(a, package); }
        assert_eq!(unsafe { (*package).class.cast::<UClass>() }, package_class);
        assert!(unsafe { (*package).outer.is_null() });
        assert_eq!(a.name_text(i32::from_ne_bytes(unsafe { (*package).name })).expect("package name"), utf32(package_name));
        assert_eq!(unsafe { (*record).object_flags } & bad, 0);
        assert!(unsafe { (*record)._linker.is_null() });
        assert!(!unsafe { (*cls).class_constructor.is_null() });
        let flags = unsafe { (*cls).class_flags };
        let compiled = hp2_abi::engine::generated::CORE_CLASS_PARSED_COMPILED_MASK;
        assert_eq!(flags & compiled, compiled);
        if i == 0 { assert_eq!(flags & (hp2_core::object_construction::CLASS_ABSTRACT | hp2_core::object_construction::CLASS_PER_OBJECT_CONFIG), 0); }
        let expected = if i + 1 == chain.len() { ptr::null_mut() } else { a.native_class(chain[i + 1].0).expect("expected super") };
        assert_eq!(unsafe { (*cls).super_field.cast::<UClass>() }, expected);
        let structure = cls.cast::<UStruct>();
        assert_eq!(unsafe { (*cls).properties_size }, bytes as i32);
        assert_eq!(unsafe { (*structure).properties_align }, 4);
        assert!(unsafe { (*structure).constructor_link.is_null() });
        let defaults = unsafe { ptr::addr_of!((*cls).defaults).read_unaligned() };
        assert!(!defaults.data.is_null() && defaults.num == bytes as i32 && defaults.max >= defaults.num);
        assert_eq!((defaults.data as usize) % alignment, 0);
        let default = unsafe { a.default_object(cls).expect("class default") };
        assert_eq!(default, defaults.data.cast());
        assert_eq!(unsafe { (*default).class.cast::<UClass>() }, cls);
        assert_eq!(unsafe { (*default).index }, -1);
        assert!(unsafe { (*default).outer.is_null() && (*default).state_frame.is_null() && (*default)._linker.is_null() });
        assert_eq!(unsafe { (*default).object_flags } & bad, 0);
        if i <= 1 {
            let property = default.cast::<UProperty>();
            assert!(unsafe { (*property).constructor_link_next.is_null() });
            assert!(unsafe { (*property.cast::<UField>()).next.is_null() });
        }
    }
    let field = a.native_class("Core.Field").expect("property within class");
    assert_eq!(unsafe { (*selected).class_within.cast::<UClass>() }, field);
    selected
}

// Actual owner-constructed indexed Core property records, explicitly synthetic
// Function parameters, not claimed Engine metadata. The type-specific class and
// default are checked before null-template construction as in white_normal.
unsafe fn parameter(a: CoreAccess<'_>, function: *mut UFunction, kind: &str, name: &str, offset: usize) -> *mut UProperty {
    // Vacancy prohibits replacement and makes this guard's root exclusive.
    let class = unsafe { scalar_class(a, kind) };
    unsafe { vacant(a, class, function.cast(), name); }
    let name = a.intern_str(name, FindName::Find).expect("fixture property name");
    // Recheck the class/default chain immediately before null-template construction.
    assert_eq!(unsafe { scalar_class(a, kind) }, class);
    let p = unsafe { a.construct_object(class, function.cast(), name, 0x4000, ptr::null_mut()).expect("construct scalar property") }.cast::<UProperty>();
    unsafe { indexed(a, p.cast()); }
    assert_eq!(unsafe { (*p.cast::<UObject>()).class.cast::<UClass>() }, class);
    assert_eq!(unsafe { (*p.cast::<UObject>()).outer.cast::<UFunction>() }, function);
    assert_eq!((p as usize) % align_of::<UProperty>(), 0);
    unsafe {
        (*p).array_dim = 1; (*p).element_size = 4;
        (*p).offset = i32::try_from(offset).expect("bounded field offset"); (*p).property_flags = 0x80;
        assert!((*p).constructor_link_next.is_null());
        assert!((*p.cast::<UField>()).next.is_null());
    }
    p
}
unsafe fn parameters(owner: &mut RootedFixture, f: *mut UFunction) -> [*mut UProperty; 5] {
    assert_eq!(size_of::<Params>(), 20);
    assert_eq!(align_of::<Params>(), 4);
    let fields = [
        ("Core.IntProperty", "HotkeyFixtureX", offset_of!(Params, x)),
        ("Core.IntProperty", "HotkeyFixtureY", offset_of!(Params, y)),
        ("Core.FloatProperty", "HotkeyFixtureScale", offset_of!(Params, scale)),
        ("Core.FloatProperty", "HotkeyFixtureXExtent", offset_of!(Params, x_extent)),
        ("Core.FloatProperty", "HotkeyFixtureYExtent", offset_of!(Params, y_extent)),
    ];
    let p = fields.map(|(kind, name, offset)| {
        let p = unsafe { parameter(owner.fixture().a(), f, kind, name, offset) };
        owner.root(p.cast());
        p
    });
    for (i, (_, _, offset)) in fields.iter().enumerate() {
        assert_eq!(*offset, i * 4, "disjoint aligned complete scalar parameter prefix");
    }
    unsafe {
        for pair in p.windows(2) { (*pair[0].cast::<UField>()).next = pair[1].cast(); }
        (*f.cast::<UStruct>()).children = p[0].cast();
        (*f.cast::<UStruct>()).properties_size = size_of::<Params>() as i32;
        (*f.cast::<UStruct>()).properties_align = align_of::<Params>() as i32;
        (*f).parms_size = size_of::<Params>() as u16; (*f).num_parms = p.len() as u8;
        assert!((*f.cast::<UStruct>()).constructor_link.is_null());
    }
    p
}
fn local(code: &mut Vec<u8>, a: CoreAccess<'_>, p: *mut UProperty) {
    unsafe { indexed(a, p.cast()); }
    code.push(0x00);
    code.extend_from_slice(&unsafe { (*p.cast::<UObject>()).index }.to_le_bytes());
}
fn tree(code: &mut Vec<u8>, a: CoreAccess<'_>, lhs: *mut UProperty, scale: *mut UProperty, extent: *mut UProperty, base: u8, site: usize) {
    assert_eq!(code.len(), site);
    code.push(0x0f); local(code, a, lhs);
    assert_eq!(code.len(), site + 6);
    // 45 af 40 45 ab 40 2c <base> <scale> 16 ac <extent> 40 2c 02 16 16
    code.extend_from_slice(&[0x45, 0xaf, 0x40, 0x45, 0xab, 0x40, 0x2c, base]);
    local(code, a, scale);
    code.extend_from_slice(&[0x16, 0xac]);
    local(code, a, extent);
    code.extend_from_slice(&[0x40, 0x2c, 2, 0x16, 0x16]);
}
fn program(a: CoreAccess<'_>, p: [*mut UProperty; 5]) -> Vec<u8> {
    let mut code = vec![0x0b; 0x183]; // canonical EX_Nothing padding
    tree(&mut code, a, p[0], p[2], p[3], 20, 0x183);
    assert_eq!(code.len(), 0x1a2);
    tree(&mut code, a, p[1], p[2], p[4], 50, 0x1a2);
    assert_eq!(code.len(), 0x1c1);
    code.extend_from_slice(&[0x04, 0x0b, 0x0b]); // Return Nothing; lookahead
    code
}
#[derive(Debug, PartialEq)]
struct Outcome {
    before: Params, after: Params, order: Vec<usize>,
    logs: Vec<(Vec<u32>, Vec<u32>)>, debug_calls: u32,
    error: Option<(Vec<u32>, Vec<Vec<u32>>, Option<PathBuf>)>,
}
fn run_one(selected: bool, expected_script: Option<&[u8]>) -> (Vec<u8>, Outcome) {
    let mode = if selected { "selected" } else { "canonical" };
    let mut owner = RootedFixture::new(&format!("hotkey-normal-{mode}"));
    let a = owner.fixture().a();
    let cls = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    unsafe { vacant(a, cls, a.transient_package(), "HotkeyFixtureReceiver"); }
    let receiver = unsafe { object(a, cls, a.transient_package(), "HotkeyFixtureReceiver", UCanvas::NATIVE_ALIGNMENT) };
    owner.root(receiver);
    let a = owner.fixture().a();
    let function_class = unsafe { class(a, "Core.Function", size_of::<UFunction>(), align_of::<UFunction>()) };
    unsafe { vacant(a, function_class, cls.cast(), "TileCanonicalBody"); }
    let f = unsafe { body(a, cls) };
    owner.root(f.cast());
    let props = unsafe { parameters(&mut owner, f) };
    let a = owner.fixture().a();
    let code = program(a, props);
    exclusive_write(&owner.fixture().root.join("hotkey-script.bin"), &code).expect("record immutable fixture program");
    if let Some(expected) = expected_script { assert_eq!(code.as_slice(), expected, "identical actual script bytes before selected execution"); }
    exclusive_write(&owner.fixture().root.join("hotkey-inputs.txt"), format!("{mode}; synthetic Function parameters; scale bits={:#010x}; X/Y extent=3; initial outputs=-111,-222\n", 0.51_f32.to_bits()).as_bytes()).expect("record inputs");
    let saved = code.clone();
    owner.fx.as_mut().expect("live fixture").attach(f, code);
    owner.fixture().verify();
    let a = owner.fixture().a();
    let mut params = Params { x: -111, y: -222, scale: 0.51, x_extent: 3.0, y_extent: 3.0 };
    let before = params;
    LOGS.with(|v| v.borrow_mut().clear()); DEBUG_CALLS.with(|v| v.set(0)); FAIL_OUTPUT.with(|v| v.set(false));
    let output_guard = OutputGuard;
    let slot = &owner.fixture().attached[0];
    SELECTION.with(|v| { assert!(v.get().is_none()); v.set(Some(Selection { function: f, receiver, base: slot.base.cast_const(), len: slot.expected.len(), selected })); });
    let selection_guard = SelectionGuard;
    // SAFETY: private synthetic script, reflected parameters and records are
    // owner-live and immutable; every actual child fully writes its typed slot.
    let binding = unsafe { a.install_script_body_replacement("hotkey-normal-fixture", &[f], replacement) }.expect("install fixture-only body");
    let result = unsafe { a.execute_event(receiver, f, (&raw mut params).cast(), ptr::null_mut()) };
    drop(binding);
    let order = ORDER.with(|v| v.borrow_mut().drain(..).collect::<Vec<_>>());
    let roots = ROOTS.with(|v| v.borrow_mut().drain(..).collect::<Vec<_>>());
    drop(selection_guard); drop(output_guard);
    let logs = LOGS.with(|v| v.borrow().clone());
    let debug_calls = DEBUG_CALLS.with(Cell::get);
    let error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec(), e.source_path().map(Path::to_path_buf)));
    exclusive_write(&owner.fixture().root.join("hotkey-observed.txt"), format!("before={before:?} after={params:?} order={order:?} roots={roots:?} logs={logs:?} debug={debug_calls} error={error:?}\n").as_bytes()).expect("record outcome");
    owner.fixture().verify();
    assert_eq!(order, vec![0x183, 0x1a2], "actual X then Y statement order");
    if selected { assert_eq!(roots, vec![0x189, 0x1a8], "actual selected RHS order"); } else { assert!(roots.is_empty()); }
    assert_eq!(error, None); assert!(logs.is_empty()); assert_eq!(debug_calls, 0);
    assert_eq!((params.x, params.y), (8, 23), "independent retail-order arithmetic expectation");
    assert_eq!(params.scale.to_bits(), 0.51_f32.to_bits());
    assert_eq!(params.x_extent.to_bits(), 3.0_f32.to_bits()); assert_eq!(params.y_extent.to_bits(), 3.0_f32.to_bits());
    let observed = Outcome { before, after: params, order, logs, debug_calls, error };
    owner.finish().expect("checked owner shutdown");
    (saved, observed)
}

#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn hotkey_xy_normal_canonical_selected() {
    let (script, canonical) = run_one(false, None);
    let (other, selected) = run_one(true, Some(&script));
    assert_eq!(script, other);
    assert_eq!(canonical, selected, "complete synthetic parameters, statement order, logs and error outcome");
}
thread_local! {
    static WITNESS_PROPERTIES: Cell<Option<[*mut UProperty; 5]>> = const { Cell::new(None) };
}
struct WitnessSelectionGuard;
impl Drop for WitnessSelectionGuard {
    fn drop(&mut self) { WITNESS_PROPERTIES.with(|p| p.set(None)); }
}

// Restore mutable metadata and the frame pointer even if an assertion panics.
struct WitnessRestore {
    function: *mut UFunction, property: *mut UProperty, frame: *mut FFrame,
    size: i32, parms_size: u16, offset: i32, dim: i32, element_size: i32, locals: *mut u8,
}
impl Drop for WitnessRestore {
    fn drop(&mut self) {
        unsafe {
            (*self.function.cast::<UStruct>()).properties_size = self.size;
            (*self.function).parms_size = self.parms_size;
            (*self.property).offset = self.offset;
            (*self.property).array_dim = self.dim;
            (*self.property).element_size = self.element_size;
            (*self.frame).locals = self.locals;
        }
    }
}

unsafe fn witness_replacement(a: CoreAccess<'_>, _: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    use hp2_core::property_value::Hp2ValueKind::{ValueFloat, ValueInt, ValueString};
    let p = WITNESS_PROPERTIES.with(Cell::get).expect("witness properties installed");
    // SAFETY: the fresh fixture owns the current frame, function and properties.
    let restore = unsafe { WitnessRestore { function, property: p[2], frame,
        size: (*function.cast::<UStruct>()).properties_size, parms_size: (*function).parms_size, offset: (*p[2]).offset,
        dim: (*p[2]).array_dim, element_size: (*p[2]).element_size, locals: (*frame).locals } };
    assert_eq!(restore.size, 20, "event allocated the known five-scalar frame");
    assert_eq!(restore.parms_size, 12, "event captured a shorter parameter prefix");
    // SAFETY: the test runs within the active replacement's current scope.
    unsafe {
        let mut shadow = frame.read();
        assert!(!a.current_script_scalar_local_matches(&raw mut shadow, p[0], ValueInt, 0), "a distinct initialized frame is not current");
        assert!(!a.current_script_parameter_span_matches(&raw mut shadow, 0, 4));
        assert!(a.current_script_scalar_local_matches(frame, p[0], ValueInt, 0));
        assert!(a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 8));
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueInt, 8));
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueString, 8));
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 12));
        // This oversized expected offset rejects before arithmetic overflow.
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, usize::MAX));
        (*p[2]).offset = -1;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 8));
        (*p[2]).offset = 9;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 9), "the complete in-bounds span is unaligned");
        (*p[2]).offset = restore.offset;
        assert!(!a.current_script_scalar_local_matches(std::ptr::null_mut(), std::ptr::null_mut(), ValueInt, 0));
        assert!(a.current_script_parameter_span_matches(frame, 4, 8));
        assert!(a.current_script_parameter_span_matches(frame, 1, 8), "unaligned prefix spans are allowed");
        assert!(a.current_script_parameter_span_matches(frame, 12, 0));
        assert!(!a.current_script_parameter_span_matches(frame, 8, 8), "local allocation bytes beyond parameter prefix are not parameter bytes");
        assert!(!a.current_script_parameter_span_matches(frame, usize::MAX, 2));
        assert!(!a.current_script_parameter_span_matches(std::ptr::null_mut(), 0, 0));
        // Inflate mutable metadata. The actual owner allocation is still 20 bytes.
        (*function.cast::<UStruct>()).properties_size = 4096;
        (*function).parms_size = 20;
        assert!(!a.current_script_parameter_span_matches(frame, 8, 8), "inflated parameter metadata must not extend the original prefix");
        assert!(a.current_script_parameter_span_matches(frame, 4, 8));
        (*p[2]).offset = 20;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 20), "inflated metadata must not extend the event allocation");
        (*p[2]).offset = 16;
        assert!(a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 16), "last complete aligned scalar is within captured extent");
        (*function.cast::<UStruct>()).properties_size = 4;
        (*function).parms_size = 4;
        assert!(a.current_script_parameter_span_matches(frame, 4, 8), "shrinking mutable metadata does not change the captured prefix");
        assert!(a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 16), "shrinking metadata must not replace the captured extent");
        (*p[2]).offset = 17;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 17));
        (*p[2]).offset = 8; (*p[2]).array_dim = 2;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 8));
        (*p[2]).array_dim = 1; (*p[2]).element_size = 8;
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 8));
        (*p[2]).element_size = 4;
        (*frame).locals = restore.locals.wrapping_add(4);
        assert!(!a.current_script_parameter_span_matches(frame, 0, 4));
        assert!(!a.current_script_scalar_local_matches(frame, p[2], ValueFloat, 8), "current locals must equal the owner witness");
    }
    drop(restore);
    // The witness-only fixture intentionally consumes the body. Its shortened
    // parameter prefix does not initialize the complete hotkey input tree.
    Ok(ScriptPreCallDecision::Consume)
}

#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn hotkey_scalar_local_allocation_witness() {
    use hp2_core::property_value::Hp2ValueKind::ValueInt;
    let mut owner = RootedFixture::new("hotkey-scalar-local-witness");
    let a = owner.fixture().a();
    let cls = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    unsafe { vacant(a, cls, a.transient_package(), "HotkeyFixtureReceiver"); }
    let receiver = unsafe { object(a, cls, a.transient_package(), "HotkeyFixtureReceiver", UCanvas::NATIVE_ALIGNMENT) };
    owner.root(receiver);
    let a = owner.fixture().a();
    let function_class = unsafe { class(a, "Core.Function", size_of::<UFunction>(), align_of::<UFunction>()) };
    unsafe { vacant(a, function_class, cls.cast(), "TileCanonicalBody"); }
    let f = unsafe { body(a, cls) };
    owner.root(f.cast());
    let props = unsafe { parameters(&mut owner, f) };
    // Capture a prefix shorter than the actual 20-byte local allocation.
    unsafe { (*f).parms_size = 12; }
    let a = owner.fixture().a();
    let code = program(a, props);
    owner.fx.as_mut().expect("live fixture").attach(f, code);
    owner.fixture().verify();
    let a = owner.fixture().a();
    assert!(!unsafe { a.current_script_scalar_local_matches(std::ptr::null_mut(), std::ptr::null_mut(), ValueInt, 0) }, "outside scope must reject before frame or property access");
    WITNESS_PROPERTIES.with(|p| { assert!(p.get().is_none()); p.set(Some(props)); });
    let witness_guard = WitnessSelectionGuard;
    let binding = unsafe { a.install_script_body_replacement("hotkey-witness-fixture", &[f], witness_replacement) }.expect("install witness body");
    let mut params = Params { x: -111, y: -222, scale: 0.51, x_extent: 3.0, y_extent: 3.0 };
    let result = unsafe { a.execute_event(receiver, f, (&raw mut params).cast(), ptr::null_mut()) };
    drop(binding); drop(witness_guard);
    result.expect("witness-only event succeeds after metadata is restored");
    assert_eq!((params.x, params.y), (-111, -222), "witness-only replacement did not execute the body");
    owner.finish().expect("checked owner shutdown");
}
