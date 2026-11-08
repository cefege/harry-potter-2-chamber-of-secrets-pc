use egui::epaint::{ClippedShape, Shape};
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};
use hp2_game::launcher_ui::LauncherUiState;
use hp2_launcher::model::{DataSource, LaunchAction, LauncherPaths, LauncherSettings};

fn draw_frame(
    ctx: &egui::Context,
    state: &mut LauncherUiState,
    events: Vec<Event>,
) -> (
    Option<hp2_launcher::model::LaunchSelection>,
    Vec<ClippedShape>,
) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 640.0))),
        events,
        ..Default::default()
    };
    let mut result = None;
    let mut output = ctx.run_ui(input, |ui| {
        if result.is_none() {
            result = state.draw(ui).unwrap();
        }
    });
    // These are headless input/store contracts, not painter acceptance.
    output.textures_delta.clear();
    (result, output.shapes)
}
fn frame(
    ctx: &egui::Context,
    state: &mut LauncherUiState,
    events: Vec<Event>,
) -> Option<hp2_launcher::model::LaunchSelection> {
    draw_frame(ctx, state, events).0
}
fn click(
    ctx: &egui::Context,
    state: &mut LauncherUiState,
    point: Pos2,
) -> Option<hp2_launcher::model::LaunchSelection> {
    let _ = frame(
        ctx,
        state,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
        ],
    );
    frame(
        ctx,
        state,
        vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        }],
    )
}
fn visit_shapes(shape: &Shape, visit: &mut impl FnMut(&Shape)) {
    if let Shape::Vec(shapes) = shape {
        for shape in shapes {
            visit_shapes(shape, visit);
        }
    } else {
        visit(shape);
    }
}
fn label_rect(shapes: &[ClippedShape], label: &str) -> Rect {
    let mut found = None;
    for clipped in shapes {
        visit_shapes(&clipped.shape, &mut |shape| {
            if let Shape::Text(text) = shape {
                if text.galley.text() == label {
                    found = Some(text.galley.rect.translate(text.pos.to_vec2()));
                }
            }
        });
    }
    found.unwrap_or_else(|| {
        panic!(
            "visible UI control not found: {label}; layout={}",
            describe_layout(shapes)
        )
    })
}
fn describe_layout(shapes: &[ClippedShape]) -> String {
    let mut output = String::new();
    for clipped in shapes {
        visit_shapes(&clipped.shape, &mut |shape| match shape {
            Shape::Text(text) => output.push_str(&format!(
                "text {:?} at {:?}; ",
                text.galley.text(),
                text.galley.rect.translate(text.pos.to_vec2())
            )),
            Shape::Rect(rect) => output.push_str(&format!("rect {:?}; ", rect.rect)),
            _ => {}
        });
    }
    output
}
fn click_label(
    ctx: &egui::Context,
    state: &mut LauncherUiState,
    label: &str,
) -> Option<hp2_launcher::model::LaunchSelection> {
    let (_, shapes) = draw_frame(ctx, state, vec![]);
    assert!(state.error.is_none(), "visible UI error: {:?}", state.error);
    click(ctx, state, label_rect(&shapes, label).center())
}
fn control_rect(shapes: &[ClippedShape], label: &str, slider: bool) -> Rect {
    let label = label_rect(shapes, label);
    let mut found: Option<Rect> = None;
    for clipped in shapes {
        visit_shapes(&clipped.shape, &mut |shape| {
            if let Shape::Rect(shape) = shape {
                let rect = shape.rect;
                let positioned = if slider {
                    (rect.center().y - label.center().y).abs() < 4.0 && rect.max.x <= label.min.x
                } else {
                    rect.min.y >= label.max.y
                        && rect.min.y - label.max.y < 12.0
                        && (rect.min.x - label.min.x).abs() < 4.0
                };
                let height_matches = rect.height() >= 14.0 && rect.height() <= 32.0;
                if positioned
                    && height_matches
                    && rect.width() >= 30.0
                    && found.is_none_or(|previous| rect.width() > previous.width())
                {
                    found = Some(rect);
                }
            }
        });
    }
    found.unwrap_or_else(|| {
        panic!(
            "painted control geometry next to {label:?}; layout={}",
            describe_layout(shapes)
        )
    })
}
fn choose(ctx: &egui::Context, state: &mut LauncherUiState, label: &str, option: &str) {
    let (_, shapes) = draw_frame(ctx, state, vec![]);
    click(ctx, state, control_rect(&shapes, label, false).center());
    click_label(ctx, state, option);
}
fn set_slider(ctx: &egui::Context, state: &mut LauncherUiState, label: &str, value: f64) {
    let (_, shapes) = draw_frame(ctx, state, vec![]);
    // Edit the slider's visible numeric field, avoiding theme-dependent handle geometry.
    click(ctx, state, control_rect(&shapes, label, true).center());
    let command = Modifiers {
        command: true,
        mac_cmd: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    frame(
        ctx,
        state,
        vec![
            Event::ModifiersChanged(command),
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: command,
            },
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: command,
            },
            Event::ModifiersChanged(Modifiers::default()),
            Event::Text(value.to_string()),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            },
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::default(),
            },
        ],
    );
}
fn fixture() -> (std::path::PathBuf, LauncherPaths) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "hp2-launcher-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let user = root.join("profile");
    let system = root.join("system");
    std::fs::create_dir_all(&user).unwrap();
    std::fs::create_dir_all(&system).unwrap();
    std::fs::write(system.join("Default.ini"), "[Engine.Engine]\n").unwrap();
    std::fs::write(system.join("DefUser.ini"), "").unwrap();
    (
        root,
        LauncherPaths {
            user_root: user.to_string_lossy().into_owned(),
            system_root: system.to_string_lossy().into_owned(),
        },
    )
}

