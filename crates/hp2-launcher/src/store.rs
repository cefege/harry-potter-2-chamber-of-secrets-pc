//! Replaces the filesystem-independent half of `HP2LauncherStore.cpp`:
//! value parsing/formatting (`Integer`/`Index`/`Number`/`Boolean`/
//! `DoubleText`/`BoolText`/`ValidCap`), settings load/apply
//! (`LoadSettings`/`ApplyGame`/`ApplyUser` and their per-field
//! helpers), UTF-8 validation (`GetUtf8`/`IsValidUtf8`), the POSIX
//! branch of `IsAbsolutePath` (this project targets macOS/Linux only;
//! see `AGENTS.md`'s platform table -- the `HP2_WINDOWS` branch is not
//! ported), `SourceText`/`ParseSource`, and `ValidateLauncherSettings`.
//!
//! **Not in this module: every filesystem-touching function in
//! `HP2LauncherStore.cpp`** -- `Stage`/`Publish`/`Backup`/`Discard`/
//! `RestorePublishedFile` (atomic staged writes, see `atomic.rs`), the
//! `openat`/`O_NOFOLLOW`-based directory-fd traversal security
//! hardening (`OpenDirectoryNoFollow`, `OpenDirectoryAtNoFollow`,
//! `DirectoryNamesAt`, `CopyRegularFileAt`, `CopyLegacyTreeAt`,
//! `RemoveTreeAt`, and friends, see `tree.rs`), save discovery
//! (`DiscoverSaves` and its helpers, see `saves.rs`), profile
//! staging/publishing (`StageProfile`/`PublishProfile`/
//! `RemovePublishedProfile`), legacy-tree migration
//! (`MigrateLegacySettings` and its helpers), and the top-level
//! `Load`/`Commit*` orchestration functions that call all of the
//! above. This is deliberately scoped as its own, smaller, real-
//! filesystem-free increment, matching this crate's and
//! `hp2-import`'s established pattern of porting self-contained pure
//! logic before the security-sensitive filesystem layer underneath it
//! -- not a decision to skip that layer permanently.
//!
//! `Document`'s own `Get`/`Set`/`Erase`/`Decode`/`Encode` wrappers in
//! the real C++ are not replicated either: they are themselves already
//! thin calls into `crates/hp2-ini`'s `launcher::Document` (via
//! `hp2_rs.h`) in the real C++, so this crate calls
//! `hp2_ini::launcher::Document` directly -- one language, no FFI
//! marshaling needed for an in-process Rust-to-Rust call.

use hp2_ini::launcher::Document;

use crate::model::{
    ANISOTROPY_VALUES, ANTI_ALIASING_SAMPLE_VALUES, ControlMode, DataSource, Difficulty,
    FRAME_RATE_LIMIT_VALUES, LauncherSettings, ObjectDetail, RENDER_SCALE_VALUES, RenderBackend,
    ScreenMode, TextureDetail, UI_SCALE_VALUES,
};

/// Replaces `Lower`.
fn lower(value: u8) -> u8 {
    value.to_ascii_lowercase()
}

/// Replaces `Lowercase`.
pub(crate) fn lowercase(value: &str) -> String {
    value.bytes().map(lower).map(|b| b as char).collect()
}

/// Replaces `Same`: ASCII case-insensitive equality, byte for byte.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(x, y)| lower(x) == lower(y))
}

/// Replaces `Space`.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | 0x0b | 0x0c | b'\r' | b'\n')
}

/// Replaces `Trim`.
pub(crate) fn trim(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut begin = 0;
    let mut end = bytes.len();
    while begin < end && is_space(bytes[begin]) {
        begin += 1;
    }
    while end > begin && is_space(bytes[end - 1]) {
        end -= 1;
    }
    &value[begin..end]
}

/// Replaces `Integer`: a signed decimal integer, overflow-checked
/// against `i32`'s exact range (not truncated or saturated), leading
/// `+`/`-` allowed, surrounding whitespace trimmed first.
pub(crate) fn integer(text: &str) -> Option<i32> {
    let source = trim(text);
    if source.is_empty() {
        return None;
    }
    let bytes = source.as_bytes();
    let mut i = 0;
    let negative = match bytes[0] {
        b'+' => {
            i = 1;
            false
        }
        b'-' => {
            i = 1;
            true
        }
        _ => false,
    };
    if i == bytes.len() {
        return None;
    }
    let limit: i64 = i64::from(i32::MAX) + i64::from(negative);
    let mut result: i64 = 0;
    for &b in &bytes[i..] {
        if !b.is_ascii_digit() {
            return None;
        }
        let digit = i64::from(b - b'0');
        if result > (limit - digit) / 10 {
            return None;
        }
        result = result * 10 + digit;
    }
    Some(if negative { -result } else { result } as i32)
}

