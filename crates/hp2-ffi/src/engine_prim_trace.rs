//! C ABI for the collision-primitive (`hp2_engine::prim`) and BSP-trace
//! (`hp2_engine::trace`) ports. The C++ shells in `UnPrim.cpp` and `UnTrace.cpp`
//! build the `Hp2PrimOwner`, `Hp2BspArgs` and `Hp2TraceModel` arguments, whose
//! layouts are mirrored in `HP2PrimTrace.h` and checked there with
//! `static_assert`. Every pointer argument is `void*` and is cast here.
//!
//! Null policy: a null input or output pointer makes the export return its
//! default (`1` = no hit for checks, `-1` for indices and distances) without
//! writing. C++ callers always pass live objects, so this only avoids UB.
//! A null owner is accepted by the cylinder and aligned-oval checks, which
//! treat it as no hit, as the C++ `Owner == NULL` path does. The oriented, UBox
//! and UBoxPrim checks dereference the owner in C++, so a null owner returns no
//! hit here.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FBox, FCheckResult, FPoly};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_engine::prim::{self, Aabb, BspArrays, FCheck, Owner};
use hp2_engine::trace::{self, ModelView};
use hp2_ffi_support::guard;

/// `Hp2PrimOwnerArg` in `HP2PrimTrace.h`: the owner frame and extents the C++
/// shell reads from `AActor`. Rust never dereferences `actor`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PrimOwner {
    pub actor: *mut c_void,
    pub location: FVector,
    pub collision_radius: f32,
    pub collision_width: f32,
    pub collision_height: f32,
    pub to_local: FCoords,
    pub to_world: FCoords,
    pub cyl_extent: FVector,
    pub cyl_extent_zero_width: FVector,
}

/// `Hp2BspArgs` in `HP2PrimTrace.h`: the `UModel` BSP arrays a query reads.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2BspArgs {
    pub nodes: *const c_void,
    pub num_nodes: i32,
    pub points: *const c_void,
    pub num_points: i32,
    pub verts: *const c_void,
    pub num_verts: i32,
    pub surfs: *const c_void,
    pub num_surfs: i32,
}

/// `Hp2TraceModelArg` in `HP2PrimTrace.h`: the `UModel` fields the BSP traces
/// read. `root_outside` is `UBOOL` (nonzero = true).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TraceModel {
    pub bsp: Hp2BspArgs,
    pub leaf_hulls: *const c_void,
    pub num_leaf_hulls: i32,
    pub bounding_box: FBox,
    pub root_outside: i32,
    pub num_zones: i32,
    pub primitive: *mut c_void,
}

fn b2i(b: bool) -> i32 {
    i32::from(b)
}

/// Borrows the `T` at `p`, or `None` when `p` is null.
///
/// # Safety
/// A non-null `p` must point to a live `T` for the chosen lifetime.
unsafe fn at<'a, T>(p: *const c_void) -> Option<&'a T> {
    unsafe { p.cast::<T>().as_ref() }
}

/// Mutable form of [`at`].
///
/// # Safety
/// A non-null `p` must point to a live, exclusively accessed `T`.
unsafe fn at_mut<'a, T>(p: *mut c_void) -> Option<&'a mut T> {
    unsafe { p.cast::<T>().as_mut() }
}

/// `n` elements at `p`; a null pointer or negative count yields an empty slice.
///
/// # Safety
/// A non-null `p` must point to `n` live `T` values.
unsafe fn slice<'a, T>(p: *const c_void, n: i32) -> &'a [T] {
    match usize::try_from(n) {
        Ok(n) if !p.is_null() => unsafe { std::slice::from_raw_parts(p.cast::<T>(), n) },
        _ => &[],
    }
}

/// Mutable form of [`slice`].
///
/// # Safety
/// A non-null `p` must point to `n` live, exclusively accessed `T` values.
unsafe fn slice_mut<'a, T>(p: *mut c_void, n: i32) -> &'a mut [T] {
    match usize::try_from(n) {
        Ok(n) if !p.is_null() => unsafe { std::slice::from_raw_parts_mut(p.cast::<T>(), n) },
        _ => &mut [],
    }
}

