use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};
use hp2_engine::settings_config::{self, SettingsError};
use hp2_engine::settings_policy::{
    ApplyMode, ConfigFile, ControlMode, Difficulty, ObjectDetail, Resolution, ScreenMode,
    SettingsPatch, TextureDetail,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn units(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn owner(root: &PathBuf) -> CoreOwner {
    CoreOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("Game.ini"),
        user_ini: root.join("User.ini"),
        command_line: vec![],
        output,
        log_file: None,
    })
    .unwrap()
}
fn fixture() -> (PathBuf, CoreOwner) {
    let artifact =
        PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR required"));
    assert!(artifact.is_dir());
    let root = artifact.join(format!(
        "settings-config-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Game.ini"), b"[SDLDrv.SDLClient]\r\nBrightness=0.4\r\nShowFPS=False\r\nOther=keep\r\n[Other]\r\nValue=preserve\r\n").unwrap();
    let runtime = owner(&root);
    (root, runtime)
}
fn value(snapshot: &settings_config::SettingsSnapshot, key: &str) -> Option<String> {
    snapshot
        .values
        .iter()
        .find(|(k, _)| k.key == key)
        .unwrap()
        .1
        .clone()
}

#[test]
fn read_patch_persist_reload_and_keep_other_cached_values() {
    let (root, runtime) = fixture();
    let access = runtime.access();
    assert_eq!(
        value(&settings_config::read(access).unwrap(), "Brightness").as_deref(),
        Some("0.4")
    );
    access
        .config_set_units_from(
            None,
            &units("Other"),
            &units("LiveCacheChange"),
            &units("kept"),
        )
        .unwrap();
    let user = units("User");
    access
        .config_set_units_from(
            Some(&user),
            &units("Private"),
            &units("Value"),
            &units("not-flushed"),
        )
        .unwrap();
    let plan = settings_config::apply(
        access,
        &SettingsPatch {
            brightness: Some(0.7),
            show_fps: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        plan.changes()
            .iter()
            .all(|c| c.mode == ApplyMode::RestartRequired)
    );
    assert!(
        !root.join("User.ini").exists(),
        "adapter must not flush unrelated files"
    );
    let game = fs::read_to_string(root.join("Game.ini")).unwrap();
    for text in [
        "Brightness=0.7",
        "ShowFPS=True",
        "Other=keep",
        "Value=preserve",
        "LiveCacheChange=kept",
    ] {
        assert!(game.contains(text), "missing {text}");
    }
    runtime.shutdown().unwrap();
    let second = owner(&root);
    let snapshot = settings_config::read(second.access()).unwrap();
    assert_eq!(value(&snapshot, "Brightness").as_deref(), Some("0.7"));
    assert_eq!(value(&snapshot, "ShowFPS").as_deref(), Some("True"));
    second.shutdown().unwrap();
}

#[test]
fn system_launcher_mappings_preserve_unrelated_config() {
    let (root, runtime) = fixture();
    let patch = SettingsPatch {
        screen_mode: Some(ScreenMode::BorderlessDesktop),
        resolution: Some(Resolution {
            width: 1280,
            height: 720,
        }),
        render_scale: Some(0.75),
        ui_scale: Some(1.25),
        anti_aliasing_samples: Some(2),
        anisotropy: Some(8),
        texture_detail: Some(TextureDetail::Medium),
        maintain_vertical_fov: Some(false),
        screen_flashes: Some(false),
        joystick_enabled: Some(true),
        sound_enabled: Some(true),
        sound_volume: Some(0.9),
        music_volume: Some(0.5),
        ..Default::default()
    };
    settings_config::apply(runtime.access(), &patch).unwrap();
    let text = fs::read_to_string(root.join("Game.ini")).unwrap();
    for expected in [
        "StartupFullscreen=True",
        "BorderlessWindow=True",
        "UseDesktopResolution=True",
        "WindowedViewportX=1280",
        "FullscreenViewportY=720",
        "RenderScale=0.75",
        "UIScale=1.25",
        "UseAA=True",
        "NumAASamples=2",
        "MaxAnisotropy=8",
        "TextureDetail=Medium",
        "MaintainVerticalFOV=False",
        "ScreenFlashes=False",
        "UseJoystick=True",
        "UseSound=True",
        "SoundVolume=0.9",
        "MusicVolume=0.5",
        "Other=keep",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    runtime.shutdown().unwrap();
}

#[test]
fn user_fields_persist_and_mixed_patches_are_rejected() {
    let (root, runtime) = fixture();
    let user = root.join("User.ini");
    fs::write(&user, b"[Engine.PlayerPawn]\nMouseSensitivity=3\nKeep=untouched\n[HGame.Harry]\nbAutoQuaff=True\n").unwrap();
    let access = runtime.access();
    let before_game = fs::read(root.join("Game.ini")).unwrap();
    let before_user = fs::read(&user).unwrap();
    assert!(matches!(
        settings_config::apply(
            access,
            &SettingsPatch {
                brightness: Some(0.8),
                invert_mouse: Some(true),
                ..Default::default()
            }
        ),
        Err(SettingsError::MixedFiles)
    ));
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), before_game);
    assert_eq!(fs::read(&user).unwrap(), before_user);
    let patch = SettingsPatch {
        mouse_sensitivity: Some(5.0),
        invert_mouse: Some(true),
        control_mode: Some(ControlMode::Modern),
        difficulty: Some(Difficulty::Hard),
        object_detail: Some(ObjectDetail::Low),
        auto_center_camera: Some(false),
        move_while_casting: Some(true),
        auto_quaff: Some(false),
        ..Default::default()
    };
    let plan = settings_config::apply(access, &patch).unwrap();
    assert!(
        plan.changes()
            .iter()
            .all(|v| v.file == ConfigFile::User && v.mode == ApplyMode::RestartRequired)
    );
    let snapshot = settings_config::read_file(access, ConfigFile::User).unwrap();
    assert_eq!(
        value(&snapshot, "Difficulty").as_deref(),
        Some("DifficultyHard")
    );
    assert_eq!(
        value(&snapshot, "bModernThirdPersonControls").as_deref(),
        Some("True")
    );
    let text = fs::read_to_string(&user).unwrap();
    for expected in [
        "MouseSensitivity=5",
        "bInvertMouse=True",
        "bModernThirdPersonControls=True",
        "Difficulty=DifficultyHard",
        "ObjectDetail=ObjectDetailLow",
        "bAutoCenterCamera=False",
        "bMoveWhileCasting=True",
        "bAutoQuaff=False",
        "Keep=untouched",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), before_game);
    runtime.shutdown().unwrap();
}