/// Replaces `Number`: parses the trimmed text as a finite `f64` with
/// nothing left over. `std::istringstream >> double` (classic locale)
/// extracts the longest valid floating-point prefix, and the real C++
/// then fails if anything beyond that remains (`stream >> trailing`
/// succeeding means leftover content, since `Trim` already stripped
/// surrounding whitespace before the stream ever saw the text). Rust's
/// `f64::from_str` requires the *entire* input to match the grammar in
/// one pass -- equivalent for every input: either the whole trimmed
/// string is one valid float token (both succeed, same value) or it
/// isn't (both fail), since a non-matching remainder after a valid
/// numeric prefix is a parse failure in Rust just as it is a "trailing
/// content" failure in C++. `is_finite()` rejects `inf`/`nan`, which
/// both grammars otherwise accept as tokens, matching the real C++'s
/// explicit `std::isfinite` check.
pub(crate) fn number(text: &str) -> Option<f64> {
    trim(text).parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Replaces `Boolean`.
pub(crate) fn boolean(text: &str) -> Option<bool> {
    match lowercase(trim(text)).as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// Replaces `DoubleText`: shortest-round-trip-independent, fixed
/// 15-significant-digit formatting matching
/// `std::setprecision(15) << value` in the classic locale -- Rust's
/// `{}` formatting for `f64` is shortest-round-trip (different digit
/// count than C++'s fixed precision for most values), so this needs
/// its own formatting, not a direct `to_string()`.
pub(crate) fn double_text(value: f64) -> String {
    format_general(value, 15)
}

/// Matches `operator<<(ostream&, double)`'s default (general) format
/// with `std::setprecision(digits)`: scientific notation when the
/// value's base-10 exponent is `< -4` or `>= digits` (the C `%g`
/// threshold this iostream mode follows), fixed notation otherwise;
/// trailing zeros (and a now-trailing decimal point) are always
/// stripped in both branches, matching the classic-locale default
/// formatting's own trailing-zero suppression. Most call sites in this
/// module pass a value validated to a narrow range (brightness
/// 0.1-1.0, mouse sensitivity 0.2-10.0, and so on) where the
/// scientific branch never fires -- but `soundVolume`/`musicVolume`
/// are validated only to `>= 0.0`, with no *lower* bound away from
/// zero, so an arbitrarily tiny positive volume genuinely can reach
/// it (confirmed directly: the real C++ renders `0.00001` as
/// `"1e-05"`, not `"0.00001"`). Both branches are verified against the
/// real compiled `DoubleText`, not just the fixed-notation one.
fn format_general(value: f64, digits: i32) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    // The true base-10 exponent comes from Rust's own scientific-
    // notation formatting (a correct float-to-decimal conversion), not
    // `value.abs().log10().floor()`: that floating-point computation's
    // own rounding error can be off by one near a power-of-10 boundary
    // -- confirmed directly, a real bug caught by this port's own
    // tests against the real compiled `DoubleText`:
    // `999999999999999.0_f64.log10().floor()` evaluates to `15`, but
    // the value's true exponent is `14` (`format!("{:e}", ...)`
    // correctly gives `9.99999999999999e14`), which flipped the
    // threshold check and produced scientific notation for a value the
    // real C++ renders in fixed notation.
    let full_scientific = format!("{value:e}");
    let (_, exp_str) = full_scientific
        .split_once('e')
        .expect("e-formatted float has an 'e'");
    let exponent: i32 = exp_str.parse().expect("exponent is a valid integer");
    if exponent < -4 || exponent >= digits {
        let mantissa_decimals = (digits - 1).max(0) as usize;
        let formatted = format!("{value:.mantissa_decimals$e}");
        let (mantissa, exp) = formatted
            .split_once('e')
            .expect("e-formatted float has an 'e'");
        let exp_value: i32 = exp.parse().unwrap_or(0);
        format!(
            "{}e{}{:02}",
            strip_trailing_zeros(mantissa),
            if exp_value < 0 { "-" } else { "+" },
            exp_value.abs()
        )
    } else {
        let decimals = (digits - 1 - exponent).max(0) as usize;
        strip_trailing_zeros(&format!("{value:.decimals$}"))
    }
}

fn strip_trailing_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    let mut text = text.to_string();
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

/// Replaces `BoolText`.
pub(crate) fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Replaces `ValidCap`.
fn valid_cap(value: i32) -> bool {
    FRAME_RATE_LIMIT_VALUES.contains(&value)
}

fn load_bool(doc: &Document, section: &str, key: &str, target: &mut bool) {
    if let Some(text) = doc.get(section, key)
        && let Some(parsed) = boolean(&text)
    {
        *target = parsed;
    }
}

fn load_number(doc: &Document, section: &str, key: &str, min: f64, max: f64, target: &mut f64) {
    if let Some(text) = doc.get(section, key)
        && let Some(parsed) = number(&text)
        && parsed >= min
        && parsed <= max
    {
        *target = parsed;
    }
}

fn load_discrete_number(
    doc: &Document,
    section: &str,
    key: &str,
    accepted: &[f64],
    target: &mut f64,
) {
    if let Some(text) = doc.get(section, key)
        && let Some(parsed) = number(&text)
        && accepted.contains(&parsed)
    {
        *target = parsed;
    }
}

fn load_discrete_integer(
    doc: &Document,
    section: &str,
    key: &str,
    accepted: &[i32],
    target: &mut i32,
) {
    let Some(text) = doc.get(section, key) else {
        return;
    };
    let Some(parsed) = number(&text) else {
        return;
    };
    if parsed != parsed.floor() || parsed < f64::from(i32::MIN) || parsed > f64::from(i32::MAX) {
        return;
    }
    let discrete = parsed as i32;
    if accepted.contains(&discrete) {
        *target = discrete;
    }
}

fn load_anti_aliasing(doc: &Document, target: &mut i32) {
    let Some(text) = doc.get("XOpenGLDrv.XOpenGLRenderDevice", "UseAA") else {
        return;
    };
    let Some(enabled) = boolean(&text) else {
        return;
    };
    if !enabled {
        *target = 0;
        return;
    }
    if let Some(text) = doc.get("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples")
        && let Some(samples) = integer(&text)
        && (samples == 2 || samples == 4)
    {
        *target = samples;
    }
}

fn load_dimension(doc: &Document, key: &str, target: &mut i32) {
    if let Some(text) = doc.get("SDLDrv.SDLClient", key)
        && let Some(parsed) = integer(&text)
        && (320..=16384).contains(&parsed)
    {
        *target = parsed;
    }
}

/// Replaces `LoadSettings`.
pub fn load_settings(game: &Document, user: &Document) -> LauncherSettings {
    let mut settings = LauncherSettings::default();
    let mut fullscreen = false;
    let mut borderless = false;
    let mut desktop = false;
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "StartupFullscreen",
        &mut fullscreen,
    );
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "BorderlessWindow",
        &mut borderless,
    );
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "UseDesktopResolution",
        &mut desktop,
    );
    if borderless || desktop {
        settings.screen_mode = ScreenMode::BorderlessDesktop;
    } else if fullscreen {
        settings.screen_mode = ScreenMode::Fullscreen;
    }
    let full_size = settings.screen_mode != ScreenMode::Windowed;
    load_dimension(
        game,
        if full_size {
            "FullscreenViewportX"
        } else {
            "WindowedViewportX"
        },
        &mut settings.resolution.width,
    );
    load_dimension(
        game,
        if full_size {
            "FullscreenViewportY"
        } else {
            "WindowedViewportY"
        },
        &mut settings.resolution.height,
    );
    settings.resolution.label = format!(
        "{} x {}",
        settings.resolution.width, settings.resolution.height
    );
    if let Some(render_device) = game.get("Engine.Engine", "GameRenderDevice") {
        settings.render_backend = if same(trim(&render_device), "VulkanDrv.VulkanRenderDevice") {
            RenderBackend::Vulkan
        } else {
            RenderBackend::XOpenGL
        };
    }
    load_bool(
        game,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "UseVSync",
        &mut settings.vertical_sync,
    );
    load_discrete_number(
        game,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        &RENDER_SCALE_VALUES,
        &mut settings.render_scale,
    );
    load_discrete_number(
        game,
        "SDLDrv.SDLClient",
        "UIScale",
        &UI_SCALE_VALUES,
        &mut settings.ui_scale,
    );
    load_bool(game, "SDLDrv.SDLClient", "ShowFPS", &mut settings.show_fps);
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "MaintainVerticalFOV",
        &mut settings.maintain_vertical_fov,
    );
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "NativeText",
        &mut settings.native_text,
    );
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "ScreenFlashes",
        &mut settings.screen_flashes,
    );
    load_number(
        game,
        "SDLDrv.SDLClient",
        "Brightness",
        0.1,
        1.0,
        &mut settings.brightness,
    );
    load_anti_aliasing(game, &mut settings.anti_aliasing_samples);
    load_discrete_integer(
        game,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "MaxAnisotropy",
        &ANISOTROPY_VALUES,
        &mut settings.anisotropy,
    );
    load_bool(
        game,
        "SDLDrv.SDLClient",
        "UseJoystick",
        &mut settings.joystick_enabled,
    );
    load_bool(
        game,
        "Engine.GameEngine",
        "UseSound",
        &mut settings.sound_enabled,
    );
    load_number(
        game,
        "ALAudio.ALAudioSubsystem",
        "SoundVolume",
        0.0,
        1.0,
        &mut settings.sound_volume,
    );
    load_number(
        game,
        "ALAudio.ALAudioSubsystem",
        "MusicVolume",
        0.0,
        1.0,
        &mut settings.music_volume,
    );
    if let Some(value) = game.get("Engine.GameEngine", "FrameRateLimit")
        && let Some(cap) = number(&value)
        && cap == cap.floor()
        && cap >= 0.0
        && cap <= f64::from(i32::MAX)
        && valid_cap(cap as i32)
    {
        settings.frame_rate_limit = cap as i32;
    }
    if let Some(value) = game.get("SDLDrv.SDLClient", "TextureDetail") {
        let value = trim(&value);
        if same(value, "Low") {
            settings.texture_detail = TextureDetail::Low;
        } else if same(value, "Medium") {
            settings.texture_detail = TextureDetail::Medium;
        } else if same(value, "High") {
            settings.texture_detail = TextureDetail::High;
        }
    }
    load_number(
        user,
        "Engine.PlayerPawn",
        "MouseSensitivity",
        0.2,
        10.0,
        &mut settings.mouse_sensitivity,
    );
    load_bool(
        user,
        "Engine.PlayerPawn",
        "bInvertMouse",
        &mut settings.invert_mouse,
    );
    let mut modern_third_person_controls = false;
    load_bool(
        user,
        "Engine.PlayerPawn",
        "bModernThirdPersonControls",
        &mut modern_third_person_controls,
    );
    settings.control_mode = if modern_third_person_controls {
        ControlMode::Modern
    } else {
        ControlMode::Classic
    };
    load_bool(
        user,
        "HGame.Harry",
        "bAutoCenterCamera",
        &mut settings.auto_center_camera,
    );
    load_bool(
        user,
        "HGame.Harry",
        "bMoveWhileCasting",
        &mut settings.move_while_casting,
    );
    load_bool(user, "HGame.Harry", "bAutoQuaff", &mut settings.auto_quaff);
    if let Some(value) = user.get("Engine.PlayerPawn", "Difficulty") {
        let value = trim(&value);
        if same(value, "DifficultyEasy") {
            settings.difficulty = Difficulty::Easy;
        } else if same(value, "DifficultyMedium") {
            settings.difficulty = Difficulty::Medium;
        } else if same(value, "DifficultyHard") {
            settings.difficulty = Difficulty::Hard;
        }
    }
    if let Some(value) = user.get("Engine.PlayerPawn", "ObjectDetail") {
        let value = trim(&value);
        if same(value, "ObjectDetailVeryLow") {
            settings.object_detail = ObjectDetail::VeryLow;
        } else if same(value, "ObjectDetailLow") {
            settings.object_detail = ObjectDetail::Low;
        } else if same(value, "ObjectDetailMedium") {
            settings.object_detail = ObjectDetail::Medium;
        } else if same(value, "ObjectDetailHigh") {
            settings.object_detail = ObjectDetail::High;
        } else if same(value, "ObjectDetailVeryHigh") {
            settings.object_detail = ObjectDetail::VeryHigh;
        }
    }
    settings
}

