//! Metadata contract for the unmodified retail graphics-page extension.
use hp2_abi::engine::generated::{UClass, UFunction, UProperty};
use hp2_core::{core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult}, name_owner::FindName};
use hp2_engine::EngineOwner;
use std::{fs, path::{Path, PathBuf}, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap(); let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() { copy_tree(&entry.path(), &target); } else { fs::copy(entry.path(), target).unwrap(); }
    }
}
#[test]
#[ignore = "requires verified retail data and HP2_ARTIFACT_DIR"]
fn retail_display_control_metadata() {
    let data = PathBuf::from(std::env::var_os("HP2_TEST_DATA_ROOT").unwrap());
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("display-retail-{}", std::process::id()));
    assert!(!root.exists()); copy_tree(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] { let src = data.join(dir); if src.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(src).unwrap(), root.join(dir)).unwrap(); } }
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: vec![], output, log_file: None }).unwrap();
    let a = owner.access(); a.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    unsafe { a.load_package(&root.join("System/Engine.u"), 1).unwrap(); a.load_package(&root.join("System/HGame.u"), 1).unwrap(); }
    let class = |path: &str| -> *mut UClass { let s: Vec<u32> = path.chars().map(u32::from).chain([0]).collect(); let p = unsafe { a.static_load_class(a.native_class("Core.Object").unwrap(), ptr::null_mut(), &s, 2) }.unwrap(); assert!(!p.is_null(), "{path}"); p };
    let page = class("HGame.FESoundVideoPage"); let combo = class("HGame.HPMenuOptionCombo"); let dialog = class("HGame.HPMessageBox"); let edit = class("HGame.HPMenuOptionEditBox");
    let check = |structure: *mut std::ffi::c_void, name: &str, kind: &str| -> *mut UProperty {
        let p = unsafe { a.find_field(structure.cast(), a.intern_str(name, FindName::Find).unwrap(), a.native_class(kind).unwrap()) }.cast::<UProperty>();
        assert!(!p.is_null(), "missing {name} ({kind})");
        if kind != "Core.Function" {
            let expected = match kind {
                "Core.StrProperty" => std::mem::size_of::<hp2_abi::engine::generated::TArrayRaw>(),
                "Core.ObjectProperty" | "Core.ClassProperty" => std::mem::size_of::<*mut std::ffi::c_void>(),
                "Core.ByteProperty" => 1,
                _ => 4,
            };
            assert_eq!(unsafe { (*p).element_size } as usize, expected, "width for {name} ({kind})");
            assert_eq!(unsafe { (*p).array_dim }, 1, "array dimension for {name}");
        }
        p
    };
    for (c, name, kind) in [(page, "Root", "Core.ObjectProperty"), (page, "ResolutionCombo", "Core.ObjectProperty"),
        (combo, "EditBox", "Core.ObjectProperty"), (combo, "EditBoxWidth", "Core.FloatProperty"), (combo, "bAcceptsFocus", "Core.BoolProperty"),
        (combo, "bWindowVisible", "Core.BoolProperty"), (combo, "bListVisible", "Core.BoolProperty"), (combo, "TextColor", "Core.StructProperty"),
        (edit, "Value", "Core.StrProperty"), (edit, "TextColor", "Core.StructProperty"),
        (edit, "WinLeft", "Core.FloatProperty"), (edit, "WinTop", "Core.FloatProperty"),
        (edit, "WinWidth", "Core.FloatProperty"), (edit, "WinHeight", "Core.FloatProperty"),
        (dialog, "Result", "Core.StrProperty"), (dialog, "bClosing", "Core.BoolProperty")] { check(c.cast(), name, kind); }
    for (c, name, args) in [
        (page, "CreateControl", &[("ControlClass", "Core.ClassProperty"), ("X", "Core.FloatProperty"), ("Y", "Core.FloatProperty"), ("W", "Core.FloatProperty"), ("H", "Core.FloatProperty"), ("ReturnValue", "Core.ObjectProperty")][..]),
        (page, "Notify", &[("C", "Core.ObjectProperty"), ("E", "Core.ByteProperty")][..]),
        (page, "doHPMessageBox", &[("msg", "Core.StrProperty"), ("textButton1", "Core.StrProperty"), ("textButton2", "Core.StrProperty"), ("timeOut", "Core.FloatProperty"), ("ReturnValue", "Core.ObjectProperty")][..]),
        (combo, "SetText", &[("NewText", "Core.StrProperty")][..]),
        (combo, "SetFont", &[("NewFont", "Core.IntProperty")][..]),
        (combo, "SetEditable", &[("bNewCanEdit", "Core.BoolProperty")][..]),
        (combo, "AddItem", &[("S", "Core.StrProperty")][..]),
        (combo, "CloseUpWithNoSound", &[][..]),
    ] {
        let f = check(c.cast(), name, "Core.Function").cast::<UFunction>();
        for &(name, kind) in args {
            let p = check(f.cast(), name, kind);
            unsafe { assert_ne!((*p).property_flags & 0x80, 0, "{name}"); assert!((*p).offset >= 0 && ((*p).offset as usize + (*p).element_size as usize) <= (*f).parms_size as usize, "{name}"); }
        }
    }
}
