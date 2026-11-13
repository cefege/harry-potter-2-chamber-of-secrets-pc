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
//!
//! Deterministic verification keeps the logical grab result but skips every
//! OS capture call, so the user's cursor is never trapped and relative mouse
//! sampling is never armed; see [`update_mouse_grab_state_with`].

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
        pub fn SDL_PumpEvents();
        pub fn SDL_GetMouseState(x: *mut c_int, y: *mut c_int) -> u32;
        pub fn SDL_GetRelativeMouseState(x: *mut c_int, y: *mut c_int) -> u32;
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
///
/// Deterministic verification reports no held buttons instead of reading the
/// physical device, so a caller polling button state cannot observe hardware.
#[cfg(feature = "sdl")]
pub fn get_viewport_button_flags() -> u32 {
    if crate::input_pump::deterministic_verification() {
        return 0;
    }
    let mut x: std::os::raw::c_int = 0;
    let mut y: std::os::raw::c_int = 0;
    // SAFETY: SDL_GetMouseState is safe to call once SDL's video
    // subsystem is initialized (true by construction: C++ owns that
    // init and only calls into this crate afterward); `x`/`y` are
    // valid local out-params for the duration of the call.
    let state = unsafe { ffi::SDL_GetMouseState(&mut x, &mut y) };
    buttons_from_sdl_state(state)
}

/// Replaces `Hp2PlatformViewportLeaf_RelativeMouse` (`SDLViewport.cpp:
/// 317-320`). Returns `(x, y)`, initialized to `(0, 0)` as the original
/// was before the call.
#[cfg(feature = "sdl")]
pub fn get_relative_mouse_state() -> (i32, i32) {
    let mut x: std::os::raw::c_int = 0;
    let mut y: std::os::raw::c_int = 0;
    // SAFETY: see get_viewport_button_flags.
    unsafe { ffi::SDL_GetRelativeMouseState(&mut x, &mut y) };
    (x, y)
}

/// Replaces `USDLViewport::GetMouseState` (`SDLViewport.cpp:550-556`).
/// Returns `(x, y, buttons)` in viewport pixels (scaled by
/// `mouse_scale_x`/`mouse_scale_y`, mirroring
/// `USDLViewport::MouseScaleX`/`Y`).
///
/// Deterministic verification reports the fixed neutral position with no
/// buttons rather than the physical pointer, matching the input pump's
/// isolation of hardware-derived state.
#[cfg(feature = "sdl")]
pub fn get_mouse_state(mouse_scale_x: f32, mouse_scale_y: f32) -> (f32, f32, u32) {
    if crate::input_pump::deterministic_verification() {
        return (0.0, 0.0, 0);
    }
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

// Keep the transition policy testable without a linked SDL library. Pumping
// before capture incorporates pending menu-era native motion; SDL's relative
// mode change flushes motion events but does not clear its delta accumulator.
#[cfg(any(test, feature = "sdl"))]
trait CaptureBackend {
    fn pump_events(&mut self);
    fn set_grab(&mut self, capture: bool);
    fn set_relative(&mut self, capture: bool);
    fn discard_relative(&mut self);
    fn set_cursor(&mut self, capture: bool);
}

#[cfg(any(test, feature = "sdl"))]
fn capture_transition<B: CaptureBackend>(
    backend: &mut B, capture: bool, grabbed: bool, verification: bool, replay: bool,
) -> (bool, bool) {
    if verification || capture == grabbed {
        return (if verification { capture } else { grabbed }, !capture);
    }
    if capture && !replay {
        backend.pump_events();
    }
    backend.set_grab(capture);
    backend.set_relative(capture);
    if capture && !replay {
        backend.discard_relative();
    }
    backend.set_cursor(capture);
    (capture, !capture)
}

/// Replaces `USDLViewport::UpdateMouseGrabState` (`SDLViewport.cpp:558-579`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard around `SDL_SetWindowGrab` specifically -- the other two SDL
/// calls run unconditionally either way, same as the real C++).
/// Returns the new `(mouse_is_grabbed, windows_mouse_available)` pair
/// for the caller to store back into its own UObject fields (Rust
/// does not own them).
///
/// Under deterministic verification this keeps the logical result but
/// performs no SDL capture, relative-mode, or cursor calls; see
/// [`update_mouse_grab_state_with`].
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
    // SAFETY: the arguments carry this function's own contract.
    unsafe {
        update_mouse_grab_state_with(
            sdl_window,
            capture,
            mouse_is_grabbed,
            crate::input_pump::deterministic_verification(),
        )
    }
}