fn texture_text(value: TextureDetail) -> &'static str {
    match value {
        TextureDetail::Low => "Low",
        TextureDetail::Medium => "Medium",
        TextureDetail::High => "High",
    }
}

fn object_text(value: ObjectDetail) -> &'static str {
    match value {
        ObjectDetail::VeryLow => "ObjectDetailVeryLow",
        ObjectDetail::Low => "ObjectDetailLow",
        ObjectDetail::Medium => "ObjectDetailMedium",
        ObjectDetail::High => "ObjectDetailHigh",
        ObjectDetail::VeryHigh => "ObjectDetailVeryHigh",
    }
}

fn difficulty_text(value: Difficulty) -> &'static str {
    match value {
        Difficulty::Easy => "DifficultyEasy",
        Difficulty::Medium => "DifficultyMedium",
        Difficulty::Hard => "DifficultyHard",
    }
}

fn render_device_class(value: RenderBackend) -> &'static str {
    match value {
        RenderBackend::XOpenGL => "XOpenGLDrv.XOpenGLRenderDevice",
        RenderBackend::Vulkan => "VulkanDrv.VulkanRenderDevice",
    }
}

/// Replaces `ApplyGame`.
pub fn apply_game(game: &mut Document, settings: &LauncherSettings) {
    game.set("Engine.Engine", "ViewportManager", "SDLDrv.SDLClient");
    let render_device = render_device_class(settings.render_backend);
    game.set("Engine.Engine", "GameRenderDevice", render_device);
    game.set("Engine.Engine", "WindowedRenderDevice", render_device);
    game.set("Engine.Engine", "RenderDevice", render_device);
    game.set("Engine.Engine", "AudioDevice", "ALAudio.ALAudioSubsystem");
    game.set(
        "Engine.GameEngine",
        "UseSound",
        bool_text(settings.sound_enabled),
    );
    game.set(
        "Engine.GameEngine",
        "FrameRateLimit",
        &settings.frame_rate_limit.to_string(),
    );
    let width = settings.resolution.width.to_string();
    let height = settings.resolution.height.to_string();
    game.set("SDLDrv.SDLClient", "WindowedViewportX", &width);
    game.set("SDLDrv.SDLClient", "WindowedViewportY", &height);
    game.set("SDLDrv.SDLClient", "FullscreenViewportX", &width);
    game.set("SDLDrv.SDLClient", "FullscreenViewportY", &height);
    game.set("SDLDrv.SDLClient", "WindowedColorBits", "32");
    game.set("SDLDrv.SDLClient", "FullscreenColorBits", "32");
    game.set(
        "SDLDrv.SDLClient",
        "StartupFullscreen",
        bool_text(settings.screen_mode != ScreenMode::Windowed),
    );
    game.set(
        "SDLDrv.SDLClient",
        "BorderlessWindow",
        bool_text(settings.screen_mode == ScreenMode::BorderlessDesktop),
    );
    game.set(
        "SDLDrv.SDLClient",
        "UseDesktopResolution",
        bool_text(settings.screen_mode == ScreenMode::BorderlessDesktop),
    );
    game.set(
        "SDLDrv.SDLClient",
        "Brightness",
        &double_text(settings.brightness),
    );
    game.set(
        "SDLDrv.SDLClient",
        "TextureDetail",
        texture_text(settings.texture_detail),
    );
    game.set(
        "SDLDrv.SDLClient",
        "UseJoystick",
        bool_text(settings.joystick_enabled),
    );
    game.set(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "UseVSync",
        if settings.vertical_sync { "On" } else { "Off" },
    );
    game.set(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        &double_text(settings.render_scale),
    );
    game.set(
        "SDLDrv.SDLClient",
        "UIScale",
        &double_text(settings.ui_scale),
    );
    game.set("SDLDrv.SDLClient", "ShowFPS", bool_text(settings.show_fps));
    game.set(
        "SDLDrv.SDLClient",
        "MaintainVerticalFOV",
        bool_text(settings.maintain_vertical_fov),
    );
    game.set(
        "SDLDrv.SDLClient",
        "NativeText",
        bool_text(settings.native_text),
    );
    game.set(
        "SDLDrv.SDLClient",
        "ScreenFlashes",
        bool_text(settings.screen_flashes),
    );
    game.set(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "UseAA",
        bool_text(settings.anti_aliasing_samples != 0),
    );
    game.set(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "NumAASamples",
        &settings.anti_aliasing_samples.to_string(),
    );
    game.set(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "MaxAnisotropy",
        &settings.anisotropy.to_string(),
    );
    game.set(
        "ALAudio.ALAudioSubsystem",
        "MusicVolume",
        &double_text(settings.music_volume),
    );
    game.set(
        "ALAudio.ALAudioSubsystem",
        "SoundVolume",
        &double_text(settings.sound_volume),
    );
}

