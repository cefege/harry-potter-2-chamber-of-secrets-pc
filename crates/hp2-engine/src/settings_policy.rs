//! Side-effect-free settings policy. Plans do not update runtime objects.
//! See `settings_config` for the engine-owned configuration adapter.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyMode {
    /// Persist for the next engine start; do not change the live object.
    RestartRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigKey {
    pub section: &'static str,
    pub key: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigFile {
    System,
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectDetail {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlMode {
    Classic,
    Modern,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureDetail {
    Low,
    Medium,
    High,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenMode {
    Windowed,
    Fullscreen,
    BorderlessDesktop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub file: ConfigFile,
    pub key: ConfigKey,
    pub value: String,
    pub mode: ApplyMode,
}

/// Only `Some` fields are changed. This deliberately covers a small subset
/// of launcher settings; unsupported settings must not be inferred from it.
#[derive(Clone, Debug, Default)]
pub struct SettingsPatch {
    pub brightness: Option<f64>,
    pub show_fps: Option<bool>,
    pub native_text: Option<bool>,
    pub frame_rate_limit: Option<i32>,
    pub vertical_sync: Option<bool>,
    pub screen_mode: Option<ScreenMode>,
    pub resolution: Option<Resolution>,
    pub render_scale: Option<f64>,
    pub anti_aliasing_samples: Option<i32>,
    pub anisotropy: Option<i32>,
    pub texture_detail: Option<TextureDetail>,
    pub ui_scale: Option<f64>,
    pub maintain_vertical_fov: Option<bool>,
    pub screen_flashes: Option<bool>,
    pub joystick_enabled: Option<bool>,
    pub sound_enabled: Option<bool>,
    pub sound_volume: Option<f64>,
    pub music_volume: Option<f64>,
    pub mouse_sensitivity: Option<f64>,
    pub mouse_smoothing: Option<bool>,
    pub invert_mouse: Option<bool>,
    pub control_mode: Option<ControlMode>,
    pub difficulty: Option<Difficulty>,
    pub object_detail: Option<ObjectDetail>,
    pub auto_center_camera: Option<bool>,
    pub move_while_casting: Option<bool>,
    pub auto_quaff: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsPlan {
    changes: Vec<Change>,
}

impl SettingsPlan {
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Merge into a caller-owned snapshot. This performs no IO and does not
    /// resolve concurrent changes; the adapter must use a current snapshot.
    pub fn merge_into(&self, snapshot: &mut BTreeMap<(ConfigFile, String, String), String>) {
        for change in &self.changes {
            let mut found = false;
            for ((file, section, key), value) in snapshot.iter_mut() {
                if *file == change.file
                    && section.eq_ignore_ascii_case(change.key.section)
                    && key.eq_ignore_ascii_case(change.key.key)
                {
                    *value = change.value.clone();
                    found = true;
                }
            }
            if !found {
                snapshot.insert(
                    (
                        change.file,
                        change.key.section.to_owned(),
                        change.key.key.to_owned(),
                    ),
                    change.value.clone(),
                );
            }
        }
    }
}

impl SettingsPatch {
    /// Validate the entire patch before returning any changes.
    pub fn plan(&self) -> Result<SettingsPlan, ValidationError> {
        if let Some(value) = self.brightness {
            if !value.is_finite() || !(0.1..=1.0).contains(&value) {
                return Err(ValidationError {
                    field: "brightness",
                    message: "must be finite and between 0.1 and 1.0",
                });
            }
        }
        if let Some(value) = self.frame_rate_limit {
            if ![0, 30, 60, 120, 144].contains(&value) {
                return Err(ValidationError {
                    field: "frame_rate_limit",
                    message: "must be 0, 30, 60, 120, or 144",
                });
            }
        }
        for (field, value, min, max) in [
            ("sound_volume", self.sound_volume, 0.0, 1.0),
            ("music_volume", self.music_volume, 0.0, 1.0),
            ("mouse_sensitivity", self.mouse_sensitivity, 0.2, 10.0),
        ] {
            if let Some(value) = value {
                if !value.is_finite() || !(min..=max).contains(&value) {
                    return Err(ValidationError {
                        field,
                        message: "outside allowed range or non-finite",
                    });
                }
            }
        }
        if let Some(value) = self.ui_scale {
            if ![0.75, 1.0, 1.25, 1.5, 1.75, 2.0].contains(&value) {
                return Err(ValidationError {
                    field: "ui_scale",
                    message: "must match an allowed UI scale",
                });
            }
        }
        if let Some(v) = self.resolution {
            if !(320..=16384).contains(&v.width) || !(320..=16384).contains(&v.height) {
                return Err(ValidationError {
                    field: "resolution",
                    message: "dimensions must be between 320 and 16384",
                });
            }
        }
        if let Some(v) = self.render_scale {
            // Preserve the legacy 67% launcher choice as well as the stock
            // slider's 5% steps. Match launcher validation exactly.
            if ![0.50, 0.55, 0.60, 0.65, 0.67, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95, 1.00].contains(&v) {
                return Err(ValidationError {
                    field: "render_scale",
                    message: "must match an allowed render scale",
                });
            }
        }
        if let Some(v) = self.anti_aliasing_samples {
            if ![0, 2, 4].contains(&v) {
                return Err(ValidationError {
                    field: "anti_aliasing_samples",
                    message: "must be 0, 2, or 4",
                });
            }
        }
        if let Some(v) = self.anisotropy {
            if ![0, 4, 8, 16].contains(&v) {
                return Err(ValidationError {
                    field: "anisotropy",
                    message: "must be 0, 4, 8, or 16",
                });
            }
        }
        let mut changes = Vec::new();
        let mut add = |section, key, value: String, mode| {
            changes.push(Change {
                file: ConfigFile::System,
                key: ConfigKey { section, key },
                value,
                mode,
            });
        };
        let live = ApplyMode::RestartRequired;
        if let Some(value) = self.brightness {
            add("SDLDrv.SDLClient", "Brightness", value.to_string(), live);
        }
        if let Some(value) = self.show_fps {
            add("SDLDrv.SDLClient", "ShowFPS", bool_text(value).into(), live);
        }
        if let Some(value) = self.native_text {
            add(
                "SDLDrv.SDLClient",
                "NativeText",
                bool_text(value).into(),
                live,
            );
        }
        if let Some(value) = self.frame_rate_limit {
            add(
                "Engine.GameEngine",
                "FrameRateLimit",
                value.to_string(),
                live,
            );
        }
        if let Some(value) = self.vertical_sync {
            add(
                "WgpuDrv.WgpuRenderDevice",
                "UseVSync",
                if value { "On" } else { "Off" }.into(),
                ApplyMode::RestartRequired,
            );
        }
        if let Some(v) = self.screen_mode {
            add(
                "SDLDrv.SDLClient",
                "StartupFullscreen",
                bool_text(v != ScreenMode::Windowed).into(),
                live,
            );
            add(
                "SDLDrv.SDLClient",
                "BorderlessWindow",
                bool_text(v == ScreenMode::BorderlessDesktop).into(),
                live,
            );
            add(
                "SDLDrv.SDLClient",
                "UseDesktopResolution",
                bool_text(v == ScreenMode::BorderlessDesktop).into(),
                live,
            );
        }
        if let Some(v) = self.resolution {
            for (key, value) in [
                ("WindowedViewportX", v.width),
                ("WindowedViewportY", v.height),
                ("FullscreenViewportX", v.width),
                ("FullscreenViewportY", v.height),
            ] {
                add("SDLDrv.SDLClient", key, value.to_string(), live);
            }
        }
        if let Some(v) = self.render_scale {
            add(
                "WgpuDrv.WgpuRenderDevice",
                "RenderScale",
                v.to_string(),
                live,
            );
        }
        if let Some(v) = self.anti_aliasing_samples {
            add(
                "WgpuDrv.WgpuRenderDevice",
                "UseAA",
                bool_text(v != 0).into(),
                live,
            );
            add(
                "WgpuDrv.WgpuRenderDevice",
                "NumAASamples",
                v.to_string(),
                live,
            );
        }
        if let Some(v) = self.anisotropy {
            add(
                "WgpuDrv.WgpuRenderDevice",
                "MaxAnisotropy",
                v.to_string(),
                live,
            );
        }
        if let Some(v) = self.texture_detail {
            add(
                "SDLDrv.SDLClient",
                "TextureDetail",
                match v {
                    TextureDetail::Low => "Low",
                    TextureDetail::Medium => "Medium",
                    TextureDetail::High => "High",
                }
                .into(),
                live,
            );
        }
        if let Some(v) = self.ui_scale {
            add("SDLDrv.SDLClient", "UIScale", v.to_string(), live);
        }
        if let Some(v) = self.maintain_vertical_fov {
            add(
                "SDLDrv.SDLClient",
                "MaintainVerticalFOV",
                bool_text(v).into(),
                live,
            );
        }
        if let Some(v) = self.screen_flashes {
            add(
                "SDLDrv.SDLClient",
                "ScreenFlashes",
                bool_text(v).into(),
                live,
            );
        }
        if let Some(v) = self.joystick_enabled {
            add("SDLDrv.SDLClient", "UseJoystick", bool_text(v).into(), live);
        }
        if let Some(v) = self.sound_enabled {
            add("Engine.GameEngine", "UseSound", bool_text(v).into(), live);
        }
        if let Some(v) = self.sound_volume {
            add(
                "ALAudio.ALAudioSubsystem",
                "SoundVolume",
                v.to_string(),
                live,
            );
        }
        if let Some(v) = self.music_volume {
            add(
                "ALAudio.ALAudioSubsystem",
                "MusicVolume",
                v.to_string(),
                live,
            );
        }
        let mut add_user = |section, key, value: String| {
            changes.push(Change {
                file: ConfigFile::User,
                key: ConfigKey { section, key },
                value,
                mode: ApplyMode::RestartRequired,
            });
        };
        if let Some(v) = self.mouse_sensitivity {
            add_user("Engine.PlayerPawn", "MouseSensitivity", v.to_string());
        }
        if let Some(v) = self.mouse_smoothing {
            add_user("Engine.PlayerPawn", "bMaxMouseSmoothing", bool_text(v).into());
        }
        if let Some(v) = self.invert_mouse {
            add_user("Engine.PlayerPawn", "bInvertMouse", bool_text(v).into());
        }
        if let Some(v) = self.control_mode {
            add_user(
                "Engine.PlayerPawn",
                "bModernThirdPersonControls",
                bool_text(v == ControlMode::Modern).into(),
            );
        }
        if let Some(v) = self.difficulty {
            add_user(
                "Engine.PlayerPawn",
                "Difficulty",
                match v {
                    Difficulty::Easy => "DifficultyEasy",
                    Difficulty::Medium => "DifficultyMedium",
                    Difficulty::Hard => "DifficultyHard",
                }
                .into(),
            );
        }
        if let Some(v) = self.object_detail {
            add_user(
                "Engine.PlayerPawn",
                "ObjectDetail",
                match v {
                    ObjectDetail::VeryLow => "ObjectDetailVeryLow",
                    ObjectDetail::Low => "ObjectDetailLow",
                    ObjectDetail::Medium => "ObjectDetailMedium",
                    ObjectDetail::High => "ObjectDetailHigh",
                    ObjectDetail::VeryHigh => "ObjectDetailVeryHigh",
                }
                .into(),
            );
        }
        if let Some(v) = self.auto_center_camera {
            add_user("HGame.Harry", "bAutoCenterCamera", bool_text(v).into());
        }
        if let Some(v) = self.move_while_casting {
            add_user("HGame.Harry", "bMoveWhileCasting", bool_text(v).into());
        }
        if let Some(v) = self.auto_quaff {
            add_user("HGame.Harry", "bAutoQuaff", bool_text(v).into());
        }
        Ok(SettingsPlan { changes })
    }
}

fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}