/// Writes `v` through `p` when it is non-null.
///
/// # Safety
/// A non-null `p` must be valid for writes of `T`.
unsafe fn put<T: Copy>(p: *mut c_void, v: T) {
    if let Some(out) = unsafe { at_mut::<T>(p) } {
        *out = v;
    }
}

/// Converts the shell's owner argument; `None` when `p` is null.
///
/// # Safety
/// A non-null `p` must point to a live `Hp2PrimOwner`.
unsafe fn owner_of(p: *const c_void) -> Option<Owner> {
    let o = unsafe { at::<Hp2PrimOwner>(p) }?;
    Some(Owner {
        actor: o.actor,
        location: o.location,
        collision_radius: o.collision_radius,
        collision_width: o.collision_width,
        collision_height: o.collision_height,
        to_local: o.to_local,
        to_world: o.to_world,
        cyl_extent: o.cyl_extent,
        cyl_extent_zero_width: o.cyl_extent_zero_width,
    })
}

fn aabb_of(b: &FBox) -> Aabb {
    Aabb {
        min: b.min,
        max: b.max,
        valid: b.is_valid != 0,
    }
}

fn box_out(a: &Aabb) -> FBox {
    // SAFETY: `FBox` is floats and bytes; the private padding stays zero.
    let mut b: FBox = unsafe { std::mem::zeroed() };
    b.min = a.min;
    b.max = a.max;
    b.is_valid = u8::from(a.valid);
    b
}

/// # Safety
/// `b` must be a live `Hp2BspArgs`; its array pointers must be valid for their counts.
unsafe fn bsp_of<'a>(b: &Hp2BspArgs) -> BspArrays<'a> {
    unsafe {
        BspArrays {
            nodes: slice::<FBspNode>(b.nodes, b.num_nodes),
            points: slice::<FVector>(b.points, b.num_points),
            verts: slice::<FVert>(b.verts, b.num_verts),
            surfs: slice::<FBspSurf>(b.surfs, b.num_surfs),
        }
    }
}

/// # Safety
/// A non-null `p` must point to a live `Hp2TraceModel` with valid arrays.
unsafe fn model_of<'a>(p: *const c_void) -> Option<ModelView<'a>> {
    let m = unsafe { at::<Hp2TraceModel>(p) }?;
    Some(ModelView {
        bsp: unsafe { bsp_of(&m.bsp) },
        leaf_hulls: unsafe { slice::<i32>(m.leaf_hulls, m.num_leaf_hulls) },
        bounding_box: aabb_of(&m.bounding_box),
        root_outside: m.root_outside != 0,
        num_zones: m.num_zones,
        primitive: m.primitive,
    })
}

/// # Safety
/// Pointers follow the `UPrimitive::PointCheck` shell contract (see module doc).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_cyl_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_cyl_point_check", 1, move || unsafe {
        let (Some(res), Some(loc), Some(ext)) = (
            at_mut::<FCheckResult>(res),
            at::<FVector>(location),
            at::<FVector>(extent),
        ) else {
            return 1;
        };
        b2i(prim::cyl_point_check(
            res,
            owner_of(owner).as_ref(),
            *loc,
            *ext,
        ))
    })
}

/// # Safety
/// Pointers follow the `UPrimitive::LineCheck` shell contract (see module doc).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_cyl_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_cyl_line_check", 1, move || unsafe {
        let (Some(res), Some(e), Some(s), Some(ext)) = (
            at_mut::<FCheckResult>(res),
            at::<FVector>(end),
            at::<FVector>(start),
            at::<FVector>(extent),
        ) else {
            return 1;
        };
        b2i(prim::cyl_line_check(
            res,
            owner_of(owner).as_ref(),
            *e,
            *s,
            *ext,
        ))
    })
}

