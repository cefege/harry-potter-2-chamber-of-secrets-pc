//! Port of `Tests/LauncherTests.cpp` driving hp-app public APIs under
//! `HP2_LAUNCHER_TESTING=1`.
//!
//! Every oracle case group maps 1:1 onto a `#[test]` here unless the group
//! depends on engine-runtime surfaces that live outside this slice; those
//! fragments carry a `DEFERRED` comment with the reason inline.

// The oracle builds selections and settings by mutating a default instance,
// mirroring the C++ fixture style; keep that shape verbatim.
#![allow(clippy::field_reassign_with_default)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use hp_app::atomic_file::set_publish_failure_for_testing;
use hp_app::migration::{migrate_legacy_settings, LegacyRow, MigrationChange};
use hp_app::policy::{
    build_selected_command, deserialize_launch_selection, serialize_launch_selection,
    should_run_launcher, LaunchAction, LaunchSelection, SelectionField,
};
use hp_app::settings;
use hp_app::settings_model::{
    AA_SAMPLE_VALUES, ANISOTROPY_VALUES, ControlMode, Difficulty, FRAME_RATE_LIMITS,
    ObjectDetail, RENDER_SCALES, Resolution, ScreenMode, Settings, TextureDetail, UI_SCALES,
};
use hp_app::store::{self, DataSource, DataSourceConfiguration};

// ------------------------------------------------------------- harness

static ROOT_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The native oracle drives one process single-threadedly; the Rust suite
/// runs tests in parallel, so every test that publishes launcher files
/// shares this lock. Without it a concurrent publication would consume a
/// fault-injection window reserved by another test.
static IO_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn io_guard() -> std::sync::MutexGuard<'static, ()> {
    std::sync::Mutex::lock(&IO_LOCK).expect("io lock poisoned")
}

struct TemporaryRoots {
    root: PathBuf,
}

impl TemporaryRoots {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "hp2-launcher-oracle-{}-{}",
            std::process::id(),
            ROOT_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("System")).expect("system root");
        std::fs::create_dir_all(root.join("User")).expect("user root");
        Self { root }
    }

    fn system(&self) -> PathBuf {
        self.root.join("System")
    }

    fn user(&self) -> PathBuf {
        self.root.join("User")
    }
}

impl Drop for TemporaryRoots {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Fault-injection tests require the oracle gate; without it they skip so
/// a bare `cargo test` listing stays green while the gate invocation runs
/// everything.
fn gate_enabled() -> bool {
    matches!(std::env::var("HP2_LAUNCHER_TESTING").as_deref(), Ok("1"))
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent directory");
    }
    std::fs::write(path, bytes).expect("test file write");
}

fn write_text(path: &Path, text: &str) {
    write_bytes(path, text.as_bytes());
}

fn read_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {:?}: {error}", path))
}

fn read_text(path: &Path) -> String {
    String::from_utf8(read_bytes(path)).expect("utf-8 test file")
}

fn unix_mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("stat")
        .permissions()
        .mode()
        & 0o7777
}

fn set_unix_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(mode),
    )
    .expect("chmod");
}

fn symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("symlink");
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

fn only_crlf(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .enumerate()
        .all(|(index, &byte)| byte != b'\n' || (index > 0 && bytes[index - 1] == b'\r'))
}

fn utf16_ascii(text: &str, little: bool) -> Vec<u8> {
    let mut bytes = vec![if little { 0xFF } else { 0xFE }, if little { 0xFE } else { 0xFF }];
    for character in text.bytes() {
        bytes.push(if little { character } else { 0 });
        bytes.push(if little { 0 } else { character });
    }
    bytes
}

fn decode_utf16_ascii(bytes: &[u8], little: bool) -> String {
    bytes[2..]
        .chunks_exact(2)
        .map(|pair| if little { pair[0] } else { pair[1] } as char)
        .collect()
}

fn default_templates(roots: &TemporaryRoots) {
    write_text(
        &roots.system().join("Default.ini"),
        "; immutable game default\n\
         [Engine.Engine]\n\
         ViewportManager=WinDrv.WindowsClient\n\
         GameRenderDevice=D3DDrv.D3DRenderDevice\n\
         [Engine.GameEngine]\n\
         UseSound=True\n\
         FrameRateLimit=60.000000\n\
         [SDLDrv.SDLClient]\n\
         WindowedViewportX=800\nWindowedViewportY=600\n\
         FullscreenViewportX=800\nFullscreenViewportY=600\n\
         StartupFullscreen=False\nBrightness=0.4\nUseJoystick=True\n\
         ShowFPS=False\nMaintainVerticalFOV=True\nNativeText=True\nScreenFlashes=True\n\
         [XOpenGLDrv.XOpenGLRenderDevice]\n\
         UseAA=False\nNumAASamples=0\nMaxAnisotropy=4\n\
         [ALAudio.ALAudioSubsystem]\nMusicVolume=0.53\nSoundVolume=0.9\n",
    );
    write_text(
        &roots.system().join("DefUser.ini"),
        "; immutable user default\n\
         [Engine.PlayerPawn]\nbModernThirdPersonControls=False\nDifficulty=DifficultyEasy\n\
         [HGame.Harry]\nbAutoCenterCamera=True\nbMoveWhileCasting=True\nbAutoQuaff=True\n",
    );
}

fn distinct_settings() -> Settings {
    let mut settings = Settings::default();
    settings.screen_mode = ScreenMode::BorderlessDesktop;
    settings.resolution = Resolution {
        width: 1440,
        height: 900,
        label: "1440 x 900".to_string(),
    };
    settings.vertical_sync = true;
    settings.render_scale = 0.85;
    settings.ui_scale = 1.75;
    settings.frame_rate_limit = 144;
    settings.show_fps = true;
    settings.maintain_vertical_fov = false;
    settings.native_text = false;
    settings.screen_flashes = false;
    settings.anti_aliasing_samples = 4;
    settings.anisotropy = 16;
    settings.brightness = 0.75;
    settings.texture_detail = TextureDetail::Low;
    settings.object_detail = ObjectDetail::VeryHigh;
    settings.sound_enabled = false;
    settings.sound_volume = 0.25;
    settings.music_volume = 0.5;
    settings.mouse_sensitivity = 7.25;
    settings.invert_mouse = true;
    settings.control_mode = ControlMode::Modern;
    settings.auto_center_camera = false;
    settings.move_while_casting = false;
    settings.auto_quaff = false;
    settings.difficulty = Difficulty::Hard;
    settings.joystick_enabled = false;
    settings
}

fn continue_selection(index: i32, slot: i32) -> LaunchSelection {
    LaunchSelection {
        action: LaunchAction::Continue,
        has_save: true,
        save: hp_app::policy::SaveCoordinate {
            save_index: index,
            uses_slot_directory: slot >= 0,
            slot,
        },
    }
}

fn same_selection_core(left: &LaunchSelection, right: &LaunchSelection) -> bool {
    left.action == right.action
        && left.has_save == right.has_save
        && left.save.save_index == right.save.save_index
        && left.save.uses_slot_directory == right.save.uses_slot_directory
        && left.save.slot == right.save.slot
}

fn field(key: &'static str, value: &str) -> SelectionField {
    SelectionField {
        key,
        value: value.to_string(),
    }
}

fn row(section: &str, key: &str, value: &str) -> LegacyRow {
    LegacyRow::new(section, key, value)
}

fn change(section: &str, key: &str, previous: &str, next: &str, reason: &str) -> MigrationChange {
    MigrationChange {
        section: section.to_string(),
        key: key.to_string(),
        previous_value: previous.to_string(),
        new_value: next.to_string(),
        reason: reason.to_string().leak() as &'static str,
    }
}

/// Creates a plausible installed data root with sentinel files.
fn create_data_root(roots: &TemporaryRoots, name: &str, sentinel: &str) -> PathBuf {
    let data_root = roots.root.join(name);
    std::fs::create_dir_all(data_root.join("System")).expect("System");
    std::fs::create_dir_all(data_root.join("Maps")).expect("Maps");
    std::fs::create_dir_all(data_root.join("Textures")).expect("Textures");
    write_text(&data_root.join("System").join("Default.ini"), &format!("[Sentinel]\nName={sentinel}\n"));
    write_text(&data_root.join("Maps").join("PrivetDr.unr"), sentinel);
    write_text(&data_root.join("Maps").join("Startup.unr"), sentinel);
    write_text(&data_root.join("Textures").join("Shared.utx"), sentinel);
    data_root
}

// -------------------------------------------------- TestPolicyClassification

fn runs_launcher(arguments: &[&str]) -> bool {
    let owned: Vec<String> = arguments.iter().map(|argument| argument.to_string()).collect();
    should_run_launcher(&owned)
}

