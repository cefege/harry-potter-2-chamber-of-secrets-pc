//! FFI exports for `hp2_platform::viewport`: the pure decision logic
//! behind `USDLViewport` (`HarryPotter2/Unreal/SDLDrv/Src/SDLViewport.cpp`).
//! Each export is a thin wrapper; the C++ shell keeps every `SDL_*` call
//! and every `UViewport`/`RenDev` field write.

use hp2_ffi_support::guard;
use hp2_platform::viewport as vp;

/// Mirror of the viewport fields `ResizeViewport` writes. Layout is
/// pinned by the `size_of`/`offset_of` test below and by the matching
/// `static_assert`s in `SDLViewport.cpp`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiViewportSize {
    pub blit_flags: u32,
    pub size_x: i32,
    pub size_y: i32,
    pub color_bytes: i32,
}

impl From<FfiViewportSize> for vp::ViewportSize {
    fn from(v: FfiViewportSize) -> Self {
        vp::ViewportSize {
            blit_flags: v.blit_flags,
            size_x: v.size_x,
            size_y: v.size_y,
            color_bytes: v.color_bytes,
        }
    }
}

impl From<vp::ViewportSize> for FfiViewportSize {
    fn from(v: vp::ViewportSize) -> Self {
        FfiViewportSize {
            blit_flags: v.blit_flags,
            size_x: v.size_x,
            size_y: v.size_y,
            color_bytes: v.color_bytes,
        }
    }
}

/// Replaces `UpdateMouseScale`'s math (`SDLViewport.cpp:34-41`).
///
/// # Safety
/// `out_x`/`out_y` must be valid, aligned, writable `f32` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_viewport_mouse_scale(
    window_x: i32,
    window_y: i32,
    size_x: i32,
    size_y: i32,
    out_x: *mut f32,
    out_y: *mut f32,
) {
    guard("hp2_rs_viewport_mouse_scale", (), || {
        let (x, y) = vp::mouse_scale(window_x, window_y, size_x, size_y);
        // SAFETY: caller contract above.
        unsafe {
            *out_x = x;
            *out_y = y;
        }
    })
}

/// Replaces the constructor's desktop colour-depth block
/// (`SDLViewport.cpp:84-101`). `has_bpp` is 0 when the desktop query
/// failed or the format was unknown; otherwise `bits_per_pixel` is
/// `SDL_BITSPERPIXEL(format)`.
///
/// # Safety
/// `out_color_bytes`/`out_rgb565` must be valid, aligned, writable `i32`
/// pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_viewport_desktop_color_depth(
    has_bpp: i32,
    bits_per_pixel: i32,
    out_color_bytes: *mut i32,
    out_rgb565: *mut i32,
) {
    guard("hp2_rs_viewport_desktop_color_depth", (), || {
        let bits = (has_bpp != 0).then_some(bits_per_pixel);
        let d = vp::desktop_color_depth(bits);
        // SAFETY: caller contract above.
        unsafe {
            *out_color_bytes = d.color_bytes;
            *out_rgb565 = i32::from(d.rgb565);
        }
    })
}

/// Replaces `ColorBytes = C->WindowedColorBits <= 16 ? 2 : 4`
/// (`SDLViewport.cpp:337`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_window_color_bytes(windowed_color_bits: i32) -> i32 {
    guard("hp2_rs_viewport_window_color_bytes", 4, || {
        vp::window_color_bytes(windowed_color_bits)
    })
}

/// Replaces the width/height selection in `OpenWindow`
/// (`SDLViewport.cpp:299-300`).
///
/// # Safety
/// `out_width`/`out_height` must be valid, aligned, writable `i32` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_viewport_open_window_size(
    new_x: i32,
    new_y: i32,
    open_x: i32,
    open_y: i32,
    windowed_viewport_x: i32,
    windowed_viewport_y: i32,
    out_width: *mut i32,
    out_height: *mut i32,
) {
    guard("hp2_rs_viewport_open_window_size", (), || {
        let (w, h) = vp::open_window_size(
            new_x,
            new_y,
            open_x,
            open_y,
            windowed_viewport_x,
            windowed_viewport_y,
        );
        // SAFETY: caller contract above.
        unsafe {
            *out_width = w;
            *out_height = h;
        }
    })
}

