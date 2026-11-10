//! Replaces `FLauncherRound::CollectDisplayModes`'s three pure-SDL2 leaves
//! (`SDLLaunch.cpp`, formerly lines 295-349's `initialize_video`/
//! `platform_count`/`platform_dimensions` lambdas): the `SDL_SetHint`/
//! `SDL_WasInit`/`SDL_InitSubSystem` video-subsystem bring-up, and the
//! `SDL_GetNumDisplayModes`/`SDL_GetDisplayMode` enumeration. No engine or
//! reflection dependency -- plain SDL2 C API, so `hp2-ffi`'s
//! `launcher_display_modes` leaf calls these directly instead of
//! round-tripping through a C++ leaf table. The two remaining leaves
//! (`request_count`/`request_dimensions`/`append_mode`) still cross into
//! C++ because they read/write `FLauncherRound::Request.displayModes`, a
//! real C++-owned `std::vector`.

#[cfg(feature = "sdl")]
mod ffi {
    use std::ffi::c_void;
    use std::os::raw::{c_char, c_int};

    /// `SDL_DisplayMode` (`SDL_video.h`): plain data, not opaque, but only
    /// ever read back after `SDL_GetDisplayMode` fills it in.
    #[repr(C)]
    pub struct SdlDisplayMode {
        pub format: u32,
        pub w: c_int,
        pub h: c_int,
        pub refresh_rate: c_int,
        pub driverdata: *mut c_void,
    }

    unsafe extern "C" {
        pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> c_int;
        pub fn SDL_WasInit(flags: u32) -> u32;
        pub fn SDL_InitSubSystem(flags: u32) -> c_int;
        pub fn SDL_QuitSubSystem(flags: u32);
        pub fn SDL_GetNumDisplayModes(display_index: c_int) -> c_int;
        pub fn SDL_GetDisplayMode(
            display_index: c_int,
            mode_index: c_int,
            mode: *mut SdlDisplayMode,
        ) -> c_int;
    }
}

/// `SDL_INIT_VIDEO` (`SDL.h`).
#[cfg(feature = "sdl")]
const SDL_INIT_VIDEO: u32 = 0x0000_0020;

/// `SDL_HINT_VIDEODRIVER` (`SDL_hints.h`).
#[cfg(feature = "sdl")]
const SDL_HINT_VIDEODRIVER: &[u8] = b"SDL_VIDEODRIVER\0";

/// Selects the native video backend before SDL video initialization.
///
/// On macOS, explicitly uses foreground application behavior for both bundled
/// and unbundled game launches. SDL otherwise defaults unbundled executables to
/// background-app behavior on macOS 14+, preventing their window from activating.
/// Explicit environment hints retain SDL's normal priority over these defaults.
#[cfg(feature = "sdl")]
pub fn set_video_driver_hint() {
    #[cfg(target_os = "macos")]
    const DRIVER: &[u8] = b"cocoa\0";
    #[cfg(target_os = "linux")]
    const DRIVER: &[u8] = b"wayland,x11\0";
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    unsafe {
        ffi::SDL_SetHint(SDL_HINT_VIDEODRIVER.as_ptr().cast(), DRIVER.as_ptr().cast());
        #[cfg(target_os = "macos")]
        ffi::SDL_SetHint(c"SDL_MAC_BACKGROUND_APP".as_ptr(), c"0".as_ptr());
    }
}

/// Replaces `SDLLaunch.cpp:309-311`: `if (!(SDL_WasInit(SDL_INIT_VIDEO) &
/// SDL_INIT_VIDEO)) { if (SDL_InitSubSystem(SDL_INIT_VIDEO) == 0)
/// Round.OwnsVideo = true; }`. Returns whether this call newly
/// initialized the subsystem, so the caller can still gate its own
/// `SDL_QuitSubSystem` cleanup on it exactly as `Round.OwnsVideo` did.
#[cfg(feature = "sdl")]
pub fn ensure_video_initialized() -> bool {
    unsafe {
        if ffi::SDL_WasInit(SDL_INIT_VIDEO) & SDL_INIT_VIDEO != 0 {
            return false;
        }
        if !crate::sdl_runtime::configure_fullscreen_hint() { return false; }
        ffi::SDL_InitSubSystem(SDL_INIT_VIDEO) == 0
    }
}

/// Replaces `SDLLaunch.cpp:317`: `SDL_GetNumDisplayModes(0)`.
#[cfg(feature = "sdl")]
pub fn mode_count() -> i32 {
    unsafe { ffi::SDL_GetNumDisplayModes(0) }
}

/// Replaces `SDLLaunch.cpp:319-326`: `SDL_GetDisplayMode(0, index, &mode)
/// == 0`, returning `(w, h)` on success and `None` otherwise (the
/// original's `*Found = false` with `*Width`/`*Height` left untouched).
#[cfg(feature = "sdl")]
pub fn mode_dimensions(index: i32) -> Option<(i32, i32)> {
    let mut mode = ffi::SdlDisplayMode {
        format: 0,
        w: 0,
        h: 0,
        refresh_rate: 0,
        driverdata: std::ptr::null_mut(),
    };
    unsafe {
        if ffi::SDL_GetDisplayMode(0, index, &mut mode) == 0 {
            Some((mode.w, mode.h))
        } else {
            None
        }
    }
}

/// Replaces `SDL_QuitSubSystem(SDL_INIT_VIDEO)` (`SDLLaunch.cpp`,
/// `Hp2LauncherRoundLeaf_QuitVideo`).
#[cfg(feature = "sdl")]
pub fn quit_video() {
    unsafe { ffi::SDL_QuitSubSystem(SDL_INIT_VIDEO) }
}