/// # Safety
/// Pointers follow the `UAlignedOvalCylinder::PointCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_aligned_oval_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_aligned_oval_point_check",
        1,
        move || unsafe {
            let (Some(res), Some(loc), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                at::<FVector>(location),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::aligned_oval_point_check(
                res,
                owner_of(owner).as_ref(),
                *loc,
                *ext,
            ))
        },
    )
}

/// # Safety
/// Pointers follow the `UAlignedOvalCylinder::LineCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_aligned_oval_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_aligned_oval_line_check",
        1,
        move || unsafe {
            let (Some(res), Some(e), Some(s), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                at::<FVector>(end),
                at::<FVector>(start),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::aligned_oval_line_check(
                res,
                owner_of(owner).as_ref(),
                *e,
                *s,
                *ext,
            ))
        },
    )
}

/// # Safety
/// Pointers follow the `UOrientedCylinder::PointCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_cyl_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_oriented_cyl_point_check",
        1,
        move || unsafe {
            let (Some(res), Some(o), Some(loc), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                owner_of(owner),
                at::<FVector>(location),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::oriented_cyl_point_check(res, &o, *loc, *ext))
        },
    )
}

/// # Safety
/// Pointers follow the `UOrientedCylinder::LineCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_cyl_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_oriented_cyl_line_check",
        1,
        move || unsafe {
            let (Some(res), Some(o), Some(e), Some(s), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                owner_of(owner),
                at::<FVector>(end),
                at::<FVector>(start),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::oriented_cyl_line_check(res, &o, *e, *s, *ext))
        },
    )
}

/// # Safety
/// Pointers follow the `UOrientedOvalCylinder::PointCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_oval_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_oriented_oval_point_check",
        1,
        move || unsafe {
            let (Some(res), Some(o), Some(loc), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                owner_of(owner),
                at::<FVector>(location),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::oriented_oval_point_check(res, &o, *loc, *ext))
        },
    )
}

/// # Safety
/// Pointers follow the `UOrientedOvalCylinder::LineCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_oval_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_prim_oriented_oval_line_check",
        1,
        move || unsafe {
            let (Some(res), Some(o), Some(e), Some(s), Some(ext)) = (
                at_mut::<FCheckResult>(res),
                owner_of(owner),
                at::<FVector>(end),
                at::<FVector>(start),
                at::<FVector>(extent),
            ) else {
                return 1;
            };
            b2i(prim::oriented_oval_line_check(res, &o, *e, *s, *ext))
        },
    )
}

/// Writes the `FBox` of the base cylinder (`UPrimitive::GetCollisionBoundingBox`).
/// `world != 0` adds the owner's location.
///
/// # Safety
/// `owner` must be null or a live `Hp2PrimOwner`; `out` must be null or a live `FBox`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_cyl_bounds(
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_prim_cyl_bounds", (), move || unsafe {
        if let Some(o) = owner_of(owner) {
            put(out, box_out(&prim::cyl_bounds(&o, world != 0)));
        }
    })
}

/// Writes `FBox` for `GetCollisionBoundingBox` shells. `world != 0` applies `ToWorld`.
///
/// # Safety
/// `owner` must be null or a live `Hp2PrimOwner`; `out` must be null or a live `FBox`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_cyl_bounds(
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_prim_oriented_cyl_bounds",
        (),
        move || unsafe {
            if let Some(o) = owner_of(owner) {
                put(out, box_out(&prim::oriented_cyl_bounds(&o, world != 0)));
            }
        },
    )
}

/// # Safety
/// Same pointer contract as [`hp2_rs_engine_prim_oriented_cyl_bounds`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_oriented_oval_bounds(
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_prim_oriented_oval_bounds",
        (),
        move || unsafe {
            if let Some(o) = owner_of(owner) {
                put(out, box_out(&prim::oriented_oval_bounds(&o, world != 0)));
            }
        },
    )
}

/// # Safety
/// Same pointer contract as [`hp2_rs_engine_prim_oriented_cyl_bounds`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_aligned_oval_bounds(
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_prim_aligned_oval_bounds",
        (),
        move || unsafe {
            if let Some(o) = owner_of(owner) {
                put(out, box_out(&prim::aligned_oval_bounds(&o, world != 0)));
            }
        },
    )
}

