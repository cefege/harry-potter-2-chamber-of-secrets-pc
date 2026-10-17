//! Replaces `USDLViewport::UpdateWindowFrame`/`SetTitleBar`
//! (`SDLViewport.cpp:233-239`, `:749-759`).
//!
//! `SdlWindow` crosses as an opaque pointer, same design as
//! `mouse_state`'s `update_mouse_grab_state`: the real C++ already
//! owns it and passes it in; this module never creates, destroys, or
//! stores it.

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::c_char;

    /// Opaque; every caller (including the real C++) only ever holds
    /// a pointer to this, never reads its layout. Distinct from
    /// `mouse_state::SdlWindow` (a separate zero-sized type with the
    /// same name) only in that they live in different modules -- both
    /// represent the identical real `SDL_Window*`; callers cast the
    /// same opaque pointer to whichever this module's functions want.
    #[repr(C)]
    pub struct SdlWindow {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_SetWindowBordered(window: *mut SdlWindow, bordered: i32);
        pub fn SDL_SetWindowTitle(window: *mut SdlWindow, title: *const c_char);
    }
}

#[cfg(feature = "sdl")]
pub use ffi::SdlWindow;

/// `SDL_SetWindowBordered`'s `bordered` parameter: the inverse of
/// `borderless` (`SDL_SetWindowBordered(window, SDL_TRUE)` means
/// "show a border", so a borderless window passes `SDL_FALSE`).
/// `SDL_TRUE`/`SDL_FALSE` are `1`/`0` per `<SDL_stdinc.h>`'s
/// `SDL_bool` -- pure logic, no FFI dependency, independently
/// unit-testable without the `sdl` feature (unlike the SDL call
/// itself, which cannot be verified by reading
/// `SDL_GetWindowFlags`/`SDL_WINDOW_BORDERLESS` back under the
/// dummy video driver -- confirmed empirically: that flag does not
/// change in response to `SDL_SetWindowBordered` under that driver
/// at all, driver limitation, not a port bug).
pub fn sdl_bordered_flag(borderless: bool) -> i32 {
    if borderless {
        0 // SDL_FALSE
    } else {
        1 // SDL_TRUE
    }
}

/// Replaces `USDLViewport::UpdateWindowFrame` (`SDLViewport.cpp:233-239`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard, a no-op in that case).
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call.
#[cfg(feature = "sdl")]
pub unsafe fn update_window_frame(sdl_window: *mut SdlWindow, borderless: bool) {
    if sdl_window.is_null() {
        return;
    }
    // SAFETY: non-null checked above; validity is this function's own
    // contract (see above).
    unsafe {
        ffi::SDL_SetWindowBordered(sdl_window, sdl_bordered_flag(borderless));
    }
}

/// Replaces `USDLViewport::SetTitleBar` (`SDLViewport.cpp:749-759`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard, a no-op in that case). `title` is UTF-8; embedded NULs
/// truncate the title at that byte, matching what a NUL-terminated
/// C string would do if one were built from the same bytes (the real
/// C++'s `TCHAR*` -> UTF-8 conversion has no such input today, so
/// this is unreachable in practice, not a behavior divergence).
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call.
#[cfg(feature = "sdl")]
pub unsafe fn set_title_bar(sdl_window: *mut SdlWindow, title: &str) {
    if sdl_window.is_null() {
        return;
    }
    // CString::new only fails on an embedded NUL; truncate there
    // rather than drop the whole title, matching "best-effort display
    // text" rather than treating it as a hard error.
    let c_title = std::ffi::CString::new(title)
        .unwrap_or_else(|e| std::ffi::CString::new(&title.as_bytes()[..e.nul_position()]).unwrap());
    // SAFETY: non-null checked above; `c_title` is a valid
    // NUL-terminated C string for the duration of this call;
    // `SDL_SetWindowTitle` copies it internally.
    unsafe {
        ffi::SDL_SetWindowTitle(sdl_window, c_title.as_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the `borderless` -> `SDL_TRUE`/`SDL_FALSE` inversion
    /// directly: this is the only part of `update_window_frame` a
    /// dummy-driver ctest cannot verify by reading SDL state back
    /// (see `sdl_bordered_flag`'s own doc comment for why), so it
    /// gets its own unconditional, no-`sdl`-feature-needed check.
    #[test]
    fn sdl_bordered_flag_inverts_borderless() {
        assert_eq!(
            sdl_bordered_flag(true),
            0,
            "borderless=true must request SDL_FALSE (no border)"
        );
        assert_eq!(
            sdl_bordered_flag(false),
            1,
            "borderless=false must request SDL_TRUE (bordered)"
        );
    }
}
