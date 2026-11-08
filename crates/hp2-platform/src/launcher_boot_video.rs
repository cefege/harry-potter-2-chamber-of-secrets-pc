//! Replaces the boot sequence's `SDL_Init(SDL_INIT_VIDEO |
//! SDL_INIT_JOYSTICK)` leaf (`SDLLaunch.cpp`,
//! `Hp2LauncherBootLeaf_SdlInit`) and `FinishProcess`'s matching
//! `SDL_Quit()` teardown call. The adjacent video-driver hint
//! (`Hp2LauncherBootLeaf_SetVideoHint`) reuses
//! [`crate::launcher_display_modes::set_video_driver_hint`] directly: with
//! the retired `HP2_WINDOWS` branch gone, both leaves hint identically
//! (`cocoa` on macOS, `wayland,x11` on Linux, no hint otherwise). The
//! failure log (`Hp2LauncherBootLeaf_LogSdlInitFailure`, a `debugf` call)
//! stays in C++: it is a `GLog`-routed line, and `HP2RustLogSink` prefixes
//! every Rust-originated log line with `"HP2Rust: "`, which would break
//! the byte-identical log text the oracle compares.

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::c_int;

    unsafe extern "C" {
        pub fn SDL_Init(flags: u32) -> c_int;
        pub fn SDL_Quit();
    }
}

/// `SDL_INIT_VIDEO | SDL_INIT_JOYSTICK` (`SDL.h`).
#[cfg(feature = "sdl")]
const SDL_INIT_VIDEO_JOYSTICK: u32 = 0x0000_0020 | 0x0000_0200;

/// Replaces `SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK)`
/// (`Hp2LauncherBootLeaf_SdlInit`). Returns the raw SDL status (0 on
/// success) so the caller can still decide whether to log the failure.
#[cfg(feature = "sdl")]
pub fn init_video_and_joystick() -> i32 {
    unsafe { ffi::SDL_Init(SDL_INIT_VIDEO_JOYSTICK) }
}

/// Replaces `FinishProcess`'s `SDL_Quit()` call (`SDLLaunch.cpp`).
#[cfg(feature = "sdl")]
pub fn quit() {
    unsafe { ffi::SDL_Quit() }
}