#[test]
fn policy_classification() {
    assert!(runs_launcher(&[]), "empty command line opens launcher");
    assert!(runs_launcher(&["-datadir=/Retail"]), "-datadir alone opens launcher");
    assert!(
        runs_launcher(&["-DaTaDiR=/Retail", "-unknown"]),
        "datadir is case-insensitive and unknown options stay interactive"
    );
    assert!(
        runs_launcher(&["--datadir=/Retail"]),
        "double-dash datadir remains an unrelated interactive option"
    );
    assert!(runs_launcher(&["-unknown"]), "unrelated option stays interactive");

    assert!(!runs_launcher(&["PrivetDr.unr"]), "explicit map bypasses launcher");
    assert!(!runs_launcher(&["unreal://127.0.0.1/Map"]), "explicit URL bypasses launcher");
    assert!(
        !runs_launcher(&["-datadir=/Retail", "Startup.unr"]),
        "datadir does not hide explicit map"
    );

    const NAMES: [&str; 12] = [
        "LOAD", "NOFRONTEND", "TESTTICKS", "TESTRENDEV", "TESTNATIVETEXT", "SERVER", "REPLAY",
        "RECORD", "BENCHMARK", "COMMANDLET", "MAKE", "EXEC",
    ];
    for name in NAMES {
        let upper = format!("-{name}=value");
        assert!(!runs_launcher(&[&upper]), "{name} bypasses launcher");
        let mut mixed = name.to_string();
        for index in (0..mixed.len()).step_by(2) {
            mixed.replace_range(index..index + 1, &mixed[index..index + 1].to_lowercase());
        }
        let doubled = format!("--{mixed}");
        assert!(
            !runs_launcher(&[&doubled]),
            "{name} matching is case-insensitive with repeated dashes"
        );
    }
    assert!(
        !runs_launcher(&["-datadir=/Retail", "-eXeC=Boot.txt"]),
        "datadir-only filtering still detects bypass option"
    );
}

// ---------------------------------------------------- TestSelectedCommands

#[test]
fn selected_commands() {
    let mut selection = LaunchSelection::default();
    assert_eq!(
        build_selected_command(&selection),
        Ok(String::new()),
        "Quit produces an empty command"
    );
    selection.action = LaunchAction::NewGame;
    assert_eq!(
        build_selected_command(&selection),
        Ok("PrivetDr.unr".to_string()),
        "New Game selects PrivetDr.unr"
    );
    selection.action = LaunchAction::Continue;
    selection.has_save = true;
    selection.save.save_index = 12;
    selection.save.uses_slot_directory = false;
    selection.save.slot = -1;
    assert_eq!(
        build_selected_command(&selection),
        Ok("Startup.unr -LOAD=12".to_string()),
        "flat save command is exact"
    );
    selection.save.uses_slot_directory = true;
    selection.save.slot = 3;
    assert_eq!(
        build_selected_command(&selection),
        Ok("Startup.unr -LOAD=12 -SAVESLOT=3".to_string()),
        "slotted save command is exact"
    );
    selection.has_save = false;
    assert!(
        build_selected_command(&selection).is_err(),
        "Continue rejects absent save"
    );
    selection.has_save = true;
    selection.save.save_index = -1;
    assert!(
        build_selected_command(&selection).is_err(),
        "Continue rejects negative save index"
    );
    selection.save.save_index = 1;
    selection.save.slot = -1;
    assert!(
        build_selected_command(&selection).is_err(),
        "slotted Continue rejects negative slot"
    );
    selection.action = LaunchAction::Error;
    assert!(
        build_selected_command(&selection).is_err(),
        "error selection does not launch"
    );
}

// ------------------------------------------------ TestValidationBoundaries

fn valid(settings: &Settings) -> bool {
    settings::validate(settings).is_ok()
}

#[test]
fn validation_boundaries() {
    let mut settings = Settings::default();
    settings.resolution = Resolution {
        width: 320,
        height: 320,
        label: String::new(),
    };
    settings.brightness = 0.1;
    settings.mouse_sensitivity = 0.2;
    settings.sound_volume = 0.0;
    settings.music_volume = 1.0;
    settings.frame_rate_limit = 0;
    assert!(valid(&settings), "all lower validation boundaries are accepted");
    settings.resolution = Resolution {
        width: 16384,
        height: 16384,
        label: String::new(),
    };
    settings.brightness = 1.0;
    settings.mouse_sensitivity = 10.0;
    settings.sound_volume = 1.0;
    settings.music_volume = 0.0;
    settings.frame_rate_limit = 144;
    assert!(valid(&settings), "all upper validation boundaries are accepted");

    // NOTE: the oracle's out-of-range enum casts (screenMode 99, texture
    // detail -1, ...) are unrepresentable in Rust's closed enums; the type
    // system makes those rejection cases satisfied by construction.
    let mut bad = settings.clone();
    bad.resolution.width = 319;
    assert!(!valid(&bad), "width below boundary is rejected");
    bad = settings.clone();
    bad.resolution.height = 16385;
    assert!(!valid(&bad), "height above boundary is rejected");
    bad = settings.clone();
    bad.brightness = 0.099;
    assert!(!valid(&bad), "brightness below boundary is rejected");
    bad = settings.clone();
    bad.brightness = f64::NAN;
    assert!(!valid(&bad), "NaN brightness is rejected");
    bad = settings.clone();
    bad.mouse_sensitivity = 10.01;
    assert!(!valid(&bad), "sensitivity above boundary is rejected");
    bad = settings.clone();
    bad.mouse_sensitivity = f64::INFINITY;
    assert!(!valid(&bad), "infinite sensitivity is rejected");
    bad = settings.clone();
    bad.sound_volume = -0.01;
    assert!(!valid(&bad), "negative sound volume is rejected");
    bad = settings.clone();
    bad.music_volume = 1.01;
    assert!(!valid(&bad), "music volume above one is rejected");
    bad = settings.clone();
    bad.music_volume = f64::NAN;
    assert!(!valid(&bad), "NaN music volume is rejected");
    for cap in FRAME_RATE_LIMITS {
        let mut good = settings.clone();
        good.frame_rate_limit = cap;
        assert!(valid(&good), "allowed frame cap is accepted");
    }
    bad = settings.clone();
    bad.frame_rate_limit = 59;
    assert!(!valid(&bad), "unknown frame cap is rejected");
    for samples in AA_SAMPLE_VALUES {
        let mut good = settings.clone();
        good.anti_aliasing_samples = samples;
        assert!(valid(&good), "allowed MSAA sample count is accepted");
    }
    for anisotropy in ANISOTROPY_VALUES {
        let mut good = settings.clone();
        good.anisotropy = anisotropy;
        assert!(valid(&good), "allowed anisotropy value is accepted");
    }
    bad = settings.clone();
    bad.anti_aliasing_samples = 1;
    assert!(!valid(&bad), "unknown MSAA sample count is rejected");
    bad = settings.clone();
    bad.anti_aliasing_samples = 8;
    assert!(!valid(&bad), "unsupported MSAA sample count is rejected");
    bad = settings.clone();
    bad.anisotropy = 2;
    assert!(!valid(&bad), "unknown anisotropy value is rejected");
    bad = settings.clone();
    bad.anisotropy = 32;
    assert!(!valid(&bad), "unsupported anisotropy value is rejected");
}

// ------------------------------------------ TestScaleDefaultsAndValidation

#[test]
fn scale_defaults_and_validation() {
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let loaded =
        settings::load(&roots.system(), &roots.user()).expect("scale defaults load when keys are absent");
    assert_eq!(loaded.render_scale, 1.0, "missing RenderScale defaults to 1.0");
    assert_eq!(loaded.ui_scale, 1.0, "missing UIScale defaults to 1.0");
    assert!(loaded.native_text, "available native renderer is enabled by default");
    assert!(!loaded.show_fps, "FPS counter is disabled by default");
    assert_eq!(loaded.control_mode, ControlMode::Classic, "Classic controls are the default");

    let mut settings = Settings::default();
    for value in RENDER_SCALES {
        settings.render_scale = value;
        assert!(valid(&settings), "accepted render scale validates");
    }
    for value in UI_SCALES {
        settings.render_scale = 1.0;
        settings.ui_scale = value;
        assert!(valid(&settings), "accepted UI scale validates");
    }

    for value in [-1.0, 0.0, 0.49, 0.51, 0.66, 0.68, 0.74, 0.76, 0.84, 0.86, 1.01, f64::INFINITY, f64::NAN] {
        let mut bad = Settings::default();
        bad.render_scale = value;
        assert!(!valid(&bad), "render scale outside the discrete choices is rejected");
    }
    for value in [
        -1.0, 0.0, 0.74, 0.76, 0.99, 1.01, 1.24, 1.26, 1.49, 1.51, 1.74, 1.76, 1.99, 2.01,
        f64::INFINITY, f64::NAN,
    ] {
        let mut bad = Settings::default();
        bad.ui_scale = value;
        assert!(!valid(&bad), "UI scale outside the discrete choices is rejected");
    }
}

// ------------------------------------------ TestConfigRoundTripAndBackups

