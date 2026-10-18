//! Port of the `FPoly` editor-polygon math in `Engine/Src/UnFPoly.cpp`, with the
//! `FVector`/`FPlane` operators it uses from `Core/Inc/UnMath.h`.
//!
//! FMA contraction: the engine is built with clang `-ffp-contract=on`, which fuses
//! `a*b + c*d` only inside one C++ expression. `dot`, `cross`, `plane_dot` and the
//! `*_plane_*` helpers reproduce the fused forms observed in the original object
//! code. Products that cross an operator-call or a separate statement stay unfused.
//!
//! Errors: the original `appErrorf` calls become `Err(message)`; the C++ shell
//! raises them. Warnings (`debugf(NAME_Warning, ...)`) go through [`Log`].

use hp2_abi::engine::generated::FPoly;
use hp2_abi::{FCoords, FPlane, FVector};

use crate::math::app_sqrt;
use crate::vec::{add, scale, size_squared, sub, zero};

pub const MAX_VERTICES: usize = 16;
pub const VERTEX_THRESHOLD: i32 = 14;
pub const PF_ED_CUT: u32 = 0x8000_0000;

pub const SP_COPLANAR: i32 = 0;
pub const SP_FRONT: i32 = 1;
pub const SP_BACK: i32 = 2;
pub const SP_SPLIT: i32 = 3;

const SMALL_NUMBER: f32 = 1.0e-8;
const FLOAT_NORMAL_THRESH: f32 = 0.0001;
const THRESH_ZERO_NORM_SQUARED: f32 = 0.0001;
const THRESH_POINTS_ARE_SAME: f32 = 0.002;
const THRESH_SPLIT_POLY_WITH_PLANE: f32 = 0.25;
const THRESH_SPLIT_POLY_PRECISELY: f32 = 0.01;

/// Result type: `Err` carries the message the original raises with `appErrorf`.
pub type Res<T> = Result<T, String>;

/// Receives the warnings the original emits with `debugf(NAME_Warning, ...)`.
pub trait Log {
    fn warn(&mut self, msg: &str);
}

/// Discards warnings.
pub struct NoLog;

impl Log for NoLog {
    fn warn(&mut self, _msg: &str) {}
}

/// `a|b`: `X*V.X + Y*V.Y + Z*V.Z`, contracted as `fma(a.z, b.z, fma(a.x, b.x, a.y*b.y))`.
#[inline]
pub(crate) fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `a^b`: `(Y*V.Z - Z*V.Y, Z*V.X - X*V.Z, X*V.Y - Y*V.X)`, contracted per component.
#[inline]
pub(crate) fn cross(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.y.mul_add(b.z, -(a.z * b.y)),
        y: a.z.mul_add(b.x, -(a.x * b.z)),
        z: a.x.mul_add(b.y, -(a.y * b.x)),
    }
}

#[inline]
fn is_zero(a: FVector) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

/// `FVector::Normalize()`: scales in place; returns false (vector unchanged) when
/// the squared length is below `SMALL_NUMBER`.
pub(crate) fn normalize(v: &mut FVector) -> bool {
    let square_sum = size_squared(*v);
    if square_sum >= SMALL_NUMBER {
        let s = (1.0f64 / app_sqrt(f64::from(square_sum))) as f32;
        *v = scale(*v, s);
        return true;
    }
    false
}

/// `FVector::SafeNormal()` (out-of-line in `UnMath.cpp`): zero vector below `SMALL_NUMBER`.
pub(crate) fn safe_normal(v: FVector) -> FVector {
    let square_sum = size_squared(v);
    if square_sum < SMALL_NUMBER {
        return zero();
    }
    let size = app_sqrt(f64::from(square_sum)) as f32;
    let s = 1.0f32 / size;
    scale(v, s)
}

/// `FVector::TransformVectorBy(Coords)`: each component is `self | axis`.
#[inline]
pub(crate) fn transform_vector_by(v: FVector, c: &FCoords) -> FVector {
    FVector {
        x: dot(v, c.x_axis),
        y: dot(v, c.y_axis),
        z: dot(v, c.z_axis),
    }
}

