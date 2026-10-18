//! Pure decision logic extracted from `USDLViewport`
//! (`HarryPotter2/Unreal/SDLDrv/Src/SDLViewport.cpp`): mouse scale,
//! desktop/windowed colour depth, window dimensions and creation flags,
//! `ResizeViewport`'s blit-flag and size transitions, `SetModeCursor`'s
//! index clamp, and `Lock`'s render-device gate.
//!
//! Nothing here touches SDL or the engine. The C++ shell keeps every
//! `SDL_*` call and every `UViewport`/`RenDev` field write; it stores the
//! values returned here into its own fields. Each function mirrors one
//! C++ block line for line. Reference outputs were captured by compiling
//! those blocks verbatim with clang++ (`Tests` below pin them).

/// `INDEX_NONE` (`UnCore.h`): "argument not supplied" sentinel.
pub const INDEX_NONE: i32 = -1;

/// `BLIT_Fullscreen` (`UnCamera.h`).
pub const BLIT_FULLSCREEN: u32 = 0x0001;
/// `BLIT_NoWindowChange` (`UnCamera.h`).
pub const BLIT_NO_WINDOW_CHANGE: u32 = 0x0020;

/// `SDL_WINDOW_OPENGL`, `SDL_WINDOW_HIDDEN`, `SDL_WINDOW_BORDERLESS`,
/// `SDL_WINDOW_RESIZABLE` (`SDL_video.h`, pinned SDL2).
pub const SDL_WINDOW_OPENGL: u32 = 0x0000_0002;
pub const SDL_WINDOW_HIDDEN: u32 = 0x0000_0008;
pub const SDL_WINDOW_BORDERLESS: u32 = 0x0000_0010;
pub const SDL_WINDOW_RESIZABLE: u32 = 0x0000_0020;

/// `NumSystemCursors` (`SDLDrv.h`): arrow, sizeall, sizenesw, sizens,
/// sizenwse, sizewe, wait.
pub const NUM_SYSTEM_CURSORS: i32 = 7;

/// `UpdateMouseScale` (`SDLViewport.cpp:34-41`). `window_x`/`window_y`
/// are the `SDL_GetWindowSize` results (0 when there is no window).
/// Returns `(MouseScaleX, MouseScaleY)`.
pub fn mouse_scale(window_x: i32, window_y: i32, size_x: i32, size_y: i32) -> (f32, f32) {
    let scale_x = if window_x > 0 && size_x > 0 {
        size_x as f32 / window_x as f32
    } else {
        1.0
    };
    let scale_y = if window_y > 0 && size_y > 0 {
        size_y as f32 / window_y as f32
    } else {
        1.0
    };
    (scale_x, scale_y)
}

/// Colour depth the viewport adopts from the desktop mode
/// (`USDLViewport` constructor, `SDLViewport.cpp:84-101`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorDepth {
    pub color_bytes: i32,
    /// True when the constructor sets `CC_RGB565` on `Caps`.
    pub rgb565: bool,
}

/// `desktop_bits_per_pixel` is `None` when `SDL_GetDesktopDisplayMode`
/// failed or reported `SDL_PIXELFORMAT_UNKNOWN` (the 32-bit default
/// stands). `Some(bits)` is `SDL_BITSPERPIXEL(format)`.
pub fn desktop_color_depth(desktop_bits_per_pixel: Option<i32>) -> ColorDepth {
    match desktop_bits_per_pixel {
        Some(bits) if bits > 0 && bits <= 16 => ColorDepth {
            color_bytes: 2,
            rgb565: true,
        },
        _ => ColorDepth {
            color_bytes: 4,
            rgb565: false,
        },
    }
}

/// `OpenWindow`'s `ColorBytes = C->WindowedColorBits <= 16 ? 2 : 4`
/// (`SDLViewport.cpp:337`).
pub fn window_color_bytes(windowed_color_bits: i32) -> i32 {
    if windowed_color_bits <= 16 { 2 } else { 4 }
}

/// Requested window size in `OpenWindow` (`SDLViewport.cpp:299-300`):
/// the first of `NewX`/`OpenX`/`WindowedViewportX` that is not
/// `INDEX_NONE`, and likewise for Y. Returns `(width, height)`.
pub fn open_window_size(
    new_x: i32,
    new_y: i32,
    open_x: i32,
    open_y: i32,
    windowed_viewport_x: i32,
    windowed_viewport_y: i32,
) -> (i32, i32) {
    let width = if new_x != INDEX_NONE {
        new_x
    } else if open_x != INDEX_NONE {
        open_x
    } else {
        windowed_viewport_x
    };
    let height = if new_y != INDEX_NONE {
        new_y
    } else if open_y != INDEX_NONE {
        open_y
    } else {
        windowed_viewport_y
    };
    (width, height)
}