#[test]
fn user_settings_reload_preserves_unrelated_input_bindings() {
    let (root, runtime) = fixture();
    let user_path = root.join("User.ini");
    fs::write(&user_path, b"[Engine.Input]\nW=MoveForward\nSpace=Jump\nMouseX=Count bXAxis | Axis aMouseX Speed=2.0\n[Engine.PlayerPawn]\nMouseSensitivity=3\nbInvertMouse=False\n[HGame.Harry]\nbAutoQuaff=True\n").unwrap();
    let game_before = fs::read(root.join("Game.ini")).unwrap();
    let access = runtime.access();
    let user = units("User");
    // Retain an already-dirty input update from the engine's canonical cache.
    access
        .config_set_units_from(
            Some(&user),
            &units("Engine.Input"),
            &units("F"),
            &units("Interact"),
        )
        .unwrap();
    let plan = settings_config::apply(
        access,
        &SettingsPatch {
            mouse_sensitivity: Some(4.5),
            invert_mouse: Some(true),
            control_mode: Some(ControlMode::Modern),
            difficulty: Some(Difficulty::Medium),
            object_detail: Some(ObjectDetail::High),
            auto_center_camera: Some(false),
            move_while_casting: Some(false),
            auto_quaff: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(plan.changes().len(), 8);
    assert!(
        plan.changes()
            .iter()
            .all(|c| c.file == ConfigFile::User && c.mode == ApplyMode::RestartRequired)
    );
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), game_before);
    runtime.shutdown().unwrap();

    let reloaded = owner(&root);
    let access = reloaded.access();
    let snapshot = settings_config::read_file(access, ConfigFile::User).unwrap();
    for (key, expected) in [
        ("MouseSensitivity", "4.5"),
        ("bInvertMouse", "True"),
        ("bModernThirdPersonControls", "True"),
        ("Difficulty", "DifficultyMedium"),
        ("ObjectDetail", "ObjectDetailHigh"),
        ("bAutoCenterCamera", "False"),
        ("bMoveWhileCasting", "False"),
        ("bAutoQuaff", "False"),
    ] {
        assert_eq!(value(&snapshot, key).as_deref(), Some(expected), "{key}");
    }
    for (key, expected) in [
        ("W", "MoveForward"),
        ("Space", "Jump"),
        ("MouseX", "Count bXAxis | Axis aMouseX Speed=2.0"),
        ("F", "Interact"),
    ] {
        assert_eq!(
            access
                .config_get_units_from(Some(&user), &units("Engine.Input"), &units(key))
                .unwrap(),
            Some(units(expected)),
            "{key}"
        );
    }
    reloaded.shutdown().unwrap();
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), game_before);
}