/// `FPointPlaneDist(Point, PlaneBase, PlaneNormal) = (Point - PlaneBase) | PlaneNormal`.
#[inline]
pub(crate) fn point_plane_dist(p: FVector, base: FVector, normal: FVector) -> f32 {
    dot(sub(p, base), normal)
}

/// `FPlane::PlaneDot(P) = X*P.X + Y*P.Y + Z*P.Z - W`.
#[inline]
pub(crate) fn plane_dot(plane: FPlane, p: FVector) -> f32 {
    dot(plane_xyz(plane), p) - plane.w
}

#[inline]
fn plane_xyz(plane: FPlane) -> FVector {
    FVector {
        x: plane.x,
        y: plane.y,
        z: plane.z,
    }
}

/// `FPlane(InBase, InNormal)`: normal as XYZ, `W = InBase | InNormal`.
#[inline]
pub(crate) fn plane_from(base: FVector, normal: FVector) -> FPlane {
    FPlane {
        x: normal.x,
        y: normal.y,
        z: normal.z,
        w: dot(base, normal),
    }
}

#[inline]
fn within_same(t: f32) -> bool {
    t > -THRESH_POINTS_ARE_SAME && t < THRESH_POINTS_ARE_SAME
}

/// `FPointsAreSame(P, Q)`: every axis difference lies strictly inside the threshold.
#[inline]
pub(crate) fn points_are_same(p: FVector, q: FVector) -> bool {
    within_same(p.x - q.x) && within_same(p.y - q.y) && within_same(p.z - q.z)
}

/// `FPointsAreNear(P1, P2, Dist)`: returns false as soon as an axis difference is
/// `>= Dist` (NaN comparisons fall through, matching the original `if` form).
#[inline]
// The negated form is deliberate: a NaN difference must not return early, as in the original `if (x >= d) return 0`.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub(crate) fn points_are_near(p1: FVector, p2: FVector, dist: f32) -> bool {
    !((p1.x - p2.x).abs() >= dist)
        && !((p1.y - p2.y).abs() >= dist)
        && !((p1.z - p2.z).abs() >= dist)
}

/// `FLinePlaneIntersection(P1, P2, PlaneOrigin, PlaneNormal)`.
#[inline]
pub(crate) fn line_plane_intersection(
    p1: FVector,
    p2: FVector,
    origin: FVector,
    normal: FVector,
) -> FVector {
    let num = dot(sub(origin, p1), normal);
    let den = dot(sub(p2, p1), normal);
    add(p1, scale(sub(p2, p1), num / den))
}

/// `FLinePlaneIntersection(P1, P2, FPlane)`.
#[inline]
fn line_plane_intersection_plane(p1: FVector, p2: FVector, plane: FPlane) -> FVector {
    let n = plane_xyz(plane);
    let num = plane.w - dot(p1, n);
    let den = dot(sub(p2, p1), n);
    add(p1, scale(sub(p2, p1), num / den))
}

/// Fixed-capacity vertex list; `n` keeps counting past the capacity so the caller
/// can report an overflow.
#[derive(Clone, Copy)]
struct VertList {
    v: [FVector; MAX_VERTICES],
    n: usize,
}

impl VertList {
    fn new() -> Self {
        VertList {
            v: [zero(); MAX_VERTICES],
            n: 0,
        }
    }

    #[inline]
    fn push(&mut self, x: FVector) {
        if self.n < MAX_VERTICES {
            self.v[self.n] = x;
        }
        self.n += 1;
    }

    fn store(&self, dst: &mut FPoly, what: &str) -> Res<()> {
        if self.n > MAX_VERTICES {
            return Err(format!("FPoly::{what}: Vertex overflow"));
        }
        dst.vertex[..self.n].copy_from_slice(&self.v[..self.n]);
        dst.num_vertices = self.n as i32;
        Ok(())
    }
}