/// Replaces the `SDL_CreateWindow` flag expression in `OpenWindow`
/// (`SDLViewport.cpp:324-326`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_window_flags(temporary: i32, borderless: i32) -> u32 {
    guard("hp2_rs_viewport_window_flags", 0, || {
        vp::window_flags(temporary != 0, borderless != 0)
    })
}

/// Replaces the `StartupFullscreen || UseDesktopResolution` test in
/// `OpenWindow` (`SDLViewport.cpp:339`). Returns 1 or 0.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_startup_fullscreen(
    startup_fullscreen: i32,
    use_desktop_resolution: i32,
) -> i32 {
    guard("hp2_rs_viewport_startup_fullscreen", 0, || {
        i32::from(vp::startup_fullscreen(
            startup_fullscreen != 0,
            use_desktop_resolution != 0,
        ))
    })
}

/// Replaces `ResizeViewport`'s field transitions (`SDLViewport.cpp:216-232`).
///
/// # Safety
/// `inout` must be a valid, aligned, writable pointer to an
/// `FfiViewportSize`; it is read and overwritten in place.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_viewport_resize(
    inout: *mut FfiViewportSize,
    blit_type: u32,
    new_x: i32,
    new_y: i32,
    new_color_bytes: i32,
) {
    guard("hp2_rs_viewport_resize", (), || {
        // SAFETY: caller contract above.
        unsafe {
            let current = vp::ViewportSize::from(*inout);
            *inout = FfiViewportSize::from(vp::resize_viewport(
                current,
                blit_type,
                new_x,
                new_y,
                new_color_bytes,
            ));
        }
    })
}

/// Replaces `IsFullscreen` (`SDLViewport.cpp:750-753`). Returns 1 or 0.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_is_fullscreen(blit_flags: u32) -> i32 {
    guard("hp2_rs_viewport_is_fullscreen", 0, || {
        i32::from(vp::is_fullscreen(blit_flags))
    })
}

/// Replaces `SetModeCursor`'s index clamp (`SDLViewport.cpp:270`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_mode_cursor_index(selected_cursor: i32) -> i32 {
    guard("hp2_rs_viewport_mode_cursor_index", 0, || {
        vp::mode_cursor_index(selected_cursor) as i32
    })
}

/// Replaces `Lock`'s early-out (`SDLViewport.cpp:207-208`). Returns 1 or 0.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_viewport_lock_allowed(
    has_render_device: i32,
    size_x: i32,
    size_y: i32,
) -> i32 {
    guard("hp2_rs_viewport_lock_allowed", 0, || {
        i32::from(vp::lock_allowed(has_render_device != 0, size_x, size_y))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn ffi_viewport_size_layout_is_pinned() {
        assert_eq!(size_of::<FfiViewportSize>(), 16);
        assert_eq!(offset_of!(FfiViewportSize, blit_flags), 0);
        assert_eq!(offset_of!(FfiViewportSize, size_x), 4);
        assert_eq!(offset_of!(FfiViewportSize, size_y), 8);
        assert_eq!(offset_of!(FfiViewportSize, color_bytes), 12);
    }

    #[test]
    fn resize_export_round_trips_through_mirror() {
        let mut v = FfiViewportSize {
            blit_flags: 0,
            size_x: 640,
            size_y: 480,
            color_bytes: 4,
        };
        // SAFETY: `v` is a valid local.
        unsafe { hp2_rs_viewport_resize(&mut v, 0, 1024, -1, 2) };
        assert_eq!(
            v,
            FfiViewportSize {
                blit_flags: 0,
                size_x: 1024,
                size_y: 480,
                color_bytes: 2,
            }
        );
    }
}
