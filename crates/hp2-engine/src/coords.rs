//! Port of the `FCoords` operators used by `UMesh`/`ULodMesh` (`UnMath.h`
//! 2272-2476, `FSheerSnap` UnMath.h:54, `FCoords::operator/=(FCoords)` and
//! `FBox << FCoords` in `UnMath.cpp`).
//!
//! Float fidelity: every `FVector` dot product (`TransformPointBy`,
//! `TransformVectorBy`, `FVector::operator|`) uses the fused form from
//! [`prim::dot`]. `*=(FCoords)`, `*=`/`/=(FRotator)`, `*=(FScale)`'s sheer
//! step and `FBox << FCoords` inherit that fusion through the dot.
//! `/=(FScale)` also goes through the sheer dot. The componentwise axis and
//! origin scalings are plain. `/=(FCoords)` compiles to plain multiply-add in
//! the reference build (checked against the disassembly of the `UnMath.cpp`
//! TU), so it stays plain. Every operation keeps the C++ operand order.

use hp2_abi::{FCoords, FVector};

use crate::prim::{self, Aabb};

/// `ESheerAxis` (`UnMath.h:1138`).
pub const SHEER_NONE: i32 = 0;
pub const SHEER_XY: i32 = 1;
pub const SHEER_XZ: i32 = 2;
pub const SHEER_YX: i32 = 3;
pub const SHEER_YZ: i32 = 4;
pub const SHEER_ZX: i32 = 5;
pub const SHEER_ZY: i32 = 6;

/// `FRotator` (pitch, yaw, roll) as in `hp2_abi::engine::generated`.
pub use hp2_abi::engine::generated::FRotator;

/// `FScale` (`UnMath.h`): per-axis scale, sheer rate and sheer axis.
#[derive(Clone, Copy, Debug)]
pub struct FScale {
    pub scale: FVector,
    pub sheer_rate: f32,
    pub sheer_axis: i32,
}

/// Trig callbacks for `GMath.SinTab` and `GMath.CosTab`.
pub struct Trig<'a> {
    pub sin: &'a dyn Fn(i32) -> f32,
    pub cos: &'a dyn Fn(i32) -> f32,
}

/// `GMath.UnitCoords`: the identity frame.
pub fn unit_coords() -> FCoords {
    FCoords {
        origin: prim::v3(0.0, 0.0, 0.0),
        x_axis: prim::v3(1.0, 0.0, 0.0),
        y_axis: prim::v3(0.0, 1.0, 0.0),
        z_axis: prim::v3(0.0, 0.0, 1.0),
    }
}

/// `FCoords(const FVector& InOrigin)`: identity axes at `origin`.
pub fn coords_from_origin(origin: FVector) -> FCoords {
    FCoords {
        origin,
        ..unit_coords()
    }
}

/// `FSheerSnap` (`UnMath.h:54`).
pub fn sheer_snap(sheer: f32) -> f32 {
    if sheer < -0.65 {
        sheer + 0.15
    } else if sheer > 0.65 {
        sheer - 0.15
    } else if sheer < -0.55 {
        -0.50
    } else if sheer > 0.55 {
        0.50
    } else if sheer < -0.05 {
        sheer + 0.05
    } else if sheer > 0.05 {
        sheer - 0.05
    } else {
        0.0
    }
}

/// `FCoords::operator*=(const FCoords&)` (`UnMath.h:2272`).
pub fn mul_coords(c: &mut FCoords, t: &FCoords) {
    let origin = prim::transform_point(c.origin, t);
    let x = prim::transform_vector(c.x_axis, t);
    let y = prim::transform_vector(c.y_axis, t);
    let z = prim::transform_vector(c.z_axis, t);
    c.origin = origin;
    c.x_axis = x;
    c.y_axis = y;
    c.z_axis = z;
}

