//! Synthetic IntConst geometry under the actual selected 466 handoff.
//! Synthetic IntConst cases do not exercise Engine.Bitmap.USize or retail admission.
#[path = "geometry_real_property.rs"]
mod real_property;
use super::*;
use hp2_core::{ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision, vm_owner::StepControl};

const CONTEXT: usize = 0x2c9;
const NATIVE: usize = 0x2d2;
const FLOAT_STARTS: [usize; 6] = [0x2d9, 0x2ef, 0x2fb, 0x300, 0x305, 0x314];
const FLOAT_ENDS: [usize; 6] = [0x2ef, 0x2fb, 0x300, 0x305, 0x314, 0x319];

#[derive(Clone, Copy)]
struct Selection {
    function: *mut UFunction,
    canvas: *mut UCanvas,
    base: *const u8,
    len: usize,
    descriptor: &'static VmNativeSpec,
}
#[derive(Debug, PartialEq, Eq)]
struct FloatObservation { start: usize, end: usize, bits: u32 }
thread_local! {
    static SELECTION: Cell<Option<Selection>> = const { Cell::new(None) };
    static FLOATS: RefCell<Vec<FloatObservation>> = const { RefCell::new(Vec::new()) };
    static HANDOFFS: Cell<usize> = const { Cell::new(0) };
}
struct SelectionGuard;
impl Drop for SelectionGuard {
    fn drop(&mut self) {
        SELECTION.with(|value| value.set(None));
        FLOATS.with(|value| value.borrow_mut().clear());
        HANDOFFS.with(|value| value.set(0));
    }
}
fn current() -> RuntimeResult<Selection> {
    SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("geometry fixture selection absent"))
}

fn object_constant(code: &mut Vec<u8>, index: i32) {
    code.push(0x20);
    code.extend_from_slice(&index.to_le_bytes());
}
fn float_constant(code: &mut Vec<u8>, value: f32) {
    code.push(0x1e);
    code.extend_from_slice(&value.to_bits().to_le_bytes());
}
// The IntConst is deliberately not an invented USize property. With icon -1,
// Core's real ObjectConst resolves None and Context writes exactly 4 zero bytes.
fn int_context(code: &mut Vec<u8>, icon: i32) {
    code.push(0x19);
    object_constant(code, icon);
    code.extend_from_slice(&[5, 0, 4, 0x1d]);
    code.extend_from_slice(&13_i32.to_le_bytes());
}
fn program(canvas: i32, texture: i32, icon: i32, debug: bool) -> Vec<u8> {
    let mut code = vec![0x0b; CONTEXT]; // EX_Nothing statements
    code.push(0x19);
    object_constant(&mut code, canvas);
    let skip = code.len();
    code.extend_from_slice(&[0, 0, 0]); // skip, Context result width
    assert_eq!(code.len(), NATIVE);
    code.extend_from_slice(&[0x61, 0xd2]);
    object_constant(&mut code, texture);
    assert_eq!(code.len(), 0x2d9);
    code.extend_from_slice(&[0xab, 0x40]);
    int_context(&mut code, icon);
    float_constant(&mut code, 1.5);
    code.push(0x16);
    assert_eq!(code.len(), 0x2ef);
    code.push(0xab);
    float_constant(&mut code, 4.0);
    float_constant(&mut code, 1.5);
    code.push(0x16);
    assert_eq!(code.len(), 0x2fb);
    float_constant(&mut code, 1.0);
    float_constant(&mut code, 2.0);
    assert_eq!(code.len(), 0x305);
    code.push(0x40);
    int_context(&mut code, icon);
    assert_eq!(code.len(), 0x314);
    float_constant(&mut code, 7.0);
    assert_eq!(code.len(), 0x319);
    code.push(0x16);
    if debug {
        code.push(0x38);
        code.extend_from_slice(&100_i32.to_le_bytes());
        code.extend_from_slice(&7_i32.to_le_bytes());
        code.extend_from_slice(&9_i32.to_le_bytes());
        code.push(0);
    }
    let ret = code.len();
    code.extend_from_slice(&[0x04, 0x0b, 0x0b]);
    code[skip..skip + 2].copy_from_slice(&u16::try_from(ret - NATIVE).expect("bounded Context skip").to_le_bytes());
    code
}

