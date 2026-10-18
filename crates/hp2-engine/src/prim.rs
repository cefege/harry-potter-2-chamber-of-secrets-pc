//! Port of `Engine/Src/UnPrim.cpp`: cylinder, oval, oriented, box and FBox
//! collision checks, plus `FCheckResult::BspNode` / `BspSurf`.
//!
//! The C++ shells keep class identity (`Cast`, `IsA`, `XLevel`) and pass
//! plain values here. Owner state is a value copy, so the aligned-oval checks
//! mutate a local copy instead of `AActor` in place. Every check returns the
//! C++ `UBOOL` meaning as `bool`: `true` means "no hit" (C++ `1`).

use std::ffi::c_void;

use hp2_abi::engine::generated::FCheckResult;
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert};
use hp2_abi::{FCoords, FPlane, FVector};

use crate::math::app_sqrt;

/// C++ `INDEX_NONE`.
pub const INDEX_NONE: i32 = -1;

#[inline]
pub(crate) fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

#[inline]
pub(crate) fn add(a: FVector, b: FVector) -> FVector {
    v3(a.x + b.x, a.y + b.y, a.z + b.z)
}

#[inline]
pub(crate) fn sub(a: FVector, b: FVector) -> FVector {
    v3(a.x - b.x, a.y - b.y, a.z - b.z)
}

#[inline]
pub(crate) fn neg(a: FVector) -> FVector {
    v3(-a.x, -a.y, -a.z)
}

/// `FVector * FLOAT`.
#[inline]
pub(crate) fn scale(a: FVector, s: f32) -> FVector {
    v3(a.x * s, a.y * s, a.z * s)
}

/// `FVector * FVector(1,1,0)` (C++ `operator*`, componentwise).
#[inline]
pub(crate) fn flatten_xy(a: FVector) -> FVector {
    v3(a.x * 1.0, a.y * 1.0, a.z * 0.0)
}

