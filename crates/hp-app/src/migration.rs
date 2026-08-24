//! Pure legacy-schema settings migration, ported from
//! `HP2LauncherStore.cpp MigrateLegacySettings`.
//!
//! Consumes parsed INI rows and returns the canonical modern-schema rows
//! plus a journal entry (`settings.*` reason codes) per rewritten row. No
//! filesystem, locale, or global state access — fully table-testable and
//! idempotent.

use crate::ini_text::{bool_text, double_text};
use crate::settings_model::{ANISOTROPY_VALUES, FRAME_RATE_LIMITS, RENDER_SCALES, UI_SCALES};

/// One parsed INI row of legacy launcher configuration. Duplicate
/// section/key rows are allowed; lookups are case-insensitive final-wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRow {
    pub section: String,
    pub key: String,
    pub value: String,
}

impl LegacyRow {
    pub fn new(section: &str, key: &str, value: &str) -> Self {
        Self {
            section: section.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

/// One observable normalization; an empty `previous_value` means the key was
/// absent and is being materialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationChange {
    pub section: String,
    pub key: String,
    pub previous_value: String,
    pub new_value: String,
    pub reason: &'static str,
}

/// Case-insensitive equality over ASCII (the store's `Same`).
fn same(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn final_value<'a>(values: &'a [LegacyRow], section: &str, key: &str) -> Option<&'a str> {
    values
        .iter()
        .rev()
        .find(|row| same(&row.section, section) && same(&row.key, key))
        .map(|row| row.value.as_str())
}

/// Rewrites the final occurrence of a key in place (or appends) and journals
/// the change when it is observable.
fn rewrite(
    values: &mut Vec<LegacyRow>,
    changes: &mut Vec<MigrationChange>,
    section: &str,
    key: &str,
    next: String,
    reason: &'static str,
) {
    let previous = final_value(values, section, key).map(str::to_string);
    match values
        .iter_mut()
        .rposition(|row| same(&row.section, section) && same(&row.key, key))
    {
        Some(last) => values[last].value = next.clone(),
        None => values.push(LegacyRow::new(section, key, &next)),
    }
    if previous.as_deref() != Some(next.as_str()) {
        changes.push(MigrationChange {
            section: section.to_string(),
            key: key.to_string(),
            previous_value: previous.unwrap_or_default(),
            new_value: next,
            reason,
        });
    }
}

/// C-locale whole-value float parse.
fn number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

fn integer(text: &str) -> Option<i32> {
    let text = text.trim();
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

/// `Boolean`: true/on/yes/1 vs false/off/no/0.
fn boolean(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn valid_cap(value: i32) -> bool {
    FRAME_RATE_LIMITS.contains(&value)
}

struct OwnedBoolean {
    section: &'static str,
    key: &'static str,
    fallback: bool,
}

const OWNED_BOOLEANS: [OwnedBoolean; 16] = [
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "StartupFullscreen",
        fallback: false,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "BorderlessWindow",
        fallback: false,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "UseDesktopResolution",
        fallback: false,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "ShowFPS",
        fallback: false,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "MaintainVerticalFOV",
        fallback: true,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "NativeText",
        fallback: true,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "ScreenFlashes",
        fallback: true,
    },
    OwnedBoolean {
        section: "SDLDrv.SDLClient",
        key: "UseJoystick",
        fallback: true,
    },
    OwnedBoolean {
        section: "XOpenGLDrv.XOpenGLRenderDevice",
        key: "UseVSync",
        fallback: false,
    },
    OwnedBoolean {
        section: "XOpenGLDrv.XOpenGLRenderDevice",
        key: "UseAA",
        fallback: false,
    },
    OwnedBoolean {
        section: "Engine.GameEngine",
        key: "UseSound",
        fallback: true,
    },
    OwnedBoolean {
        section: "Engine.PlayerPawn",
        key: "bInvertMouse",
        fallback: false,
    },
    OwnedBoolean {
        section: "Engine.PlayerPawn",
        key: "bModernThirdPersonControls",
        fallback: false,
    },
    OwnedBoolean {
        section: "HGame.Harry",
        key: "bAutoCenterCamera",
        fallback: true,
    },
    OwnedBoolean {
        section: "HGame.Harry",
        key: "bMoveWhileCasting",
        fallback: true,
    },
    OwnedBoolean {
        section: "HGame.Harry",
        key: "bAutoQuaff",
        fallback: true,
    },
];

/// The pure migration entry point. Idempotent: rerunning on migrated output
/// yields identical rows with an empty journal.
pub fn migrate_legacy_settings(
    legacy_values: &[LegacyRow],
) -> (Vec<LegacyRow>, Vec<MigrationChange>) {
    let mut values = legacy_values.to_vec();
    let mut changes = Vec::new();

    // Owned booleans canonicalize to True/False; invalid spellings fall back
    // to the model default the loader would have used.
    for owned in OWNED_BOOLEANS.iter() {
        let Some(text) = final_value(&values, owned.section, owned.key) else {
            continue;
        };
        let parsed = boolean(text);
        let value = parsed.unwrap_or(owned.fallback);
        rewrite(
            &mut values,
            &mut changes,
            owned.section,
            owned.key,
            bool_text(value).to_string(),
            if parsed.is_some() {
                "settings.boolean_spelling"
            } else {
                "settings.boolean_invalid_defaulted"
            },
        );
    }

    // Screen mode: borderless/desktop overrides fullscreen, exactly as the
    // loader resolves the three legacy switches.
    let parse_mode_switch = |key: &str| -> Option<bool> {
        final_value(&values, "SDLDrv.SDLClient", key).and_then(boolean)
    };
    let startup_fullscreen = parse_mode_switch("StartupFullscreen");
    let borderless = parse_mode_switch("BorderlessWindow");
    let desktop_resolution = parse_mode_switch("UseDesktopResolution");
    if startup_fullscreen.is_some() || borderless.is_some() || desktop_resolution.is_some() {
        let borderless_desktop = borderless.unwrap_or(false) || desktop_resolution.unwrap_or(false);
        if let Some(fullscreen) = startup_fullscreen {
            rewrite(
                &mut values,
                &mut changes,
                "SDLDrv.SDLClient",
                "StartupFullscreen",
                bool_text(fullscreen && !borderless_desktop).to_string(),
                "settings.screen_mode_consolidated",
            );
        }
        if let Some(borderless_value) = borderless {
            rewrite(
                &mut values,
                &mut changes,
                "SDLDrv.SDLClient",
                "BorderlessWindow",
                bool_text(borderless_value || desktop_resolution.unwrap_or(false)).to_string(),
                "settings.screen_mode_consolidated",
            );
        }

        // Viewport quartet collapses onto the resolution the launcher would
        // actually run for the consolidated mode.
        let full_size = borderless_desktop || startup_fullscreen.unwrap_or(false);
        let (width_key, height_key) = if full_size {
            ("FullscreenViewportX", "FullscreenViewportY")
        } else {
            ("WindowedViewportX", "WindowedViewportY")
        };
        let mut width = 800;
        let mut height = 600;
        if let Some(text) = final_value(&values, "SDLDrv.SDLClient", width_key)
            && let Some(parsed) = integer(text)
            && (320..=16384).contains(&parsed)
        {
            width = parsed;
        }
        if let Some(text) = final_value(&values, "SDLDrv.SDLClient", height_key)
            && let Some(parsed) = integer(text)
            && (320..=16384).contains(&parsed)
        {
            height = parsed;
        }
        for viewport_key in [
            "WindowedViewportX",
            "WindowedViewportY",
            "FullscreenViewportX",
            "FullscreenViewportY",
        ] {
            if final_value(&values, "SDLDrv.SDLClient", viewport_key).is_none() {
                continue;
            }
            let is_height = same(viewport_key, "WindowedViewportY")
                || same(viewport_key, "FullscreenViewportY");
            rewrite(
                &mut values,
                &mut changes,
                "SDLDrv.SDLClient",
                viewport_key,
                (if is_height { height } else { width }).to_string(),
                "settings.viewport_consolidated",
            );
        }
    }

    // Frame rate cap: renderer floats like "60.000000" become plain integers.
    if let Some(text) = final_value(&values, "Engine.GameEngine", "FrameRateLimit") {
        let mut canonical = 60;
        let mut reason = "settings.frame_rate_limit_invalid_defaulted";
        if let Some(cap) = number(text)
            && cap == cap.trunc()
            && cap >= 0.0
            && cap <= f64::from(i32::MAX)
            && valid_cap(cap as i32)
        {
            canonical = cap as i32;
            reason = "settings.frame_rate_limit_normalized";
        }
        rewrite(
            &mut values,
            &mut changes,
            "Engine.GameEngine",
            "FrameRateLimit",
            canonical.to_string(),
            reason,
        );
    }

    let mut migrate_discrete_scale =
        |section: &'static str, key: &'static str, accepted: &[f64], fallback: f64| {
            let Some(raw) = final_value(&values, section, key) else {
                return;
            };
            let mut canonical = fallback;
            let mut accepted_value = false;
            if let Some(parsed) = number(raw)
                && accepted.contains(&parsed)
            {
                canonical = parsed;
                accepted_value = true;
            }
            let reason = if accepted_value {
                "settings.scale_normalized"
            } else {
                "settings.scale_invalid_defaulted"
            };
            rewrite(
                &mut values,
                &mut changes,
                section,
                key,
                double_text(canonical),
                reason,
            );
        };
    migrate_discrete_scale(
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        &RENDER_SCALES,
        1.0,
    );
    migrate_discrete_scale("SDLDrv.SDLClient", "UIScale", &UI_SCALES, 1.0);

    let mut migrate_ranged_number =
        |section: &'static str, key: &'static str, minimum: f64, maximum: f64, fallback: f64| {
            let Some(raw) = final_value(&values, section, key) else {
                return;
            };
            let valid = number(raw)
                .map(|parsed| parsed.is_finite() && (minimum..=maximum).contains(&parsed))
                .unwrap_or(false);
            let canonical = if valid {
                number(raw).expect("validated above")
            } else {
                fallback
            };
            let reason = if valid {
                "settings.range_normalized"
            } else {
                "settings.range_invalid_defaulted"
            };
            rewrite(
                &mut values,
                &mut changes,
                section,
                key,
                double_text(canonical),
                reason,
            );
        };
    migrate_ranged_number("SDLDrv.SDLClient", "Brightness", 0.1, 1.0, 0.4);
    migrate_ranged_number("ALAudio.ALAudioSubsystem", "SoundVolume", 0.0, 1.0, 0.9);
    migrate_ranged_number("ALAudio.ALAudioSubsystem", "MusicVolume", 0.0, 1.0, 0.53);
    migrate_ranged_number("Engine.PlayerPawn", "MouseSensitivity", 0.2, 10.0, 3.0);

    // Anti-aliasing: a disabled or unrecognized UseAA clears any stale
    // sample count, mirroring LoadAntiAliasing.
    {
        let use_aa =
            final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA").map(str::to_string);
        let samples_raw = final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples")
            .map(str::to_string);
        if use_aa.is_some() || samples_raw.is_some() {
            let enabled_parsed = use_aa.as_deref().and_then(boolean).unwrap_or(false);
            let mut samples = 0;
            if let Some(samples_text) = samples_raw.as_deref().and_then(integer)
                && enabled_parsed
                && (samples_text == 2 || samples_text == 4)
            {
                samples = samples_text;
            }
            let enabled = samples != 0;
            if use_aa.is_some() {
                rewrite(
                    &mut values,
                    &mut changes,
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "UseAA",
                    bool_text(enabled).to_string(),
                    "settings.antialiasing_normalized",
                );
            }
            if samples_raw.is_some() {
                rewrite(
                    &mut values,
                    &mut changes,
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "NumAASamples",
                    samples.to_string(),
                    "settings.antialiasing_normalized",
                );
            }
        }
    }

    if let Some(text) = final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy") {
        let mut canonical = 4;
        let mut reason = "settings.anisotropy_invalid_defaulted";
        if let Some(parsed) = number(text)
            && parsed == parsed.trunc()
            && parsed >= f64::from(i32::MIN)
            && parsed <= f64::from(i32::MAX)
        {
            let discrete = parsed as i32;
            if ANISOTROPY_VALUES.contains(&discrete) {
                canonical = discrete;
                reason = "settings.anisotropy_normalized";
            }
        }
        rewrite(
            &mut values,
            &mut changes,
            "XOpenGLDrv.XOpenGLRenderDevice",
            "MaxAnisotropy",
            canonical.to_string(),
            reason,
        );
    }

    let mut migrate_enum = |section: &'static str,
                            key: &'static str,
                            spellings: &[&'static str],
                            fallback_index: usize| {
        let Some(raw) = final_value(&values, section, key) else {
            return;
        };
        let trimmed = raw.trim();
        let selected = spellings
            .iter()
            .position(|spelling| same(trimmed, spelling))
            .unwrap_or(fallback_index);
        let recognized = selected != fallback_index || same(trimmed, spellings[fallback_index]);
        let reason = if recognized {
            "settings.enum_normalized"
        } else {
            "settings.enum_invalid_defaulted"
        };
        rewrite(
            &mut values,
            &mut changes,
            section,
            key,
            spellings[selected].to_string(),
            reason,
        );
    };
    migrate_enum(
        "SDLDrv.SDLClient",
        "TextureDetail",
        &["Low", "Medium", "High"],
        2,
    );
    migrate_enum(
        "Engine.PlayerPawn",
        "ObjectDetail",
        &[
            "ObjectDetailVeryLow",
            "ObjectDetailLow",
            "ObjectDetailMedium",
            "ObjectDetailHigh",
            "ObjectDetailVeryHigh",
        ],
        2,
    );
    migrate_enum(
        "Engine.PlayerPawn",
        "Difficulty",
        &["DifficultyEasy", "DifficultyMedium", "DifficultyHard"],
        0,
    );

    (values, changes)
}
