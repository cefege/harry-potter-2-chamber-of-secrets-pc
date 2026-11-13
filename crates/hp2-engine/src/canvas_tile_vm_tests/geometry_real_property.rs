//! Prototype-backed Bitmap.USize VM operand through one 466 geometry comparison.
//! The synthetic body is immutable; this does not qualify retail admission or drawing.
use super::*;
use hp2_abi::engine::generated::{EX_InstanceVariable, FObjectExport, ULinkerLoad, UProperty};
use hp2_core::sha256::{self, Sha256Context};
use std::{collections::HashSet, mem::{MaybeUninit, offset_of}};

const CORE_HASH: &str = "e0812a1931322d99d8b85db2f42fb2dacc8df33e919ea566a64e7a834e0d56e8";
const ENGINE_HASH: &str = "c75c3e849bd07a3a315d3e87f24bb5d8960b5ce32f8fc1438d2c1c3906f93182";
const INSTANCE_SITES: [usize; 2] = [0x2e4, 0x30f];

fn digest(bytes: &[u8]) -> String {
    // SAFETY: Sha256Context contains only integer fields, so zero is valid.
    let mut ctx = unsafe { MaybeUninit::<Sha256Context>::zeroed().assume_init() };
    // SAFETY: context and each bounded source chunk remain live and disjoint.
    unsafe {
        sha256::init(&mut ctx);
        for chunk in bytes.chunks(65536) {
            sha256::update(&mut ctx, chunk.as_ptr(), chunk.len() as u32);
        }
    }
    let mut out = [0u8; 32];
    // SAFETY: out has 32 writable bytes and ctx is initialized.
    unsafe { sha256::final_digest(out.as_mut_ptr(), &mut ctx) };
    out.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn copy_package(source: &Path, target: &Path, expected: &str, root: &Path) {
    let bytes = fs::read(source).unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
    let source_hash = digest(&bytes);
    journal(root, &format!("source={} bytes={} sha256={source_hash}", source.display(), bytes.len()))
        .expect("record source package identity");
    assert_eq!(source_hash, expected, "prototype package identity: {}", source.display());
    exclusive_write(target, &bytes).expect("write exclusive prototype copy");
    let copy = fs::read(target).expect("read back prototype copy");
    let copy_hash = digest(&copy);
    journal(root, &format!("copy={} bytes={} sha256={copy_hash}", target.display(), copy.len()))
        .expect("record copied package identity");
    assert_eq!(copy_hash, expected, "copied package identity: {}", target.display());
}

fn prototype_fixture() -> Fixture {
    let artifacts = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated HP2_ARTIFACT_DIR"));
    assert!(artifacts.is_dir(), "artifact parent exists");
    let source = fs::canonicalize(PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT")
        .expect("prototype HP2_CORE_DATA_ROOT"))).expect("resolve prototype root");
    let root = artifacts.join(format!("real-usize-geometry-{}", std::process::id()));
    fs::create_dir(&root).expect("create exclusive fixture root");
    exclusive_write(&root.join("setup-journal.txt"), b"profile=data-prototype\n")
        .expect("create setup journal");
    let system = root.join("System");
    fs::create_dir(&system).expect("create isolated System");
    for (name, hash) in [("Core.u", CORE_HASH), ("Engine.u", ENGINE_HASH)] {
        copy_package(&source.join("System").join(name), &system.join(name), hash, &root);
    }
    exclusive_write(&system.join("Default.ini"), b"[Core.System]\nPaths=../System/*.u\n")
        .expect("write isolated system configuration");
    exclusive_write(&system.join("DefUser.ini"), b"").expect("write isolated user configuration");
    journal(&root, "creating prototype EngineOwner").expect("journal owner setup");
    let owner = match crate::EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: system.join("Default.ini"),
        user_ini: system.join("DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }) {
        Ok(owner) => owner,
        Err(error) => {
            journal_secondary(&root, format_args!("EngineOwner::new first failure: {error:?}"));
            panic!("construct prototype EngineOwner: {error:?}");
        }
    };
    // Reuse only the existing attachment and teardown guard, not Fixture::new
    // or its empty-data class and object construction assumptions.
    let fx = Fixture { owner: Some(owner), attached: Vec::new(), scripts: Vec::new(), root };
    fx.a().set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
    fx.checkpoint("prototype EngineOwner constructed; client/scriptable mode set");
    fx
}

// Table membership checks identity, not object liveness.
// SAFETY: the caller supplies a live canonical UObject prefix from this owner;
// the owner/load/allocation contract and no-GC/no-destruction interval retain it.
unsafe fn live_index(a: CoreAccess<'_>, object: *mut UObject, label: &str) -> i32 {
    assert!(!object.is_null(), "{label} is non-null");
    let index = (0..a.object_count()).find(|&index| a.object_at(index) == object)
        .unwrap_or_else(|| panic!("{label} is not in this owner's object table"));
    // SAFETY: the caller's live-prefix contract, not the table lookup, permits this read.
    assert_eq!(unsafe { ptr::addr_of!((*object).index).read_unaligned() }, index, "{label} index round-trip");
    index
}