/// `FVector | FVector`. Left-fused like `vec::size_squared`.
#[inline]
pub(crate) fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `FVector ^ FVector`.
#[inline]
pub(crate) fn cross(a: FVector, b: FVector) -> FVector {
    v3(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

/// `Square(a) + Square(b)`, left-fused.
#[inline]
fn sq_sum(a: f32, b: f32) -> f32 {
    a.mul_add(a, b * b)
}

#[inline]
fn sq(a: f32) -> f32 {
    a * a
}

/// C++ `Min(A,B)` (`UTemplate.h`).
#[inline]
pub(crate) fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// C++ `Max(A,B)` (`UTemplate.h`).
#[inline]
pub(crate) fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// C++ `Clamp(X,Min,Max)` (`UTemplate.h`).
#[inline]
pub(crate) fn clamp_f(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

/// `FVector::SafeNormal` (`UnMath.cpp:259`).
pub(crate) fn safe_normal(a: FVector) -> FVector {
    let sq_len = dot(a, a);
    if sq_len < 1.0e-8 {
        return v3(0.0, 0.0, 0.0);
    }
    let size = app_sqrt(sq_len as f64) as f32;
    let s = 1.0f32 / size;
    scale(a, s)
}

/// `FVector::Normalize` (`UnMath.h:800`). Returns `false` if too small.
pub(crate) fn normalize(a: &mut FVector) -> bool {
    let sq_len = dot(*a, *a);
    if sq_len >= 1.0e-8 {
        // C++: `1.f / appSqrt(...)` is computed in double, then stored as FLOAT.
        let s = (1.0f64 / app_sqrt(sq_len as f64)) as f32;
        *a = scale(*a, s);
        true
    } else {
        false
    }
}

/// `FPlane::PlaneDot`: `X*P.X + Y*P.Y + Z*P.Z - W`.
#[inline]
pub(crate) fn plane_dot(p: FPlane, x: FVector) -> f32 {
    dot(v3(p.x, p.y, p.z), x) - p.w
}

/// `TransformPointBy(C)`: `(this - C.Origin) | axes`.
pub(crate) fn transform_point(p: FVector, c: &FCoords) -> FVector {
    let t = sub(p, c.origin);
    v3(dot(t, c.x_axis), dot(t, c.y_axis), dot(t, c.z_axis))
}

/// `TransformVectorBy(C)`: `this | axes`.
pub(crate) fn transform_vector(p: FVector, c: &FCoords) -> FVector {
    v3(dot(p, c.x_axis), dot(p, c.y_axis), dot(p, c.z_axis))
}

/// `FCoords::Transpose()`.
pub(crate) fn transpose(c: &FCoords) -> FCoords {
    FCoords {
        origin: neg(transform_vector(c.origin, c)),
        x_axis: v3(c.x_axis.x, c.y_axis.x, c.z_axis.x),
        y_axis: v3(c.x_axis.y, c.y_axis.y, c.z_axis.y),
        z_axis: v3(c.x_axis.z, c.y_axis.z, c.z_axis.z),
    }
}

/// `FPlane::TransformPlaneByOrtho(C)` (`UnMath.h:2212`).
pub(crate) fn transform_plane_by_ortho(p: FPlane, c: &FCoords) -> FPlane {
    let n = v3(p.x, p.y, p.z);
    let nn = transform_vector(n, c);
    let w = p.w - dot(transform_vector(c.origin, c), nn);
    FPlane {
        x: nn.x,
        y: nn.y,
        z: nn.z,
        w,
    }
}

/// C++ `FBox`, valid flag included. Pure Rust so it can be built freely;
/// the generated `FBox` mirror has private padding.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: FVector,
    pub max: FVector,
    pub valid: bool,
}

impl Aabb {
    /// `FBox(INT)`: zero and invalid.
    pub fn empty() -> Self {
        Aabb {
            min: v3(0.0, 0.0, 0.0),
            max: v3(0.0, 0.0, 0.0),
            valid: false,
        }
    }

    /// `FBox(const FVector& InExt)`: `(-InExt, InExt)`, valid.
    pub fn from_extent(ext: FVector) -> Self {
        Aabb {
            min: neg(ext),
            max: ext,
            valid: true,
        }
    }

    /// `FBox(Min, Max)`, valid.
    pub fn from_min_max(min: FVector, max: FVector) -> Self {
        Aabb {
            min,
            max,
            valid: true,
        }
    }

    /// `FBox::operator+=(FVector)`.
    pub fn add_point(&mut self, p: FVector) {
        if self.valid {
            self.min = v3(
                min_f(self.min.x, p.x),
                min_f(self.min.y, p.y),
                min_f(self.min.z, p.z),
            );
            self.max = v3(
                max_f(self.max.x, p.x),
                max_f(self.max.y, p.y),
                max_f(self.max.z, p.z),
            );
        } else {
            self.min = p;
            self.max = p;
            self.valid = true;
        }
    }

    /// `FBox::operator+=(FBox)`: merges only when `other` is valid.
    pub fn add_box(&mut self, other: &Aabb) {
        if !other.valid {
            return;
        }
        if self.valid {
            self.min = v3(
                min_f(self.min.x, other.min.x),
                min_f(self.min.y, other.min.y),
                min_f(self.min.z, other.min.z),
            );
            self.max = v3(
                max_f(self.max.x, other.max.x),
                max_f(self.max.y, other.max.y),
                max_f(self.max.z, other.max.z),
            );
        } else {
            *self = *other;
        }
    }

    /// `FBox::TransformBy(C)`: transforms the 8 corners and re-boxes them.
    pub fn transform_by(&self, c: &FCoords) -> Aabb {
        let mut out = Aabb::empty();
        let ext = |i: usize| if i == 0 { self.min } else { self.max };
        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    let p = v3(ext(i).x, ext(j).y, ext(k).z);
                    out.add_point(transform_point(p, c));
                }
            }
        }
        out
    }
}

/// `TransformExtent(V, C)` (`UnPrim.cpp:271`).
pub(crate) fn transform_extent(v: FVector, c: &FCoords) -> FVector {
    let b = Aabb::from_extent(v).transform_by(c);
    scale(sub(b.max, b.min), 0.5)
}

/// `bIntersects(A, B, tol)` (`UnPrim.cpp:545`).
pub(crate) fn b_intersects(a: &Aabb, b: &Aabb, tol: f32) -> bool {
    a.min.x + tol < b.max.x
        && a.max.x - tol > b.min.x
        && a.min.y + tol < b.max.y
        && a.max.y - tol > b.min.y
        && a.min.z + tol < b.max.z
        && a.max.z - tol > b.min.z
}