/// Replaces `ApplyUser`.
pub fn apply_user(user: &mut Document, settings: &LauncherSettings) {
    user.set(
        "Engine.PlayerPawn",
        "MouseSensitivity",
        &double_text(settings.mouse_sensitivity),
    );
    user.set(
        "Engine.PlayerPawn",
        "bInvertMouse",
        bool_text(settings.invert_mouse),
    );
    user.set(
        "Engine.PlayerPawn",
        "bModernThirdPersonControls",
        bool_text(settings.control_mode == ControlMode::Modern),
    );
    user.set(
        "Engine.PlayerPawn",
        "Difficulty",
        difficulty_text(settings.difficulty),
    );
    user.set(
        "Engine.PlayerPawn",
        "ObjectDetail",
        object_text(settings.object_detail),
    );
    user.set(
        "HGame.Harry",
        "bAutoCenterCamera",
        bool_text(settings.auto_center_camera),
    );
    user.set(
        "HGame.Harry",
        "bMoveWhileCasting",
        bool_text(settings.move_while_casting),
    );
    user.set("HGame.Harry", "bAutoQuaff", bool_text(settings.auto_quaff));
}

/// Replaces `GetUtf8`: decodes one UTF-8 code point starting at byte
/// offset `i`, advancing `i` past it. `None` for any malformed
/// sequence (short input, bad continuation byte, overlong encoding, a
/// surrogate code point, or an out-of-range value) -- matches the real
/// C++'s manual bit-twiddling decoder exactly, including which specific
/// malformed shapes it rejects.
fn get_utf8(bytes: &[u8], i: &mut usize) -> Option<u32> {
    let first = bytes[*i] as u32;
    *i += 1;
    if first < 0x80 {
        return Some(first);
    }
    let (count, mut cp, minimum) = if first & 0xe0 == 0xc0 {
        (1, first & 0x1f, 0x80u32)
    } else if first & 0xf0 == 0xe0 {
        (2, first & 0x0f, 0x800u32)
    } else if first & 0xf8 == 0xf0 {
        (3, first & 0x07, 0x10000u32)
    } else {
        return None;
    };
    if *i + count > bytes.len() {
        return None;
    }
    for _ in 0..count {
        let next = bytes[*i] as u32;
        *i += 1;
        if next & 0xc0 != 0x80 {
            return None;
        }
        cp = (cp << 6) | (next & 0x3f);
    }
    if cp >= minimum && cp <= 0x10ffff && !(0xd800..=0xdfff).contains(&cp) {
        Some(cp)
    } else {
        None
    }
}

