//! Actual-Core, no-device characterization of canonical Canvas tile natives.
//! Synthetic metadata bypasses retail HUD admission; this does not qualify rendering.
#[path = "canvas_tile_vm_tests/selected.rs"]
mod selected;
#[path = "ui_health_tile_geometry.rs"]
mod tile_geometry_policy;
#[path = "canvas_tile_vm_tests/geometry.rs"]
mod geometry;
#[path = "ui_health_white_channels.rs"]
mod white_channels;
#[path = "canvas_tile_vm_tests/white_normal.rs"]
mod white_normal;
#[path = "ui_combat_hotkey_rhs.rs"]
mod hotkey_rhs;
#[path = "canvas_tile_vm_tests/hotkey_normal.rs"]
mod hotkey_normal;
use hp2_abi::engine::generated::{FFrame, TArrayRaw, UBitmap, UCanvas, UClass, UField, UFunction, UObject, UStruct, UTexture};
use hp2_core::{
    canonical_vm_native::VmNativeSpec,
    core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeError, RuntimeResult, ScriptDebugger},
    core_native_catalog::CoreNative,
    name_owner::FindName,
    object_owner::CanonicalNativeBinding,
};
use std::{cell::{Cell, RefCell}, fs::{self, OpenOptions}, io::Write, mem::{align_of, size_of}, path::{Path, PathBuf}, ptr, rc::Rc};

thread_local! {
    static LOGS: RefCell<Vec<(Vec<u32>, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    static DEBUG_CALLS: Cell<u32> = const { Cell::new(0) };
    static FAIL_OUTPUT: Cell<bool> = const { Cell::new(false) };
}
fn output(a: CoreAccess<'_>, category: i32, message: &[u32]) -> RuntimeResult<()> {
    let name = a.name_text(category)?;
    LOGS.with(|logs| logs.borrow_mut().push((name, message.to_vec())));
    if FAIL_OUTPUT.with(Cell::get) { return Err(RuntimeError::new("fixture tile output error")); }
    Ok(())
}
fn utf32(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }

struct FinishFailure;
impl ScriptDebugger for FinishFailure {
    unsafe fn debug_info(&self, _: CoreAccess<'_>, _: *mut UObject, _: *mut FFrame, text: &[u32], line: i32, input: i32) -> RuntimeResult<()> {
        DEBUG_CALLS.with(|n| n.set(n.get() + 1));
        if !text.is_empty() || line != 7 || input != 9 {
            return Err(RuntimeError::new("fixture unexpected DebugInfo payload"));
        }
        Err(RuntimeError::new("fixture tile finish debug error"))
    }
}
struct OutputGuard;
impl Drop for OutputGuard { fn drop(&mut self) { FAIL_OUTPUT.with(|fail| fail.set(false)); } }
struct DebugGuard<'a>(CoreAccess<'a>);
impl Drop for DebugGuard<'_> { fn drop(&mut self) { self.0.set_debugger(None); } }

// Normal-path journal failure is a test failure. During error handling and Drop,
// a journal failure is secondary and must not replace the original failure.
fn journal(root: &Path, line: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().append(true).open(root.join("setup-journal.txt"))?;
    writeln!(file, "{line}")
}
// Only best-effort reporting is used during unwind; stderr write errors cannot
// cause a second panic through eprintln. All sinks can still fail.
fn stderr_secondary(message: std::fmt::Arguments<'_>) {
    let _ignored_stderr_write_failure = writeln!(std::io::stderr().lock(), "{message}");
}
fn journal_secondary(root: &Path, message: std::fmt::Arguments<'_>) {
    let result = OpenOptions::new().append(true).open(root.join("setup-journal.txt"))
        .and_then(|mut file| writeln!(file, "{message}"));
    if let Err(error) = result { stderr_secondary(format_args!("secondary journal failure: {error:?}; message: {message}")); }
}
fn exclusive_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.flush()
}