/// `Contains(Box, V, tol)` (`UnPrim.cpp:638`).
pub(crate) fn contains(b: &Aabb, v: FVector, tol: f32) -> bool {
    v.x > b.min.x + tol
        && v.x < b.max.x - tol
        && v.y > b.min.y + tol
        && v.y < b.max.y - tol
        && v.z > b.min.z + tol
        && v.z < b.max.z - tol
}

/// `FBoxPushOut(Normal, Size)` (`UnMath.h:2494`).
pub(crate) fn box_push_out(n: FVector, s: FVector) -> f32 {
    (n.x * s.x).abs() + (n.y * s.y).abs() + (n.z * s.z).abs()
}

/// Owner (`AActor`) state read by the collision checks. Plain values, so no
/// raw pointer is dereferenced in Rust. `actor` is only written back to
/// `FCheckResult::Actor`.
#[derive(Clone, Copy, Debug)]
pub struct Owner {
    pub actor: *mut c_void,
    pub location: FVector,
    pub collision_radius: f32,
    pub collision_width: f32,
    pub collision_height: f32,
    /// `ToLocal()`.
    pub to_local: FCoords,
    /// `ToWorld()`.
    pub to_world: FCoords,
    /// `GetCylinderExtent()` with the real `CollisionWidth`.
    pub cyl_extent: FVector,
    /// `GetCylinderExtent()` with `CollisionWidth == 0` (oval checks).
    pub cyl_extent_zero_width: FVector,
}

/// C++ `FCheck`: a point or segment with an extent and an optional owner.
#[derive(Clone, Copy, Debug)]
pub struct FCheck {
    pub end: FVector,
    pub start: FVector,
    pub extent: FVector,
    pub owner: Option<Owner>,
}

impl FCheck {
    /// `FCheck(Start, Extent, Owner)`: a point check (`End = Start`).
    pub fn point(start: FVector, extent: FVector, owner: Option<Owner>) -> Self {
        FCheck {
            end: start,
            start,
            extent,
            owner,
        }
    }
}

/// `FCheckResult()` with the same bit pattern as a zeroed C++ object.
/// Fields are plain data and raw pointers, so all-zero is valid.
pub fn empty_result() -> FCheckResult {
    // SAFETY: `FCheckResult` holds only f32, i32 and raw-pointer fields.
    unsafe { std::mem::zeroed() }
}

/// `UPrimitive::GetCollisionExtent` (cylinder): `(Radius, Radius, Height)`.
pub fn cyl_extent(owner: &Owner) -> FVector {
    v3(
        owner.collision_radius,
        owner.collision_radius,
        owner.collision_height,
    )
}

/// `UPrimitive::PointCheck` (cylinder). Returns `true` on no hit.
pub fn cyl_point_check(
    res: &mut FCheckResult,
    owner: Option<&Owner>,
    location: FVector,
    extent: FVector,
) -> bool {
    let Some(o) = owner else {
        return true;
    };
    let loc = o.location;
    let inside_z = sq((loc.z + o.collision_width) - location.z) < sq(o.collision_height + extent.z);
    let inside_xy =
        sq_sum(loc.x - location.x, loc.y - location.y) < sq(o.collision_radius + extent.x);
    if !(inside_z && inside_xy) {
        return true;
    }
    res.actor = o.actor;
    res.normal = safe_normal(sub(location, loc));
    if res.normal.z < -0.5 || res.normal.z > 0.5 {
        res.location = v3(location.x, location.y, loc.z - extent.z);
    } else {
        let flat = safe_normal(flatten_xy(res.normal));
        let base = sub(loc, scale(flat, extent.x));
        res.location = add(base, v3(0.0, 0.0, location.z));
    }
    false
}

/// `Start + (End-Start)*T` via inline `FVector` operators: unfused (oracle-verified).
fn start_plus_span(start: FVector, end: FVector, t: f32) -> FVector {
    add(start, scale(sub(end, start), t))
}

