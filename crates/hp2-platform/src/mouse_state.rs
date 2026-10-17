//! Replaces `USDLViewport::GetMouseState`/`GetViewportButtonFlags`/
//! `UpdateMouseGrabState` (`SDLViewport.cpp:544-579`) and the file's
//! own `ButtonsFromSdlState` helper (`:46-53`).
//!
//! `SDL_GetMouseState`, `SDL_SetRelativeMouseMode`, and
//! `SDL_ShowCursor` are all process-global SDL calls with no window
//! parameter at all -- only `SDL_SetWindowGrab` needs a specific
//! `SDL_Window*`, which the real C++ already owns
//! (`USDLViewport::SdlWindow`) and passes in here as an opaque
//! pointer; this module never creates, destroys, or otherwise owns
//! that pointer, matching the "C++ owns objects, Rust operates on
//! them given a pointer" rule for everything short of full Phase 5.

/// `SDL_BUTTON_LMASK`/`MMASK`/`RMASK` (`SDL_mouse.h`:
/// `SDL_BUTTON(X) = 1 << (X-1)`, `SDL_BUTTON_LEFT=1`, `_MIDDLE=2`,
/// `_RIGHT=3`), confirmed against the pinned SDL2 headers.
mod sdl_button {
    pub const LMASK: u32 = 1 << 0;
    pub const MMASK: u32 = 1 << 1;
    pub const RMASK: u32 = 1 << 2;
}

/// `MOUSE_Left`/`Right`/`Middle` (`Engine/Inc/UnCamera.h:194-196`).
mod mouse {
    pub const LEFT: u32 = 0x0001;
    pub const RIGHT: u32 = 0x0002;
    pub const MIDDLE: u32 = 0x0004;
}

/// Replaces `ButtonsFromSdlState` (`SDLViewport.cpp:46-53`). Pure bit
/// translation, no FFI dependency.
pub fn buttons_from_sdl_state(state: u32) -> u32 {
    let mut buttons = 0;
    if state & sdl_button::LMASK != 0 {
        buttons |= mouse::LEFT;
    }
    if state & sdl_button::RMASK != 0 {
        buttons |= mouse::RIGHT;
    }
    if state & sdl_button::MMASK != 0 {
        buttons |= mouse::MIDDLE;
    }
    buttons
}

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::c_int;

    /// Opaque; every caller (including the real C++) only ever holds
    /// a pointer to this, never reads its layout.
    #[repr(C)]
    pub struct SdlWindow {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_GetMouseState(x: *mut c_int, y: *mut c_int) -> u32;
        pub fn SDL_SetWindowGrab(window: *mut SdlWindow, grabbed: i32);
        pub fn SDL_SetRelativeMouseMode(enabled: i32) -> c_int;
        pub fn SDL_ShowCursor(toggle: i32) -> i32;
    }
}

#[cfg(feature = "sdl")]
pub use ffi::SdlWindow;

#[cfg(feature = "sdl")]
const SDL_DISABLE: i32 = 0;
#[cfg(feature = "sdl")]
const SDL_ENABLE: i32 = 1;
#[cfg(feature = "sdl")]
const SDL_FALSE: i32 = 0;
#[cfg(feature = "sdl")]
const SDL_TRUE: i32 = 1;

/// Replaces `USDLViewport::GetViewportButtonFlags` (`SDLViewport.cpp:544-548`).
#[cfg(feature = "sdl")]
pub fn get_viewport_button_flags() -> u32 {
    let mut x: std::os::raw::c_int = 0;
    let mut y: std::os::raw::c_int = 0;
    // SAFETY: SDL_GetMouseState is safe to call once SDL's video
    // subsystem is initialized (true by construction: C++ owns that
    // init and only calls into this crate afterward); `x`/`y` are
    // valid local out-params for the duration of the call.
    let state = unsafe { ffi::SDL_GetMouseState(&mut x, &mut y) };
    buttons_from_sdl_state(state)
}

/// Replaces `USDLViewport::GetMouseState` (`SDLViewport.cpp:550-556`).
/// Returns `(x, y, buttons)` in viewport pixels (scaled by
/// `mouse_scale_x`/`mouse_scale_y`, mirroring
/// `USDLViewport::MouseScaleX`/`Y`).
#[cfg(feature = "sdl")]
pub fn get_mouse_state(mouse_scale_x: f32, mouse_scale_y: f32) -> (f32, f32, u32) {
    let mut pixel_x: std::os::raw::c_int = 0;
    let mut pixel_y: std::os::raw::c_int = 0;
    // SAFETY: see get_viewport_button_flags.
    let state = unsafe { ffi::SDL_GetMouseState(&mut pixel_x, &mut pixel_y) };
    let buttons = buttons_from_sdl_state(state);
    (
        pixel_x as f32 * mouse_scale_x,
        pixel_y as f32 * mouse_scale_y,
        buttons,
    )
}

/// Replaces `USDLViewport::UpdateMouseGrabState` (`SDLViewport.cpp:558-579`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard around `SDL_SetWindowGrab` specifically -- the other two SDL
/// calls run unconditionally either way, same as the real C++).
/// Returns the new `(mouse_is_grabbed, windows_mouse_available)` pair
/// for the caller to store back into its own UObject fields (Rust
/// does not own them).
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call.
#[cfg(feature = "sdl")]
pub unsafe fn update_mouse_grab_state(
    sdl_window: *mut SdlWindow,
    capture: bool,
    mouse_is_grabbed: bool,
) -> (bool, bool) {
    let windows_mouse_available = !capture;
    if capture == mouse_is_grabbed {
        // No transition; cursor visibility must stay untouched.
        return (mouse_is_grabbed, windows_mouse_available);
    }
    let sdl_bool = if capture { SDL_TRUE } else { SDL_FALSE };
    // SAFETY: `sdl_window` validity is this function's own contract
    // (see above); the other two calls take no pointer at all.
    unsafe {
        if !sdl_window.is_null() {
            ffi::SDL_SetWindowGrab(sdl_window, sdl_bool);
        }
        ffi::SDL_SetRelativeMouseMode(sdl_bool);
        // Releasing shows the cursor; grabbing hides it again.
        ffi::SDL_ShowCursor(if capture { SDL_DISABLE } else { SDL_ENABLE });
    }
    (capture, windows_mouse_available)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the real `ButtonsFromSdlState`'s three `if` checks.
    #[test]
    fn buttons_from_sdl_state_translates_each_mask_independently() {
        assert_eq!(buttons_from_sdl_state(0), 0);
        assert_eq!(buttons_from_sdl_state(sdl_button::LMASK), mouse::LEFT);
        assert_eq!(buttons_from_sdl_state(sdl_button::RMASK), mouse::RIGHT);
        assert_eq!(buttons_from_sdl_state(sdl_button::MMASK), mouse::MIDDLE);
        assert_eq!(
            buttons_from_sdl_state(sdl_button::LMASK | sdl_button::RMASK | sdl_button::MMASK),
            mouse::LEFT | mouse::RIGHT | mouse::MIDDLE
        );
        // A bit the real C++ doesn't check (e.g. SDL_BUTTON_X1MASK) is
        // ignored, not reflected into the result.
        assert_eq!(buttons_from_sdl_state(1 << 3), 0);
    }
}
