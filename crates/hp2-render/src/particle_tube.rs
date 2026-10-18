//! Port of `TriTubeParticles::SetupVerts` and `PolyStripParticles::SetupVerts`
//! (`Render/Src/UnParticleRn.cpp`) and the engine math they call: `FVector`
//! `^`/`|`, `FVector::Rotation`, `FCoords` multiplies by a point, vector, and
//! rotator, `FVector::DivSizeApprox`, and `FTransform::Project`.
//!
//! Trig and reciprocal-sqrt come from `GMath` through `Hp2RenderServices`
//! (`sin_tab`, `cos_tab`, `div_sqrt_approx`); no table is copied here.
//! Float contraction follows `particle.rs`: each `a*b + c` inside one engine
//! expression is `madd`, which is one `fma` on arm64 under `-ffp-contract=on`.
//! Output goes to a caller-owned `FTransTexture` buffer (12 entries for
//! TriTube, 4 for PolyStrip). The C++ side keeps the previous-particle state and
//! passes it in.

use hp2_abi::render_particle::{FTransTexture, FTransform};
use hp2_abi::{FCoords, FVector};

use crate::particle::{
    FVF_OUT_ALL, ParticleFrame, combine_clip_codes, compute_outcode, madd, set_uv,
};
use crate::services::Hp2RenderServices;
use crate::sprite::size_squared;

/// `appAtan2`/`appSqrt` operate in double; `PI` is the engine's `FLOAT` constant.
#[allow(clippy::approx_constant, clippy::excessive_precision)] // C literal, value kept exact
const PI: f32 = 3.141_592_653_589_793_2_f32;

/// One particle as the particle-tube math reads it (`UParticle` position,
/// `Width`, and `Length`). The C++ side fills it from the live particle.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TubeParticle {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub width: f32,
    pub length: f32,
}

