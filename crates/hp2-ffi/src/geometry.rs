//! FPoly and UModel geometry exports (`crates/hp2-engine::fpoly`, `::model`).
//! The C++ `FPoly` methods in `Engine/Src/UnFPoly.cpp` and `UModel::Transform`
//! in `Engine/Src/UnModel.cpp` forward here. Polygons cross as `FPoly*`, vectors
//! and planes as `FVector*`/`FPlane*`, coords as `FCoords*` (layouts asserted in
//! `hp2-abi`). Warnings go through [`Hp2GeometryServices::warn`]. Fatal errors
//! are written NUL-terminated into the caller's `err` buffer; the C++ shell
//! raises them with `appErrorf`, so no Rust panic or C++ exception crosses the
//! boundary.

use std::ffi::{CString, c_char, c_void};

use hp2_abi::engine::generated::{FBox, FPoly};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FZoneProperties};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::fpoly::{self, Log};
use hp2_engine::model::{self, ModelServices};
use hp2_ffi_support::guard;

/// Per-slice services for the geometry exports, mirrored by
/// `Engine/Inc/Hp2GeometryServices.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2GeometryServices {
    /// `debugf(NAME_Warning, TEXT("%s"), ANSI_TO_TCHAR(Msg))`. `msg` is
    /// NUL-terminated ANSI text, valid only for the duration of the call.
    pub warn: unsafe extern "C" fn(msg: *const c_char),
    /// `UModel::Surfs.ModifyItem(index)`: the transaction hook for one surface.
    pub surf_modify_item: unsafe extern "C" fn(model: *mut c_void, index: i32),
    /// `ABrush::Brush->Polys->Element.ModifyItem(index)` for a surface's brush actor.
    pub brush_poly_modify_item: unsafe extern "C" fn(actor: *mut c_void, index: i32),
    /// `UModel::Surfs(surf).Nodes.AddItem(node)`.
    pub surf_add_node: unsafe extern "C" fn(model: *mut c_void, surf: i32, node: i32),
}

/// Routes warnings to the C++ `warn` callback. A null table drops them.
struct Warn(*const Hp2GeometryServices);

impl Log for Warn {
    fn warn(&mut self, msg: &str) {
        // SAFETY: the caller passes the live constant table (or null).
        let Some(svc) = (unsafe { self.0.as_ref() }) else {
            return;
        };
        let text = CString::new(msg.replace('\0', "")).unwrap_or_default();
        // SAFETY: `text` is NUL-terminated and outlives the call.
        unsafe { (svc.warn)(text.as_ptr()) };
    }
}

/// Writes `msg` NUL-terminated into the caller's buffer (at most `cap - 1` bytes).
///
/// # Safety
/// `err`, when non-null, must be valid for writes of `cap` bytes.
unsafe fn write_err(err: *mut c_char, cap: i32, msg: &str) {
    let Ok(cap) = usize::try_from(cap) else {
        return;
    };
    if err.is_null() || cap == 0 {
        return;
    }
    let n = msg.len().min(cap - 1);
    let out = err.cast::<u8>();
    // SAFETY: `n + 1 <= cap`, so both writes stay inside the caller's buffer.
    unsafe {
        std::ptr::copy_nonoverlapping(msg.as_ptr(), out, n);
        *out.add(n) = 0;
    }
}

/// Clears the caller's error buffer on entry, so an unset error is never read as
/// garbage: the C++ shell raises `appErrorf` only when `err[0] != 0`.
///
/// # Safety
/// `err`, when non-null and `cap > 0`, must be valid for writes of at least one byte.
unsafe fn clear_err(err: *mut c_char, cap: i32) {
    if !err.is_null() && cap > 0 {
        // SAFETY: `cap > 0`, so index 0 is inside the caller's buffer.
        unsafe { *err = 0 };
    }
}

/// # Safety
/// `p`, when non-null, must point to a live `FPoly`.
unsafe fn poly_mut<'a>(p: *mut c_void) -> Option<&'a mut FPoly> {
    unsafe { p.cast::<FPoly>().as_mut() }
}

/// # Safety
/// `p`, when non-null, must point to a live `FPoly`.
unsafe fn poly_ref<'a>(p: *const c_void) -> Option<&'a FPoly> {
    unsafe { p.cast::<FPoly>().as_ref() }
}