// An actual linked constructor chain can contain managed properties. Validate
// the owner's live records, finite links and complete in-bounds default extents;
// never assume that the chain is null or replace Core's typed copy/destruction.
unsafe fn managed_class(a: CoreAccess<'_>, root: &Path, name: &str, minimum: usize, native_align: usize, super_name: Option<&str>) -> *mut UClass {
    let class = a.native_class(name).expect("same-owner native class");
    unsafe { live_index(a, class.cast(), name); }
    let record = class.cast::<UObject>();
    assert_eq!(unsafe { ptr::addr_of!((*record).class).read_unaligned() }.cast::<UClass>(), a.native_class("Core.Class").expect("metaclass"));
    let package = unsafe { ptr::addr_of!((*record).outer).read_unaligned().cast::<UObject>() };
    unsafe { live_index(a, package, "class package"); }
    assert_eq!(unsafe { ptr::addr_of!((*package).class).read_unaligned() }.cast::<UClass>(), a.native_class("Core.Package").expect("package class"));
    assert!(unsafe { ptr::addr_of!((*package).outer).read_unaligned() }.is_null());
    assert_eq!(unsafe { a.object_path_text(record) }.expect("class path"), utf32(name), "class path");
    let bad = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.preloading
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load
        | hp2_core::object_lifecycle::RF_UNREACHABLE
        | hp2_core::object_lifecycle::RF_DESTROYED
        | hp2_core::object_lifecycle::RF_DEBUG_DESTROY;
    assert_eq!(unsafe { ptr::addr_of!((*record).object_flags).read_unaligned() } & bad, 0, "{name} is loaded");
    let expected_super = super_name.map(|parent| a.native_class(parent).expect("expected superclass")).unwrap_or(ptr::null_mut());
    assert_eq!(unsafe { ptr::addr_of!((*class).super_field).read_unaligned() }.cast::<UClass>(), expected_super, "{name} superclass");
    let flags = unsafe { ptr::addr_of!((*class).class_flags).read_unaligned() };
    let compiled = hp2_abi::engine::generated::CORE_CLASS_PARSED_COMPILED_MASK;
    assert_eq!(flags & compiled, compiled, "{name} is parsed and compiled");
    if matches!(name, "Engine.Texture" | "Engine.Canvas" | "Core.Function") {
        assert_eq!(flags & (hp2_core::object_construction::CLASS_ABSTRACT | hp2_core::object_construction::CLASS_PER_OBJECT_CONFIG), 0, "{name} is constructible without config callbacks");
    }
    assert!(!unsafe { ptr::addr_of!((*class).class_constructor).read_unaligned() }.is_null(), "{name} has constructor identity");
    let size = unsafe { ptr::addr_of!((*class).properties_size).read_unaligned() };
    let alignment = unsafe { ptr::addr_of!((*class.cast::<UStruct>()).properties_align).read_unaligned() };
    assert!(size >= i32::try_from(minimum).expect("native size fits"), "{name} includes its native layout");
    assert!(alignment > 0 && (alignment as u32).is_power_of_two() && native_align % alignment as usize == 0, "{name} reflected alignment is supported by native allocation");
    let defaults = unsafe { ptr::addr_of!((*class).defaults).read_unaligned() };
    assert!(!defaults.data.is_null() && defaults.num == size && defaults.max >= size, "{name} has complete class-owned defaults");
    assert_eq!((defaults.data as usize) % native_align, 0, "{name} defaults alignment");
    let default = unsafe { a.default_object(class).expect("validated class defaults") };
    assert_eq!(default, defaults.data.cast());
    assert_eq!(unsafe { ptr::addr_of!((*default).class).read_unaligned() }.cast::<UClass>(), class);
    assert_eq!(unsafe { ptr::addr_of!((*default).index).read_unaligned() }, -1);
    assert!(unsafe { ptr::addr_of!((*default).outer).read_unaligned() }.is_null());
    assert!(unsafe { ptr::addr_of!((*default).state_frame).read_unaligned() }.is_null());
    assert!(unsafe { ptr::addr_of!((*default)._linker).read_unaligned() }.is_null());
    assert_eq!(unsafe { ptr::addr_of!((*default).object_flags).read_unaligned() } & bad, 0);
    if name == "Core.Function" || name == "Core.Struct" {
        let structure = default.cast::<UStruct>();
        let script = unsafe { ptr::addr_of!((*structure).script).read_unaligned() };
        assert!(script.data.is_null() && script.num == 0 && script.max == 0, "{name} default script is empty");
        assert!(unsafe { ptr::addr_of!((*structure).script_text).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).children).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).ref_link).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).struct_link).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).property_link).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).config_link).read_unaligned() }.is_null());
        assert!(unsafe { ptr::addr_of!((*structure).constructor_link).read_unaligned() }.is_null());
    }

    let property_class = a.native_class("Core.Property").expect("Property class");
    let mut seen = HashSet::new();
    let mut property = unsafe { ptr::addr_of!((*class.cast::<UStruct>()).constructor_link).read_unaligned().cast::<UProperty>() };
    while !property.is_null() {
        let object = property.cast::<UObject>();
        let property_index = unsafe { live_index(a, object, "managed constructor property") };
        assert_eq!(unsafe { ptr::addr_of!((*object).object_flags).read_unaligned() } & bad, 0, "managed property is loaded and not destroyed");
        assert!(seen.insert(property as usize), "{name} constructor chain must be acyclic");
        let mut kind = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast::<UClass>() };
        let mut class_seen = HashSet::new();
        let mut found = false;
        while !kind.is_null() {
            unsafe { live_index(a, kind.cast(), "managed property class"); }
            assert!(class_seen.insert(kind as usize), "property class chain must be acyclic");
            if kind == property_class { found = true; break; }
            kind = unsafe { ptr::addr_of!((*kind).super_field).read_unaligned().cast::<UClass>() };
        }
        assert!(found, "constructor link identifies a real Property");
        let (offset, element, dim, flags) = unsafe {
            (ptr::addr_of!((*property).offset).read_unaligned(),
             ptr::addr_of!((*property).element_size).read_unaligned(),
             ptr::addr_of!((*property).array_dim).read_unaligned(),
             ptr::addr_of!((*property).property_flags).read_unaligned())
        };
        assert_ne!(flags & hp2_core::property_link::CTOR as u32, 0, "linked property requires typed construction");
        let end = element.checked_mul(dim).and_then(|extent| offset.checked_add(extent))
            .expect("managed property extent is bounded");
        assert!(offset >= size_of::<UObject>() as i32 && element > 0 && dim > 0 && end <= size, "{name} constructor property has complete default storage");
        if name == "Engine.Texture" {
            for native_start in [offset_of!(UTexture, mips), offset_of!(UTexture, comp_mips)] {
                let native_end = native_start + size_of::<TArrayRaw>();
                assert!((end as usize) <= native_start || offset as usize >= native_end,
                    "managed property must not overlap Texture's separately constructed mip arrays");
            }
        }
        journal(root, &format!("class={name} managed_property_index={property_index} offset={offset} element={element} dim={dim}"))
            .expect("record managed constructor property");
        property = unsafe { ptr::addr_of!((*property).constructor_link_next).read_unaligned().cast() };
    }
    journal(root, &format!("class={name} size={size} alignment={alignment} managed_count={} defaults_bytes={}", seen.len(), defaults.num))
        .expect("record class and managed constructor contract");
    class
}

