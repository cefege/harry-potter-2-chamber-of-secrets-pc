//! Port of the clipping math in `Render/Src/UnParticleRn.cpp`:
//! `ComputeClipCodes`, the `ClipToPlane` distance/interpolate pairs,
//! `Clip<Plane>`, and `ClipPolygon`. C++ keeps `New<FTransTexture>(GMem)`:
//! the caller supplies new-vertex storage through an allocator callback,
//! invoked in the same order as the C++ allocations.
//!
//! Float contraction follows `particle.rs`: `madd` is fused on arm64, where the
//! reference build uses `-ffp-contract=on`. Subtract-only and compare-only
//! expressions stay unfused.
//!
//! The `DEBUG_CLIPPING` edge adjustment is compiled out in the reference build
//! and not ported; `ADJUST_EDGE` is the identity.

use crate::particle::{
    FTransTexture, FVF_OUT_NEAR, FVF_OUT_XMAX, FVF_OUT_XMIN, FVF_OUT_YMAX, FVF_OUT_YMIN, FVector,
    ParticleFrame, compute_outcode, lerp_trans,
};

/// `a*b + c` as the reference build compiles it (see `particle::madd`).
#[inline]
fn madd(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        a * b + c
    }
}

/// `ClipPolygon`'s `LocalPts[2][64]` scratch capacity.
pub const CLIP_CAPACITY: usize = 64;

/// `ComputeClipCodes(Frame, tt)`: `tt.ComputeOutcode(Frame)`, which writes the
/// outcode into `tt.transform.flags`.
pub fn compute_clip_codes(frame: &ParticleFrame, tt: &mut FTransTexture) {
    tt.transform.flags = compute_outcode(frame, &tt.transform.point);
}

/// `FTransform::Project` (`Engine/Inc/UnRender.h`): perspective divide and
/// screen mapping. `IntY` is `appFloor(ScreenY)`, which on this target is
/// `static_cast<INT>` (truncation toward zero), not floor.
fn project(frame: &ParticleFrame, tt: &mut FTransTexture) {
    let t = &mut tt.transform;
    t.rz = frame.proj_z / t.point.z;
    t.screen_x = madd(t.point.x, t.rz, frame.fx15);
    t.screen_y = madd(t.point.y, t.rz, frame.fy15);
    t.int_y = t.screen_y as i32;
}

/// The five `ClipToPlane` specializations (`ClipNearPlane`, `ClipLeftPlane`,
/// `ClipRightPlane`, `ClipTopPlane`, `ClipBottomPlane`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipPlane {
    Near,
    Left,
    Right,
    Top,
    Bottom,
}

impl ClipPlane {
    /// `distance(pt)`. Near is `Z - d` with `d = 1`. The edges are `d*Z ± X|Y`,
    /// where `d` is the frame's `PrjXM`/`PrjXP`/`PrjYM`/`PrjYP`.
    #[inline]
    fn distance(self, frame: &ParticleFrame, p: &FVector) -> f32 {
        match self {
            ClipPlane::Near => p.z - 1.0,
            ClipPlane::Left => madd(frame.prjxm, p.z, p.x),
            ClipPlane::Right => madd(frame.prjxp, p.z, -p.x),
            ClipPlane::Top => madd(frame.prjym, p.z, p.y),
            ClipPlane::Bottom => madd(frame.prjyp, p.z, -p.y),
        }
    }
}