impl TubeParticle {
    fn pos(&self) -> FVector {
        FVector {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

#[inline]
fn add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
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

/// `FVector::operator|`: `X*V.X + Y*V.Y + Z*V.Z`, contracted as in `sprite.rs`.
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    madd(a.z, b.z, madd(a.x, b.x, a.y * b.y))
}

/// `FVector::operator^`: the three components are contracted as
/// `madd(a, b, -(c*d))`, matching the reference build's disassembly.
#[inline]
fn cross(a: FVector, b: FVector) -> FVector {
    FVector {
        x: madd(a.y, b.z, -(a.z * b.y)),
        y: madd(a.z, b.x, -(a.x * b.z)),
        z: madd(a.x, b.y, -(a.y * b.x)),
    }
}

/// `FVector::DivSizeApprox`: `GMath.DivSqrtApprox(SizeSquared() + 1e-12f)`.
#[inline]
fn div_size_approx(svc: &Hp2RenderServices, v: FVector) -> f32 {
    // SAFETY: the services table is supplied by the C++ side for the call.
    unsafe { (svc.gmath_div_sqrt_approx)(size_squared(v) + 1e-12_f32) }
}

/// `V *= V.DivSizeApprox()`: the scale is taken from the value before the multiply.
#[inline]
fn scale_div_approx(svc: &Hp2RenderServices, v: FVector) -> FVector {
    scale(v, div_size_approx(svc, v))
}

/// `FVector::Rotation()` (`Core/Src/UnMath.cpp`). Both angles use double
/// `appAtan2`/`appSqrt`; `2.f*PI` is a float constant widened to double, and
/// `(FLOAT)MAXWORD` is 65535. `Roll` is always 0. `as i32` truncates and
/// saturates, as the arm64 `fcvtzs` does.
pub fn rotation(v: FVector) -> (i32, i32, i32) {
    let two_pi = f64::from(2.0_f32 * PI);
    let maxword = f64::from(65535.0_f32);
    let yaw = (f64::from(v.y).atan2(f64::from(v.x)) * maxword / two_pi) as i32;
    let xy = madd(v.x, v.x, v.y * v.y);
    let pitch = (f64::from(v.z).atan2(f64::from(xy).sqrt()) * maxword / two_pi) as i32;
    (yaw, pitch, 0)
}

/// `FVector::TransformPointBy(FCoords)`: `Temp = *this - Origin`, then the dots.
#[inline]
fn transform_point_by(p: FVector, c: &FCoords) -> FVector {
    let temp = sub(p, c.origin);
    FVector {
        x: dot(temp, c.x_axis),
        y: dot(temp, c.y_axis),
        z: dot(temp, c.z_axis),
    }
}

/// `FVector::TransformVectorBy(FCoords)`: the dots without the origin shift.
#[inline]
fn transform_vector_by(v: FVector, c: &FCoords) -> FVector {
    FVector {
        x: dot(v, c.x_axis),
        y: dot(v, c.y_axis),
        z: dot(v, c.z_axis),
    }
}

/// `FCoords::operator*=(FCoords)`: the origin and each axis are transformed
/// by `t`. Each output reads only its own input, so the order does not matter.
fn coords_mul_coords(c: &FCoords, t: &FCoords) -> FCoords {
    FCoords {
        origin: transform_point_by(c.origin, t),
        x_axis: transform_vector_by(c.x_axis, t),
        y_axis: transform_vector_by(c.y_axis, t),
        z_axis: transform_vector_by(c.z_axis, t),
    }
}

/// `FCoords::operator*=(FVector)`: `Origin -= Point`, so the origin is `0.0 - p`
/// when the coords start at `UnitCoords`.
fn coords_mul_point(c: &FCoords, p: FVector) -> FCoords {
    FCoords {
        origin: sub(c.origin, p),
        x_axis: c.x_axis,
        y_axis: c.y_axis,
        z_axis: c.z_axis,
    }
}

/// `FCoords::operator*=(FRotator)`: yaw, then pitch, then roll. Each step is
/// `*this *= FCoords(0, X, Y, Z)` with the `GMath` trig values, as in `UnMath.h`.
fn coords_mul_rotator(
    svc: &Hp2RenderServices,
    c: &FCoords,
    yaw: i32,
    pitch: i32,
    roll: i32,
) -> FCoords {
    let origin = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let (sin, cos) = (svc.sin_tab, svc.cos_tab);

    let (sy, cy) = unsafe { (sin(yaw), cos(yaw)) };
    let c = coords_mul_coords(
        c,
        &FCoords {
            origin,
            x_axis: FVector {
                x: cy,
                y: sy,
                z: 0.0,
            },
            y_axis: FVector {
                x: -sy,
                y: cy,
                z: 0.0,
            },
            z_axis: FVector {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        },
    );

    let (sp, cp) = unsafe { (sin(pitch), cos(pitch)) };
    let c = coords_mul_coords(
        &c,
        &FCoords {
            origin,
            x_axis: FVector {
                x: cp,
                y: 0.0,
                z: sp,
            },
            y_axis: FVector {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            z_axis: FVector {
                x: -sp,
                y: 0.0,
                z: cp,
            },
        },
    );

    let (sr, cr) = unsafe { (sin(roll), cos(roll)) };
    coords_mul_coords(
        &c,
        &FCoords {
            origin,
            x_axis: FVector {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            y_axis: FVector {
                x: 0.0,
                y: cr,
                z: -sr,
            },
            z_axis: FVector {
                x: 0.0,
                y: sr,
                z: cr,
            },
        },
    )
}

/// `GMath.UnitCoords`: origin 0, identity axes.
fn unit_coords() -> FCoords {
    FCoords {
        origin: FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
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
    }
}

/// `FTransform::Project(Frame)`: `RZ = Proj.Z / Point.Z`, then the screen
/// coordinates as `madd(p, rz, fx15)`. `IntY = appFloor(ScreenY)` is
/// `static_cast<INT>` on the reference build, which truncates.
#[inline]
fn project(frame: &ParticleFrame, t: &mut FTransform) {
    t.rz = frame.proj_z / t.point.z;
    t.screen_x = madd(t.point.x, t.rz, frame.fx15);
    t.screen_y = madd(t.point.y, t.rz, frame.fy15);
    t.int_y = t.screen_y as i32;
}

/// `verts[i].Point = World.TransformPointBy(Frame->Coords); verts[i].Project(Frame)`.
#[inline]
fn place(frame: &ParticleFrame, coords: &FCoords, world: FVector, tt: &mut FTransTexture) {
    tt.transform.point = transform_point_by(world, coords);
    project(frame, &mut tt.transform);
}

/// `ComputeClipCodes(Frame, tt)`: the flags are assigned, not OR-ed.
#[inline]
fn clip_code(frame: &ParticleFrame, tt: &mut FTransTexture) {
    tt.transform.flags = compute_outcode(frame, &tt.transform.point);
}

/// `TriTubeParticles::SetupVerts`. `prev` is `None` on the first call (the C++
/// `!pPrevParticle` branch). That branch sets `prev_strip_vect` from the next
/// particle, returns 0, and leaves `verts` untouched. Otherwise the 12 verts
/// (indices 0..=11, shared corners duplicated as in the C++) are filled and the
/// combined clip byte is returned. `prev_strip_vect` is updated to the current
/// strip vector.
#[allow(clippy::too_many_arguments)]
pub fn tritube_setup_verts(
    svc: &Hp2RenderServices,
    frame: &ParticleFrame,
    frame_coords: &FCoords,
    tex_usize: i32,
    tex_vsize: i32,
    prev: Option<&TubeParticle>,
    curr: &TubeParticle,
    next: Option<&TubeParticle>,
    prev_strip_vect: &mut FVector,
    verts: &mut [FTransTexture; 12],
) -> u8 {
    let Some(prev) = prev else {
        if let Some(next) = next {
            *prev_strip_vect = sub(next.pos(), curr.pos());
            *prev_strip_vect = scale_div_approx(svc, *prev_strip_vect);
        }
        return 0;
    };

    // to_world = UnitCoords * prev.Position * PrevStripVect.Rotation()
    let (yaw, pitch, roll) = rotation(*prev_strip_vect);
    let to_world = coords_mul_rotator(
        svc,
        &coords_mul_point(&unit_coords(), prev.pos()),
        yaw,
        pitch,
        roll,
    );

    let pw = prev.width;
    let pl = prev.length;
    let v0_local = FVector {
        x: 0.0,
        y: -pw * 0.5,
        z: -pl * 0.5,
    };
    let v3_local = FVector {
        x: 0.0,
        y: 0.0,
        z: pl * 0.5,
    };
    let v7_local = FVector {
        x: 0.0,
        y: pw * 0.5,
        z: -pl * 0.5,
    };
    let v0_world = transform_point_by(v0_local, &to_world);
    let v3_world = transform_point_by(v3_local, &to_world);
    let v7_world = transform_point_by(v7_local, &to_world);

    place(frame, frame_coords, v0_world, &mut verts[0]);
    place(frame, frame_coords, v3_world, &mut verts[3]);
    place(frame, frame_coords, v7_world, &mut verts[7]);
    verts[11] = verts[0];

    // StripVect: the next particle orients the strip, else keep the last one.
    let strip_vect = match next {
        Some(next) => scale_div_approx(svc, sub(next.pos(), curr.pos())),
        None => *prev_strip_vect,
    };

    // to_world = UnitCoords * curr.Position * StripVect.Rotation()
    let (yaw, pitch, roll) = rotation(strip_vect);
    let to_world = coords_mul_rotator(
        svc,
        &coords_mul_point(&unit_coords(), curr.pos()),
        yaw,
        pitch,
        roll,
    );

    let cw = curr.width;
    let cl = curr.length;
    let v1_local = FVector {
        x: 0.0,
        y: -cw * 0.5,
        z: -cl * 0.5,
    };
    let v2_local = FVector {
        x: 0.0,
        y: 0.0,
        z: cl * 0.5,
    };
    let v6_local = FVector {
        x: 0.0,
        y: cw * 0.5,
        z: -cl * 0.5,
    };
    let v1_world = transform_point_by(v1_local, &to_world);
    let v2_world = transform_point_by(v2_local, &to_world);
    let v6_world = transform_point_by(v6_local, &to_world);

    place(frame, frame_coords, v1_world, &mut verts[1]);
    place(frame, frame_coords, v2_world, &mut verts[2]);
    place(frame, frame_coords, v6_world, &mut verts[6]);
    verts[10] = verts[1];

    // Back triangle, then front triangle.
    set_uv(&mut verts[0], tex_usize, tex_vsize, 0.00, 0.00);
    set_uv(&mut verts[3], tex_usize, tex_vsize, 0.33, 0.00);
    set_uv(&mut verts[7], tex_usize, tex_vsize, 0.66, 0.00);
    set_uv(&mut verts[11], tex_usize, tex_vsize, 1.00, 0.00);
    set_uv(&mut verts[1], tex_usize, tex_vsize, 0.00, 1.0);
    set_uv(&mut verts[2], tex_usize, tex_vsize, 0.33, 1.0);
    set_uv(&mut verts[6], tex_usize, tex_vsize, 0.66, 1.0);
    set_uv(&mut verts[10], tex_usize, tex_vsize, 1.00, 1.0);

    for i in [0, 3, 7, 11, 1, 2, 6, 10] {
        clip_code(frame, &mut verts[i]);
    }

    // Exact duplicates: 4<-3, 8<-7, 5<-2, 9<-6.
    verts[4] = verts[3];
    verts[8] = verts[7];
    verts[5] = verts[2];
    verts[9] = verts[6];

    // Back-face culls on the world-space triangles.
    let camera = scale_div_approx(svc, sub(curr.pos(), frame_coords.origin));

    let cull = |side: FVector, top: FVector| -> bool {
        let surface = scale_div_approx(svc, cross(top, side));
        dot(surface, camera) <= 0.0
    };

    if cull(sub(v3_world, v0_world), sub(v0_world, v1_world)) {
        for v in &mut verts[0..4] {
            v.transform.flags = FVF_OUT_ALL;
        }
    }
    if cull(sub(v7_world, v3_world), sub(v3_world, v2_world)) {
        for v in &mut verts[4..8] {
            v.transform.flags = FVF_OUT_ALL;
        }
    }
    if cull(sub(v0_world, v7_world), sub(v7_world, v6_world)) {
        for v in &mut verts[8..12] {
            v.transform.flags = FVF_OUT_ALL;
        }
    }

    *prev_strip_vect = strip_vect;
    combine_clip_codes(&verts[..])
}

/// `PolyStripParticles::SetupVerts`. `prev` is `None` on the first call. That
/// branch sets `prev_strip_vect` and `prev_edge_vect` from the next particle,
/// returns 0, and leaves `verts` untouched. Otherwise the 4 verts are filled and
/// the combined clip byte is returned. Both state vectors are updated.
#[allow(clippy::too_many_arguments)]
pub fn poly_strip_setup_verts(
    svc: &Hp2RenderServices,
    frame: &ParticleFrame,
    frame_coords: &FCoords,
    tex_usize: i32,
    tex_vsize: i32,
    prev: Option<&TubeParticle>,
    curr: &TubeParticle,
    next: Option<&TubeParticle>,
    prev_strip_vect: &mut FVector,
    prev_edge_vect: &mut FVector,
    verts: &mut [FTransTexture; 4],
) -> u8 {
    let Some(prev) = prev else {
        if let Some(next) = next {
            // Vector from camera to the current particle.
            let look = scale_div_approx(svc, sub(curr.pos(), frame_coords.origin));
            *prev_strip_vect = sub(next.pos(), curr.pos());
            *prev_strip_vect = scale_div_approx(svc, *prev_strip_vect);
            *prev_edge_vect = scale_div_approx(svc, cross(look, *prev_strip_vect));
        }
        return 0;
    };

    let edge_prev = scale(*prev_edge_vect, prev.width);
    let mut strip_vect = *prev_strip_vect;

    let mut edges = [FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; 4];
    edges[0] = sub(prev.pos(), edge_prev);
    edges[3] = add(prev.pos(), edge_prev);

    if let Some(next) = next {
        strip_vect = scale_div_approx(svc, sub(next.pos(), curr.pos()));
    }

    let look = scale_div_approx(svc, sub(curr.pos(), frame_coords.origin));
    let mut edge_vect = scale_div_approx(svc, cross(look, strip_vect));

    if dot(edge_vect, *prev_edge_vect) < 0.0 {
        edge_vect = scale(edge_vect, -1.0);
    }

    edges[2] = curr.pos();
    edges[1] = curr.pos();
    edges[2] = add(edges[2], scale(edge_vect, curr.width));
    edges[1] = sub(edges[1], scale(edge_vect, curr.width));

    for (vert, edge) in verts.iter_mut().zip(edges.iter()) {
        place(frame, frame_coords, *edge, vert);
    }

    set_uv(&mut verts[0], tex_usize, tex_vsize, 0.0, 0.0);
    set_uv(&mut verts[2], tex_usize, tex_vsize, 1.0, 1.0);
    set_uv(&mut verts[3], tex_usize, tex_vsize, 1.0, 0.0);
    set_uv(&mut verts[1], tex_usize, tex_vsize, 0.0, 1.0);
    for i in [0, 2, 3, 1] {
        clip_code(frame, &mut verts[i]);
    }

    *prev_strip_vect = strip_vect;
    *prev_edge_vect = edge_vect;
    combine_clip_codes(&verts[..])
}
