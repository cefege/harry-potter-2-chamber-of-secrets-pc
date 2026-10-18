//! Port of the model-level logic in `Engine/Src/UnModel.cpp`. The C++ shell keeps
//! the `TArray`/`TTransArray` mutation (`Empty`, `Shrink`, `AddItem`), the
//! transaction hooks (`ModifyItem`), object allocation, and serialization; those
//! reach Rust through [`ModelServices`] or stay in C++.

use std::ffi::c_void;

use hp2_abi::engine::generated::FPoly;
use hp2_abi::render_scene::{FBspNode, FBspSurf, FZoneProperties};
use hp2_abi::{FCoords, FPlane, FVector};

use crate::fpoly;
use crate::math::app_sqrt;
use crate::vec;

/// `PF_Selected` (`Engine/Inc/UnObj.h`): editor poly selection flag.
pub const PF_SELECTED: u32 = 0x0200_0000;

/// `FBox` value as the Rust side computes it. The C++ shell copies it into its
/// `FBox` member field by field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub min: FVector,
    pub max: FVector,
    pub is_valid: bool,
}

/// `FBox(0)`: zeroed corners, invalid.
pub fn empty_bound() -> Bound {
    Bound {
        min: vec::zero(),
        max: vec::zero(),
        is_valid: false,
    }
}

