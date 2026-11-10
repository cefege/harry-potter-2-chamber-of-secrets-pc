//! Real data-prototype HGame reflection and console selector characterization.
use hp2_abi::engine::generated::{UObject, UProperty};
use hp2_core::{core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult}, name_owner::FindName};
use hp2_engine::EngineOwner;
use std::{fs, path::{Path, PathBuf}, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }
fn copy_tree(source: &Path, dest: &Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() { copy_tree(&entry.path(), &dest.join(entry.file_name())); }
        else { fs::copy(entry.path(), dest.join(entry.file_name())).unwrap(); }
    }
}
#[test]
fn real_hgame_globalconfig_fields_and_selector_route() {
    let data = std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-hgame-research-{}", std::process::id()));
    copy_tree(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    // An isolated selector fixture, not an assertion about the retail ini.
    let system = root.join("System/Default.ini");
    let mut text = fs::read_to_string(&system).unwrap();
    text.push_str("\n[HGame]\nHarry=HGame.Harry\n[Engine]\nPlayerPawn=Engine.PlayerPawn\n");
    fs::write(&system, text).unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: system,
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    for package in ["Engine.u", "HGame.u"] { unsafe { access.load_package(&root.join("System").join(package), 1) }.unwrap(); }
    let harry = unsafe { access.static_load_class(access.native_class("Engine.PlayerPawn").unwrap(), ptr::null_mut(), &wide("HGame.Harry"), 2) }.unwrap();
    assert!(!harry.is_null());
    let property_class = access.native_class("Core.Property").unwrap();
    for name in ["bAutoCenterCamera", "bAutoQuaff", "bMoveWhileCasting", "ObjectDetail", "Difficulty", "bInvertMouse"] {
        let index = access.intern_str(name, FindName::Find).unwrap();
        let field = unsafe { access.find_field(harry.cast(), index, property_class).cast::<UProperty>() };
        assert!(!field.is_null(), "{name}");
        let flags = unsafe { (*field).property_flags };
        assert_eq!(flags & 0x44000, 0x44000, "{name} must be config and globalconfig");
        let declaring = unsafe { (*field.cast::<UObject>()).outer.cast::<UObject>() };
        let declared_name = access.name_text(i32::from_ne_bytes(unsafe { (*declaring).name })).unwrap();
        let declared: String = declared_name.into_iter().filter_map(char::from_u32).collect();
        let expected = if name.starts_with("bAuto") || name == "bMoveWhileCasting" { "Harry" } else { "PlayerPawn" };
        assert!(declared.eq_ignore_ascii_case(expected), "declaring class {declared} != {expected}");
    }
    let mut direct = Vec::new();
    let mut configured = Vec::new();
    for (command, out) in [("GET HGame.Harry bAutoCenterCamera", &mut direct), ("GET ini:HGame.Harry bAutoCenterCamera", &mut configured)] {
        assert!(unsafe { access.execute_static_command(wide(command).as_ptr(), &mut |line| { out.push(line.to_vec()); Ok(()) }) }.unwrap());
    }
    assert_eq!(direct, configured);
    assert_eq!(direct.len(), 1);
    assert!(!direct[0].iter().copied().filter_map(char::from_u32).collect::<String>().starts_with("Unrecognized"));
    owner.shutdown().unwrap();
}
