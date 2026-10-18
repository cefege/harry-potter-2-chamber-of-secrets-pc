//! Port of `ShardParticles::SetupVerts`, `LiquidParticles::SetupVerts`, and the
//! free function `ScreenVector` from `Render/Src/UnParticleRn.cpp`.
//!
//! Float contraction follows `particle.rs`: each C++ `a*b + c` (or `c - a*b`)
//! that the arm64 reference build fuses is written through `madd` / `msub`.
//! Trig and `DivSqrtApprox` come through [`RenderMath`], which the services
//! table implements by calling `GMath`; the engine tables are never copied.

use hp2_abi::FCoords;

use crate::particle::{
    FTransTexture, FTransform, FVector, ParticleFrame, combine_clip_codes, compute_outcode, madd,
    set_trans, set_uv,
};
use crate::services::Hp2RenderServices;

/// The `GMath` accessors the particle geometry uses.
pub trait RenderMath {
    /// `GMath.SinTab(angle)`.
    fn sin_tab(&self, angle: i32) -> f32;
    /// `GMath.CosTab(angle)`.
    fn cos_tab(&self, angle: i32) -> f32;
    /// `GMath.DivSqrtApprox(f)`.
    fn gmath_div_sqrt_approx(&self, f: f32) -> f32;
}

impl RenderMath for Hp2RenderServices {
    fn sin_tab(&self, angle: i32) -> f32 {
        // SAFETY: `GRenderServices` is a process-lifetime table whose entries
        // are C++ functions that read the engine's `GMath` tables.
        unsafe { (self.sin_tab)(angle) }
    }

    fn cos_tab(&self, angle: i32) -> f32 {
        // SAFETY: as in `sin_tab`.
        unsafe { (self.cos_tab)(angle) }
    }

    fn gmath_div_sqrt_approx(&self, f: f32) -> f32 {
        // SAFETY: as in `sin_tab`.
        unsafe { (self.gmath_div_sqrt_approx)(f) }
    }
}

/// The `UParticle` fields the liquid and streak geometry reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleState {
    pub position: FVector,
    pub velocity: FVector,
    pub length: f32,
    pub width: f32,
    pub drip_timer: f32,
}

const ZERO: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

/// `a - b*c` as the reference build compiles it: one `fmsub`/`fnmsub`.
#[inline]
fn msub(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    {
        (-b).mul_add(c, a)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        a - b * c
    }
}

#[inline]
fn sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

#[inline]
fn scale(a: FVector, s: f32) -> FVector {
    FVector {
        x: a.x * s,
        y: a.y * s,
        z: a.z * s,
    }
}

/// `FVector::operator|`: `X*V.X + Y*V.Y + Z*V.Z`, fused as
/// `madd(z, v.z, madd(x, v.x, y*v.y))`.
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    madd(a.z, b.z, madd(a.x, b.x, a.y * b.y))
}

/// `FVector::SizeSquared`.
#[inline]
fn size_squared(a: FVector) -> f32 {
    dot(a, a)
}

/// `FVector::Size`: `appSqrt(X*X+Y*Y+Z*Z)`. `appSqrt` takes a `DOUBLE`, but
/// the double sqrt rounded back to float equals the float sqrt.
#[inline]
fn size(a: FVector) -> f32 {
    size_squared(a).sqrt()
}

/// `FVector::Size2D`: `appSqrt(X*X+Y*Y)`.
#[inline]
fn size_2d(a: FVector) -> f32 {
    madd(a.x, a.x, a.y * a.y).sqrt()
}

/// `FVector::SafeNormal`: zero for a squared length below `SMALL_NUMBER`.
#[inline]
fn safe_normal(a: FVector) -> FVector {
    const SMALL_NUMBER: f32 = 1e-8;
    let square_sum = size_squared(a);
    if square_sum < SMALL_NUMBER {
        return ZERO;
    }
    let scale_factor = 1.0 / square_sum.sqrt();
    scale(a, scale_factor)
}

