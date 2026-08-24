//! Launcher settings data model shared by policy, store, and UI layers.
//!
//! Discrete choices and defaults mirror
//! `HarryPotter2/Unreal/SDLLaunch/Src/HP2LauncherModel.h`.

/// Screen mode (`ScreenMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenMode {
    Windowed,
    Fullscreen,
    BorderlessDesktop,
}

/// Texture detail (`TextureDetail`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureDetail {
    Low,
    Medium,
    High,
}

/// Object detail (`ObjectDetail`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectDetail {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Difficulty (`Difficulty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

/// Control mode (`ControlMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Classic,
    Modern,
}

/// Render backend (`RenderBackend`). The rewrite launches a single wgpu
/// backend regardless; this persists which renderer-era device class the
/// engine INIs name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    XOpenGL,
    Vulkan,
}

macro_rules! indexable {
    ($name:ident, $from:expr, $count:expr) => {
        impl $name {
            pub fn as_index(self) -> usize {
                self as usize
            }

            pub fn from_index(index: usize) -> Self {
                const COUNT: usize = $count;
                assert!(index < COUNT);
                $from[index]
            }
        }
    };
}

indexable!(TextureDetail, [Self::Low, Self::Medium, Self::High], 3);
indexable!(
    ObjectDetail,
    [
        Self::VeryLow,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::VeryHigh
    ],
    5
);
indexable!(Difficulty, [Self::Easy, Self::Medium, Self::Hard], 3);
indexable!(ControlMode, [Self::Classic, Self::Modern], 2);
indexable!(RenderBackend, [Self::XOpenGL, Self::Vulkan], 2);

impl ScreenMode {
    pub fn as_index(self) -> usize {
        self as usize
    }
}

/// Accepted discrete render scales (`RenderScaleValues`).
pub const RENDER_SCALES: [f64; 5] = [0.50, 0.67, 0.75, 0.85, 1.00];
/// Accepted discrete UI scales (`UIScaleValues`).
pub const UI_SCALES: [f64; 6] = [0.75, 1.00, 1.25, 1.50, 1.75, 2.00];
/// Accepted MSAA sample counts (`AntiAliasingSampleValues`).
pub const AA_SAMPLE_VALUES: [i32; 3] = [0, 2, 4];
/// Accepted anisotropy values (`AnisotropyValues`).
pub const ANISOTROPY_VALUES: [i32; 4] = [0, 4, 8, 16];
/// Accepted frame caps (`FrameRateLimitValues`; 0 = unlimited).
pub const FRAME_RATE_LIMITS: [i32; 5] = [0, 30, 60, 120, 144];

/// Resolution pair with a derived display label (`DisplayResolution`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
    pub label: String,
}

/// The launcher settings model (`LauncherSettings`) with its documented
/// defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub screen_mode: ScreenMode,
    pub resolution: Resolution,
    pub render_backend: RenderBackend,
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            screen_mode: ScreenMode::Windowed,
            resolution: Resolution {
                width: 800,
                height: 600,
                label: "800 x 600".to_string(),
            },
            render_backend: RenderBackend::XOpenGL,
            vertical_sync: false,
            render_scale: 1.0,
            ui_scale: 1.0,
            frame_rate_limit: 60,
            show_fps: false,
            maintain_vertical_fov: true,
            native_text: true,
            anti_aliasing_samples: 0,
            anisotropy: 4,
            brightness: 0.4,
            texture_detail: TextureDetail::High,
            object_detail: ObjectDetail::Medium,
            sound_enabled: true,
            sound_volume: 0.9,
            music_volume: 0.53,
            mouse_sensitivity: 3.0,
            invert_mouse: false,
            control_mode: ControlMode::Classic,
            auto_center_camera: true,
            move_while_casting: true,
            auto_quaff: true,
            screen_flashes: true,
            difficulty: Difficulty::Easy,
            joystick_enabled: true,
        }
    }
}

impl Settings {
    /// Recomputes the resolution label after a dimension change.
    pub fn refresh_resolution_label(&mut self) {
        self.resolution.label = format!("{} x {}", self.resolution.width, self.resolution.height);
    }
}
