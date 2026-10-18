//! Pure math of `FDynamicChunk::Filter` (split and filter-down) and of the
//! decision arithmetic in `FDynamicLight::Filter` (`Render/Src/UnSprite.cpp`).
//!
//! The C++ side keeps the BSP node/frame lookups, `GDynMem`/`GMem` allocation
//! in the original order, the `new(GDynMem)` chunk and light constructions,
//! the engine calls and the planar loop.

pub use hp2_abi::render_particle::FTransform;
pub use hp2_abi::render_span::FRasterSpan;
pub use hp2_abi::render_sprite_filter::FRasterPolyHeader;
pub use hp2_abi::{FPlane, FVector};

/// `INDEX_NONE` (`Core/Inc/UnObj.h`).
pub const INDEX_NONE: i32 = -1;

/// `FLOAT Dist` vs the double literal `0.01`: the float is promoted to double.
const SIDE_EPS: f64 = 0.01;

/// `a*b + c` contracted to an fma on arm64 (`-ffp-contract=on`), as in particle.rs.
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

/// `appFloor` and `appRound` on the non-PSX path of `Core/Inc/UnGnuG.h`:
/// both are `static_cast<INT>(f)` (truncation toward zero).
#[inline]
pub fn app_floor(f: f32) -> i32 {
    f as i32
}

/// `appRound`, same non-PSX truncation as `appFloor` in `Core/Inc/UnGnuG.h`.
#[inline]
pub fn app_round(f: f32) -> i32 {
    f as i32
}

/// `FPlane::PlaneDot(P) = X*P.X + Y*P.Y + Z*P.Z - W` (`Core/Inc/UnMath.h`).
/// Contracted left-to-right: `fma(Z, P.Z, fma(X, P.X, Y*P.Y)) - W`.
#[inline]
pub fn plane_dot(plane: &FPlane, p: &FVector) -> f32 {
    madd(plane.z, p.z, madd(plane.x, p.x, plane.y * p.y)) - plane.w
}

/// Result of classifying the four proxy verts against a node plane.
#[derive(Clone, Copy, Debug)]
pub struct Crossings {
    /// `Dist[i]` = `Node.Plane.PlaneDot(ProxyVerts[i].Point)`.
    pub dist: [f32; 4],
    /// Count of `Dist[i] > +0.01`.
    pub front: i32,
    /// Count of `Dist[i] < -0.01`.
    pub back: i32,
    /// Number of edge crossings (`NumInt`); only filled when `front != 0 && back != 0`.
    pub num: usize,
    /// `Intersect[i]` as `(ScreenX, ScreenY)`, in the order the edges were found.
    pub points: [(f32, f32); 4],
}

/// Classify the four verts and find the edge intersections.
pub fn crossings(verts: &[FTransform; 4], plane: &FPlane) -> Crossings {
    let mut dist = [0.0f32; 4];
    let mut front = 0;
    let mut back = 0;
    for i in 0..4 {
        dist[i] = plane_dot(plane, &verts[i].point);
        if (dist[i] as f64) > SIDE_EPS {
            front += 1;
        }
        if (dist[i] as f64) < -SIDE_EPS {
            back += 1;
        }
    }

    let mut points = [(0.0f32, 0.0f32); 4];
    let mut num = 0;
    if front != 0 && back != 0 {
        // Edges are (prev, cur) = (3,0), (0,1), (1,2), (2,3).
        for i in 0..4 {
            let prev = (i + 3) % 4;
            let d1 = dist[prev];
            let d2 = dist[i];
            if (d1 * d2) < 0.0 {
                let alpha = d1 / (d1 - d2);
                let v1 = &verts[prev];
                let v2 = &verts[i];
                points[num] = (
                    madd(alpha, v2.screen_x - v1.screen_x, v1.screen_x),
                    madd(alpha, v2.screen_y - v1.screen_y, v1.screen_y),
                );
                num += 1;
            }
        }
    }

    Crossings {
        dist,
        front,
        back,
        num,
        points,
    }
}

/// Byte size `FDynamicChunk::Filter` requests for one raster:
/// `sizeof(FRasterPoly) + (EndY - StartY) * sizeof(FRasterSpan)`.
#[inline]
pub fn raster_alloc_bytes(src_start_y: i32, src_end_y: i32) -> usize {
    size_of::<FRasterPolyHeader>()
        + (src_end_y.wrapping_sub(src_start_y) as usize) * size_of::<FRasterSpan>()
}