#[test]
fn config_round_trip_and_backups() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let original_game = "; Preserve This Comment\r\n\
        [engine.engine]\r\nCustomCase=KeepMe\r\n\
        [ENGINE.GAMEENGINE]\r\nFrameRateLimit=30\r\nfrAMeRateLimit = 60 ; final value\r\nUseSound=True\r\n\
        malformed line without equals\r\n\
        [SDLDrv.SDLClient]\r\nBrightness=0.4\r\nUIScale=1.0\r\nUIScale = 1.25 ; final UI scale\r\n\
        ShowFPS=False\r\nMaintainVerticalFOV=True\r\nNativeText=True\r\nScreenFlashes=True\r\n\
        [XOpenGLDrv.XOpenGLRenderDevice]\r\nDriverNote=PreserveRendererSetting\r\n\
        UseAA=Off\r\nNumAASamples=0\r\nMaxAnisotropy=4.000000\r\n\
        RenderScale=0.67\r\nRenderScale = 0.75 ; final render scale\r\n\
        [Unrelated.Section]\r\nMiXeDKey=MiXeDValue\r\n";
    let original_user = "# user comment\n[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\nDifficulty = DifficultyMedium ; final\n\
        [HGame.Harry]\nbAutoQuaff=True\n[Other]\nCaseKey=CaseValue\n";
    let game_path = roots.user().join("Game.ini");
    let user_path = roots.user().join("User.ini");
    write_text(&game_path, original_game);
    write_text(&user_path, original_user);
    set_unix_mode(&game_path, 0o640);
    set_unix_mode(&user_path, 0o604);

    let selected = distinct_settings();
    if gate_enabled() {
        set_publish_failure_for_testing(2);
        let error = settings::commit(&roots.system(), &roots.user(), &selected)
            .expect_err("second-file publication failure is reported");
        assert!(
            error.message().contains("Injected"),
            "publication failure preserves its diagnostic: {}",
            error.message()
        );
        set_publish_failure_for_testing(-1);
        assert_eq!(
            read_bytes(&game_path),
            original_game.as_bytes(),
            "failed User.ini publication rolls Game.ini back exactly"
        );
        assert_eq!(
            read_bytes(&user_path),
            original_user.as_bytes(),
            "failed User.ini publication rolls User.ini back exactly"
        );
    }

    settings::commit(&roots.system(), &roots.user(), &selected)
        .expect("settings commit succeeds");
    let game = read_text(&game_path);
    let user = read_text(&user_path);
    assert!(game.contains("; Preserve This Comment"), "game comment is preserved");
    assert!(game.contains("CustomCase=KeepMe"), "unrelated game key is preserved");
    assert!(game.contains("MiXeDKey=MiXeDValue"), "unrelated key case is preserved");
    assert!(game.contains("malformed line without equals"), "malformed line is preserved");
    assert!(
        game.contains("FrameRateLimit=30"),
        "earlier repeated owned key remains untouched"
    );
    assert!(
        game.contains("frAMeRateLimit = 144 ; final value"),
        "only final repeated owned key value is replaced while case/comment remain"
    );
    assert!(
        user.contains("Difficulty=DifficultyEasy"),
        "earlier repeated user key remains untouched"
    );
    assert!(
        user.contains("Difficulty = DifficultyHard ; final"),
        "final user key is replaced in place"
    );
    assert!(
        user.contains("bModernThirdPersonControls=True"),
        "Modern control mode persists under Engine.PlayerPawn"
    );
    assert!(
        game.contains("UIScale=1.0"),
        "earlier repeated UI scale remains untouched"
    );
    assert!(
        game.contains("UIScale = 1.75 ; final UI scale"),
        "final UI scale is replaced in place"
    );
    assert!(
        game.contains("RenderScale=0.67"),
        "earlier repeated render scale remains untouched"
    );
    assert!(
        game.contains("RenderScale = 0.85 ; final render scale"),
        "final render scale is replaced in place"
    );
    assert!(
        game.contains("DriverNote=PreserveRendererSetting"),
        "unrelated renderer setting is preserved"
    );
    assert!(
        game.contains("\r\nShowFPS=True\r\n")
            && game.contains("MaintainVerticalFOV=False")
            && game.contains("NativeText=False")
            && game.contains("ScreenFlashes=False"),
        "FPS, widescreen, text rendering, and flash preferences persist under SDLDrv.SDLClient"
    );
    assert!(
        game.contains("UseAA=True")
            && game.contains("NumAASamples=4")
            && game.contains("MaxAnisotropy=16"),
        "MSAA and anisotropy persist under XOpenGLDrv"
    );
    assert!(
        !user.contains("ShowFPS")
            && !user.contains("MaintainVerticalFOV")
            && !user.contains("ScreenFlashes")
            && !user.contains("NumAASamples"),
        "modern video settings do not leak into User.ini"
    );
    assert!(
        only_crlf(game.as_bytes()),
        "Game.ini CRLF newline style is preserved"
    );
    assert!(!user.contains('\r'), "User.ini LF newline style is preserved");
    assert_eq!(
        read_bytes(&roots.user().join("Game.ini.bak")),
        original_game.as_bytes(),
        "first Game.ini backup is exact"
    );
    assert_eq!(
        read_bytes(&roots.user().join("User.ini.bak")),
        original_user.as_bytes(),
        "first User.ini backup is exact"
    );
    assert_eq!(unix_mode(&game_path), 0o640, "Game.ini destination mode is retained");
    assert_eq!(unix_mode(&user_path), 0o604, "User.ini destination mode is retained");

    let loaded = settings::load(&roots.system(), &roots.user()).expect("committed settings reload");
    assert_eq!(loaded.screen_mode, selected.screen_mode, "screen mode round-trips");
    assert_eq!(
        (loaded.resolution.width, loaded.resolution.height),
        (1440, 900),
        "resolution round-trips"
    );
    assert!(
        loaded.vertical_sync && loaded.frame_rate_limit == 144,
        "VSync and frame cap round-trip"
    );
    assert!(loaded.show_fps, "FPS counter preference round-trips");
    assert!(
        !loaded.maintain_vertical_fov
            && !loaded.native_text
            && loaded.anti_aliasing_samples == 4
            && loaded.anisotropy == 16,
        "widescreen, text rendering, MSAA, and anisotropy round-trip"
    );
    assert_eq!(
        (loaded.render_scale, loaded.ui_scale),
        (0.85, 1.75),
        "render and UI scales round-trip"
    );
    assert!(
        loaded.texture_detail == TextureDetail::Low
            && loaded.object_detail == ObjectDetail::VeryHigh,
        "detail levels round-trip"
    );
    assert!(
        !loaded.sound_enabled
            && loaded.sound_volume == 0.25
            && loaded.music_volume == 0.5,
        "audio settings round-trip"
    );
    assert!(
        loaded.mouse_sensitivity == 7.25 && loaded.invert_mouse,
        "mouse settings round-trip"
    );
    assert_eq!(loaded.control_mode, ControlMode::Modern, "Modern control mode round-trips");
    assert_eq!(loaded.difficulty, Difficulty::Hard, "difficulty round-trips");
    assert!(
        !loaded.auto_center_camera && !loaded.move_while_casting && !loaded.auto_quaff,
        "gameplay toggles round-trip"
    );
    assert!(!loaded.screen_flashes, "screen flash preference round-trips");

    let mut second = selected.clone();
    second.frame_rate_limit = 30;
    second.control_mode = ControlMode::Classic;
    settings::commit(&roots.system(), &roots.user(), &second).expect("second settings commit succeeds");
    assert!(
        read_text(&user_path).contains("bModernThirdPersonControls=False"),
        "Classic control mode rewrites the engine preference to False"
    );
    assert_eq!(
        read_bytes(&roots.user().join("Game.ini.bak")),
        original_game.as_bytes(),
        "Game.ini backup is never overwritten"
    );
    assert_eq!(
        read_bytes(&roots.user().join("User.ini.bak")),
        original_user.as_bytes(),
        "User.ini backup is never overwritten"
    );
}

// ---------------------------------------------------- TestEveryScaleRoundTrip

#[test]
fn every_scale_round_trip() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    for value in RENDER_SCALES {
        let mut settings_model = Settings::default();
        settings_model.render_scale = value;
        settings::commit(&roots.system(), &roots.user(), &settings_model)
            .expect("accepted render scale commits");
        let loaded = settings::load(&roots.system(), &roots.user())
            .expect("accepted render scale reloads");
        assert_eq!(
            loaded.render_scale, value,
            "accepted render scale round-trips exactly"
        );
    }
    for value in UI_SCALES {
        let mut settings_model = Settings::default();
        settings_model.ui_scale = value;
        settings::commit(&roots.system(), &roots.user(), &settings_model)
            .expect("accepted UI scale commits");
        let loaded =
            settings::load(&roots.system(), &roots.user()).expect("accepted UI scale reloads");
        assert_eq!(loaded.ui_scale, value, "accepted UI scale round-trips exactly");
    }
}