/// `UPrimitive::LineCheck` (cylinder). Returns `true` on no hit.
pub fn cyl_line_check(
    res: &mut FCheckResult,
    owner: Option<&Owner>,
    end: FVector,
    start: FVector,
    extent: FVector,
) -> bool {
    let Some(o) = owner else {
        return true;
    };
    let loc = o.location;
    let net = add(extent, o.cyl_extent);

    // Quick X/Y rejects.
    let max_x = loc.x + net.x;
    if start.x >= max_x && end.x >= max_x {
        return true;
    }
    let min_x = loc.x - net.x;
    if start.x <= min_x && end.x <= min_x {
        return true;
    }
    let max_y = loc.y + net.y;
    if start.y >= max_y && end.y >= max_y {
        return true;
    }
    let min_y = loc.y - net.y;
    if start.y <= min_y && end.y <= min_y {
        return true;
    }

    // Quick Z rejects.
    let top_z = loc.z + o.collision_width + net.z;
    if start.z >= top_z && end.z >= top_z {
        return true;
    }
    let bot_z = loc.z + o.collision_width - net.z;
    if start.z <= bot_z && end.z <= bot_z {
        return true;
    }

    // Clip to top and bottom of cylinder.
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    if start.z >= top_z && end.z < top_z {
        t0 = (top_z - start.z) / (end.z - start.z);
        res.normal = v3(0.0, 0.0, 1.0);
    } else if start.z < top_z && end.z > top_z {
        t1 = (top_z - start.z) / (end.z - start.z);
    }
    if start.z <= bot_z && end.z > bot_z {
        t0 = (bot_z - start.z) / (end.z - start.z);
        res.normal = v3(0.0, 0.0, -1.0);
    } else if start.z > bot_z && end.z < bot_z {
        t1 = (bot_z - start.z) / (end.z - start.z);
    }

    // 2D circle clip about the origin.
    let kx = start.x - loc.x;
    let ky = start.y - loc.y;
    let vx = end.x - start.x;
    let vy = end.y - start.y;
    let a = vx.mul_add(vx, vy * vy);
    let b = 2.0 * kx.mul_add(vx, ky * vy);
    let c = kx.mul_add(kx, ky * ky) - net.x * net.x;
    let mut discrim = b.mul_add(b, -(4.0 * a * c));

    // Already inside the sphere: oppose further inward movement.
    if c < sq(1.0) && start.z > bot_z && start.z < top_z {
        let dir = dot(flatten_xy(sub(end, start)), sub(start, loc));
        if dir < -0.1 {
            res.time = 0.0;
            res.location = start;
            res.normal = safe_normal(flatten_xy(sub(start, loc)));
            res.actor = o.actor;
            res.primitive = std::ptr::null_mut();
            return false;
        }
        return true;
    }

    if discrim < 0.0 {
        return true;
    }

    if a < sq(0.0001) {
        // Outside or on the circle.
        if c >= 0.0 {
            return true;
        }
    } else {
        discrim = app_sqrt(discrim as f64) as f32;
        let r2a = 0.5f32 / a;
        t1 = min_f(t1, (discrim - b) * r2a);
        let t = -(discrim + b) * r2a;
        if t >= t0 {
            t0 = t;
            res.normal = sub(start_plus_span(start, end, t0), loc);
            res.normal.z = 0.0;
            normalize(&mut res.normal);
        }
        if t0 >= t1 {
            // Tangent, or passed the end caps.
            return true;
        }
    }

    res.time = clamp_f(t0 - 0.001, 0.0, 1.0);
    res.location = start_plus_span(start, end, res.time);
    res.actor = o.actor;
    res.primitive = std::ptr::null_mut();
    false
}

/// `UAlignedOvalCylinder` owner view: copy with `CollisionWidth = 0`,
/// `Location.X` scaled and the zero-width cylinder extent.
fn oval_owner(o: &Owner, ratio: f32) -> Owner {
    let mut m = *o;
    m.collision_width = 0.0;
    m.location.x *= ratio;
    m.cyl_extent = o.cyl_extent_zero_width;
    m
}

/// `UAlignedOvalCylinder::PointCheck`. Returns `true` on no hit.
pub fn aligned_oval_point_check(
    res: &mut FCheckResult,
    owner: Option<&Owner>,
    location: FVector,
    extent: FVector,
) -> bool {
    let Some(o) = owner else {
        return true;
    };
    if (o.collision_radius as f64) < 0.0001 || (o.collision_width as f64) < 0.0001 {
        return true;
    }
    let ratio = o.collision_radius / o.collision_width;
    let m = oval_owner(o, ratio);
    let mut loc = location;
    loc.x *= ratio;
    let mut ext = extent;
    ext.x *= ratio;
    if cyl_point_check(res, Some(&m), loc, ext) {
        return true;
    }
    res.location.x /= ratio;
    res.normal.x /= ratio;
    normalize(&mut res.normal);
    false
}