struct Attached { function: *mut UFunction, old: TArrayRaw, base: *mut u8, expected: Vec<u8> }
struct Fixture {
    owner: Option<crate::EngineOwner>,
    attached: Vec<Attached>,
    scripts: Vec<Box<[u8]>>,
    root: PathBuf,
}
impl Fixture {
    fn new(label: &str, native: Native, case: Case, strict: bool) -> Self {
        let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"));
        assert!(artifact.is_dir(), "artifact parent must already exist");
        let root = artifact.join(format!("drawtile-canonical-{label}-{}", std::process::id()));
        fs::create_dir(&root).expect("exclusively create fresh root; do not delete an existing root");
        exclusive_write(&root.join("setup-journal.txt"), b"root created exclusively\n")
            .expect("create exclusive setup journal");
        let (xl, yl) = match case { Case::ClipZeroX => (0.0_f32, 6.0_f32), Case::ClipZeroY => (8.0, 0.0), _ => (8.0, 6.0) };
        let inputs = format!("label={label} native={native:?} case={case:?} strict={strict}\nstyle=STY_NONE initial: X=-2 Y=-1 CurYL=3 clip=100,100 origin=0,0 SpaceX=0.5 SpaceY=0 Z=1\nXL={:#010x} YL={:#010x} U=1 V=2 UL=8 VL=6\ntexture=real-constructor-or-null-by-case context=self-or-null-by-case\n", xl.to_bits(), yl.to_bits());
        exclusive_write(&root.join("inputs.txt"), inputs.as_bytes()).expect("create exclusive case inputs");
        journal(&root, "inputs recorded; creating isolated System configuration").expect("journal setup milestone");
        fs::create_dir(root.join("System")).expect("create System");
        exclusive_write(&root.join("System/Default.ini"), b"").expect("isolated system ini");
        exclusive_write(&root.join("System/DefUser.ini"), b"").expect("isolated user ini");
        journal(&root, "configuration recorded; starting EngineOwner::new").expect("journal setup milestone");
        let command_line = if strict { utf32("-STRICT") } else { Vec::new() };
        let owner = match crate::EngineOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line, output, log_file: None,
        }) {
            Ok(owner) => owner,
            Err(error) => {
                stderr_secondary(format_args!("EngineOwner::new first failure: {error:?}"));
                journal_secondary(&root, format_args!("EngineOwner::new first failure: {error:?}"));
                panic!("construct real EngineOwner; stop on setup failure: {error:?}");
            }
        };
        // Install the owner in the partial-setup guard before any later checkpoint.
        let fixture = Self { owner: Some(owner), attached: Vec::new(), scripts: Vec::new(), root };
        fixture.checkpoint("EngineOwner constructed; setting process mode");
        fixture.a().set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
        fixture.checkpoint("process mode set");
        fixture
    }
    fn a(&self) -> CoreAccess<'_> { self.owner.as_ref().expect("live owner").access() }
    fn checkpoint(&self, line: &str) { journal(&self.root, line).expect("write setup milestone"); }
    fn attach(&mut self, function: *mut UFunction, code: Vec<u8>) {
        assert!(!self.attached.iter().any(|x| x.function == function));
        // SAFETY: this is a live freshly constructed Core Function in this owner.
        let old = unsafe { ptr::addr_of!((*function.cast::<UStruct>()).script).read_unaligned() };
        assert!(old.data.is_null() && old.num == 0 && old.max == 0, "new Function must have an empty script header");
        let expected = code.clone();
        let mut allocation = code.into_boxed_slice();
        let base = allocation.as_mut_ptr();
        let len = i32::try_from(allocation.len()).expect("bounded code length");
        self.scripts.push(allocation);
        self.attached.push(Attached { function, old, base, expected });
        // Both ownership records exist before the borrowed header is published.
        // SAFETY: the box and Function outlive the complete synchronous event.
        unsafe { ptr::addr_of_mut!((*function.cast::<UStruct>()).script).write_unaligned(TArrayRaw { data: base.cast(), num: len, max: len }); }
    }
    fn verify(&self) {
        for slot in &self.attached {
            // SAFETY: the Function and immutable box are live and attached.
            let h = unsafe { ptr::addr_of!((*slot.function.cast::<UStruct>()).script).read_unaligned() };
            assert_eq!(h.data.cast::<u8>(), slot.base);
            assert_eq!(h.num, slot.expected.len() as i32);
            assert_eq!(h.max, h.num);
            assert_eq!(unsafe { std::slice::from_raw_parts(slot.base, slot.expected.len()) }, slot.expected.as_slice(), "active script must remain unchanged");
        }
    }
    fn detach(&mut self) {
        // No event, debugger, callback or other observer remains active.
        for slot in &self.attached {
            // SAFETY: the owner and Function remain live until after detach.
            unsafe { ptr::addr_of_mut!((*slot.function.cast::<UStruct>()).script).write_unaligned(slot.old); }
        }
        self.attached.clear();
        self.scripts.clear();
    }
    fn finish(mut self) -> RuntimeResult<()> {
        self.verify();
        self.checkpoint("script verified; detaching before owner shutdown");
        self.detach();
        let result = self.owner.take().expect("live owner").shutdown();
        match result {
            Ok(()) => journal(&self.root, "EngineOwner shutdown succeeded")
                .map_err(|error| RuntimeError::new("fixture shutdown journal failure").with_context(&error.to_string())),
            Err(error) => {
                stderr_secondary(format_args!("EngineOwner shutdown failure: {error:?}"));
                journal_secondary(&self.root, format_args!("EngineOwner shutdown failure: {error:?}"));
                Err(error)
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.owner.is_some() {
            journal_secondary(&self.root, format_args!("partial setup or unwind: see first failure in process stderr; detaching scripts"));
        }
        self.detach();
        if let Some(owner) = self.owner.take() {
            // Do not retry or intentionally replace the primary failure. Owner
            // shutdown and other destructors are not proven panic-free.
            match owner.shutdown() {
                Ok(()) => journal_secondary(&self.root, format_args!("unwind/partial-setup EngineOwner shutdown succeeded")),
                Err(error) => {
                    stderr_secondary(format_args!("fixture teardown during unwind: {error:?}"));
                    journal_secondary(&self.root, format_args!("fixture teardown during unwind: {error:?}"));
                }
            }
        }
    }
}

// SAFETY for these routines: all inputs are canonical records returned by this
// same live owner. A failed check stops setup before use of a typed native view.
unsafe fn indexed(a: CoreAccess<'_>, object: *mut UObject) {
    assert!(!object.is_null());
    let index = unsafe { ptr::addr_of!((*object).index).read_unaligned() };
    assert!(index >= 0 && index < a.object_count());
    assert_eq!(a.object_at(index), object);
}
// Only the three known construction paths below are admitted. Every class in
// each static source-reviewed superclass chain is checked before construction.
unsafe fn class(a: CoreAccess<'_>, name: &str, minimum: usize, alignment: usize) -> *mut UClass {
    let chain: &[(&str, usize, usize, i32)] = match name {
        "Engine.Canvas" => &[("Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT, UCanvas::NATIVE_ALIGNMENT as i32), ("Core.Object", size_of::<UObject>(), align_of::<UObject>(), 4)],
        "Engine.Texture" => &[("Engine.Texture", size_of::<UTexture>(), UTexture::NATIVE_ALIGNMENT, UTexture::NATIVE_ALIGNMENT as i32), ("Engine.Bitmap", size_of::<UBitmap>(), UBitmap::NATIVE_ALIGNMENT, UBitmap::NATIVE_ALIGNMENT as i32), ("Core.Object", size_of::<UObject>(), align_of::<UObject>(), 4)],
        "Core.Function" => &[("Core.Function", size_of::<UFunction>(), align_of::<UFunction>(), 4), ("Core.Struct", size_of::<UStruct>(), align_of::<UStruct>(), 4), ("Core.Field", size_of::<UField>(), align_of::<UField>(), 4), ("Core.Object", size_of::<UObject>(), align_of::<UObject>(), 4)],
        _ => panic!("fixture does not admit construction of {name}"),
    };
    assert_eq!((minimum, alignment), (chain[0].1, chain[0].2), "exact requested native layout");
    let meta = a.native_class("Core.Class").expect("Class metaclass");
    unsafe { indexed(a, meta.cast()); }
    let bad_flags = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.preloading
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load
        | hp2_core::object_lifecycle::RF_UNREACHABLE
        | hp2_core::object_lifecycle::RF_DESTROYED
        | hp2_core::object_lifecycle::RF_DEBUG_DESTROY;
    let mut selected = ptr::null_mut();
    for (index, &(class_name, bytes, native_align, property_align)) in chain.iter().enumerate() {
        let cls = a.native_class(class_name).expect("registered native class in exact chain");
        unsafe { indexed(a, cls.cast()); }
        if index == 0 { selected = cls; }
        let record = cls.cast::<UObject>();
        assert_eq!(unsafe { ptr::addr_of!((*record).class).read_unaligned() }.cast::<UClass>(), meta, "class metaclass");
        let (package_name, short_name) = class_name.split_once('.').expect("qualified native class name");
        assert_eq!(a.name_text(i32::from_ne_bytes(unsafe { ptr::addr_of!((*record).name).read_unaligned() })).expect("class name"), utf32(short_name));
        let package = unsafe { ptr::addr_of!((*record).outer).read_unaligned().cast::<UObject>() };
        unsafe { indexed(a, package); }
        assert_eq!(a.name_text(i32::from_ne_bytes(unsafe { ptr::addr_of!((*package).name).read_unaligned() })).expect("package name"), utf32(package_name));
        assert_eq!(unsafe { ptr::addr_of!((*package).class).read_unaligned() }.cast::<UClass>(), a.native_class("Core.Package").expect("Package class"));
        assert!(unsafe { ptr::addr_of!((*package).outer).read_unaligned().is_null() });
        assert_eq!(unsafe { ptr::addr_of!((*record).object_flags).read_unaligned() } & bad_flags, 0, "class cannot need load/postload or be preloading/destroyed");
        assert!(unsafe { ptr::addr_of!((*record)._linker).read_unaligned().is_null() }, "fixture requires registered native class without linker");
        assert!(!unsafe { ptr::addr_of!((*cls).class_constructor).read_unaligned().is_null() }, "published constructor identity");
        let class_flags = unsafe { ptr::addr_of!((*cls).class_flags).read_unaligned() };
        let compiled = hp2_abi::engine::generated::CORE_CLASS_PARSED_COMPILED_MASK;
        assert_eq!(class_flags & compiled, compiled, "native class parsed and compiled");
        if index == 0 {
            assert_eq!(class_flags & (hp2_core::object_construction::CLASS_ABSTRACT | hp2_core::object_construction::CLASS_PER_OBJECT_CONFIG), 0, "no abstract class or per-object config load");
        }
        let expected_super = if index + 1 == chain.len() { ptr::null_mut() } else { a.native_class(chain[index + 1].0).expect("expected superclass") };
        assert_eq!(unsafe { ptr::addr_of!((*cls).super_field).read_unaligned() }.cast::<UClass>(), expected_super, "exact superclass chain");
        let size = unsafe { ptr::addr_of!((*cls).properties_size).read_unaligned() };
        assert_eq!(size, i32::try_from(bytes).expect("native size fits"), "exact native class size");
        let structure = cls.cast::<UStruct>();
        assert_eq!(unsafe { ptr::addr_of!((*structure).properties_align).read_unaligned() }, property_align, "registered property alignment");
        assert!(unsafe { ptr::addr_of!((*structure).constructor_link).read_unaligned().is_null() }, "no managed reflected constructor chain");
        let d = unsafe { ptr::addr_of!((*cls).defaults).read_unaligned() };
        assert!(!d.data.is_null() && d.num == size && d.max >= d.num, "complete Core-owned class default allocation");
        assert_eq!((d.data as usize) % native_align, 0, "native class default alignment");
        let default = unsafe { a.default_object(cls).expect("native class default") };
        assert_eq!(default, d.data.cast());
        assert_eq!(unsafe { ptr::addr_of!((*default).class).read_unaligned() }.cast::<UClass>(), cls);
        assert_eq!(unsafe { ptr::addr_of!((*default).index).read_unaligned() }, -1);
        assert!(unsafe { ptr::addr_of!((*default).outer).read_unaligned().is_null() });
        assert!(unsafe { ptr::addr_of!((*default).state_frame).read_unaligned().is_null() });
        assert!(unsafe { ptr::addr_of!((*default)._linker).read_unaligned().is_null() });
        assert_eq!(unsafe { ptr::addr_of!((*default).object_flags).read_unaligned() } & bad_flags, 0, "default is not in load/preload/destruction");
        if class_name == "Core.Function" || class_name == "Core.Struct" {
            let dstruct = default.cast::<UStruct>();
            let script = unsafe { ptr::addr_of!((*dstruct).script).read_unaligned() };
            assert!(script.data.is_null() && script.num == 0 && script.max == 0, "default owned script must be empty before construction");
            assert!(unsafe { ptr::addr_of!((*dstruct).script_text).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).children).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).ref_link).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).struct_link).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).property_link).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).config_link).read_unaligned().is_null() });
            assert!(unsafe { ptr::addr_of!((*dstruct).constructor_link).read_unaligned().is_null() });
        }
        if class_name == "Engine.Texture" {
            let texture = default.cast::<UTexture>();
            for array in [unsafe { ptr::addr_of!((*texture).mips).read_unaligned() }, unsafe { ptr::addr_of!((*texture).comp_mips).read_unaligned() }] {
                assert!(array.data.is_null() && array.num == 0 && array.max == 0, "default owned mip arrays must be empty before construction");
            }
        }
        if class_name == "Engine.Bitmap" || class_name == "Engine.Texture" {
            assert!(unsafe { ptr::addr_of!((*default.cast::<UBitmap>()).palette).read_unaligned().is_null() }, "no default palette reference");
        }
    }
    selected
}
unsafe fn object(a: CoreAccess<'_>, class_ptr: *mut UClass, outer: *mut UObject, name: &str, alignment: usize) -> *mut UObject {
    unsafe { indexed(a, outer); }
    let name = a.intern_str(name, FindName::Add).expect("fixture name");
    let (class_name, size) = if class_ptr == a.native_class("Engine.Canvas").expect("Canvas class") {
        ("Engine.Canvas", size_of::<UCanvas>())
    } else if class_ptr == a.native_class("Engine.Texture").expect("Texture class") {
        ("Engine.Texture", size_of::<UTexture>())
    } else if class_ptr == a.native_class("Core.Function").expect("Function class") {
        ("Core.Function", size_of::<UFunction>())
    } else { panic!("fixture does not admit this construction class"); };
    // Check again immediately before the null-template call. No preload or
    // callback-producing operation intervenes after these checks.
    assert_eq!(unsafe { class(a, class_name, size, alignment) }, class_ptr);
    let object = unsafe { a.construct_object(class_ptr, outer, name, 0x4000, ptr::null_mut()).expect("canonical constructor") };
    unsafe { indexed(a, object); }
    assert_eq!((object as usize) % alignment, 0, "native record alignment");
    assert_eq!(unsafe { ptr::addr_of!((*object).class).read_unaligned() }.cast::<UClass>(), class_ptr);
    assert_eq!(unsafe { ptr::addr_of!((*object).outer).read_unaligned() }.cast::<UObject>(), outer);
    object
}
unsafe fn body(a: CoreAccess<'_>, canvas_class: *mut UClass) -> *mut UFunction {
    let function_class = unsafe { class(a, "Core.Function", size_of::<UFunction>(), align_of::<UFunction>()) };
    let f = unsafe { object(a, function_class, canvas_class.cast(), "TileCanonicalBody", align_of::<UFunction>()) }.cast::<UFunction>();
    // No arguments, local fields, return local, constructors or native flag.
    unsafe {
        (*f).function_flags = 0; (*f).i_native = 0; (*f).num_parms = 0;
        (*f).parms_size = 0; (*f).return_value_offset = u16::MAX;
        (*f.cast::<UStruct>()).properties_size = 0;
        (*f.cast::<UStruct>()).properties_align = 1;
        assert!((*f.cast::<UStruct>()).children.is_null());
        assert!((*f.cast::<UStruct>()).constructor_link.is_null());
    }
    f
}
unsafe fn texture(a: CoreAccess<'_>) -> *mut UTexture {
    let cls = unsafe { class(a, "Engine.Texture", size_of::<UTexture>(), UTexture::NATIVE_ALIGNMENT) };
    let tex = unsafe { object(a, cls, a.transient_package(), "TileCanonicalTexture", UTexture::NATIVE_ALIGNMENT) }.cast::<UTexture>();
    // The real native constructor initializes empty mip arrays and bitmap defaults.
    // This is not a render-ready or sampleable texture and is never locked.
    for array in [unsafe { ptr::addr_of!((*tex).mips).read_unaligned() }, unsafe { ptr::addr_of!((*tex).comp_mips).read_unaligned() }] {
        assert!(array.data.is_null() && array.num == 0 && array.max == 0, "real constructor empty mip state");
    }
    assert_eq!(unsafe { ptr::addr_of!((*tex).base.u_size).read_unaligned() }, 0);
    assert_eq!(unsafe { ptr::addr_of!((*tex).base.v_size).read_unaligned() }, 0);
    assert_eq!(unsafe { ptr::addr_of!((*tex).base.format).read_unaligned() }, 0);
    tex
}