// --------------------------------------------------- TestModernOptionRoundTrips

#[test]
fn modern_option_round_trips() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);

    for samples in AA_SAMPLE_VALUES {
        let mut settings_model = Settings::default();
        settings_model.anti_aliasing_samples = samples;
        settings::commit(&roots.system(), &roots.user(), &settings_model)
            .expect("accepted MSAA setting commits");
        let loaded = settings::load(&roots.system(), &roots.user())
            .expect("accepted MSAA setting reloads");
        assert_eq!(
            loaded.anti_aliasing_samples, samples,
            "accepted MSAA setting round-trips exactly"
        );
    }
    for anisotropy in ANISOTROPY_VALUES {
        let mut settings_model = Settings::default();
        settings_model.anisotropy = anisotropy;
        settings::commit(&roots.system(), &roots.user(), &settings_model)
            .expect("accepted anisotropy setting commits");
        let loaded = settings::load(&roots.system(), &roots.user())
            .expect("accepted anisotropy setting reloads");
        assert_eq!(
            loaded.anisotropy, anisotropy,
            "accepted anisotropy setting round-trips exactly"
        );
    }
    for enabled in [false, true] {
        let mut settings_model = Settings::default();
        settings_model.show_fps = enabled;
        settings_model.maintain_vertical_fov = enabled;
        settings_model.screen_flashes = enabled;
        settings_model.native_text = enabled;
        settings::commit(&roots.system(), &roots.user(), &settings_model)
            .expect("modern boolean settings commit");
        let loaded =
            settings::load(&roots.system(), &roots.user()).expect("modern boolean settings reload");
        assert_eq!(
            loaded.maintain_vertical_fov, enabled,
            "widescreen preference round-trips exactly"
        );
        assert_eq!(loaded.show_fps, enabled, "FPS counter preference round-trips exactly");
        assert_eq!(
            loaded.screen_flashes, enabled,
            "screen flash preference round-trips exactly"
        );
        assert_eq!(
            loaded.native_text, enabled,
            "native text preference round-trips exactly"
        );
    }

    write_text(
        &roots.user().join("Game.ini"),
        "[SDLDrv.SDLClient]\nShowFPS=True\nMaintainVerticalFOV=True\nNativeText=True\nScreenFlashes=True\n\
         [XOpenGLDrv.XOpenGLRenderDevice]\n\
         UseAA=False\nNumAASamples=4\nMaxAnisotropy=8.000000\n",
    );
    let loaded = settings::load(&roots.system(), &roots.user())
        .expect("runtime-formatted modern options load");
    assert_eq!(
        loaded.anti_aliasing_samples, 0,
        "disabled UseAA takes precedence over a stale sample count"
    );
    assert_eq!(
        loaded.anisotropy, 8,
        "floating-point renderer anisotropy text loads as a discrete value"
    );
    assert!(
        loaded.native_text,
        "enabled native text preference loads from Game.ini"
    );
    assert!(loaded.show_fps, "enabled FPS counter preference loads from Game.ini");

    write_text(
        &roots.user().join("Game.ini"),
        "[SDLDrv.SDLClient]\nMaintainVerticalFOV=True\nScreenFlashes=True\n",
    );
    let loaded = settings::load(&roots.system(), &roots.user())
        .expect("modern options load without a NativeText key");
    assert!(
        loaded.native_text,
        "missing NativeText uses the available renderer default"
    );
    assert!(
        !loaded.show_fps,
        "missing ShowFPS uses the disabled model default"
    );
}

// ------------------------------- TestReadOnlyCancellationAndMaterialization

#[test]
fn read_only_cancellation_and_materialization() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let default_before = read_bytes(&roots.system().join("Default.ini"));
    let user_default_before = read_bytes(&roots.system().join("DefUser.ini"));
    let state = settings::load(&roots.system(), &roots.user())
        .expect("load from immutable defaults succeeds");
    let quit = LaunchSelection::default();
    assert!(
        build_selected_command(&quit).is_ok(),
        "cancel selection is accepted"
    );
    assert!(
        !roots.user().join("Game.ini").exists() && !roots.user().join("User.ini").exists(),
        "load/cancel path creates no writable INIs"
    );
    assert!(
        read_bytes(&roots.system().join("Default.ini")) == default_before
            && read_bytes(&roots.system().join("DefUser.ini")) == user_default_before,
        "load/cancel path never changes immutable defaults"
    );

    if gate_enabled() {
        set_publish_failure_for_testing(2);
        assert!(
            settings::commit(&roots.system(), &roots.user(), &state).is_err(),
            "failed first-write publication is reported"
        );
        set_publish_failure_for_testing(-1);
        assert!(
            !roots.user().join("Game.ini").exists() && !roots.user().join("User.ini").exists(),
            "failed first-write publication restores both files to absence"
        );
    }

    settings::commit(&roots.system(), &roots.user(), &state)
        .expect("first commit materializes missing writable INIs");
    assert!(
        roots.user().join("Game.ini").exists() && roots.user().join("User.ini").exists(),
        "commit seeds both writable INIs"
    );
    assert!(
        !roots.user().join("Game.ini.bak").exists() && !roots.user().join("User.ini.bak").exists(),
        "materializing absent INIs does not invent backups"
    );
    assert_eq!(unix_mode(&roots.user().join("Game.ini")), 0o600, "new writable INI is mode 0600");
}

// ------------------------------------------------------- TestMalformedRecovery

#[test]
fn malformed_recovery() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let malformed_game = "; remains intact\n[Engine.GameEngine\nFrameRateLimit=120\n\
        not=a truncation marker\n[Engine.GameEngine]\nFrameRateLimit=invalid\n\
        [SDLDrv.SDLClient]\nBrightness=nan\nUIScale=1.1\nWindowedViewportX=12\nWindowedViewportY=999999\n\
        ShowFPS=perhaps\nMaintainVerticalFOV=maybe\nNativeText=not-a-boolean\nScreenFlashes=sometimes\n\
        [XOpenGLDrv.XOpenGLRenderDevice]\nRenderScale=0.8\nUseAA=True\nNumAASamples=8\nMaxAnisotropy=2\n";
    let malformed_user = "[Engine.PlayerPawn\nDifficulty=DifficultyHard\n\
        [Engine.PlayerPawn]\nDifficulty=Unknown\nMouseSensitivity=not-a-number\nbModernThirdPersonControls=maybe\n\
        [HGame.Harry]\nbAutoQuaff=maybe\n";
    write_text(&roots.user().join("Game.ini"), malformed_game);
    write_text(&roots.user().join("User.ini"), malformed_user);
    let state = settings::load(&roots.system(), &roots.user())
        .expect("malformed INIs recover without truncation failure");
    let defaults = Settings::default();
    assert_eq!(
        state.frame_rate_limit, defaults.frame_rate_limit,
        "invalid final frame cap falls back to model default"
    );
    assert_eq!(
        state.brightness, defaults.brightness,
        "invalid brightness falls back to model default"
    );
    assert_eq!(
        (state.resolution.width, state.resolution.height),
        (defaults.resolution.width, defaults.resolution.height),
        "invalid resolution falls back to model defaults"
    );
    assert_eq!(
        (state.render_scale, state.ui_scale),
        (defaults.render_scale, defaults.ui_scale),
        "invalid discrete scale values fall back to model defaults"
    );
    assert!(
        !state.show_fps
            && state.maintain_vertical_fov == defaults.maintain_vertical_fov
            && state.native_text == defaults.native_text
            && state.screen_flashes == defaults.screen_flashes,
        "invalid modern booleans fall back to model defaults"
    );
    assert!(
        state.anti_aliasing_samples == defaults.anti_aliasing_samples
            && state.anisotropy == defaults.anisotropy,
        "invalid MSAA and anisotropy values fall back to model defaults"
    );
    assert!(
        state.difficulty == defaults.difficulty
            && state.mouse_sensitivity == defaults.mouse_sensitivity,
        "invalid user values fall back to model defaults"
    );
    assert_eq!(
        state.control_mode,
        ControlMode::Classic,
        "malformed control mode falls back to Classic"
    );
    assert_eq!(
        state.auto_quaff, defaults.auto_quaff,
        "invalid boolean falls back to model default"
    );
    settings::commit(&roots.system(), &roots.user(), &state)
        .expect("malformed but decodable INIs remain writable");
    assert!(
        read_text(&roots.user().join("Game.ini")).contains("[Engine.GameEngine"),
        "malformed section header is preserved on commit"
    );
    assert_eq!(
        read_bytes(&roots.user().join("Game.ini.bak")),
        malformed_game.as_bytes(),
        "malformed original is backed up exactly"
    );
}

// ------------------------------------- TestUtf16AndInvalidEncodingRecovery

