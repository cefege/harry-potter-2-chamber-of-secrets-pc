//! Characterization of the config-cache publication seam used by console SET.
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};
use std::{fs, path::PathBuf};

fn units(value: &str) -> Vec<u32> { value.chars().map(u32::from).collect() }
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

#[test]
fn selective_flush_uses_raw_cache_key_not_system_alias() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join("console-config-flush-research");
    fs::create_dir_all(&root).unwrap();
    let system = root.join("Default.ini");
    let user = root.join("DefUser.ini");
    fs::write(&system, "[Example]\nValue=old\n").unwrap();
    fs::write(&user, "[Example]\nValue=other\n").unwrap();
    let owner = CoreOwner::new(CoreOptions { data_root: root, system_ini: system.clone(), user_ini: user.clone(), command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.config_set_units_from(Some(&units("System")), &units("Example"), &units("Value"), &units("new")).unwrap();
    access.config_set_units_from(Some(&units("User")), &units("Example"), &units("Value"), &units("pending")).unwrap();
    // This does not select the canonical absolute-path cache key. A successful
    // return therefore does not prove the property was published.
    access.config_flush_units_from(Some(&units("System")), false).unwrap();
    assert_eq!(fs::read_to_string(&system).unwrap(), "[Example]\nValue=old\n");
    assert_eq!(fs::read_to_string(&user).unwrap(), "[Example]\nValue=other\n");
    owner.shutdown().unwrap();
}
