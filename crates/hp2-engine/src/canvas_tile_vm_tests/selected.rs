//! Selected 466 handoff through the actual current EngineOwner event.
use super::*;
use hp2_core::{ScriptBodyStatements, ScriptCallArguments, ScriptPreCallDecision, vm_owner::StepControl};

#[derive(Clone, Copy)]
struct Selection {
    function: *mut UFunction,
    canvas: *mut UCanvas,
    base: *const u8,
    len: usize,
    first_float: usize,
    descriptor: &'static VmNativeSpec,
}

thread_local! {
    static SELECTION: Cell<Option<Selection>> = const { Cell::new(None) };
    static OFFSETS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static HANDOFFS: Cell<usize> = const { Cell::new(0) };
}

struct SelectionGuard;
impl Drop for SelectionGuard {
    fn drop(&mut self) {
        SELECTION.with(|slot| slot.set(None));
        OFFSETS.with(|offsets| offsets.borrow_mut().clear());
        HANDOFFS.with(|count| count.set(0));
    }
}

fn current() -> RuntimeResult<Selection> {
    SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("selected tile fixture scope absent"))
}

// SAFETY: called only by the current selected native input. The fixture keeps
// the original script, Canvas and Function alive for the complete event.
unsafe fn evaluate_float(
    _: CoreAccess<'_>, args: &ScriptCallArguments<'_>, frame: *mut FFrame, destination: *mut f32,
) -> RuntimeResult<()> {
    let selection = current()?;
    if frame.is_null() || destination.is_null()
        || unsafe { ptr::addr_of!((*frame).node).read_unaligned().cast::<UFunction>() } != selection.function
        || unsafe { ptr::addr_of!((*frame).object).read_unaligned().cast::<UCanvas>() } != selection.canvas
        || args.body_script_snapshot() != (selection.base, selection.len)
    {
        return Err(RuntimeError::new("selected tile input is not the current admitted body"));
    }
    let ordinal = OFFSETS.with(|offsets| offsets.borrow().len());
    if ordinal >= 6 { return Err(RuntimeError::new("too many selected tile floats")); }
    let expected = selection.first_float + ordinal * 5;
    if expected.checked_add(5).is_none_or(|end| end > selection.len) {
        return Err(RuntimeError::new("selected tile float outside admitted script"));
    }
    // SAFETY: the original immutable box and its full length are retained by
    // the fixture; the current operand is an admitted FloatConst expression.
    let code = unsafe { ptr::addr_of!((*frame).code).read_unaligned().cast_const() };
    if code != unsafe { selection.base.add(expected) } || unsafe { selection.base.add(expected).read() } != 0x1e {
        return Err(RuntimeError::new("unexpected selected tile float offset or opcode"));
    }
    OFFSETS.with(|offsets| offsets.borrow_mut().push(expected));
    // SAFETY: the supplied current frame, fresh host and actual Engine f32
    // local remain live and writable for this single synchronous operation.
    unsafe { args.step_argument(frame, destination.cast()) }
}

// SAFETY: this replacement is installed only for the fixture's live Function.
// It operates the callback's current frame and does not retain it or its scope.
unsafe fn replacement(
    a: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame,
) -> RuntimeResult<ScriptPreCallDecision> {
    let selection = current()?;
    if frame.is_null() || function != selection.function || object.cast::<UCanvas>() != selection.canvas
        || unsafe { ptr::addr_of!((*frame).node).read_unaligned().cast::<UFunction>() } != function
        || unsafe { ptr::addr_of!((*frame).object).read_unaligned().cast::<UCanvas>() } != selection.canvas
    {
        return Err(RuntimeError::new("selected tile replacement is not the current admitted body"));
    }
    let mut statements = ScriptBodyStatements::new();
    while let Some(_) = unsafe { statements.begin(a, frame)? } {
        let live = unsafe { ptr::addr_of!((*frame).code).read_unaligned().cast_const() };
        let flow = if live == selection.base {
            HANDOFFS.with(|count| count.set(count.get() + 1));
            // SAFETY: the fixture admitted the immutable Context/native sites,
            // actual installed descriptor and complete typed continuation.
            unsafe {
                a.execute_script_context_tile_with(
                    frame, statements.result(), selection.base, selection.base.add(5),
                    selection.canvas.cast(), selection.descriptor,
                    super::super::health_draw_tile_selected, evaluate_float,
                )?
            }
        } else {
            unsafe { a.execute_script_body_step(frame, statements.result())? }
        };
        if flow == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

pub(super) fn execute(
    fx: &Fixture, a: CoreAccess<'_>, canvas: *mut UCanvas, function: *mut UFunction,
    descriptor: &'static VmNativeSpec, texture_index: Option<i32>, null_context: bool, debug: bool,
) -> RuntimeResult<()> {
    assert_eq!(fx.attached.len(), 1, "one immutable fixture body");
    let slot = &fx.attached[0];
    assert_eq!(slot.function, function, "admitted body identity");
    assert_eq!(slot.expected, program(Native::Tile, texture_index, null_context, 8.0, 6.0, debug), "admit complete immutable script and operand types");
    assert_eq!(slot.expected[0], 0x19);
    assert_eq!(&slot.expected[5..7], &[0x61, 0xd2]);
    let first_float = 7 + if texture_index.is_some() { 5 } else { 1 };
    let selection = Selection { function, canvas, base: slot.base.cast_const(), len: slot.expected.len(), first_float, descriptor };
    SELECTION.with(|state| { assert!(state.get().is_none(), "selection must not be nested"); state.set(Some(selection)); });
    let selection_guard = SelectionGuard;
    OFFSETS.with(|offsets| offsets.borrow_mut().clear());
    HANDOFFS.with(|count| count.set(0));
    // SAFETY: fx retains all selected records and script storage. The guard is
    // dropped before selection cleanup and before fx can detach or shut down.
    let binding = unsafe { a.install_script_body_replacement("selected-tile-fixture", &[function], replacement) }
        .expect("install selected fixture body replacement");
    let result = unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) };
    drop(binding);
    let actual = OFFSETS.with(|offsets| offsets.borrow().clone());
    let expected: Vec<usize> = if null_context { Vec::new() } else { (0..6).map(|n| first_float + n * 5).collect() };
    assert_eq!(HANDOFFS.with(Cell::get), 1, "one Context handoff; event result: {result:?}");
    assert_eq!(actual, expected, "exact selected float offsets; event result: {result:?}");
    drop(selection_guard);
    result
}

#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_null_texture() { run_case("selected-466-null-texture", Native::Tile, Case::NullTexture, false, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_null_texture_log_error() { run_case("selected-466-null-texture-log-error", Native::Tile, Case::NullTextureLogError, false, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_style_none() { run_case("selected-466-style-none", Native::Tile, Case::StyleNone, false, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_finish_error() { run_case("selected-466-finish-error", Native::Tile, Case::FinishError, false, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_null_context() { run_case("selected-466-null-context", Native::Tile, Case::NullContext, false, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_selected_466_null_context_strict() { run_case("selected-466-null-context-strict", Native::Tile, Case::NullContext, true, true); }