/// `SDL_CreateWindow` flags in `OpenWindow` (`SDLViewport.cpp:324-326`).
pub fn window_flags(temporary: bool, borderless: bool) -> u32 {
    let mut flags = SDL_WINDOW_OPENGL | SDL_WINDOW_RESIZABLE;
    if temporary {
        flags |= SDL_WINDOW_HIDDEN;
    }
    if borderless {
        flags |= SDL_WINDOW_BORDERLESS;
    }
    flags
}

/// `OpenWindow` fullscreen start (`SDLViewport.cpp:338-343`): whether
/// `BLIT_Fullscreen` is set after the window opens.
pub fn startup_fullscreen(startup_fullscreen: bool, use_desktop_resolution: bool) -> bool {
    startup_fullscreen || use_desktop_resolution
}

/// The viewport fields `ResizeViewport` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportSize {
    pub blit_flags: u32,
    pub size_x: i32,
    pub size_y: i32,
    pub color_bytes: i32,
}

/// `USDLViewport::ResizeViewport` field transitions
/// (`SDLViewport.cpp:216-232`). `blit_type` is the engine's `BlitType`.
pub fn resize_viewport(
    current: ViewportSize,
    blit_type: u32,
    new_x: i32,
    new_y: i32,
    new_color_bytes: i32,
) -> ViewportSize {
    let mut out = current;
    if blit_type & BLIT_NO_WINDOW_CHANGE == 0 {
        if blit_type & BLIT_FULLSCREEN != 0 {
            out.blit_flags |= BLIT_FULLSCREEN;
        } else {
            out.blit_flags &= !BLIT_FULLSCREEN;
        }
    }
    if new_x != INDEX_NONE {
        out.size_x = new_x;
    }
    if new_y != INDEX_NONE {
        out.size_y = new_y;
    }
    if new_color_bytes != INDEX_NONE {
        out.color_bytes = clamp_i32(new_color_bytes, 2, 4);
    }
    out
}

/// `IsFullscreen` (`SDLViewport.cpp:750-753`).
pub fn is_fullscreen(blit_flags: u32) -> bool {
    blit_flags & BLIT_FULLSCREEN != 0
}

/// `SetModeCursor`'s index (`SDLViewport.cpp:270`):
/// `Clamp(SelectedCursor, 0, NumSystemCursors - 1)`.
pub fn mode_cursor_index(selected_cursor: i32) -> usize {
    clamp_i32(selected_cursor, 0, NUM_SYSTEM_CURSORS - 1) as usize
}

/// `Lock`'s early-out (`SDLViewport.cpp:207-208`): proceed only with a
/// render device and a non-zero viewport.
pub fn lock_allowed(has_render_device: bool, size_x: i32, size_y: i32) -> bool {
    has_render_device && size_x != 0 && size_y != 0
}