/// `UAlignedOvalCylinder::LineCheck`. Returns `true` on no hit.
pub fn aligned_oval_line_check(
    res: &mut FCheckResult,
    owner: Option<&Owner>,
    end: FVector,
    start: FVector,
    extent: FVector,
) -> bool {
    let Some(o) = owner else {
        return true;
    };
    if (o.collision_radius as f64) < 0.0001 || (o.collision_width as f64) < 0.0001 {
        return true;
    }
    let ratio = o.collision_radius / o.collision_width;
    let m = oval_owner(o, ratio);
    let mut e = end;
    e.x *= ratio;
    let mut s = start;
    s.x *= ratio;
    let mut ext = extent;
    ext.x *= ratio;
    if cyl_line_check(res, Some(&m), e, s, ext) {
        return true;
    }
    res.location.x /= ratio;
    res.normal.x /= ratio;
    normalize(&mut res.normal);
    false
}

/// Shared shape of `UOrientedCylinder` / `UOrientedOvalCylinder` point
/// checks: move inputs into the owner's local frame, run `inner`, then map
/// the result back with `(Location - Owner->Location)`.
pub fn oriented_point_check(
    res: &mut FCheckResult,
    o: &Owner,
    location: FVector,
    extent: FVector,
    inner: fn(&mut FCheckResult, Option<&Owner>, FVector, FVector) -> bool,
) -> bool {
    let local = o.to_local;
    let loc = add(transform_point(location, &local), o.location);
    let ext = transform_extent(extent, &local);
    if inner(res, Some(o), loc, ext) {
        return true;
    }
    let t = transpose(&local);
    res.location = transform_point(sub(res.location, o.location), &t);
    res.normal = transform_vector(res.normal, &t);
    false
}

/// Shared shape of the oriented line checks.
pub fn oriented_line_check(
    res: &mut FCheckResult,
    o: &Owner,
    end: FVector,
    start: FVector,
    extent: FVector,
    inner: fn(&mut FCheckResult, Option<&Owner>, FVector, FVector, FVector) -> bool,
) -> bool {
    let local = o.to_local;
    let s = add(transform_point(start, &local), o.location);
    let e = add(transform_point(end, &local), o.location);
    let ext = transform_extent(extent, &local);
    if inner(res, Some(o), e, s, ext) {
        return true;
    }
    let t = transpose(&local);
    res.location = transform_point(sub(res.location, o.location), &t);
    res.normal = transform_vector(res.normal, &t);
    false
}

/// `UOrientedCylinder::PointCheck`.
pub fn oriented_cyl_point_check(
    res: &mut FCheckResult,
    o: &Owner,
    location: FVector,
    extent: FVector,
) -> bool {
    oriented_point_check(res, o, location, extent, cyl_point_check)
}

/// `UOrientedCylinder::LineCheck`.
pub fn oriented_cyl_line_check(
    res: &mut FCheckResult,
    o: &Owner,
    end: FVector,
    start: FVector,
    extent: FVector,
) -> bool {
    oriented_line_check(res, o, end, start, extent, cyl_line_check)
}

/// `UOrientedOvalCylinder::PointCheck`.
pub fn oriented_oval_point_check(
    res: &mut FCheckResult,
    o: &Owner,
    location: FVector,
    extent: FVector,
) -> bool {
    oriented_point_check(res, o, location, extent, aligned_oval_point_check)
}

/// `UOrientedOvalCylinder::LineCheck`.
pub fn oriented_oval_line_check(
    res: &mut FCheckResult,
    o: &Owner,
    end: FVector,
    start: FVector,
    extent: FVector,
) -> bool {
    oriented_line_check(res, o, end, start, extent, aligned_oval_line_check)
}

