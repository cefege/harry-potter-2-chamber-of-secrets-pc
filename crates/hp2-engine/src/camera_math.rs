//! Pure float math from `Engine/Src/UnCamera.cpp`: `Draw2DClippedLine` clip
//! steps, `ClipFloatFromZero`, and the `Draw3DLine` perspective projection.
//!
//! FMA fidelity: the perspective step `P.X = P.X * Proj.Z * P.Z + SX2` is
//! fused in the reference object (`fmul` then `fmadd` at UnCamera.cpp:255-256),
//! so it is written as `(x * proj_z).mul_add(z, sx2)`. Dot products use
//! [`crate::prim::dot`].

use hp2_abi::FVector;

/// `LINE_NEAR_CLIP_Z` (UnCamera.cpp:19).
pub const LINE_NEAR_CLIP_Z: f32 = 1.0;

/// `ClipFloatFromZero(F, Range)` (UnMath.h:159). Decided on the bit pattern:
/// a set sign bit (including -0.0) clamps to 0.0; a value above `range` in
/// bit order clamps to `range`.
#[inline]
pub fn clip_float_from_zero(f: f32, range: f32) -> f32 {
    if f.to_bits() >= 0x8000_0000 {
        0.0
    } else if f.to_bits() > range.to_bits() {
        range
    } else {
        f
    }
}

/// Perspective step of `Draw3DLine` for one point (UnCamera.cpp:255).
/// `p` must already be clipped to the near plane.
#[inline]
pub fn perspective_point(p: FVector, proj_z: f32, sx2: f32, sy2: f32) -> FVector {
    let z = 1.0f32 / p.z;
    FVector {
        x: (p.x * proj_z).mul_add(z, sx2),
        y: (p.y * proj_z).mul_add(z, sy2),
        z,
    }
}

/// X and Y clip of `Draw2DClippedLine` (UnCamera.cpp:120-157).
/// Returns `None` when the line is rejected. Coordinate clamping is applied
/// by the caller via [`clip_float_from_zero`].
pub fn clip_2d_line(
    fx: f32,
    fy: f32,
    mut p1: FVector,
    mut p2: FVector,
) -> Option<(FVector, FVector)> {
    if p1.x > p2.x {
        std::mem::swap(&mut p1, &mut p2);
    }
    if p2.x < 0.0 || p1.x > fx {
        return None;
    }
    if p1.x < 0.0 {
        if (p2.x - p1.x).abs() < 0.001 {
            return None;
        }
        p1.y += (0.0 - p1.x) * (p2.y - p1.y) / (p2.x - p1.x);
        p1.x = 0.0;
    }
    if p2.x >= fx {
        if (p2.x - p1.x).abs() < 0.001 {
            return None;
        }
        p2.y += ((fx - 1.0) - p2.x) * (p2.y - p1.y) / (p2.x - p1.x);
        p2.x = fx - 1.0;
    }

    if p1.y > p2.y {
        std::mem::swap(&mut p1, &mut p2);
    }
    if p2.y < 0.0 || p1.y > fy {
        return None;
    }
    if p1.y < 0.0 {
        if (p2.y - p1.y).abs() < 0.001 {
            return None;
        }
        p1.x += (0.0 - p1.y) * (p2.x - p1.x) / (p2.y - p1.y);
        p1.y = 0.0;
    }
    if p2.y >= fy {
        if (p2.y - p1.y).abs() < 0.001 {
            return None;
        }
        p2.x += ((fy - 1.0) - p2.y) * (p2.x - p1.x) / (p2.y - p1.y);
        p2.y = fy - 1.0;
    }

    p1.x = clip_float_from_zero(p1.x, fx);
    p2.x = clip_float_from_zero(p2.x, fx);
    p1.y = clip_float_from_zero(p1.y, fy);
    p2.y = clip_float_from_zero(p2.y, fy);
    Some((p1, p2))
}