/// `::Min(A, B)` from `UnTemplate.h`: `(A<=B) ? A : B`.
fn min(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `::Max(A, B)` from `UnTemplate.h`: `(A>=B) ? A : B`.
fn max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `FBox::operator+=(const FVector&)`: the first point seeds both corners.
fn bound_add(b: &mut Bound, p: FVector) {
    if b.is_valid {
        b.min = FVector {
            x: min(b.min.x, p.x),
            y: min(b.min.y, p.y),
            z: min(b.min.z, p.z),
        };
        b.max = FVector {
            x: max(b.max.x, p.x),
            y: max(b.max.y, p.y),
            z: max(b.max.z, p.z),
        };
    } else {
        b.min = p;
        b.max = p;
        b.is_valid = true;
    }
}

/// `FBox(const FVector* Points, INT Count)` over the points in iteration order.
pub fn bound_of_points(points: impl IntoIterator<Item = FVector>) -> Bound {
    let mut b = empty_bound();
    for p in points {
        bound_add(&mut b, p);
    }
    b
}

/// `FSphere(const FVector* Pts, INT Count)`. Returns `FPlane(0,0,0,0)` when no
/// points are given. Otherwise the center is the box center and the radius is the
/// farthest point distance, widened by 1.001 in DOUBLE before narrowing.
pub fn sphere_of_points<I>(points: I) -> FPlane
where
    I: Iterator<Item = FVector> + Clone,
{
    let bx = bound_of_points(points.clone());
    if !bx.is_valid {
        return FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
    }
    let center = vec::div(vec::add(bx.min, bx.max), 2.0);
    let mut w = 0.0f32;
    for p in points {
        // `FDistSquared(Pts[i], *this)`: V2 - V1 with V2 the sphere center. The
        // original object is not FMA-contracted here (fixture-verified).
        let d = vec::sub(center, p);
        let dist = d.x * d.x + d.y * d.y + d.z * d.z;
        if dist > w {
            w = dist;
        }
    }
    // `W = appSqrt(W) * 1.001f`: `appSqrt` is DOUBLE, so the multiply is DOUBLE.
    w = (app_sqrt(f64::from(w)) * f64::from(1.001f32)) as f32;
    FPlane {
        x: center.x,
        y: center.y,
        z: center.z,
        w,
    }
}

/// `UModel::BuildBound` for the polygon list. Returns the box and, when there are
/// polygons, the sphere. With no polygons the C++ keeps `BoundingSphere` untouched.
pub fn build_bound(polys: &[FPoly]) -> (Bound, Option<FPlane>) {
    if polys.is_empty() {
        return (empty_bound(), None);
    }
    // Vertices in polygon order, then vertex order, as the C++ `Points` TArray.
    let points = polys.iter().flat_map(|p| {
        let n = usize::try_from(p.num_vertices).unwrap_or(0);
        p.vertex[..n].iter().copied()
    });
    let bx = bound_of_points(points.clone());
    (bx, Some(sphere_of_points(points)))
}

/// `UModel::PotentiallyVisible`: "the amazing superfast patent-pending 1 cpu cycle
/// potential visibility algorithm". Always visible.
pub fn potentially_visible(_leaf1: i32, _leaf2: i32) -> i32 {
    1
}

/// Services the surface decisions call back into. They are C++ `TTransArray` and
/// `UModel` operations (transaction hooks and `TArray::AddItem`), not math.
pub trait ModelServices {
    /// `UModel::Surfs.ModifyItem(index)` on the owning model.
    fn surf_modify_item(&mut self, index: i32);
    /// `Surf.Actor->Brush->Polys->Element.ModifyItem(iBrushPoly)`.
    fn brush_poly_modify_item(&mut self, actor: *mut c_void, index: i32);
    /// `Surfs(surf).Nodes.AddItem(node)` on the owning model.
    fn surf_add_node(&mut self, surf: i32, node: i32);
}

/// `UModel::ModifySurf(Index, UpdateMaster)`. Indices outside `surfs` are a C++
/// out-of-bounds read (`checkSlow` is compiled out), so the port skips the
/// master update for them, after the transaction hook has run.
pub fn modify_surf(
    svc: &mut impl ModelServices,
    surfs: &[FBspSurf],
    index: i32,
    update_master: i32,
) {
    svc.surf_modify_item(index);
    let Some(surf) = usize::try_from(index).ok().and_then(|i| surfs.get(i)) else {
        return;
    };
    if update_master != 0 && !surf.actor.is_null() {
        svc.brush_poly_modify_item(surf.actor, surf.i_brush_poly);
    }
}

/// `UModel::ModifyAllSurfs(UpdateMaster)`.
pub fn modify_all_surfs(svc: &mut impl ModelServices, surfs: &[FBspSurf], update_master: i32) {
    for i in 0..surfs.len() {
        modify_surf(svc, surfs, i as i32, update_master);
    }
}

/// `UModel::ModifySelectedSurfs(UpdateMaster)`: only surfaces with `PF_Selected`.
pub fn modify_selected_surfs(svc: &mut impl ModelServices, surfs: &[FBspSurf], update_master: i32) {
    for (i, surf) in surfs.iter().enumerate() {
        if surf.poly_flags & PF_SELECTED != 0 {
            modify_surf(svc, surfs, i as i32, update_master);
        }
    }
}

/// `UModel::PostLoad`'s loop: each node registers itself with its surface.
pub fn post_load_nodes(svc: &mut impl ModelServices, nodes: &[FBspNode]) {
    for (i, node) in nodes.iter().enumerate() {
        svc.surf_add_node(node.i_surf, i as i32);
    }
}

/// `EmptyModel`'s zone reset: every zone is its own connectivity bit, sees all
/// zones, and has no zone actor. `LastRenderTime` is left as it was.
pub fn reset_zones(zones: &mut [FZoneProperties]) {
    debug_assert!(zones.len() <= 64, "connectivity holds 64 zone bits");
    for (i, zone) in zones.iter_mut().enumerate() {
        zone.zone_actor = std::ptr::null_mut();
        zone.connectivity = 1u64 << i;
        zone.visibility = !0u64;
    }
}

/// `UModel::Transform`'s per-polygon loop: `Polys->Element(i).Transform(Coords,
/// PrePivot, Location, Orientation)` for every polygon in order.
pub fn transform_polys(
    polys: &mut [FPoly],
    point_xform: &FCoords,
    vector_xform: &FCoords,
    pre: FVector,
    post: FVector,
    orientation: f32,
) {
    for poly in polys {
        fpoly::transform(poly, point_xform, vector_xform, pre, post, orientation);
    }
}
