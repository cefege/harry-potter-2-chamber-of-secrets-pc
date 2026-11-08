//! Replaces `HP2LauncherModel.h`'s portable launcher data model: field
//! for field, enum variant for enum variant, same default values.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LaunchAction {
    Continue,
    NewGame,
    Quit,
    #[default]
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScreenMode {
    #[default]
    Windowed,
    Fullscreen,
    BorderlessDesktop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextureDetail {
    Low,
    Medium,
    #[default]
    High,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ObjectDetail {
    VeryLow,
    Low,
    Medium,
    High,
    #[default]
    VeryHigh,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ControlMode {
    #[default]
    Classic,
    Modern,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DataSource {
    #[default]
    Retail,
    Prototype,
}

#[derive(Clone, Debug, Default)]
pub struct DataSourceConfiguration {
    pub selected: DataSource,
    pub retail_root: String,
    pub prototype_root: String,
}

#[derive(Clone, Debug, Default)]
pub struct DataSourceOption {
    pub source: DataSource,
    pub root: String,
    pub available: bool,
    pub error: String,
}

#[derive(Clone, Debug)]
pub struct DisplayResolution {
    pub width: i32,
    pub height: i32,
    pub label: String,
}

impl Default for DisplayResolution {
    fn default() -> Self {
        DisplayResolution {
            width: 800,
            height: 600,
            label: String::new(),
        }
    }
}

pub const RENDER_SCALE_VALUES: [f64; 5] = [0.50, 0.67, 0.75, 0.85, 1.00];
pub const UI_SCALE_VALUES: [f64; 6] = [0.75, 1.00, 1.25, 1.50, 1.75, 2.00];
pub const ANTI_ALIASING_SAMPLE_VALUES: [i32; 3] = [0, 2, 4];
pub const ANISOTROPY_VALUES: [i32; 4] = [0, 4, 8, 16];
pub const FRAME_RATE_LIMIT_VALUES: [i32; 5] = [0, 30, 60, 120, 144];

#[derive(Clone, Debug)]
pub struct SaveRecord {
    pub slot: i32,
    pub save_index: i32,
    pub uses_slot_directory: bool,
    pub save_path: String,
    pub thumbnail_path: String,
    pub display_name: String,
    pub size: i64,
    pub modified_seconds: i64,
}

impl SaveRecord {
    /// Matches `SaveRecord { int slot = -1; ... }` -- the one field C++
    /// defaults away from zero. `Default` is implemented by hand (not
    /// derived) so `slot: -1` is the single, only-possible default --
    /// `#[derive(Default)]` plus a separate `new()` would leave
    /// `SaveRecord::default()` silently giving `slot: 0` (a real save
    /// slot, not "none") wherever something reaches it instead of
    /// `new()`, exactly the kind of divergence this struct's doc note
    /// exists to prevent.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SaveRecord {
    fn default() -> Self {
        SaveRecord {
            slot: -1,
            save_index: 0,
            uses_slot_directory: false,
            save_path: String::new(),
            thumbnail_path: String::new(),
            display_name: String::new(),
            size: 0,
            modified_seconds: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LauncherSettings {
    pub screen_mode: ScreenMode,
    pub resolution: DisplayResolution,
    pub vertical_sync: bool,
    pub render_scale: f64,
    pub ui_scale: f64,
    pub frame_rate_limit: i32,
    pub show_fps: bool,
    pub maintain_vertical_fov: bool,
    pub native_text: bool,
    pub anti_aliasing_samples: i32,
    pub anisotropy: i32,
    pub brightness: f64,
    pub texture_detail: TextureDetail,
    pub object_detail: ObjectDetail,
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub music_volume: f64,
    pub mouse_sensitivity: f64,
    pub invert_mouse: bool,
    pub control_mode: ControlMode,
    pub auto_center_camera: bool,
    pub move_while_casting: bool,
    pub auto_quaff: bool,
    pub screen_flashes: bool,
    pub difficulty: Difficulty,
    pub joystick_enabled: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        LauncherSettings {
            screen_mode: ScreenMode::default(),
            resolution: DisplayResolution::default(),
            vertical_sync: false,
            render_scale: 1.0,
            ui_scale: 1.0,
            frame_rate_limit: 144,
            show_fps: false,
            maintain_vertical_fov: true,
            native_text: true,
            anti_aliasing_samples: 4,
            anisotropy: 16,
            brightness: 0.4,
            texture_detail: TextureDetail::default(),
            object_detail: ObjectDetail::default(),
            sound_enabled: true,
            sound_volume: 0.9,
            music_volume: 0.53,
            mouse_sensitivity: 3.0,
            invert_mouse: false,
            control_mode: ControlMode::default(),
            auto_center_camera: true,
            move_while_casting: true,
            auto_quaff: true,
            screen_flashes: true,
            difficulty: Difficulty::default(),
            joystick_enabled: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LauncherPaths {
    pub user_root: String,
    pub system_root: String,
}

#[derive(Clone, Debug, Default)]
pub struct LauncherState {
    pub settings: LauncherSettings,
    pub saves: Vec<SaveRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct LauncherRequest {
    pub settings: LauncherSettings,
    pub saves: Vec<SaveRecord>,
    pub display_modes: Vec<DisplayResolution>,
    pub error_message: String,
    pub user_root: String,
    pub log_path: String,
    pub data_source_options: Vec<DataSourceOption>,
    pub data_sources: DataSourceConfiguration,
    pub has_explicit_data_root_override: bool,
    pub explicit_data_root: String,
    pub retail_data_root: String,
    pub needs_game_data: bool,
}

#[derive(Clone, Debug)]
pub struct LaunchSelection {
    pub action: LaunchAction,
    pub has_save: bool,
    pub save: SaveRecord,
}

impl LaunchSelection {
    /// Matches `LaunchSelection { LaunchAction action = LaunchAction::Quit;
    /// ... }` -- the default C++ *selection* is `Quit` (the safe
    /// fallback), not `Error` (`LaunchAction`'s own bare default,
    /// correct for `LaunchAction` itself -- `LauncherAutomationRequest`
    /// uses that default directly). `Default` is implemented by hand
    /// (not derived) specifically so every path to a default
    /// `LaunchSelection` -- this constructor, `Default::default()`
    /// calls elsewhere, and anything that transitively derives
    /// `Default` through this type (`LauncherResult`) -- agrees on
    /// `Quit`, with no second, divergent "derived" default to
    /// accidentally reach instead.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for LaunchSelection {
    fn default() -> Self {
        LaunchSelection {
            action: LaunchAction::Quit,
            has_save: false,
            save: SaveRecord::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LauncherResult {
    pub selection: LaunchSelection,
    pub settings: LauncherSettings,
    pub data_sources: DataSourceConfiguration,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `LauncherSettings::default()` field against the literal
    /// `HP2LauncherModel.h` member initializers (`struct LauncherSettings`),
    /// not merely internal self-consistency with another default-derived
    /// value.
    #[test]
    fn launcher_settings_default_matches_cpp_member_initializers() {
        let d = LauncherSettings::default();
        assert_eq!(d.screen_mode, ScreenMode::Windowed);
        assert_eq!(d.resolution.width, 800);
        assert_eq!(d.resolution.height, 600);
        assert_eq!(d.resolution.label, "");
        assert!(!d.vertical_sync);
        assert_eq!(d.render_scale, 1.0);
        assert_eq!(d.ui_scale, 1.0);
        assert_eq!(d.frame_rate_limit, 144);
        assert!(!d.show_fps);
        assert!(d.maintain_vertical_fov);
        assert!(d.native_text);
        assert_eq!(d.anti_aliasing_samples, 4);
        assert_eq!(d.anisotropy, 16);
        assert_eq!(d.brightness, 0.4);
        assert_eq!(d.texture_detail, TextureDetail::High);
        assert_eq!(d.object_detail, ObjectDetail::VeryHigh);
        assert!(d.sound_enabled);
        assert_eq!(d.sound_volume, 0.9);
        assert_eq!(d.music_volume, 0.53);
        assert_eq!(d.mouse_sensitivity, 3.0);
        assert!(!d.invert_mouse);
        assert_eq!(d.control_mode, ControlMode::Classic);
        assert!(d.auto_center_camera);
        assert!(d.move_while_casting);
        assert!(d.auto_quaff);
        assert!(d.screen_flashes);
        assert_eq!(d.difficulty, Difficulty::Easy);
        assert!(d.joystick_enabled);
    }

    /// `DisplayResolution { int width = 800; int height = 600; }`.
    #[test]
    fn display_resolution_default_matches_cpp_member_initializers() {
        let d = DisplayResolution::default();
        assert_eq!(d.width, 800);
        assert_eq!(d.height, 600);
        assert_eq!(d.label, "");
    }

    /// `SaveRecord { int slot = -1; }`, the one field C++ defaults away
    /// from zero.
    #[test]
    fn save_record_default_matches_cpp_member_initializers() {
        let d = SaveRecord::default();
        assert_eq!(d.slot, -1);
        assert_eq!(d.save_index, 0);
        assert!(!d.uses_slot_directory);
        assert_eq!(d.size, 0);
        assert_eq!(d.modified_seconds, 0);
    }

    /// `LaunchSelection`'s hand-written `Default` -- `Quit`, not
    /// `LaunchAction`'s own bare `#[default]` (`Error`). See the doc
    /// comment on `impl LaunchSelection` for why this must stay hand-written.
    #[test]
    fn launch_selection_default_is_quit_not_launch_actions_own_default() {
        assert_eq!(LaunchSelection::default().action, LaunchAction::Quit);
        assert_ne!(LaunchAction::default(), LaunchAction::Quit);
    }
}
