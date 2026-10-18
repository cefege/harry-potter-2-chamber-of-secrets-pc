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