/// `Clip<Plane>(edge, Out, In, Count)`. Sutherland-Hodgman over one plane.
/// A new crossing vertex is allocated only when an edge changes sign, before
/// it is interpolated and projected. Returns the number of entries written to
/// `out`; a polygon with fewer than 3 vertices clips to 0 without allocating.
///
/// # Safety
/// Every pointer in `input` must point to a live `FTransTexture`. Each pointer
/// returned by `alloc` must be non-null, live, unaliased, and distinct from the
/// inputs.
unsafe fn clip(
    plane: ClipPlane,
    frame: &ParticleFrame,
    input: &[*mut FTransTexture],
    out: &mut [*mut FTransTexture],
    alloc: &mut dyn FnMut() -> *mut FTransTexture,
) -> usize {
    let count = input.len();
    if count < 3 {
        return 0;
    }

    let mut dst = 0usize;
    let mut j = count - 1;
    for i in 0..count {
        // SAFETY: `input` entries are live per the function contract.
        let cur = unsafe { &*input[i] };
        let prev = unsafe { &*input[j] };
        let dist1 = plane.distance(frame, &cur.transform.point);
        let dist2 = plane.distance(frame, &prev.transform.point);
        if dist1 * dist2 < 0.0 {
            // Opposite sides of the plane.
            let f = dist1 / (dist1 - dist2);
            let new = alloc();
            assert!(!new.is_null(), "clip: allocator returned null");
            out[dst] = new;
            dst += 1;
            // SAFETY: `new` is non-null, live, and distinct from the inputs.
            let out_vert = unsafe { &mut *new };
            // `ClipToPlane::interpolate`: lerp(f, In[i], In[j], out), then Project.
            lerp_trans(f, cur, prev, out_vert);
            project(frame, out_vert);
        }
        if dist1 >= 0.0 {
            out[dst] = input[i];
            dst += 1;
        }
        j = i;
    }
    dst
}

