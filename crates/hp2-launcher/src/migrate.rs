//! Replaces `MigrateLegacySettings` and its two local helpers
//! (`MigrationFinalValue`, `MigrationRewrite`) from
//! `HP2LauncherStore.cpp`, plus the `LegacySettingValue`/
//! `SettingsMigrationChange` types from `HP2LauncherStore.h`.
//!
//! Per that header's own doc comment, this "performs no filesystem,
//! locale, or global state access, so it is fully table-testable" --
//! unlike the rest of `HP2LauncherStore.cpp`'s non-`store.rs` scope,
//! this belongs alongside `store.rs`'s pure-logic functions, not the
//! filesystem-touching modules (`atomic.rs`/`tree.rs`/`saves.rs`). It
//! gets its own file purely because of its size (~300 lines of real
//! migration-table logic in the real C++).

use crate::model::{ANISOTROPY_VALUES, RENDER_SCALE_VALUES, UI_SCALE_VALUES};
use crate::store::{bool_text, boolean, double_text, integer, number, same, trim, valid_cap};

/// Replaces `LegacySettingValue`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacySettingValue {
    pub section: String,
    pub key: String,
    pub value: String,
}

/// Replaces `SettingsMigrationChange`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsMigrationChange {
    pub section: String,
    pub key: String,
    pub previous_value: String,
    pub new_value: String,
    pub reason: String,
}

/// Replaces `MigrationFinalValue`: the value of the *last* matching
/// row (duplicate section/key rows are legal; last occurrence wins),
/// or `None` if no row matches.
fn migration_final_value(
    values: &[LegacySettingValue],
    section: &str,
    key: &str,
) -> Option<String> {
    let mut found = None;
    for entry in values {
        if same(&entry.section, section) && same(&entry.key, key) {
            found = Some(entry.value.clone());
        }
    }
    found
}

/// Replaces `MigrationRewrite`: overwrites the last matching row's
/// value (or appends a new row if none matched), and records a change
/// journal entry if the value is new or different.
fn migration_rewrite(
    values: &mut Vec<LegacySettingValue>,
    section: &str,
    key: &str,
    next: &str,
    reason: &str,
    changes: &mut Vec<SettingsMigrationChange>,
) {
    let previous = migration_final_value(values, section, key);
    let existed = previous.is_some();
    let mut last = None;
    for (i, entry) in values.iter().enumerate() {
        if same(&entry.section, section) && same(&entry.key, key) {
            last = Some(i);
        }
    }
    match last {
        Some(i) => values[i].value = next.to_string(),
        None => values.push(LegacySettingValue {
            section: section.to_string(),
            key: key.to_string(),
            value: next.to_string(),
        }),
    }
    let previous_text = previous.unwrap_or_default();
    if !existed || previous_text != next {
        changes.push(SettingsMigrationChange {
            section: section.to_string(),
            key: key.to_string(),
            previous_value: previous_text,
            new_value: next.to_string(),
            reason: reason.to_string(),
        });
    }
}

struct OwnedBoolean {
    section: &'static str,
    key: &'static str,
    fallback: bool,
}

