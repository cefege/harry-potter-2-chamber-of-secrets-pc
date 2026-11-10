//! Retail package metadata prerequisite for a stock-menu FPS control.
//! This test does not create live windows or verify placement or dispatch.
use hp2_abi::engine::generated::{UProperty, UStruct};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_engine::EngineOwner;
use std::{fs, path::{Path, PathBuf}, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dst = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() { copy_tree(&entry.path(), &dst); }
        else { fs::copy(entry.path(), dst).unwrap(); }
    }
}

#[test]
#[ignore = "requires separately verified retail data and HP2_ARTIFACT_DIR"]
fn retail_stock_menu_reflection_prerequisites() {
    let data = PathBuf::from(std::env::var_os("HP2_TEST_DATA_ROOT").expect("HP2_TEST_DATA_ROOT"));
    let artifacts = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR"));
    let root = artifacts.join(format!("stock-fps-retail-metadata-{}", std::process::id()));
    assert!(!root.exists());
    copy_tree(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(),
        output, log_file: None,
    }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    unsafe {
        access.load_package(&root.join("System/Engine.u"), 1).unwrap();
        access.load_package(&root.join("System/HGame.u"), 1).unwrap();
    }
    let class_class = access.native_class("Core.Object").unwrap();
    let load = |name: &str| {
        let text: Vec<u32> = name.chars().map(u32::from).chain([0]).collect();
        let class = unsafe { access.static_load_class(class_class, ptr::null_mut(), &text, 2) }.unwrap();
        assert!(!class.is_null(), "missing class {name}");
        class
    };
    let page = load("HGame.FESoundVideoPage");
    let book = load("HGame.FEBook");
    let checkbox = load("HGame.HPMenuOptionCheckBox");
    let function_kind = access.native_class("Core.Function").unwrap();
    let object_kind = access.native_class("Core.ObjectProperty").unwrap();
    let bool_kind = access.native_class("Core.BoolProperty").unwrap();
    let mut create = ptr::null_mut();
    let mut notify = ptr::null_mut();
    for (class, name, kind) in [
        (book, "SoundVideoPage", object_kind),
        (page, "Created", function_kind),
        (page, "Notify", function_kind),
        (page, "CreateControl", function_kind),
        (checkbox, "bChecked", bool_kind),
        (checkbox, "LMouseUp", function_kind),
    ] {
        let index = access.intern_str(name, FindName::Find).unwrap();
        let field = unsafe { access.find_field(class.cast::<UStruct>(), index, kind) };
        assert!(!field.is_null(), "missing field {name}");
        if class == page && name == "CreateControl" { create = field; }
        if class == page && name == "Notify" { notify = field; }
        if kind != function_kind {
            let property = field.cast::<UProperty>();
            unsafe {
                assert_eq!((*property).array_dim, 1, "{name}");
                let offset = usize::try_from((*property).offset).unwrap();
                let size = usize::try_from((*property).element_size).unwrap();
                assert!(offset.checked_add(size).unwrap() <= (*class).properties_size as usize, "{name}");
            }
        }
    }
    // Runtime invocation must use reflected fields, not C ABI struct layout.
    for (function, args) in [
        (create, &[ ("ControlClass", "Core.ClassProperty"), ("X", "Core.FloatProperty"),
            ("Y", "Core.FloatProperty"), ("W", "Core.FloatProperty"), ("H", "Core.FloatProperty"),
            ("OwnerWindow", "Core.ObjectProperty"), ("ReturnValue", "Core.ObjectProperty") ][..]),
        (notify, &[ ("C", "Core.ObjectProperty"), ("E", "Core.ByteProperty") ][..]),
    ] {
        let function = function.cast::<hp2_abi::engine::generated::UFunction>();
        let size = unsafe { (*function).parms_size as usize };
        for &(name, kind) in args {
            let field = unsafe { access.find_field(function.cast::<UStruct>(), access.intern_str(name, FindName::Find).unwrap(), access.native_class(kind).unwrap()) }.cast::<UProperty>();
            assert!(!field.is_null(), "missing {name} ({kind})");
            unsafe {
                assert_ne!((*field).property_flags & 0x80, 0, "{name} is not a parameter");
                let offset = usize::try_from((*field).offset).unwrap();
                let width = usize::try_from((*field).element_size).unwrap();
                assert!(offset.checked_add(width).is_some_and(|end| end <= size), "{name} outside parameter frame");
            }
        }
    }
}