#[test]
fn utf16_and_invalid_encoding_recovery() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let game_text = "; UTF16 game comment\r\n[Engine.GameEngine]\r\nFrameRateLimit=60.000000\r\n";
    let user_text = "; UTF16 user comment\n[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\n";
    let game_path = roots.user().join("Game.ini");
    let user_path = roots.user().join("User.ini");
    write_bytes(&game_path, &utf16_ascii(game_text, true));
    write_bytes(&user_path, &utf16_ascii(user_text, false));
    settings::commit(&roots.system(), &roots.user(), &distinct_settings())
        .expect("UTF-16 LE/BE settings commit succeeds");
    let game = read_bytes(&game_path);
    let user = read_bytes(&user_path);
    assert_eq!(
        &game[..2],
        &[0xFF, 0xFE],
        "UTF-16 LE encoding is preserved"
    );
    assert_eq!(
        &user[..2],
        &[0xFE, 0xFF],
        "UTF-16 BE encoding is preserved"
    );
    assert!(
        decode_utf16_ascii(&game, true).contains("; UTF16 game comment"),
        "UTF-16 comment survives round-trip"
    );
    assert!(
        decode_utf16_ascii(&user, false).contains("Difficulty=DifficultyHard"),
        "UTF-16 owned user value is updated"
    );

    let invalid = TemporaryRoots::new();
    default_templates(&invalid);
    let invalid_utf8 = b"[Engine.GameEngine]\nFrameRateLimit=\xff\n".to_vec();
    let invalid_game = invalid.user().join("Game.ini");
    let invalid_user = invalid.user().join("User.ini");
    write_bytes(&invalid_game, &invalid_utf8);
    let invalid_utf16: Vec<u8> = vec![0xFF, 0xFE, 0x00];
    write_bytes(&invalid_user, &invalid_utf16);
    let recovered = settings::load(&invalid.system(), &invalid.user())
        .expect("invalid writable encoding falls back to immutable defaults in memory");
    assert_eq!(
        read_bytes(&invalid_game),
        invalid_utf8,
        "encoding recovery during load is read-only"
    );
    settings::commit(&invalid.system(), &invalid.user(), &recovered)
        .expect("commit materializes recovered default documents");
    assert_eq!(
        read_bytes(&invalid.game_bak()),
        invalid_utf8,
        "invalid UTF-8 original is backed up exactly"
    );
    assert_eq!(
        read_bytes(&invalid.user_bak()),
        invalid_utf16,
        "invalid UTF-16 original is backed up exactly"
    );
    assert!(
        read_text(&invalid_game).contains("FrameRateLimit=60"),
        "recovered Game.ini is valid baseline plus owned settings"
    );
}

impl TemporaryRoots {
    fn game_bak(&self) -> PathBuf {
        self.user().join("Game.ini.bak")
    }

    fn user_bak(&self) -> PathBuf {
        self.user().join("User.ini.bak")
    }
}

// ------------------------------------------------------ TestUnsafeBackupRejected

#[test]
fn unsafe_backup_rejected() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    write_text(
        &roots.user().join("Game.ini"),
        "[Engine.GameEngine]\nFrameRateLimit=60\n",
    );
    write_text(
        &roots.user().join("User.ini"),
        "[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\n",
    );
    let elsewhere = roots.user().join("elsewhere");
    write_text(&elsewhere, "do not replace");
    symlink(&elsewhere, &roots.user().join("Game.ini.bak"));
    assert!(
        settings::commit(&roots.system(), &roots.user(), &Settings::default()).is_err(),
        "symlink backup path is rejected"
    );
    assert_eq!(
        read_text(&elsewhere),
        "do not replace",
        "unsafe backup target is untouched"
    );
}

// ---------------------------------------------- TestDataSourceCatalogPersistence
//
// DEFERRED fragment: ValidateHP2DataRoot bootstrap validation and the
// per-root map-sentinel resolution belong to the engine HP2Paths surface
// (`InstallHP2Paths`), which is outside this hp-app slice.

#[test]
fn data_source_catalog_persistence() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    let retail = create_data_root(&roots, "RetailData", "retail");
    let prototype = create_data_root(&roots, "PrototypeData", "prototype");
    let unavailable = roots.root.join("UnavailableData");
    let canonical_retail = std::fs::canonicalize(&retail).expect("canonical retail");
    let canonical_prototype = std::fs::canonicalize(&prototype).expect("canonical prototype");

    let configuration = store::load_data_source_configuration(&roots.user())
        .expect("missing launcher catalog defaults successfully");
    assert!(
        configuration.selected.is_none()
            && configuration.retail_root.is_empty()
            && configuration.prototype_root.is_empty(),
        "missing launcher catalog defaults to an unset selection"
    );

    let mut configuration = DataSourceConfiguration::default();
    configuration.selected = Some(DataSource::Prototype);
    configuration.retail_root = canonical_retail.display().to_string();
    configuration.prototype_root = canonical_prototype.display().to_string();
    store::commit_data_source_configuration(&roots.user(), &configuration)
        .expect("absolute named roots commit to the launcher catalog");
    let catalog = roots.user().join("Launcher.ini");
    let first_catalog = read_text(&catalog);
    assert!(
        first_catalog.contains("[DataSources]")
            && first_catalog.contains("Selected=Prototype")
            && first_catalog.contains(&format!("RetailRoot={}", canonical_retail.display()))
            && first_catalog.contains(&format!("PrototypeRoot={}", canonical_prototype.display())),
        "launcher catalog serializes its selected source and both canonical roots"
    );

    let loaded =
        store::load_data_source_configuration(&roots.user()).expect("saved launcher catalog reloads");
    assert!(
        loaded.selected == Some(DataSource::Prototype)
            && loaded.retail_root == canonical_retail.display().to_string()
            && loaded.prototype_root == canonical_prototype.display().to_string(),
        "both named roots round-trip independently"
    );

    let mut unavailable_catalog = loaded.clone();
    unavailable_catalog.selected = Some(DataSource::Retail);
    unavailable_catalog.retail_root = unavailable.display().to_string();
    store::commit_data_source_configuration(&roots.user(), &unavailable_catalog)
        .expect("an unavailable but absolute saved root remains editable catalog state");
    let loaded =
        store::load_data_source_configuration(&roots.user()).expect("unavailable catalog reloads");
    assert!(
        loaded.selected == Some(DataSource::Retail)
            && loaded.retail_root == unavailable.display().to_string(),
        "unavailable root is retained instead of silently selecting another source"
    );

    let catalog_before_rejected_commit = read_bytes(&catalog);
    let mut invalid = loaded.clone();
    invalid.selected = None;
    assert!(
        store::commit_data_source_configuration(&roots.user(), &invalid).is_err(),
        "unknown data source enum is rejected"
    );
    invalid = loaded.clone();
    invalid.prototype_root = "relative-data-root".to_string();
    assert!(
        store::commit_data_source_configuration(&roots.user(), &invalid).is_err(),
        "relative named root is rejected"
    );
    assert_eq!(
        read_bytes(&catalog),
        catalog_before_rejected_commit,
        "rejected catalog inputs never alter persisted state"
    );

    if gate_enabled() {
        set_publish_failure_for_testing(1);
        let mut failing = loaded;
        failing.prototype_root = canonical_prototype.display().to_string();
        assert!(
            store::commit_data_source_configuration(&roots.user(), &failing).is_err(),
            "catalog publication failure is reported"
        );
        set_publish_failure_for_testing(-1);
        assert_eq!(
            read_bytes(&catalog),
            catalog_before_rejected_commit,
            "failed catalog publication restores prior bytes"
        );
    }
}

// ------------------------------------- TestMalformedDataSourceCatalogRecovery

#[test]
fn malformed_data_source_catalog_recovery() {
    let roots = TemporaryRoots::new();
    let prototype = create_data_root(&roots, "PrototypeData", "prototype");
    let canonical_prototype = std::fs::canonicalize(&prototype).expect("canonical prototype");
    write_text(
        &roots.user().join("Launcher.ini"),
        &format!(
            "[DataSources]\nSelected=UnknownEdition\nRetailRoot=relative-retail\nPrototypeRoot={}\n",
            canonical_prototype.display()
        ),
    );

    let loaded = store::load_data_source_configuration(&roots.user())
        .expect("malformed catalog recovers without failing launch setup");
    // An unknown persisted source falls back to the native Retail default;
    // the Rust model represents that default as `None`.
    assert!(
        loaded.selected.is_none(),
        "unknown persisted source defaults to Retail"
    );
    assert!(
        loaded.retail_root.is_empty(),
        "malformed relative Retail root is discarded"
    );
    assert_eq!(
        loaded.prototype_root,
        canonical_prototype.display().to_string(),
        "valid independent Prototype root survives recovery"
    );
}

// ------------------------------------- TestDataSourceProfilesAndMigration
//
// DEFERRED fragment: the cross-profile save-discovery assertions rely on
// `DiscoverSaves`, whose save-tree scanner has no hp-app port yet.