/// # Safety
/// `p`, when non-null, must point to a live `FVector`.
unsafe fn vec_at(p: *const c_void) -> Option<FVector> {
    unsafe { p.cast::<FVector>().as_ref().copied() }
}

/// # Safety
/// `p`, when non-null, must point to a live `FPlane`.
unsafe fn plane_at(p: *const c_void) -> Option<FPlane> {
    unsafe { p.cast::<FPlane>().as_ref().copied() }
}

/// # Safety
/// `p`, when non-null, must point to a live `FCoords`.
unsafe fn coords_at<'a>(p: *const c_void) -> Option<&'a FCoords> {
    unsafe { p.cast::<FCoords>().as_ref() }
}

/// `FPoly::Reverse()`.
///
/// # Safety
/// `poly` must be a live `FPoly` (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_reverse(poly: *mut c_void) {
    guard("hp2_rs_engine_fpoly_reverse", (), move || unsafe {
        if let Some(p) = poly_mut(poly) {
            fpoly::reverse(p);
        }
    })
}

/// `FPoly::Fix()`: returns the vertex count after removing coincident points.
///
/// # Safety
/// `services` must be a live `Hp2GeometryServices` (or null); `poly` a live `FPoly` (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_fix(
    services: *const Hp2GeometryServices,
    poly: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_fpoly_fix", 0, move || unsafe {
        match poly_mut(poly) {
            Some(p) => fpoly::fix(p, &mut Warn(services)),
            None => 0,
        }
    })
}

/// `FPoly::Area()`.
///
/// # Safety
/// `poly` must be a live `FPoly` (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_area(poly: *const c_void) -> f32 {
    guard("hp2_rs_engine_fpoly_area", 0.0, move || unsafe {
        poly_ref(poly).map_or(0.0, fpoly::area)
    })
}

/// `FPoly::CalcNormal(bSilent)`: returns 0 on success, 1 on a zero-area polygon.
///
/// # Safety
/// `services` must be a live `Hp2GeometryServices` (or null); `poly` a live `FPoly` (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_calc_normal(
    services: *const Hp2GeometryServices,
    poly: *mut c_void,
    bsilent: i32,
) -> i32 {
    guard("hp2_rs_engine_fpoly_calc_normal", 1, move || unsafe {
        match poly_mut(poly) {
            Some(p) => fpoly::calc_normal(p, bsilent != 0, &mut Warn(services)),
            None => 1,
        }
    })
}

/// `FPoly::Finalize(NoError)`. A fatal failure writes its message to `err`
/// and returns 0; the shell raises it.
///
/// # Safety
/// `services` must be a live `Hp2GeometryServices` (or null); `poly` a live
/// `FPoly` (or null); `err` valid for `err_cap` bytes (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_finalize(
    services: *const Hp2GeometryServices,
    poly: *mut c_void,
    no_error: i32,
    err: *mut c_char,
    err_cap: i32,
) -> i32 {
    unsafe { clear_err(err, err_cap) };
    guard("hp2_rs_engine_fpoly_finalize", 0, move || unsafe {
        let Some(p) = poly_mut(poly) else {
            return 0;
        };
        match fpoly::finalize(p, no_error != 0, &mut Warn(services)) {
            Ok(r) => r,
            Err(msg) => {
                write_err(err, err_cap, &msg);
                0
            }
        }
    })
}

/// `FPoly::SplitWithPlane(PlaneBase, PlaneNormal, FrontPoly, BackPoly, VeryPrecise)`.
/// `front`/`back` both null is the status-only query. Overflow writes to `err`.
///
/// # Safety
/// `services` must be a live `Hp2GeometryServices` (or null); `poly` a live `FPoly`;
/// `base`/`normal` live `FVector`s; `front`/`back` live `FPoly`s or both null;
/// `err` valid for `err_cap` bytes (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_split_with_plane(
    services: *const Hp2GeometryServices,
    poly: *const c_void,
    base: *const c_void,
    normal: *const c_void,
    front: *mut c_void,
    back: *mut c_void,
    very_precise: i32,
    err: *mut c_char,
    err_cap: i32,
) -> i32 {
    unsafe { clear_err(err, err_cap) };
    guard("hp2_rs_engine_fpoly_split_with_plane", 0, move || unsafe {
        let (Some(p), Some(base), Some(normal)) = (poly_ref(poly), vec_at(base), vec_at(normal))
        else {
            return 0;
        };
        let out = match (poly_mut(front), poly_mut(back)) {
            (Some(f), Some(b)) => Some((f, b)),
            _ => None,
        };
        match fpoly::split_with_plane(p, base, normal, out, very_precise != 0, &mut Warn(services))
        {
            Ok(r) => r,
            Err(msg) => {
                write_err(err, err_cap, &msg);
                0
            }
        }
    })
}

