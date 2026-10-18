//! Canvas exports (`crates/hp2-engine/src/canvas.rs`). The C++
//! `UCanvas::DrawTile` in `Engine/Src/UnCanvas.cpp` calls these for the
//! clip and UV math; texture locking and `RenDev->DrawTile` stay in C++.

use hp2_engine::canvas::{self, Hp2CanvasTileRect};
use hp2_ffi_support::guard;

/// `UCanvas::DrawTile` clip block against `Frame->FX` / `Frame->FY`.
/// Reads and writes the tile at `rect`. Returns 1 when the tile is kept,
/// 0 when rejected (`rect` is then left unchanged).
///
/// # Safety
/// `rect` must be null or point to a live `Hp2CanvasTileRect` (`#[repr(C)]`, 8 f32).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_canvas_clip_tile(
    frame_fx: f32,
    frame_fy: f32,
    rect: *mut Hp2CanvasTileRect,
) -> u8 {
    guard("hp2_rs_canvas_clip_tile", 0, move || unsafe {
        let Some(rect) = rect.as_mut() else {
            return 0;
        };
        let mut clipped = *rect;
        if canvas::clip_tile(frame_fx, frame_fy, &mut clipped) {
            *rect = clipped;
            1
        } else {
            0
        }
    })
}

/// `Info.UScale * Info.USize / Texture->USize` (or the V variant).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_canvas_uv_factor(scale: f32, locked_size: i32, base_size: i32) -> f32 {
    guard("hp2_rs_canvas_uv_factor", 0.0, move || {
        canvas::uv_factor(scale, locked_size, base_size)
    })
}