/// `UPrimitive::GetCollisionBoundingBox` (`UnPrim.cpp:74`): the cylinder box
/// `(Loc - Extent, Loc + Extent)` with `Loc = (0, 0, Width)`, shifted by the
/// owner's location when `world`.
pub fn cyl_bounds(o: &Owner, world: bool) -> Aabb {
    let extent = v3(o.collision_radius, o.collision_radius, o.collision_height);
    let mut loc = v3(0.0, 0.0, o.collision_width);
    if world {
        loc = add(loc, o.location);
    }
    Aabb::from_min_max(sub(loc, extent), add(loc, extent))
}

/// `UOrientedCylinder`/`UOrientedOvalCylinder` bounding box helpers.
pub fn oriented_cyl_bounds(o: &Owner, world: bool) -> Aabb {
    let b = Aabb::from_extent(v3(
        o.collision_radius,
        o.collision_radius,
        o.collision_height,
    ));
    if world {
        b.transform_by(&o.to_world)
    } else {
        b
    }
}

pub fn oriented_oval_bounds(o: &Owner, world: bool) -> Aabb {
    let b = Aabb::from_extent(v3(
        o.collision_width,
        o.collision_radius,
        o.collision_height,
    ));
    if world {
        b.transform_by(&o.to_world)
    } else {
        b
    }
}

/// `UAlignedOvalCylinder::GetCollisionBoundingBox`.
pub fn aligned_oval_bounds(o: &Owner, world: bool) -> Aabb {
    let ext = v3(o.collision_width, o.collision_radius, o.collision_height);
    let loc = if world { o.location } else { v3(0.0, 0.0, 0.0) };
    Aabb::from_min_max(sub(loc, ext), add(loc, ext))
}

/// C++ `PointCheck(Result, Check, Box)` (`UnPrim.cpp:558`). `true` = no hit.
pub fn box_point_check(res: &mut FCheckResult, check: &FCheck, bx: &Aabb) -> bool {
    if let Some(o) = check.owner {
        let local = o.to_local;
        let inner = FCheck::point(
            transform_point(check.start, &local),
            transform_extent(check.extent, &local),
            None,
        );
        if box_point_check(res, &inner, bx) {
            return true;
        }
        let t = transpose(&local);
        res.normal = transform_vector(res.normal, &t);
        res.location = transform_point(res.location, &t);
        res.actor = o.actor;
        return false;
    }

    let source = Aabb::from_min_max(
        sub(check.start, check.extent),
        add(check.start, check.extent),
    );
    // Tolerance absorbs error accumulated by transformations in LineCheck.
    if !b_intersects(bx, &source, 0.003) {
        return true;
    }

    // Nearest point on the box to the start point.
    res.location = v3(
        clamp_f(check.start.x, bx.min.x, bx.max.x),
        clamp_f(check.start.y, bx.min.y, bx.max.y),
        clamp_f(check.start.z, bx.min.z, bx.max.z),
    );

    // Box plane that the source intersects least.
    let mut t = bx.max.z - source.min.z;
    res.time = t;
    res.normal = v3(0.0, 0.0, 1.0);
    t = source.max.z - bx.min.z;
    if t < res.time {
        res.time = t;
        res.normal = v3(0.0, 0.0, -1.0);
    }
    t = bx.max.y - source.min.y;
    if t < res.time {
        res.time = t;
        res.normal = v3(0.0, 1.0, 0.0);
    }
    t = source.max.y - bx.min.y;
    if t < res.time {
        res.time = t;
        res.normal = v3(0.0, -1.0, 0.0);
    }
    t = bx.max.x - source.min.x;
    if t < res.time {
        res.time = t;
        res.normal = v3(1.0, 0.0, 0.0);
    }
    t = source.max.x - bx.min.x;
    if t < res.time {
        res.time = t;
        res.normal = v3(-1.0, 0.0, 0.0);
    }
    false
}