// SAFETY: Core invokes this on the current handoff frame and actual Engine
// local; the fixture keeps the original admitted script and objects live.
unsafe fn evaluate(a: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame, destination: *mut f32) -> RuntimeResult<()> {
    let selection = current()?;
    if frame.is_null() || destination.is_null()
        || unsafe { ptr::addr_of!((*frame).node).read_unaligned().cast::<UFunction>() } != selection.function
        || unsafe { ptr::addr_of!((*frame).object).read_unaligned().cast::<UCanvas>() } != selection.canvas
        || args.body_script_snapshot() != (selection.base, selection.len)
    { return Err(RuntimeError::new("geometry input is not the current admitted body")); }
    let ordinal = FLOATS.with(|values| values.borrow().len());
    if ordinal >= FLOAT_STARTS.len() { return Err(RuntimeError::new("too many geometry float inputs")); }
    let code = unsafe { ptr::addr_of!((*frame).code).read_unaligned().cast_const() };
    if code != unsafe { selection.base.add(FLOAT_STARTS[ordinal]) } {
        return Err(RuntimeError::new("unexpected geometry float root"));
    }
    // SAFETY: complete immutable typed continuations and the current frame,
    // actual writable float destination and object lifetimes were admitted.
    unsafe { super::tile_geometry_policy::evaluate(a, args, frame, destination)?; }
    let after = unsafe { ptr::addr_of!((*frame).code).read_unaligned().cast_const() };
    if after != unsafe { selection.base.add(FLOAT_ENDS[ordinal]) } {
        return Err(RuntimeError::new("unexpected geometry float end"));
    }
    let bits = unsafe { destination.read_unaligned() }.to_bits();
    FLOATS.with(|values| values.borrow_mut().push(FloatObservation { start: FLOAT_STARTS[ordinal], end: FLOAT_ENDS[ordinal], bits }));
    Ok(())
}

