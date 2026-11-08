//! Replaces the main loop's `Hp2LauncherMainLoopLeaf_KeyboardFocusAbsent`
//! leaf (`SDLLaunch.cpp`): `SDL_GetKeyboardFocus() == NULL`. Plain SDL2 C
//! API querying process-wide (not per-window) keyboard focus, no engine
//! dependency.

#[cfg(feature = "sdl")]
mod ffi {
    use std::ffi::c_void;

    unsafe extern "C" {
        pub fn SDL_GetKeyboardFocus() -> *mut c_void;
    }
}

/// Replaces `SDL_GetKeyboardFocus() == NULL`.
#[cfg(feature = "sdl")]
pub fn keyboard_focus_absent() -> bool {
    unsafe { ffi::SDL_GetKeyboardFocus().is_null() }
}
