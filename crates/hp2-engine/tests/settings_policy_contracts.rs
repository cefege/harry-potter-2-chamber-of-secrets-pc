use std::collections::BTreeMap;

use hp2_engine::settings_policy::{ApplyMode, ConfigFile, SettingsPatch};

fn key(section: &str, key: &str) -> (ConfigFile, String, String) {
    let file = if section == "Engine.PlayerPawn" {
        ConfigFile::User
    } else {
        ConfigFile::System
    };
    (file, section.to_owned(), key.to_owned())
}

#[test]
fn patch_merges_only_changed_keys_and_classifies_them() {
    let brightness = key("sdldrv.sdlclient", "brightness");
    let fps = key("SDLDrv.SDLClient", "ShowFPS");
    let unrelated = key("SDLDrv.SDLClient", "UseJoystick");
    let user = key("Engine.PlayerPawn", "MouseSensitivity");
    let vsync = key("WgpuDrv.WgpuRenderDevice", "UseVSync");
    let mut snapshot = BTreeMap::from([
        (brightness.clone(), "0.4".to_string()),
        (fps.clone(), "False".to_string()),
        (unrelated.clone(), "True".to_string()),
        (user.clone(), "3".to_string()),
        (vsync.clone(), "Off".to_string()),
    ]);
    let plan = SettingsPatch {
        brightness: Some(0.7),
        vertical_sync: Some(true),
        ..Default::default()
    }
    .plan()
    .unwrap();
    assert_eq!(plan.changes().len(), 2);
    assert_eq!(plan.changes()[0].mode, ApplyMode::RestartRequired);
    assert_eq!(plan.changes()[1].mode, ApplyMode::RestartRequired);
    plan.merge_into(&mut snapshot);
    assert_eq!(snapshot.len(), 5);
    assert_eq!(snapshot[&brightness], "0.7");
    assert_eq!(snapshot[&vsync], "On");
    assert_eq!(snapshot[&fps], "False");
    assert_eq!(snapshot[&unrelated], "True");
    assert_eq!(snapshot[&user], "3");
}

#[test]
fn validation_rejects_entire_patch_and_checks_boundaries() {
    for value in [f64::NAN, f64::INFINITY, 0.09, 1.01] {
        let patch = SettingsPatch {
            brightness: Some(value),
            show_fps: Some(true),
            ..Default::default()
        };
        assert_eq!(patch.plan().unwrap_err().field, "brightness");
    }
    for value in [0.1, 1.0] {
        assert!(
            SettingsPatch {
                brightness: Some(value),
                ..Default::default()
            }
            .plan()
            .is_ok()
        );
    }
    for value in [0, 30, 60, 120, 144] {
        assert!(
            SettingsPatch {
                frame_rate_limit: Some(value),
                ..Default::default()
            }
            .plan()
            .is_ok()
        );
    }
    for patch in [
        SettingsPatch {
            sound_volume: Some(f64::NAN),
            ..Default::default()
        },
        SettingsPatch {
            mouse_sensitivity: Some(0.19),
            ..Default::default()
        },
        SettingsPatch {
            ui_scale: Some(1.1),
            ..Default::default()
        },
        SettingsPatch {
            render_scale: Some(0.6),
            ..Default::default()
        },
        SettingsPatch {
            anti_aliasing_samples: Some(3),
            ..Default::default()
        },
        SettingsPatch {
            anisotropy: Some(2),
            ..Default::default()
        },
        SettingsPatch {
            resolution: Some(hp2_engine::settings_policy::Resolution {
                width: 319,
                height: 600,
            }),
            ..Default::default()
        },
    ] {
        assert!(patch.plan().is_err());
    }
    for value in [-1, 50, 145] {
        assert_eq!(
            SettingsPatch {
                frame_rate_limit: Some(value),
                ..Default::default()
            }
            .plan()
            .unwrap_err()
            .field,
            "frame_rate_limit"
        );
    }
}

#[test]
fn empty_patch_is_noop_and_live_candidates_are_not_claimed_applied() {
    assert!(
        SettingsPatch::default()
            .plan()
            .unwrap()
            .changes()
            .is_empty()
    );
    let plan = SettingsPatch {
        show_fps: Some(false),
        native_text: Some(true),
        frame_rate_limit: Some(60),
        ..Default::default()
    }
    .plan()
    .unwrap();
    assert_eq!(
        plan.changes()
            .iter()
            .map(|c| (c.key.key, c.value.as_str(), c.mode))
            .collect::<Vec<_>>(),
        vec![
            ("ShowFPS", "False", ApplyMode::RestartRequired),
            ("NativeText", "True", ApplyMode::RestartRequired),
            ("FrameRateLimit", "60", ApplyMode::RestartRequired),
        ]
    );
}