/// `FCoords::operator/=(const FCoords&)` (`UnMath.cpp:145`).
/// `XAxis = XAxis.X*T.XAxis + XAxis.Y*T.YAxis + XAxis.Z*T.ZAxis` with plain
/// multiply and add. The origin also receives `+ T.Origin` last.
pub fn div_coords(c: &mut FCoords, t: &FCoords) {
    let mix = |v: FVector| -> FVector {
        prim::v3(
            v.x * t.x_axis.x + v.y * t.y_axis.x + v.z * t.z_axis.x,
            v.x * t.x_axis.y + v.y * t.y_axis.y + v.z * t.z_axis.y,
            v.x * t.x_axis.z + v.y * t.y_axis.z + v.z * t.z_axis.z,
        )
    };
    let origin = prim::add(mix(c.origin), t.origin);
    let x = mix(c.x_axis);
    let y = mix(c.y_axis);
    let z = mix(c.z_axis);
    c.origin = origin;
    c.x_axis = x;
    c.y_axis = y;
    c.z_axis = z;
}

/// `FCoords::operator*=(const FVector&)`: `Origin -= Point`.
pub fn mul_vec(c: &mut FCoords, p: FVector) {
    c.origin = prim::sub(c.origin, p);
}

/// `FCoords::operator/=(const FVector&)` (`FCoords << FVector`): `Origin += Point`.
pub fn div_vec(c: &mut FCoords, p: FVector) {
    c.origin = prim::add(c.origin, p);
}

/// `FCoords::operator*=(const FRotator&)` (`UnMath.h:2296`): yaw, pitch, roll.
pub fn mul_rot(c: &mut FCoords, rot: FRotator, trig: &Trig) {
    let (cy, sy) = ((trig.cos)(rot.yaw), (trig.sin)(rot.yaw));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(cy, sy, 0.0),
            y_axis: prim::v3(-sy, cy, 0.0),
            z_axis: prim::v3(0.0, 0.0, 1.0),
        },
    );

    let (cp, sp) = ((trig.cos)(rot.pitch), (trig.sin)(rot.pitch));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(cp, 0.0, sp),
            y_axis: prim::v3(0.0, 1.0, 0.0),
            z_axis: prim::v3(-sp, 0.0, cp),
        },
    );

    let (cr, sr) = ((trig.cos)(rot.roll), (trig.sin)(rot.roll));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(1.0, 0.0, 0.0),
            y_axis: prim::v3(0.0, cr, -sr),
            z_axis: prim::v3(0.0, sr, cr),
        },
    );
}

/// `FCoords::operator/=(const FRotator&)` (`UnMath.h:2344`): inverse roll,
/// pitch, yaw. The literal `-0.f` entries are kept so signed zeros match.
pub fn div_rot(c: &mut FCoords, rot: FRotator, trig: &Trig) {
    let (cr, sr) = ((trig.cos)(rot.roll), (trig.sin)(rot.roll));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(1.0, -0.0, 0.0),
            y_axis: prim::v3(-0.0, cr, sr),
            z_axis: prim::v3(0.0, -sr, cr),
        },
    );

    let (cp, sp) = ((trig.cos)(rot.pitch), (trig.sin)(rot.pitch));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(cp, 0.0, -sp),
            y_axis: prim::v3(0.0, 1.0, -0.0),
            z_axis: prim::v3(sp, 0.0, cp),
        },
    );

    let (cy, sy) = ((trig.cos)(rot.yaw), (trig.sin)(rot.yaw));
    mul_coords(
        c,
        &FCoords {
            origin: prim::v3(0.0, 0.0, 0.0),
            x_axis: prim::v3(cy, -sy, -0.0),
            y_axis: prim::v3(sy, cy, 0.0),
            z_axis: prim::v3(-0.0, 0.0, 1.0),
        },
    );
}