/// # Safety
/// Same pointer contract as [`hp2_rs_engine_prim_oriented_cyl_bounds`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_ubox_bounds(
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_prim_ubox_bounds", (), move || unsafe {
        if let Some(o) = owner_of(owner) {
            put(out, box_out(&prim::ubox_bounds(&o, world != 0)));
        }
    })
}

/// Writes the `FVector` UBox collision extent to `out`.
///
/// # Safety
/// `owner` must be null or a live `Hp2PrimOwner`; `out` must be null or a live `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_ubox_extent(owner: *const c_void, out: *mut c_void) {
    guard("hp2_rs_engine_prim_ubox_extent", (), move || unsafe {
        if let Some(o) = owner_of(owner) {
            put(out, prim::ubox_extent(&o));
        }
    })
}

/// # Safety
/// Pointers follow the `UBox::PointCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_ubox_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_ubox_point_check", 1, move || unsafe {
        let (Some(res), Some(o), Some(loc), Some(ext)) = (
            at_mut::<FCheckResult>(res),
            owner_of(owner),
            at::<FVector>(location),
            at::<FVector>(extent),
        ) else {
            return 1;
        };
        b2i(prim::ubox_point_check(res, &o, *loc, *ext))
    })
}

/// # Safety
/// Pointers follow the `UBox::LineCheck` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_ubox_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_ubox_line_check", 1, move || unsafe {
        let (Some(res), Some(o), Some(e), Some(s), Some(ext)) = (
            at_mut::<FCheckResult>(res),
            owner_of(owner),
            at::<FVector>(end),
            at::<FVector>(start),
            at::<FVector>(extent),
        ) else {
            return 1;
        };
        b2i(prim::ubox_line_check(res, &o, *e, *s, *ext))
    })
}

/// # Safety
/// Pointers follow the `UBoxPrim::GetCollisionBoundingBox` shell contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_boxprim_bounds(
    bx: *const c_void,
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_prim_boxprim_bounds", (), move || unsafe {
        let (Some(b), Some(o)) = (aabb_of_ptr(bx), owner_of(owner)) else {
            return;
        };
        put(out, box_out(&prim::boxprim_bounds(&b, &o, world != 0)));
    })
}

/// Writes the `FVector` UBoxPrim collision extent to `out`.
///
/// # Safety
/// `bx` must be null or a live `FBox`; `out` must be null or a live `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_boxprim_extent(bx: *const c_void, out: *mut c_void) {
    guard("hp2_rs_engine_prim_boxprim_extent", (), move || unsafe {
        if let Some(b) = aabb_of_ptr(bx) {
            put(out, prim::boxprim_extent(&b));
        }
    })
}

/// Writes `[radius, height]` (`f32` x 2) to `out`.
///
/// # Safety
/// `bx` must be null or a live `FBox`; `out` must be null or a live `[f32; 2]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_boxprim_validate(bx: *const c_void, out: *mut c_void) {
    guard("hp2_rs_engine_prim_boxprim_validate", (), move || unsafe {
        if let Some(b) = aabb_of_ptr(bx) {
            let (radius, height) = prim::boxprim_validate(&b);
            put(out, [radius, height]);
        }
    })
}

/// # Safety
/// `bsp` must be null or a live `Hp2BspArgs`; `location`, `normal` must be live
/// `FVector`s; `mover_to_local` must be null or a live `FCoords`.
/// Returns the node index, or `-1` for none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_bsp_node(
    bsp: *const c_void,
    item: i32,
    location: *const c_void,
    normal: *const c_void,
    radius: f32,
    mover_to_local: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_bsp_node", -1, move || unsafe {
        let (Some(loc), Some(norm)) = (at::<FVector>(location), at::<FVector>(normal)) else {
            return -1;
        };
        let arrays = at::<Hp2BspArgs>(bsp).map(|b| bsp_of(b));
        let mover = at::<FCoords>(mover_to_local);
        prim::bsp_node(arrays.as_ref(), item, *loc, *norm, radius, mover)
            .map_or(-1, |i| i32::try_from(i).unwrap_or(-1))
    })
}