/// C++ `ClipPlane` (`UnPrim.cpp:649`).
fn clip_plane(res: &mut FCheckResult, check: &FCheck, bx: &Aabb, normal: FVector, d: f32, e: f32) {
    // Tolerance guarantees a clip check even if start is slightly inside.
    if d > -0.003 && max_f(0.0, d) < e * res.time {
        let mut t = d / e;
        if t < 0.0 {
            t = 0.0;
        }
        let mut mid = add(scale(check.start, 1.0 - t), scale(check.end, t));
        // Push in to guarantee inside of this plane.
        mid = sub(mid, scale(normal, 0.001));
        if contains(bx, mid, 0.0) {
            // Push out to put the intersection point just outside the box.
            t -= 0.001;
            if t < 0.0 {
                t = 0.0;
            }
            res.time = t;
            let mut loc = add(scale(check.start, 1.0 - t), scale(check.end, t));
            // Move inward by extent.
            loc = sub(loc, scale(normal, dot(normal, check.extent)));
            res.location = loc;
            res.normal = normal;
        }
    }
}

/// C++ `LineCheck(Result, Check, Box)` (`UnPrim.cpp:683`). `true` = no hit.
pub fn box_line_check(res: &mut FCheckResult, check: &FCheck, bx: &Aabb) -> bool {
    if let Some(o) = check.owner {
        let local = o.to_local;
        let inner = FCheck {
            end: transform_point(check.end, &local),
            start: transform_point(check.start, &local),
            extent: transform_extent(check.extent, &local),
            owner: None,
        };
        if box_line_check(res, &inner, bx) {
            return true;
        }
        let t = transpose(&local);
        res.location = transform_point(res.location, &t);
        res.normal = transform_vector(res.normal, &t);
        res.actor = o.actor;
        return false;
    }

    // Start already inside the box: check the reverse segment.
    if !box_point_check(res, check, bx) {
        let reverse = FCheck {
            end: check.start,
            start: check.end,
            extent: check.extent,
            owner: check.owner,
        };
        let mut reverse_result = empty_result();
        if box_point_check(&mut reverse_result, &reverse, bx) && reverse_result.time > res.time {
            // End is moving further into the box; it's a hit.
            res.time = 0.0;
            return false;
        }
        // Pretend there's no hit to permit traveling outward.
        return true;
    }

    res.time = 1.0;
    let big = Aabb::from_min_max(sub(bx.min, check.extent), add(bx.max, check.extent));
    clip_plane(
        res,
        check,
        &big,
        v3(-1.0, 0.0, 0.0),
        big.min.x - check.start.x,
        check.end.x - check.start.x,
    );
    clip_plane(
        res,
        check,
        &big,
        v3(1.0, 0.0, 0.0),
        check.start.x - big.max.x,
        check.start.x - check.end.x,
    );
    clip_plane(
        res,
        check,
        &big,
        v3(0.0, -1.0, 0.0),
        big.min.y - check.start.y,
        check.end.y - check.start.y,
    );
    clip_plane(
        res,
        check,
        &big,
        v3(0.0, 1.0, 0.0),
        check.start.y - big.max.y,
        check.start.y - check.end.y,
    );
    clip_plane(
        res,
        check,
        &big,
        v3(0.0, 0.0, -1.0),
        big.min.z - check.start.z,
        check.end.z - check.start.z,
    );
    clip_plane(
        res,
        check,
        &big,
        v3(0.0, 0.0, 1.0),
        check.start.z - big.max.z,
        check.start.z - check.end.z,
    );
    res.time == 1.0
}

/// `UBox` / `UBoxPrim`-free helper: `UBox::GetCollisionBoundingBox`.
pub fn ubox_bounds(o: &Owner, world: bool) -> Aabb {
    let width = if o.collision_width != 0.0 {
        o.collision_width
    } else {
        o.collision_radius
    };
    let b = Aabb::from_extent(v3(o.collision_radius, width, o.collision_height));
    if world {
        b.transform_by(&o.to_world)
    } else {
        b
    }
}

/// `UBox::GetCollisionExtent`.
pub fn ubox_extent(o: &Owner) -> FVector {
    let width = if o.collision_width != 0.0 {
        o.collision_width
    } else {
        o.collision_radius
    };
    v3(o.collision_radius, width, o.collision_height)
}

/// `UBox::PointCheck`.
pub fn ubox_point_check(
    res: &mut FCheckResult,
    o: &Owner,
    location: FVector,
    extent: FVector,
) -> bool {
    let check = FCheck::point(location, extent, Some(*o));
    box_point_check(res, &check, &ubox_bounds(o, false))
}

