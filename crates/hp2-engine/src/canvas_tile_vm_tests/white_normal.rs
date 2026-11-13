//! Fixed IntConstByte(255) white-channel comparison with actual startup metadata.
//! The Function/local Canvas parameter is synthetic; Canvas/Color fields are not.
//! No null, strict, partial-effect, redirected-child or retail coverage is claimed.
use super::*;
use hp2_abi::engine::generated::{FColor, UObjectProperty, UProperty, UStructProperty};
use hp2_core::{ScriptBodyStatements, ScriptPreCallDecision, vm_owner::StepControl};
use std::mem::offset_of;

const SITES: [(usize, usize); 3] = [(0x11a, 0x12e), (0x131, 0x145), (0x148, 0x15c)];
#[derive(Clone, Copy)]
struct Selection { function: *mut UFunction, canvas: *mut UCanvas, base: *const u8 }
thread_local! {
    static SELECTION: Cell<Option<Selection>> = const { Cell::new(None) };
    static ROOTS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}
struct SelectionGuard;
impl Drop for SelectionGuard {
    fn drop(&mut self) { SELECTION.with(|s| s.set(None)); ROOTS.with(|r| r.borrow_mut().clear()); }
}

// SAFETY: Core calls this synchronously for the installed Function and its
// current live frame. fx retains the complete immutable typed program.
unsafe fn replacement(a: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame) -> RuntimeResult<ScriptPreCallDecision> {
    let selection = SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("white fixture selection absent"))?;
    if frame.is_null() || function != selection.function || object.cast::<UCanvas>() != selection.canvas
        || unsafe { (*frame).node.cast::<UFunction>() } != function
        || unsafe { (*frame).object.cast::<UCanvas>() } != selection.canvas
    { return Err(RuntimeError::new("white fixture current body mismatch")); }
    let mut statements = ScriptBodyStatements::new();
    while let Some(_) = unsafe { statements.begin(a, frame)? } {
        let live = unsafe { (*frame).code.cast_const() };
        let current = SITES.iter().position(|(site, _)| live == unsafe { selection.base.add(*site) });
        let step = if let Some(i) = current {
            // The current Let owns its actual LHS, destination and RHS gate.
            let callback: hp2_core::ScriptAssignmentValue = match i {
                0 => trace_red, 1 => trace_green, _ => trace_blue,
            };
            unsafe { a.execute_script_let_with_rhs(frame, statements.result(), selection.base.add(SITES[i].0), selection.base.add(SITES[i].1), callback)? }
        } else {
            unsafe { a.execute_script_body_step(frame, statements.result())? }
        };
        if step == StepControl::Return { break; }
    }
    Ok(ScriptPreCallDecision::Consume)
}

// Only the actually invoked current RHS is recorded; the frame is not retained.
unsafe fn trace(a: CoreAccess<'_>, args: &hp2_core::ScriptCallArguments<'_>, frame: *mut FFrame, object: *mut std::ffi::c_void, dst: *mut std::ffi::c_void, index: usize) -> RuntimeResult<()> {
    let selection = SELECTION.with(Cell::get).ok_or_else(|| RuntimeError::new("white trace selection absent"))?;
    if args.body_script_snapshot().0 != selection.base || unsafe { (*frame).code.cast_const() } != unsafe { selection.base.add(SITES[index].1) } {
        return Err(RuntimeError::new("white trace RHS mismatch"));
    }
    ROOTS.with(|v| v.borrow_mut().push(SITES[index].1));
    let call: hp2_core::ScriptAssignmentValue = match index {
        0 => super::white_channels::white_red,
        1 => super::white_channels::white_green,
        _ => super::white_channels::white_blue,
    };
    unsafe { call(a, args, frame, object, dst) }
}
unsafe fn trace_red(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { trace(a, s, f, o, d, 0) } }
unsafe fn trace_green(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { trace(a, s, f, o, d, 1) } }
unsafe fn trace_blue(a: CoreAccess<'_>, s: &hp2_core::ScriptCallArguments<'_>, f: *mut FFrame, o: *mut std::ffi::c_void, d: *mut std::ffi::c_void) -> RuntimeResult<()> { unsafe { trace(a, s, f, o, d, 2) } }

