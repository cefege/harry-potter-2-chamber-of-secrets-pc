//! Contract against the actual launcher serializer and validator, not a copy.
//!
//! Intentional differences: a changed-field patch leaves launcher-wide device
//! bindings and fixed color-bit settings alone; mixed files cannot be committed
//! atomically by the adapter; and patch validation ignores unrelated fields.
//! Continuous floats use Rust round-trip text instead of launcher's 15-digit
//! formatting, pinned separately below; this can preserve more precision.
use hp2_engine::settings_policy as e;
use hp2_ini::launcher::Document;
use hp2_launcher::{model as l, store};

fn check(settings: &l::LauncherSettings, patch: e::SettingsPatch) {
    store::validate_launcher_settings(settings).unwrap();
    let plan = patch.plan().unwrap();
    assert!(!plan.changes().is_empty());
    let (mut game, mut user) = (Document::new(), Document::new());
    store::apply_game(&mut game, settings);
    store::apply_user(&mut user, settings);
    for change in plan.changes() {
        let doc = match change.file {
            e::ConfigFile::System => &game,
            e::ConfigFile::User => &user,
        };
        assert_eq!(
            doc.get(change.key.section, change.key.key).as_deref(),
            Some(change.value.as_str()),
            "{} / {}",
            change.key.section,
            change.key.key
        );
    }
}

#[test]
fn launcher_system_discrete_and_grouped_fields_match() {
    let mut s = l::LauncherSettings::default();
    for (lm, em) in [
        (l::ScreenMode::Windowed, e::ScreenMode::Windowed),
        (l::ScreenMode::Fullscreen, e::ScreenMode::Fullscreen),
        (
            l::ScreenMode::BorderlessDesktop,
            e::ScreenMode::BorderlessDesktop,
        ),
    ] {
        s.screen_mode = lm;
        check(
            &s,
            e::SettingsPatch {
                screen_mode: Some(em),
                ..Default::default()
            },
        );
    }
    for (w, h) in [(320, 320), (1280, 720), (16384, 16384)] {
        s.resolution.width = w;
        s.resolution.height = h;
        check(
            &s,
            e::SettingsPatch {
                resolution: Some(e::Resolution {
                    width: w,
                    height: h,
                }),
                ..Default::default()
            },
        );
    }
    for v in l::RENDER_SCALE_VALUES {
        s.render_scale = v;
        check(
            &s,
            e::SettingsPatch {
                render_scale: Some(v),
                ..Default::default()
            },
        );
    }
    for v in l::UI_SCALE_VALUES {
        s.ui_scale = v;
        check(
            &s,
            e::SettingsPatch {
                ui_scale: Some(v),
                ..Default::default()
            },
        );
    }
    for v in l::FRAME_RATE_LIMIT_VALUES {
        s.frame_rate_limit = v;
        check(
            &s,
            e::SettingsPatch {
                frame_rate_limit: Some(v),
                ..Default::default()
            },
        );
    }
    for v in l::ANTI_ALIASING_SAMPLE_VALUES {
        s.anti_aliasing_samples = v;
        check(
            &s,
            e::SettingsPatch {
                anti_aliasing_samples: Some(v),
                ..Default::default()
            },
        );
    }
    for v in l::ANISOTROPY_VALUES {
        s.anisotropy = v;
        check(
            &s,
            e::SettingsPatch {
                anisotropy: Some(v),
                ..Default::default()
            },
        );
    }
    for (lt, et) in [
        (l::TextureDetail::Low, e::TextureDetail::Low),
        (l::TextureDetail::Medium, e::TextureDetail::Medium),
        (l::TextureDetail::High, e::TextureDetail::High),
    ] {
        s.texture_detail = lt;
        check(
            &s,
            e::SettingsPatch {
                texture_detail: Some(et),
                ..Default::default()
            },
        );
    }
}

