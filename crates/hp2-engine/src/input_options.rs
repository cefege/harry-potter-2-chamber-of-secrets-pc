//! Player-facing input options persisted in `Game.ini` `[SDLDrv.SDLClient]`.
//!
//! | Key | Type / range | Default | Effect |
//! |---|---|---|---|
//! | `RawMouse` | bool | `False` | Pins SDL relative mouse mode to unscaled raw motion (no OS acceleration curve, no warp emulation). |
//! | `MouseSensitivityX` | float 0.1..=10 | `1.0` | Horizontal multiplier on top of `PlayerPawn.MouseSensitivity`. |
//! | `MouseSensitivityY` | float 0.1..=10 | `1.0` | Vertical multiplier on top of `PlayerPawn.MouseSensitivity`. |
//! | `GamepadLookSensitivity` | float 0.1..=10 | `1.0` | Right-stick look multiplier for game controllers. |
//! | `UseGameController` | bool | `True` | Map recognized gamepads through SDL GameController onto the `Joy*` keys (needs `UseJoystick`); `False` keeps the raw SDL joystick layout. |
//!
//! Each setter persists through `config_update_system_staged` first and only
//! then applies the value live, so a failed write changes nothing. The values
//! reach the SDL input pump through `hp2_platform::input_options` and
//! `hp2_platform::game_controller`.

use hp2_core::core_access::{CoreAccess, RuntimeResult};
use hp2_platform::input_options as platform;

pub(crate) const SECTION: &str = "SDLDrv.SDLClient";
pub(crate) const KEY_RAW_MOUSE: &str = "RawMouse";
pub(crate) const KEY_MOUSE_SENSITIVITY_X: &str = "MouseSensitivityX";
pub(crate) const KEY_MOUSE_SENSITIVITY_Y: &str = "MouseSensitivityY";
pub(crate) const KEY_GAMEPAD_LOOK_SENSITIVITY: &str = "GamepadLookSensitivity";
pub(crate) const KEY_USE_GAME_CONTROLLER: &str = "UseGameController";

fn units(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn read(access: CoreAccess<'_>, key: &str) -> Option<String> {
    access
        .config_get_units(&units(SECTION), &units(key))
        .ok()
        .flatten()
        .map(|v| v.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect())
}

fn parse_bool(text: &str, default: bool) -> bool {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => true,
        "false" | "off" | "no" | "0" => false,
        _ => default,
    }
}

fn read_bool(access: CoreAccess<'_>, key: &str, default: bool) -> bool {
    read(access, key).map_or(default, |v| parse_bool(&v, default))
}

fn read_sensitivity(access: CoreAccess<'_>, key: &str) -> f32 {
    read(access, key)
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map_or(platform::DEFAULT_SENSITIVITY, platform::clamp_sensitivity)
}

fn persist(access: CoreAccess<'_>, key: &str, value: &str) -> RuntimeResult<()> {
    access.config_update_system_staged(&[(units(SECTION), units(key), units(value))])
}

fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Persisted `RawMouse` (default `false`).
pub(crate) fn raw_mouse(access: CoreAccess<'_>) -> bool {
    read_bool(access, KEY_RAW_MOUSE, false)
}

/// Persists and applies `RawMouse`.
pub(crate) fn set_raw_mouse(access: CoreAccess<'_>, on: bool) -> RuntimeResult<()> {
    persist(access, KEY_RAW_MOUSE, bool_text(on))?;
    platform::set_raw_mouse(on);
    Ok(())
}

/// Persisted `MouseSensitivityX` (default `1.0`).
pub(crate) fn mouse_sensitivity_x(access: CoreAccess<'_>) -> f32 {
    read_sensitivity(access, KEY_MOUSE_SENSITIVITY_X)
}

/// Persists and applies `MouseSensitivityX` (clamped to 0.1..=10).
pub(crate) fn set_mouse_sensitivity_x(access: CoreAccess<'_>, value: f32) -> RuntimeResult<()> {
    let value = platform::clamp_sensitivity(value);
    persist(access, KEY_MOUSE_SENSITIVITY_X, &format!("{value:.6}"))?;
    platform::set_mouse_sensitivity_x(value);
    Ok(())
}

/// Persisted `MouseSensitivityY` (default `1.0`).
pub(crate) fn mouse_sensitivity_y(access: CoreAccess<'_>) -> f32 {
    read_sensitivity(access, KEY_MOUSE_SENSITIVITY_Y)
}

/// Persists and applies `MouseSensitivityY` (clamped to 0.1..=10).
pub(crate) fn set_mouse_sensitivity_y(access: CoreAccess<'_>, value: f32) -> RuntimeResult<()> {
    let value = platform::clamp_sensitivity(value);
    persist(access, KEY_MOUSE_SENSITIVITY_Y, &format!("{value:.6}"))?;
    platform::set_mouse_sensitivity_y(value);
    Ok(())
}

/// Persisted `GamepadLookSensitivity` (default `1.0`).
pub(crate) fn gamepad_look_sensitivity(access: CoreAccess<'_>) -> f32 {
    read_sensitivity(access, KEY_GAMEPAD_LOOK_SENSITIVITY)
}

/// Persists and applies `GamepadLookSensitivity` (clamped to 0.1..=10).
pub(crate) fn set_gamepad_look_sensitivity(access: CoreAccess<'_>, value: f32) -> RuntimeResult<()> {
    let value = platform::clamp_sensitivity(value);
    persist(access, KEY_GAMEPAD_LOOK_SENSITIVITY, &format!("{value:.6}"))?;
    platform::set_gamepad_look_sensitivity(value);
    Ok(())
}

/// Persisted `UseGameController` (default `true`).
pub(crate) fn use_game_controller(access: CoreAccess<'_>) -> bool {
    read_bool(access, KEY_USE_GAME_CONTROLLER, true)
}

/// Persists and applies `UseGameController`; disabling releases a held pad.
pub(crate) fn set_use_game_controller(access: CoreAccess<'_>, on: bool) -> RuntimeResult<()> {
    persist(access, KEY_USE_GAME_CONTROLLER, bool_text(on))?;
    #[cfg(feature = "sdl")]
    hp2_platform::game_controller::set_enabled(on);
    Ok(())
}

/// Publishes every persisted value to the platform layer at client start-up.
/// `joysticks_allowed` is the client's `UseJoystick`; game controllers need it.
pub(crate) fn apply_persisted(access: CoreAccess<'_>, joysticks_allowed: bool) {
    platform::set_mouse_sensitivity_x(mouse_sensitivity_x(access));
    platform::set_mouse_sensitivity_y(mouse_sensitivity_y(access));
    platform::set_gamepad_look_sensitivity(gamepad_look_sensitivity(access));
    platform::set_raw_mouse(raw_mouse(access));
    hp2_platform::game_controller::set_allowed(joysticks_allowed);
    #[cfg(feature = "sdl")]
    if !use_game_controller(access) {
        hp2_platform::game_controller::set_enabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_text_parses_the_ini_spellings() {
        assert!(parse_bool("True", false));
        assert!(parse_bool(" on ", false));
        assert!(!parse_bool("False", true));
        assert!(!parse_bool("0", true));
        assert!(parse_bool("garbage", true));
        assert!(!parse_bool("garbage", false));
    }
}
