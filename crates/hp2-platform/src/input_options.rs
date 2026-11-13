//! Player-facing input options that tune the SDL input pump without touching
//! the stock `PlayerPawn.MouseSensitivity` script path.
//!
//! The engine reads the persisted values (`[SDLDrv.SDLClient]` in `Game.ini`)
//! and publishes them here; the pump reads them once per event/frame. Every
//! default reproduces the original behaviour: multipliers of exactly `1.0`
//! leave deltas bit-identical and `RawMouse=False` never touches an SDL hint.
//!
//! * `MouseSensitivityX` / `MouseSensitivityY` multiply the relative mouse
//!   deltas on top of `MouseSensitivity` (applied before the script sees
//!   `aMouseX`/`aMouseY`).
//! * `GamepadLookSensitivity` multiplies the right-stick look axes of the
//!   SDL game-controller layer (`game_controller`).
//! * `RawMouse` pins SDL's relative mouse mode to unscaled raw motion.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Smallest accepted sensitivity multiplier.
pub const MIN_SENSITIVITY: f32 = 0.1;
/// Largest accepted sensitivity multiplier.
pub const MAX_SENSITIVITY: f32 = 10.0;
/// Default of every multiplier: the original behaviour.
pub const DEFAULT_SENSITIVITY: f32 = 1.0;

static MOUSE_SENSITIVITY_X: AtomicU32 = AtomicU32::new(DEFAULT_SENSITIVITY.to_bits());
static MOUSE_SENSITIVITY_Y: AtomicU32 = AtomicU32::new(DEFAULT_SENSITIVITY.to_bits());
static GAMEPAD_LOOK_SENSITIVITY: AtomicU32 = AtomicU32::new(DEFAULT_SENSITIVITY.to_bits());
static RAW_MOUSE: AtomicBool = AtomicBool::new(false);

/// Clamps a requested multiplier into the accepted range; non-finite input
/// falls back to the default.
pub fn clamp_sensitivity(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_SENSITIVITY, MAX_SENSITIVITY)
    } else {
        DEFAULT_SENSITIVITY
    }
}

/// Current horizontal mouse multiplier.
pub fn mouse_sensitivity_x() -> f32 {
    f32::from_bits(MOUSE_SENSITIVITY_X.load(Ordering::Relaxed))
}

/// Current vertical mouse multiplier.
pub fn mouse_sensitivity_y() -> f32 {
    f32::from_bits(MOUSE_SENSITIVITY_Y.load(Ordering::Relaxed))
}

/// Current right-stick look multiplier.
pub fn gamepad_look_sensitivity() -> f32 {
    f32::from_bits(GAMEPAD_LOOK_SENSITIVITY.load(Ordering::Relaxed))
}

/// Publishes the horizontal mouse multiplier (clamped); returns the stored value.
pub fn set_mouse_sensitivity_x(value: f32) -> f32 {
    let value = clamp_sensitivity(value);
    MOUSE_SENSITIVITY_X.store(value.to_bits(), Ordering::Relaxed);
    value
}

/// Publishes the vertical mouse multiplier (clamped); returns the stored value.
pub fn set_mouse_sensitivity_y(value: f32) -> f32 {
    let value = clamp_sensitivity(value);
    MOUSE_SENSITIVITY_Y.store(value.to_bits(), Ordering::Relaxed);
    value
}

/// Publishes the right-stick look multiplier (clamped); returns the stored value.
pub fn set_gamepad_look_sensitivity(value: f32) -> f32 {
    let value = clamp_sensitivity(value);
    GAMEPAD_LOOK_SENSITIVITY.store(value.to_bits(), Ordering::Relaxed);
    value
}

/// Whether raw mouse input is requested.
pub fn raw_mouse() -> bool {
    RAW_MOUSE.load(Ordering::Relaxed)
}

/// The SDL hints `RawMouse=True` pins, as `(name, value)`.
///
/// Relative mode uses the OS raw-input path instead of cursor warping, and
/// neither the system acceleration curve nor an application speed scale is
/// applied to the deltas. (macOS reports deltas after the OS pointer curve;
/// SDL 2 offers no switch for that, so the pin is a no-op beyond SDL there.)
pub const RAW_MOUSE_HINTS: [(&str, &str); 3] = [
    ("SDL_MOUSE_RELATIVE_MODE_WARP", "0"),
    ("SDL_MOUSE_RELATIVE_SYSTEM_SCALE", "0"),
    ("SDL_MOUSE_RELATIVE_SPEED_SCALE", "1"),
];

/// Publishes `RawMouse` and applies it to SDL. Enabling sets the hints in
/// [`RAW_MOUSE_HINTS`]; disabling resets exactly those hints (only when this
/// module set them) so the default state never differs from stock SDL. SDL
/// re-reads the speed/system scale hints live; the warp hint takes effect the
/// next time relative mode is entered (every return to gameplay).
pub fn set_raw_mouse(enabled: bool) {
    let previous = RAW_MOUSE.swap(enabled, Ordering::Relaxed);
    #[cfg(feature = "sdl")]
    if enabled != previous {
        use std::ffi::CString;
        for (name, value) in RAW_MOUSE_HINTS {
            let name = CString::new(name).expect("hint names contain no NUL");
            let value = CString::new(value).expect("hint values contain no NUL");
            // SAFETY: both strings are valid and NUL-terminated for the call.
            unsafe {
                if enabled {
                    ffi::SDL_SetHint(name.as_ptr(), value.as_ptr());
                } else {
                    ffi::SDL_ResetHint(name.as_ptr());
                }
            }
        }
    }
    #[cfg(not(feature = "sdl"))]
    let _ = previous;
}

#[cfg(feature = "sdl")]
mod ffi {
    use std::ffi::c_char;
    unsafe extern "C" {
        pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> i32;
        pub fn SDL_ResetHint(name: *const c_char) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_is_clamped_and_defaults_are_identity() {
        assert_eq!(DEFAULT_SENSITIVITY, 1.0);
        assert_eq!(clamp_sensitivity(0.0), MIN_SENSITIVITY);
        assert_eq!(clamp_sensitivity(99.0), MAX_SENSITIVITY);
        assert_eq!(clamp_sensitivity(f32::NAN), DEFAULT_SENSITIVITY);
        assert_eq!(clamp_sensitivity(f32::INFINITY), DEFAULT_SENSITIVITY);
        assert_eq!(clamp_sensitivity(2.5), 2.5);
    }
}