/// Whether the split path is entered, which is where the C++ allocates its two
/// rasters. The allocation happens before the `Y0 > Y1` test, so it can precede a
/// `NoSplit` result.
#[inline]
pub fn split_allocates(c: &Crossings) -> bool {
    c.front != 0 && c.back != 0 && c.num >= 2
}

/// Source raster: `FRasterPoly` header values and its span array.
pub struct Source<'a> {
    pub start_y: i32,
    pub end_y: i32,
    pub lines: &'a [FRasterSpan],
}

/// One output raster: header plus the span array that follows it.
pub struct Buf<'a> {
    pub header: &'a mut FRasterPolyHeader,
    pub lines: &'a mut [FRasterSpan],
}

/// Which raster pointer the C++ passes on: a freshly split buffer, the source
/// raster (`NoSplit`), or none (discarded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raster {
    Null,
    Front,
    Back,
    Source,
}

/// Filter-down action for one side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing to construct.
    None,
    /// `new(GDynMem)FDynamicChunk(iChild, Sprite, Raster)`.
    Chunk,
    /// `new(GDynMem)FDynamicFinalChunk(iNode, Sprite, Raster, is_back)`.
    Final,
}

/// Outcome of `FDynamicChunk::Filter`, minus the engine-side construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkFilter {
    pub front: Raster,
    pub back: Raster,
    pub front_action: Action,
    pub back_action: Action,
}

/// Port of `FDynamicChunk::Filter` from the `Dist` classification through the
/// filter-down decision. `bufs` must be `Some([front, back])` whenever
/// [`split_allocates`] holds (the C++ allocates them exactly then).
#[allow(clippy::too_many_arguments)]
pub fn filter_chunk(
    verts: &[FTransform; 4],
    plane: &FPlane,
    src: &Source,
    bufs: Option<[Buf; 2]>,
    i_front: i32,
    i_back: i32,
    outside: bool,
    csg: bool,
) -> ChunkFilter {
    let c = crossings(verts, plane);

    let (front, back) = if split_allocates(&c) {
        let mut bufs =
            bufs.expect("front/back rasters are allocated whenever the split path is entered");
        split_into(&c, src, &mut bufs).unwrap_or((Raster::Source, Raster::Source))
    } else {
        (Raster::Source, Raster::Source)
    };

    let front_action = if c.front != 0 && front != Raster::Null {
        if i_front != INDEX_NONE {
            Action::Chunk
        } else if outside || csg {
            Action::Final
        } else {
            Action::None
        }
    } else {
        Action::None
    };

    let back_action = if c.back != 0 && back != Raster::Null {
        if i_back != INDEX_NONE {
            Action::Chunk
        } else if outside && !csg {
            Action::Final
        } else {
            Action::None
        }
    } else {
        Action::None
    };

    ChunkFilter {
        front,
        back,
        front_action,
        back_action,
    }
}

