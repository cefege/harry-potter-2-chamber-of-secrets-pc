//! Replaces `USDLViewport`'s SDL window create/destroy/resize/fullscreen/
//! size/flags/GL-attribute leaves (`SDLViewport.cpp:222`, `:269-277`,
//! `:294-297`, `:330`, `:184-191`). `Hp2PlatformViewportLeaf_WindowStore`
//! (a pure field write, no SDL call) is the only window leaf the C++
//! shell still performs; it publishes the pointer this module's
//! `create_window` returns.
//!
//! `SdlWindow` crosses as an opaque pointer, same design as
//! `window_chrome`/`mouse_state`: the real C++ owns the field it is
//! stored in; this module only creates, destroys, or queries it on
//! request.

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::{c_char, c_int};

    /// Opaque; every caller (including the real C++) only ever holds a
    /// pointer to this, never reads its layout. Distinct from
    /// `window_chrome::SdlWindow`/`mouse_state::SdlWindow` (separate
    /// zero-sized types with the same name) only in that they live in
    /// different modules -- all represent the identical real
    /// `SDL_Window*`.
    #[repr(C)]
    pub struct SdlWindow {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_CreateWindow(
            title: *const c_char,
            x: c_int,
            y: c_int,
            w: c_int,
            h: c_int,
            flags: u32,
        ) -> *mut SdlWindow;
        pub fn SDL_DestroyWindow(window: *mut SdlWindow);
        pub fn SDL_SetWindowSize(window: *mut SdlWindow, w: c_int, h: c_int);
        pub fn SDL_SetWindowFullscreen(window: *mut SdlWindow, flags: u32) -> c_int;
        pub fn SDL_GetWindowSize(window: *mut SdlWindow, w: *mut c_int, h: *mut c_int);
        pub fn SDL_GL_GetDrawableSize(window: *mut SdlWindow, w: *mut c_int, h: *mut c_int);
        pub fn SDL_GetWindowFlags(window: *mut SdlWindow) -> u32;
        pub fn SDL_GL_SetAttribute(attr: c_int, value: c_int) -> c_int;
        pub fn SDL_GetNumVideoDisplays() -> c_int;
        pub fn SDL_SetWindowPosition(window: *mut SdlWindow, x: c_int, y: c_int);
    }
}
/// Returns the flags of a launcher window.
///
/// # Safety
/// `window`, if non-null, must remain a live SDL window for this call.
/// The caller must obey SDL's video-thread requirements.
#[cfg(feature = "sdl")]
pub unsafe fn launcher_window_flags(window: *mut SdlWindow) -> u32 {
    // SAFETY: the caller supplies a live SDL window.
    unsafe { ffi::SDL_GetWindowFlags(window) }
}

#[cfg(feature = "sdl")]
pub use ffi::SdlWindow;

/// `SDL_WINDOWPOS_CENTERED` (`SDL_video.h`: `SDL_WINDOWPOS_CENTERED_MASK|0`,
/// `0x2FFF0000`).
#[cfg(feature = "sdl")]
const SDL_WINDOWPOS_CENTERED: std::os::raw::c_int = 0x2FFF_0000u32 as std::os::raw::c_int;
/// `SDL_WINDOW_FULLSCREEN_DESKTOP` (`SDL_video.h`:
/// `SDL_WINDOW_FULLSCREEN | 0x1000`).
#[cfg(feature = "sdl")]
const SDL_WINDOW_FULLSCREEN_DESKTOP: u32 = 0x0000_1001;

/// Replaces `Hp2PlatformViewportLeaf_WindowCreate`
/// (`SDLViewport.cpp:294-297`): `SDL_CreateWindow("Harry Potter 2",
/// SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, width, height, flags)`.
/// The caller stores the (possibly null, on failure) result itself via
/// `WindowStore`.
#[cfg(feature = "sdl")]
pub fn create_window(width: i32, height: i32, flags: u32) -> *mut SdlWindow {
    create_window_on_display(width, height, flags, 0)
}

/// `SDL_WINDOWPOS_CENTERED_DISPLAY(display)` (`SDL_video.h`:
/// `SDL_WINDOWPOS_CENTERED_MASK | display`). Negative indices fall back to display 0.
pub const fn centered_on_display(display: i32) -> i32 {
    (0x2FFF_0000u32 | if display < 0 { 0 } else { display as u32 & 0xFFFF }) as i32
}

/// `SDL_GetNumVideoDisplays()`; at least 1 so a failed query keeps display 0 usable.
#[cfg(feature = "sdl")]
pub fn display_count() -> i32 {
    // SAFETY: argument-free SDL query; SDL video must be initialized for a real answer.
    unsafe { ffi::SDL_GetNumVideoDisplays() }.max(1)
}

