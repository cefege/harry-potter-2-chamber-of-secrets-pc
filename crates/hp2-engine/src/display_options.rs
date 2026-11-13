//! Player-facing display options: fullscreen flavour, target monitor and HUD size.
//!
//! All three persist in `[SDLDrv.SDLClient]` of the user `Game.ini` through Core's staged
//! config API and every default reproduces the original behaviour (borderless desktop
//! fullscreen, display 0, HUD at 100 %). The getters only read config; the setters write
//! config and apply live where SDL allows it.
//!
//! | Key | Type | Range | Default |
//! |---|---|---|---|
//! | `FullscreenMode` | `Exclusive` / `Borderless` / `NativeSpaces` | | `Borderless` |
//! | `Display` | integer display index | `0..displays-1` | `0` |
//! | `HudScale` | percent | `50..=200` | `100` |
use std::sync::atomic::{AtomicU32, Ordering};

use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
#[cfg(feature = "sdl")]
use hp2_platform::live_display_mode::{Backend, DisplayMode};

const SECTION: &str = "SDLDrv.SDLClient";
const KEY_FULLSCREEN_MODE: &str = "FullscreenMode";
const KEY_DISPLAY: &str = "Display";
const KEY_HUD_SCALE: &str = "HudScale";

/// Smallest and largest accepted HUD size, in percent.
pub(crate) const HUD_SCALE_MIN_PERCENT: f32 = 50.0;
pub(crate) const HUD_SCALE_MAX_PERCENT: f32 = 200.0;
pub(crate) const HUD_SCALE_DEFAULT_PERCENT: f32 = 100.0;

/// What "fullscreen" means. `Exclusive` is SDL's real fullscreen (display mode change),
/// `Borderless` the desktop-sized borderless window, `NativeSpaces` the macOS green-button
/// Space (SDL reads that choice once at video init, so it needs a restart).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FullscreenMode { Exclusive, Borderless, NativeSpaces }

impl FullscreenMode {
    pub(crate) const ALL: [Self; 3] = [Self::Exclusive, Self::Borderless, Self::NativeSpaces];
    pub(crate) const fn name(self) -> &'static str {
        match self { Self::Exclusive => "Exclusive", Self::Borderless => "Borderless", Self::NativeSpaces => "NativeSpaces" }
    }
    /// Combo caption for the option.
    pub(crate) const fn caption(self) -> &'static str {
        match self { Self::Exclusive => "Exclusive", Self::Borderless => "Borderless", Self::NativeSpaces => "macOS Spaces" }
    }
    pub(crate) fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| text.trim().eq_ignore_ascii_case(mode.name()))
    }
    pub(crate) fn index(self) -> usize { Self::ALL.iter().position(|mode| *mode == self).unwrap_or(1) }
    pub(crate) fn from_index(index: usize) -> Self { Self::ALL.get(index).copied().unwrap_or(Self::Borderless) }
    /// The SDL window state this flavour enters; `NativeSpaces` is borderless-desktop SDL
    /// state with the Spaces hint selected at launch.
    #[cfg(feature = "sdl")]
    pub(crate) fn display_mode(self) -> DisplayMode {
        match self { Self::Exclusive => DisplayMode::ExclusiveFullscreen, Self::Borderless | Self::NativeSpaces => DisplayMode::BorderlessFullscreen }
    }
}

fn units(text: &str) -> Vec<u32> { text.chars().map(u32::from).collect() }

fn read_text(access: CoreAccess<'_>, key: &str) -> Option<String> {
    let value = access.config_get_units_from(None, &units(SECTION), &units(key)).ok().flatten()?;
    let text: String = value.into_iter().take_while(|&c| c != 0).filter_map(char::from_u32).collect();
    Some(text.trim().to_owned())
}

fn write_texts(access: CoreAccess<'_>, pairs: &[(&str, String)]) -> RuntimeResult<()> {
    let rows: Vec<_> = pairs.iter().map(|(key, value)| (units(SECTION), units(key), units(value))).collect();
    access.config_update_system_staged(&rows)
}

// ---- FullscreenMode ---------------------------------------------------------------------

/// Persisted fullscreen flavour; an absent or invalid value is the original `Borderless`.
pub(crate) fn fullscreen_mode(access: CoreAccess<'_>) -> FullscreenMode {
    read_text(access, KEY_FULLSCREEN_MODE).and_then(|text| FullscreenMode::parse(&text)).unwrap_or(FullscreenMode::Borderless)
}