/// `FPoly::SplitWithPlaneFast(Plane, FrontPoly, BackPoly)`. `front`/`back` both
/// null is the status-only query. Overflow writes to `err`.
///
/// # Safety
/// `poly` a live `FPoly`; `plane` a live `FPlane`; `front`/`back` live `FPoly`s
/// or both null; `err` valid for `err_cap` bytes (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_split_with_plane_fast(
    poly: *const c_void,
    plane: *const c_void,
    front: *mut c_void,
    back: *mut c_void,
    err: *mut c_char,
    err_cap: i32,
) -> i32 {
    unsafe { clear_err(err, err_cap) };
    guard(
        "hp2_rs_engine_fpoly_split_with_plane_fast",
        0,
        move || unsafe {
            let (Some(p), Some(plane)) = (poly_ref(poly), plane_at(plane)) else {
                return 0;
            };
            let out = match (poly_mut(front), poly_mut(back)) {
                (Some(f), Some(b)) => Some((f, b)),
                _ => None,
            };
            match fpoly::split_with_plane_fast(p, plane, out) {
                Ok(r) => r,
                Err(msg) => {
                    write_err(err, err_cap, &msg);
                    0
                }
            }
        },
    )
}

/// `FPoly::SplitInHalf(OtherHalf)`. Failure writes to `err` and leaves both
/// polygons untouched.
///
/// # Safety
/// `poly` and `other` live `FPoly`s (or null); `err` valid for `err_cap` bytes (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_split_in_half(
    poly: *mut c_void,
    other: *mut c_void,
    err: *mut c_char,
    err_cap: i32,
) {
    unsafe { clear_err(err, err_cap) };
    guard("hp2_rs_engine_fpoly_split_in_half", (), move || unsafe {
        let (Some(p), Some(o)) = (poly_mut(poly), poly_mut(other)) else {
            return;
        };
        if let Err(msg) = fpoly::split_in_half(p, o) {
            write_err(err, err_cap, &msg);
        }
    })
}

/// `FPoly::Split(Normal, Base, NoOverflow)`: returns the new vertex count, 0 when clipped away.
///
/// # Safety
/// `poly` a live `FPoly`; `normal`/`base` live `FVector`s; `err` valid for `err_cap` bytes (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_split(
    poly: *mut c_void,
    normal: *const c_void,
    base: *const c_void,
    no_overflow: i32,
    err: *mut c_char,
    err_cap: i32,
) -> i32 {
    unsafe { clear_err(err, err_cap) };
    guard("hp2_rs_engine_fpoly_split", 0, move || unsafe {
        let (Some(p), Some(normal), Some(base)) = (poly_mut(poly), vec_at(normal), vec_at(base))
        else {
            return 0;
        };
        match fpoly::split(p, normal, base, no_overflow != 0) {
            Ok(r) => r,
            Err(msg) => {
                write_err(err, err_cap, &msg);
                0
            }
        }
    })
}

/// `FPoly::Faces(Test)`.
///
/// # Safety
/// `poly` and `test` live `FPoly`s (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_faces(
    poly: *const c_void,
    test: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_fpoly_faces", 0, move || unsafe {
        match (poly_ref(poly), poly_ref(test)) {
            (Some(a), Some(t)) => fpoly::faces(a, t),
            _ => 0,
        }
    })
}