const OWNED_BOOLEANS: &[OwnedBoolean] = &[
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

const VIEWPORT_KEYS: &[&str] = &[
    "WindowedViewportX",
    "WindowedViewportY",
    "FullscreenViewportX",
    "FullscreenViewportY",
];

const TEXTURE_SPELLINGS: &[&str] = &["Low", "Medium", "High"];
const OBJECT_SPELLINGS: &[&str] = &[
    "ObjectDetailVeryLow",
    "ObjectDetailLow",
    "ObjectDetailMedium",
    "ObjectDetailHigh",
    "ObjectDetailVeryHigh",
];
const DIFFICULTY_SPELLINGS: &[&str] = &["DifficultyEasy", "DifficultyMedium", "DifficultyHard"];

fn migrate_discrete_scale(
    values: &mut Vec<LegacySettingValue>,
    changes: &mut Vec<SettingsMigrationChange>,
    section: &str,
    key: &str,
    accepted: &[f64],
    fallback: f64,
) {
    let Some(raw) = migration_final_value(values, section, key) else {
        return;
    };
    let mut canonical = fallback;
    let mut accepted_value = false;
    if let Some(parsed) = number(&raw) {
        for &a in accepted {
            if parsed == a {
                canonical = a;
                accepted_value = true;
                break;
            }
        }
    }
    let reason = if accepted_value {
        "settings.scale_normalized"
    } else {
        "settings.scale_invalid_defaulted"
    };
    migration_rewrite(
        values,
        section,
        key,
        &double_text(canonical),
        reason,
        changes,
    );
}

fn migrate_ranged_number(
    values: &mut Vec<LegacySettingValue>,
    changes: &mut Vec<SettingsMigrationChange>,
    section: &str,
    key: &str,
    minimum: f64,
    maximum: f64,
    fallback: f64,
) {
    let Some(raw) = migration_final_value(values, section, key) else {
        return;
    };
    let parsed = number(&raw);
    let valid = parsed.is_some_and(|v| v >= minimum && v <= maximum);
    let canonical = if valid { parsed.unwrap() } else { fallback };
    let reason = if valid {
        "settings.range_normalized"
    } else {
        "settings.range_invalid_defaulted"
    };
    migration_rewrite(
        values,
        section,
        key,
        &double_text(canonical),
        reason,
        changes,
    );
}

fn migrate_enum(
    values: &mut Vec<LegacySettingValue>,
    changes: &mut Vec<SettingsMigrationChange>,
    section: &str,
    key: &str,
    spellings: &[&str],
    fallback_index: usize,
) {
    let Some(raw) = migration_final_value(values, section, key) else {
        return;
    };
    let mut selected = fallback_index;
    let mut recognized = false;
    let trimmed = trim(&raw);
    for (i, spelling) in spellings.iter().enumerate() {
        if same(trimmed, spelling) {
            selected = i;
            recognized = true;
            break;
        }
    }
    let reason = if recognized {
        "settings.enum_normalized"
    } else {
        "settings.enum_invalid_defaulted"
    };
    migration_rewrite(values, section, key, spellings[selected], reason, changes);
}

/// Replaces `MigrateLegacySettings`. Returns the canonicalized rows
/// and the journal of every normalization performed.
pub fn migrate_legacy_settings(
    legacy_values: &[LegacySettingValue],
) -> (Vec<LegacySettingValue>, Vec<SettingsMigrationChange>) {
    let mut changes = Vec::new();
    let mut values: Vec<LegacySettingValue> = legacy_values.to_vec();

    // Owned booleans canonicalize to True/False; invalid spellings
    // fall back to the model default the loader would have used.
    for owned in OWNED_BOOLEANS {
        let Some(text) = migration_final_value(&values, owned.section, owned.key) else {
            continue;
        };
        let (parsed, recognized) = match boolean(&text) {
            Some(b) => (b, true),
            None => (owned.fallback, false),
        };
        let reason = if recognized {
            "settings.boolean_spelling"
        } else {
            "settings.boolean_invalid_defaulted"
        };
        migration_rewrite(
            &mut values,
            owned.section,
            owned.key,
            bool_text(parsed),
            reason,
            &mut changes,
        );
    }

    // Screen mode: borderless/desktop overrides fullscreen, exactly as
    // the loader resolves the three legacy switches.
    let has_startup_fullscreen =
        migration_final_value(&values, "SDLDrv.SDLClient", "StartupFullscreen")
            .and_then(|t| boolean(&t));
    let has_borderless = migration_final_value(&values, "SDLDrv.SDLClient", "BorderlessWindow")
        .and_then(|t| boolean(&t));
    let has_desktop_resolution =
        migration_final_value(&values, "SDLDrv.SDLClient", "UseDesktopResolution")
            .and_then(|t| boolean(&t));
    if has_startup_fullscreen.is_some()
        || has_borderless.is_some()
        || has_desktop_resolution.is_some()
    {
        let startup_fullscreen = has_startup_fullscreen.unwrap_or(false);
        let borderless = has_borderless.unwrap_or(false);
        let desktop_resolution = has_desktop_resolution.unwrap_or(false);
        let borderless_desktop = borderless || desktop_resolution;
        if has_startup_fullscreen.is_some() {
            migration_rewrite(
                &mut values,
                "SDLDrv.SDLClient",
                "StartupFullscreen",
                bool_text(startup_fullscreen && !borderless_desktop),
                "settings.screen_mode_consolidated",
                &mut changes,
            );
        }
        if has_borderless.is_some() {
            migration_rewrite(
                &mut values,
                "SDLDrv.SDLClient",
                "BorderlessWindow",
                bool_text(borderless_desktop),
                "settings.screen_mode_consolidated",
                &mut changes,
            );
        }

        // Viewport quartet collapses onto the resolution the launcher
        // would actually run for the consolidated mode: the loader
        // reads the fullscreen pair only when the mode is not
        // windowed.
        let full_size = borderless_desktop || startup_fullscreen;
        let width_key = if full_size {
            "FullscreenViewportX"
        } else {
            "WindowedViewportX"
        };
        let height_key = if full_size {
            "FullscreenViewportY"
        } else {
            "WindowedViewportY"
        };
        let mut width = 800;
        let mut height = 600;
        if let Some(text) = migration_final_value(&values, "SDLDrv.SDLClient", width_key)
            && let Some(parsed) = integer(&text)
            && (320..=16384).contains(&parsed)
        {
            width = parsed;
        }
        if let Some(text) = migration_final_value(&values, "SDLDrv.SDLClient", height_key)
            && let Some(parsed) = integer(&text)
            && (320..=16384).contains(&parsed)
        {
            height = parsed;
        }
        for &viewport_key in VIEWPORT_KEYS {
            if migration_final_value(&values, "SDLDrv.SDLClient", viewport_key).is_none() {
                continue;
            }
            let is_height =
                viewport_key == "WindowedViewportY" || viewport_key == "FullscreenViewportY";
            let value = if is_height { height } else { width };
            migration_rewrite(
                &mut values,
                "SDLDrv.SDLClient",
                viewport_key,
                &value.to_string(),
                "settings.viewport_consolidated",
                &mut changes,
            );
        }
    }

    // Frame rate cap: renderer floats like "60.000000" become plain
    // integers.
    if let Some(text) = migration_final_value(&values, "Engine.GameEngine", "FrameRateLimit") {
        let mut canonical = 60i32;
        let mut reason = "settings.frame_rate_limit_invalid_defaulted";
        if let Some(cap) = number(&text)
            && cap == cap.floor()
            && cap >= 0.0
            && cap <= i32::MAX as f64
            && valid_cap(cap as i32)
        {
            canonical = cap as i32;
            reason = "settings.frame_rate_limit_normalized";
        }
        migration_rewrite(
            &mut values,
            "Engine.GameEngine",
            "FrameRateLimit",
            &canonical.to_string(),
            reason,
            &mut changes,
        );
    }

    migrate_discrete_scale(
        &mut values,
        &mut changes,
        "XOpenGLDrv.XOpenGLRenderDevice",
        "RenderScale",
        &RENDER_SCALE_VALUES,
        1.0,
    );
    migrate_discrete_scale(
        &mut values,
        &mut changes,
        "SDLDrv.SDLClient",
        "UIScale",
        &UI_SCALE_VALUES,
        1.0,
    );

    migrate_ranged_number(
        &mut values,
        &mut changes,
        "SDLDrv.SDLClient",
        "Brightness",
        0.1,
        1.0,
        0.4,
    );
    migrate_ranged_number(
        &mut values,
        &mut changes,
        "ALAudio.ALAudioSubsystem",
        "SoundVolume",
        0.0,
        1.0,
        0.9,
    );
    migrate_ranged_number(
        &mut values,
        &mut changes,
        "ALAudio.ALAudioSubsystem",
        "MusicVolume",
        0.0,
        1.0,
        0.53,
    );
    migrate_ranged_number(
        &mut values,
        &mut changes,
        "Engine.PlayerPawn",
        "MouseSensitivity",
        0.2,
        10.0,
        3.0,
    );

    // Anti-aliasing: a disabled or unrecognized UseAA clears any stale
    // sample count, mirroring `load_anti_aliasing`.
    {
        let aa_raw = migration_final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "UseAA");
        let samples_raw =
            migration_final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples");
        let has_use_aa = aa_raw.is_some();
        let has_samples = samples_raw.is_some();
        if has_use_aa || has_samples {
            let enabled = aa_raw.as_deref().and_then(boolean).unwrap_or(false);
            let mut samples = 0;
            if enabled
                && let Some(raw) = &samples_raw
                && let Some(parsed_samples) = integer(raw)
                && (parsed_samples == 2 || parsed_samples == 4)
            {
                samples = parsed_samples;
            }
            let enabled = samples != 0;
            if has_use_aa {
                migration_rewrite(
                    &mut values,
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "UseAA",
                    bool_text(enabled),
                    "settings.antialiasing_normalized",
                    &mut changes,
                );
            }
            if has_samples {
                migration_rewrite(
                    &mut values,
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "NumAASamples",
                    &samples.to_string(),
                    "settings.antialiasing_normalized",
                    &mut changes,
                );
            }
        }
    }

    if let Some(text) =
        migration_final_value(&values, "XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy")
    {
        let mut canonical = 4i32;
        let mut reason = "settings.anisotropy_invalid_defaulted";
        if let Some(parsed) = number(&text)
            && parsed == parsed.floor()
            && parsed >= i32::MIN as f64
            && parsed <= i32::MAX as f64
        {
            let discrete = parsed as i32;
            if ANISOTROPY_VALUES.contains(&discrete) {
                canonical = discrete;
                reason = "settings.anisotropy_normalized";
            }
        }
        migration_rewrite(
            &mut values,
            "XOpenGLDrv.XOpenGLRenderDevice",
            "MaxAnisotropy",
            &canonical.to_string(),
            reason,
            &mut changes,
        );
    }

    migrate_enum(
        &mut values,
        &mut changes,
        "SDLDrv.SDLClient",
        "TextureDetail",
        TEXTURE_SPELLINGS,
        2,
    );
    migrate_enum(
        &mut values,
        &mut changes,
        "Engine.PlayerPawn",
        "ObjectDetail",
        OBJECT_SPELLINGS,
        2,
    );
    migrate_enum(
        &mut values,
        &mut changes,
        "Engine.PlayerPawn",
        "Difficulty",
        DIFFICULTY_SPELLINGS,
        0,
    );

    (values, changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(section: &str, key: &str, value: &str) -> LegacySettingValue {
        LegacySettingValue {
            section: section.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    fn change(
        section: &str,
        key: &str,
        previous: &str,
        next: &str,
        reason: &str,
    ) -> SettingsMigrationChange {
        SettingsMigrationChange {
            section: section.to_string(),
            key: key.to_string(),
            previous_value: previous.to_string(),
            new_value: next.to_string(),
            reason: reason.to_string(),
        }
    }

    #[test]
    fn empty_input_produces_empty_output() {
        let (values, changes) = migrate_legacy_settings(&[]);
        assert!(values.is_empty());
        assert!(changes.is_empty());
    }

    // Verified against the real C++ `MigrateLegacySettings` (externally
    // linked, called directly) via a throwaway probe on this exact
    // 27-row fixture: full output (both the canonicalized rows and the
    // change journal, in order) matched byte-for-byte, covering owned
    // booleans (both valid and invalid spellings), screen mode
    // consolidation (StartupFullscreen present twice -- duplicate-row
    // last-occurrence-wins), viewport consolidation, the frame rate
    // cap, a valid discrete scale and an invalid one, valid and
    // out-of-range ranged numbers, anti-aliasing with samples
    // preserved, anisotropy, valid and invalid enum spellings
    // (including surrounding whitespace), and an unrelated
    // passthrough key.
    #[test]
    fn migrate_legacy_settings_matches_cpp_ground_truth_on_a_full_fixture() {
        let input = vec![
            value("SDLDrv.SDLClient", "StartupFullscreen", "TRUE"),
            value("SDLDrv.SDLClient", "BorderlessWindow", "garbage"),
            value("SDLDrv.SDLClient", "ShowFPS", "yes"),
            value("SDLDrv.SDLClient", "MaintainVerticalFOV", "nonsense"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", "0"),
            value("Engine.GameEngine", "UseSound", "off"),
            value("Engine.PlayerPawn", "bInvertMouse", "1"),
            value("HGame.Harry", "bAutoCenterCamera", "false"),
            value("SDLDrv.SDLClient", "FullscreenViewportX", "1920"),
            value("SDLDrv.SDLClient", "FullscreenViewportY", "1080"),
            value("SDLDrv.SDLClient", "WindowedViewportX", "640"),
            value("SDLDrv.SDLClient", "WindowedViewportY", "480"),
            value("Engine.GameEngine", "FrameRateLimit", "60.000000"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.67"),
            value("SDLDrv.SDLClient", "UIScale", "garbage"),
            value("SDLDrv.SDLClient", "Brightness", "0.4"),
            value("ALAudio.ALAudioSubsystem", "SoundVolume", "1.5"),
            value("ALAudio.ALAudioSubsystem", "MusicVolume", "0.3"),
            value("Engine.PlayerPawn", "MouseSensitivity", "bogus"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "true"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "8"),
            value("SDLDrv.SDLClient", "TextureDetail", "  High  "),
            value("Engine.PlayerPawn", "ObjectDetail", "Unknown"),
            value("Engine.PlayerPawn", "Difficulty", "DifficultyHard"),
            value("SomeOther.Section", "UnrelatedKey", "passthrough"),
            value("SDLDrv.SDLClient", "StartupFullscreen", "FALSE"),
        ];
        let (values, changes) = migrate_legacy_settings(&input);
        assert_eq!(
            values,
            vec![
                value("SDLDrv.SDLClient", "StartupFullscreen", "TRUE"),
                value("SDLDrv.SDLClient", "BorderlessWindow", "False"),
                value("SDLDrv.SDLClient", "ShowFPS", "True"),
                value("SDLDrv.SDLClient", "MaintainVerticalFOV", "True"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "UseVSync", "False"),
                value("Engine.GameEngine", "UseSound", "False"),
                value("Engine.PlayerPawn", "bInvertMouse", "True"),
                value("HGame.Harry", "bAutoCenterCamera", "False"),
                value("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
                value("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
                value("SDLDrv.SDLClient", "WindowedViewportX", "640"),
                value("SDLDrv.SDLClient", "WindowedViewportY", "480"),
                value("Engine.GameEngine", "FrameRateLimit", "60"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.67"),
                value("SDLDrv.SDLClient", "UIScale", "1"),
                value("SDLDrv.SDLClient", "Brightness", "0.4"),
                value("ALAudio.ALAudioSubsystem", "SoundVolume", "0.9"),
                value("ALAudio.ALAudioSubsystem", "MusicVolume", "0.3"),
                value("Engine.PlayerPawn", "MouseSensitivity", "3"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "True"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "8"),
                value("SDLDrv.SDLClient", "TextureDetail", "High"),
                value("Engine.PlayerPawn", "ObjectDetail", "ObjectDetailMedium"),
                value("Engine.PlayerPawn", "Difficulty", "DifficultyHard"),
                value("SomeOther.Section", "UnrelatedKey", "passthrough"),
                value("SDLDrv.SDLClient", "StartupFullscreen", "False"),
            ]
        );
        assert_eq!(
            changes,
            vec![
                change(
                    "SDLDrv.SDLClient",
                    "StartupFullscreen",
                    "FALSE",
                    "False",
                    "settings.boolean_spelling"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "BorderlessWindow",
                    "garbage",
                    "False",
                    "settings.boolean_invalid_defaulted"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "ShowFPS",
                    "yes",
                    "True",
                    "settings.boolean_spelling"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "MaintainVerticalFOV",
                    "nonsense",
                    "True",
                    "settings.boolean_invalid_defaulted"
                ),
                change(
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "UseVSync",
                    "0",
                    "False",
                    "settings.boolean_spelling"
                ),
                change(
                    "XOpenGLDrv.XOpenGLRenderDevice",
                    "UseAA",
                    "true",
                    "True",
                    "settings.boolean_spelling"
                ),
                change(
                    "Engine.GameEngine",
                    "UseSound",
                    "off",
                    "False",
                    "settings.boolean_spelling"
                ),
                change(
                    "Engine.PlayerPawn",
                    "bInvertMouse",
                    "1",
                    "True",
                    "settings.boolean_spelling"
                ),
                change(
                    "HGame.Harry",
                    "bAutoCenterCamera",
                    "false",
                    "False",
                    "settings.boolean_spelling"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "FullscreenViewportX",
                    "1920",
                    "640",
                    "settings.viewport_consolidated"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "FullscreenViewportY",
                    "1080",
                    "480",
                    "settings.viewport_consolidated"
                ),
                change(
                    "Engine.GameEngine",
                    "FrameRateLimit",
                    "60.000000",
                    "60",
                    "settings.frame_rate_limit_normalized"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "UIScale",
                    "garbage",
                    "1",
                    "settings.scale_invalid_defaulted"
                ),
                change(
                    "ALAudio.ALAudioSubsystem",
                    "SoundVolume",
                    "1.5",
                    "0.9",
                    "settings.range_invalid_defaulted"
                ),
                change(
                    "Engine.PlayerPawn",
                    "MouseSensitivity",
                    "bogus",
                    "3",
                    "settings.range_invalid_defaulted"
                ),
                change(
                    "SDLDrv.SDLClient",
                    "TextureDetail",
                    "  High  ",
                    "High",
                    "settings.enum_normalized"
                ),
                change(
                    "Engine.PlayerPawn",
                    "ObjectDetail",
                    "Unknown",
                    "ObjectDetailMedium",
                    "settings.enum_invalid_defaulted"
                ),
            ]
        );
    }

    // Verified against the real C++ the same way: duplicate rows for
    // the same section/key are legal, the *last* occurrence is both
    // the value read (as "previous") and the one overwritten in
    // place -- earlier duplicates are left untouched.
    #[test]
    fn duplicate_rows_use_and_rewrite_only_the_last_occurrence() {
        let input = vec![
            value("SDLDrv.SDLClient", "ShowFPS", "false"),
            value("SDLDrv.SDLClient", "ShowFPS", "true"),
            value("SDLDrv.SDLClient", "ShowFPS", "garbage"),
        ];
        let (values, changes) = migrate_legacy_settings(&input);
        assert_eq!(
            values,
            vec![
                value("SDLDrv.SDLClient", "ShowFPS", "false"),
                value("SDLDrv.SDLClient", "ShowFPS", "true"),
                value("SDLDrv.SDLClient", "ShowFPS", "False"),
            ]
        );
        assert_eq!(
            changes,
            vec![change(
                "SDLDrv.SDLClient",
                "ShowFPS",
                "garbage",
                "False",
                "settings.boolean_invalid_defaulted"
            )]
        );
    }

    // Verified against the real C++: UseAA=false clears a stale
    // NumAASamples to 0 even though the sample count itself parses
    // fine, mirroring `load_anti_aliasing`'s own "disabled wins"
    // precedence.
    #[test]
    fn disabled_anti_aliasing_clears_a_stale_sample_count() {
        let input = vec![
            value("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "false"),
            value("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
        ];
        let (values, _changes) = migrate_legacy_settings(&input);
        assert_eq!(
            values,
            vec![
                value("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "False"),
                value("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "0"),
            ]
        );
    }

    #[test]
    fn unrecognized_keys_pass_through_untouched() {
        let input = vec![value("SomeOther.Section", "UnrelatedKey", "anything")];
        let (values, changes) = migrate_legacy_settings(&input);
        assert_eq!(values, input);
        assert!(changes.is_empty());
    }
}