/// # Safety
/// Same contract as [`hp2_rs_engine_prim_bsp_node`]. Returns the surface index, or `-1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_bsp_surf(
    bsp: *const c_void,
    item: i32,
    location: *const c_void,
    normal: *const c_void,
    radius: f32,
    mover_to_local: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_bsp_surf", -1, move || unsafe {
        let (Some(loc), Some(norm)) = (at::<FVector>(location), at::<FVector>(normal)) else {
            return -1;
        };
        let arrays = at::<Hp2BspArgs>(bsp).map(|b| bsp_of(b));
        let mover = at::<FCoords>(mover_to_local);
        prim::bsp_surf(arrays.as_ref(), item, *loc, *norm, radius, mover)
            .map_or(-1, |i| i32::try_from(i).unwrap_or(-1))
    })
}

/// Box point check. `owner` may be null (no owner frame).
///
/// # Safety
/// `res`, `location`, `extent`, `bx` must be null or live; `owner` null or a live `Hp2PrimOwner`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_box_point_check(
    res: *mut c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
    bx: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_box_point_check", 1, move || unsafe {
        let (Some(res), Some(loc), Some(ext), Some(b)) = (
            at_mut::<FCheckResult>(res),
            at::<FVector>(location),
            at::<FVector>(extent),
            aabb_of_ptr(bx),
        ) else {
            return 1;
        };
        let check = FCheck::point(*loc, *ext, owner_of(owner));
        b2i(prim::box_point_check(res, &check, &b))
    })
}

/// Box line check. `owner` may be null (no owner frame).
///
/// # Safety
/// `res`, `end`, `start`, `extent`, `bx` must be null or live; `owner` null or a live `Hp2PrimOwner`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_prim_box_line_check(
    res: *mut c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
    bx: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_prim_box_line_check", 1, move || unsafe {
        let (Some(res), Some(e), Some(s), Some(ext), Some(b)) = (
            at_mut::<FCheckResult>(res),
            at::<FVector>(end),
            at::<FVector>(start),
            at::<FVector>(extent),
            aabb_of_ptr(bx),
        ) else {
            return 1;
        };
        let check = FCheck {
            end: *e,
            start: *s,
            extent: *ext,
            owner: owner_of(owner),
        };
        b2i(prim::box_line_check(res, &check, &b))
    })
}

/// Reads an `FBox` pointer into an [`Aabb`].
///
/// # Safety
/// A non-null `p` must point to a live `FBox`.
unsafe fn aabb_of_ptr(p: *const c_void) -> Option<Aabb> {
    let b = unsafe { at::<FBox>(p) }?;
    Some(aabb_of(b))
}

/// # Safety
/// `nodes` must be null or `num_nodes` live `FBspNode`s; `sphere` must be null or a live `FPlane`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_precompute_sphere_filter(
    nodes: *mut c_void,
    num_nodes: i32,
    sphere: *const c_void,
) {
    guard(
        "hp2_rs_engine_trace_precompute_sphere_filter",
        (),
        move || unsafe {
            let Some(sphere) = at::<FPlane>(sphere) else {
                return;
            };
            let nodes = slice_mut::<FBspNode>(nodes, num_nodes);
            trace::precompute_sphere_filter(nodes, *sphere);
        },
    )
}

/// Writes the render bounding box of a model. `has_polys == 0` means no
/// `UPolys` (`Polys == NULL`), so the transformed `BoundingBox` is used.
///
/// # Safety
/// `model`, `polys`, `out` must be null or live; `owner` null or a live `Hp2PrimOwner`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_get_render_bounding_box(
    model: *const c_void,
    owner: *const c_void,
    exact: i32,
    has_polys: i32,
    polys: *const c_void,
    num_polys: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_trace_get_render_bounding_box",
        (),
        move || unsafe {
            let Some(m) = model_of(model) else {
                return;
            };
            let polys = (has_polys != 0).then(|| slice::<FPoly>(polys, num_polys));
            let b = trace::get_render_bounding_box(&m, owner_of(owner), exact != 0, polys);
            put(out, box_out(&b));
        },
    )
}