/// `FPoly::Transform(Coords, PreSubtract, PostAdd, Orientation)`.
///
/// # Safety
/// `poly` a live `FPoly`; `point_xform`/`vector_xform` live `FCoords`; `pre`/`post` live `FVector`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_transform(
    poly: *mut c_void,
    point_xform: *const c_void,
    vector_xform: *const c_void,
    pre: *const c_void,
    post: *const c_void,
    orientation: f32,
) {
    guard("hp2_rs_engine_fpoly_transform", (), move || unsafe {
        let (Some(p), Some(point), Some(vector), Some(pre), Some(post)) = (
            poly_mut(poly),
            coords_at(point_xform),
            coords_at(vector_xform),
            vec_at(pre),
            vec_at(post),
        ) else {
            return;
        };
        fpoly::transform(p, point, vector, pre, post, orientation);
    })
}

/// `FPoly::RemoveColinears()`: returns 1 if convex, 0 if nonconvex or collapsed.
///
/// # Safety
/// `poly` must be a live `FPoly` (or null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_fpoly_remove_colinears(poly: *mut c_void) -> i32 {
    guard("hp2_rs_engine_fpoly_remove_colinears", 0, move || unsafe {
        poly_mut(poly).map_or(0, fpoly::remove_colinears)
    })
}

/// `UModel::Transform` poly loop: transforms `count` polygons in place.
///
/// # Safety
/// `polys` must point to `count` contiguous live `FPoly`s (or be null with
/// `count == 0`); the coords/vectors must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_transform_polys(
    polys: *mut c_void,
    count: i32,
    point_xform: *const c_void,
    vector_xform: *const c_void,
    pre: *const c_void,
    post: *const c_void,
    orientation: f32,
) {
    guard("hp2_rs_engine_model_transform_polys", (), move || unsafe {
        let (Ok(n), Some(point), Some(vector), Some(pre), Some(post)) = (
            usize::try_from(count),
            coords_at(point_xform),
            coords_at(vector_xform),
            vec_at(pre),
            vec_at(post),
        ) else {
            return;
        };
        if n == 0 || polys.is_null() {
            return;
        }
        let slice = std::slice::from_raw_parts_mut(polys.cast::<FPoly>(), n);
        model::transform_polys(slice, point, vector, pre, post, orientation);
    })
}

/// Adapts the C++ `UModel`/`ABrush` services for one model to [`ModelServices`].
struct ModelSvc<'a> {
    table: &'a Hp2GeometryServices,
    model: *mut c_void,
}

impl ModelServices for ModelSvc<'_> {
    fn surf_modify_item(&mut self, index: i32) {
        // SAFETY: the export's contract makes `model` the live owning UModel.
        unsafe { (self.table.surf_modify_item)(self.model, index) };
    }
    fn brush_poly_modify_item(&mut self, actor: *mut c_void, index: i32) {
        // SAFETY: `actor` is the live ABrush a surface points at.
        unsafe { (self.table.brush_poly_modify_item)(actor, index) };
    }
    fn surf_add_node(&mut self, surf: i32, node: i32) {
        // SAFETY: as for `surf_modify_item`.
        unsafe { (self.table.surf_add_node)(self.model, surf, node) };
    }
}

/// # Safety
/// `p`, when non-null with `count > 0`, must point to `count` live `T`s.
unsafe fn slice_at<'a, T>(p: *const c_void, count: i32) -> &'a [T] {
    let Ok(n) = usize::try_from(count) else {
        return &[];
    };
    if n == 0 || p.is_null() {
        return &[];
    }
    // SAFETY: `p` points to `n` live elements per the caller's contract.
    unsafe { std::slice::from_raw_parts(p.cast::<T>(), n) }
}

/// # Safety
/// `p`, when non-null with `count > 0`, must point to `count` live `T`s that are
/// not otherwise borrowed for the returned lifetime.
unsafe fn slice_at_mut<'a, T>(p: *mut c_void, count: i32) -> &'a mut [T] {
    let Ok(n) = usize::try_from(count) else {
        return &mut [];
    };
    if n == 0 || p.is_null() {
        return &mut [];
    }
    // SAFETY: `p` points to `n` live elements per the caller's contract.
    unsafe { std::slice::from_raw_parts_mut(p.cast::<T>(), n) }
}