#[test]
fn egui_new_game_publishes_completed_selection() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths,
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        true,
    );
    let ctx = egui::Context::default();
    assert!(state.needs_game_data);
    frame(&ctx, &mut state, vec![]);
    // The Play page's New Game control is presented below the saved-game area.
    let result = click_label(&ctx, &mut state, "New Game").unwrap();
    assert_eq!(result.action, LaunchAction::NewGame);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_setting_edits_persist_through_launcher_store() {
    let (root, paths) = fixture();
    let settings = LauncherSettings {
        native_text: true,
        ..LauncherSettings::default()
    };
    let mut state = LauncherUiState::new(
        settings,
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    // Select Graphics, then toggle Native text. Both interactions travel through
    // egui RawInput and the same transactional launcher settings store as boot.
    click_label(&ctx, &mut state, "Graphics");
    click_label(&ctx, &mut state, "Native text");
    let loaded = hp2_launcher::orchestrate::load_launcher_state(&paths).unwrap();
    assert!(!loaded.settings.native_text);
    assert!(!state.settings.native_text);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_audio_and_gameplay_settings_persist_through_store() {
    let (root, paths) = fixture();
    let settings = LauncherSettings {
        sound_enabled: true,
        invert_mouse: false,
        ..LauncherSettings::default()
    };
    let mut state = LauncherUiState::new(
        settings,
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Audio");
    click_label(&ctx, &mut state, "Sound enabled");
    click_label(&ctx, &mut state, "Gameplay");
    click_label(&ctx, &mut state, "Invert mouse");
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert!(!persisted.sound_enabled);
    assert!(persisted.invert_mouse);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_continue_uses_selected_save_from_import_session_policy() {
    let (root, paths) = fixture();
    let mut save = hp2_launcher::model::SaveRecord::default();
    save.save_index = 17;
    save.display_name = "Hogwarts".into();
    save.size = 4096;
    save.modified_seconds = 1_700_000_000;
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths,
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![save],
        false,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    let result = click_label(&ctx, &mut state, "Continue")
        .unwrap_or_else(|| panic!("Continue did not complete: {:?}", state.error));
    assert_eq!(result.action, LaunchAction::Continue);
    assert!(result.has_save);
    assert_eq!(result.save.save_index, 17);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_game_data_source_selection_uses_import_session_policy() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths,
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        "/data/retail".into(),
        "/data/prototype".into(),
        vec![],
        false,
    );
    assert_eq!(
        state.result_data_sources.as_ref().unwrap().selected,
        DataSource::Retail
    );
    assert_eq!(
        state.result_data_sources.as_ref().unwrap().retail_root,
        "/data/retail"
    );
    assert!(state.result_settings.is_some());
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Game Data");
    choose(&ctx, &mut state, "Data source", "Prototype data");
    assert_eq!(state.data_source, DataSource::Prototype);
    assert!(!state.needs_game_data);
    click_label(&ctx, &mut state, "Play");
    let selection = click_label(&ctx, &mut state, "New Game").unwrap();
    assert_eq!(selection.action, LaunchAction::NewGame);
    assert_eq!(
        state.result_data_sources.as_ref().unwrap().selected,
        DataSource::Prototype
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_quit_action_runs_import_session_completion_policy() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths,
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    let result = click_label(&ctx, &mut state, "Quit").unwrap();
    assert_eq!(result.action, LaunchAction::Quit);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_graphics_and_gameplay_flags_persist_through_the_real_store() {
    let (root, paths) = fixture();
    let settings = LauncherSettings {
        vertical_sync: true,
        show_fps: false,
        maintain_vertical_fov: false,
        native_text: true,
        joystick_enabled: true,
        auto_center_camera: true,
        move_while_casting: true,
        auto_quaff: true,
        screen_flashes: true,
        ..LauncherSettings::default()
    };
    let mut state = LauncherUiState::new(
        settings,
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Graphics");
    for label in [
        "Vertical Sync",
        "Show FPS",
        "Maintain vertical FOV",
        "Native text",
    ] {
        click_label(&ctx, &mut state, label);
    }
    click_label(&ctx, &mut state, "Gameplay");
    for label in [
        "Joystick",
        "Auto-center camera",
        "Move while casting",
        "Auto-quaff potions",
        "Screen flashes",
    ] {
        click_label(&ctx, &mut state, label);
    }
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert!(!persisted.vertical_sync);
    assert!(persisted.show_fps);
    assert!(persisted.maintain_vertical_fov);
    assert!(!persisted.native_text);
    assert!(!persisted.joystick_enabled);
    assert!(!persisted.auto_center_camera);
    assert!(!persisted.move_while_casting);
    assert!(!persisted.auto_quaff);
    assert!(!persisted.screen_flashes);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_invalid_settings_follow_the_completion_error_policy() {
    let (root, paths) = fixture();
    let mut settings = LauncherSettings::default();
    settings.render_scale = 0.0;
    let mut state = LauncherUiState::new(
        settings,
        paths,
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        true,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    assert!(click_label(&ctx, &mut state, "New Game").is_none());
    assert!(state.error.is_some());
    assert!(!state.finished);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_screen_mode_and_resolution_controls_persist_selected_values() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    state.modes = vec![
        hp2_launcher::model::DisplayResolution {
            width: 800,
            height: 600,
            label: "800 × 600".into(),
        },
        hp2_launcher::model::DisplayResolution {
            width: 1024,
            height: 768,
            label: "1024 × 768".into(),
        },
    ];
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Graphics");
    choose(&ctx, &mut state, "Screen mode", "Fullscreen");
    choose(&ctx, &mut state, "Resolution", "1024 × 768");
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert_eq!(
        persisted.screen_mode,
        hp2_launcher::model::ScreenMode::Fullscreen
    );
    assert_eq!(
        (persisted.resolution.width, persisted.resolution.height),
        (1024, 768)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_audio_sliders_persist_through_the_real_store() {
    let (root, paths) = fixture();
    let settings = LauncherSettings {
        sound_enabled: true,
        ..LauncherSettings::default()
    };
    let mut state = LauncherUiState::new(
        settings,
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        true,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Audio");
    set_slider(&ctx, &mut state, "Effects volume", 0.25);
    set_slider(&ctx, &mut state, "Music volume", 0.75);
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert_eq!(persisted.sound_volume, 0.25);
    assert_eq!(persisted.music_volume, 0.75);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_gameplay_combos_persist_through_the_real_store() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        true,
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut state, vec![]);
    click_label(&ctx, &mut state, "Gameplay");
    choose(&ctx, &mut state, "Controls mode", "Modern");
    choose(&ctx, &mut state, "Difficulty", "Hard");
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert_eq!(
        persisted.control_mode,
        hp2_launcher::model::ControlMode::Modern
    );
    assert_eq!(persisted.difficulty, hp2_launcher::model::Difficulty::Hard);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn egui_remaining_graphics_and_sensitivity_controls_persist_selected_values() {
    let (root, paths) = fixture();
    let mut state = LauncherUiState::new(
        LauncherSettings::default(),
        paths.clone(),
        root.to_string_lossy().into_owned(),
        DataSource::Retail,
        String::new(),
        String::new(),
        vec![],
        false,
    );
    let ctx = egui::Context::default();
    click_label(&ctx, &mut state, "Graphics");
    choose(&ctx, &mut state, "Render scale", "67%");
    choose(&ctx, &mut state, "UI scale", "125%");
    choose(&ctx, &mut state, "FPS cap", "120");
    choose(&ctx, &mut state, "Anti-aliasing", "2x");
    choose(&ctx, &mut state, "Anisotropy", "8x");
    set_slider(&ctx, &mut state, "Brightness", 0.42);
    choose(&ctx, &mut state, "Texture detail", "Low");
    choose(&ctx, &mut state, "Object detail", "Very high");
    click_label(&ctx, &mut state, "Gameplay");
    set_slider(&ctx, &mut state, "Mouse sensitivity", 3.25);
    let persisted = hp2_launcher::orchestrate::load_launcher_state(&paths)
        .unwrap()
        .settings;
    assert_eq!(persisted.render_scale, 0.67);
    assert_eq!(persisted.ui_scale, 1.25);
    assert_eq!(persisted.frame_rate_limit, 120);
    assert_eq!(persisted.anti_aliasing_samples, 2);
    assert_eq!(persisted.anisotropy, 8);
    assert!((persisted.brightness - 0.42).abs() < 0.000001);
    assert_eq!(
        persisted.texture_detail,
        hp2_launcher::model::TextureDetail::Low
    );
    assert_eq!(
        persisted.object_detail,
        hp2_launcher::model::ObjectDetail::VeryHigh
    );
    assert_eq!(persisted.mouse_sensitivity, 3.25);
    std::fs::remove_dir_all(root).unwrap();
}
