//! Engine-cache adapter for the supported Game.ini settings.
//!
//! This adapter never mutates runtime objects. Every change is pending restart.
//! It uses canonical System and User caches, not separately opened INI aliases.
//! Mixed-file patches are rejected; callers may explicitly apply separate patches. Core stages edits
//! in a private cache copy and commits it only after publication succeeds.
//! External writers must be serialized: Core's conflict check cannot close
//! the check/rename race.

use crate::settings_policy::{ConfigFile, ConfigKey, SettingsPatch, SettingsPlan, ValidationError};
use hp2_core::core_access::{CoreAccess, RuntimeError};

const KEYS: &[ConfigKey] = &[
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "Brightness",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "ShowFPS",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "NativeText",
    },
    ConfigKey {
        section: "Engine.GameEngine",
        key: "FrameRateLimit",
    },
    ConfigKey {
        section: "WgpuDrv.WgpuRenderDevice",
        key: "UseVSync",
    },
    ConfigKey {
        section: "WgpuDrv.WgpuRenderDevice",
        key: "RenderScale",
    },
    ConfigKey {
        section: "WgpuDrv.WgpuRenderDevice",
        key: "UseAA",
    },
    ConfigKey {
        section: "WgpuDrv.WgpuRenderDevice",
        key: "NumAASamples",
    },
    ConfigKey {
        section: "WgpuDrv.WgpuRenderDevice",
        key: "MaxAnisotropy",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "StartupFullscreen",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "BorderlessWindow",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "UseDesktopResolution",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "WindowedViewportX",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "WindowedViewportY",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "FullscreenViewportX",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "FullscreenViewportY",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "TextureDetail",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "UIScale",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "MaintainVerticalFOV",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "ScreenFlashes",
    },
    ConfigKey {
        section: "SDLDrv.SDLClient",
        key: "UseJoystick",
    },
    ConfigKey {
        section: "Engine.GameEngine",
        key: "UseSound",
    },
    ConfigKey {
        section: "ALAudio.ALAudioSubsystem",
        key: "SoundVolume",
    },
    ConfigKey {
        section: "ALAudio.ALAudioSubsystem",
        key: "MusicVolume",
    },
];
const USER_KEYS: &[ConfigKey] = &[
    ConfigKey {
        section: "Engine.PlayerPawn",
        key: "MouseSensitivity",
    },
    ConfigKey {
        section: "Engine.PlayerPawn",
        key: "bInvertMouse",
    },
    ConfigKey {
        section: "Engine.PlayerPawn",
        key: "bModernThirdPersonControls",
    },
    ConfigKey {
        section: "Engine.PlayerPawn",
        key: "Difficulty",
    },
    ConfigKey {
        section: "Engine.PlayerPawn",
        key: "ObjectDetail",
    },
    ConfigKey {
        section: "HGame.Harry",
        key: "bAutoCenterCamera",
    },
    ConfigKey {
        section: "HGame.Harry",
        key: "bMoveWhileCasting",
    },
    ConfigKey {
        section: "HGame.Harry",
        key: "bAutoQuaff",
    },
];

/// Raw cached values. Missing values remain absent; no launcher defaults are
/// invented and invalid persisted values are not silently normalized.
#[derive(Debug, PartialEq, Eq)]
pub struct SettingsSnapshot {
    pub values: Vec<(ConfigKey, Option<String>)>,
}

#[derive(Debug)]
pub enum SettingsError {
    Validation(ValidationError),
    /// Staging or publication failed; the canonical cache is unchanged.
    CacheOrFlush(RuntimeError),
    /// System and User cannot be atomically committed together.
    MixedFiles,
}

fn units(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// Read the canonical System/Game settings.
pub fn read(access: CoreAccess<'_>) -> Result<SettingsSnapshot, RuntimeError> {
    read_file(access, ConfigFile::System)
}

/// Read supported settings from either canonical cache.
pub fn read_file(
    access: CoreAccess<'_>,
    file: ConfigFile,
) -> Result<SettingsSnapshot, RuntimeError> {
    let keys = match file {
        ConfigFile::System => KEYS,
        ConfigFile::User => USER_KEYS,
    };
    let user = units("User");
    let filename = match file {
        ConfigFile::System => None,
        ConfigFile::User => Some(user.as_slice()),
    };
    let mut values = Vec::with_capacity(keys.len());
    for &key in keys {
        let value = access.config_get_units_from(filename, &units(key.section), &units(key.key))?;
        let value = value
            .map(|value| {
                value
                    .into_iter()
                    .map(|unit| {
                        char::from_u32(unit)
                            .ok_or_else(|| RuntimeError::new("Invalid config value"))
                    })
                    .collect::<Result<String, _>>()
            })
            .transpose()?;
        values.push((key, value));
    }
    Ok(SettingsSnapshot { values })
}

/// Validate, stage selected fields, publish, then commit the staged cache.
/// Success does not mean that the running engine changed. An empty patch is a
/// no-op. Failure leaves the canonical cache and its dirty state unchanged.
pub fn apply(access: CoreAccess<'_>, patch: &SettingsPatch) -> Result<SettingsPlan, SettingsError> {
    let plan = patch.plan().map_err(SettingsError::Validation)?;
    if plan.changes().is_empty() {
        return Ok(plan);
    }
    let changes: Vec<_> = plan
        .changes()
        .iter()
        .map(|change| {
            (
                units(change.key.section),
                units(change.key.key),
                units(&change.value),
            )
        })
        .collect();
    let file = plan.changes()[0].file;
    if plan.changes().iter().any(|change| change.file != file) {
        return Err(SettingsError::MixedFiles);
    }
    match file {
        ConfigFile::System => access.config_update_system_staged(&changes),
        ConfigFile::User => access.config_update_user_staged(&changes),
    }
    .map_err(SettingsError::CacheOrFlush)?;
    Ok(plan)
}