// SAFETY: the guard installs this only on the live admitted fixture Function.
unsafe fn replacement(a: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let selection = current()?;
    if frame.is_null() || function != selection.function || object.cast::<UCanvas>() != selection.canvas
        || unsafe { ptr::addr_of!((*frame).node).read_unaligned().cast::<UFunction>() } != function
        || unsafe { ptr::addr_of!((*frame).object).read_unaligned().cast::<UCanvas>() } != selection.canvas
    { return Err(RuntimeError::new("geometry replacement is not the current admitted body")); }
    let mut statements = ScriptBodyStatements::new();
    while let Some(_) = unsafe { statements.begin(a, frame)? } {
        let live = unsafe { ptr::addr_of!((*frame).code).read_unaligned().cast_const() };
        let flow = if live == unsafe { selection.base.add(CONTEXT) } {
            HANDOFFS.with(|count| count.set(count.get() + 1));
            // SAFETY: the admitted bytecode, current owner and actual descriptor
            // remain live; no borrowed scope or frame escapes this callback.
            unsafe { a.execute_script_context_tile_with(frame, statements.result(), selection.base.add(CONTEXT), selection.base.add(NATIVE), selection.canvas.cast(), selection.descriptor, super::super::health_draw_tile_selected, evaluate)? }
        } else {
            unsafe { a.execute_script_body_step(frame, statements.result())? }
        };
        if flow == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

fn warning(a: CoreAccess<'_>, canvas: *mut UCanvas, function: *mut UFunction, offset: usize) -> Vec<u32> {
    let mut text = unsafe { full_name(a, canvas.cast()) };
    text.extend(utf32(" ("));
    text.extend(unsafe { full_name(a, function.cast()) });
    text.extend(utf32(&format!(":{offset:04X}) Accessed None: Unknown")));
    text
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    before: CanvasBits,
    after: CanvasBits,
    logs: Vec<(Vec<u32>, Vec<u32>)>,
    debug_calls: u32,
    error: Option<(Vec<u32>, Vec<Vec<u32>>, Option<PathBuf>)>,
}

fn run_one(label: &str, null_icon: bool, debug: bool, selected: bool, expected_script: Option<&[u8]>) -> (Vec<u8>, Observed) {
    let mode = if selected { "selected" } else { "canonical" };
    let root_label = format!("{label}-{mode}");
    let mut fx = Fixture::new(&root_label, Native::Tile, Case::StyleNone, false);
    let a = fx.a();
    let descriptor = installed(a, Native::Tile);
    let canvas_class = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    let canvas = unsafe { object(a, canvas_class, a.transient_package(), "TileCanonicalCanvas", UCanvas::NATIVE_ALIGNMENT) }.cast::<UCanvas>();
    let texture_class = unsafe { class(a, "Engine.Texture", size_of::<UTexture>(), UTexture::NATIVE_ALIGNMENT) };
    let texture = unsafe { object(a, texture_class, a.transient_package(), "TileCanonicalTexture", UTexture::NATIVE_ALIGNMENT) }.cast::<UTexture>();
    let function = unsafe { body(a, canvas_class) };
    let canvas_index = unsafe { ptr::addr_of!((*canvas.cast::<UObject>()).index).read_unaligned() };
    let texture_index = unsafe { ptr::addr_of!((*texture.cast::<UObject>()).index).read_unaligned() };
    assert!(canvas_index >= 0 && texture_index >= 0);
    assert!(a.object_at(-1).is_null(), "ObjectConst -1 resolves to None");
    let code = program(canvas_index, texture_index, if null_icon { -1 } else { texture_index }, debug);
    exclusive_write(&fx.root.join("geometry-script.bin"), &code).expect("record synthetic immutable script");
    if let Some(expected) = expected_script {
        assert_eq!(code.as_slice(), expected, "independent owners must admit identical script bytes before execution");
    }
    exclusive_write(&fx.root.join("geometry-inputs.txt"), format!("mode={mode} synthetic IntConst=13 scale=1.5 height=4 U=1 V=2 VL=7 null_icon={null_icon} debug={debug}\ninputs.txt records base fixture defaults only, not geometry operands\n").as_bytes()).expect("record geometry inputs");
    let script = code.clone();
    fx.attach(function, code);
    fx.verify();
    let a = fx.a();
    unsafe { set_canvas(canvas, 0); }
    let before = unsafe { snapshot(canvas) };
    LOGS.with(|logs| logs.borrow_mut().clear());
    DEBUG_CALLS.with(|count| count.set(0));
    FAIL_OUTPUT.with(|fail| fail.set(false));
    FLOATS.with(|values| values.borrow_mut().clear());
    HANDOFFS.with(|count| count.set(0));
    let debug_guard = DebugGuard(a);
    if debug { a.set_debugger(Some(Rc::new(FinishFailure))); }
    let (result, floats, handoffs) = if selected {
        let slot = &fx.attached[0];
        let selection = Selection { function, canvas, base: slot.base.cast_const(), len: slot.expected.len(), descriptor };
        SELECTION.with(|state| { assert!(state.get().is_none(), "geometry selection must not be nested"); state.set(Some(selection)); });
        let selection_guard = SelectionGuard;
        FLOATS.with(|values| values.borrow_mut().clear());
        HANDOFFS.with(|count| count.set(0));
        // SAFETY: fx retains the actual Function, Canvas, Texture and immutable
        // fully-typed script. The binding drops before detach and shutdown.
        let binding = unsafe { a.install_script_body_replacement("synthetic-geometry-fixture", &[function], replacement) }.expect("install geometry replacement");
        let result = unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) };
        drop(binding);
        let floats = FLOATS.with(|values| values.borrow_mut().drain(..).collect::<Vec<_>>());
        let handoffs = HANDOFFS.with(Cell::get);
        drop(selection_guard);
        (result, floats, handoffs)
    } else {
        SELECTION.with(|state| assert!(state.get().is_none(), "canonical event cannot inherit selected scope"));
        // SAFETY: this independent owner retains the same complete typed script
        // and records throughout the ordinary canonical event.
        (unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) }, Vec::new(), 0)
    };
    drop(debug_guard);
    let after = unsafe { snapshot(canvas) };
    let logs = LOGS.with(|logs| logs.borrow().clone());
    let debug_calls = DEBUG_CALLS.with(Cell::get);
    let error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec(), e.source_path().map(Path::to_path_buf)));
    exclusive_write(&fx.root.join("geometry-observed.txt"), format!("handoffs={handoffs} floats={floats:?} before={before:?} after={after:?} logs={logs:?} debug={debug_calls} error={error:?}\n").as_bytes()).expect("record geometry observation");
    fx.verify();
    if selected {
        assert_eq!(handoffs, 1, "one real selected Context handoff");
        let width = if null_icon { 0.0_f32 } else { 19.5_f32 };
        let source = if null_icon { 0.0_f32 } else { 13.0_f32 };
        let expected_bits = [width, 6.0, 1.0, 2.0, source, 7.0].map(f32::to_bits);
        let expected: Vec<_> = (0..6).map(|i| FloatObservation { start: FLOAT_STARTS[i], end: FLOAT_ENDS[i], bits: expected_bits[i] }).collect();
        assert_eq!(floats, expected, "exact roots, end cursors and independently computed values");
    }
    let warnings = if null_icon { vec![(utf32("ScriptWarning"), warning(a, canvas, function, 0x2e1)), (utf32("ScriptWarning"), warning(a, canvas, function, 0x30c))] } else { Vec::new() };
    assert_eq!(logs, warnings, "exact nested Context warning output");
    if debug {
        let expected_contexts = vec![context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecDebugInfo), 56), context_error(CanonicalNativeBinding::External(descriptor.handler), 466), context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecContext), 25), utf32("stepping Core VM bytecode"), utf32(&format!("Core event {:#x}", function as usize))];
        assert_eq!(error, Some((utf32("fixture tile finish debug error"), expected_contexts, None)));
        assert_eq!(debug_calls, 1);
        assert_eq!(after, before, "finish failure precedes Canvas effects");
    } else {
        assert_eq!(error, None);
        assert_eq!(debug_calls, 0);
        let mut expected_canvas = before.clone();
        expected_canvas.fields[0] = if null_icon { (-1.5_f32).to_bits() } else { 18.0_f32.to_bits() };
        expected_canvas.fields[2] = 6.0_f32.to_bits();
        assert_eq!(after, expected_canvas, "independent Canvas cursor and height bits");
    }
    // The complete raw outcome was written before assertions. Only after the
    // exact error stack above has been asserted, normalize this owner's event
    // pointer for the cross-owner comparison; retain every other context byte.
    let mut normalized_error = error;
    if let Some((_, contexts, _)) = &mut normalized_error {
        let event = contexts.last_mut().expect("expected event context already asserted");
        assert_eq!(*event, utf32(&format!("Core event {:#x}", function as usize)));
        *event = utf32("Core event <owner-specific function>");
    }
    let observed = Observed { before, after, logs, debug_calls, error: normalized_error };
    fx.finish().expect("checked EngineOwner shutdown");
    (script, observed)
}

fn run(label: &str, null_icon: bool, debug: bool) {
    let (canonical_script, canonical) = run_one(label, null_icon, debug, false, None);
    let (selected_script, selected) = run_one(label, null_icon, debug, true, Some(&canonical_script));
    assert_eq!(canonical_script, selected_script, "independent owners must execute identical immutable script bytes");
    assert_eq!(canonical, selected, "full canonical and selected outcomes must match");
}

#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_synthetic_geometry_constants() { run("synthetic-geometry-constants", false, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_synthetic_geometry_constants_null_icon() { run("synthetic-geometry-constants-null-icon", true, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_synthetic_geometry_constants_finish_error() { run("synthetic-geometry-constants-finish-error", false, true); }
