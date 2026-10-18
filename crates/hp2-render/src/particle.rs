//! Port of the pure helpers in `Render/Src/UnParticleRn.cpp`: `SetUV`,
//! `SetTrans`, `SetTrans2D`, `ComputeClipCodes2D`, `CombineClipCodes`, and
//! the `lerp` overloads. C++ keeps the UnrealScript natives, the particle
//! system/chunk storage, and the GMem-backed clipping code.
//!
//! Float contraction is replicated through `madd`: `mul_add` on arm64, where the
//! reference build's `-ffp-contract=on` fuses `a*b + c`, and a plain `a*b + c`
//! on other targets, where clang does not contract. The fixture test is aarch64-only.

pub use hp2_abi::render_particle::{FTransTexture, FTransform};
pub use hp2_abi::{FPlane, FVector};

/// `EVectorFlags` outcode bits (`Core/Inc/UnMath.h:176-185`).
pub const FVF_OUT_XMIN: u8 = 0x04;
pub const FVF_OUT_XMAX: u8 = 0x08;
pub const FVF_OUT_YMIN: u8 = 0x10;
pub const FVF_OUT_YMAX: u8 = 0x20;
pub const FVF_OUT_NEAR: u8 = 0x40;
/// `FVF_OutAll` from `UnParticleRn.cpp:40`.
pub const FVF_OUT_ALL: u8 =
    FVF_OUT_NEAR | FVF_OUT_XMIN | FVF_OUT_YMIN | FVF_OUT_XMAX | FVF_OUT_YMAX;

/// The `FSceneNode` fields `UnParticleRn.cpp` reads, in the order the
/// C++ forwarder packs them. `#[repr(C)]` so C++ can build it by value.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleFrame {
    pub proj_z: f32,
    pub rproj_z: f32,
    pub fx15: f32,
    pub fy15: f32,
    pub prjxm: f32,
    pub prjxp: f32,
    pub prjym: f32,
    pub prjyp: f32,
}

/// `IsNegativeFloat` (`Core/Inc/UnMath.h:140`): sign bit test, so `-0.0`
/// and negative NaNs count as negative.
#[inline]
fn is_negative_float(x: f32) -> bool {
    x.to_bits() >= 0x8000_0000
}

/// `a*b + c` as the reference build compiles it. On arm64, clang's
/// `-ffp-contract=on` fuses it into one `fma`; other presets do not contract.
#[inline]
pub(crate) fn madd(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        a * b + c
    }
}

/// `FTransform::ComputeOutcode` (`Engine/Inc/UnRender.h`): the four
/// `Prj*Z +/- X|Y` terms, each `< 0` selecting one reject bit.
/// `ClipXP`/`ClipYP` are `PrjXP*Z - X`, written as `madd(prjxp, z, -x)`.
#[inline]
pub fn compute_outcode(frame: &ParticleFrame, p: &FVector) -> u8 {
    let clip_xm = madd(frame.prjxm, p.z, p.x);
    let clip_xp = madd(frame.prjxp, p.z, -p.x);
    let clip_ym = madd(frame.prjym, p.z, p.y);
    let clip_yp = madd(frame.prjyp, p.z, -p.y);
    (if clip_xm < 0.0 { FVF_OUT_XMIN } else { 0 })
        | (if clip_xp < 0.0 { FVF_OUT_XMAX } else { 0 })
        | (if clip_ym < 0.0 { FVF_OUT_YMIN } else { 0 })
        | (if clip_yp < 0.0 { FVF_OUT_YMAX } else { 0 })
}

/// `FTexture` UV scaling in `SetUV`: `FLOAT(USize-1)*u`, `FLOAT(VSize-1)*v`.
pub fn set_uv(tt: &mut FTransTexture, tex_usize: i32, tex_vsize: i32, u: f32, v: f32) {
    tt.u = (tex_usize.wrapping_sub(1)) as f32 * u;
    tt.v = (tex_vsize.wrapping_sub(1)) as f32 * v;
}

/// `SetTrans`: fill a transformed vertex from screen position and depth.
/// `IntY = appFloor(ScreenY)` is `static_cast<INT>` (truncating), as on the
/// reference build.
#[allow(clippy::too_many_arguments)]
pub fn set_trans(
    tt: &mut FTransTexture,
    frame: &ParticleFrame,
    tex_usize: i32,
    tex_vsize: i32,
    screen_x: f32,
    screen_y: f32,
    screen_z: f32,
    u: f32,
    v: f32,
) {
    let t = &mut tt.transform;
    t.screen_x = screen_x;
    t.screen_y = screen_y;
    t.int_y = screen_y as i32;
    t.rz = screen_z;
    set_uv(tt, tex_usize, tex_vsize, u, v);
    let t = &mut tt.transform;
    t.point.z = frame.proj_z / screen_z;
    t.point.x = (screen_x - frame.fx15) * frame.rproj_z * t.point.z;
    t.point.y = (screen_y - frame.fy15) * frame.rproj_z * t.point.z;
    t.flags = compute_outcode(frame, &t.point);
}