#[test]
fn launcher_booleans_ranges_and_user_enums_match() {
    let mut s = l::LauncherSettings::default();
    for v in [false, true] {
        s.vertical_sync = v;
        s.show_fps = v;
        s.maintain_vertical_fov = v;
        s.native_text = v;
        s.screen_flashes = v;
        s.joystick_enabled = v;
        s.sound_enabled = v;
        s.invert_mouse = v;
        s.auto_center_camera = v;
        s.move_while_casting = v;
        s.auto_quaff = v;
        let bool_patch = e::SettingsPatch {
            vertical_sync: Some(v),
            show_fps: Some(v),
            maintain_vertical_fov: Some(v),
            native_text: Some(v),
            screen_flashes: Some(v),
            joystick_enabled: Some(v),
            sound_enabled: Some(v),
            invert_mouse: Some(v),
            auto_center_camera: Some(v),
            move_while_casting: Some(v),
            auto_quaff: Some(v),
            ..Default::default()
        };
        assert_eq!(bool_patch.plan().unwrap().changes().len(), 11);
        check(&s, bool_patch);
    }
    for v in [0.1, 0.4, 1.0] {
        s.brightness = v;
        check(
            &s,
            e::SettingsPatch {
                brightness: Some(v),
                ..Default::default()
            },
        );
    }
    for v in [0.0, 0.5, 1.0] {
        s.sound_volume = v;
        s.music_volume = v;
        check(
            &s,
            e::SettingsPatch {
                sound_volume: Some(v),
                music_volume: Some(v),
                ..Default::default()
            },
        );
    }
    for v in [0.2, 3.0, 10.0] {
        s.mouse_sensitivity = v;
        check(
            &s,
            e::SettingsPatch {
                mouse_sensitivity: Some(v),
                ..Default::default()
            },
        );
    }
    for (lc, ec) in [
        (l::ControlMode::Classic, e::ControlMode::Classic),
        (l::ControlMode::Modern, e::ControlMode::Modern),
    ] {
        s.control_mode = lc;
        check(
            &s,
            e::SettingsPatch {
                control_mode: Some(ec),
                ..Default::default()
            },
        );
    }
    for (ld, ed) in [
        (l::Difficulty::Easy, e::Difficulty::Easy),
        (l::Difficulty::Medium, e::Difficulty::Medium),
        (l::Difficulty::Hard, e::Difficulty::Hard),
    ] {
        s.difficulty = ld;
        check(
            &s,
            e::SettingsPatch {
                difficulty: Some(ed),
                ..Default::default()
            },
        );
    }
    for (ld, ed) in [
        (l::ObjectDetail::VeryLow, e::ObjectDetail::VeryLow),
        (l::ObjectDetail::Low, e::ObjectDetail::Low),
        (l::ObjectDetail::Medium, e::ObjectDetail::Medium),
        (l::ObjectDetail::High, e::ObjectDetail::High),
        (l::ObjectDetail::VeryHigh, e::ObjectDetail::VeryHigh),
    ] {
        s.object_detail = ld;
        check(
            &s,
            e::SettingsPatch {
                object_detail: Some(ed),
                ..Default::default()
            },
        );
    }
}

#[test]
fn invalid_values_match_launcher_validation() {
    for value in [f64::NAN, f64::INFINITY, 0.09, 1.01] {
        let s = l::LauncherSettings {
            brightness: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                brightness: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [-1, 50, 145] {
        let s = l::LauncherSettings {
            frame_rate_limit: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                frame_rate_limit: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [0.61, f64::NAN] {
        let s = l::LauncherSettings {
            render_scale: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                render_scale: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [f64::NAN, -0.01, 1.01] {
        let s = l::LauncherSettings {
            sound_volume: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                sound_volume: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
        let s = l::LauncherSettings {
            music_volume: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                music_volume: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [f64::NAN, 0.19, 10.01] {
        let s = l::LauncherSettings {
            mouse_sensitivity: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                mouse_sensitivity: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [f64::NAN, 0.6, 2.1] {
        let s = l::LauncherSettings {
            ui_scale: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                ui_scale: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [-1, 1, 3, 8] {
        let s = l::LauncherSettings {
            anti_aliasing_samples: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                anti_aliasing_samples: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for value in [-1, 1, 2, 32] {
        let s = l::LauncherSettings {
            anisotropy: value,
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                anisotropy: Some(value),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
    for (width, height) in [(319, 600), (800, 319), (16385, 600), (800, 16385)] {
        let s = l::LauncherSettings {
            resolution: l::DisplayResolution {
                width,
                height,
                label: String::new(),
            },
            ..Default::default()
        };
        assert!(store::validate_launcher_settings(&s).is_err());
        assert!(
            e::SettingsPatch {
                resolution: Some(e::Resolution { width, height }),
                ..Default::default()
            }
            .plan()
            .is_err()
        );
    }
}

#[test]
fn floating_point_text_precision_difference_is_explicit() {
    let value = 0.12345678901234567;
    let s = l::LauncherSettings {
        brightness: value,
        ..Default::default()
    };
    let mut doc = Document::new();
    store::apply_game(&mut doc, &s);
    let plan = e::SettingsPatch {
        brightness: Some(value),
        ..Default::default()
    }
    .plan()
    .unwrap();
    assert_eq!(
        doc.get("SDLDrv.SDLClient", "Brightness").unwrap(),
        "0.123456789012346"
    );
    assert_eq!(plan.changes()[0].value, value.to_string());
    assert_ne!(
        doc.get("SDLDrv.SDLClient", "Brightness").unwrap(),
        plan.changes()[0].value
    );
}