/// Replaces `IsValidUtf8`.
pub fn is_valid_utf8(text: &[u8]) -> bool {
    let mut offset = 0;
    while offset < text.len() {
        if get_utf8(text, &mut offset).is_none() {
            return false;
        }
    }
    true
}

/// Replaces the POSIX branch of `IsAbsolutePath` (this project targets
/// macOS/Linux only; the `HP2_WINDOWS` branch is not ported -- see
/// `AGENTS.md`'s platform table).
pub fn is_absolute_path(path: &str) -> bool {
    !path.is_empty() && path.as_bytes()[0] == b'/' && !path.as_bytes().contains(&0)
}

/// Replaces `SourceText`.
pub fn source_text(source: DataSource) -> &'static str {
    match source {
        DataSource::Retail => "Retail",
        DataSource::Prototype => "Prototype",
    }
}

/// Replaces `ParseSource`.
pub fn parse_source(text: &str) -> Option<DataSource> {
    if same(text, "Retail") {
        Some(DataSource::Retail)
    } else if same(text, "Prototype") {
        Some(DataSource::Prototype)
    } else {
        None
    }
}

/// Replaces `ValidateLauncherSettings`.
pub fn validate_launcher_settings(settings: &LauncherSettings) -> Result<(), String> {
    if !(320..=16384).contains(&settings.resolution.width)
        || !(320..=16384).contains(&settings.resolution.height)
    {
        return Err("Resolution dimensions must each be between 320 and 16384.".to_string());
    }
    if !settings.brightness.is_finite() || settings.brightness < 0.1 || settings.brightness > 1.0 {
        return Err("Brightness must be between 0.1 and 1.0.".to_string());
    }
    if !settings.render_scale.is_finite() || !RENDER_SCALE_VALUES.contains(&settings.render_scale) {
        return Err("Render scale must be 0.50, 0.67, 0.75, 0.85, or 1.00.".to_string());
    }
    if !settings.ui_scale.is_finite() || !UI_SCALE_VALUES.contains(&settings.ui_scale) {
        return Err("UI scale must be 0.75, 1.00, 1.25, 1.50, 1.75, or 2.00.".to_string());
    }
    if !settings.mouse_sensitivity.is_finite()
        || settings.mouse_sensitivity < 0.2
        || settings.mouse_sensitivity > 10.0
    {
        return Err("Mouse sensitivity must be between 0.2 and 10.0.".to_string());
    }
    if !settings.sound_volume.is_finite()
        || !(0.0..=1.0).contains(&settings.sound_volume)
        || !settings.music_volume.is_finite()
        || !(0.0..=1.0).contains(&settings.music_volume)
    {
        return Err("Sound and music volumes must be between 0.0 and 1.0.".to_string());
    }
    if !valid_cap(settings.frame_rate_limit) {
        return Err("Frame rate limit must be unlimited, 30, 60, 120, or 144.".to_string());
    }
    if !ANTI_ALIASING_SAMPLE_VALUES.contains(&settings.anti_aliasing_samples) {
        return Err("Anti-aliasing must be off, 2x MSAA, or 4x MSAA.".to_string());
    }
    if !ANISOTROPY_VALUES.contains(&settings.anisotropy) {
        return Err("Anisotropy must be off, 4x, 8x, or 16x.".to_string());
    }
    // The enum range checks (screen/texture/object/difficulty/control/
    // backend each within their valid discriminant range) have no
    // equivalent here: Rust's enums cannot hold an out-of-range
    // discriminant the way the C++ `static_cast<int>` on an enum class
    // could in principle observe one, so there is no invalid state left
    // to check for by the time a `LauncherSettings` value exists.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every case confirmed against the real compiled `Number`.
    #[test]
    fn number_matches_cpp_cases() {
        assert_eq!(number("5"), Some(5.0));
        assert_eq!(number("5.5"), Some(5.5));
        assert_eq!(number("-3.2e-1"), Some(-0.32));
        assert_eq!(number("5abc"), None);
        assert_eq!(number("  5  "), Some(5.0));
        assert_eq!(number("inf"), None);
        assert_eq!(number("nan"), None);
        assert_eq!(number(""), None);
        assert_eq!(number("."), None);
        assert_eq!(number("-"), None);
        assert_eq!(number("+5"), Some(5.0));
        assert_eq!(number("0.1"), Some(0.1));
        assert_eq!(number("1e10"), Some(1e10));
        assert_eq!(number("0"), Some(0.0));
    }

    /// Every case confirmed against the real compiled `Integer`,
    /// including the exact `i32::MIN`/`i32::MAX` overflow boundary.
    #[test]
    fn integer_matches_cpp_cases() {
        assert_eq!(integer("2147483647"), Some(i32::MAX));
        assert_eq!(integer("2147483648"), None);
        assert_eq!(integer("-2147483648"), Some(i32::MIN));
        assert_eq!(integer("-2147483649"), None);
        assert_eq!(integer("+5"), Some(5));
        assert_eq!(integer("  5  "), Some(5));
        assert_eq!(integer("5.5"), None);
        assert_eq!(integer(""), None);
        assert_eq!(integer("-"), None);
        assert_eq!(integer("0"), Some(0));
    }

    /// Every case confirmed against the real compiled `Boolean`.
    #[test]
    fn boolean_matches_cpp_cases() {
        assert_eq!(boolean("TRUE"), Some(true));
        assert_eq!(boolean("On"), Some(true));
        assert_eq!(boolean("yes"), Some(true));
        assert_eq!(boolean("1"), Some(true));
        assert_eq!(boolean("FALSE"), Some(false));
        assert_eq!(boolean("off"), Some(false));
        assert_eq!(boolean("no"), Some(false));
        assert_eq!(boolean("0"), Some(false));
        assert_eq!(boolean("bogus"), None);
        assert_eq!(boolean(""), None);
        assert_eq!(boolean("  true  "), Some(true));
    }

    /// Every case confirmed against the real compiled `DoubleText`,
    /// including the scientific-notation branch (`1e-05`-shaped
    /// output) this port's own doc comment originally, incorrectly,
    /// claimed was unreachable -- see `format_general`'s doc for the
    /// correction and why `soundVolume`/`musicVolume` can actually
    /// reach it.
    #[test]
    fn double_text_matches_cpp_cases() {
        assert_eq!(double_text(0.1), "0.1");
        assert_eq!(double_text(0.123456789012345), "0.123456789012345");
        assert_eq!(double_text(1.0), "1");
        assert_eq!(double_text(10.0), "10");
        assert_eq!(double_text(0.3333333333333333), "0.333333333333333");
        // Rounds up across a magnitude boundary: 15 significant digits
        // of 9.999999999999998 rounds to exactly 10.
        assert_eq!(double_text(9.999999999999998), "10");
        assert_eq!(double_text(0.53), "0.53");
        assert_eq!(double_text(16.0), "16");
        assert_eq!(double_text(0.0001), "0.0001");
        assert_eq!(double_text(0.00001), "1e-05");
        assert_eq!(double_text(123456789012345.0), "123456789012345");
        assert_eq!(double_text(0.2), "0.2");
        assert_eq!(double_text(0.00009999999999999), "9.999999999999e-05");
        assert_eq!(double_text(1e-100), "1e-100");
        assert_eq!(double_text(1.5e-10), "1.5e-10");
        assert_eq!(double_text(100000000000000.0), "100000000000000");
        assert_eq!(double_text(999999999999999.0), "999999999999999");
        assert_eq!(double_text(1000000000000000.0), "1e+15");
        assert_eq!(double_text(0.00009), "9e-05");
        assert_eq!(double_text(0.0), "0");
    }

    #[test]
    fn trim_and_same_match_cpp() {
        assert_eq!(trim("  hi  "), "hi");
        assert_eq!(trim("\t\r\n hi \x0b\x0c"), "hi");
        assert_eq!(trim(""), "");
        assert!(same("Retail", "retail"));
        assert!(same("RETAIL", "Retail"));
        assert!(!same("Retail", "Prototype"));
        assert!(!same("Retail", "Retai"));
    }

    #[test]
    fn is_valid_utf8_matches_cpp_cases() {
        assert!(is_valid_utf8(b"hello"));
        assert!(is_valid_utf8(b""));
        assert!(is_valid_utf8("héllo".as_bytes()));
        assert!(!is_valid_utf8(b"\xc3\x28"));
        assert!(!is_valid_utf8(b"\xed\xa0\x80"));
    }

    #[test]
    fn is_absolute_path_matches_cpp_cases() {
        assert!(is_absolute_path("/foo/bar"));
        assert!(!is_absolute_path("foo/bar"));
        assert!(!is_absolute_path(""));
        assert!(is_absolute_path("/"));
    }

    #[test]
    fn parse_source_matches_cpp_cases() {
        assert_eq!(parse_source("Retail"), Some(DataSource::Retail));
        assert_eq!(parse_source("retail"), Some(DataSource::Retail));
        assert_eq!(parse_source("PROTOTYPE"), Some(DataSource::Prototype));
        assert_eq!(parse_source("bogus"), None);
    }

    #[test]
    fn source_text_round_trips_through_parse_source() {
        for source in [DataSource::Retail, DataSource::Prototype] {
            assert_eq!(parse_source(source_text(source)), Some(source));
        }
    }

    #[test]
    fn validate_launcher_settings_matches_cpp_cases() {
        validate_launcher_settings(&LauncherSettings::default()).unwrap();
        let mut s = LauncherSettings {
            resolution: crate::model::DisplayResolution {
                width: 100,
                height: 600,
                label: String::new(),
            },
            ..Default::default()
        };
        assert_eq!(
            validate_launcher_settings(&s).unwrap_err(),
            "Resolution dimensions must each be between 320 and 16384."
        );
        s = LauncherSettings::default();
        s.render_scale = 0.6;
        assert_eq!(
            validate_launcher_settings(&s).unwrap_err(),
            "Render scale must be 0.50, 0.67, 0.75, 0.85, or 1.00."
        );
        s = LauncherSettings::default();
        s.frame_rate_limit = 50;
        assert_eq!(
            validate_launcher_settings(&s).unwrap_err(),
            "Frame rate limit must be unlimited, 30, 60, 120, or 144."
        );
        s = LauncherSettings::default();
        s.anti_aliasing_samples = 3;
        assert_eq!(
            validate_launcher_settings(&s).unwrap_err(),
            "Anti-aliasing must be off, 2x MSAA, or 4x MSAA."
        );
        s = LauncherSettings::default();
        s.anisotropy = 2;
        assert_eq!(
            validate_launcher_settings(&s).unwrap_err(),
            "Anisotropy must be off, 4x, 8x, or 16x."
        );
    }

    fn fixture_documents() -> (Document, Document) {
        let mut game = Document::new();
        game.set("SDLDrv.SDLClient", "StartupFullscreen", "False");
        game.set("SDLDrv.SDLClient", "BorderlessWindow", "False");
        game.set("SDLDrv.SDLClient", "UseDesktopResolution", "False");
        game.set("SDLDrv.SDLClient", "WindowedViewportX", "1024");
        game.set("SDLDrv.SDLClient", "WindowedViewportY", "768");
        game.set("SDLDrv.SDLClient", "ShowFPS", "True");
        game.set("SDLDrv.SDLClient", "MaintainVerticalFOV", "False");
        game.set("SDLDrv.SDLClient", "NativeText", "True");
        game.set("SDLDrv.SDLClient", "ScreenFlashes", "False");
        game.set("SDLDrv.SDLClient", "Brightness", "0.65");
        game.set("SDLDrv.SDLClient", "UseJoystick", "False");
        game.set(
            "Engine.Engine",
            "GameRenderDevice",
            "XOpenGLDrv.XOpenGLRenderDevice",
        );
        game.set("XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", "True");
        game.set("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.75");
        game.set("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "True");
        game.set("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "2");
        game.set("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "8");
        game.set("Engine.GameEngine", "UseSound", "True");
        game.set("Engine.GameEngine", "FrameRateLimit", "60");
        game.set("ALAudio.ALAudioSubsystem", "SoundVolume", "0.75");
        game.set("ALAudio.ALAudioSubsystem", "MusicVolume", "0.33");
        let mut user = Document::new();
        user.set("Engine.PlayerPawn", "MouseSensitivity", "5.5");
        user.set("Engine.PlayerPawn", "bInvertMouse", "True");
        user.set("Engine.PlayerPawn", "bModernThirdPersonControls", "True");
        user.set("Engine.PlayerPawn", "Difficulty", "DifficultyHard");
        user.set("Engine.PlayerPawn", "ObjectDetail", "ObjectDetailLow");
        user.set("HGame.Harry", "bAutoCenterCamera", "False");
        user.set("HGame.Harry", "bMoveWhileCasting", "False");
        user.set("HGame.Harry", "bAutoQuaff", "False");
        (game, user)
    }

    /// Confirmed against the real compiled `LoadSettings` on this exact
    /// fixture (built the same way via the real `Document`/`Set`, not
    /// hand-written INI text) -- every field, byte-identical.
    #[test]
    fn load_settings_matches_cpp_on_a_full_fixture() {
        let (game, user) = fixture_documents();
        let settings = load_settings(&game, &user);
        assert_eq!(settings.screen_mode, ScreenMode::Windowed);
        assert_eq!(settings.resolution.width, 1024);
        assert_eq!(settings.resolution.height, 768);
        assert_eq!(settings.render_backend, RenderBackend::XOpenGL);
        assert!(settings.vertical_sync);
        assert_eq!(settings.render_scale, 0.75);
        assert_eq!(settings.ui_scale, 1.0);
        assert!(settings.show_fps);
        assert!(!settings.maintain_vertical_fov);
        assert!(settings.native_text);
        assert_eq!(settings.anti_aliasing_samples, 2);
        assert_eq!(settings.anisotropy, 8);
        assert_eq!(settings.brightness, 0.65);
        assert_eq!(settings.texture_detail, TextureDetail::High);
        assert_eq!(settings.object_detail, ObjectDetail::Low);
        assert!(settings.sound_enabled);
        assert_eq!(settings.sound_volume, 0.75);
        assert_eq!(settings.music_volume, 0.33);
        assert_eq!(settings.mouse_sensitivity, 5.5);
        assert!(settings.invert_mouse);
        assert_eq!(settings.control_mode, ControlMode::Modern);
        assert!(!settings.auto_center_camera);
        assert!(!settings.move_while_casting);
        assert!(!settings.auto_quaff);
        assert!(!settings.screen_flashes);
        assert_eq!(settings.difficulty, Difficulty::Hard);
        assert!(!settings.joystick_enabled);
        assert_eq!(settings.frame_rate_limit, 60);
    }

    /// Confirmed against the real compiled `ApplyGame`/`ApplyUser`: a
    /// full `LoadSettings` -> `ApplyGame`/`ApplyUser` round trip on the
    /// same fixture produces byte-identical encoded documents (checked
    /// directly against the real compiled output via a throwaway probe
    /// before this test was written; this pins the same fixture's
    /// settings values and a representative sample of the applied
    /// fields rather than the full encoded text, which would be a
    /// brittle, header-order-coupled byte comparison for marginal extra
    /// coverage over the field-by-field checks already above).
    #[test]
    fn apply_game_and_apply_user_round_trip_matches_cpp() {
        let (game, user) = fixture_documents();
        let settings = load_settings(&game, &user);
        let mut game_out = Document::new();
        let mut user_out = Document::new();
        apply_game(&mut game_out, &settings);
        apply_user(&mut user_out, &settings);
        assert_eq!(
            game_out.get("Engine.Engine", "GameRenderDevice").as_deref(),
            Some("XOpenGLDrv.XOpenGLRenderDevice")
        );
        assert_eq!(
            game_out
                .get("SDLDrv.SDLClient", "WindowedViewportX")
                .as_deref(),
            Some("1024")
        );
        assert_eq!(
            game_out
                .get("XOpenGLDrv.XOpenGLRenderDevice", "UseVSync")
                .as_deref(),
            Some("On")
        );
        assert_eq!(
            game_out
                .get("ALAudio.ALAudioSubsystem", "MusicVolume")
                .as_deref(),
            Some("0.33")
        );
        assert_eq!(
            user_out.get("Engine.PlayerPawn", "Difficulty").as_deref(),
            Some("DifficultyHard")
        );
        assert_eq!(
            user_out
                .get("Engine.PlayerPawn", "bModernThirdPersonControls")
                .as_deref(),
            Some("True")
        );
    }

    /// `LoadSettings` on empty documents leaves every field at its
    /// `LauncherSettings::default()` value -- confirmed against the
    /// real compiled `LoadSettings` on two empty documents.
    #[test]
    fn load_settings_on_empty_documents_is_all_defaults() {
        let game = Document::new();
        let user = Document::new();
        let settings = load_settings(&game, &user);
        let defaults = LauncherSettings::default();
        assert_eq!(settings.screen_mode, defaults.screen_mode);
        assert_eq!(settings.resolution.width, defaults.resolution.width);
        assert_eq!(settings.resolution.height, defaults.resolution.height);
        assert_eq!(settings.frame_rate_limit, defaults.frame_rate_limit);
        assert_eq!(settings.brightness, defaults.brightness);
        assert_eq!(
            settings.anti_aliasing_samples,
            defaults.anti_aliasing_samples
        );
    }
}