/// `UModel::BuildBound`: the bounding box over every polygon vertex and, when
/// there are polygons, the bounding sphere. With no polygons the sphere is left
/// untouched, as in the C++ `else` branch.
///
/// # Safety
/// `polys` must point to `count` live `FPoly`s (or be null / `count == 0`).
/// `bound` must be a live `FBox`; `sphere`, when non-null, a live `FPlane`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_build_bound(
    polys: *const c_void,
    count: i32,
    bound: *mut c_void,
    sphere: *mut c_void,
) {
    guard("hp2_rs_engine_model_build_bound", (), move || unsafe {
        let (bx, sph) = model::build_bound(slice_at::<FPoly>(polys, count));
        if let Some(out) = bound.cast::<FBox>().as_mut() {
            out.min = bx.min;
            out.max = bx.max;
            out.is_valid = u8::from(bx.is_valid);
        }
        if let (Some(out), Some(s)) = (sphere.cast::<FPlane>().as_mut(), sph) {
            *out = s;
        }
    })
}

/// `UModel::PotentiallyVisible`: always visible.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_model_potentially_visible(leaf1: i32, leaf2: i32) -> i32 {
    guard("hp2_rs_engine_model_potentially_visible", 1, move || {
        model::potentially_visible(leaf1, leaf2)
    })
}

/// `UModel::ModifySurf(Index, UpdateMaster)`.
///
/// # Safety
/// `svc` must be the live services table; `model` the live owning `UModel`;
/// `surfs` must point to `count` live `FBspSurf`s (or be null / `count == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_modify_surf(
    svc: *const Hp2GeometryServices,
    model: *mut c_void,
    surfs: *const c_void,
    count: i32,
    index: i32,
    update_master: i32,
) {
    guard("hp2_rs_engine_model_modify_surf", (), move || unsafe {
        let Some(table) = svc.as_ref() else {
            return;
        };
        let mut adapter = ModelSvc { table, model };
        let surfs = slice_at::<FBspSurf>(surfs, count);
        model::modify_surf(&mut adapter, surfs, index, update_master);
    })
}

/// `UModel::ModifyAllSurfs(UpdateMaster)`.
///
/// # Safety
/// Same contract as [`hp2_rs_engine_model_modify_surf`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_modify_all_surfs(
    svc: *const Hp2GeometryServices,
    model: *mut c_void,
    surfs: *const c_void,
    count: i32,
    update_master: i32,
) {
    guard("hp2_rs_engine_model_modify_all_surfs", (), move || unsafe {
        let Some(table) = svc.as_ref() else {
            return;
        };
        let mut adapter = ModelSvc { table, model };
        let surfs = slice_at::<FBspSurf>(surfs, count);
        model::modify_all_surfs(&mut adapter, surfs, update_master);
    })
}

/// `UModel::ModifySelectedSurfs(UpdateMaster)`.
///
/// # Safety
/// Same contract as [`hp2_rs_engine_model_modify_surf`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_modify_selected_surfs(
    svc: *const Hp2GeometryServices,
    model: *mut c_void,
    surfs: *const c_void,
    count: i32,
    update_master: i32,
) {
    guard(
        "hp2_rs_engine_model_modify_selected_surfs",
        (),
        move || unsafe {
            let Some(table) = svc.as_ref() else {
                return;
            };
            let mut adapter = ModelSvc { table, model };
            let surfs = slice_at::<FBspSurf>(surfs, count);
            model::modify_selected_surfs(&mut adapter, surfs, update_master);
        },
    )
}

/// `UModel::PostLoad`'s node loop: registers each BSP node with its surface.
///
/// # Safety
/// `nodes` must point to `count` live `FBspNode`s (or be null / `count == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_post_load_nodes(
    svc: *const Hp2GeometryServices,
    model: *mut c_void,
    nodes: *const c_void,
    count: i32,
) {
    guard("hp2_rs_engine_model_post_load_nodes", (), move || unsafe {
        let Some(table) = svc.as_ref() else {
            return;
        };
        let mut adapter = ModelSvc { table, model };
        let nodes = slice_at::<FBspNode>(nodes, count);
        model::post_load_nodes(&mut adapter, nodes);
    })
}

/// `UModel::EmptyModel`'s zone reset over `count` zone records.
///
/// # Safety
/// `zones` must point to `count` live `FZoneProperties` (or be null / `count == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_model_reset_zones(zones: *mut c_void, count: i32) {
    guard("hp2_rs_engine_model_reset_zones", (), move || unsafe {
        model::reset_zones(slice_at_mut::<FZoneProperties>(zones, count));
    })
}