/// `FVector::TransformPointBy(FCoords)`: `(p - origin)` dotted with each axis.
#[inline]
fn transform_point_by(p: FVector, c: &FCoords) -> FVector {
    let t = sub(p, c.origin);
    FVector {
        x: dot(t, c.x_axis),
        y: dot(t, c.y_axis),
        z: dot(t, c.z_axis),
    }
}

/// `FVector::TransformVectorBy(FCoords)`: `v` dotted with each axis.
#[inline]
fn transform_vector_by(v: FVector, c: &FCoords) -> FVector {
    FVector {
        x: dot(v, c.x_axis),
        y: dot(v, c.y_axis),
        z: dot(v, c.z_axis),
    }
}

/// `FCoords::operator*=(FCoords)`: origin as a point, axes as vectors, all
/// against the other transform. Axes are read before they are written.
fn coords_mul_coords(c: &FCoords, t: &FCoords) -> FCoords {
    FCoords {
        origin: transform_point_by(c.origin, t),
        x_axis: transform_vector_by(c.x_axis, t),
        y_axis: transform_vector_by(c.y_axis, t),
        z_axis: transform_vector_by(c.z_axis, t),
    }
}

/// `FCoords::operator*=(FVector)`: `Origin -= Point`.
#[inline]
fn coords_mul_point(c: &FCoords, p: FVector) -> FCoords {
    FCoords {
        origin: sub(c.origin, p),
        ..*c
    }
}