/// `FCoords::operator*=(const FScale&)` (`UnMath.h:2394`).
pub fn mul_scale(c: &mut FCoords, s: &FScale) {
    let sheer = sheer_snap(s.sheer_rate);
    let mut temp = unit_coords();
    match s.sheer_axis {
        SHEER_XY => temp.x_axis.y = sheer,
        SHEER_XZ => temp.x_axis.z = sheer,
        SHEER_YX => temp.y_axis.x = sheer,
        SHEER_YZ => temp.y_axis.z = sheer,
        SHEER_ZX => temp.z_axis.x = sheer,
        SHEER_ZY => temp.z_axis.y = sheer,
        _ => {}
    }
    mul_coords(c, &temp);

    c.x_axis = mul_comp(c.x_axis, s.scale);
    c.y_axis = mul_comp(c.y_axis, s.scale);
    c.z_axis = mul_comp(c.z_axis, s.scale);
    c.origin.x /= s.scale.x;
    c.origin.y /= s.scale.y;
    c.origin.z /= s.scale.z;
}

/// `FCoords::operator/=(const FScale&)` (`UnMath.h:2442`): the inverse of
/// [`mul_scale`], with the sheer negated.
pub fn div_scale(c: &mut FCoords, s: &FScale) {
    c.x_axis = div_comp(c.x_axis, s.scale);
    c.y_axis = div_comp(c.y_axis, s.scale);
    c.z_axis = div_comp(c.z_axis, s.scale);
    c.origin.x *= s.scale.x;
    c.origin.y *= s.scale.y;
    c.origin.z *= s.scale.z;

    let mut temp = unit_coords();
    let sheer = sheer_snap(s.sheer_rate);
    match s.sheer_axis {
        SHEER_XY => temp.x_axis.y = -sheer,
        SHEER_XZ => temp.x_axis.z = -sheer,
        SHEER_YX => temp.y_axis.x = -sheer,
        SHEER_YZ => temp.y_axis.z = -sheer,
        SHEER_ZX => temp.z_axis.x = -sheer,
        SHEER_ZY => temp.z_axis.y = -sheer,
        _ => {}
    }
    mul_coords(c, &temp);
}

/// `FVector << FCoords` (`UnMath.h`): `V.X*C.XAxis.X + V.Y*C.YAxis.X + V.Z*C.ZAxis.X + C.Origin.X`
/// per component, with the dot fused as in [`prim::dot`], then the origin added plainly.
pub fn vec_times_coords(v: FVector, c: &FCoords) -> FVector {
    prim::add(
        prim::v3(
            prim::dot(v, prim::v3(c.x_axis.x, c.y_axis.x, c.z_axis.x)),
            prim::dot(v, prim::v3(c.x_axis.y, c.y_axis.y, c.z_axis.y)),
            prim::dot(v, prim::v3(c.x_axis.z, c.y_axis.z, c.z_axis.z)),
        ),
        c.origin,
    )
}

/// `FBox << FCoords` (`UnMath.cpp:246`):
/// `FCoords(Box.Min << C, C.XAxis*(Max.X-Min.X), C.YAxis*(Max.Y-Min.Y), C.ZAxis*(Max.Z-Min.Z))`.
pub fn box_times_coords(b: &Aabb, c: &FCoords) -> FCoords {
    let ext = prim::sub(b.max, b.min);
    FCoords {
        origin: vec_times_coords(b.min, c),
        x_axis: prim::scale(c.x_axis, ext.x),
        y_axis: prim::scale(c.y_axis, ext.y),
        z_axis: prim::scale(c.z_axis, ext.z),
    }
}

/// `FVector *= FVector` / `operator*` (componentwise).
pub(crate) fn mul_comp(a: FVector, b: FVector) -> FVector {
    prim::v3(a.x * b.x, a.y * b.y, a.z * b.z)
}

/// `FVector /= FVector` (componentwise).
fn div_comp(a: FVector, b: FVector) -> FVector {
    prim::v3(a.x / b.x, a.y / b.y, a.z / b.z)
}