/// Clamps a persisted display index to the displays that exist now, so an unplugged
/// monitor falls back to the nearest remaining one instead of failing.
pub const fn clamp_display(display: i32, count: i32) -> i32 {
    let last = if count < 1 { 0 } else { count - 1 };
    if display < 0 { 0 } else if display > last { last } else { display }
}

/// [`create_window`] centered on `display` (clamped to the connected displays).
#[cfg(feature = "sdl")]
pub fn create_window_on_display(width: i32, height: i32, flags: u32, display: i32) -> *mut SdlWindow {
    const TITLE: &std::ffi::CStr = c"Harry Potter 2";
    let at = centered_on_display(clamp_display(display, display_count()));
    // SAFETY: `TITLE` is a valid NUL-terminated C string for the
    // duration of this call; SDL copies it internally.
    unsafe { ffi::SDL_CreateWindow(TITLE.as_ptr(), at, at, width, height, flags) }
}

/// Centers a windowed (not fullscreen) window on `display`; SDL ignores moves of fullscreen
/// windows, so callers leave fullscreen first.
///
/// # Safety
/// `window` must be a live SDL window on the video thread.
#[cfg(feature = "sdl")]
pub unsafe fn center_window_on_display(window: *mut SdlWindow, display: i32) {
    let at = centered_on_display(clamp_display(display, display_count()));
    // SAFETY: the caller supplies a live SDL window.
    unsafe { ffi::SDL_SetWindowPosition(window, at, at) }
}

/// Creates the standalone egui launcher window on the shared SDL video subsystem.
#[cfg(feature = "sdl")]
pub fn create_launcher_window(title: &std::ffi::CStr, width: i32, height: i32) -> *mut SdlWindow {
    const WINDOW_SHOWN: u32 = 0x0000_0004;
    const WINDOW_RESIZABLE: u32 = crate::viewport::SDL_WINDOW_RESIZABLE;
    const WINDOW_ALLOW_HIGHDPI: u32 = 0x0000_2000;
    const WINDOW_METAL: u32 = 0x2000_0000;
    unsafe {
        ffi::SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            width,
            height,
            WINDOW_SHOWN | WINDOW_RESIZABLE | WINDOW_ALLOW_HIGHDPI | WINDOW_METAL,
        )
    }
}

/// Returns logical window size and physical drawable size for egui's point scale.
///
/// # Safety
/// `window`, if non-null, must remain a live SDL window for both calls.
/// The caller must obey SDL's video-thread requirements. For a null window,
/// the initialized sizes remain zero; SDL can set its error state.
#[cfg(feature = "sdl")]
pub unsafe fn launcher_window_sizes(window: *mut SdlWindow) -> ((i32, i32), (i32, i32)) {
    let (mut width, mut height, mut drawable_width, mut drawable_height) = (0, 0, 0, 0);
    unsafe {
        ffi::SDL_GetWindowSize(window, &mut width, &mut height);
        ffi::SDL_GL_GetDrawableSize(window, &mut drawable_width, &mut drawable_height);
    }
    ((width, height), (drawable_width, drawable_height))
}

/// Replaces `Hp2PlatformViewportLeaf_WindowDestroy` (`SDLViewport.cpp:222`).
///
/// # Safety
/// `window` must be a valid, non-null `SDL_Window*` for the duration of
/// this call; the caller must not use it again afterward.
#[cfg(feature = "sdl")]
pub unsafe fn destroy_window(window: *mut SdlWindow) {
    // SAFETY: validity is this function's own contract (see above).
    unsafe { ffi::SDL_DestroyWindow(window) }
}

/// Replaces `Hp2PlatformViewportLeaf_WindowSetSize` (`SDLViewport.cpp:269`).
/// `window` may be null; SDL's own `CHECK_WINDOW_MAGIC` guard makes that a
/// harmless no-op (an ignored error), matching every call site here, which
/// already null-checks before calling.
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn set_window_size(window: *mut SdlWindow, width: i32, height: i32) {
    // SAFETY: validity is this function's own contract (see above).
    unsafe { ffi::SDL_SetWindowSize(window, width, height) }
}

/// Replaces `Hp2PlatformViewportLeaf_WindowSetFullscreen`
/// (`SDLViewport.cpp:270-272`): `SDL_SetWindowFullscreen(window,
/// fullscreen ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0)`. Ignores the SDL
/// result, matching the original. `window` may be null; SDL's own
/// `CHECK_WINDOW_MAGIC` guard makes that a harmless no-op.
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn set_window_fullscreen(window: *mut SdlWindow, fullscreen: bool) {
    let flags = if fullscreen { SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 };
    // SAFETY: validity is this function's own contract (see above).
    unsafe {
        ffi::SDL_SetWindowFullscreen(window, flags);
    }
}

