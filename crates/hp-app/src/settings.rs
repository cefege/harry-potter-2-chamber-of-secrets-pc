//! Launcher settings model: validated setters and INI load/commit through
//! the profile store.
//!
//! Ranges mirror `HarryPotter2/Unreal/SDLLaunch/Src/HP2MacLauncher.mm`
//! (resolution 320..16384 per dimension, brightness 0.1–1.0, mouse
//! sensitivity 0.2–10.0); validation boundaries port
//! `ValidateLauncherSettings`, and the load/apply mappings port
//! `LoadSettings`/`ApplyGame`/`ApplyUser` from `HP2LauncherStore.cpp`.

use std::path::Path;

use crate::error::AppError;
use crate::ini_text::{
    bool_text, double_text, get, get_bool, get_discrete_float, get_discrete_integer, get_integer,
    get_number, get_ranged_float, parse_ini,
};
use crate::settings_model::{
    AA_SAMPLE_VALUES, ANISOTROPY_VALUES, FRAME_RATE_LIMITS, RENDER_SCALES, UI_SCALES,
};
pub use crate::settings_model::{
    ControlMode, Difficulty, ObjectDetail, RenderBackend, ScreenMode, Settings, TextureDetail,
};
pub type Result<T, E = AppError> = std::result::Result<T, E>;

// ------------------------------------------------------------- validation

/// Port of `ValidateLauncherSettings`.
pub fn validate(settings: &Settings) -> Result<()> {
    let invalid = |message: &'static str| AppError::new("app.settings_invalid", message);
    if !(320..=16384).contains(&settings.resolution.width)
        || !(320..=16384).contains(&settings.resolution.height)
    {
        return Err(invalid(
            "Resolution dimensions must each be between 320 and 16384.",
        ));
    }
    if !settings.brightness.is_finite() || !(0.1..=1.0).contains(&settings.brightness) {
        return Err(invalid("Brightness must be between 0.1 and 1.0."));
    }
    if is_discrete(settings.render_scale, &RENDER_SCALES).is_none() {
        return Err(invalid(
            "Render scale must be 0.50, 0.67, 0.75, 0.85, or 1.00.",
        ));
    }
    if is_discrete(settings.ui_scale, &UI_SCALES).is_none() {
        return Err(invalid(
            "UI scale must be 0.75, 1.00, 1.25, 1.50, 1.75, or 2.00.",
        ));
    }
    if !settings.mouse_sensitivity.is_finite()
        || !(0.2..=10.0).contains(&settings.mouse_sensitivity)
    {
        return Err(invalid("Mouse sensitivity must be between 0.2 and 10.0."));
    }
    if !volume_ok(settings.sound_volume) || !volume_ok(settings.music_volume) {
        return Err(invalid(
            "Sound and music volumes must be between 0.0 and 1.0.",
        ));
    }
    if !FRAME_RATE_LIMITS.contains(&settings.frame_rate_limit) {
        return Err(invalid(
            "Frame rate limit must be unlimited, 30, 60, 120, or 144.",
        ));
    }
    if !AA_SAMPLE_VALUES.contains(&settings.anti_aliasing_samples) {
        return Err(invalid("Anti-aliasing must be off, 2x MSAA, or 4x MSAA."));
    }
    if !ANISOTROPY_VALUES.contains(&settings.anisotropy) {
        return Err(invalid("Anisotropy must be off, 4x, 8x, or 16x."));
    }
    let object_ok = settings.object_detail.as_index() <= ObjectDetail::VeryHigh.as_index();
    let enums_ok = settings.screen_mode.as_index() <= ScreenMode::BorderlessDesktop.as_index()
        && settings.texture_detail.as_index() <= TextureDetail::High.as_index()
        && object_ok
        && settings.difficulty.as_index() <= Difficulty::Hard.as_index()
        && settings.control_mode.as_index() <= ControlMode::Modern.as_index()
        && settings.render_backend.as_index() <= RenderBackend::Vulkan.as_index();
    if !enums_ok {
        return Err(invalid("Launcher settings contain an invalid selection."));
    }
    Ok(())
}

fn volume_ok(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// Membership test for the accepted discrete choices.
pub fn is_discrete(value: f64, accepted: &[f64]) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    accepted
        .iter()
        .copied()
        .find(|candidate| *candidate == value)
}