unsafe fn construct(a: CoreAccess<'_>, class: *mut UClass, outer: *mut UObject, name: &str, alignment: usize) -> *mut UObject {
    unsafe { live_index(a, class.cast(), "construction class"); }
    unsafe { live_index(a, outer, "construction outer"); }
    let name = a.intern_str(name, FindName::Add).expect("fixture object name");
    // Exact outer/name vacancy excludes the replacement branch for every class.
    // SAFETY: class and outer are live; lookup neither allocates nor calls out.
    assert!(unsafe { a.find_object(ptr::null_mut(), outer, name, false) }.is_null(), "fixture name must be new in its outer");
    // SAFETY: the source-reviewed class/default/managed-link contracts hold;
    // the caller excludes GC, destruction and unrelated root mutation.
    let object = unsafe { a.construct_object(class, outer, name, 0x4000, ptr::null_mut()) }
        .expect("canonical constructor including managed defaults");
    unsafe { live_index(a, object, "constructed object"); }
    assert_eq!((object as usize) % alignment, 0, "native allocation alignment");
    assert_eq!(unsafe { ptr::addr_of!((*object).class).read_unaligned() }.cast::<UClass>(), class);
    assert_eq!(unsafe { ptr::addr_of!((*object).outer).read_unaligned() }.cast::<UObject>(), outer);
    object
}

// The fixture owns these NEW objects exclusively. This is not a nested root lease.
struct FixtureRoots<'owner> {
    access: CoreAccess<'owner>,
    added: [*mut UObject; 3],
    len: usize,
}
impl<'owner> FixtureRoots<'owner> {
    fn new(access: CoreAccess<'owner>) -> Self {
        Self { access, added: [ptr::null_mut(); 3], len: 0 }
    }