#[test]
fn data_source_profiles_and_migration() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    write_text(
        &roots.user().join("Game.ini"),
        "[Engine.GameEngine]\nFrameRateLimit=30\n",
    );
    write_text(
        &roots.user().join("User.ini"),
        "[Engine.PlayerPawn]\nDifficulty=DifficultyHard\n",
    );
    std::fs::create_dir_all(roots.user().join("Save").join("cache")).expect("save cache dir");
    std::fs::create_dir_all(roots.user().join("Cache")).expect("cache dir");
    write_text(&roots.user().join("Save").join("Save1.usa"), "retail save");
    write_text(&roots.user().join("Save").join("cache").join("Thumb1.bmp"), "cache");
    write_text(&roots.user().join("Cache").join("Engine.cache"), "engine cache");

    let retail_profile = store::prepare_data_source_profile(&roots.user(), DataSource::Retail)
        .expect("first Retail profile prepares and migrates legacy state");
    let expected_retail = roots.user().join("Profiles").join("Retail");
    assert_eq!(
        retail_profile,
        expected_retail,
        "Retail profile is source-specific"
    );
    assert!(
        read_text(&expected_retail.join("Game.ini")).contains("FrameRateLimit=30")
            && read_text(&expected_retail.join("Save").join("Save1.usa")) == "retail save",
        "legacy settings and saves migrate into the selected profile"
    );
    assert!(
        roots.user().join("Game.ini").exists() && roots.user().join("Save").join("Save1.usa").exists(),
        "legacy mutable state remains intact after durable profile migration"
    );
    assert!(
        read_text(&roots.user().join("Launcher.ini")).contains("LegacyProfile=Retail"),
        "successful migration records its selected source in launcher-owned catalog"
    );

    let mut retail_settings = Settings::default();
    retail_settings.frame_rate_limit = 30;
    settings::commit(&roots.system(), &retail_profile, &retail_settings)
        .expect("Retail profile settings commit");
    std::fs::create_dir_all(expected_retail.join("Save")).expect("retail save dir");
    write_text(&expected_retail.join("Save").join("Save2.usa"), "retail profile save");

    let prototype_profile = store::prepare_data_source_profile(&roots.user(), DataSource::Prototype)
        .expect("Prototype profile prepares after Retail migration");
    let expected_prototype = roots.user().join("Profiles").join("Prototype");
    assert_eq!(
        prototype_profile,
        expected_prototype,
        "Prototype profile is source-specific"
    );
    assert!(
        !expected_prototype.join("Game.ini").exists()
            && !expected_prototype.join("Save").join("Save1.usa").exists(),
        "legacy state does not leak into the second profile"
    );
    let mut prototype_settings = Settings::default();
    prototype_settings.frame_rate_limit = 120;
    settings::commit(&roots.system(), &prototype_profile, &prototype_settings)
        .expect("Prototype profile settings commit");
    std::fs::create_dir_all(expected_prototype.join("Save")).expect("prototype save dir");
    write_text(
        &expected_prototype.join("Save").join("Save3.usa"),
        "prototype profile save",
    );

    let retail_state =
        settings::load(&roots.system(), &retail_profile).expect("Retail profile settings reload");
    let prototype_state = settings::load(&roots.system(), &prototype_profile)
        .expect("Prototype profile settings reload");
    assert_eq!(
        (retail_state.frame_rate_limit, prototype_state.frame_rate_limit),
        (30, 120),
        "profile settings remain isolated"
    );
}

// ------------------------------------ TestDataSourceMigrationFailureAndQuit

#[test]
fn data_source_migration_failure_and_quit() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    default_templates(&roots);
    let quit = LaunchSelection::default();
    assert!(
        build_selected_command(&quit).is_ok(),
        "Quit-like selection remains valid"
    );
    assert!(
        !roots.user().join("Launcher.ini").exists() && !roots.user().join("Profiles").exists(),
        "Quit-like selection leaves catalog and profiles untouched"
    );

    write_text(
        &roots.user().join("Game.ini"),
        "[Engine.GameEngine]\nFrameRateLimit=30\n",
    );
    let outside_save = roots.user().join("outside-save");
    write_text(&outside_save, "must not follow");
    std::fs::create_dir_all(roots.user().join("Save")).expect("save directory");
    symlink(&outside_save, &roots.user().join("Save").join("Save1.usa"));
    assert!(
        store::prepare_data_source_profile(&roots.user(), DataSource::Retail).is_err(),
        "irregular legacy state rejects profile migration"
    );
    assert!(
        !roots.user().join("Profiles").join("Retail").exists()
            && !roots.user().join("Launcher.ini").exists(),
        "failed migration publishes neither a profile nor a catalog marker"
    );
    assert!(
        read_text(&roots.user().join("Game.ini")).contains("FrameRateLimit=30")
            && std::fs::symlink_metadata(roots.user().join("Save").join("Save1.usa"))
                .expect("save link")
                .is_symlink(),
        "failed migration leaves legacy source state untouched"
    );
}

// ---------------------------------------------------- TestLegacySettingsMigration