// ---------------------------------------------------------- setter layer

impl Settings {
    /// Validated resolution setter (`HP2MacLauncher.mm`
    /// MinimumResolutionWidth/Height = 320, MaximumResolutionDimension =
    /// 16384).
    pub fn set_resolution(&mut self, width: i32, height: i32) -> Result<()> {
        self.resolution.width = width;
        self.resolution.height = height;
        self.refresh_resolution_label();
        check(
            !(320..=16384).contains(&width) || !(320..=16384).contains(&height),
            "Resolution dimensions must each be between 320 and 16384.",
        )
    }

    /// Validated brightness setter (0.1–1.0; NaN rejected).
    pub fn set_brightness(&mut self, value: f64) -> Result<()> {
        check(
            !value.is_finite() || !(0.1..=1.0).contains(&value),
            "Brightness must be between 0.1 and 1.0.",
        )?;
        self.brightness = value;
        Ok(())
    }

    /// Validated mouse-sensitivity setter (0.2–10.0).
    pub fn set_mouse_sensitivity(&mut self, value: f64) -> Result<()> {
        check(
            !value.is_finite() || !(0.2..=10.0).contains(&value),
            "Mouse sensitivity must be between 0.2 and 10.0.",
        )?;
        self.mouse_sensitivity = value;
        Ok(())
    }

    /// Validated render-scale setter (discrete choices only).
    pub fn set_render_scale(&mut self, value: f64) -> Result<()> {
        check(
            is_discrete(value, &RENDER_SCALES).is_none(),
            "Render scale must be 0.50, 0.67, 0.75, 0.85, or 1.00.",
        )?;
        self.render_scale = value;
        Ok(())
    }

    /// Validated UI-scale setter (discrete choices only).
    pub fn set_ui_scale(&mut self, value: f64) -> Result<()> {
        check(
            is_discrete(value, &UI_SCALES).is_none(),
            "UI scale must be 0.75, 1.00, 1.25, 1.50, 1.75, or 2.00.",
        )?;
        self.ui_scale = value;
        Ok(())
    }

    /// Validated sound-volume setter.
    pub fn set_sound_volume(&mut self, value: f64) -> Result<()> {
        set_volume_checked(self, value, true)
    }

    /// Validated music-volume setter.
    pub fn set_music_volume(&mut self, value: f64) -> Result<()> {
        set_volume_checked(self, value, false)
    }

    /// Validated frame-cap setter (discrete choices only).
    pub fn set_frame_rate_limit(&mut self, value: i32) -> Result<()> {
        check(
            !FRAME_RATE_LIMITS.contains(&value),
            "Frame rate limit must be unlimited, 30, 60, 120, or 144.",
        )?;
        self.frame_rate_limit = value;
        Ok(())
    }

    /// Validated MSAA setter.
    pub fn set_anti_aliasing_samples(&mut self, value: i32) -> Result<()> {
        check(
            !AA_SAMPLE_VALUES.contains(&value),
            "Anti-aliasing must be off, 2x MSAA, or 4x MSAA.",
        )?;
        self.anti_aliasing_samples = value;
        Ok(())
    }

    /// Validated anisotropy setter.
    pub fn set_anisotropy(&mut self, value: i32) -> Result<()> {
        check(
            !ANISOTROPY_VALUES.contains(&value),
            "Anisotropy must be off, 4x, 8x, or 16x.",
        )?;
        self.anisotropy = value;
        Ok(())
    }
}

fn check(rejected: bool, message: &'static str) -> Result<()> {
    if rejected {
        Err(AppError::new("app.settings_invalid", message))
    } else {
        Ok(())
    }
}

fn set_volume_checked(settings: &mut Settings, value: f64, sound: bool) -> Result<()> {
    if !volume_ok(value) {
        return Err(AppError::new(
            "app.settings_invalid",
            "Sound and music volumes must be between 0.0 and 1.0.",
        ));
    }
    if sound {
        settings.sound_volume = value;
    } else {
        settings.music_volume = value;
    }
    Ok(())
}

// -------------------------------------------------------------------- io

fn io_error(path: &Path, error: std::io::Error) -> AppError {
    AppError::new(
        "app.settings_io_failed",
        format!("Unable to read '{}': {error}", path.display()),
    )
}