    // SAFETY: object is a live, fresh, fixture-exclusive object that is not
    // rooted already. No other code roots or destroys it until this guard drops.
    unsafe fn add_fresh(&mut self, object: *mut UObject) {
        assert!(!object.is_null(), "cannot root a null fixture object");
        assert!(self.len < self.added.len(), "at most three fixture roots");
        assert!(!self.added[..self.len].contains(&object), "fixture roots are distinct");
        // Record first. If add_root unwinds after a partial effect, Drop removes it.
        self.added[self.len] = object;
        self.len += 1;
        // SAFETY: the caller guarantees this fresh object stays live through Drop.
        unsafe { self.access.add_root(object); }
    }
}
impl Drop for FixtureRoots<'_> {
    fn drop(&mut self) {
        for &object in self.added[..self.len].iter().rev() {
            self.access.remove_root(object);
        }
    }
}

// Named Canvas state only: omit UObject identity, padding and unused flag bits.
// The pointer values remain typed and are compared only within this same owner.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CanvasState {
    geometry: CanvasBits,
    color: [u8; 4],
    flags: u8,
    size: [i32; 2],
    pointers: [*mut std::ffi::c_void; 8],
}
const CANVAS_NAMED_FLAGS: u8 = 0b11; // bCenter and bNoSmooth

// SAFETY: c is a complete, initialized live canonical Canvas in this owner.
unsafe fn canvas_state(c: *mut UCanvas) -> CanvasState {
    unsafe {
        CanvasState {
            geometry: snapshot(c),
            color: ptr::addr_of!((*c).color).read_unaligned(),
            flags: c.cast::<u8>().add(offset_of!(UCanvas, color) + 4).read() & CANVAS_NAMED_FLAGS,
            size: [ptr::addr_of!((*c).x).read_unaligned(), ptr::addr_of!((*c).y).read_unaligned()],
            pointers: [
                ptr::addr_of!((*c).font).read_unaligned(), ptr::addr_of!((*c).small_font).read_unaligned(),
                ptr::addr_of!((*c).med_font).read_unaligned(), ptr::addr_of!((*c).big_font).read_unaligned(),
                ptr::addr_of!((*c).large_font).read_unaligned(), ptr::addr_of!((*c).viewport).read_unaligned(),
                ptr::addr_of!((*c).frame).read_unaligned(), ptr::addr_of!((*c).render).read_unaligned(),
            ],
        }
    }
}

