//! Replaces `USDLClient::OpenJoystick`/`CloseJoystick`
//! (`SDLClient.cpp:234-274`).
//!
//! Unlike `mouse_state`/`window_chrome`, this module owns the live
//! `SDL_Joystick*` handle itself (returned to the caller as an opaque
//! pointer token, stored in the real C++'s own `Joystick` field only
//! so it can be handed back to `close_joystick` later -- Rust never
//! reads or writes through it beyond the open/close pair). SDL
//! ref-counts/owns the actual device; this module's only state is
//! "which handle did `open_joystick` last hand out", which it does
//! not even track -- `close_joystick` takes whatever handle the
//! caller passes back, matching the real C++'s own
//! `if (Joystick) { SDL_JoystickClose(Joystick); Joystick = NULL; }`.
//!
//! Verified with `SDL_JoystickAttachVirtual` (SDL2 2.0.14+, present
//! in the pinned 2.32.10): creates a software joystick device with no
//! real hardware or display needed, confirmed empirically to work
//! under the "dummy" video driver (and does not even need video
//! initialized -- `SDL_INIT_JOYSTICK` alone suffices) before writing
//! `Tests/PlatformJoystickTests.cpp`. This makes the device-present
//! path genuinely testable headless, unlike the clipboard/mouse-state/
//! window-chrome cutovers.

#[cfg(feature = "sdl")]
mod ffi {
    #[repr(C)]
    pub struct SdlJoystick {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_InitSubSystem(flags: u32) -> i32;
        pub fn SDL_NumJoysticks() -> i32;
        pub fn SDL_JoystickOpen(device_index: i32) -> *mut SdlJoystick;
        pub fn SDL_JoystickClose(joystick: *mut SdlJoystick);
        pub fn SDL_JoystickEventState(state: i32) -> i32;
        pub fn SDL_JoystickNumButtons(joystick: *mut SdlJoystick) -> i32;
        pub fn SDL_JoystickNumHats(joystick: *mut SdlJoystick) -> i32;
        pub fn SDL_JoystickNumAxes(joystick: *mut SdlJoystick) -> i32;
    }
}

#[cfg(feature = "sdl")]
pub use ffi::SdlJoystick;

#[cfg(feature = "sdl")]
const SDL_INIT_JOYSTICK: u32 = 0x0000_0200;
#[cfg(feature = "sdl")]
const SDL_ENABLE: i32 = 1;

/// `MaxJoystickAxes` (`SDLDrv.h:31`).
const MAX_JOYSTICK_AXES: i32 = 8;
/// The engine's `IK_Joy1..IK_Joy16` range (`SDLClient.cpp:249`'s
/// comment).
const MAX_JOYSTICK_BUTTONS: i32 = 16;
/// Hats fold onto buttons starting at `IK_Joy13`, displacing buttons
/// 13-16 -- only possible once there are more than twelve real
/// buttons to begin with (`SDLClient.cpp:253-256`'s comment).
const HAT_FOLD_BUTTON_THRESHOLD: i32 = 12;

/// Replaces `USDLClient::OpenJoystick`'s derived-field computation
/// (`SDLClient.cpp:249-256`), the pure part once the raw SDL counts
/// are known. Pure logic, no FFI dependency, independently
/// unit-testable without the `sdl` feature. `buttons` deliberately
/// has no lower bound, matching the real C++'s `Min(SDL_
/// JoystickNumButtons(Joystick), 16)` exactly -- `Min` is a plain
/// `(A<=B) ? A : B` (`UnTemplate.h:115-118`), so SDL's own -1 error
/// sentinel for "couldn't determine the count" passes through
/// unmodified as `-1`, not floored to 0. `hats` and `axes` do have a
/// floor/clamp in the real C++ (`Max`/`Clamp`), matched here the same
/// way.
pub fn derive_joystick_fields(
    physical_buttons: i32,
    physical_hats: i32,
    physical_axes: i32,
    ignore_hat: bool,
) -> (i32, i32, i32, bool) {
    let buttons = physical_buttons.min(MAX_JOYSTICK_BUTTONS);
    let hats = physical_hats.max(0);
    let axes = physical_axes.clamp(0, MAX_JOYSTICK_AXES);
    let physical_buttons = physical_buttons.max(0);
    let hat_folds_to_buttons =
        physical_buttons > HAT_FOLD_BUTTON_THRESHOLD && hats > 0 && !ignore_hat;
    (buttons, hats, axes, hat_folds_to_buttons)
}

