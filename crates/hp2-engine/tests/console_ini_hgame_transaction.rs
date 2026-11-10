//! Transactional stock commands against the real data-prototype HGame package.
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
unsafe fn command(access: CoreAccess<'_>, text: &str) -> RuntimeResult<Vec<String>> {
    let mut lines = Vec::new();
    unsafe { access.execute_static_command(wide(text).as_ptr(), &mut |line| {
        lines.push(line.iter().copied().filter_map(char::from_u32).collect()); Ok(())
    })?; }
    Ok(lines)
}
#[test]
fn stock_hgame_globalconfig_live_defaults_disk_and_failure() {
    let data = std::env::var_os("HP2_CORE_DATA_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-hgame-transaction-{}", std::process::id()));
    copy_tree(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let system = root.join("System/Default.ini");
    let mut text = fs::read_to_string(&system).unwrap();
    text.push_str("\n[HGame]\nHarry=HGame.Harry\n[Engine]\nPlayerPawn=Engine.PlayerPawn\n");
    fs::write(&system, text).unwrap();
    let user = root.join("System/DefUser.ini");
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: system, user_ini: user.clone(), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
    for package in ["Engine.u", "HGame.u"] { unsafe { access.load_package(&root.join("System").join(package), 1) }.unwrap(); }
    let pawn = access.native_class("Engine.PlayerPawn").unwrap();
    let harry = unsafe { access.static_load_class(pawn, ptr::null_mut(), &wide("HGame.Harry"), 2) }.unwrap();
    assert!(!harry.is_null());
    let name = access.intern_str("ConsoleTransactionHarry", FindName::Add).unwrap();
    let live = unsafe { access.construct_object(harry, harry.cast(), name, 0, ptr::null_mut()) }.unwrap();
    let property_class = access.native_class("Core.Property").unwrap();
    for (key, expected_owner) in [("bAutoCenterCamera", "harry"), ("bAutoQuaff", "harry"), ("bMoveWhileCasting", "harry"), ("ObjectDetail", "playerpawn"), ("Difficulty", "playerpawn"), ("bInvertMouse", "playerpawn")] {
        let index = access.intern_str(key, FindName::Find).unwrap();
        let field = unsafe { access.find_field(harry.cast(), index, property_class).cast::<UProperty>() };
        assert!(!field.is_null(), "{key}");
        assert_eq!(unsafe { (*field).property_flags } & 0x44000, 0x44000, "{key}");
        let owner_class = unsafe { (*field.cast::<hp2_abi::engine::generated::UObject>()).outer.cast::<hp2_abi::engine::generated::UObject>() };
        let owner_name = access.name_text(i32::from_ne_bytes(unsafe { (*owner_class).name })).unwrap();
        assert_eq!(owner_name.into_iter().filter_map(char::from_u32).collect::<String>().to_ascii_lowercase(), expected_owner);
    }
    for (key, value) in [("bAutoCenterCamera", "False"), ("bAutoQuaff", "False"), ("bMoveWhileCasting", "True"), ("ObjectDetail", "ObjectDetailLow"), ("Difficulty", "DifficultyHard")] {
        unsafe { command(access, &format!("SET ini:HGame.Harry {key} {value}")) }.unwrap();
        let lines = unsafe { command(access, &format!("GET ini:HGame.Harry {key}")) }.unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].eq_ignore_ascii_case(value), "{key}: {:?}", lines);
        let index = access.intern_str(key, FindName::Find).unwrap();
        let property = unsafe { access.find_field(harry.cast(), index, property_class).cast::<UProperty>() };
        let address = unsafe { access.property_address(live, property) }.unwrap();
        if value == "True" || value == "False" {
            let mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask as u32 };
            let bits = unsafe { address.cast::<u32>().read_unaligned() };
            assert_eq!(bits & mask != 0, value == "True", "live {key}");
        } else {
            let enumeration = unsafe { (*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum.cast::<hp2_abi::engine::generated::UEnum>() };
            let names = unsafe { &(*enumeration).names };
            let actual = unsafe { address.read() } as usize;
            assert!(actual < names.num as usize);
            let entry = unsafe { *names.data.cast::<hp2_abi::engine::generated::FName>().add(actual) };
            let text: String = access.name_text(entry.index).unwrap().into_iter().filter_map(char::from_u32).collect();
            assert!(text.eq_ignore_ascii_case(value), "live {key}: {text}");
        }
    }
    unsafe { command(access, "SET ini:Engine.PlayerPawn bInvertMouse True") }.unwrap();
    let property = unsafe { access.find_field(harry.cast(), access.intern_str("bInvertMouse", FindName::Find).unwrap(), property_class).cast::<UProperty>() };
    let address = unsafe { access.property_address(live, property) }.unwrap();
    let mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask as u32 };
    assert_ne!(unsafe { address.cast::<u32>().read_unaligned() } & mask, 0);
    let disk = fs::read_to_string(&user).unwrap();
    assert!(disk.to_ascii_lowercase().contains("bautocentercamera=false"), "{disk}");
    assert!(disk.to_ascii_lowercase().contains("objectdetail=objectdetaillow"), "{disk}");
    assert!(disk.to_ascii_lowercase().contains("binvertmouse=true"), "{disk}");
    let external = root.join("System/external-user.ini");
    fs::write(&external, "external write\n").unwrap();
    fs::rename(&external, &user).unwrap();
    let before = unsafe { (*live.cast::<AActor>()).b_light_changed() };
    assert!(unsafe { command(access, "SET ini:Engine.PlayerPawn bInvertMouse False") }.is_err());
    assert_ne!(unsafe { address.cast::<u32>().read_unaligned() } & mask, 0);
    assert_eq!(unsafe { (*live.cast::<AActor>()).b_light_changed() }, before);
    assert_eq!(fs::read_to_string(&user).unwrap(), "external write\n");
    fs::write(&external, disk).unwrap();
    fs::rename(&external, &user).unwrap();
    owner.shutdown().unwrap();
}