/// `UBox::LineCheck`.
pub fn ubox_line_check(
    res: &mut FCheckResult,
    o: &Owner,
    end: FVector,
    start: FVector,
    extent: FVector,
) -> bool {
    let check = FCheck {
        end,
        start,
        extent,
        owner: Some(*o),
    };
    box_line_check(res, &check, &ubox_bounds(o, false))
}

/// `UBoxPrim::GetCollisionBoundingBox`.
pub fn boxprim_bounds(bounding_box: &Aabb, o: &Owner, world: bool) -> Aabb {
    if world {
        bounding_box.transform_by(&o.to_world)
    } else {
        *bounding_box
    }
}

/// `UBoxPrim::GetCollisionExtent`.
pub fn boxprim_extent(bounding_box: &Aabb) -> FVector {
    v3(
        max_f(-bounding_box.min.x, bounding_box.max.x),
        max_f(-bounding_box.min.y, bounding_box.max.y),
        max_f(-bounding_box.min.z, bounding_box.max.z),
    )
}

/// `UBoxPrim::ValidateActor`: returns `(CollisionRadius = CollisionWidth, CollisionHeight)`.
pub fn boxprim_validate(bounding_box: &Aabb) -> (f32, f32) {
    let ext = boxprim_extent(bounding_box);
    (max_f(ext.x, ext.y), ext.z)
}

/// Read-only view of one `UModel`'s BSP arrays, from the C++ `TArray` storage.
#[derive(Clone, Copy)]
pub struct BspArrays<'a> {
    pub nodes: &'a [FBspNode],
    pub points: &'a [FVector],
    pub verts: &'a [FVert],
    pub surfs: &'a [FBspSurf],
}

/// `FCheckResult::BspNode(Radius)` (`UnPrim.cpp:838`), index form.
///
/// The shell has already done `Cast<UModel>(Primitive)`: pass `None` if it
/// is not a model. If `Actor` is a mover, pass its `ToLocal()` as
/// `mover_to_local`. Returns the matching node index.
pub fn bsp_node(
    bsp: Option<&BspArrays>,
    item: i32,
    location: FVector,
    normal: FVector,
    radius: f32,
    mover_to_local: Option<&FCoords>,
) -> Option<usize> {
    let bsp = bsp?;
    if item == INDEX_NONE {
        return None;
    }
    let (loc, norm) = match mover_to_local {
        Some(c) => (transform_point(location, c), transform_vector(normal, c)),
        None => (location, normal),
    };

    let mut i_node = item;
    let mut any: Option<usize> = None;
    while i_node != INDEX_NONE {
        let node = &bsp.nodes[i_node as usize];
        let num = node.num_vertices as usize;
        let plane3 = v3(node.plane.x, node.plane.y, node.plane.z);
        if num > 0 && dot(plane3, norm) >= 0.99 {
            any = Some(i_node as usize);
            let vs = &bsp.verts[node.i_vert_pool as usize..];
            let mut prev = bsp.points[vs[num - 1].p_vertex as usize];
            let mut sense = 1.0f32;
            let mut i = 0usize;
            while i < num {
                let vertex = bsp.points[vs[i].p_vertex as usize];
                let mut clip_norm = cross(plane3, sub(vertex, prev));
                normalize(&mut clip_norm);
                // FPlane(Vertex, ClipNorm): W = Vertex | ClipNorm.
                let w = dot(vertex, clip_norm);
                let clip_plane = FPlane {
                    x: clip_norm.x,
                    y: clip_norm.y,
                    z: clip_norm.z,
                    w,
                };
                let d = plane_dot(clip_plane, loc);
                if d * sense < -radius {
                    if i == 0 {
                        sense = -1.0;
                    } else {
                        break;
                    }
                }
                prev = vertex;
                i += 1;
            }
            if i == num {
                return Some(i_node as usize);
            }
        }
        i_node = node.i_plane;
    }

    if radius != 0.0 { None } else { any }
}

/// `FCheckResult::BspSurf(Radius)`: the surface index of the matching node.
pub fn bsp_surf(
    bsp: Option<&BspArrays>,
    item: i32,
    location: FVector,
    normal: FVector,
    radius: f32,
    mover_to_local: Option<&FCoords>,
) -> Option<usize> {
    let node = bsp_node(bsp, item, location, normal, radius, mover_to_local)?;
    let bsp = bsp?;
    Some(bsp.nodes[node].i_surf as usize)
}
