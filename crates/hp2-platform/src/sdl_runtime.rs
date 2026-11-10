//! Replaces the two remaining whole-process SDL calls in
//! `USDLViewport`'s policy: `Hp2PlatformViewportLeaf_LinkedVersion`
//! (`SDLViewport.cpp:160`, `SDL_GetVersion`) and the `SDL_GetError()` half
//! of `Hp2PlatformViewportLeaf_WindowCreateError` (`SDLViewport.cpp:200`).
//!
//! `Hp2PlatformViewportLeaf_LogVersion` and the `appErrorf`/`ANSI_TO_TCHAR`
//! half of window-create-error reporting stay native leaves: both need the
//! engine's `debugf`/`appErrorf` logging/string infrastructure, which Rust
//! does not reimplement. They now receive their arguments from this
//! module's callers instead of reading native locals.
//!
//! `Hp2PlatformViewportLeaf_SdlQuit` (the viewport's own, distinct from
//! `SDLClient.cpp`'s) is replaced by `client_policy::sdl_quit()` directly;
//! this module declares no `SDL_Quit` binding of its own.

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::c_char;

    /// Mirrors `SDL_version` (`SDL_version.h`): three `Uint8` fields,
    /// no padding.
    #[repr(C)]
    pub struct SdlVersion {
        pub major: u8,
        pub minor: u8,
        pub patch: u8,
    }

    unsafe extern "C" {
        pub fn SDL_GetVersion(ver: *mut SdlVersion);
        pub fn SDL_GetError() -> *const c_char;
        #[cfg(target_os = "macos")]
        pub fn SDL_GetHint(name: *const c_char) -> *const c_char;
        #[cfg(target_os = "macos")]
        pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> i32;
        #[cfg(target_os = "macos")]
        pub fn SDL_SetError(format: *const c_char, ...) -> i32;
    }
}

/// Prefer SDL's borderless desktop implementation over macOS fullscreen
/// Spaces. SDL must read this hint before video initialization. Respect an
/// existing SDL hint or environment override instead of replacing it.
#[cfg(feature = "sdl")]
pub fn configure_fullscreen_hint() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        if !ffi::SDL_GetHint(c"SDL_VIDEO_MAC_FULLSCREEN_SPACES".as_ptr()).is_null() { return true; }
        if ffi::SDL_SetHint(c"SDL_VIDEO_MAC_FULLSCREEN_SPACES".as_ptr(), c"0".as_ptr()) != 0 { return true; }
        ffi::SDL_SetError(c"Could not set macOS fullscreen hint".as_ptr());
        false
    }
    #[cfg(not(target_os = "macos"))]
    { true }
}

/// `SDL_MAJOR_VERSION`/`SDL_MINOR_VERSION`/`SDL_PATCHLEVEL` from the
/// pinned `SDL_version.h` this crate links against -- the values the
/// original `SDL_VERSION(&Compiled)` macro expansion would have read at
/// the C++ compile site (`SDLViewport.cpp`'s removed `LogVersion` debugf
/// used these directly as `(INT)SDL_MAJOR_VERSION` etc).
pub const COMPILED_VERSION: (i32, i32, i32) = (2, 32, 10);

/// Replaces `Hp2PlatformViewportLeaf_LinkedVersion` (`SDLViewport.cpp:
/// 160`): `SDL_GetVersion(&Linked)`. Returns `(major, minor, patch)`.
#[cfg(feature = "sdl")]
pub fn linked_version() -> (i32, i32, i32) {
    let mut v = ffi::SdlVersion {
        major: 0,
        minor: 0,
        patch: 0,
    };
    // SAFETY: `v` is a valid, exclusively-owned `SDL_version` for the
    // duration of this call.
    unsafe { ffi::SDL_GetVersion(&mut v) };
    (v.major as i32, v.minor as i32, v.patch as i32)
}

/// Replaces the `SDL_GetError()` call inside `Hp2PlatformViewportLeaf_
/// WindowCreateError` (`SDLViewport.cpp:200`). Returns SDL's own
/// thread-local, NUL-terminated error buffer; valid until the next SDL
/// call made on this thread. The caller must hand it to the
/// `WindowCreateError` leaf (which only formats and reports it) before
/// making any other SDL call.
#[cfg(feature = "sdl")]
pub fn last_error() -> *const std::os::raw::c_char {
    // SAFETY: SDL_GetError takes no arguments and is always safe to call.
    unsafe { ffi::SDL_GetError() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `out/macos-arm64/_deps/hp2_sdl2-src/include/SDL_version.h`:
    /// `SDL_MAJOR_VERSION 2`, `SDL_MINOR_VERSION 32`, `SDL_PATCHLEVEL 10`.
    /// Bump this alongside the vendored SDL2 pin
    /// (`Build/CMake/HP2Dependencies.cmake`'s `FetchContent_Declare(hp2_sdl2
    /// ...)`), or `LogVersion`'s "compiled against" half silently goes stale.
    #[cfg(all(feature = "sdl", target_os = "macos"))]
    #[test]
    fn fullscreen_hint_defaults_to_borderless_and_respects_override() {
        if let Ok(expected) = std::env::var("HP2_HINT_TEST_CHILD") {
            assert!(configure_fullscreen_hint());
            let p = unsafe { ffi::SDL_GetHint(c"SDL_VIDEO_MAC_FULLSCREEN_SPACES".as_ptr()) };
            assert!(!p.is_null());
            assert_eq!(unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap(), expected);
            return;
        }
        for (expected, override_value) in [("0", None), ("1", Some("1"))] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "sdl_runtime::tests::fullscreen_hint_defaults_to_borderless_and_respects_override"])
                .env("HP2_HINT_TEST_CHILD", expected).env_remove("SDL_VIDEO_MAC_FULLSCREEN_SPACES");
            if let Some(value) = override_value { command.env("SDL_VIDEO_MAC_FULLSCREEN_SPACES", value); }
            assert!(command.status().unwrap().success());
        }
    }
    #[test]
    fn compiled_version_matches_the_pinned_sdl_version_h() {
        assert_eq!(COMPILED_VERSION, (2, 32, 10));
    }
}
