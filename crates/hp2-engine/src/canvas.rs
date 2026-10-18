//! Port of the pure clip and UV math in `Engine/Src/UnCanvas.cpp`
//! (`UCanvas::DrawTile`, lines 88-113 of commit 40bde73). Everything here
//! is free of engine state: the caller passes the frame extents, and the
//! texture-lock results are passed in as values. Float operations keep the
//! C++ evaluation order so results are bit-identical.

/// Tile rectangle in screen space plus its texture-space rectangle, as
/// passed through `UCanvas::DrawTile` (`X, Y, XL, YL, U, V, UL, VL`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hp2CanvasTileRect {
    pub x: f32,
    pub y: f32,
    pub xl: f32,
    pub yl: f32,
    pub u: f32,
    pub v: f32,
    pub ul: f32,
    pub vl: f32,
}

/// `UCanvas::DrawTile` rejection and clipping against the frame
/// (`Frame->FX`, `Frame->FY`). The top clip edge is `ClipY0 = 0`.
///
/// Returns `false` when the tile is rejected; otherwise `rect` holds the
/// clipped rectangle.
pub fn clip_tile(frame_fx: f32, frame_fy: f32, rect: &mut Hp2CanvasTileRect) -> bool {
    let clip_y0: f32 = 0.0;
    let clip_y1: f32 = frame_fy;
    let Hp2CanvasTileRect {
        mut x,
        mut y,
        mut xl,
        mut yl,
        mut u,
        mut v,
        mut ul,
        mut vl,
    } = *rect;

    // Reject.
    if xl <= 0.0 || yl <= 0.0 || x + xl <= 0.0 || y + yl <= clip_y0 || x >= frame_fx || y >= clip_y1
    {
        return false;
    }

    // Clip.
    if x < 0.0 {
        let c = x * ul / xl;
        u -= c;
        ul += c;
        xl += x;
        x = 0.0;
    }
    if y < 0.0 {
        let c = y * vl / yl;
        v -= c;
        vl += c;
        yl += y;
        y = 0.0;
    }
    if xl > frame_fx - x {
        ul += (frame_fx - x - xl) * ul / xl;
        xl = frame_fx - x;
    }
    if yl > frame_fy - y {
        vl += (frame_fy - y - yl) * vl / yl;
        yl = frame_fy - y;
    }

    *rect = Hp2CanvasTileRect {
        x,
        y,
        xl,
        yl,
        u,
        v,
        ul,
        vl,
    };
    true
}

/// `Info.UScale * Info.USize / Texture->USize` (or the V variant): the
/// factor that maps base-texture UV into locked-mip UV.
pub fn uv_factor(scale: f32, locked_size: i32, base_size: i32) -> f32 {
    scale * (locked_size as f32) / (base_size as f32)
}