/// `FullscreenMode` string `ViewportBackend::persist` must store after entering `mode`
/// (`None` leaves it alone): the combo's explicit Exclusive/Borderless choices keep the
/// flavour in step, while a Spaces preference survives a Borderless window.
#[cfg(feature = "sdl")]
pub(crate) fn persisted_flavour(access: CoreAccess<'_>, mode: DisplayMode) -> Option<(&'static str, String)> {
    let value = match mode {
        DisplayMode::Windowed => return None,
        DisplayMode::ExclusiveFullscreen => FullscreenMode::Exclusive,
        DisplayMode::BorderlessFullscreen if fullscreen_mode(access) == FullscreenMode::NativeSpaces => FullscreenMode::NativeSpaces,
        DisplayMode::BorderlessFullscreen => FullscreenMode::Borderless,
    };
    Some((KEY_FULLSCREEN_MODE, value.name().to_owned()))
}

/// Publishes the launch-time Spaces choice. Call after config load and before `SDL_Init`.
pub fn publish_boot_preferences(access: CoreAccess<'_>) {
    #[cfg(feature = "sdl")]
    hp2_platform::sdl_runtime::request_native_spaces(fullscreen_mode(access) == FullscreenMode::NativeSpaces);
    #[cfg(not(feature = "sdl"))]
    let _ = access;
}

/// Saves the fullscreen flavour. If the window is fullscreen right now, Exclusive and
/// Borderless are entered immediately. Returns `true` when the choice is already in effect;
/// `false` when it applies from the next launch (any change to or from `NativeSpaces`,
/// whose SDL hint is read once at startup).
pub(crate) fn set_fullscreen_mode(access: CoreAccess<'_>, mode: FullscreenMode) -> RuntimeResult<bool> {
    let previous = fullscreen_mode(access);
    write_texts(access, &[(KEY_FULLSCREEN_MODE, mode.name().to_owned())])?;
    if (previous == FullscreenMode::NativeSpaces) != (mode == FullscreenMode::NativeSpaces) {
        return Ok(false);
    }
    #[cfg(feature = "sdl")]
    if let Some(viewport) = live_viewport(access) {
        let mut backend = unsafe { crate::stock_display_transition::ViewportBackend::new(access, viewport)? };
        let current = backend.current()?;
        let wanted = mode.display_mode();
        if current != DisplayMode::Windowed && current != wanted {
            if let Err(error) = backend.apply(wanted) {
                let _ = backend.apply(current);
                return Err(error);
            }
            backend.persist(wanted)?;
        }
    }
    Ok(true)
}

#[cfg(feature = "sdl")]
fn live_viewport(access: CoreAccess<'_>) -> Option<*mut hp2_abi::engine::generated::USDLViewport> {
    crate::engine_owner::platform_state(access).ok()?.primary_viewport()
}

// ---- Display ----------------------------------------------------------------------------

/// Connected monitors (1 without SDL video).
pub(crate) fn display_count() -> i32 {
    #[cfg(feature = "sdl")]
    { hp2_platform::window_lifecycle::display_count() }
    #[cfg(not(feature = "sdl"))]
    { 1 }
}

/// Persisted monitor index, clamped to the monitors connected now (an unplugged monitor falls
/// back to the nearest one); default 0.
pub(crate) fn display_index(access: CoreAccess<'_>) -> i32 {
    let saved = read_text(access, KEY_DISPLAY).and_then(|text| text.parse::<i32>().ok()).unwrap_or(0);
    clamp_display(saved, display_count())
}

fn clamp_display(display: i32, count: i32) -> i32 {
    let last = count.max(1) - 1;
    display.clamp(0, last)
}

/// Saves the monitor and moves the game there at once: a windowed window is re-centered on
/// it; a fullscreen one leaves fullscreen, moves, and re-enters its mode.
pub(crate) fn set_display_index(access: CoreAccess<'_>, display: i32) -> RuntimeResult<()> {
    let display = clamp_display(display, display_count());
    write_texts(access, &[(KEY_DISPLAY, display.to_string())])?;
    #[cfg(feature = "sdl")]
    if let Some(viewport) = live_viewport(access) {
        let mut backend = unsafe { crate::stock_display_transition::ViewportBackend::new(access, viewport)? };
        backend.move_to_display(display)?;
    }
    Ok(())
}

// ---- HudScale ---------------------------------------------------------------------------

/// Cached multiplier bits; 0 means "not read yet" (a real multiplier is never 0).
static HUD_SCALE: AtomicU32 = AtomicU32::new(0);

/// Clamps a requested HUD size to 50..=200 percent; non-finite input is the default.
pub(crate) fn clamp_hud_percent(percent: f32) -> f32 {
    if percent.is_finite() { percent.clamp(HUD_SCALE_MIN_PERCENT, HUD_SCALE_MAX_PERCENT) } else { HUD_SCALE_DEFAULT_PERCENT }
}