/// The split body of `FDynamicChunk::Filter`. Index 0 of `bufs` is the front raster,
/// index 1 the back raster. Returns `None` for `NoSplit` (`Y0 > Y1`), before any
/// header or span is written.
#[allow(clippy::explicit_counter_loop)] // counters mirror the C pointer walks
fn split_into(c: &Crossings, src: &Source, bufs: &mut [Buf; 2]) -> Option<(Raster, Raster)> {
    const FRONT: usize = 0;
    const BACK: usize = 1;

    let mut pts = c.points;
    if pts[0].1 > pts[1].1 {
        pts.swap(0, 1);
    }

    let mut y0 = app_floor(pts[0].1).max(src.start_y);
    let mut y1 = app_floor(pts[1].1).min(src.end_y);
    if y0 > y1 {
        return None;
    }

    let top = if y0 > src.start_y {
        Some(if c.dist[0] >= 0.0 { FRONT } else { BACK })
    } else {
        None
    };
    let bottom = if y1 < src.end_y {
        Some(if c.dist[2] >= 0.0 { FRONT } else { BACK })
    } else {
        None
    };

    let (left, right) = if pts[1].0 >= pts[0].0 {
        if c.dist[1] >= 0.0 {
            (BACK, FRONT)
        } else {
            (FRONT, BACK)
        }
    } else if c.dist[0] >= 0.0 {
        (FRONT, BACK)
    } else {
        (BACK, FRONT)
    };

    for &b in &[left, right] {
        bufs[b].header.start_y = y0;
        bufs[b].header.end_y = y1;
    }

    if let Some(t) = top {
        bufs[t].header.start_y = src.start_y;
        let n = (y0 - src.start_y) as usize;
        bufs[t].lines[..n].copy_from_slice(&src.lines[..n]);
    }

    if let Some(b) = bottom {
        bufs[b].header.end_y = src.end_y;
        let src_off = (y1 - src.start_y) as usize;
        let dst_off = (y1 - bufs[b].header.start_y) as usize;
        let n = (bufs[b].header.end_y - y1) as usize;
        bufs[b].lines[dst_off..dst_off + n].copy_from_slice(&src.lines[src_off..src_off + n]);
    }

    if y1 != y0 {
        let float_y_adjust = (y0 as f32) + 1.0 - pts[0].1;
        let float_fix_dx = 65536.0f32 * (pts[1].0 - pts[0].0) / (pts[1].1 - pts[0].1);
        let fix_dx = app_round(float_fix_dx);
        let mut fix_x = app_round(madd(65536.0, pts[0].0, float_fix_dx * float_y_adjust));

        if src.start_y > y0 {
            fix_x = fix_x.wrapping_add((src.start_y - y0).wrapping_mul(fix_dx));
            y0 = src.start_y;
        }
        if src.end_y < y1 {
            y1 = src.end_y;
        }

        let mut s = (y0 - src.start_y) as usize;
        let mut l = (y0 - bufs[left].header.start_y) as usize;
        let mut r = (y0 - bufs[right].header.start_y) as usize;
        let steps = (y1 - y0).max(0);
        for _ in 0..steps {
            let source = src.lines[s];
            bufs[left].lines[l] = source;
            bufs[right].lines[r] = source;
            let x = fix_x >> 16;
            if x < bufs[left].lines[l].x[1] {
                bufs[left].lines[l].x[1] = x;
            }
            if x > bufs[right].lines[r].x[0] {
                bufs[right].lines[r].x[0] = x;
            }
            fix_x = fix_x.wrapping_add(fix_dx);
            s += 1;
            l += 1;
            r += 1;
        }
    }

    let front_pick = if bufs[FRONT].header.end_y <= bufs[FRONT].header.start_y {
        Raster::Null
    } else {
        Raster::Front
    };
    let back_pick = if bufs[BACK].header.end_y <= bufs[BACK].header.start_y {
        Raster::Null
    } else {
        Raster::Back
    };
    Some((front_pick, back_pick))
}

/// `FDynamicLight::Filter` bit: `Dist > -Radius`.
pub const LIGHT_FRONT_ENTER: u8 = 1 << 0;
/// `FDynamicLight::Filter` bit: `IsVol && Dist > -WorldVolumetricRadius()`.
pub const LIGHT_FRONT_VOLUMETRIC: u8 = 1 << 1;
/// `FDynamicLight::Filter` bit: `Dist < Radius` (also gates the planar loop).
pub const LIGHT_BACK_ENTER: u8 = 1 << 2;
/// `FDynamicLight::Filter` bit: `IsVol && Dist < WorldVolumetricRadius()`.
pub const LIGHT_BACK_VOLUMETRIC: u8 = 1 << 3;

/// Decision arithmetic of `FDynamicLight::Filter`: the distance of the actor's
/// location from the node plane compared against the light radii.
#[inline]
pub fn light_flags(
    plane: &FPlane,
    location: &FVector,
    radius: f32,
    vol_radius: f32,
    is_vol: bool,
) -> u8 {
    let dist = plane_dot(plane, location);
    let mut flags = 0u8;
    if dist > -radius {
        flags |= LIGHT_FRONT_ENTER;
    }
    if is_vol && dist > -vol_radius {
        flags |= LIGHT_FRONT_VOLUMETRIC;
    }
    if dist < radius {
        flags |= LIGHT_BACK_ENTER;
    }
    if is_vol && dist < vol_radius {
        flags |= LIGHT_BACK_VOLUMETRIC;
    }
    flags
}
