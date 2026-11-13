//! Metadata-only prerequisite for the separate script/GC investigation.
//!
//! This does not run an event, explicitly collect garbage, or exercise a script
//! body. Owner shutdown can itself perform collection or other lifecycle work.
use hp2_abi::engine::generated::{CORE_LINKER_CONSTANTS, UClass, UFunction, UObject, UStruct, UTextBuffer};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeResult};
use hp2_core::core_class_specs::NATIVE_CLASSES;
use hp2_core::name_owner::FindName;
use hp2_core::sha256::{self, Sha256Context};
use std::{collections::HashMap, fs, mem::MaybeUninit, path::PathBuf, ptr};

#[path = "support/gc_script_metadata.rs"]
mod gc_script_metadata;

const CORE_SHA256: &str = "e0812a1931322d99d8b85db2f42fb2dacc8df33e919ea566a64e7a834e0d56e8";
const CORE_BYTES: usize = 61685;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

fn digest(bytes: &[u8]) -> String {
    // SAFETY: the context contains only integer fields and is initialized by
    // init before use. The input length is bounded by the fixture's exact size.
    unsafe {
        let mut context = MaybeUninit::<Sha256Context>::zeroed().assume_init();
        let mut out = [0u8; 32];
        sha256::init(&raw mut context);
        sha256::update(&raw mut context, bytes.as_ptr(), u32::try_from(bytes.len()).unwrap());
        sha256::final_digest(out.as_mut_ptr(), &raw mut context);
        out.iter().map(|v| format!("{v:02x}")).collect()
    }
}

// Establish membership before reading even the UObject header. The engine is
// single-thread confined here; no callbacks or loading occur during the walk.
fn indexed(access: CoreAccess<'_>, table: &HashMap<usize, i32>, object: *mut UObject) -> i32 {
    assert!(!object.is_null(), "null metadata record");
    let index = *table.get(&(object as usize)).expect("metadata pointer must be indexed");
    assert_eq!(access.object_at(index), object, "object table must remain stable");
    // SAFETY: membership establishes a live canonical UObject allocation.
    assert_eq!(unsafe { ptr::addr_of!((*object).index).read_unaligned() }, index);
    index
}

