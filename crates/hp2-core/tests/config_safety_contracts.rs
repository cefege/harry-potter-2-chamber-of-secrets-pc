use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn units(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).collect()
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}

fn fixture() -> (CoreOwner, PathBuf, Vec<u32>) {
    let artifact = PathBuf::from(
        std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated HP2_ARTIFACT_DIR required"),
    );
    assert!(artifact.is_dir());
    let root = artifact.join(format!(
        "config-safety-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let game = root.join("Game.ini");
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System.ini"),
        user_ini: root.join("User.ini"),
        command_line: Vec::new(),
        output,
        log_file: None,
    })
    .unwrap();
    let name = units(game.to_str().unwrap());
    (owner, game, name)
}

#[test]
fn staged_user_update_targets_canonical_cache_and_preserves_other_files() {
    let (owner, game, _) = fixture();
    let user_path = game.with_file_name("User.ini");
    fs::write(&user_path, b"[Engine.Input]\nW=MoveForward\nA=StrafeLeft\n").unwrap();
    let access = owner.access();
    access
        .config_set_units_from(None, &units("SystemOnly"), &units("Pending"), &units("yes"))
        .unwrap();
    let patch = [(units("Engine.Input"), units("W"), units("Jump"))];
    access.config_update_user_staged(&patch).unwrap();
    let text = fs::read_to_string(&user_path).unwrap();
    assert!(text.contains("W=Jump"));
    assert!(text.contains("A=StrafeLeft"));
    assert_eq!(
        access
            .config_get_units_from(Some(&units("User")), &units("Engine.Input"), &units("W"))
            .unwrap(),
        Some(units("Jump"))
    );
    assert!(
        !game.with_file_name("System.ini").exists(),
        "User update must not flush System"
    );
    owner.shutdown().unwrap();
    assert!(
        fs::read_to_string(game.with_file_name("System.ini"))
            .unwrap()
            .contains("Pending=yes")
    );
}

#[test]
fn staged_user_conflict_rolls_back_patch_and_keeps_prior_dirty_state() {
    let (owner, game, _) = fixture();
    let user_path = game.with_file_name("User.ini");
    let original = b"[Engine.Input]\nW=MoveForward\nA=StrafeLeft\n";
    fs::write(&user_path, original).unwrap();
    let access = owner.access();
    let user = units("User");
    access
        .config_set_units_from(
            Some(&user),
            &units("Engine.Input"),
            &units("Pending"),
            &units("Keep"),
        )
        .unwrap();
    let external = b"[Engine.Input]\nW=Other\nA=StrafeLeft\n";
    fs::write(&user_path, external).unwrap();
    let patch = [(units("Engine.Input"), units("W"), units("Jump"))];
    assert!(access.config_update_user_staged(&patch).is_err());
    assert_eq!(fs::read(&user_path).unwrap(), external);
    assert_eq!(
        access
            .config_get_units_from(Some(&user), &units("Engine.Input"), &units("W"))
            .unwrap(),
        Some(units("MoveForward"))
    );
    fs::write(&user_path, original).unwrap();
    owner.shutdown().unwrap();
    let text = fs::read_to_string(&user_path).unwrap();
    assert!(text.contains("W=MoveForward"));
    assert!(text.contains("Pending=Keep"));
    assert!(!text.contains("W=Jump"));
}

#[cfg(unix)]
#[test]
fn staged_user_write_failure_does_not_leak_into_later_shutdown() {
    use std::os::unix::fs::PermissionsExt;
    let (owner, game, _) = fixture();
    let user_path = game.with_file_name("User.ini");
    fs::write(&user_path, b"[Engine.Input]\nW=MoveForward\n").unwrap();
    let access = owner.access();
    let user = units("User");
    access
        .config_get_units_from(Some(&user), &units("Engine.Input"), &units("W"))
        .unwrap();
    let dir = game.parent().unwrap();
    fs::set_permissions(dir, fs::Permissions::from_mode(0o500)).unwrap();
    let result =
        access.config_update_user_staged(&[(units("Engine.Input"), units("W"), units("Jump"))]);
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(
        access
            .config_get_units_from(Some(&user), &units("Engine.Input"), &units("W"))
            .unwrap(),
        Some(units("MoveForward"))
    );
    owner.shutdown().unwrap();
    assert!(
        fs::read_to_string(user_path)
            .unwrap()
            .contains("W=MoveForward")
    );
}