/// Selects the writable user document when present, else the immutable
/// system template (`SelectDocument`). Errors loudly when neither exists.
fn select_document(user_path: &Path, default_path: &Path) -> Result<crate::ini_text::IniDoc> {
    match std::fs::read(user_path) {
        Ok(bytes) => Ok(parse_ini(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let bytes = std::fs::read(default_path).map_err(|default_error| {
                if default_error.kind() == std::io::ErrorKind::NotFound {
                    AppError::new(
                        "app.settings_io_failed",
                        format!(
                            "Neither '{}' nor template '{}' exists",
                            user_path.display(),
                            default_path.display()
                        ),
                    )
                } else {
                    io_error(default_path, default_error)
                }
            })?;
            Ok(parse_ini(&bytes))
        }
        Err(error) => Err(io_error(user_path, error)),
    }
}

/// Loads the settings model for a profile: renderer-era keys come from
/// user `Game.ini` overlaying the system `Default.ini` template; player
/// keys from user `User.ini` overlaying `DefUser.ini` (`LoadSettings`).
pub fn load(system_root: &Path, profile_root: &Path) -> Result<Settings> {
    let game = select_document(
        &profile_root.join("Game.ini"),
        &system_root.join("Default.ini"),
    )?;
    let user = select_document(
        &profile_root.join("User.ini"),
        &system_root.join("DefUser.ini"),
    )?;

    let mut settings = Settings::default();

    // Screen mode consolidation: borderless/desktop overrides fullscreen.
    let fullscreen = get_bool(&game, "SDLDrv.SDLClient", "StartupFullscreen");
    let borderless = get_bool(&game, "SDLDrv.SDLClient", "BorderlessWindow");
    let desktop = get_bool(&game, "SDLDrv.SDLClient", "UseDesktopResolution");
    if borderless.unwrap_or(false) || desktop.unwrap_or(false) {
        settings.screen_mode = ScreenMode::BorderlessDesktop;
    } else if fullscreen.unwrap_or(false) {
        settings.screen_mode = ScreenMode::Fullscreen;
    }
    let full_size = settings.screen_mode != ScreenMode::Windowed;
    let (width_key, height_key) = if full_size {
        ("FullscreenViewportX", "FullscreenViewportY")
    } else {
        ("WindowedViewportX", "WindowedViewportY")
    };
    if let Some(width) = get_dimension(&game, width_key) {
        settings.resolution.width = width;
    }
    if let Some(height) = get_dimension(&game, height_key) {
        settings.resolution.height = height;
    }
    settings.refresh_resolution_label();

    if let Some(device) = get(&game, "Engine.Engine", "GameRenderDevice") {
        settings.render_backend = if device
            .trim()
            .eq_ignore_ascii_case("VulkanDrv.VulkanRenderDevice")
        {
            RenderBackend::Vulkan
        } else {
            RenderBackend::XOpenGL
        };
    }
    if let Some(enabled) = get_bool(&game, "XOpenGLDrv.XOpenGLRenderDevice", "UseVSync") {
        settings.vertical_sync = enabled;
    }
    if let Some(scale) = get_discrete_float(
        &game,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        &RENDER_SCALES,
    ) {
        settings.render_scale = scale;
    }
    if let Some(scale) = get_discrete_float(&game, "SDLDrv.SDLClient", "UIScale", &UI_SCALES) {
        settings.ui_scale = scale;
    }
    if let Some(enabled) = get_bool(&game, "SDLDrv.SDLClient", "ShowFPS") {
        settings.show_fps = enabled;
    }
    if let Some(enabled) = get_bool(&game, "SDLDrv.SDLClient", "MaintainVerticalFOV") {
        settings.maintain_vertical_fov = enabled;
    }
    if let Some(enabled) = get_bool(&game, "SDLDrv.SDLClient", "NativeText") {
        settings.native_text = enabled;
    }
    if let Some(enabled) = get_bool(&game, "SDLDrv.SDLClient", "ScreenFlashes") {
        settings.screen_flashes = enabled;
    }
    if let Some(brightness) = get_ranged_float(&game, "SDLDrv.SDLClient", "Brightness", 0.1, 1.0) {
        settings.brightness = brightness;
    }
    load_anti_aliasing(&game, &mut settings);
    if let Some(anisotropy) = get_discrete_integer(
        &game,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "MaxAnisotropy",
        &ANISOTROPY_VALUES,
    ) {
        settings.anisotropy = anisotropy;
    }
    if let Some(enabled) = get_bool(&game, "SDLDrv.SDLClient", "UseJoystick") {
        settings.joystick_enabled = enabled;
    }
    if let Some(enabled) = get_bool(&game, "Engine.GameEngine", "UseSound") {
        settings.sound_enabled = enabled;
    }
    if let Some(volume) =
        get_ranged_float(&game, "ALAudio.ALAudioSubsystem", "SoundVolume", 0.0, 1.0)
    {
        settings.sound_volume = volume;
    }
    if let Some(volume) =
        get_ranged_float(&game, "ALAudio.ALAudioSubsystem", "MusicVolume", 0.0, 1.0)
    {
        settings.music_volume = volume;
    }
    if let Some(cap) = frame_cap(&game) {
        settings.frame_rate_limit = cap;
    }
    const TEXTURE_SPELLINGS: [&str; 3] = ["Low", "Medium", "High"];
    if let Some(index) = enum_index(
        &game,
        "SDLDrv.SDLClient",
        "TextureDetail",
        &TEXTURE_SPELLINGS,
    ) {
        settings.texture_detail = TextureDetail::from_index(index);
    }
    if let Some(sensitivity) =
        get_ranged_float(&user, "Engine.PlayerPawn", "MouseSensitivity", 0.2, 10.0)
    {
        settings.mouse_sensitivity = sensitivity;
    }
    if let Some(inverted) = get_bool(&user, "Engine.PlayerPawn", "bInvertMouse") {
        settings.invert_mouse = inverted;
    }
    if let Some(modern) = get_bool(&user, "Engine.PlayerPawn", "bModernThirdPersonControls") {
        settings.control_mode = if modern {
            ControlMode::Modern
        } else {
            ControlMode::Classic
        };
    }
    if let Some(enabled) = get_bool(&user, "HGame.Harry", "bAutoCenterCamera") {
        settings.auto_center_camera = enabled;
    }
    if let Some(enabled) = get_bool(&user, "HGame.Harry", "bMoveWhileCasting") {
        settings.move_while_casting = enabled;
    }
    if let Some(enabled) = get_bool(&user, "HGame.Harry", "bAutoQuaff") {
        settings.auto_quaff = enabled;
    }
    const DIFFICULTY_SPELLINGS: [&str; 3] =
        ["DifficultyEasy", "DifficultyMedium", "DifficultyHard"];
    if let Some(index) = enum_index(
        &user,
        "Engine.PlayerPawn",
        "Difficulty",
        &DIFFICULTY_SPELLINGS,
    ) {
        settings.difficulty = Difficulty::from_index(index);
    }
    const OBJECT_SPELLINGS: [&str; 5] = [
        "ObjectDetailVeryLow",
        "ObjectDetailLow",
        "ObjectDetailMedium",
        "ObjectDetailHigh",
        "ObjectDetailVeryHigh",
    ];
    if let Some(index) = enum_index(
        &user,
        "Engine.PlayerPawn",
        "ObjectDetail",
        &OBJECT_SPELLINGS,
    ) {
        settings.object_detail = ObjectDetail::from_index(index);
    }
    Ok(settings)
}

fn get_dimension(doc: &crate::ini_text::IniDoc, key: &str) -> Option<i32> {
    let parsed = get_integer(doc, "SDLDrv.SDLClient", key)?;
    (320..=16384).contains(&parsed).then_some(parsed)
}

/// `LoadAntiAliasing`: a missing or unrecognized `UseAA` leaves any stale
/// sample count behind; disabled clears to zero.
fn load_anti_aliasing(doc: &crate::ini_text::IniDoc, settings: &mut Settings) {
    let section = "XOpenGLDrv.XOpenGLRenderDevice";
    let enabled = match get_bool(doc, section, "UseAA") {
        Some(enabled) => enabled,
        None => return,
    };
    if !enabled {
        settings.anti_aliasing_samples = 0;
        return;
    }
    if let Some(samples) = get_integer(doc, section, "NumAASamples")
        && (samples == 2 || samples == 4)
    {
        settings.anti_aliasing_samples = samples;
    }
}

fn frame_cap(doc: &crate::ini_text::IniDoc) -> Option<i32> {
    let cap = get_number(doc, "Engine.GameEngine", "FrameRateLimit")?;
    let integral = cap == cap.trunc() && (0.0..=f64::from(i32::MAX)).contains(&cap);
    let cap = integral.then_some(cap as i32)?;
    FRAME_RATE_LIMITS.contains(&cap).then_some(cap)
}

fn enum_index(
    doc: &crate::ini_text::IniDoc,
    section: &str,
    key: &str,
    spellings: &[&str],
) -> Option<usize> {
    let raw = get(doc, section, key)?;
    let trimmed = raw.trim();
    spellings
        .iter()
        .position(|spelling| trimmed.eq_ignore_ascii_case(spelling))
}

// ---------------------------------------------------------------- commit

fn texture_value(value: TextureDetail) -> &'static str {
    match value {
        TextureDetail::Low => "Low",
        TextureDetail::Medium => "Medium",
        TextureDetail::High => "High",
    }
}