#[test]
fn reference_binary_metadata_preflight_without_event_or_explicit_collection() {
    let source = std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal"));
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"));
    assert!(artifact.is_dir());
    let root = artifact.join(format!("gc-script-metadata-preflight-{}", std::process::id()));
    assert!(!root.exists(), "use a fresh isolated fixture directory");
    let core = fs::read(source.join("System/Core.u")).expect("read configured prototype Core.u");
    assert_eq!(core.len(), CORE_BYTES);
    let hash = digest(&core);
    assert_eq!(hash, CORE_SHA256, "prototype Core.u identity before staging");
    println!("INPUT {} bytes={} sha256={hash}", source.join("System/Core.u").display(), core.len());
    fs::create_dir_all(root.join("System")).unwrap();
    // Stage the exact verified bytes, not a second read subject to replacement.
    fs::write(root.join("System/Core.u"), &core).unwrap();
    for (from, to) in [("Default.ini", "System.ini"), ("DefUser.ini", "User.ini")] {
        let input = source.join("System").join(from);
        println!("INI {}", input.display());
        fs::copy(input, root.join("System").join(to)).expect("stage runtime17 prototype INI");
    }
    let owner = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/System.ini"),
        user_ini: root.join("System/User.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    {
        let access = owner.access();
        access.set_process_mode(CoreProcessMode { is_client: true, is_scriptable: true, ..CoreProcessMode::default() });
        let commandlet: Vec<u32> = "Core.SimpleCommandlet".chars().map(u32::from).chain([0]).collect();
        // SAFETY: this follows the runtime17 fixture's actual class-load setup.
        unsafe { access.static_load_class(access.native_class("Commandlet").unwrap(), ptr::null_mut(), &commandlet, 1).unwrap(); }
        let function_class = access.native_class("Function").unwrap();
        let property_class = access.native_class("IntProperty").unwrap();
        let text_class = access.native_class("TextBuffer").unwrap();
        let object_class = access.native_class("Object").unwrap();
        // Resolve all exact native identities before entering the read-only scan;
        // native_class may populate a Core cache and must not run in the helper.
        let mut native = Vec::new();
        for spec in NATIVE_CLASSES {
            native.push(gc_script_metadata::NativeClass {
                address: access.native_class(spec.short_name).unwrap(), name: spec.short_name, kind: spec.kind,
            });
        }
        let meta_class = access.native_class("Class").unwrap();
        let mut table = HashMap::new();
        for index in 0..access.object_count() {
            let object = access.object_at(index);
            if !object.is_null() { assert!(table.insert(object as usize, index).is_none()); }
        }
        for class in [function_class, property_class, text_class, object_class] {
            let object = class.cast::<UObject>();
            indexed(access, &table, object);
            // SAFETY: indexed class record; its class must be the exact native metaclass.
            unsafe {
                assert_eq!(ptr::addr_of!((*object).class).read_unaligned(), meta_class.cast());
                assert_eq!(ptr::addr_of!((*object).object_flags).read_unaligned() & (CORE_LINKER_CONSTANTS.need_load | CORE_LINKER_CONSTANTS.preloading), 0, "class needs load or is preloading");
            }
        }
        let mut serializable = Vec::new();
        for (class, expected) in [
            (function_class, &["Function", "Struct", "Field", "Object"][..]),
            (property_class, &["IntProperty", "Property", "Field", "Object"][..]),
            (text_class, &["TextBuffer", "Object"][..]),
        ] {
            // SAFETY: exact native identities were resolved for this owner. No
            // callbacks, loading, GC, or mutation can run during this scan.
            let report = unsafe { gc_script_metadata::scan(access, class, &native) }.expect("metadata scan");
            assert_eq!(report.classes.iter().map(String::as_str).collect::<Vec<_>>().as_slice(), expected, "complete class-super chain");
            for line in report.lines { println!("{line}"); }
            serializable.extend(report.serializable);
        }
        assert!(serializable.is_empty(), "Reference BinaryProperties would serialize: {serializable:?}");

        // The class records were indexed, type-checked and scanned above. These
        // checks happen before any of this fixture's 3 object constructions.
        for class in [function_class, property_class, text_class] {
            // SAFETY: exact canonical Class records provide the UStruct prefix.
            let link = unsafe { ptr::addr_of!((*class.cast::<UStruct>()).constructor_link).read_unaligned() };
            assert!(link.is_null(), "target class has a constructor link: {class:p}");
        }
        for (class, minimum, label) in [
            (function_class, std::mem::size_of::<UFunction>(), "Function"),
            (text_class, std::mem::size_of::<UTextBuffer>(), "TextBuffer"),
        ] {
            // SAFETY: the scanned target is a canonical live class record.
            let size = unsafe { ptr::addr_of!((*class).properties_size).read_unaligned() };
            assert!(usize::try_from(size).is_ok_and(|size| size >= minimum), "{label} defaults are too small: {size}");
        }
        // SAFETY: canonical bootstrap supplies initialized class-owned defaults;
        // the getter checks storage and size, and the size checks above cover the
        // complete declared native prefixes. No default header is changed.
        let function_default = unsafe { access.default_object(function_class).expect("Function defaults") };
        let text_default = unsafe { access.default_object(text_class).expect("TextBuffer defaults") };
        let default_script = unsafe { ptr::addr_of!((*function_default.cast::<UStruct>()).script).read_unaligned() };
        let default_text = unsafe { ptr::addr_of!((*text_default.cast::<UTextBuffer>()).text).read_unaligned() };
        assert!(default_script.data.is_null() && default_script.num == 0 && default_script.max == 0, "Function default script header must be empty");
        assert!(default_text.data.is_null() && default_text.num == 0 && default_text.max == 0, "TextBuffer default text header must be empty");

        let make = |class: *mut UClass, outer: *mut UObject, name: &str| {
            let name = access.intern_str(name, FindName::Add).unwrap();
            // SAFETY: class/outer are canonical and live; Core owns the result.
            unsafe { access.construct_object(class, outer, name, 0x4000, ptr::null_mut()).unwrap() }
        };
        let function = make(function_class, text_class.cast(), "MetadataPreflightFunction");
        let property = make(property_class, function, "MetadataPreflightProperty");
        let buffer = make(text_class, access.transient_package(), "MetadataPreflightTextBuffer");
        // Construction changes the table; take a fresh snapshot for the records.
        let mut table = HashMap::new();
        for index in 0..access.object_count() {
            let object = access.object_at(index);
            if !object.is_null() { assert!(table.insert(object as usize, index).is_none()); }
        }
        for (object, class) in [(function, function_class), (property, property_class), (buffer, text_class)] {
            indexed(access, &table, object);
            // SAFETY: the object was just constructed by its exact native class.
            unsafe {
                assert_eq!(ptr::addr_of!((*object).class).read_unaligned(), class.cast());
                assert_eq!(ptr::addr_of!((*object).object_flags).read_unaligned() & (CORE_LINKER_CONSTANTS.need_load | CORE_LINKER_CONSTANTS.preloading), 0, "record needs load or is preloading");
                assert!(ptr::addr_of!((*object)._linker).read_unaligned().is_null());
                assert_eq!(ptr::addr_of!((*object)._linker_index).read_unaligned(), -1);
                assert!(ptr::addr_of!((*object).state_frame).read_unaligned().is_null(), "fixture record must not have a state frame");
            }
            println!("SYNTHETIC index={} class={class:p}", indexed(access, &table, object));
        }
        // SAFETY: exact native constructions provide the complete prefixes.
        let script = unsafe { ptr::addr_of!((*function.cast::<UStruct>()).script).read_unaligned() };
        let text = unsafe { ptr::addr_of!((*buffer.cast::<UTextBuffer>()).text).read_unaligned() };
        assert!(script.data.is_null() && script.num == 0 && script.max == 0, "fixture must not attach a script");
        assert!(text.data.is_null() && text.num == 0 && text.max == 0, "fixture TextBuffer text must be empty");
    }
    // This may perform collection and other owner teardown; it is outside any event.
    owner.shutdown().expect("Core owner shutdown");
}
