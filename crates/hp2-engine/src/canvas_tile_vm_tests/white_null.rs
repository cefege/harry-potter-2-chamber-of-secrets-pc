// Fixed IntConstByte white channels: all-null sink and synthetic G-null failure.
// Included only as a child of the accepted white_normal test module.
use super::*;

#[derive(Clone, Copy, Debug)]
enum Scenario { AllNull, GreenNullOutputFailure }
impl Scenario {
    fn label(self) -> &'static str { match self { Self::AllNull => "white-all-null", Self::GreenNullOutputFailure => "white-green-null-output-failure" } }
}
thread_local! {
    // Values read synchronously after each successful actual Let callback.
    static WRITES: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
struct WriteGuard;
impl Drop for WriteGuard { fn drop(&mut self) { WRITES.with(|v| v.borrow_mut().clear()); } }

// SAFETY: the current Let host owns the actual u8 destination and frame. No
// address or borrowed capability is retained after this synchronous call.
unsafe fn observe(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void, i: usize) -> RuntimeResult<()> {
    unsafe { trace(a, s, f, o, d, i)?; }
    let byte = unsafe { d.cast::<u8>().read_unaligned() };
    WRITES.with(|v| v.borrow_mut().push(byte));
    Ok(())
}
unsafe fn red(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { observe(a, s, f, o, d, 0) } }
unsafe fn green(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { observe(a, s, f, o, d, 1) } }
unsafe fn blue(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { observe(a, s, f, o, d, 2) } }

// SAFETY: the binding runs only for the admitted current fixture Function.
unsafe fn replacement_for_cases(a: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let selection = SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("white case selection absent"))?;
    if frame.is_null() || function != selection.function || object.cast::<UCanvas>() != selection.canvas
        || unsafe { (*frame).node.cast::<UFunction>() } != function || unsafe { (*frame).object.cast::<UCanvas>() } != selection.canvas
    { return Err(RuntimeError::new("white case current body mismatch")); }
    let mut statements = ScriptBodyStatements::new();
    while let Some(_) = unsafe { statements.begin(a, frame)? } {
        let live = unsafe { (*frame).code.cast_const() };
        let current = SITES.iter().position(|(site, _)| live == unsafe { selection.base.add(*site) });
        let step = if let Some(i) = current {
            let callback: hp2_core::ScriptAssignmentValue = match i { 0 => red, 1 => green, _ => blue };
            unsafe { a.execute_script_let_with_rhs(frame, statements.result(), selection.base.add(SITES[i].0), selection.base.add(SITES[i].1), callback)? }
        } else { unsafe { a.execute_script_body_step(frame, statements.result())? } };
        if step == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

fn message(a: CoreAccess<'_>, canvas: *mut UCanvas, f: *mut UFunction, offset: usize, text: &str) -> Vec<u32> {
    let mut value = unsafe { full_name(a, canvas.cast()) };
    value.extend(utf32(" ("));
    value.extend(unsafe { full_name(a, f.cast()) });
    value.extend(utf32(&format!(":{offset:04X}) {text}")));
    value
}
fn expected_logs(a: CoreAccess<'_>, canvas: *mut UCanvas, f: *mut UFunction, case: Scenario) -> Vec<(Vec<u32>, Vec<u32>)> {
    match case {
        Scenario::AllNull => {
            let mut entries = Vec::new();
            for (context, rhs) in [(0x126, 0x12e), (0x13d, 0x145), (0x154, 0x15c)] {
                entries.push((utf32("ScriptWarning"), message(a, canvas, f, context, "Accessed None: WhiteFixtureCanvas")));
                entries.push((utf32("ScriptWarning"), message(a, canvas, f, rhs, "Attempt to assigned variable through None")));
            }
            entries
        }
        // ObjectConst(-1) does not alter lvalue slots. R's successful
        // StructMember left the last property at Color.R before this warning.
        Scenario::GreenNullOutputFailure => vec![(utf32("ScriptWarning"), message(a, canvas, f, 0x13d, "Accessed None: R"))],
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CaseOutcome { before: CanvasState, after: CanvasState, logs: Vec<(Vec<u32>, Vec<u32>)>, error: Option<(Vec<u32>, Vec<Vec<u32>>, Option<PathBuf>)>, debug_calls: u32 }

fn run_case(case: Scenario, selected: bool, expected_script: Option<&[u8]>) -> (Vec<u8>, CaseOutcome) {
    let mode = if selected { "selected" } else { "canonical" };
    let mut fx = Fixture::new(&format!("{}-{mode}", case.label()), Native::Tile, Case::StyleNone, false);
    let a = fx.a();
    let cls = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    let canvas = unsafe { object(a, cls, a.transient_package(), "WhiteFixtureCanvas", UCanvas::NATIVE_ALIGNMENT) }.cast::<UCanvas>();
    // Reuse the independently accepted normal fixture's actual same-owner
    // indexed Canvas/Color descriptor and local parameter assertions.
    let metadata = unsafe { metadata(a, cls) };
    let function = unsafe { body(a, cls) };
    let local = unsafe { canvas_parameter(a, function, cls) };
    let mut code = program(a, local, &metadata);
    if matches!(case, Scenario::GreenNullOutputFailure) {
        assert!(a.object_at(-1).is_null(), "owner ObjectConst(-1) resolves None");
        assert_eq!(code[0x138], 0x00, "replace the synthetic G receiver before event only");
        code[0x138] = 0x20;
        code[0x139..0x13d].copy_from_slice(&(-1_i32).to_le_bytes());
    }
    for (_, rhs) in SITES { assert_eq!(&code[rhs..rhs + 3], &[0x3e, 0x2c, 0xff]); }
    exclusive_write(&fx.root.join("white-case-script.bin"), &code).expect("record immutable case script");
    if let Some(expected) = expected_script { assert_eq!(code.as_slice(), expected, "both owners use identical immutable bytes before execution"); }
    exclusive_write(&fx.root.join("white-case-inputs.txt"), format!("mode={mode} case={case:?} non-strict fixed RHS IntConstByte=255 RGBA=11,22,33,44; synthetic Function local; G ObjectConst(-1) only in partial variant\ninputs.txt contains base fixture defaults only\n").as_bytes()).expect("record case inputs");
    let script = code.clone();
    fx.attach(function, code);
    fx.verify();
    let a = fx.a();
    unsafe { set_canvas(canvas, 0); (*canvas).color = [11, 22, 33, 44]; }
    let before = unsafe { canvas_state(canvas) };
    assert_eq!(before.pointers, [0; 8], "cross-owner pointer comparison");
    LOGS.with(|v| v.borrow_mut().clear()); DEBUG_CALLS.with(|v| v.set(0));
    FAIL_OUTPUT.with(|v| v.set(matches!(case, Scenario::GreenNullOutputFailure)));
    let output_guard = OutputGuard;
    let mut parameter = if matches!(case, Scenario::AllNull) { ptr::null_mut::<UObject>() } else { canvas.cast::<UObject>() };
    let initial_parameter = parameter;
    let (result, roots, writes) = if selected {
        let slot = &fx.attached[0];
        SELECTION.with(|v| { assert!(v.get().is_none()); v.set(Some(Selection { function, canvas, base: slot.base.cast_const() })); });
        let selection_guard = SelectionGuard;
        let write_guard = WriteGuard;
        ROOTS.with(|v| v.borrow_mut().clear()); WRITES.with(|v| v.borrow_mut().clear());
        // SAFETY: actual owner, original immutable typed bytecode and records
        // stay live until the replacement is uninstalled and event has ended.
        let binding = unsafe { a.install_script_body_replacement("white-channel-null-fixture", &[function], replacement_for_cases) }.expect("install selected replacement");
        let result = unsafe { a.execute_event(canvas.cast(), function, (&raw mut parameter).cast(), ptr::null_mut()) };
        drop(binding);
        let roots = ROOTS.with(|v| v.borrow_mut().drain(..).collect::<Vec<_>>());
        let writes = WRITES.with(|v| v.borrow_mut().drain(..).collect::<Vec<_>>());
        drop(write_guard); drop(selection_guard);
        (result, roots, writes)
    } else {
        SELECTION.with(|v| assert!(v.get().is_none()));
        (unsafe { a.execute_event(canvas.cast(), function, (&raw mut parameter).cast(), ptr::null_mut()) }, Vec::new(), Vec::new())
    };
    drop(output_guard);
    let after = unsafe { canvas_state(canvas) };
    let logs = LOGS.with(|v| v.borrow().clone());
    let debug_calls = DEBUG_CALLS.with(Cell::get);
    let mut error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec(), e.source_path().map(Path::to_path_buf)));
    exclusive_write(&fx.root.join("white-case-observed.txt"), format!("roots={roots:?} writes={writes:?} before={before:?} after={after:?} logs={logs:?} error={error:?} debug={debug_calls} parameter={parameter:p}\n").as_bytes()).expect("record raw case outcome");
    fx.verify();
    assert_eq!(parameter, initial_parameter, "Function parameter unchanged");
    assert_eq!(logs, expected_logs(a, canvas, function, case), "exact first/all warnings and order");
    assert_eq!(debug_calls, 0);
    assert_eq!(after.geometry, before.geometry); assert_eq!(after.bits, before.bits);
    assert_eq!(after.size, before.size); assert_eq!(after.pointers, before.pointers);
    match case {
        Scenario::AllNull => {
            assert_eq!(error, None);
            assert_eq!(after.color, [11, 22, 33, 44], "real Canvas untouched; all 3 Lets use Core sinks");
            if selected { assert_eq!(roots, vec![0x12e, 0x145, 0x15c]); assert_eq!(writes, vec![255, 255, 255], "actual sink byte after each selected conversion"); }
        }
        Scenario::GreenNullOutputFailure => {
            assert_eq!(after.color, [255, 22, 33, 44], "R committed before failing G warning; G/B unchanged");
            if selected { assert_eq!(roots, vec![0x12e]); assert_eq!(writes, vec![255]); }
            let contexts = vec![
                context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecContext), 25),
                context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecStructMember), 54),
                context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecLet), 15),
                utf32("stepping Core VM bytecode"),
                utf32(&format!("Core event {:#x}", function as usize)),
            ];
            assert_eq!(error, Some((utf32("fixture tile output error"), contexts, None)), "exact first output failure and canonical contexts");
            let (_, stack, _) = error.as_mut().expect("asserted error");
            let event = stack.last_mut().expect("asserted event context");
            assert_eq!(*event, utf32(&format!("Core event {:#x}", function as usize)));
            *event = utf32("Core event <owner-specific function>");
        }
    }
    let outcome = CaseOutcome { before, after, logs, error, debug_calls };
    fx.finish().expect("checked EngineOwner shutdown");
    (script, outcome)
}
fn compare(case: Scenario) {
    let (script, canonical) = run_case(case, false, None);
    let (other, selected) = run_case(case, true, Some(&script));
    assert_eq!(script, other);
    assert_eq!(canonical, selected, "all observable named Canvas/log/error/debug state");
}
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_white_channels_all_null_sink() { compare(Scenario::AllNull); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_white_channels_green_null_output_failure() { compare(Scenario::GreenNullOutputFailure); }