/// `FPoly::Reverse()`.
pub fn reverse(p: &mut FPoly) {
    p.normal = scale(p.normal, -1.0);
    let n = p.num_vertices.max(0) as usize;
    for i in 0..n / 2 {
        p.vertex.swap(i, (n - 1) - i);
    }
}

/// `FPoly::Fix()`: removes coincident vertices; returns the vertex count (0 or >= 3).
pub fn fix(p: &mut FPoly, log: &mut dyn Log) -> i32 {
    let n = p.num_vertices.max(0) as usize;
    let mut j = 0usize;
    let mut prev = n.wrapping_sub(1);
    for i in 0..n {
        if !points_are_same(p.vertex[i], p.vertex[prev]) {
            if j != i {
                p.vertex[j] = p.vertex[i];
            }
            prev = j;
            j += 1;
        } else {
            log.warn("FPoly::Fix: Collapsed a point");
        }
    }
    p.num_vertices = if j >= 3 { j as i32 } else { 0 };
    p.num_vertices
}

/// `FPoly::Area()`.
pub fn area(p: &FPoly) -> f32 {
    let n = p.num_vertices.max(0) as usize;
    let mut area = 0.0f32;
    let mut side1 = sub(p.vertex[1], p.vertex[0]);
    for i in 2..n {
        let side2 = sub(p.vertex[i], p.vertex[0]);
        area += crate::vec::size(cross(side1, side2));
        side1 = side2;
    }
    area
}

/// `FPoly::SplitWithPlane(PlaneBase, PlaneNormal, FrontPoly, BackPoly, VeryPrecise)`.
///
/// The caller (C++ shell) performs the `check(NumVertices>=3)` and
/// `check(NumVertices<=MAX_VERTICES)` assertions. `out` is `None` for the
/// status-only query; otherwise both halves are written.
pub fn split_with_plane(
    p: &FPoly,
    plane_base: FVector,
    plane_normal: FVector,
    out: Option<(&mut FPoly, &mut FPoly)>,
    very_precise: bool,
    log: &mut dyn Log,
) -> Res<i32> {
    let thresh = if very_precise {
        THRESH_SPLIT_POLY_PRECISELY
    } else {
        THRESH_SPLIT_POLY_WITH_PLANE
    };
    let n = p.num_vertices.max(0) as usize;
    let v = &p.vertex;

    #[derive(Clone, Copy, PartialEq)]
    enum Side {
        Front,
        Back,
        Either,
    }

    let mut dist = 0.0f32;
    let mut max_dist = 0.0f32;
    let mut min_dist = 0.0f32;
    let mut prev_status = Side::Either;
    for (i, &vi) in v[..n].iter().enumerate() {
        dist = point_plane_dist(vi, plane_base, plane_normal);
        if i == 0 || dist > max_dist {
            max_dist = dist;
        }
        if i == 0 || dist < min_dist {
            min_dist = dist;
        }
        if dist > thresh {
            prev_status = Side::Front;
        } else if dist < -thresh {
            prev_status = Side::Back;
        }
    }
    if max_dist < thresh && min_dist > -thresh {
        return Ok(SP_COPLANAR);
    } else if max_dist < thresh {
        return Ok(SP_BACK);
    } else if min_dist > -thresh {
        return Ok(SP_FRONT);
    }

    // Split.
    let Some((front, back)) = out else {
        return Ok(SP_SPLIT);
    };
    *front = *p;
    front.poly_flags |= PF_ED_CUT;
    front.num_vertices = 0;
    *back = *p;
    back.poly_flags |= PF_ED_CUT;
    back.num_vertices = 0;

    let mut fl = VertList::new();
    let mut bl = VertList::new();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let prev_dist = dist;
        dist = point_plane_dist(v[i], plane_base, plane_normal);

        let status = if dist > thresh {
            Side::Front
        } else if dist < -thresh {
            Side::Back
        } else {
            prev_status
        };

        if status != prev_status {
            // Crossing. Either Front-to-Back or Back-to-Front.
            if dist >= -thresh && dist < thresh {
                // This point lies on the plane: it belongs to both halves.
                fl.push(v[i]);
                bl.push(v[i]);
            } else if prev_dist >= -thresh && prev_dist < thresh {
                // Previous point lies on the plane.
                if status == Side::Front {
                    fl.push(v[j]);
                    fl.push(v[i]);
                } else {
                    bl.push(v[j]);
                    bl.push(v[i]);
                }
            } else {
                // Intersection point lies strictly between the two vertices.
                let inter = line_plane_intersection(v[j], v[i], plane_base, plane_normal);
                if prev_status == Side::Front {
                    fl.push(inter);
                    bl.push(inter);
                    bl.push(v[i]);
                } else {
                    bl.push(inter);
                    fl.push(inter);
                    fl.push(v[i]);
                }
            }
        } else if status == Side::Front {
            fl.push(v[i]);
        } else {
            bl.push(v[i]);
        }
        j = i;
        prev_status = status;
    }

    fl.store(front, "SplitWithPlane")?;
    bl.store(back, "SplitWithPlane")?;

    // Handle possibility of sliver polys due to precision errors.
    if fix(front, log) < 3 {
        log.warn("FPoly::SplitWithPlane: Ignored front sliver");
        return Ok(SP_BACK);
    } else if fix(back, log) < 3 {
        log.warn("FPoly::SplitWithPlane: Ignored back sliver");
        return Ok(SP_FRONT);
    }
    Ok(SP_SPLIT)
}