/// Replaces `Hp2PlatformViewportLeaf_WindowStartFullscreen`
/// (`SDLViewport.cpp:273`): `SDL_SetWindowFullscreen(window,
/// SDL_WINDOW_FULLSCREEN_DESKTOP)`. Ignores the SDL result, matching the
/// original. `window` may be null (the original called this
/// unconditionally in `OpenWindow`, relying on the same `CHECK_WINDOW_
/// MAGIC` no-op SDL itself provides).
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn start_fullscreen(window: *mut SdlWindow) {
    // SAFETY: validity is this function's own contract (see above).
    unsafe {
        ffi::SDL_SetWindowFullscreen(window, SDL_WINDOW_FULLSCREEN_DESKTOP);
    }
}

/// Replaces `Hp2PlatformViewportLeaf_WindowSize` (`SDLViewport.cpp:274-277`).
/// Returns `(x, y)`, initialized to `(0, 0)` as the original was before
/// the call. `window` may be null; SDL's own `CHECK_WINDOW_MAGIC` guard
/// makes that a harmless no-op (an ignored error), matching the only call
/// site here, which already null-checks before calling.
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn get_window_size(window: *mut SdlWindow) -> (i32, i32) {
    let mut x: std::os::raw::c_int = 0;
    let mut y: std::os::raw::c_int = 0;
    // SAFETY: validity is this function's own contract (see above);
    // `x`/`y` are valid local out-params for the duration of the call.
    unsafe { ffi::SDL_GetWindowSize(window, &mut x, &mut y) };
    (x, y)
}

/// Replaces `Hp2PlatformViewportLeaf_WindowFlags` (`SDLViewport.cpp:330`).
/// `window` may be null; SDL's own `CHECK_WINDOW_MAGIC` guard makes that
/// a harmless no-op (an ignored error), matching every call site here,
/// which already null-checks before calling.
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn get_window_flags(window: *mut SdlWindow) -> u32 {
    // SAFETY: validity is this function's own contract (see above).
    unsafe { ffi::SDL_GetWindowFlags(window) }
}

/// `SDL_GLattr` values (`SDL_video.h`), only the entries
/// `Hp2PlatformViewportLeaf_GlAttribute`'s table used
/// (`SDLViewport.cpp:184-191`, now deleted).
#[cfg(feature = "sdl")]
pub const SDL_GL_RED_SIZE: std::os::raw::c_int = 0;
#[cfg(feature = "sdl")]
pub const SDL_GL_GREEN_SIZE: std::os::raw::c_int = 1;
#[cfg(feature = "sdl")]
pub const SDL_GL_BLUE_SIZE: std::os::raw::c_int = 2;
#[cfg(feature = "sdl")]
pub const SDL_GL_DOUBLEBUFFER: std::os::raw::c_int = 5;
#[cfg(feature = "sdl")]
pub const SDL_GL_DEPTH_SIZE: std::os::raw::c_int = 6;

/// Replaces `Hp2PlatformViewportLeaf_GlAttribute` (`SDLViewport.cpp:
/// 184-191`): `SDL_GL_SetAttribute(attr, value)`. Ignores the SDL result,
/// matching the original (which also never checked it).
#[cfg(feature = "sdl")]
pub fn set_gl_attribute(attr: std::os::raw::c_int, value: i32) {
    // SAFETY: SDL_GL_SetAttribute is safe to call with any attr/value pair;
    // an invalid combination is reported through SDL_GetError, which the
    // original leaf ignored and so do we.
    unsafe {
        ffi::SDL_GL_SetAttribute(attr, value);
    }
}

#[cfg(test)]
mod display_tests {
    use super::{centered_on_display, clamp_display};

    #[test]
    fn centered_display_matches_sdl_windowpos_macro() {
        assert_eq!(centered_on_display(0) as u32, 0x2FFF_0000);
        assert_eq!(centered_on_display(2) as u32, 0x2FFF_0002);
        assert_eq!(centered_on_display(-3) as u32, 0x2FFF_0000);
    }

    #[test]
    fn saved_display_falls_back_to_a_connected_one() {
        assert_eq!(clamp_display(0, 1), 0);
        assert_eq!(clamp_display(1, 2), 1);
        assert_eq!(clamp_display(5, 2), 1);
        assert_eq!(clamp_display(-1, 3), 0);
        assert_eq!(clamp_display(4, 0), 0);
    }
}