/// Replaces `USDLClient::OpenJoystick` (`SDLClient.cpp:234-258`),
/// minus the `CloseJoystick()` call at its start -- the real C++
/// still makes that call itself, against its own `Joystick` field,
/// before calling this. Returns `None` on any failure (matching the
/// real C++'s `return 0`), or `Some((handle, buttons, hats, axes,
/// hat_folds_to_buttons))` on success.
///
/// # Safety
/// Safe to call at any time; owns no state across calls.
#[cfg(feature = "sdl")]
pub unsafe fn open_joystick(
    joystick_number: i32,
    ignore_hat: bool,
) -> Option<(*mut SdlJoystick, i32, i32, i32, bool)> {
    // SAFETY: every SDL call below is safe to make at any time; none
    // borrow Rust-side state.
    unsafe {
        if ffi::SDL_InitSubSystem(SDL_INIT_JOYSTICK) < 0 {
            return None;
        }
        let device_count = ffi::SDL_NumJoysticks();
        if joystick_number < 0 || joystick_number >= device_count {
            return None;
        }
        let joystick = ffi::SDL_JoystickOpen(joystick_number);
        if joystick.is_null() {
            return None;
        }
        ffi::SDL_JoystickEventState(SDL_ENABLE);
        let (buttons, hats, axes, hat_folds_to_buttons) = derive_joystick_fields(
            ffi::SDL_JoystickNumButtons(joystick),
            ffi::SDL_JoystickNumHats(joystick),
            ffi::SDL_JoystickNumAxes(joystick),
            ignore_hat,
        );
        Some((joystick, buttons, hats, axes, hat_folds_to_buttons))
    }
}

/// Replaces `USDLClient::CloseJoystick` (`SDLClient.cpp:261-274`)'s
/// SDL half; the caller zeroes its own `JoyButtons`/`JoyHats`/
/// `JoyAxes`/`JoyHatFoldsToButtons`/`Joystick` fields, which Rust does
/// not own. `joystick` may be null (a no-op), matching the real C++'s
/// own `if (Joystick)` guard.
///
/// # Safety
/// `joystick`, if non-null, must be a valid, still-open
/// `SDL_Joystick*` obtained from `open_joystick` and not already
/// closed.
#[cfg(feature = "sdl")]
pub unsafe fn close_joystick(joystick: *mut SdlJoystick) {
    if joystick.is_null() {
        return;
    }
    // SAFETY: non-null checked above; validity is this function's own
    // contract (see above).
    unsafe {
        ffi::SDL_JoystickClose(joystick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `SDLClient.cpp:249-256`'s comments precisely: button
    /// clamp to 16, hat floor at 0, axis clamp to `MaxJoystickAxes`
    /// (8), and the fold-to-buttons threshold (>12 real buttons, at
    /// least one hat, `IgnoreHat` off).
    #[test]
    fn derive_joystick_fields_matches_the_real_cpp_clamps() {
        // Plain clamps, no folding (no hats, or <=12 buttons).
        assert_eq!(derive_joystick_fields(20, 0, 10, false), (16, 0, 8, false));
        assert_eq!(derive_joystick_fields(12, 1, 4, false), (12, 1, 4, false));
        assert_eq!(derive_joystick_fields(14, 0, 4, false), (14, 0, 4, false));
        assert_eq!(derive_joystick_fields(0, 0, 0, false), (0, 0, 0, false));

        // Folding: >12 buttons, >=1 hat, IgnoreHat off. Also exercises
        // the button clamp (20 -> 16) happening independently of the
        // fold decision, which reads the pre-clamp physical count.
        assert_eq!(derive_joystick_fields(14, 1, 4, false), (14, 1, 4, true));
        assert_eq!(derive_joystick_fields(20, 2, 10, false), (16, 2, 8, true));

        // IgnoreHat suppresses folding even when the shape otherwise
        // qualifies.
        assert_eq!(derive_joystick_fields(14, 1, 4, true), (14, 1, 4, false));

        // SDL's own "unknown"/error sentinel is -1. buttons passes it
        // through unmodified (Min has no floor); hats/axes floor to 0
        // (Max(...,0)/Clamp(...,0,N) do); the fold check reads the
        // floored-to-0 value, so it never folds on an error read.
        assert_eq!(derive_joystick_fields(-1, -1, -1, false), (-1, 0, 0, false));
    }
}