#[derive(Clone, Copy, Debug)]
enum Native { Tile, Clipped }
impl Native {
    fn index(self) -> u16 { match self { Self::Tile => 466, Self::Clipped => 468 } }
    fn name(self) -> &'static str { match self { Self::Tile => "DrawTile", Self::Clipped => "DrawTileClipped" } }
    fn low(self) -> u8 { match self { Self::Tile => 0xd2, Self::Clipped => 0xd4 } }
}
fn installed(a: CoreAccess<'_>, native: Native) -> &'static VmNativeSpec {
    let mut matches = crate::engine_vm_native::ENGINE_VM_NATIVES.iter().filter(|s| {
        s.class_name == "Engine.Canvas" && s.function_name == native.name() && s.native_index == Some(native.index())
    });
    let found = matches.next().expect("actual aggregate entry");
    assert!(matches.next().is_none(), "exact aggregate entry must be unique");
    assert!(a.effective_external_vm_native_is(native.index(), found), "actual installed descriptor wins Core and external precedence");
    found
}
fn program(native: Native, tex: Option<i32>, null_context: bool, xl: f32, yl: f32, debug: bool) -> Vec<u8> {
    let mut code = vec![0x19, if null_context { 0x2a } else { 0x17 }, 0, 0, 0];
    let inner = code.len();
    code.extend_from_slice(&[0x61, native.low()]);
    match tex { Some(index) => { code.push(0x20); code.extend_from_slice(&index.to_le_bytes()); }, None => code.push(0x2a) }
    for value in [xl, yl, 1.0_f32, 2.0, 8.0, 6.0] {
        code.push(0x1e); code.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    code.push(0x16); // seven fully typed expressions, then actual terminator
    if debug { code.push(0x38); code.extend_from_slice(&100_i32.to_le_bytes()); code.extend_from_slice(&7_i32.to_le_bytes()); code.extend_from_slice(&9_i32.to_le_bytes()); code.push(0); }
    let ret = code.len();
    code.extend_from_slice(&[0x04, 0x0b, 0x0b]); // Return Nothing; readable trailing byte
    code[2..4].copy_from_slice(&u16::try_from(ret - inner).expect("bounded Context skip").to_le_bytes());
    assert_eq!(code[4], 0); assert_eq!(&code[inner..inner+2], &[0x61, native.low()]); assert_eq!(code[ret], 4);
    code
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanvasBits { fields: [u32; 10], style: u8 }
unsafe fn set_canvas(canvas: *mut UCanvas, style: u8) {
    // SAFETY: the record is a real indexed UCanvas; only ordinary scalar state changes.
    unsafe {
        ptr::addr_of_mut!((*canvas).cur_x).write_unaligned(-2.0); ptr::addr_of_mut!((*canvas).cur_y).write_unaligned(-1.0);
        ptr::addr_of_mut!((*canvas).cur_yl).write_unaligned(3.0); ptr::addr_of_mut!((*canvas).clip_x).write_unaligned(100.0);
        ptr::addr_of_mut!((*canvas).clip_y).write_unaligned(100.0); ptr::addr_of_mut!((*canvas).org_x).write_unaligned(0.0);
        ptr::addr_of_mut!((*canvas).org_y).write_unaligned(0.0); ptr::addr_of_mut!((*canvas).space_x).write_unaligned(0.5);
        ptr::addr_of_mut!((*canvas).space_y).write_unaligned(0.0); ptr::addr_of_mut!((*canvas).z).write_unaligned(1.0);
        ptr::addr_of_mut!((*canvas).style).write_unaligned(style);
    }
}
unsafe fn snapshot(c: *mut UCanvas) -> CanvasBits {
    // SAFETY: the same live canonical Canvas is read only after event completion.
    unsafe { CanvasBits { fields: [
        ptr::addr_of!((*c).cur_x).read_unaligned().to_bits(), ptr::addr_of!((*c).cur_y).read_unaligned().to_bits(),
        ptr::addr_of!((*c).cur_yl).read_unaligned().to_bits(), ptr::addr_of!((*c).clip_x).read_unaligned().to_bits(),
        ptr::addr_of!((*c).clip_y).read_unaligned().to_bits(), ptr::addr_of!((*c).org_x).read_unaligned().to_bits(),
        ptr::addr_of!((*c).org_y).read_unaligned().to_bits(), ptr::addr_of!((*c).space_x).read_unaligned().to_bits(),
        ptr::addr_of!((*c).space_y).read_unaligned().to_bits(), ptr::addr_of!((*c).z).read_unaligned().to_bits(),
    ], style: ptr::addr_of!((*c).style).read_unaligned() } }
}
// Independent recreation of the exact object_full_name rule for the known
// indexed class/outer graph; no address or partial message matching.
unsafe fn full_name(a: CoreAccess<'_>, object: *mut UObject) -> Vec<u32> {
    unsafe { indexed(a, object); }
    let cls = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast::<UObject>() };
    unsafe { indexed(a, cls); }
    let mut result = a.name_text(i32::from_ne_bytes(unsafe { ptr::addr_of!((*cls).name).read_unaligned() })).expect("class name");
    result.push(' ' as u32);
    let mut chain = Vec::new(); let mut cursor = object;
    while !cursor.is_null() {
        assert!(chain.len() < 32, "outer chain must terminate");
        unsafe { indexed(a, cursor); }
        chain.push(a.name_text(i32::from_ne_bytes(unsafe { ptr::addr_of!((*cursor).name).read_unaligned() })).expect("object name"));
        cursor = unsafe { ptr::addr_of!((*cursor).outer).read_unaligned().cast() };
    }
    for (i, part) in chain.iter().rev().enumerate() { if i != 0 { result.push('.' as u32); } result.extend_from_slice(part); }
    result
}
fn context_error(binding: CanonicalNativeBinding, opcode: u16) -> Vec<u32> {
    utf32(&format!("Core native {binding:?} ({opcode})"))
}
#[derive(Debug)]
struct Outcome { before: CanvasBits, after: CanvasBits, diagnostic: Option<Vec<u32>>, contexts: Vec<Vec<u32>>, source_path: Option<PathBuf>, logs: Vec<(Vec<u32>, Vec<u32>)>, debugger_calls: u32, binding: String }

#[derive(Clone, Copy, Debug)]
enum Case { NullTexture, NullTextureLogError, StyleNone, ClipZeroX, ClipZeroY, NullContext, FinishError }
fn run(label: &str, native: Native, case: Case, strict: bool) {
    run_case(label, native, case, strict, false);
}

fn run_case(label: &str, native: Native, case: Case, strict: bool, selected: bool) {
    let mut fx = Fixture::new(label, native, case, strict);
    fx.checkpoint("checking installed native descriptor");
    let a = fx.a();
    let descriptor = installed(a, native);
    fx.checkpoint("descriptor identity checked; constructing Canvas");
    let binding = CanonicalNativeBinding::External(descriptor.handler);
    let canvas_class = unsafe { class(a, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT) };
    let canvas = unsafe { object(a, canvas_class, a.transient_package(), "TileCanonicalCanvas", UCanvas::NATIVE_ALIGNMENT) }.cast::<UCanvas>();
    assert!(unsafe { ptr::addr_of!((*canvas).frame).read_unaligned().is_null() });
    assert!(unsafe { ptr::addr_of!((*canvas).viewport).read_unaligned().is_null() });
    fx.checkpoint("Canvas constructed and null frame/viewport checked; constructing optional Texture");
    let tex = if matches!(case, Case::NullTexture | Case::NullTextureLogError | Case::NullContext) { None } else { Some(unsafe { texture(a) }) };
    fx.checkpoint("Texture constructor state checked if present; constructing Function");
    let function = unsafe { body(a, canvas_class) };
    fx.checkpoint("Function constructed; building bytecode");
    let texture_index = tex.map(|t| unsafe { ptr::addr_of!((*t.cast::<UObject>()).index).read_unaligned() });
    let (xl, yl) = match case { Case::ClipZeroX => (0.0, 6.0), Case::ClipZeroY => (8.0, 0.0), _ => (8.0, 6.0) };
    let debug = matches!(case, Case::FinishError);
    let code = program(native, texture_index, matches!(case, Case::NullContext), xl, yl, debug);
    // The exact indexed object operand is only known after real construction.
    // Retain bytes and layout before the immutable script header is attached.
    let inner = 5_usize;
    let ret = code.len() - 3;
    let end_parms = ret - if debug { 15 } else { 1 };
    let floats = inner + 2 + if texture_index.is_some() { 5 } else { 1 };
    let recipe = format!("length={} context=0 receiver=1 skip=2..4 width=4 native={}..{} object={} float_operands={:?} terminator={} return={} texture_index={texture_index:?} debug={debug}\n", code.len(), inner, inner + 2, inner + 2, [floats, floats + 5, floats + 10, floats + 15, floats + 20, floats + 25], end_parms, ret);
    exclusive_write(&fx.root.join("script.bin"), &code).expect("record exact immutable script bytes exclusively");
    exclusive_write(&fx.root.join("script-layout.txt"), recipe.as_bytes()).expect("record script offsets exclusively");
    fx.checkpoint("script bytes and offsets recorded; attaching header");
    fx.attach(function, code);
    fx.checkpoint("script header attached; preparing event");
    let a = fx.a();
    unsafe { set_canvas(canvas, 0); } // STY_NONE only; never enter the draw leaf.
    let before = unsafe { snapshot(canvas) };
    LOGS.with(|logs| logs.borrow_mut().clear()); DEBUG_CALLS.with(|n| n.set(0));
    let _output_guard = OutputGuard;
    FAIL_OUTPUT.with(|fail| fail.set(matches!(case, Case::NullTextureLogError)));
    let guard = DebugGuard(a);
    if debug { a.set_debugger(Some(Rc::new(FinishFailure))); }
    // SAFETY: owner holds the real Canvas and Function, zero-parameter layout,
    // complete immutable bytecode, indexed Texture, and all synchronous storage.
    fx.checkpoint("event starting; stdout/stderr retain first failure");
    let result = if selected {
        assert!(matches!(native, Native::Tile), "selected handoff is only for native 466");
        fx.verify();
        selected::execute(&fx, a, canvas, function, descriptor, texture_index, matches!(case, Case::NullContext), debug)
    } else {
        unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) }
    };
    drop(guard);
    drop(_output_guard);
    let after = unsafe { snapshot(canvas) };
    let (diagnostic, contexts, source_path) = match result {
        Ok(()) => (None, Vec::new(), None),
        Err(e) => (Some(e.diagnostic().to_vec()), e.context().to_vec(), e.source_path().map(Path::to_path_buf)),
    };
    let observed = Outcome { before, after, diagnostic, contexts, source_path, logs: LOGS.with(|x| x.borrow().clone()), debugger_calls: DEBUG_CALLS.with(Cell::get), binding: format!("{binding:?}") };
    // Print the complete first event outcome BEFORE any fallible observation I/O
    // or script verification. A later journal failure cannot hide this evidence.
    let stderr_result = writeln!(std::io::stderr().lock(), "{native:?} {case:?} strict={strict}: {observed:#?}");
    if let Err(error) = &stderr_result { journal_secondary(&fx.root, format_args!("event observation stderr failure: {error:?}")); }
    exclusive_write(&fx.root.join("observed.txt"), format!("{native:?} {case:?} strict={strict} binding={}\n{observed:#?}\n", observed.binding).as_bytes()).expect("write full case observation exclusively");
    stderr_result.expect("write full event observation to stderr");
    fx.checkpoint("full event outcome recorded; verifying immutable script");
    fx.verify();
    fx.checkpoint("immutable script verified; checking complete expected outcome");
    assert_eq!(observed.source_path, None, "synthetic event error must not carry a package source path");

    let context = context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecContext), 25);
    let stepping = utf32("stepping Core VM bytecode");
    let event = utf32(&format!("Core event {:#x}", function as usize));
    let native_context = context_error(binding, native.index());
    if matches!(case, Case::NullContext) {
        let mut warning = unsafe { full_name(a, canvas.cast()) }; warning.extend(utf32(" ("));
        warning.extend(unsafe { full_name(a, function.cast()) }); warning.extend(utf32(":0002) Accessed None: Unknown"));
        assert_eq!(observed.after, observed.before); assert_eq!(observed.debugger_calls, 0);
        if strict {
            assert_eq!(observed.diagnostic, Some(warning));
            assert_eq!(observed.contexts, vec![context, stepping, event]); assert!(observed.logs.is_empty());
        } else {
            assert_eq!(observed.diagnostic, None); assert!(observed.contexts.is_empty());
            assert_eq!(observed.logs, vec![(utf32("ScriptWarning"), warning)]);
        }
    } else if matches!(case, Case::FinishError | Case::NullTextureLogError) {
        let expected = if debug { "fixture tile finish debug error" } else { "fixture tile output error" };
        assert_eq!(observed.diagnostic, Some(utf32(expected))); assert_eq!(observed.after, observed.before);
        if debug { assert!(observed.logs.is_empty()); }
        else { assert_eq!(observed.logs, vec![(utf32("Log"), utf32(match native { Native::Tile => "DrawTile: Missing Texture", Native::Clipped => "DrawTileClipped: Missing Texture" }))]); }
        let mut contexts = Vec::new();
        if debug { contexts.push(context_error(CanonicalNativeBinding::Core(CoreNative::intUObjectexecDebugInfo), 56)); }
        contexts.extend([native_context, context, stepping, event]);
        assert_eq!(observed.contexts, contexts); assert_eq!(observed.debugger_calls, u32::from(debug));
    } else {
        assert_eq!(observed.diagnostic, None); assert!(observed.contexts.is_empty()); assert_eq!(observed.debugger_calls, 0);
        let mut expected = observed.before.clone();
        match case {
            Case::NullTexture => assert_eq!(observed.logs, vec![(utf32("Log"), utf32(match native { Native::Tile => "DrawTile: Missing Texture", Native::Clipped => "DrawTileClipped: Missing Texture" }))]),
            Case::StyleNone => {
                assert!(observed.logs.is_empty()); expected.fields[0] = 6.5_f32.to_bits();
                expected.fields[2] = match native { Native::Tile => 6.0_f32.to_bits(), Native::Clipped => 5.0_f32.to_bits() };
                if matches!(native, Native::Clipped) { expected.fields[1] = 0.0_f32.to_bits(); }
            }
            Case::ClipZeroX | Case::ClipZeroY => { assert!(matches!(native, Native::Clipped)); assert!(observed.logs.is_empty()); }
            _ => unreachable!("other cases handled above"),
        }
        assert_eq!(observed.after, expected, "exact Canvas bits");
    }
    fx.finish().expect("checked EngineOwner shutdown");
}