/// `FPoly::SplitWithPlaneFast(Plane, FrontPoly, BackPoly)`.
pub fn split_with_plane_fast(
    p: &FPoly,
    plane: FPlane,
    out: Option<(&mut FPoly, &mut FPoly)>,
) -> Res<i32> {
    let n = p.num_vertices.max(0) as usize;
    // true = V_FRONT, false = V_BACK.
    let mut is_front = [false; MAX_VERTICES];
    let mut any_front = false;
    let mut any_back = false;
    for (i, &vert) in p.vertex[..n].iter().enumerate() {
        let dist = plane_dot(plane, vert);
        if dist >= 0.0 {
            is_front[i] = true;
            if dist > THRESH_SPLIT_POLY_WITH_PLANE {
                any_front = true;
            }
        } else if dist < -THRESH_SPLIT_POLY_WITH_PLANE {
            any_back = true;
        }
    }
    if !any_front {
        return Ok(if any_back { SP_BACK } else { SP_COPLANAR });
    }
    if !any_back {
        return Ok(SP_FRONT);
    }

    if let Some((front, back)) = out {
        let mut fl = VertList::new();
        let mut bl = VertList::new();
        let mut prev_front = is_front[n - 1];
        let mut w = p.vertex[n - 1];
        for (i, &v) in p.vertex[..n].iter().enumerate() {
            let status = is_front[i];
            if status != prev_front {
                // Crossing: the intersection lands on both halves.
                let inter = line_plane_intersection_plane(w, v, plane);
                fl.push(inter);
                bl.push(inter);
                if prev_front {
                    bl.push(v);
                } else {
                    fl.push(v);
                }
            } else if status {
                fl.push(v);
            } else {
                bl.push(v);
            }
            prev_front = status;
            w = v;
        }
        fl.store(front, "SplitWithPlaneFast")?;
        front.base = p.base;
        front.normal = p.normal;
        front.poly_flags = p.poly_flags;

        bl.store(back, "SplitWithPlaneFast")?;
        back.base = p.base;
        back.normal = p.normal;
        back.poly_flags = p.poly_flags;
    }
    Ok(SP_SPLIT)
}

/// `FPoly::SplitInHalf(OtherHalf)`.
///
/// Errors (`Err`) leave both polygons untouched.
pub fn split_in_half(p: &mut FPoly, other: &mut FPoly) -> Res<()> {
    let n = p.num_vertices;
    if n <= 3 || n > MAX_VERTICES as i32 {
        return Err(format!("FPoly::SplitInHalf: {n} Vertices"));
    }
    let m = n / 2;

    *other = *p;
    other.num_vertices = (n - m) + 1;
    p.num_vertices = m + 1;

    for i in 0..(other.num_vertices - 1) as usize {
        other.vertex[i] = p.vertex[i + m as usize];
    }
    other.vertex[(other.num_vertices - 1) as usize] = p.vertex[0];

    p.poly_flags |= PF_ED_CUT;
    other.poly_flags |= PF_ED_CUT;
    Ok(())
}