#[test]
fn legacy_settings_migration() {
    struct MigrationCase {
        description: &'static str,
        input: Vec<LegacyRow>,
        expected: Vec<LegacyRow>,
        journal: Vec<MigrationChange>,
    }
    let mut cases: Vec<MigrationCase> = Vec::new();

    cases.push(MigrationCase {
        description: "renderer-era legacy configuration normalizes completely",
        input: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "On"),
            row("SDLDrv.SDLClient", "BorderlessWindow", "yes"),
            row("SDLDrv.SDLClient", "UseDesktopResolution", "False"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "1024"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "768"),
            row("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
            row("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
            row("Engine.GameEngine", "FrameRateLimit", "30.000000"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.850000"),
            row("SDLDrv.SDLClient", "UIScale", "1.25"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "1"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16.000000"),
            row("SDLDrv.SDLClient", "Brightness", "0.400000"),
            row("ALAudio.ALAudioSubsystem", "SoundVolume", "0.900000"),
            row("ALAudio.ALAudioSubsystem", "MusicVolume", "0.53"),
            row("Engine.PlayerPawn", "MouseSensitivity", "3"),
            row("Engine.PlayerPawn", "Difficulty", "difficultyhard"),
            row("Engine.PlayerPawn", "ObjectDetail", "objectdetailhigh"),
            row("SDLDrv.SDLClient", "TextureDetail", "LOW"),
            row("HGame.Harry", "bAutoQuaff", "No"),
            row("Unrelated.Section", "KeepMe", "Untouched"),
        ],
        expected: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "False"),
            row("SDLDrv.SDLClient", "BorderlessWindow", "True"),
            row("SDLDrv.SDLClient", "UseDesktopResolution", "False"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "640"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "480"),
            row("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
            row("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
            row("Engine.GameEngine", "FrameRateLimit", "30"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.85"),
            row("SDLDrv.SDLClient", "UIScale", "1.25"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "True"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16"),
            row("SDLDrv.SDLClient", "Brightness", "0.4"),
            row("ALAudio.ALAudioSubsystem", "SoundVolume", "0.9"),
            row("ALAudio.ALAudioSubsystem", "MusicVolume", "0.53"),
            row("Engine.PlayerPawn", "MouseSensitivity", "3"),
            row("Engine.PlayerPawn", "Difficulty", "DifficultyHard"),
            row("Engine.PlayerPawn", "ObjectDetail", "ObjectDetailHigh"),
            row("SDLDrv.SDLClient", "TextureDetail", "Low"),
            row("HGame.Harry", "bAutoQuaff", "False"),
            row("Unrelated.Section", "KeepMe", "Untouched"),
        ],
        journal: vec![
            change("SDLDrv.SDLClient", "StartupFullscreen", "On", "True", "settings.boolean_spelling"),
            change("SDLDrv.SDLClient", "BorderlessWindow", "yes", "True", "settings.boolean_spelling"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "1", "True", "settings.boolean_spelling"),
            change("HGame.Harry", "bAutoQuaff", "No", "False", "settings.boolean_spelling"),
            change("SDLDrv.SDLClient", "StartupFullscreen", "True", "False", "settings.screen_mode_consolidated"),
            change("SDLDrv.SDLClient", "WindowedViewportX", "1024", "640", "settings.viewport_consolidated"),
            change("SDLDrv.SDLClient", "WindowedViewportY", "768", "480", "settings.viewport_consolidated"),
            change("Engine.GameEngine", "FrameRateLimit", "30.000000", "30", "settings.frame_rate_limit_normalized"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.850000", "0.85", "settings.scale_normalized"),
            change("SDLDrv.SDLClient", "Brightness", "0.400000", "0.4", "settings.range_normalized"),
            change("ALAudio.ALAudioSubsystem", "SoundVolume", "0.900000", "0.9", "settings.range_normalized"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16.000000", "16", "settings.anisotropy_normalized"),
            change("SDLDrv.SDLClient", "TextureDetail", "LOW", "Low", "settings.enum_normalized"),
            change("Engine.PlayerPawn", "ObjectDetail", "objectdetailhigh", "ObjectDetailHigh", "settings.enum_normalized"),
            change("Engine.PlayerPawn", "Difficulty", "difficultyhard", "DifficultyHard", "settings.enum_normalized"),
        ],
    });

    cases.push(MigrationCase {
        description: "conflicting and invalid legacy values fall back to model defaults",
        input: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "True"),
            row("SDLDrv.SDLClient", "BorderlessWindow", "True"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "12"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "768"),
            row("Engine.GameEngine", "FrameRateLimit", "59"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.9"),
            row("SDLDrv.SDLClient", "UIScale", "abc"),
            row("SDLDrv.SDLClient", "Brightness", "nan"),
            row("Engine.PlayerPawn", "MouseSensitivity", "not-a-number"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "maybe"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "8"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "32"),
            row("Engine.PlayerPawn", "Difficulty", "Ultra"),
            row("SDLDrv.SDLClient", "NativeText", "garbage"),
        ],
        expected: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "False"),
            row("SDLDrv.SDLClient", "BorderlessWindow", "True"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "800"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "600"),
            row("Engine.GameEngine", "FrameRateLimit", "60"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "1"),
            row("SDLDrv.SDLClient", "UIScale", "1"),
            row("SDLDrv.SDLClient", "Brightness", "0.4"),
            row("Engine.PlayerPawn", "MouseSensitivity", "3"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "False"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "0"),
            row("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "4"),
            row("Engine.PlayerPawn", "Difficulty", "DifficultyEasy"),
            row("SDLDrv.SDLClient", "NativeText", "True"),
        ],
        journal: vec![
            change("SDLDrv.SDLClient", "NativeText", "garbage", "True", "settings.boolean_invalid_defaulted"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "maybe", "False", "settings.boolean_invalid_defaulted"),
            change("SDLDrv.SDLClient", "StartupFullscreen", "True", "False", "settings.screen_mode_consolidated"),
            change("SDLDrv.SDLClient", "WindowedViewportX", "12", "800", "settings.viewport_consolidated"),
            change("SDLDrv.SDLClient", "WindowedViewportY", "768", "600", "settings.viewport_consolidated"),
            change("Engine.GameEngine", "FrameRateLimit", "59", "60", "settings.frame_rate_limit_invalid_defaulted"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.9", "1", "settings.scale_invalid_defaulted"),
            change("SDLDrv.SDLClient", "UIScale", "abc", "1", "settings.scale_invalid_defaulted"),
            change("SDLDrv.SDLClient", "Brightness", "nan", "0.4", "settings.range_invalid_defaulted"),
            change("Engine.PlayerPawn", "MouseSensitivity", "not-a-number", "3", "settings.range_invalid_defaulted"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "8", "0", "settings.antialiasing_normalized"),
            change("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "32", "4", "settings.anisotropy_invalid_defaulted"),
            change("Engine.PlayerPawn", "Difficulty", "Ultra", "DifficultyEasy", "settings.enum_invalid_defaulted"),
        ],
    });

    cases.push(MigrationCase {
        description: "windowed mode consolidates onto the windowed resolution pair",
        input: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "False"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "1280"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "800"),
            row("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
            row("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
        ],
        expected: vec![
            row("SDLDrv.SDLClient", "StartupFullscreen", "False"),
            row("SDLDrv.SDLClient", "WindowedViewportX", "1280"),
            row("SDLDrv.SDLClient", "WindowedViewportY", "800"),
            row("SDLDrv.SDLClient", "FullscreenViewportX", "1280"),
            row("SDLDrv.SDLClient", "FullscreenViewportY", "800"),
        ],
        journal: vec![
            change("SDLDrv.SDLClient", "FullscreenViewportX", "640", "1280", "settings.viewport_consolidated"),
            change("SDLDrv.SDLClient", "FullscreenViewportY", "480", "800", "settings.viewport_consolidated"),
        ],
    });

    {
        let input = vec![
            row("Engine.Engine", "GameRenderDevice", "D3DDrv.D3DRenderDevice"),
            row("SDLDrv.SDLClient", "ShowFPS", "True"),
            row("Unrelated.Section", "MiXeDKey", "MiXeDValue"),
        ];
        cases.push(MigrationCase {
            description: "canonical and unrelated rows leave no journal entries",
            expected: input.clone(),
            input,
            journal: vec![],
        });
    }

    cases.push(MigrationCase {
        description: "final duplicated row wins while shadowed rows stay untouched",
        input: vec![
            row("Engine.GameEngine", "FrameRateLimit", "120"),
            row("Engine.GameEngine", "FrameRateLimit", "30.000000"),
        ],
        expected: vec![
            row("Engine.GameEngine", "FrameRateLimit", "120"),
            row("Engine.GameEngine", "FrameRateLimit", "30"),
        ],
        journal: vec![change(
            "Engine.GameEngine",
            "FrameRateLimit",
            "30.000000",
            "30",
            "settings.frame_rate_limit_normalized",
        )],
    });

    for test_case in &cases {
        let (migrated, changes) = migrate_legacy_settings(&test_case.input);
        assert_eq!(
            migrated.len(),
            test_case.expected.len(),
            "{}: row count is {}, expected {}",
            test_case.description,
            migrated.len(),
            test_case.expected.len()
        );
        for (index, (actual, expected)) in migrated.iter().zip(&test_case.expected).enumerate() {
            assert_eq!(
                actual, expected,
                "{}: row {index} mismatch",
                test_case.description
            );
        }
        assert_eq!(
            changes.len(),
            test_case.journal.len(),
            "{}: journal length is {}, expected {}",
            test_case.description,
            changes.len(),
            test_case.journal.len()
        );
        for (index, (actual, expected)) in changes.iter().zip(&test_case.journal).enumerate() {
            assert_eq!(
                actual, expected,
                "{}: journal entry {index} mismatch",
                test_case.description
            );
        }

        let (rerun, rerun_changes) = migrate_legacy_settings(&migrated);
        assert_eq!(
            rerun, test_case.expected,
            "{} (idempotence)",
            test_case.description
        );
        assert!(
            rerun_changes.is_empty(),
            "{}: rerun journal is empty",
            test_case.description
        );
    }
}

// ---------------------------------- TestLaunchSelectionSerializationContract

#[test]
fn launch_selection_serialization_contract() {
    let quit = LaunchSelection::default();
    let mut new_game = LaunchSelection::default();
    new_game.action = LaunchAction::NewGame;
    let serializable = [
        ("quit selection serializes", quit.clone()),
        ("new game selection serializes", new_game),
        ("flat continue selection serializes", continue_selection(12, -1)),
        ("slotted continue selection serializes", continue_selection(12, 3)),
    ];
    for (description, selection) in &serializable {
        let fields =
            serialize_launch_selection(selection).unwrap_or_else(|error| panic!("{description}: {error}"));
        let parsed = deserialize_launch_selection(&fields);
        assert!(
            parsed.error.is_none(),
            "{description} deserializes: {:?}",
            parsed.error
        );
        assert!(
            same_selection_core(&parsed.selection, selection),
            "{description} round-trips identically"
        );
    }

    let fields =
        serialize_launch_selection(&quit).expect("quit serializes");
    assert!(
        fields.len() == 1 && fields[0].key == "Action" && fields[0].value == "Quit",
        "quit serializes to a single Action row"
    );
    let fields = serialize_launch_selection(&continue_selection(12, 3))
        .expect("slotted continue serializes");
    assert!(
        fields.len() == 4
            && fields[1].key == "HasSave"
            && fields[1].value == "True"
            && fields[2].key == "SaveIndex"
            && fields[2].value == "12"
            && fields[3].key == "SaveSlot"
            && fields[3].value == "3",
        "slotted continue serializes Action, HasSave, SaveIndex, and SaveSlot rows"
    );

    let mut unsaved_continue = continue_selection(-1, -1);
    unsaved_continue.has_save = false;
    let mut negative_slot = continue_selection(5, -1);
    negative_slot.save.uses_slot_directory = true;
    negative_slot.save.slot = -7;
    let mut error_action = LaunchSelection::default();
    error_action.action = LaunchAction::Error;
    let rejected = [
        ("continue without a save is not serializable", unsaved_continue),
        ("continue with a negative save index is not serializable", continue_selection(-1, -1)),
        ("slotted continue with a negative slot is not serializable", negative_slot),
        ("error action is not serializable", error_action),
    ];
    for (description, selection) in &rejected {
        assert!(
            serialize_launch_selection(selection).is_err(),
            "{description}"
        );
    }

    let malformed: Vec<(&str, Vec<SelectionField>)> = vec![
        ("empty store falls back to quit", vec![]),
        ("unknown action falls back to quit", vec![field("Action", "Bogus")]),
        (
            "continue without a save marker falls back to quit",
            vec![field("Action", "Continue")],
        ),
        (
            "continue with an explicit false save marker falls back to quit",
            vec![field("Action", "Continue"), field("HasSave", "False"), field("SaveIndex", "5")],
        ),
        (
            "continue with a missing save index falls back to quit",
            vec![field("Action", "Continue"), field("HasSave", "True")],
        ),
        (
            "continue with a nonnumeric save index falls back to quit",
            vec![field("Action", "Continue"), field("HasSave", "True"), field("SaveIndex", "nope")],
        ),
        (
            "continue with a negative save index falls back to quit",
            vec![field("Action", "Continue"), field("HasSave", "True"), field("SaveIndex", "-5")],
        ),
        (
            "slotted continue with a bad slot falls back to quit",
            vec![
                field("Action", "Continue"),
                field("HasSave", "True"),
                field("SaveIndex", "5"),
                field("SaveSlot", "x"),
            ],
        ),
        ("unrelated rows alone fall back to quit", vec![field("RetailRoot", "/Retail")]),
    ];
    for (description, fields) in &malformed {
        let parsed = deserialize_launch_selection(fields);
        assert!(
            parsed.error.is_some(),
            "{description} reports malformed store"
        );
        assert!(
            parsed.selection.action == LaunchAction::Quit && !parsed.selection.has_save,
            "{description} falls back to the safe quit selection"
        );
    }
    let parsed = deserialize_launch_selection(&[]);
    assert!(
        parsed.error.is_some() && parsed.selection.action == LaunchAction::Quit,
        "fallback selection is populated even when parsing fails"
    );

    {
        let reordered = vec![
            field("SaveSlot", "3"),
            field("Unrelated", "ignored"),
            field("saveindex", "12"),
            field("hassave", "True"),
            field("ACTION", "CONTINUE"),
        ];
        let parsed = deserialize_launch_selection(&reordered);
        assert!(
            parsed.error.is_none(),
            "field order, casing, and unrelated rows are tolerated: {:?}",
            parsed.error
        );
        assert!(
            same_selection_core(&parsed.selection, &continue_selection(12, 3)),
            "reordered case-insensitive rows restore the slotted continue selection"
        );

        let duplicated = vec![
            field("Action", "NewGame"),
            field("Action", "Continue"),
            field("HasSave", "True"),
            field("SaveIndex", "7"),
        ];
        let parsed = deserialize_launch_selection(&duplicated);
        assert!(parsed.error.is_none(), "duplicated rows deserialize");
        assert!(
            same_selection_core(&parsed.selection, &continue_selection(7, -1)),
            "the final duplicated action row wins"
        );
    }
}

// ---------------------------------- TestLaunchSelectionPersistenceRoundTrip

#[test]
fn launch_selection_persistence_round_trip() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    let catalog = roots.user().join("Launcher.ini");

    let quit = LaunchSelection::default();
    let mut new_game = LaunchSelection::default();
    new_game.action = LaunchAction::NewGame;
    let persisted = [
        ("quit selection persists", quit.clone()),
        ("new game selection persists", new_game.clone()),
        ("flat continue selection persists", continue_selection(12, -1)),
        ("slotted continue selection persists", continue_selection(12, 3)),
    ];
    for (description, selection) in &persisted {
        store::commit_launch_selection(&roots.user(), selection)
            .unwrap_or_else(|error| panic!("{description}: {error}"));
        let outcome =
            store::load_launch_selection(&roots.user()).unwrap_or_else(|error| panic!("{description}: {error}"));
        assert!(
            outcome.ok(),
            "{description} reloads cleanly: {:?}",
            outcome.error
        );
        assert!(
            same_selection_core(&outcome.selection, selection),
            "{description} round-trips identically"
        );
    }

    store::commit_launch_selection(&roots.user(), &new_game).expect("minimal selection commits");
    let minimal_store = read_text(&catalog);
    assert!(
        minimal_store.contains("[LastLaunch]")
            && minimal_store.contains("Action=NewGame")
            && !minimal_store.contains("SaveIndex")
            && !minimal_store.contains("HasSave"),
        "non-continue selections persist only their action row"
    );

    let malformed: Vec<(&str, &str)> = vec![
        ("an empty catalog falls back to quit", ""),
        ("an unknown action falls back to quit", "[LastLaunch]\nAction=Bogus\n"),
        (
            "continue without a save index falls back to quit",
            "[LastLaunch]\nAction=Continue\nHasSave=True\n",
        ),
        (
            "a nonnumeric save index falls back to quit",
            "[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=nope\n",
        ),
        (
            "a negative save index falls back to quit",
            "[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=-5\n",
        ),
        (
            "a bad slot coordinate falls back to quit",
            "[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=5\nSaveSlot=x\n",
        ),
        (
            "an inconsistent save marker falls back to quit",
            "[LastLaunch]\nAction=Continue\nHasSave=False\nSaveIndex=5\n",
        ),
    ];
    for (description, contents) in &malformed {
        write_text(&catalog, contents);
        let outcome = store::load_launch_selection(&roots.user())
            .unwrap_or_else(|error| panic!("{description}: {error}"));
        assert!(
            outcome.error.is_some(),
            "{description} reports a malformed store"
        );
        assert!(
            outcome.selection.action == LaunchAction::Quit && !outcome.selection.has_save,
            "{description}"
        );
    }

    let missing_roots = TemporaryRoots::new();
    let missing = store::load_launch_selection(&missing_roots.user())
        .expect("missing catalog yields an outcome, not an io failure");
    assert!(
        missing.error.is_some() && missing.selection.action == LaunchAction::Quit,
        "a missing catalog yields the quit fallback without creating files"
    );
    assert!(
        !missing_roots.user().join("Launcher.ini").exists(),
        "loading a missing catalog never materializes it"
    );

    write_text(&catalog, "[DataSources]\nSelected=Prototype\n");
    let sources = store::load_data_source_configuration(&roots.user())
        .expect("data source catalog loads alongside launch selection state");
    assert_eq!(
        sources.selected,
        Some(DataSource::Prototype),
        "data source catalog loads alongside launch selection state"
    );
    store::commit_launch_selection(&roots.user(), &continue_selection(7, -1))
        .expect("selection commits into an existing catalog");
    let sources = store::load_data_source_configuration(&roots.user())
        .expect("selection commit preserves the data source section");
    assert_eq!(
        sources.selected,
        Some(DataSource::Prototype),
        "selection commit preserves the data source section"
    );
    let combined =
        store::load_launch_selection(&roots.user()).expect("both catalogs coexist after a selection commit");
    assert!(
        combined.ok() && same_selection_core(&combined.selection, &continue_selection(7, -1)),
        "both catalogs coexist after a selection commit"
    );
}

// ----------------------------- TestDuplicateLaunchSelectionCommitKeepsLastWriter

#[test]
fn duplicate_launch_selection_commit_keeps_last_writer() {
    let _io = io_guard();
    let roots = TemporaryRoots::new();
    let catalog = roots.user().join("Launcher.ini");

    let first = continue_selection(7, -1);
    let mut second = LaunchSelection::default();
    second.action = LaunchAction::NewGame;
    store::commit_launch_selection(&roots.user(), &first)
        .expect("duplicate-write first commit succeeds");
    let first_store = read_text(&catalog);
    assert!(
        !first_store.is_empty() && !catalog.with_extension("ini.bak").exists(),
        "the first commit creates no backup because no prior generation existed"
    );

    store::commit_launch_selection(&roots.user(), &second)
        .expect("duplicate-write second commit succeeds");
    let reloaded =
        store::load_launch_selection(&roots.user()).expect("second generation reloads");
    assert!(
        reloaded.ok() && same_selection_core(&reloaded.selection, &second),
        "the last writer's selection is the persisted one"
    );

    let second_store = read_text(&catalog);
    assert_ne!(second_store, first_store, "the second commit rewrote the catalog");
    assert_eq!(
        count_occurrences(&second_store, "Action="),
        1,
        "exactly one canonical action row survives duplicate writes"
    );
    assert!(
        !second_store.contains("SaveIndex") && !second_store.contains("HasSave"),
        "the replaced generation leaves no stale save coordinates"
    );
    assert!(
        catalog.with_extension("ini.bak").exists()
            && read_text(&bak_path(&catalog)) == first_store,
        "the previous generation is preserved exactly as the backup"
    );

    let third = continue_selection(12, 3);
    store::commit_launch_selection(&roots.user(), &third)
        .expect("duplicate-write third commit succeeds");
    let reloaded = store::load_launch_selection(&roots.user()).expect("the third generation wins");
    assert!(
        reloaded.ok() && same_selection_core(&reloaded.selection, &third),
        "the third generation wins"
    );
    let third_store = read_text(&catalog);
    assert!(
        count_occurrences(&third_store, "Action=") == 1
            && third_store.contains("SaveIndex=12")
            && third_store.contains("SaveSlot=3"),
        "the third generation is complete and singular"
    );
    assert_eq!(
        read_text(&bak_path(&catalog)),
        first_store,
        "the first-generation backup is never overwritten by later writes"
    );
}

fn bak_path(catalog: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bak", catalog.display()))
}
