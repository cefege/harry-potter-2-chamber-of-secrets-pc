//! Replaces `USDLViewport`'s `ConstructorDesktop` scope: `SDL_Get
//! DesktopDisplayMode` and `SDL_BITSPERPIXEL` (`SDLViewport.cpp:236-244`).

#[cfg(feature = "sdl")]
mod ffi {
    use std::os::raw::{c_int, c_void};

    /// `SDL_DisplayMode` (`SDL_video.h`): plain data, not opaque, but
    /// only ever read back after `SDL_GetDesktopDisplayMode` fills it.
    #[repr(C)]
    pub struct SdlDisplayMode {
        pub format: u32,
        pub w: c_int,
        pub h: c_int,
        pub refresh_rate: c_int,
        pub driverdata: *mut c_void,
    }

    unsafe extern "C" {
        pub fn SDL_GetDesktopDisplayMode(display_index: c_int, mode: *mut SdlDisplayMode) -> c_int;
    }
}

/// `SDL_PIXELFORMAT_UNKNOWN` (`SDL_pixels.h`): the first, zero-valued
/// enumerator.
const SDL_PIXELFORMAT_UNKNOWN: u32 = 0;

/// `SDL_BITSPERPIXEL(format)` (`SDL_pixels.h`): `(format >> 8) & 0xFF`.
/// Pure bit extraction, no FFI dependency, independently unit-testable
/// without the `sdl` feature.
fn bits_per_pixel(format: u32) -> i32 {
    ((format >> 8) & 0xFF) as i32
}

/// Replaces the `ConstructorDesktop` phase (`SDLViewport.cpp:236-244`):
/// `SDL_GetDesktopDisplayMode(0, &mode)`, then, only on success with a
/// known format, `SDL_BITSPERPIXEL(mode.format)`. `None` when the call
/// failed or reported `SDL_PIXELFORMAT_UNKNOWN` (the original's
/// `format_known` gate) -- `viewport::desktop_color_depth` then supplies
/// the 32-bit default for either case.
#[cfg(feature = "sdl")]
pub fn desktop_color_bits() -> Option<i32> {
    let mut mode = ffi::SdlDisplayMode {
        format: 0,
        w: 0,
        h: 0,
        refresh_rate: 0,
        driverdata: std::ptr::null_mut(),
    };
    // SAFETY: `mode` is a valid, zero-initialized out-param for the
    // duration of this call; SDL's video subsystem is initialized by
    // construction (C++ owns that init and only calls into this crate
    // afterward).
    let success = unsafe { ffi::SDL_GetDesktopDisplayMode(0, &mut mode) } == 0;
    if success && mode.format != SDL_PIXELFORMAT_UNKNOWN {
        Some(bits_per_pixel(mode.format))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SDL_PIXELFORMAT_RGB888` (`SDL_pixels.h`) is `0x16161804`;
    /// `SDL_BITSPERPIXEL` extracts bits 8-15: `(0x...18.. >> 8) & 0xFF
    /// == 0x18 == 24`.
    #[test]
    fn bits_per_pixel_matches_sdl_macro() {
        assert_eq!(bits_per_pixel(0x1616_1804), 24);
    }

    /// `SDL_PIXELFORMAT_UNKNOWN` itself has no meaningful bit count.
    #[test]
    fn bits_per_pixel_of_unknown_format_is_zero() {
        assert_eq!(bits_per_pixel(SDL_PIXELFORMAT_UNKNOWN), 0);
    }
}