fn difficulty_value(value: Difficulty) -> &'static str {
    match value {
        Difficulty::Easy => "DifficultyEasy",
        Difficulty::Medium => "DifficultyMedium",
        Difficulty::Hard => "DifficultyHard",
    }
}

fn object_value(value: ObjectDetail) -> &'static str {
    match value {
        ObjectDetail::VeryLow => "ObjectDetailVeryLow",
        ObjectDetail::Low => "ObjectDetailLow",
        ObjectDetail::Medium => "ObjectDetailMedium",
        ObjectDetail::High => "ObjectDetailHigh",
        ObjectDetail::VeryHigh => "ObjectDetailVeryHigh",
    }
}

fn render_device_class(value: RenderBackend) -> &'static str {
    match value {
        RenderBackend::XOpenGL => "XOpenGLDrv.XOpenGLRenderDevice",
        RenderBackend::Vulkan => "VulkanDrv.VulkanRenderDevice",
    }
}

/// `ApplyGame`: every launcher-owned renderer-era key.
fn apply_game(doc: &mut crate::ini_text::IniDoc, settings: &Settings) {
    doc.set_string("Engine.Engine", "ViewportManager", "SDLDrv.SDLClient");
    let device = render_device_class(settings.render_backend);
    doc.set_string("Engine.Engine", "GameRenderDevice", device);
    doc.set_string("Engine.Engine", "WindowedRenderDevice", device);
    doc.set_string("Engine.Engine", "RenderDevice", device);
    doc.set_string("Engine.Engine", "AudioDevice", "ALAudio.ALAudioSubsystem");
    doc.set_string(
        "Engine.GameEngine",
        "UseSound",
        bool_text(settings.sound_enabled),
    );
    doc.set_string(
        "Engine.GameEngine",
        "FrameRateLimit",
        settings.frame_rate_limit.to_string().as_str(),
    );
    let width = settings.resolution.width.to_string();
    let height = settings.resolution.height.to_string();
    for key in ["WindowedViewportX", "FullscreenViewportX"] {
        doc.set_string("SDLDrv.SDLClient", key, width.as_str());
    }
    for key in ["WindowedViewportY", "FullscreenViewportY"] {
        doc.set_string("SDLDrv.SDLClient", key, height.as_str());
    }
    doc.set_string("SDLDrv.SDLClient", "WindowedColorBits", "32");
    doc.set_string("SDLDrv.SDLClient", "FullscreenColorBits", "32");
    doc.set_string(
        "SDLDrv.SDLClient",
        "StartupFullscreen",
        bool_text(settings.screen_mode != ScreenMode::Windowed),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "BorderlessWindow",
        bool_text(settings.screen_mode == ScreenMode::BorderlessDesktop),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "UseDesktopResolution",
        bool_text(settings.screen_mode == ScreenMode::BorderlessDesktop),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "Brightness",
        double_text(settings.brightness).as_str(),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "TextureDetail",
        texture_value(settings.texture_detail),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "UseJoystick",
        bool_text(settings.joystick_enabled),
    );
    doc.set_string(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "UseVSync",
        if settings.vertical_sync { "On" } else { "Off" },
    );
    doc.set_string(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        double_text(settings.render_scale).as_str(),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "UIScale",
        double_text(settings.ui_scale).as_str(),
    );
    doc.set_string("SDLDrv.SDLClient", "ShowFPS", bool_text(settings.show_fps));
    doc.set_string(
        "SDLDrv.SDLClient",
        "MaintainVerticalFOV",
        bool_text(settings.maintain_vertical_fov),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "NativeText",
        bool_text(settings.native_text),
    );
    doc.set_string(
        "SDLDrv.SDLClient",
        "ScreenFlashes",
        bool_text(settings.screen_flashes),
    );
    doc.set_string(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "UseAA",
        bool_text(settings.anti_aliasing_samples != 0),
    );
    doc.set_string(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "NumAASamples",
        settings.anti_aliasing_samples.to_string().as_str(),
    );
    doc.set_string(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "MaxAnisotropy",
        settings.anisotropy.to_string().as_str(),
    );
    doc.set_string(
        "ALAudio.ALAudioSubsystem",
        "MusicVolume",
        double_text(settings.music_volume).as_str(),
    );
    doc.set_string(
        "ALAudio.ALAudioSubsystem",
        "SoundVolume",
        double_text(settings.sound_volume).as_str(),
    );
}