/// `SetTrans2D`: copy a transformed point and offset it by `(dx, dy)` in
/// screen space. `ScreenX += dx*RZ` is a fused multiply-add.
pub fn set_trans_2d(tt: &mut FTransTexture, p3: &FTransform, dx: f32, dy: f32, u: f32, v: f32) {
    tt.transform = *p3;
    let t = &mut tt.transform;
    t.point.x += dx;
    t.point.y += dy;
    t.screen_x = madd(dx, t.rz, t.screen_x);
    t.screen_y = madd(dy, t.rz, t.screen_y);
    t.int_y = t.screen_y as i32;
    tt.u = u;
    tt.v = v;
}

/// `ComputeClipCodes2D`: outcode of a point offset by `(dx, dy)` in screen
/// space. Returns `FVF_OUT_ALL` if any term is negative, otherwise the
/// reject bits after subtracting `2*dx`/`2*dy` (the ADJUST_EDGE macro is the
/// identity on this build).
pub fn compute_clip_codes_2d(frame: &ParticleFrame, c: &FVector, dx: f32, dy: f32) -> u8 {
    let mut clip_xm = madd(frame.prjxm, c.z, c.x) + dx;
    let mut clip_xp = madd(frame.prjxp, c.z, -c.x) + dx;
    let mut clip_ym = madd(frame.prjym, c.z, c.y) + dy;
    let mut clip_yp = madd(frame.prjyp, c.z, -c.y) + dy;
    if is_negative_float(clip_xm)
        || is_negative_float(clip_xp)
        || is_negative_float(clip_ym)
        || is_negative_float(clip_yp)
    {
        return FVF_OUT_ALL;
    }
    clip_xm -= 2.0 * dx;
    clip_xp -= 2.0 * dx;
    clip_ym -= 2.0 * dy;
    clip_yp -= 2.0 * dy;
    (if is_negative_float(clip_xm) {
        FVF_OUT_XMIN
    } else {
        0
    }) | (if is_negative_float(clip_xp) {
        FVF_OUT_XMAX
    } else {
        0
    }) | (if is_negative_float(clip_ym) {
        FVF_OUT_YMIN
    } else {
        0
    }) | (if is_negative_float(clip_yp) {
        FVF_OUT_YMAX
    } else {
        0
    })
}

/// `CombineClipCodes`: OR of the outcodes, or `FVF_OUT_ALL` if all share a
/// reject bit (AND nonzero). Callers pass at least one vertex; an empty
/// slice returns 0.
pub fn combine_clip_codes(verts: &[FTransTexture]) -> u8 {
    let Some(first) = verts.first() else {
        return 0;
    };
    let mut clip = first.transform.flags;
    let mut cull = first.transform.flags;
    for v in &verts[1..] {
        cull &= v.transform.flags;
        clip |= v.transform.flags;
    }
    if cull != 0 { FVF_OUT_ALL } else { clip }
}

/// `lerp(FLOAT, const FVector&, const FVector&, FVector&)`:
/// `a + f*(b-a)` fused as `fma(f, b-a, a)`.
pub fn lerp_vector(f: f32, a: &FVector, b: &FVector) -> FVector {
    FVector {
        x: madd(f, b.x - a.x, a.x),
        y: madd(f, b.y - a.y, a.y),
        z: madd(f, b.z - a.z, a.z),
    }
}

/// `lerp(FLOAT, const FPlane&, const FPlane&, FPlane&)`, same fusion.
pub fn lerp_plane(f: f32, a: &FPlane, b: &FPlane) -> FPlane {
    FPlane {
        x: madd(f, b.x - a.x, a.x),
        y: madd(f, b.y - a.y, a.y),
        z: madd(f, b.z - a.z, a.z),
        w: madd(f, b.w - a.w, a.w),
    }
}

/// `lerp(FLOAT, const FTransTexture*, ..., FTransTexture*)`: interpolates
/// `U`, `V`, `Point`, and `Light` only; the other fields of `out` are left
/// untouched, as in the C++.
pub fn lerp_trans(f: f32, a: &FTransTexture, b: &FTransTexture, out: &mut FTransTexture) {
    out.u = madd(f, b.u - a.u, a.u);
    out.v = madd(f, b.v - a.v, a.v);
    out.transform.point = lerp_vector(f, &a.transform.point, &b.transform.point);
    out.light = lerp_plane(f, &a.light, &b.light);
}