/// `ClipPolygon(Frame, Pts[], LocalCount, code)`: clips the polygon in `pts`
/// against each plane selected by `code`, ping-ponging between two scratch
/// buffers. On success (result > 2) the first `result` entries of `pts` are
/// overwritten with the clipped polygon, which may be longer than the input.
/// Returns that count, or 0 if fewer than 3 vertices survive (then `pts` is
/// untouched). A negative `local_count` is treated as 0.
///
/// `alloc` supplies storage for each new crossing vertex; it is called once per
/// sign change, in the same order as the C++ `New<FTransTexture>(GMem)` calls.
///
/// # Safety
/// `pts` must be valid for `local_count` reads and, when the result is > 2, for
/// `result` writes. Every entry read must point to a live `FTransTexture`.
/// `local_count` must be at most `CLIP_CAPACITY`; the C++ scratch overflows
/// beyond that, so the Rust side panics instead.
pub unsafe fn clip_polygon(
    frame: &ParticleFrame,
    pts: *mut *mut FTransTexture,
    local_count: i32,
    code: u8,
    alloc: &mut dyn FnMut() -> *mut FTransTexture,
) -> i32 {
    let n = local_count.max(0) as usize;
    assert!(
        n <= CLIP_CAPACITY,
        "clip_polygon: {n} vertices exceed the {CLIP_CAPACITY}-entry scratch"
    );

    let mut bufs = [[std::ptr::null_mut::<FTransTexture>(); CLIP_CAPACITY]; 2];
    for (p, slot) in bufs[0][..n].iter_mut().enumerate() {
        // SAFETY: `p < n <= local_count`, readable per the function contract.
        *slot = unsafe { *pts.add(p) };
    }

    // Plane passes, in C++ order. Near is exclusive; the edge planes follow the bit order.
    let mut passes = [ClipPlane::Near; 5];
    let mut npass = 0usize;
    if code & FVF_OUT_NEAR != 0 {
        passes = [
            ClipPlane::Near,
            ClipPlane::Left,
            ClipPlane::Top,
            ClipPlane::Right,
            ClipPlane::Bottom,
        ];
        npass = 5;
    } else {
        for (bit, plane) in [
            (FVF_OUT_XMIN, ClipPlane::Left),
            (FVF_OUT_YMIN, ClipPlane::Top),
            (FVF_OUT_XMAX, ClipPlane::Right),
            (FVF_OUT_YMAX, ClipPlane::Bottom),
        ] {
            if code & bit != 0 {
                passes[npass] = plane;
                npass += 1;
            }
        }
    }

    let mut cur = 0usize; // Index of the buffer holding the current input (`i` in C++).
    let mut count = n;
    for plane in &passes[..npass] {
        let (lo, hi) = bufs.split_at_mut(1);
        let (input, output) = if cur == 0 {
            (&lo[0], &mut hi[0])
        } else {
            (&hi[0], &mut lo[0])
        };
        // SAFETY: entries in `input[..count]` were set from `pts` or by `clip`.
        count = unsafe { clip(*plane, frame, &input[..count], output, alloc) };
        cur ^= 1;
    }

    if count > 2 {
        for (p, &vert) in bufs[cur][..count].iter().enumerate() {
            // SAFETY: the caller guarantees room for `count` entries.
            unsafe { *pts.add(p) = vert };
        }
        return count as i32;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::FTransform;
    use hp2_abi::FPlane;

    fn vert(x: f32, y: f32, z: f32) -> FTransTexture {
        FTransTexture {
            transform: FTransform {
                point: FVector { x, y, z },
                flags: 0,
                screen_x: 0.0,
                screen_y: 0.0,
                int_y: 0,
                rz: 0.0,
            },
            normal: FPlane {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            light: FPlane {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            fog: FPlane {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            u: 0.0,
            v: 0.0,
        }
    }

    fn frame(proj_z: f32, fx15: f32, fy15: f32, edge: f32) -> ParticleFrame {
        ParticleFrame {
            proj_z,
            rproj_z: 1.0 / proj_z,
            fx15,
            fy15,
            prjxm: edge,
            prjxp: edge,
            prjym: edge,
            prjyp: edge,
        }
    }

    #[test]
    fn project_divides_and_truncates_screen_y() {
        let f = frame(2.0, 100.0, 50.0, 1.0);
        let mut tt = vert(1.0, 2.0, 4.0);
        project(&f, &mut tt);
        assert_eq!(tt.transform.rz, 0.5);
        assert_eq!(tt.transform.screen_x, 100.5);
        assert_eq!(tt.transform.screen_y, 51.0);
        assert_eq!(tt.transform.int_y, 51);

        // ScreenY = -1.25: truncation toward zero gives -1, not floor's -2.
        let f = frame(2.0, 0.0, 0.0, 1.0);
        let mut tt = vert(0.0, -2.5, 4.0);
        project(&f, &mut tt);
        assert_eq!(tt.transform.screen_y, -1.25);
        assert_eq!(tt.transform.int_y, -1);
    }

    #[test]
    fn compute_clip_codes_sets_one_bit_per_side() {
        let f = frame(1.0, 0.0, 0.0, 1.0);
        let mut left = vert(-2.0, 0.0, 1.0);
        compute_clip_codes(&f, &mut left);
        assert_eq!(left.transform.flags, FVF_OUT_XMIN);

        let mut right = vert(2.0, 0.0, 1.0);
        compute_clip_codes(&f, &mut right);
        assert_eq!(right.transform.flags, FVF_OUT_XMAX);

        let mut inside = vert(0.0, 0.0, 1.0);
        compute_clip_codes(&f, &mut inside);
        assert_eq!(inside.transform.flags, 0);
    }

    #[test]
    fn clip_polygon_passes_through_with_no_planes() {
        let f = frame(2.0, 0.0, 0.0, 1.0);
        let mut a = vert(0.0, 0.0, 1.0);
        let mut b = vert(1.0, 0.0, 1.0);
        let mut c = vert(0.0, 1.0, 1.0);
        let mut pts = [
            &mut a as *mut _,
            &mut b as *mut _,
            &mut c as *mut _,
            std::ptr::null_mut(),
        ];
        let mut allocs = 0;
        let mut alloc = || -> *mut FTransTexture {
            allocs += 1;
            std::ptr::null_mut()
        };
        let r = unsafe { clip_polygon(&f, pts.as_mut_ptr(), 3, 0, &mut alloc) };
        assert_eq!(r, 3);
        assert_eq!(allocs, 0);
        assert_eq!(pts[0], &mut a as *mut _);
        assert_eq!(pts[2], &mut c as *mut _);
    }

    #[test]
    fn clip_polygon_near_crossings_allocate_in_order() {
        // A(z=2), B(z=0), C(z=2). Near is z - 1, so A and C are inside, B is outside.
        let f = frame(2.0, 0.0, 0.0, 1.0);
        let mut a = vert(0.0, 0.0, 2.0);
        let mut b = vert(0.0, 0.0, 0.0);
        let mut c = vert(0.0, 0.0, 2.0);
        let mut pool = [vert(0.0, 0.0, 0.0), vert(0.0, 0.0, 0.0)];
        let base = pool.as_mut_ptr();
        let mut next = 0usize;
        let mut alloc = || {
            let p = unsafe { base.add(next) };
            next += 1;
            p
        };
        let mut pts = [
            &mut a as *mut _,
            &mut b as *mut _,
            &mut c as *mut _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ];
        let r = unsafe { clip_polygon(&f, pts.as_mut_ptr(), 3, FVF_OUT_NEAR, &mut alloc) };
        assert_eq!(r, 4);
        assert_eq!(next, 2);
        assert_eq!(pts[0], &mut a as *mut _);
        assert_eq!(pts[1], base);
        assert_eq!(pts[2], unsafe { base.add(1) });
        assert_eq!(pts[3], &mut c as *mut _);
        // Crossing points sit at z = 1, half way along each edge.
        let n1 = unsafe { &*pts[1] };
        let n2 = unsafe { &*pts[2] };
        assert_eq!(n1.transform.point.z, 1.0);
        assert_eq!(n2.transform.point.z, 1.0);
        // Projected with proj_z = 2 at z = 1: RZ = 2.
        assert_eq!(n1.transform.rz, 2.0);
    }

    #[test]
    fn clip_polygon_left_plane_interpolates_crossings() {
        // Left distance is PrjXM*Z + X with PrjXM = 1.
        // A(-2,0,1) and B(0,0,1) lie on opposite sides with C(0,1,1) inside.
        let f = frame(2.0, 0.0, 0.0, 1.0);
        let mut a = vert(-2.0, 0.0, 1.0);
        let mut b = vert(0.0, 0.0, 1.0);
        let mut c = vert(0.0, 1.0, 1.0);
        let mut pool = [vert(0.0, 0.0, 0.0), vert(0.0, 0.0, 0.0)];
        let base = pool.as_mut_ptr();
        let mut next = 0usize;
        let mut alloc = || {
            let p = unsafe { base.add(next) };
            next += 1;
            p
        };
        let mut pts = [
            &mut a as *mut _,
            &mut b as *mut _,
            &mut c as *mut _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ];
        let r = unsafe { clip_polygon(&f, pts.as_mut_ptr(), 3, FVF_OUT_XMIN, &mut alloc) };
        assert_eq!(r, 4);
        assert_eq!(next, 2);
        assert_eq!(pts[0], base);
        assert_eq!(pts[1], unsafe { base.add(1) });
        assert_eq!(pts[2], &mut b as *mut _);
        assert_eq!(pts[3], &mut c as *mut _);
        let n1 = unsafe { &*pts[0] };
        let n2 = unsafe { &*pts[1] };
        assert_eq!((n1.transform.point.x, n1.transform.point.y), (-1.0, 0.5));
        assert_eq!((n2.transform.point.x, n2.transform.point.y), (-1.0, 0.0));
    }

    #[test]
    fn clip_polygon_under_three_vertices_returns_zero_and_keeps_pts() {
        let f = frame(2.0, 0.0, 0.0, 1.0);
        let mut a = vert(0.0, 0.0, 2.0);
        let mut b = vert(0.0, 0.0, 0.0);
        let mut pts = [&mut a as *mut _, &mut b as *mut _];
        let mut allocs = 0;
        let mut alloc = || -> *mut FTransTexture {
            allocs += 1;
            std::ptr::null_mut()
        };
        let r = unsafe { clip_polygon(&f, pts.as_mut_ptr(), 2, FVF_OUT_NEAR, &mut alloc) };
        assert_eq!(r, 0);
        assert_eq!(allocs, 0);
        assert_eq!(pts[0], &mut a as *mut _);
        assert_eq!(pts[1], &mut b as *mut _);
    }
}