#[test]
fn changed_values_preserve_other_keys_and_detect_external_changes() {
    let (owner, game, filename) = fixture();
    fs::write(&game, b"[Display]\r\nBrightness=0.4\r\nOther=keep\r\n").unwrap();
    let access = owner.access();
    let (section, brightness, other) = (units("Display"), units("Brightness"), units("Other"));
    assert_eq!(
        access
            .config_get_units_from(Some(&filename), &section, &other)
            .unwrap(),
        Some(units("keep"))
    );
    access
        .config_set_units_from(Some(&filename), &section, &brightness, &units("0.7"))
        .unwrap();
    let external = b"[Display]\r\nBrightness=0.4\r\nOther=external\r\n";
    fs::write(&game, external).unwrap();
    assert!(
        access
            .config_flush_units_from(Some(&filename), false)
            .is_err()
    );
    assert_eq!(fs::read(&game).unwrap(), external);
    // Restore the exact baseline: the same dirty cache must now publish.
    fs::write(&game, b"[Display]\r\nBrightness=0.4\r\nOther=keep\r\n").unwrap();
    access
        .config_flush_units_from(Some(&filename), false)
        .unwrap();
    let disk = fs::read_to_string(&game).unwrap();
    assert!(disk.contains("Brightness=0.7"));
    assert!(disk.contains("Other=keep"));
    owner.shutdown().unwrap();
}

#[test]
fn externally_created_file_is_not_overwritten() {
    let (owner, game, filename) = fixture();
    let access = owner.access();
    access
        .config_set_units_from(
            Some(&filename),
            &units("Display"),
            &units("ShowFPS"),
            &units("True"),
        )
        .unwrap();
    fs::write(&game, b"[Other]\nUserValue=keep\n").unwrap();
    assert!(
        access
            .config_flush_units_from(Some(&filename), false)
            .is_err()
    );
    assert_eq!(fs::read(&game).unwrap(), b"[Other]\nUserValue=keep\n");
    // Avoid shutdown retry against the external file; retain it as evidence.
    assert!(owner.shutdown().is_err());
    assert_eq!(fs::read(&game).unwrap(), b"[Other]\nUserValue=keep\n");
}

#[cfg(unix)]
#[test]
fn replacement_preserves_permissions_and_rejects_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let (owner, game, filename) = fixture();
    fs::write(&game, b"[Display]\nShowFPS=False\n").unwrap();
    fs::set_permissions(&game, fs::Permissions::from_mode(0o640)).unwrap();
    let access = owner.access();
    access
        .config_set_units_from(
            Some(&filename),
            &units("Display"),
            &units("ShowFPS"),
            &units("True"),
        )
        .unwrap();
    access
        .config_flush_units_from(Some(&filename), false)
        .unwrap();
    assert_eq!(
        fs::metadata(&game).unwrap().permissions().mode() & 0o777,
        0o640
    );
    owner.shutdown().unwrap();

    let (owner, link, filename) = fixture();
    let target = link.with_file_name("target.ini");
    let original = b"[Display]\nShowFPS=False\n";
    fs::write(&target, original).unwrap();
    symlink(&target, &link).unwrap();
    assert!(
        owner
            .access()
            .config_set_units_from(
                Some(&filename),
                &units("Display"),
                &units("ShowFPS"),
                &units("True")
            )
            .is_err()
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), original);
    owner.shutdown().unwrap();
}

#[test]
fn malformed_utf16_is_rejected_without_overwrite() {
    let (owner, game, filename) = fixture();
    let bad = [0xff, 0xfe, 0x41];
    fs::write(&game, bad).unwrap();
    let access = owner.access();
    assert!(
        access
            .config_set_units_from(
                Some(&filename),
                &units("Display"),
                &units("Brightness"),
                &units("0.7")
            )
            .is_err()
    );
    assert_eq!(fs::read(&game).unwrap(), bad);
    owner.shutdown().unwrap();
    assert_eq!(fs::read(&game).unwrap(), bad);
}

#[test]
fn encode_failure_keeps_dirty_state_for_a_later_success() {
    let (owner, game, filename) = fixture();
    let initial = b"[Display]\r\nBrightness=0.4\r\n";
    fs::write(&game, initial).unwrap();
    let access = owner.access();
    let (section, key) = (units("Display"), units("Brightness"));
    access
        .config_set_units_from(Some(&filename), &section, &key, &[0xd800])
        .unwrap();
    assert!(
        access
            .config_flush_units_from(Some(&filename), false)
            .is_err()
    );
    assert_eq!(fs::read(&game).unwrap(), initial);
    assert!(
        access
            .config_flush_units_from(Some(&filename), false)
            .is_err(),
        "failed encoding must remain dirty"
    );
    access
        .config_set_units_from(Some(&filename), &section, &key, &units("0.8"))
        .unwrap();
    access
        .config_flush_units_from(Some(&filename), false)
        .unwrap();
    assert!(
        fs::read_to_string(&game)
            .unwrap()
            .contains("Brightness=0.8")
    );
    owner.shutdown().unwrap();
}