/// `FPoly::CalcNormal(bSilent)`: returns 0 on success, 1 on a zero-area polygon.
pub fn calc_normal(p: &mut FPoly, bsilent: bool, log: &mut dyn Log) -> i32 {
    let n = p.num_vertices.max(0) as usize;
    p.normal = zero();
    for i in 2..n {
        let c = cross(
            sub(p.vertex[i - 1], p.vertex[0]),
            sub(p.vertex[i], p.vertex[0]),
        );
        p.normal = add(p.normal, c);
    }
    if size_squared(p.normal) < THRESH_ZERO_NORM_SQUARED {
        if !bsilent {
            log.warn("FPoly::CalcNormal: Zero-area polygon");
        }
        return 1;
    }
    normalize(&mut p.normal);
    0
}

/// `FPoly::Finalize(NoError)`: `Ok(-1)` for a silent failure when `no_error`,
/// `Err` for a fatal failure (the shell raises it with `appErrorf`).
pub fn finalize(p: &mut FPoly, no_error: bool, log: &mut dyn Log) -> Res<i32> {
    fix(p, log);
    if p.num_vertices < 3 {
        let msg = format!("FPoly::Finalize: Not enough vertices ({})", p.num_vertices);
        log.warn(&msg);
        return if no_error { Ok(-1) } else { Err(msg) };
    }

    // If no normal, compute from cross-product and normalize it.
    if is_zero(p.normal) && p.num_vertices >= 3 && calc_normal(p, false, log) != 0 {
        let msg = format!(
            "FPoly::Finalize: Normalization failed, verts={}, size={:.6}",
            p.num_vertices,
            crate::vec::size(p.normal)
        );
        log.warn(&msg);
        return if no_error { Ok(-1) } else { Err(msg) };
    }

    // If texture U and V coordinates weren't specified, generate them.
    if is_zero(p.texture_u) && is_zero(p.texture_v) {
        let n = p.num_vertices.max(0) as usize;
        for i in 1..n {
            p.texture_u = safe_normal(cross(sub(p.vertex[0], p.vertex[i]), p.normal));
            p.texture_v = safe_normal(cross(p.normal, p.texture_u));
            if size_squared(p.texture_u) != 0.0 && size_squared(p.texture_v) != 0.0 {
                break;
            }
        }
    }
    Ok(0)
}

#[inline]
fn is_backfaced(poly: &FPoly, point: FVector) -> bool {
    dot(sub(point, poly.base), poly.normal) < 0.0
}

#[inline]
fn is_coplanar(poly: &FPoly, test: &FPoly) -> bool {
    dot(sub(poly.base, test.base), poly.normal).abs() < 0.01
        && dot(poly.normal, test.normal).abs() > 0.9999
}

/// `FPoly::Faces(Test)`: whether the two polygons face each other.
pub fn faces(p: &FPoly, test: &FPoly) -> i32 {
    if is_coplanar(p, test) {
        return 0;
    }
    let test_n = test.num_vertices.max(0) as usize;
    let n = p.num_vertices.max(0) as usize;
    for i in 0..test_n {
        if !is_backfaced(p, test.vertex[i]) {
            for j in 0..n {
                if is_backfaced(test, p.vertex[j]) {
                    return 1;
                }
            }
            return 0;
        }
    }
    0
}