/// Persisted HUD size in percent (50..=200), default 100.
pub(crate) fn hud_scale_percent(access: CoreAccess<'_>) -> f32 {
    read_text(access, KEY_HUD_SCALE).and_then(|text| text.parse::<f32>().ok()).map_or(HUD_SCALE_DEFAULT_PERCENT, clamp_hud_percent)
}

/// Saves the HUD size (clamped to 50..=200 %) and applies it to the next frame.
pub(crate) fn set_hud_scale_percent(access: CoreAccess<'_>, percent: f32) -> RuntimeResult<()> {
    if !percent.is_finite() { return Err(RuntimeError::new("HudScale must be a finite percentage")); }
    let percent = clamp_hud_percent(percent);
    write_texts(access, &[(KEY_HUD_SCALE, format_percent(percent))])?;
    HUD_SCALE.store((percent / 100.0).to_bits(), Ordering::Relaxed);
    Ok(())
}

fn format_percent(percent: f32) -> String {
    if percent.fract() == 0.0 { format!("{}", percent as i32) } else { format!("{percent:.1}") }
}

/// Multiplier (percent / 100) applied by the Rust HUD components to their size; exactly
/// `1.0` by default, or when the stock HUD is forced (`HP2_STOCK_HUD` present).
pub(crate) fn hud_scale(access: CoreAccess<'_>) -> f32 {
    if std::env::var_os("HP2_STOCK_HUD").is_some() { return 1.0; }
    let bits = HUD_SCALE.load(Ordering::Relaxed);
    if bits != 0 { return f32::from_bits(bits); }
    let scale = hud_scale_percent(access) / 100.0;
    HUD_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_core::core_access::{CoreOptions, CoreOwner};
    use std::{fs, path::PathBuf};

    #[test]
    fn fullscreen_mode_names_round_trip_and_default_is_borderless() {
        for mode in FullscreenMode::ALL {
            assert_eq!(FullscreenMode::parse(mode.name()), Some(mode));
            assert_eq!(FullscreenMode::from_index(mode.index()), mode);
        }
        assert_eq!(FullscreenMode::parse(" nativespaces "), Some(FullscreenMode::NativeSpaces));
        assert_eq!(FullscreenMode::parse("fullscreen"), None);
        assert_eq!(FullscreenMode::from_index(99), FullscreenMode::Borderless);
    }

    #[test]
    fn display_and_hud_values_clamp() {
        assert_eq!(clamp_display(-3, 2), 0);
        assert_eq!(clamp_display(1, 2), 1);
        assert_eq!(clamp_display(5, 2), 1);
        assert_eq!(clamp_display(5, 0), 0);
        assert_eq!(clamp_hud_percent(10.0), 50.0);
        assert_eq!(clamp_hud_percent(999.0), 200.0);
        assert_eq!(clamp_hud_percent(f32::NAN), 100.0);
        assert_eq!(clamp_hud_percent(125.0), 125.0);
        assert_eq!(format_percent(150.0), "150");
        assert_eq!(format_percent(62.5), "62.5");
    }

    #[test]
    fn options_persist_in_game_ini_and_defaults_are_original() {
        fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR")).join(format!("display-options-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let ini = root.join("Game.ini");
        fs::write(&ini, b"[SDLDrv.SDLClient]\nOther=unchanged\n").unwrap();
        let open = || CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: ini.clone(), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        let first = open();
        let access = first.access();
        assert_eq!(fullscreen_mode(access), FullscreenMode::Borderless);
        assert_eq!(display_index(access), 0);
        assert_eq!(hud_scale_percent(access), 100.0);
        // No live viewport in a unit test: the setters only persist.
        assert!(set_fullscreen_mode(access, FullscreenMode::Exclusive).unwrap());
        assert!(!set_fullscreen_mode(access, FullscreenMode::NativeSpaces).unwrap());
        set_display_index(access, 7).unwrap();
        set_hud_scale_percent(access, 175.0).unwrap();
        assert!(set_hud_scale_percent(access, f32::INFINITY).is_err());
        first.shutdown().unwrap();
        let text = fs::read_to_string(&ini).unwrap();
        assert!(text.contains("FullscreenMode=NativeSpaces"), "{text}");
        assert!(text.contains("HudScale=175"), "{text}");
        assert!(text.contains("Other=unchanged"), "{text}");
        let second = open();
        let access = second.access();
        assert_eq!(fullscreen_mode(access), FullscreenMode::NativeSpaces);
        assert_eq!(hud_scale_percent(access), 175.0);
        assert_eq!(display_index(access), clamp_display(7, display_count()));
        second.shutdown().unwrap();
    }
}