#[test]
fn conflict_leaves_clean_cache_unchanged_and_shutdown_preserves_external_file() {
    let (root, runtime) = fixture();
    let access = runtime.access();
    settings_config::read(access).unwrap();
    let external = b"[SDLDrv.SDLClient]\nBrightness=0.9\nOther=external\n";
    fs::write(root.join("Game.ini"), external).unwrap();
    assert!(matches!(
        settings_config::apply(
            access,
            &SettingsPatch {
                brightness: Some(0.7),
                ..Default::default()
            }
        ),
        Err(SettingsError::CacheOrFlush(_))
    ));
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
    assert_eq!(
        value(&settings_config::read(access).unwrap(), "Brightness").as_deref(),
        Some("0.4")
    );
    runtime.shutdown().unwrap();
    assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
}

#[test]
fn write_failure_is_reported_and_invalid_patch_does_not_mutate() {
    let (root, runtime) = fixture();
    let access = runtime.access();
    assert!(matches!(
        settings_config::apply(
            access,
            &SettingsPatch {
                brightness: Some(f64::NAN),
                show_fps: Some(true),
                ..Default::default()
            }
        ),
        Err(SettingsError::Validation(_))
    ));
    assert_eq!(
        value(&settings_config::read(access).unwrap(), "ShowFPS").as_deref(),
        Some("False")
    );
    // An unrelated pending engine edit must keep its pre-existing dirty state.
    access
        .config_set_units_from(None, &units("Other"), &units("Pending"), &units("retained"))
        .unwrap();
    // A read-only directory prevents staging without changing the cached file.
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
    let result = settings_config::apply(
        access,
        &SettingsPatch {
            show_fps: Some(true),
            ..Default::default()
        },
    );
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(result, Err(SettingsError::CacheOrFlush(_))));
    assert!(
        fs::read_to_string(root.join("Game.ini"))
            .unwrap()
            .contains("ShowFPS=False")
    );
    assert_eq!(
        value(&settings_config::read(access).unwrap(), "ShowFPS").as_deref(),
        Some("False")
    );
    // Shutdown publishes the unrelated prior dirty edit, not the failed patch.
    runtime.shutdown().unwrap();
    let game = fs::read_to_string(root.join("Game.ini")).unwrap();
    assert!(game.contains("Pending=retained"));
    assert!(game.contains("ShowFPS=False"));
}