/// `FPoly::Transform(Coords, PreSubtract, PostAdd, Orientation)`.
pub fn transform(
    p: &mut FPoly,
    point_xform: &FCoords,
    vector_xform: &FCoords,
    pre: FVector,
    post: FVector,
    orientation: f32,
) {
    let n = p.num_vertices.max(0) as usize;

    p.texture_u = transform_vector_by(p.texture_u, vector_xform);
    p.texture_v = transform_vector_by(p.texture_v, vector_xform);

    p.base = add(transform_vector_by(sub(p.base, pre), point_xform), post);
    for i in 0..n {
        p.vertex[i] = add(
            transform_vector_by(sub(p.vertex[i], pre), point_xform),
            post,
        );
    }

    // Flip vertex order if orientation is negative.
    if orientation < 0.0 {
        for i in 0..n / 2 {
            p.vertex.swap(i, (n - 1) - i);
        }
    }

    // The coordinate system is orthogonal but not orthonormal: renormalize.
    p.normal = safe_normal(transform_vector_by(p.normal, vector_xform));
}

/// `FPoly::RemoveColinears()`: returns 1 if convex, 0 if nonconvex or collapsed.
pub fn remove_colinears(p: &mut FPoly) -> i32 {
    let mut n = p.num_vertices.max(0) as usize;
    let normal = p.normal;
    let mut side_plane_normal = [zero(); MAX_VERTICES];

    let mut i = 0usize;
    while i < n {
        let j = if i == 0 { n - 1 } else { i - 1 };

        // Create cutting plane perpendicular to both this side and the polygon's normal.
        let side = sub(p.vertex[i], p.vertex[j]);
        side_plane_normal[i] = cross(side, normal);

        if !normalize(&mut side_plane_normal[i]) {
            // Eliminate these nearly identical points.
            p.vertex.copy_within((i + 1)..n, i);
            n -= 1;
            p.num_vertices = n as i32;
            if n < 3 {
                p.num_vertices = 0;
                return 0; // Collapsed.
            }
            // i-- (the loop increment below restores i).
            i = i.wrapping_sub(1);
        }
        i = i.wrapping_add(1);
    }

    let mut i = 0usize;
    while i < n {
        let j = if i + 1 >= n { 0 } else { i + 1 };

        if points_are_near(
            side_plane_normal[i],
            side_plane_normal[j],
            FLOAT_NORMAL_THRESH,
        ) {
            // Eliminate colinear points.
            p.vertex.copy_within((i + 1)..n, i);
            side_plane_normal.copy_within((i + 1)..n, i);
            n -= 1;
            p.num_vertices = n as i32;
            if n < 3 {
                p.num_vertices = 0;
                return 0; // Collapsed.
            }
            i = i.wrapping_sub(1);
        } else {
            for jj in 0..n {
                if jj != i {
                    match split_with_plane(
                        p,
                        p.vertex[i],
                        side_plane_normal[i],
                        None,
                        false,
                        &mut NoLog,
                    ) {
                        // Nonconvex (or numerical precision error).
                        Ok(SP_FRONT) | Ok(SP_SPLIT) => return 0,
                        // SP_BACK: convex. SP_COPLANAR: probably convex.
                        _ => {}
                    }
                }
            }
        }
        i = i.wrapping_add(1);
    }
    1
}

/// `FPoly::Split(Normal, Base, NoOverflow)`: returns the new vertex count, 0 if
/// clipped away.
///
/// Divergence: the original copies an uninitialised `FPoly Front` into `*this`
/// (unmodelled fields and `TextureU/V` are indeterminate there). This port starts
/// `Front` as a copy of `*this`, so those fields are preserved.
pub fn split(p: &mut FPoly, normal: FVector, base: FVector, no_overflow: bool) -> Res<i32> {
    let plane = plane_from(base, normal);
    if no_overflow && p.num_vertices >= VERTEX_THRESHOLD {
        // Don't split it, just reject it.
        return Ok(if split_with_plane_fast(p, plane, None)? == SP_BACK {
            0
        } else {
            p.num_vertices
        });
    }

    // Split it.
    let mut front = *p;
    let mut back = *p;
    match split_with_plane_fast(p, plane, Some((&mut front, &mut back)))? {
        SP_BACK => Ok(0),
        SP_SPLIT => {
            *p = front;
            Ok(p.num_vertices)
        }
        _ => Ok(p.num_vertices),
    }
}