/// `UModel::PointCheck`. Returns `UBOOL` Outside (1 = outside).
///
/// # Safety
/// `hit`, `model`, `location`, `extent` must be live; `owner` null or a live `Hp2PrimOwner`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_model_point_check(
    hit: *mut c_void,
    model: *const c_void,
    owner: *const c_void,
    location: *const c_void,
    extent: *const c_void,
    extra: u32,
) -> i32 {
    guard("hp2_rs_engine_trace_model_point_check", 0, move || unsafe {
        let (Some(hit), Some(m), Some(loc), Some(ext)) = (
            at_mut::<FCheckResult>(hit),
            model_of(model),
            at::<FVector>(location),
            at::<FVector>(extent),
        ) else {
            return 0;
        };
        b2i(trace::model_point_check(
            hit,
            &m,
            owner_of(owner),
            *loc,
            *ext,
            extra,
        ))
    })
}

/// `UModel::FastLineCheck`. Returns `UBOOL` (1 = outside, unblocked).
///
/// # Safety
/// `model`, `end`, `start` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_fast_line_check(
    model: *const c_void,
    end: *const c_void,
    start: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_trace_fast_line_check", 1, move || unsafe {
        let (Some(m), Some(e), Some(s)) =
            (model_of(model), at::<FVector>(end), at::<FVector>(start))
        else {
            return 1;
        };
        b2i(trace::fast_line_check(&m, *e, *s))
    })
}

/// `UModel::LineCheck`. Returns `UBOOL` (1 = unblocked).
///
/// # Safety
/// `hit`, `model`, `end`, `start`, `extent` must be live; `owner` null or a live `Hp2PrimOwner`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_model_line_check(
    hit: *mut c_void,
    model: *const c_void,
    owner: *const c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
    flags: u32,
) -> i32 {
    guard("hp2_rs_engine_trace_model_line_check", 1, move || unsafe {
        let (Some(hit), Some(m), Some(e), Some(s), Some(ext)) = (
            at_mut::<FCheckResult>(hit),
            model_of(model),
            at::<FVector>(end),
            at::<FVector>(start),
            at::<FVector>(extent),
        ) else {
            return 1;
        };
        b2i(trace::model_line_check(
            hit,
            &m,
            owner_of(owner),
            *e,
            *s,
            *ext,
            flags,
        ))
    })
}

/// `UModel::PointRegion`: writes the leaf index and zone number.
///
/// # Safety
/// `model`, `location` must be live; `out_leaf` (`i32`) and `out_zone` (`u8`) must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_point_region(
    model: *const c_void,
    location: *const c_void,
    out_leaf: *mut c_void,
    out_zone: *mut c_void,
) {
    guard("hp2_rs_engine_trace_point_region", (), move || unsafe {
        let (Some(m), Some(loc)) = (model_of(model), at::<FVector>(location)) else {
            return;
        };
        let r = trace::point_region(&m, *loc);
        put(out_leaf, r.i_leaf);
        put(out_zone, r.zone_number);
    })
}

/// `UModel::FindNearestVertex`. Returns the distance, or `-1.0` when none.
///
/// # Safety
/// `model`, `source`, `dest`, `p_vertex` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_trace_find_nearest_vertex(
    model: *const c_void,
    source: *const c_void,
    dest: *mut c_void,
    min_radius: f32,
    p_vertex: *mut c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_trace_find_nearest_vertex",
        -1.0,
        move || unsafe {
            let (Some(m), Some(src), Some(dst), Some(pv)) = (
                model_of(model),
                at::<FVector>(source),
                at_mut::<FVector>(dest),
                at_mut::<i32>(p_vertex),
            ) else {
                return -1.0;
            };
            let mut d = *dst;
            let r = trace::find_nearest_vertex(&m, *src, &mut d, min_radius, pv);
            *dst = d;
            r
        },
    )
}