// SAFETY: c and s are live. s's pointer fields came from this same Canvas;
// their targets retain the original owner/lifetime, and are not dereferenced.
unsafe fn restore_canvas(c: *mut UCanvas, s: &CanvasState) {
    let f = &s.geometry.fields;
    unsafe {
        ptr::addr_of_mut!((*c).cur_x).write_unaligned(f32::from_bits(f[0]));
        ptr::addr_of_mut!((*c).cur_y).write_unaligned(f32::from_bits(f[1]));
        ptr::addr_of_mut!((*c).cur_yl).write_unaligned(f32::from_bits(f[2]));
        ptr::addr_of_mut!((*c).clip_x).write_unaligned(f32::from_bits(f[3]));
        ptr::addr_of_mut!((*c).clip_y).write_unaligned(f32::from_bits(f[4]));
        ptr::addr_of_mut!((*c).org_x).write_unaligned(f32::from_bits(f[5]));
        ptr::addr_of_mut!((*c).org_y).write_unaligned(f32::from_bits(f[6]));
        ptr::addr_of_mut!((*c).space_x).write_unaligned(f32::from_bits(f[7]));
        ptr::addr_of_mut!((*c).space_y).write_unaligned(f32::from_bits(f[8]));
        ptr::addr_of_mut!((*c).z).write_unaligned(f32::from_bits(f[9]));
        ptr::addr_of_mut!((*c).style).write_unaligned(s.geometry.style);
        ptr::addr_of_mut!((*c).color).write_unaligned(s.color);
        let flags = c.cast::<u8>().add(offset_of!(UCanvas, color) + 4);
        flags.write((flags.read() & !CANVAS_NAMED_FLAGS) | (s.flags & CANVAS_NAMED_FLAGS));
        ptr::addr_of_mut!((*c).x).write_unaligned(s.size[0]);
        ptr::addr_of_mut!((*c).y).write_unaligned(s.size[1]);
        ptr::addr_of_mut!((*c).font).write_unaligned(s.pointers[0]);
        ptr::addr_of_mut!((*c).small_font).write_unaligned(s.pointers[1]);
        ptr::addr_of_mut!((*c).med_font).write_unaligned(s.pointers[2]);
        ptr::addr_of_mut!((*c).big_font).write_unaligned(s.pointers[3]);
        ptr::addr_of_mut!((*c).large_font).write_unaligned(s.pointers[4]);
        ptr::addr_of_mut!((*c).viewport).write_unaligned(s.pointers[5]);
        ptr::addr_of_mut!((*c).frame).write_unaligned(s.pointers[6]);
        ptr::addr_of_mut!((*c).render).write_unaligned(s.pointers[7]);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FullOutcome {
    before: CanvasState,
    after: CanvasState,
    logs: Vec<(Vec<u32>, Vec<u32>)>,
    debug_calls: u32,
    error: Option<(Vec<u32>, Vec<Vec<u32>>, Option<PathBuf>)>,
}

fn program_with_property(canvas: i32, texture: i32, property: i32) -> Vec<u8> {
    fn instance_context(code: &mut Vec<u8>, texture: i32, property: i32) {
        code.push(0x19); // EX_Context
        object_constant(code, texture);
        code.extend_from_slice(&[5, 0, 4]); // skipped expression width and result width
        code.push(u8::try_from(EX_InstanceVariable).expect("InstanceVariable opcode fits"));
        code.extend_from_slice(&property.to_le_bytes()); // live owner-local UObject index
    }
    let mut code = vec![0x0b; CONTEXT];
    code.push(0x19);
    object_constant(&mut code, canvas);
    let skip = code.len();
    code.extend_from_slice(&[0, 0, 0]);
    assert_eq!(code.len(), NATIVE);
    code.extend_from_slice(&[0x61, 0xd2]); // installed native 466
    object_constant(&mut code, texture);
    assert_eq!(code.len(), FLOAT_STARTS[0]);
    code.extend_from_slice(&[0xab, 0x40]); // multiply, int-to-float
    instance_context(&mut code, texture, property);
    float_constant(&mut code, 1.5);
    code.push(0x16);
    assert_eq!(code.len(), FLOAT_STARTS[1]);
    code.push(0xab);
    float_constant(&mut code, 4.0);
    float_constant(&mut code, 1.5);
    code.push(0x16);
    assert_eq!(code.len(), FLOAT_STARTS[2]);
    float_constant(&mut code, 1.0);
    float_constant(&mut code, 2.0);
    assert_eq!(code.len(), FLOAT_STARTS[4]);
    code.push(0x40);
    instance_context(&mut code, texture, property);
    assert_eq!(code.len(), FLOAT_STARTS[5]);
    float_constant(&mut code, 7.0);
    assert_eq!(code.len(), FLOAT_ENDS[5]);
    code.push(0x16);
    let ret = code.len();
    code.extend_from_slice(&[0x04, 0x0b, 0x0b]);
    code[skip..skip + 2].copy_from_slice(&u16::try_from(ret - NATIVE).expect("bounded Context skip").to_le_bytes());
    for at in INSTANCE_SITES {
        assert_eq!(code[at], u8::try_from(EX_InstanceVariable).expect("opcode"));
        assert_eq!(&code[at + 1..at + 5], &property.to_le_bytes(), "owner-local property operand");
    }
    code
}

#[test]
#[ignore = "requires parent-serialized prototype CTest and isolated artifacts"]
fn canvas_prototype_usize_geometry() {
    let mut fx = prototype_fixture();
    let a = fx.a();
    let core = unsafe { a.get_package_linker(&fx.root.join("System/Core.u"), 1, ptr::null_mut(), None) }
        .expect("acquire Core metadata without full export sweep");
    unsafe { live_index(a, core, "Core linker"); }
    let core_root = unsafe { ptr::addr_of!((*core.cast::<ULinkerLoad>()).base.linker_root).read_unaligned() };
    assert_eq!(core_root, a.core_package().cast(), "Core linker root");
    let linker = unsafe { a.load_package(&fx.root.join("System/Engine.u"), 1) }
        .expect("load prototype Engine metadata");
    unsafe { live_index(a, linker, "Engine linker"); }
    let object_class = a.native_class("Core.Object").expect("Object class");
    unsafe { live_index(a, object_class.cast(), "Object class"); }
    let bitmap = unsafe { a.static_load_class(object_class, ptr::null_mut(), &utf32("Engine.Bitmap"), 1) }
        .expect("load Bitmap class");
    let texture_class = unsafe { a.static_load_class(object_class, ptr::null_mut(), &utf32("Engine.Texture"), 1) }
        .expect("load Texture class and defaults");
    assert_eq!(bitmap, a.native_class("Engine.Bitmap").expect("same Bitmap"));
    assert_eq!(texture_class, a.native_class("Engine.Texture").expect("same Texture"));
    unsafe { live_index(a, bitmap.cast(), "Bitmap class"); }
    unsafe { live_index(a, texture_class.cast(), "Texture class"); }
    assert_eq!(unsafe { ptr::addr_of!((*bitmap).super_field).read_unaligned() }.cast::<UClass>(), object_class, "Bitmap directly inherits Object");
    assert_eq!(unsafe { ptr::addr_of!((*bitmap.cast::<UObject>())._linker).read_unaligned() }, linker.cast());
    assert_eq!(unsafe { ptr::addr_of!((*texture_class.cast::<UObject>())._linker).read_unaligned() }, linker.cast());
    assert_eq!(unsafe { ptr::addr_of!((*texture_class.cast::<UObject>()).outer).read_unaligned() }, unsafe { ptr::addr_of!((*bitmap.cast::<UObject>()).outer).read_unaligned() }, "Bitmap and Texture share the loaded Engine package");
    let property = unsafe { a.find_object_by_name(ptr::null_mut(), &utf32("Engine.Bitmap.USize")) }
        .expect("lookup USize").expect("package-backed USize exists");
    let property_index = unsafe { live_index(a, property, "USize descriptor") };
    let int_class = a.native_class("Core.IntProperty").expect("IntProperty class");
    unsafe { live_index(a, int_class.cast(), "IntProperty class"); }
    let bad = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.preloading
        | hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_post_load
        | hp2_core::object_lifecycle::RF_UNREACHABLE
        | hp2_core::object_lifecycle::RF_DESTROYED
        | hp2_core::object_lifecycle::RF_DEBUG_DESTROY;
    for (label, object) in [("Bitmap class", bitmap.cast::<UObject>()), ("USize descriptor", property)] {
        assert_eq!(unsafe { ptr::addr_of!((*object).object_flags).read_unaligned() } & bad, 0, "{label} is loaded and not destroyed");
    }
    assert_eq!(unsafe { ptr::addr_of!((*property).class).read_unaligned() }, int_class.cast());
    assert_eq!(unsafe { ptr::addr_of!((*property).outer).read_unaligned() }, bitmap.cast());
    assert_eq!(unsafe { ptr::addr_of!((*property)._linker).read_unaligned() }, linker.cast());
    let export_row = unsafe { ptr::addr_of!((*property)._linker_index).read_unaligned() };
    let exports = unsafe { ptr::addr_of!((*linker.cast::<ULinkerLoad>()).base.export_map).read_unaligned() };
    assert!(exports.num > 0 && exports.max >= exports.num && !exports.data.is_null());
    assert!(export_row >= 0 && export_row < exports.num, "descriptor export row is in range");
    let export = unsafe { ptr::read_unaligned(exports.data.cast::<FObjectExport>().add(export_row as usize)) };
    assert_eq!(export._object, property.cast(), "Engine export row points to this descriptor");
    let name = a.intern_str("USize", FindName::Find).expect("USize name");
    assert_ne!(name, 0);
    assert_eq!(i32::from_ne_bytes(export.object_name), name, "Engine export names USize");
    assert_eq!(unsafe { a.find_field(bitmap.cast::<UStruct>(), name, int_class) }.cast::<UObject>(), property);
    let p = property.cast::<UProperty>();
    let (dim, element, offset) = unsafe { (ptr::addr_of!((*p).array_dim).read_unaligned(), ptr::addr_of!((*p).element_size).read_unaligned(), ptr::addr_of!((*p).offset).read_unaligned()) };
    assert_eq!((dim, element, offset), (1, 4, 88), "real descriptor ABI");
    assert_eq!(offset_of!(UBitmap, u_size), 88);
    assert_eq!(unsafe { ptr::addr_of!((*bitmap).properties_size).read_unaligned() }, size_of::<UBitmap>() as i32);

    // Check loaded classes and their actual managed chains, not the no-data
    // fixture's null-link assumptions. Core's construct_object selects the
    // registered Texture class operations after copying managed defaults.
    assert_eq!(unsafe { managed_class(a, &fx.root, "Core.Object", size_of::<UObject>(), align_of::<UObject>(), None) }, object_class);
    unsafe { managed_class(a, &fx.root, "Core.Field", size_of::<UField>(), align_of::<UField>(), Some("Core.Object")); }
    unsafe { managed_class(a, &fx.root, "Core.Struct", size_of::<UStruct>(), align_of::<UStruct>(), Some("Core.Field")); }
    assert_eq!(unsafe { managed_class(a, &fx.root, "Engine.Bitmap", size_of::<UBitmap>(), UBitmap::NATIVE_ALIGNMENT, Some("Core.Object")) }, bitmap);
    let checked_texture = unsafe { managed_class(a, &fx.root, "Engine.Texture", size_of::<UTexture>(), UTexture::NATIVE_ALIGNMENT, Some("Engine.Bitmap")) };
    assert_eq!(checked_texture, texture_class);
    let texture_default = unsafe { a.default_object(texture_class).expect("Texture defaults") }.cast::<UTexture>();
    for array in [unsafe { ptr::addr_of!((*texture_default).mips).read_unaligned() }, unsafe { ptr::addr_of!((*texture_default).comp_mips).read_unaligned() }] {
        assert!(array.data.is_null() && array.num == 0 && array.max == 0, "native Texture mip defaults must be empty before its constructor resets them");
    }
    let canvas_class = unsafe { managed_class(a, &fx.root, "Engine.Canvas", size_of::<UCanvas>(), UCanvas::NATIVE_ALIGNMENT, Some("Core.Object")) };
    let function_class = unsafe { managed_class(a, &fx.root, "Core.Function", size_of::<UFunction>(), align_of::<UFunction>(), Some("Core.Struct")) };
    let canvas = unsafe { construct(a, canvas_class, a.transient_package(), "RealGeometryCanvas", UCanvas::NATIVE_ALIGNMENT) }.cast::<UCanvas>();
    let texture = unsafe { construct(a, texture_class, a.transient_package(), "RealGeometryTexture", UTexture::NATIVE_ALIGNMENT) }.cast::<UTexture>();
    let function = unsafe { construct(a, function_class, canvas_class.cast(), "RealGeometryBody", align_of::<UFunction>()) }.cast::<UFunction>();
    let canvas_index = unsafe { live_index(a, canvas.cast(), "Canvas") };
    let texture_index = unsafe { live_index(a, texture.cast(), "Texture") };
    unsafe { live_index(a, function.cast(), "Function"); }
    assert!(unsafe { ptr::addr_of!((*canvas).frame).read_unaligned() }.is_null());
    assert!(unsafe { ptr::addr_of!((*canvas).viewport).read_unaligned() }.is_null());
    for array in [unsafe { ptr::addr_of!((*texture).mips).read_unaligned() }, unsafe { ptr::addr_of!((*texture).comp_mips).read_unaligned() }] {
        assert!(array.data.is_null() && array.num == 0 && array.max == 0, "genuine Texture constructor empty mip state");
    }
    assert_eq!(unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() }, 0, "genuine Bitmap constructor initialized USize");
    assert_eq!(unsafe { ptr::addr_of!((*texture).diffuse).read_unaligned() }.to_bits(), 1.0_f32.to_bits(), "genuine Texture constructor initialized diffuse");
    // SAFETY: the actual Texture class, allocation, inherited Bitmap layout,
    // descriptor offset and constructor were checked above. This is fixture
    // input before either event, not an emulated VM property operation.
    unsafe { ptr::addr_of_mut!((*texture).base.u_size).write_unaligned(13); }
    assert_eq!(unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() }, 13);
    unsafe {
        assert!((*function.cast::<UStruct>()).children.is_null(), "synthetic body has no fields");
        assert!((*function.cast::<UStruct>()).constructor_link.is_null(), "synthetic body has no locals");
        (*function).function_flags = 0; (*function).i_native = 0; (*function).num_parms = 0;
        (*function).parms_size = 0; (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_size = 0;
        (*function.cast::<UStruct>()).properties_align = 1;
    }
    let descriptor = installed(a, Native::Tile);
    journal(&fx.root, "native=Engine.Canvas.DrawTile opcode=466 effective_installed_entry=ok")
        .expect("record installed 466 entry");
    let code = program_with_property(canvas_index, texture_index, property_index);
    exclusive_write(&fx.root.join("geometry-script.bin"), &code).expect("record original immutable body");
    journal(&fx.root, &format!("canvas_index={canvas_index} texture_index={texture_index} property_index={property_index} export_row={export_row} dim={dim} element={element} offset={offset} texture_usize=13"))
        .expect("record same-owner operands and metadata");
    let original = code.clone();
    fx.attach(function, code);
    fx.verify();
    let a = fx.a();
    let mut fixture_roots = FixtureRoots::new(a);
    // SAFETY: name-vacancy checks and the source-reviewed constructor path give
    // these three fresh, distinct, unrooted records to this fixture exclusively.
    // No GC, destruction, or unrelated root mutation may occur until Drop.
    unsafe {
        fixture_roots.add_fresh(canvas.cast());
        fixture_roots.add_fresh(texture.cast());
        fixture_roots.add_fresh(function.cast());
    }
    journal(&fx.root, "exclusive Canvas/Texture/Function roots installed; metadata not rooted by fixture")
        .expect("record fixture root setup");

    // The loaded descriptor is retained by the owner/load contract without GC
    // or destruction; its table index is identity, not a lifetime guarantee.
    // Same-owner events are sequential; restore every named Canvas field before
    // each event, not an independently reconstructed owner or runtime state.
    unsafe { set_canvas(canvas, 0); }
    let mut input = unsafe { canvas_state(canvas) };
    input.color = [11, 22, 33, 44];
    input.flags = CANVAS_NAMED_FLAGS;
    input.size = [640, 480];
    let mut outcomes = Vec::new();
    for selected in [false, true] {
        assert_eq!(unsafe { live_index(a, property, "loaded USize descriptor") }, property_index);
        assert_eq!(unsafe { live_index(a, texture.cast(), "fixture Texture") }, texture_index);
        assert_eq!(unsafe { ptr::addr_of!((*property).class).read_unaligned() }, int_class.cast());
        assert_eq!(unsafe { ptr::addr_of!((*property).outer).read_unaligned() }, bitmap.cast());
        assert_eq!(unsafe { (ptr::addr_of!((*p).array_dim).read_unaligned(), ptr::addr_of!((*p).element_size).read_unaligned(), ptr::addr_of!((*p).offset).read_unaligned()) }, (1, 4, 88));
        unsafe { restore_canvas(canvas, &input); }
        let before = unsafe { canvas_state(canvas) };
        assert_eq!(before, input, "restore all named Canvas input before each event");
        LOGS.with(|logs| logs.borrow_mut().clear());
        DEBUG_CALLS.with(|count| count.set(0));
        FAIL_OUTPUT.with(|fail| fail.set(false));
        fx.verify();
        let (result, floats, handoffs) = if selected {
            let slot = &fx.attached[0];
            let selection = Selection { function, canvas, base: slot.base.cast_const(), len: slot.expected.len(), descriptor };
            SELECTION.with(|state| { assert!(state.get().is_none()); state.set(Some(selection)); });
            let selection_guard = SelectionGuard;
            FLOATS.with(|values| values.borrow_mut().clear());
            HANDOFFS.with(|count| count.set(0));
            // SAFETY: the one owner, explicit roots, immutable admitted body,
            // complete operand layouts and current lexical frame remain live.
            let binding = unsafe { a.install_script_body_replacement("real-usize-geometry", &[function], replacement) }
                .expect("install selected replacement");
            let result = unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) };
            drop(binding);
            let floats = FLOATS.with(|values| values.borrow_mut().drain(..).collect::<Vec<_>>());
            let handoffs = HANDOFFS.with(Cell::get);
            drop(selection_guard);
            (result, floats, handoffs)
        } else {
            assert!(SELECTION.with(|state| state.get().is_none()));
            (unsafe { a.execute_event(canvas.cast(), function, ptr::null_mut(), ptr::null_mut()) }, Vec::new(), 0)
        };
        let after = unsafe { canvas_state(canvas) };
        let logs = LOGS.with(|logs| logs.borrow().clone());
        let error = result.err().map(|e| (e.diagnostic().to_vec(), e.context().to_vec(), e.source_path().map(Path::to_path_buf)));
        let observed = FullOutcome { before, after, logs, debug_calls: DEBUG_CALLS.with(Cell::get), error };
        let mode = if selected { "selected" } else { "canonical" };
        exclusive_write(&fx.root.join(format!("{mode}-observed.txt")), format!("handoffs={handoffs} floats={floats:?} outcome={observed:?}\n").as_bytes())
            .expect("retain complete event observation");
        fx.verify();
        assert_eq!(fx.attached[0].expected, original, "original body remains unchanged");
        assert_eq!(unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() }, 13, "events do not change Texture.USize");
        assert!(observed.error.is_none() && observed.logs.is_empty() && observed.debug_calls == 0, "normal event has no errors or warnings");
        let mut expected = observed.before.clone();
        expected.geometry.fields[0] = 18.0_f32.to_bits();
        expected.geometry.fields[2] = 6.0_f32.to_bits();
        assert_eq!(observed.after, expected, "all named Canvas state: only CurX and CurYL change");
        if selected {
            assert_eq!(handoffs, 1, "one selected Core handoff");
            let bits = [19.5_f32, 6.0, 1.0, 2.0, 13.0, 7.0].map(f32::to_bits);
            let expected: Vec<_> = (0..6).map(|i| FloatObservation { start: FLOAT_STARTS[i], end: FLOAT_ENDS[i], bits: bits[i] }).collect();
            assert_eq!(floats, expected, "six exact float roots, ends and values, including three geometry roots");
        }
        outcomes.push(observed);
    }
    assert_eq!(outcomes[0], outcomes[1], "canonical and selected outcomes on the same owner");
    // Release only this guard's fresh-object roots while all records are live,
    // before detach and owner shutdown. Unwind drops the guard before fx too.
    drop(fixture_roots);
    fx.finish().expect("checked EngineOwner shutdown after script detach");
}