/// Engine `Clamp(X, Min, Max)` for `INT`: `X < Min ? Min : X > Max ? Max : X`.
fn clamp_i32(x: i32, min: i32, max: i32) -> i32 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values captured from the verbatim C++ bodies with clang++
    // (see the module doc). f32 results are compared by bit pattern.

    #[test]
    fn mouse_scale_matches_cpp_reference() {
        type Case = ((i32, i32, i32, i32), (u32, u32));
        let cases: [Case; 5] = [
            ((640, 480, 320, 240), (1056964608, 1056964608)),
            ((0, 0, 0, 0), (1065353216, 1065353216)),
            ((100, 0, 50, 200), (1056964608, 1065353216)),
            ((-5, 10, 20, 30), (1065353216, 1077936128)),
            ((1024, 768, 0, 0), (1065353216, 1065353216)),
        ];
        for ((wx, wy, sx, sy), (ex, ey)) in cases {
            let (x, y) = mouse_scale(wx, wy, sx, sy);
            assert_eq!((x.to_bits(), y.to_bits()), (ex, ey), "{wx} {wy} {sx} {sy}");
        }
    }

    #[test]
    fn mouse_scale_fractional_ratio_matches_cpp_reference() {
        let (x, y) = mouse_scale(1280, 800, 1920, 1080);
        assert_eq!((x.to_bits(), y.to_bits()), (1069547520, 1068289229));
    }

    #[test]
    fn desktop_color_depth_matches_cpp_reference() {
        let cases: [(Option<i32>, i32, bool); 8] = [
            (Some(-1), 4, false),
            (Some(0), 4, false),
            (Some(8), 2, true),
            (Some(15), 2, true),
            (Some(16), 2, true),
            (Some(24), 4, false),
            (Some(32), 4, false),
            (None, 4, false),
        ];
        for (bits, cb, rgb) in cases {
            let d = desktop_color_depth(bits);
            assert_eq!((d.color_bytes, d.rgb565), (cb, rgb), "{bits:?}");
        }
    }

    #[test]
    fn window_color_bytes_matches_cpp_reference() {
        for (bits, expect) in [(0, 2), (8, 2), (16, 2), (17, 4), (24, 4), (32, 4)] {
            assert_eq!(window_color_bytes(bits), expect, "{bits}");
        }
    }

    #[test]
    fn open_window_size_matches_cpp_reference() {
        let i = INDEX_NONE;
        assert_eq!(open_window_size(i, i, i, i, 1024, 768), (1024, 768));
        assert_eq!(open_window_size(800, 600, 320, 200, 1024, 768), (800, 600));
        assert_eq!(open_window_size(i, 480, 320, 200, 1024, 768), (320, 480));
        assert_eq!(open_window_size(i, i, 320, i, 1024, 768), (320, 768));
    }

    #[test]
    fn window_flags_match_cpp_reference() {
        assert_eq!(window_flags(false, false), 34);
        assert_eq!(window_flags(false, true), 50);
        assert_eq!(window_flags(true, false), 42);
        assert_eq!(window_flags(true, true), 58);
    }

    #[test]
    fn startup_fullscreen_truth_table() {
        assert!(!startup_fullscreen(false, false));
        assert!(startup_fullscreen(true, false));
        assert!(startup_fullscreen(false, true));
        assert!(startup_fullscreen(true, true));
    }

    #[test]
    fn resize_viewport_matches_cpp_reference() {
        let base = |blit, sx, sy, cb| ViewportSize {
            blit_flags: blit,
            size_x: sx,
            size_y: sy,
            color_bytes: cb,
        };
        let i = INDEX_NONE;
        let cases = [
            // (blit_type, nx, ny, ncb, in_blit) -> out
            ((0, i, i, i, 0), base(0, 640, 480, 4)),
            ((BLIT_FULLSCREEN, 1024, 768, 2, 0), base(1, 1024, 768, 2)),
            (
                (BLIT_FULLSCREEN | BLIT_NO_WINDOW_CHANGE, 1024, 768, 3, 0),
                base(0, 1024, 768, 3),
            ),
            ((0, 800, i, 9, BLIT_FULLSCREEN), base(0, 800, 480, 4)),
            ((0, i, 600, 1, 0), base(0, 640, 600, 2)),
            (
                (BLIT_NO_WINDOW_CHANGE, 320, 200, 4, BLIT_FULLSCREEN),
                base(1, 320, 200, 4),
            ),
        ];
        for ((blit, nx, ny, ncb, in_blit), expect) in cases {
            let start = base(in_blit, 640, 480, 4);
            let got = resize_viewport(start, blit, nx, ny, ncb);
            assert_eq!(
                got, expect,
                "blit {blit} nx {nx} ny {ny} ncb {ncb} in {in_blit}"
            );
        }
    }

    #[test]
    fn resize_viewport_clamps_color_bytes_from_below_before_change_size_only() {
        let start = ViewportSize {
            blit_flags: 0,
            size_x: 640,
            size_y: 480,
            color_bytes: 4,
        };
        let got = resize_viewport(start, 0, INDEX_NONE, INDEX_NONE, 1);
        assert_eq!(got.color_bytes, 2);
        assert_eq!((got.size_x, got.size_y), (640, 480));
    }

    #[test]
    fn is_fullscreen_reads_only_blit_fullscreen_bit() {
        assert!(!is_fullscreen(0));
        assert!(is_fullscreen(BLIT_FULLSCREEN));
        assert!(!is_fullscreen(BLIT_NO_WINDOW_CHANGE));
    }

    #[test]
    fn mode_cursor_index_matches_cpp_reference() {
        for (sel, expect) in [(-3, 0), (0, 0), (3, 3), (6, 6), (7, 6), (100, 6)] {
            assert_eq!(mode_cursor_index(sel), expect, "{sel}");
        }
    }

    #[test]
    fn lock_allowed_requires_device_and_nonzero_size() {
        assert!(lock_allowed(true, 640, 480));
        assert!(!lock_allowed(false, 640, 480));
        assert!(!lock_allowed(true, 0, 480));
        assert!(!lock_allowed(true, 640, 0));
    }
}