/// `FCoords::operator*=(FRotator)`: yaw, then pitch, then roll, each applied
/// as `*= FCoords(origin 0, axes...)`.
fn coords_mul_rotator<M: RenderMath>(
    m: &M,
    c: &FCoords,
    pitch: i32,
    yaw: i32,
    roll: i32,
) -> FCoords {
    let (sin_yaw, cos_yaw) = (m.sin_tab(yaw), m.cos_tab(yaw));
    let yaw_coords = FCoords {
        origin: ZERO,
        x_axis: FVector {
            x: cos_yaw,
            y: sin_yaw,
            z: 0.0,
        },
        y_axis: FVector {
            x: -sin_yaw,
            y: cos_yaw,
            z: 0.0,
        },
        z_axis: FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    };
    let c = coords_mul_coords(c, &yaw_coords);

    let (sin_pitch, cos_pitch) = (m.sin_tab(pitch), m.cos_tab(pitch));
    let pitch_coords = FCoords {
        origin: ZERO,
        x_axis: FVector {
            x: cos_pitch,
            y: 0.0,
            z: sin_pitch,
        },
        y_axis: FVector {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        z_axis: FVector {
            x: -sin_pitch,
            y: 0.0,
            z: cos_pitch,
        },
    };
    let c = coords_mul_coords(&c, &pitch_coords);

    let (sin_roll, cos_roll) = (m.sin_tab(roll), m.cos_tab(roll));
    let roll_coords = FCoords {
        origin: ZERO,
        x_axis: FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        y_axis: FVector {
            x: 0.0,
            y: cos_roll,
            z: -sin_roll,
        },
        z_axis: FVector {
            x: 0.0,
            y: sin_roll,
            z: cos_roll,
        },
    };
    coords_mul_coords(&c, &roll_coords)
}

/// `FTransform::Project(Frame)`: sets `RZ`, `ScreenX`, `ScreenY`, `IntY`.
/// `IntY` is `appFloor` as a truncating `static_cast<INT>`.
#[inline]
pub fn project(t: &mut FTransform, frame: &ParticleFrame) {
    t.rz = frame.proj_z / t.point.z;
    t.screen_x = madd(t.point.x, t.rz, frame.fx15);
    t.screen_y = madd(t.point.y, t.rz, frame.fy15);
    t.int_y = t.screen_y as i32;
}

/// A zeroed `FTransform`. The C++ `Tail`/`Tip` locals are only read after
/// `Project`, which writes every field that is read.
#[inline]
fn zero_transform() -> FTransform {
    FTransform {
        point: ZERO,
        flags: 0,
        screen_x: 0.0,
        screen_y: 0.0,
        int_y: 0,
        rz: 0.0,
    }
}

/// `ShardParticles::SetupVerts`. Fills the three triangle vertices and
/// returns the combined clip codes.
#[allow(clippy::too_many_arguments)]
pub fn shard_setup_verts<M: RenderMath>(
    m: &M,
    frame: &ParticleFrame,
    coords: &FCoords,
    tex_usize: i32,
    tex_vsize: i32,
    position: FVector,
    spin: f32,
    length: f32,
    width: f32,
    verts: &mut [FTransTexture; 3],
) -> u8 {
    // `FRotator(Spin*65535, Spin*65535, 0)`: float-to-INT truncation.
    let angle = (spin * 65535.0) as i32;
    let unit = FCoords {
        origin: ZERO,
        x_axis: FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        y_axis: FVector {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        z_axis: FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    };
    let to_world = coords_mul_rotator(m, &coords_mul_point(&unit, position), angle, angle, 0);

    let v1_local = FVector {
        x: length * 0.5,
        y: width * 0.5,
        z: 0.0,
    };
    let v0_local = FVector {
        x: -length * 0.5,
        y: -width * 0.5,
        z: 0.0,
    };
    let v2_local = FVector {
        x: -length * 0.5,
        y: width * 0.5,
        z: 0.0,
    };

    let v0_world = transform_point_by(v0_local, &to_world);
    let v1_world = transform_point_by(v1_local, &to_world);
    let v2_world = transform_point_by(v2_local, &to_world);

    verts[0].transform.point = transform_point_by(v0_world, coords);
    verts[1].transform.point = transform_point_by(v1_world, coords);
    verts[2].transform.point = transform_point_by(v2_world, coords);

    for v in verts.iter_mut() {
        project(&mut v.transform, frame);
    }

    set_uv(&mut verts[0], tex_usize, tex_vsize, 0.0, 0.0);
    set_uv(&mut verts[1], tex_usize, tex_vsize, 1.0, 0.0);
    set_uv(&mut verts[2], tex_usize, tex_vsize, 1.0, 1.0);

    for v in verts.iter_mut() {
        v.transform.flags = compute_outcode(frame, &v.transform.point);
    }

    combine_clip_codes(&verts[..])
}

/// `LiquidParticles::SetupVerts`. `center` is the C++ `Center` member, which
/// is read and updated in place. Fills four vertices and returns the combined
/// clip codes.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
pub fn liquid_setup_verts<M: RenderMath>(
    m: &M,
    frame: &ParticleFrame,
    coords: &FCoords,
    tex_usize: i32,
    tex_vsize: i32,
    center: &mut FTransform,
    p: &ParticleState,
    verts: &mut [FTransTexture; 4],
) -> u8 {
    center.point = transform_point_by(p.position, coords);
    project(center, frame);

    if p.drip_timer > 0.0 {
        let width_persp_div2 = p.width * 0.5 * center.rz;
        let length_persp_div2 = p.length * 0.5 * center.rz;
        let rz = center.rz;
        let sx = center.screen_x;
        let sy = center.screen_y;

        set_trans(
            &mut verts[0],
            frame,
            tex_usize,
            tex_vsize,
            sx,
            sy,
            rz,
            0.0,
            1.0,
        );
        set_trans(
            &mut verts[2],
            frame,
            tex_usize,
            tex_vsize,
            sx,
            madd(length_persp_div2, 2.0, sy),
            rz,
            1.0,
            0.0,
        );
        set_trans(
            &mut verts[3],
            frame,
            tex_usize,
            tex_vsize,
            sx - width_persp_div2,
            madd(1.5, length_persp_div2, sy),
            rz,
            0.0,
            0.0,
        );
        set_trans(
            &mut verts[1],
            frame,
            tex_usize,
            tex_vsize,
            sx + width_persp_div2,
            madd(1.5, length_persp_div2, sy),
            rz,
            1.0,
            1.0,
        );
    } else {
        screen_vector(m, coords, frame, tex_usize, tex_vsize, p, verts);
    }

    combine_clip_codes(&verts[..])
}

/// `ScreenVector`: a screen-space streaked quad for a non-dripping particle.
/// Writes `pts[0..4]` as the four corners.
#[allow(clippy::approx_constant)] // C literals 3.14159f / 3.1416f, values kept exact
pub fn screen_vector<M: RenderMath>(
    m: &M,
    coords: &FCoords,
    frame: &ParticleFrame,
    tex_usize: i32,
    tex_vsize: i32,
    p: &ParticleState,
    pts: &mut [FTransTexture; 4],
) {
    let new_radius: f32;
    let mut tail_length: f32;
    let streak: FVector;

    if p.drip_timer > 0.0 {
        streak = scale(scale(safe_normal(p.velocity), 0.5), p.length);
        new_radius = p.width * 0.5;
    } else {
        // Volume of a cone with the particle's width as radius.
        let volume = 3.14159f32 / 3.0f32 * p.width * p.width * p.width;

        // Streak is the excess that is greater than the radius of the sphere due to streaking.
        streak = scale(
            scale(safe_normal(p.velocity), size(p.velocity).sqrt()),
            p.length,
        );

        tail_length = p.width + size(streak);

        // r = sqrt((3V) / (H * Pi)) for the cone of height `tail_length`.
        new_radius = ((3.0f32 * volume) / (tail_length * 3.1416f32)).sqrt();
    }

    let mut tail = zero_transform();
    let mut tip = zero_transform();

    tail.point = transform_point_by(sub(p.position, streak), coords);
    project(&mut tail, frame);

    tip.point = transform_point_by(p.position, coords);
    project(&mut tip, frame);

    let displacement = sub(tail.point, tip.point);
    let screen_displacement = size_2d(displacement);

    if screen_displacement <= new_radius {
        // Simply draw a scaled circle.
        tail_length = new_radius;
    } else {
        // Streaked drop.
        let s2d = screen_displacement * screen_displacement;
        let s3d = size_squared(displacement);
        let interpolant = (s2d / s3d).sqrt();
        // Interpolate(interpolant, NewRadius, ScreenDisplacement) = (x1 - x0)*t + x0.
        tail_length = madd(screen_displacement - new_radius, interpolant, new_radius);
    }

    // Direction vector and 2D screen-space magnitude.
    let mut dx = tail.screen_x - tip.screen_x;
    let mut dy = tail.screen_y - tip.screen_y;

    let s_sqr = madd(dx, dx, dy * dy);
    let inverse_s = m.gmath_div_sqrt_approx(s_sqr);
    dx *= inverse_s;
    dy *= inverse_s;

    dx *= tip.rz;
    dy *= tip.rz;

    let x = msub(tip.screen_x, dx, new_radius);
    let y = msub(tip.screen_y, dy, new_radius);
    set_trans(
        &mut pts[1],
        frame,
        tex_usize,
        tex_vsize,
        x,
        y,
        tip.rz,
        0.0,
        1.0,
    );

    let x = madd(dx, tail_length, tip.screen_x);
    let y = madd(dy, tail_length, tip.screen_y);
    set_trans(
        &mut pts[3],
        frame,
        tex_usize,
        tex_vsize,
        x,
        y,
        tip.rz,
        1.0,
        0.0,
    );

    // Swap(dx, dy); dy *= -1. clang folds the -1 into the fused op for y:
    // SY - (-dx)*NR == SY + dx*NR.
    let (dx, dy) = (dy, dx);

    let x = msub(tip.screen_x, dx, new_radius);
    let y = madd(dy, new_radius, tip.screen_y);
    set_trans(
        &mut pts[0],
        frame,
        tex_usize,
        tex_vsize,
        x,
        y,
        tip.rz,
        0.0,
        0.0,
    );

    // clang folds the `dy *= -1` into the fused op: SY + (-dx)*NR == SY - dx*NR.
    let x = madd(dx, new_radius, tip.screen_x);
    let y = msub(tip.screen_y, dy, new_radius);
    set_trans(
        &mut pts[2],
        frame,
        tex_usize,
        tex_vsize,
        x,
        y,
        tip.rz,
        1.0,
        1.0,
    );
}
