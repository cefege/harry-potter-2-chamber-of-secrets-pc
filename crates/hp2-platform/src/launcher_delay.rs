//! Replaces `Hp2LauncherPresentationLeaf_RunDelay`'s `SDL_Delay(8)` body
//! (`HP2Launcher.cpp:630`), the launcher UI's per-frame throttle.

#[cfg(feature = "sdl")]
mod ffi {
    unsafe extern "C" {
        pub fn SDL_Delay(ms: u32);
    }
}

/// Replaces `SDL_Delay(8)` (`HP2Launcher.cpp:630`).
#[cfg(feature = "sdl")]
pub fn delay_ms(ms: u32) {
    unsafe { ffi::SDL_Delay(ms) }
}