/// [`update_mouse_grab_state`] with the verification policy supplied
/// explicitly. Deterministic verification returns the same logical
/// `(grabbed, windows_mouse_available)` pair the capture request implies, but
/// never calls `SDL_SetWindowGrab`, `SDL_SetRelativeMouseMode`, or
/// `SDL_ShowCursor`: the user's cursor is not trapped, and the relative-mouse
/// hardware producer is never armed.
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn update_mouse_grab_state_with(
    sdl_window: *mut SdlWindow,
    capture: bool,
    mouse_is_grabbed: bool,
    verification: bool,
) -> (bool, bool) {
    struct SdlCapture(*mut SdlWindow);
    impl CaptureBackend for SdlCapture {
        fn pump_events(&mut self) { unsafe { ffi::SDL_PumpEvents() } }
        fn set_grab(&mut self, capture: bool) {
            if !self.0.is_null() {
                unsafe { ffi::SDL_SetWindowGrab(self.0, if capture { SDL_TRUE } else { SDL_FALSE }) };
            }
        }
        fn set_relative(&mut self, capture: bool) {
            unsafe { ffi::SDL_SetRelativeMouseMode(if capture { SDL_TRUE } else { SDL_FALSE }) };
        }
        fn discard_relative(&mut self) { let _ = get_relative_mouse_state(); }
        fn set_cursor(&mut self, capture: bool) {
            unsafe { ffi::SDL_ShowCursor(if capture { SDL_DISABLE } else { SDL_ENABLE }) };
        }
    }
    // Background runs never take the user's cursor.
    let verification = verification || crate::background_mode();
    // Do not initialize replay mode or sample hardware in verification mode.
    let replay = !verification && crate::input_pump::input_replaying();
    capture_transition(&mut SdlCapture(sdl_window), capture, mouse_is_grabbed, verification, replay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockCapture {
        calls: Vec<&'static str>,
        accumulated: (i32, i32),
        pending: (i32, i32),
    }
    impl CaptureBackend for MockCapture {
        fn pump_events(&mut self) {
            self.calls.push("pump");
            self.accumulated.0 += self.pending.0;
            self.accumulated.1 += self.pending.1;
            self.pending = (0, 0);
        }
        fn set_grab(&mut self, capture: bool) { self.calls.push(if capture { "grab" } else { "ungrab" }); }
        fn set_relative(&mut self, capture: bool) { self.calls.push(if capture { "relative" } else { "absolute" }); }
        fn discard_relative(&mut self) { self.calls.push("discard"); self.accumulated = (0, 0); }
        fn set_cursor(&mut self, capture: bool) { self.calls.push(if capture { "hide" } else { "show" }); }
    }

    #[test]
    fn mouse_state_recapture_discards_menu_and_pending_motion_only() {
        let mut host = MockCapture { accumulated: (17, -4), pending: (9, 2), ..Default::default() };
        assert_eq!(capture_transition(&mut host, true, false, false, false), (true, false));
        assert_eq!(host.calls, ["pump", "grab", "relative", "discard", "hide"]);
        assert_eq!(host.accumulated, (0, 0));
        // The next real movement is retained, including across a repeated request.
        host.accumulated = (3, -2);
        host.calls.clear();
        assert_eq!(capture_transition(&mut host, true, true, false, false), (true, false));
        assert_eq!(host.accumulated, (3, -2));
        assert!(host.calls.is_empty());
    }

    #[test]
    fn mouse_state_release_replay_and_verification_do_not_sample() {
        let mut host = MockCapture { accumulated: (7, 8), ..Default::default() };
        assert_eq!(capture_transition(&mut host, false, true, false, false), (false, true));
        assert_eq!(host.calls, ["ungrab", "absolute", "show"]);
        host.calls.clear();
        assert_eq!(capture_transition(&mut host, true, false, false, true), (true, false));
        assert_eq!(host.calls, ["grab", "relative", "hide"]);
        assert_eq!(host.accumulated, (7, 8));
        host.calls.clear();
        assert_eq!(capture_transition(&mut host, true, false, true, false), (true, false));
        assert!(host.calls.is_empty());
    }

    // Requires the pinned static SDL2 library and its internal event injection
    // symbol. Run alone with SDL_VIDEODRIVER=dummy and one test thread.
    #[cfg(feature = "sdl-internal-tests")]
    #[test]
    fn mouse_state_sdl_recapture_clears_accumulator_not_later_motion() {
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32;
            fn SDL_Quit();
            fn SDL_SendMouseMotion(window: *mut SdlWindow, mouse_id: u32, relative: i32, x: i32, y: i32) -> i32;
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0); // SDL_INIT_VIDEO, dummy driver
            let _ = get_relative_mouse_state();
            SDL_SendMouseMotion(std::ptr::null_mut(), 0, 1, 17, -4);
            assert_eq!(get_relative_mouse_state(), (17, -4));
            SDL_SendMouseMotion(std::ptr::null_mut(), 0, 1, 17, -4);
            assert_eq!(update_mouse_grab_state_with(std::ptr::null_mut(), true, false, false), (true, false));
            assert_eq!(get_relative_mouse_state(), (0, 0));
            SDL_SendMouseMotion(std::ptr::null_mut(), 0, 1, 3, -2);
            assert_eq!(get_relative_mouse_state(), (3, -2));
            SDL_Quit();
        }
    }

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