// Each exact test runs one case. The future parent-owned CTest driver must
// invoke them serially in separate processes and stop at the first failure.
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_null_texture() { run("466-null-texture", Native::Tile, Case::NullTexture, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_null_texture_log_error() { run("466-null-texture-log-error", Native::Tile, Case::NullTextureLogError, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_style_none() { run("466-style-none", Native::Tile, Case::StyleNone, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_finish_error() { run("466-finish-error", Native::Tile, Case::FinishError, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_null_context() { run("466-null-context", Native::Tile, Case::NullContext, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_466_null_context_strict() { run("466-null-context-strict", Native::Tile, Case::NullContext, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_null_texture() { run("468-null-texture", Native::Clipped, Case::NullTexture, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_null_texture_log_error() { run("468-null-texture-log-error", Native::Clipped, Case::NullTextureLogError, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_style_none() { run("468-style-none", Native::Clipped, Case::StyleNone, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_finish_error() { run("468-finish-error", Native::Clipped, Case::FinishError, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_null_context() { run("468-null-context", Native::Clipped, Case::NullContext, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_null_context_strict() { run("468-null-context-strict", Native::Clipped, Case::NullContext, true); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_zero_x() { run("468-zero-x", Native::Clipped, Case::ClipZeroX, false); }
#[test]
#[ignore = "requires parent-serialized isolated CTest process"]
fn canvas_canonical_468_zero_y() { run("468-zero-y", Native::Clipped, Case::ClipZeroY, false); }