// These are actual owner-provided descriptors, never constructed by this test.
// SAFETY: owner/bootstrap retains the registered class and its linked fields.
unsafe fn field(a: CoreAccess<'_>, owner: *mut UStruct, name: &str, kind: &str, offset: usize, width: i32) -> *mut UProperty {
    unsafe { indexed(a, owner.cast()); }
    let kind = a.native_class(kind).expect("actual property class");
    unsafe { indexed(a, kind.cast()); }
    let n = a.intern_str(name, FindName::Find).expect("registered field name");
    let p = unsafe { a.find_field(owner, n, kind) }.cast::<UProperty>();
    unsafe { indexed(a, p.cast()); }
    let record = p.cast::<UObject>();
    assert_eq!(unsafe { (*record).class.cast::<UClass>() }, kind, "actual exact property class");
    assert_eq!(unsafe { (*record).outer.cast::<UStruct>() }, owner, "actual property outer");
    assert_eq!(i32::from_ne_bytes(unsafe { (*record).name }), n);
    assert_eq!(unsafe { (*p).array_dim }, 1);
    assert_eq!(unsafe { (*p).element_size }, width);
    assert_eq!(unsafe { (*p).offset }, i32::try_from(offset).expect("bounded offset"));
    p
}

struct Metadata { draw: *mut UProperty, channels: [*mut UProperty; 3] }
unsafe fn metadata(a: CoreAccess<'_>, canvas_class: *mut UClass) -> Metadata {
    let draw = unsafe { field(a, canvas_class.cast(), "DrawColor", "Core.StructProperty", offset_of!(UCanvas, color), 4) };
    let color = unsafe { (*draw.cast::<UStructProperty>()).r#struct.cast::<UStruct>() };
    unsafe { indexed(a, color.cast()); }
    assert_eq!(unsafe { (*color.cast::<UObject>()).class.cast::<UClass>() }, a.native_class("Core.Struct").expect("Struct class"));
    assert_eq!(unsafe { (*color.cast::<UObject>()).outer.cast::<UClass>() }, a.native_class("Core.Object").expect("Object class"));
    assert_eq!(unsafe { a.object_path_text(color.cast()).expect("Color path") }, utf32("Core.Object.Color"));
    assert_eq!(unsafe { (*color).properties_size }, size_of::<FColor>() as i32);
    assert_eq!(unsafe { (*color).properties_align }, align_of::<FColor>() as i32);
    assert!(unsafe { (*color).constructor_link.is_null() }, "unmanaged Color");
    let r = unsafe { field(a, color, "R", "Core.ByteProperty", offset_of!(FColor, r), 1) };
    let g = unsafe { field(a, color, "G", "Core.ByteProperty", offset_of!(FColor, g), 1) };
    let b = unsafe { field(a, color, "B", "Core.ByteProperty", offset_of!(FColor, b), 1) };
    let alpha = unsafe { field(a, color, "A", "Core.ByteProperty", offset_of!(FColor, a), 1) };
    assert!(r != g && r != b && r != alpha && g != b && g != alpha && b != alpha);
    Metadata { draw, channels: [r, g, b] }
}

// A synthetic Function-owned parameter; never substitutes a Canvas/Color field.
// SAFETY: Function and registered ObjectProperty class/default are live in owner.
unsafe fn canvas_parameter(a: CoreAccess<'_>, function: *mut UFunction, canvas_class: *mut UClass) -> *mut UProperty {
    let kind = a.native_class("Core.ObjectProperty").expect("ObjectProperty class");
    unsafe { indexed(a, kind.cast()); }
    let defaults = unsafe { a.default_object(kind).expect("ObjectProperty defaults") };
    assert_eq!(unsafe { (*defaults).class.cast::<UClass>() }, kind);
    assert!(unsafe { (*defaults).outer.is_null() });
    assert!(unsafe { (*kind.cast::<UStruct>()).constructor_link.is_null() });
    assert!(unsafe { (*kind).properties_size } >= size_of::<UObjectProperty>() as i32);
    let name = a.intern_str("WhiteFixtureCanvas", FindName::Add).expect("fixture parameter name");
    let p = unsafe { a.construct_object(kind, function.cast(), name, 0x4000, ptr::null_mut()).expect("owner-constructed synthetic local") }.cast::<UObjectProperty>();
    unsafe { indexed(a, p.cast()); }
    assert_eq!(unsafe { (*p.cast::<UObject>()).class.cast::<UClass>() }, kind);
    assert_eq!(unsafe { (*p.cast::<UObject>()).outer.cast::<UFunction>() }, function);
    let prop = p.cast::<UProperty>();
    unsafe {
        (*prop).array_dim = 1; (*prop).element_size = size_of::<*mut UObject>() as i32;
        (*prop).offset = 0; (*prop).property_flags = 0x80;
        (*p).property_class = canvas_class.cast();
        assert!((*prop).constructor_link_next.is_null());
        assert!((*prop.cast::<UField>()).next.is_null());
        (*function.cast::<UStruct>()).children = prop.cast();
        (*function.cast::<UStruct>()).properties_size = size_of::<*mut UObject>() as i32;
        (*function.cast::<UStruct>()).properties_align = align_of::<*mut UObject>() as i32;
        (*function).parms_size = size_of::<*mut UObject>() as u16;
        (*function).num_parms = 1;
        assert!((*function.cast::<UStruct>()).constructor_link.is_null());
    }
    prop
}
fn operand(code: &mut Vec<u8>, op: u8, a: CoreAccess<'_>, obj: *mut UObject) {
    unsafe { indexed(a, obj); }
    code.push(op);
    code.extend_from_slice(&unsafe { (*obj).index }.to_le_bytes());
}
fn program(a: CoreAccess<'_>, local: *mut UProperty, metadata: &Metadata) -> Vec<u8> {
    let mut code = vec![0x0b; 0x11a];
    for (i, &(site, rhs)) in SITES.iter().enumerate() {
        assert_eq!(code.len(), site);
        code.push(0x0f);
        operand(&mut code, 0x36, a, metadata.channels[i].cast());
        code.push(0x19);
        operand(&mut code, 0x00, a, local.cast());
        code.extend_from_slice(&[5, 0, 4]);
        operand(&mut code, 0x01, a, metadata.draw.cast());
        assert_eq!(code.len(), rhs);
        code.extend_from_slice(&[0x3e, 0x2c, 0xff]);
    }
    assert_eq!(code.len(), 0x15f);
    code.extend_from_slice(&[0x04, 0x0b, 0x0b]);
    code
}

// All named Canvas-owned state, excluding UObject identity and opaque padding.
#[derive(Debug, PartialEq, Eq)]
struct CanvasState { geometry: CanvasBits, color: [u8; 4], bits: u8, size: [i32; 2], pointers: [usize; 8] }
unsafe fn canvas_state(c: *mut UCanvas) -> CanvasState {
    unsafe { CanvasState {
        geometry: snapshot(c), color: (*c).color,
        bits: c.cast::<u8>().add(offset_of!(UCanvas, color) + 4).read(), size: [(*c).x, (*c).y],
        pointers: [(*c).font as usize, (*c).small_font as usize, (*c).med_font as usize, (*c).big_font as usize, (*c).large_font as usize, (*c).viewport as usize, (*c).frame as usize, (*c).render as usize],
    } }
}
#[derive(Debug, PartialEq, Eq)]
struct Outcome { before: CanvasState, after: CanvasState, logs: Vec<(Vec<u32>, Vec<u32>)>, error: Option<(Vec<u32>, Vec<Vec<u32>>, Option<PathBuf>)>, debug_calls: u32 }

fn run_one(selected: bool, expected_script: Option<&[u8]>) -> (Vec<u8>, Outcome) {
    let mode = if selected { "selected" } else { "canonical" };
    let mut fx = Fixture::new(&format!("white-normal-{mode}"), Native::Tile, Case::StyleNone, false);
    let a = fx.a();
    let cls = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    let canvas = unsafe { object(a, cls, a.transient_package(), "WhiteFixtureCanvas", UCanvas::NATIVE_ALIGNMENT) }.cast::<UCanvas>();
    let m = unsafe { metadata(a, cls) };
    let f = unsafe { body(a, cls) };
    let local = unsafe { canvas_parameter(a, f, cls) };
    let code = program(a, local, &m);
    exclusive_write(&fx.root.join("white-script.bin"), &code).expect("record immutable script");
    if let Some(expected) = expected_script { assert_eq!(code.as_slice(), expected, "identical owner-local index bytes before event"); }
    exclusive_write(&fx.root.join("white-inputs.txt"), format!("mode={mode}; actual owner Canvas/Color fields; synthetic Function local; RGBA=11,22,33,44; fixed IntConstByte=255\ninputs.txt records base fixture defaults only\n").as_bytes()).expect("record inputs");
    let saved_code = code.clone();
    fx.attach(f, code);
    fx.verify();
    let a = fx.a();
    unsafe { set_canvas(canvas, 0); (*canvas).color = [11, 22, 33, 44]; }
    let before = unsafe { canvas_state(canvas) };
    assert_eq!(before.pointers, [0; 8], "cross-owner Canvas pointer state is comparable");
    LOGS.with(|v| v.borrow_mut().clear()); DEBUG_CALLS.with(|v| v.set(0)); FAIL_OUTPUT.with(|v| v.set(false));
    let output_guard = OutputGuard;
    let mut parameter = canvas.cast::<UObject>();
    let (result, roots) = if selected {
        let slot = &fx.attached[0];
        SELECTION.with(|v| { assert!(v.get().is_none()); v.set(Some(Selection { function: f, canvas, base: slot.base.cast_const() })); });
        let guard = SelectionGuard;
        ROOTS.with(|v| v.borrow_mut().clear());
        // SAFETY: original typed fixture script and owner records stay live.
        let binding = unsafe { a.install_script_body_replacement("white-channel-fixture", &[f], replacement) }.expect("install replacement");
        let outcome = unsafe { a.execute_event(canvas.cast(), f, (&raw mut parameter).cast(), ptr::null_mut()) };
        drop(binding);
        let roots = ROOTS.with(|v| v.borrow_mut().drain(..).collect());
        drop(guard);
        (outcome, roots)
    } else {
        (unsafe { a.execute_event(canvas.cast(), f, (&raw mut parameter).cast(), ptr::null_mut()) }, Vec::new())
    };
    drop(output_guard);
    let after = unsafe { canvas_state(canvas) };
    let logs = LOGS.with(|v| v.borrow().clone());
    let debug_calls = DEBUG_CALLS.with(Cell::get);
    let error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec(), e.source_path().map(Path::to_path_buf)));
    exclusive_write(&fx.root.join("white-observed.txt"), format!("roots={roots:?} before={before:?} after={after:?} logs={logs:?} error={error:?} debug={debug_calls} parameter={parameter:p}\n").as_bytes()).expect("record raw outcome");
    fx.verify();
    assert_eq!(parameter, canvas.cast(), "Function input remains unchanged");
    if selected { assert_eq!(roots, vec![0x12e, 0x145, 0x15c], "three live roots in original order"); }
    assert_eq!(error, None); assert!(logs.is_empty()); assert_eq!(debug_calls, 0);
    assert_eq!(after.color, [255, 255, 255, 44]);
    assert_eq!(after.geometry, before.geometry); assert_eq!(after.bits, before.bits);
    assert_eq!(after.size, before.size); assert_eq!(after.pointers, before.pointers);
    let result = Outcome { before, after, logs, error, debug_calls };
    fx.finish().expect("checked owner shutdown");
    (saved_code, result)
}

#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_white_channels_normal_comparison() {
    let (script, canonical) = run_one(false, None);
    let (other_script, selected) = run_one(true, Some(&script));
    assert_eq!(script, other_script);
    assert_eq!(canonical, selected, "full named Canvas state and log/error/debug outcome");
}

// Additional separately registered failure-domain comparisons.
#[cfg(test)]
mod null_cases {
    include!("white_null.rs");
}
