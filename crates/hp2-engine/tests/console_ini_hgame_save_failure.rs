//! Real HGame actor: a failed publication must not run PostEditChange.
use hp2_abi::engine::generated::{AActor, UBoolProperty, UProperty};
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
fn external_user_edit_keeps_real_harry_and_defaults_unchanged() {
    let data = std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-hgame-save-failure-{}", std::process::id()));
    copy_tree(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let system = root.join("System/Default.ini");
    let mut text = fs::read_to_string(&system).unwrap();
    text.push_str("\n[HGame]\nHarry=HGame.Harry\n");
    fs::write(&system, text).unwrap();
    let user = root.join("System/DefUser.ini");
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: system, user_ini: user.clone(), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    for package in ["Engine.u", "HGame.u"] { unsafe { access.load_package(&root.join("System").join(package), 1) }.unwrap(); }
    let harry = unsafe { access.static_load_class(access.native_class("Engine.PlayerPawn").unwrap(), ptr::null_mut(), &wide("HGame.Harry"), 2) }.unwrap();
    let live = unsafe { access.construct_object(harry, harry.cast(), access.intern_str("SaveFailureHarry", FindName::Add).unwrap(), 0, ptr::null_mut()) }.unwrap();
    let property = unsafe { access.find_field(harry.cast(), access.intern_str("bAutoQuaff", FindName::Find).unwrap(), access.native_class("Core.Property").unwrap()).cast::<UProperty>() };
    assert!(!property.is_null());
    let live_address = unsafe { access.property_address(live, property) }.unwrap();
    let default = unsafe { access.default_object(harry) }.unwrap();
    let default_address = unsafe { access.property_address(default, property) }.unwrap();
    let mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask as u32 };
    let live_bits = unsafe { live_address.cast::<u32>().read_unaligned() };
    let default_bits = unsafe { default_address.cast::<u32>().read_unaligned() };
    unsafe { (*live.cast::<AActor>()).set_b_light_changed(false); }
    let baseline = fs::read(&user).unwrap();
    let external = root.join("System/external-user.ini");
    fs::write(&external, "external edit\n").unwrap();
    fs::rename(&external, &user).unwrap();
    let value = if live_bits & mask == 0 { "True" } else { "False" };
    let cmd = wide(&format!("SET ini:HGame.Harry bAutoQuaff {value}"));
    assert!(unsafe { access.execute_static_command(cmd.as_ptr(), &mut |_| Ok(())) }.is_err());
    assert_eq!(unsafe { live_address.cast::<u32>().read_unaligned() }, live_bits);
    assert_eq!(unsafe { default_address.cast::<u32>().read_unaligned() }, default_bits);
    assert!(!unsafe { (*live.cast::<AActor>()).b_light_changed() }, "PostEditChange must not run");
    assert_eq!(fs::read(&user).unwrap(), b"external edit\n");
    fs::write(&external, &baseline).unwrap();
    fs::rename(&external, &user).unwrap();
    for text in ["SET ini:HGame.Harry bAutoQuaff not-a-bool", "SET ini:HGame.Harry Difficulty 256", "SET ini:HGame.Harry ObjectDetail InvalidDetail"] {
        assert!(unsafe { access.execute_static_command(wide(text).as_ptr(), &mut |_| Ok(())) }.is_err(), "{text}");
    }
    assert_eq!(unsafe { live_address.cast::<u32>().read_unaligned() }, live_bits);
    assert_eq!(unsafe { default_address.cast::<u32>().read_unaligned() }, default_bits);
    assert!(!unsafe { (*live.cast::<AActor>()).b_light_changed() });
    assert_eq!(fs::read(&user).unwrap(), baseline);
    owner.shutdown().unwrap();
}