/// `ApplyUser`: every launcher-owned player key.
fn apply_user(doc: &mut crate::ini_text::IniDoc, settings: &Settings) {
    doc.set_string(
        "Engine.PlayerPawn",
        "MouseSensitivity",
        double_text(settings.mouse_sensitivity).as_str(),
    );
    doc.set_string(
        "Engine.PlayerPawn",
        "bInvertMouse",
        bool_text(settings.invert_mouse),
    );
    doc.set_string(
        "Engine.PlayerPawn",
        "bModernThirdPersonControls",
        bool_text(settings.control_mode == ControlMode::Modern),
    );
    doc.set_string(
        "Engine.PlayerPawn",
        "Difficulty",
        difficulty_value(settings.difficulty),
    );
    doc.set_string(
        "Engine.PlayerPawn",
        "ObjectDetail",
        object_value(settings.object_detail),
    );
    doc.set_string(
        "HGame.Harry",
        "bAutoCenterCamera",
        bool_text(settings.auto_center_camera),
    );
    doc.set_string(
        "HGame.Harry",
        "bMoveWhileCasting",
        bool_text(settings.move_while_casting),
    );
    doc.set_string("HGame.Harry", "bAutoQuaff", bool_text(settings.auto_quaff));
}

/// Commits settings for a profile: validates, then rewrites the launcher-
/// owned keys of `Game.ini`/`User.ini` (seeding from the system templates
/// when the writable documents do not exist yet) with one-time `.bak`
/// backups and atomic staged publication (`CommitLauncherSettings`).
pub fn commit(system_root: &Path, profile_root: &Path, settings: &Settings) -> Result<()> {
    validate(settings)?;
    if !crate::atomic_file::is_directory(profile_root) {
        return Err(AppError::new(
            "app.settings_io_failed",
            format!(
                "Writable user root is not a directory: '{}'",
                profile_root.display()
            ),
        ));
    }
    commit_one(
        &profile_root.join("Game.ini"),
        &system_root.join("Default.ini"),
        |doc| apply_game(doc, settings),
    )?;
    commit_one(
        &profile_root.join("User.ini"),
        &system_root.join("DefUser.ini"),
        |doc| apply_user(doc, settings),
    )?;
    Ok(())
}

fn commit_one(
    destination: &Path,
    template: &Path,
    apply: impl FnOnce(&mut crate::ini_text::IniDoc),
) -> Result<()> {
    let existing = std::fs::read(destination).ok().map(|bytes| {
        let mode = std::fs::metadata(destination)
            .map(|meta| std::os::unix::fs::PermissionsExt::mode(&meta.permissions()))
            .unwrap_or(0o600);
        (bytes, mode)
    });
    let (original_bytes, mode, mut doc, existed) = match existing {
        Some((bytes, mode)) => {
            let doc = parse_ini(&bytes);
            (Some(bytes), mode, doc, true)
        }
        None => {
            let bytes = std::fs::read(template).map_err(|error| io_error(template, error))?;
            let doc = parse_ini(&bytes);
            (None, 0o600, doc, false)
        }
    };
    apply(&mut doc);
    doc.force_dirty();
    let rendered = doc
        .render_if_dirty()
        .expect("forced-dirty document always renders");
    crate::atomic_file::publish(
        destination,
        &rendered,
        existed,
        original_bytes.as_deref(),
        mode,
    )
}
