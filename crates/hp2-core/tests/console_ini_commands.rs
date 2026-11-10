use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};
use std::{fs, path::PathBuf};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }

#[test]
fn configured_class_get_routes_to_canonical_defaults() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join("console-ini-commands");
    fs::create_dir_all(root.join("System")).unwrap();
    let system = root.join("System/Default.ini");
    let user = root.join("System/DefUser.ini");
    fs::write(&system, "[Engine.Engine]\nAudioDevice=Core.System\nViewportManager=Core.System\n[Core.System]\nSavePath=ConfiguredValue\n").unwrap();
    fs::write(&user, "").unwrap();
    let owner = CoreOwner::new(CoreOptions { data_root: root, system_ini: system, user_ini: user,
        command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.native_class("Core.System").unwrap();
    let mut results = Vec::new();
    for command in ["GET Core.System SavePath", "GET ini:Engine.Engine.AudioDevice SavePath", "GET ini:Engine.Engine.ViewportManager SavePath"] {
        let mut lines = Vec::new();
        assert!(unsafe { access.execute_static_command(wide(command).as_ptr(), &mut |line| { lines.push(line.to_vec()); Ok(()) }) }.unwrap());
        assert_eq!(lines.len(), 1, "{command}");
        results.push(lines);
    }
    let result: String = results[0][0].iter().copied().filter_map(char::from_u32).collect();
    assert!(!result.starts_with("Unrecognized"), "{result}");
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], results[2]);

    owner.shutdown().unwrap();
}

#[test]
fn direct_class_fallback_requires_missing_ini_key_and_preserves_aliases() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join("console-ini-selector-precedence");
    fs::create_dir_all(root.join("System")).unwrap();
    let system = root.join("System/Default.ini");
    let user = root.join("System/DefUser.ini");
    fs::write(&system, "[Engine.Engine]\nAudioDevice=Core.System\n").unwrap();
    fs::write(&user, "").unwrap();
    let owner = CoreOwner::new(CoreOptions { data_root: root, system_ini: system, user_ini: user,
        command_line: Vec::new(), output, log_file: None }).unwrap();
    let access = owner.access();
    access.native_class("Core.System").unwrap();
    access.native_class("Core.Package").unwrap();
    let run = |text: &str| {
        let mut lines = Vec::new();
        assert!(unsafe { access.execute_static_command(wide(text).as_ptr(), &mut |line| {
            lines.push(line.iter().copied().filter_map(char::from_u32).collect::<String>()); Ok(())
        }) }.unwrap());
        assert_eq!(lines.len(), 1);
        lines.remove(0)
    };
    let direct = run("GET Core.System SavePath");
    assert_eq!(run("GET INI:Core.System SavePath"), direct);
    assert_eq!(run("GET ini:Engine.Engine.AudioDevice SavePath"), direct);
    assert!(run("GET usr:Core.System SavePath").starts_with("Unrecognized class"));
    access.config_set_units_from(Some(&wide("System")), &wide("Core"), &wide("System"), &wide("Core.Package")).unwrap();
    assert!(run("GET ini:Core.System SavePath").starts_with("Unrecognized property"), "a present alias must win over a loaded direct class");
    access.config_set_units_from(Some(&wide("System")), &wide("Core"), &wide("System"), &wide("Missing.Class")).unwrap();
    assert!(run("GET ini:Core.System SavePath").starts_with("Unrecognized class"), "a broken alias must not fall back");
    access.config_set_units_from(Some(&wide("System")), &wide("Core"), &wide("System"), &wide("")).unwrap();
    assert!(run("GET ini:Core.System SavePath").starts_with("Unrecognized class"), "an empty alias must not fall back");
    owner.shutdown().unwrap();
}
