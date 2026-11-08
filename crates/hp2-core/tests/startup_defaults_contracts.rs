use hp2_abi::engine::generated::UObject;
use hp2_core::{
    core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupDefault, StartupProperty, StartupPropertyKind},
};
use std::{alloc::Layout, mem::offset_of, path::PathBuf, ptr};

#[repr(C)]
struct StartupRecord {
    object: UObject,
    flags: u32,
    gamma: f32,
    width: i32,
    mode: u8,
    neighbor: u8,
    tail: u8,
}
const fn property(name: &'static str, kind: StartupPropertyKind, offset: usize) -> StartupProperty {
    StartupProperty { name, kind, category: "", property_flags: 0, object_flags: 4,
        inner_name: None, offset, array_dim: 1 }
}
static CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "DefaultsBase", package: "StartupProbe", super_name: Some("Object"),
        within: "Object", flags: 0, object_flags: 4, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<StartupRecord>()),
        operations: None,
        properties: &[
            property("Keep", StartupPropertyKind::Bool { bit_mask: 0x400 }, offset_of!(StartupRecord, flags)),
            property("Clear", StartupPropertyKind::Bool { bit_mask: 8 }, offset_of!(StartupRecord, flags)),
            property("Enable", StartupPropertyKind::Bool { bit_mask: 0x80 }, offset_of!(StartupRecord, flags)),
            property("Gamma", StartupPropertyKind::Float, offset_of!(StartupRecord, gamma)),
            property("Width", StartupPropertyKind::Int, offset_of!(StartupRecord, width)),
            property("Mode", StartupPropertyKind::ByteEnum { name: "Modes", values: &["Zero", "One", "Two"] }, offset_of!(StartupRecord, mode)),
            property("Neighbor", StartupPropertyKind::Byte, offset_of!(StartupRecord, neighbor)),
            property("Tail", StartupPropertyKind::Byte, offset_of!(StartupRecord, tail)),
        ],
        defaults: &[
            StartupDefault::SetBool { field: "Keep", value: true },
            StartupDefault::SetBool { field: "Clear", value: true },
            StartupDefault::SetFloat { field: "Gamma", value: 0.5 },
            StartupDefault::SetInt { field: "Width", value: 320 },
            StartupDefault::SetByte { field: "Mode", value: 1 },
            StartupDefault::SetByte { field: "Neighbor", value: 0xa5 },
            StartupDefault::SetByte { field: "Tail", value: 0x5a },
        ],
    },
    NativeClassSpec {
        short_name: "DefaultsChild", package: "StartupProbe", super_name: Some("DefaultsBase"),
        within: "Object", flags: 0, object_flags: 4, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<StartupRecord>()),
        operations: None, properties: &[],
        defaults: &[
            StartupDefault::SetBool { field: "Clear", value: false },
            StartupDefault::SetBool { field: "Enable", value: true },
            StartupDefault::SetFloat { field: "Gamma", value: 1.25 },
            StartupDefault::SetInt { field: "Width", value: 1024 },
            StartupDefault::SetByte { field: "Mode", value: 2 },
        ],
    },
];
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

// Also exercised by a temporary standalone consumer, outside the test runner.
pub fn exercise_inherited_startup_defaults() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"));
    let root = artifact.join(format!("startup-defaults-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    // SAFETY: every declaration uses a complete, correctly aligned UObject prefix
    // and offsets into its live record. The ordinary Object operations suffice.
    let owner = unsafe { CoreOwner::with_native_classes(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }, CLASSES) }.expect("initialize real canonical Core with native declarations");
    let access = owner.access();
    let base = access.native_class("StartupProbe.DefaultsBase").expect("register parent defaults");
    let child = access.native_class("StartupProbe.DefaultsChild").expect("register inherited defaults");
    unsafe {
        let base_default = access.default_object(base).unwrap().cast::<StartupRecord>();
        assert_eq!((*base_default).flags, 0x408);
        assert_eq!((*base_default).gamma, 0.5);
        assert_eq!((*base_default).width, 320);
        let child_default = access.default_object(child).unwrap().cast::<StartupRecord>();
        assert_eq!((*child_default).flags, 0x480, "setting and clearing inherited masks preserves neighboring bits");
        assert_eq!((*child_default).gamma, 1.25);
        assert_eq!((*child_default).width, 1024);
        assert_eq!((*child_default).mode, 2);
        assert_eq!((*child_default).neighbor, 0xa5, "byte enum writes must not clobber adjacent storage");
        assert_eq!((*child_default).tail, 0x5a);
        let object = access.construct_object(child, access.transient_package(), 0, 0, ptr::null_mut()).unwrap().cast::<StartupRecord>();
        assert_eq!((*object).flags, 0x480);
        assert_eq!((*object).gamma, 1.25);
        assert_eq!((*object).width, 1024);
        assert_eq!((*object).mode, 2);
        assert_eq!((*object).neighbor, 0xa5);
        assert_eq!((*object).tail, 0x5a);
        assert_eq!((*base_default).flags, 0x408, "child overrides must not mutate parent defaults");
        assert_eq!((*base_default).gamma, 0.5);
        assert_eq!((*base_default).width, 320);
        assert_eq!((*base_default).mode, 1);
        assert_eq!((*base_default).neighbor, 0xa5);
        assert_eq!((*base_default).tail, 0x5a);
    }
    owner.shutdown().expect("destroy canonical records and metadata");
    std::fs::write(root.join("report.txt"), "data_profile=data-none\ninherited_bool_masks_and_neighbor_preservation=ok\ninherited_float_and_int_overrides=ok\ninherited_byte_enum_and_adjacent_bytes=ok\nparent_default_independence=ok\nconstructed_object_defaults=ok\nshutdown=ok\n").unwrap();
}

#[test]
fn inherited_defaults_preserve_neighbor_bits_and_parent_values() {
    exercise_inherited_startup_defaults();
}
