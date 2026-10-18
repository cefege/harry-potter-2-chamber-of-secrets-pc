//! `#[repr(C)]` mirrors of the UnRender scene/model/dynamics layouts that the
//! UnRender port (`hp2-render`) indexes directly. Owning C++ definitions:
//!
//! - `Engine/Inc/UnRender.h`: `FSceneNode`, `FSurfaceInfo`, `FSavedPoly`,
//!   `FSurfaceFacet`, `FScreenBounds`.
//! - `Engine/Inc/UnObj.h`: `FVert`, `FBspNode`, `FBspSurf`, `FLeaf`,
//!   `FZoneProperties`.
//! - `Engine/Inc/UnActor.h`: `FActorLink`.
//! - `Render/Inc/Render.h`: `FVolActorLink`, `FDynamicItem`,
//!   `FDynamicSprite`, `FBspDrawList`, `URender::FStampedPoint`,
//!   `URender::FDynamicsCache`. These cannot take the C++ asserts in
//!   `Render.h` (off-limits this phase), so they are pinned in a
//!   `static_assert` block in `Render/Src/UnRender.cpp`.
//!
//! Every size and offset below was probed with clang++ against the pinned
//! headers (`sizeof`/`offsetof`, arm64 macOS flags) and is pinned again on
//! the C++ side.
//!
//! Conventions:
//! - C++ pointers to objects Rust never dereferences (`UViewport`, `ULevel`,
//!   `AActor`, `UTexture`, `FTextureInfo`, `AZoneInfo`, `ABrush`) are
//!   `*mut c_void`.
//! - Pointers to mirrored structs keep their concrete type.
//! - `FDynamicItem` virtuals (`Filter`, `PreRender`) are never mirrored. The
//!   vtable pointer is `vptr: *const c_void` and dispatch goes through the
//!   `Hp2UrenServices` hooks.
//! - `UBOOL` and `INT` are `i32`. `DWORD` is `u32`. `QWORD` is `u64`. `BYTE`
//!   is `u8`. `SWORD` is `i16`.

use std::ffi::c_void;

use crate::render_particle::FTransform;
use crate::render_span::FSpanBuffer;
use crate::{FColor, FCoords, FPlane, FVector};

/// `TArray<T>` header as laid out by `Core/Inc/UnTemplate.h`: `T* Data`,
/// `INT ArrayNum` at offset 8, `INT ArrayMax` at offset 12. 16 bytes.
/// Rust never reads elements through this type; it forwards the header to
/// C++ helpers that know `T`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TArrayRaw {
    pub data: *mut c_void,
    pub array_num: i32,
    pub array_max: i32,
}

const _: () = assert!(size_of::<TArrayRaw>() == 16);
const _: () = assert!(std::mem::offset_of!(TArrayRaw, data) == 0);
const _: () = assert!(std::mem::offset_of!(TArrayRaw, array_num) == 8);
const _: () = assert!(std::mem::offset_of!(TArrayRaw, array_max) == 12);

/// `FVert { INT pVertex; INT iSide; }` (`UnObj.h`). 8 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FVert {
    pub p_vertex: i32,
    pub i_side: i32,
}

const _: () = assert!(size_of::<FVert>() == 8);
const _: () = assert!(std::mem::offset_of!(FVert, p_vertex) == 0);
const _: () = assert!(std::mem::offset_of!(FVert, i_side) == 4);

/// `FBspNode` (`UnObj.h`, "64 bytes"). The `iBack`/`iChild[1]` union is
/// mirrored as `i_back` (offset 32, same storage as `iChild[0]`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FBspNode {
    pub plane: FPlane,
    pub zone_mask: u64,
    pub i_vert_pool: i32,
    pub i_surf: i32,
    pub i_back: i32,
    pub i_front: i32,
    pub i_plane: i32,
    pub i_collision_bound: i32,
    pub i_render_bound: i32,
    pub i_zone: [u8; 2],
    pub num_vertices: u8,
    pub node_flags: u8,
    pub i_leaf: [i32; 2],
}

const _: () = assert!(size_of::<FBspNode>() == 64);
const _: () = assert!(std::mem::offset_of!(FBspNode, plane) == 0);
const _: () = assert!(std::mem::offset_of!(FBspNode, zone_mask) == 16);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_vert_pool) == 24);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_surf) == 28);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_back) == 32);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_front) == 36);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_plane) == 40);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_collision_bound) == 44);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_render_bound) == 48);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_zone) == 52);
const _: () = assert!(std::mem::offset_of!(FBspNode, num_vertices) == 54);
const _: () = assert!(std::mem::offset_of!(FBspNode, node_flags) == 55);
const _: () = assert!(std::mem::offset_of!(FBspNode, i_leaf) == 56);

/// `FBspSurf` (`UnObj.h`). 80 bytes. `Decals` is `TArray<FDecal>` (element
/// layout not mirrored); `Nodes` is `TArray<INT>`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FBspSurf {
    pub texture: *mut c_void,
    pub poly_flags: u32,
    pub p_base: i32,
    pub v_normal: i32,
    pub v_texture_u: i32,
    pub v_texture_v: i32,
    pub i_light_map: i32,
    pub i_brush_poly: i32,
    pub pan_u: i16,
    pub pan_v: i16,
    pub actor: *mut c_void,
    pub decals: TArrayRaw,
    pub nodes: TArrayRaw,
}

const _: () = assert!(size_of::<FBspSurf>() == 80);
const _: () = assert!(std::mem::offset_of!(FBspSurf, texture) == 0);
const _: () = assert!(std::mem::offset_of!(FBspSurf, poly_flags) == 8);
const _: () = assert!(std::mem::offset_of!(FBspSurf, p_base) == 12);
const _: () = assert!(std::mem::offset_of!(FBspSurf, v_normal) == 16);
const _: () = assert!(std::mem::offset_of!(FBspSurf, v_texture_u) == 20);
const _: () = assert!(std::mem::offset_of!(FBspSurf, v_texture_v) == 24);
const _: () = assert!(std::mem::offset_of!(FBspSurf, i_light_map) == 28);
const _: () = assert!(std::mem::offset_of!(FBspSurf, i_brush_poly) == 32);
const _: () = assert!(std::mem::offset_of!(FBspSurf, pan_u) == 36);
const _: () = assert!(std::mem::offset_of!(FBspSurf, pan_v) == 38);
const _: () = assert!(std::mem::offset_of!(FBspSurf, actor) == 40);
const _: () = assert!(std::mem::offset_of!(FBspSurf, decals) == 48);
const _: () = assert!(std::mem::offset_of!(FBspSurf, nodes) == 64);

/// `FLeaf` (`UnObj.h`). 24 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FLeaf {
    pub i_zone: i32,
    pub i_permeating: i32,
    pub i_volumetric: i32,
    pub visible_zones: u64,
}

const _: () = assert!(size_of::<FLeaf>() == 24);
const _: () = assert!(std::mem::offset_of!(FLeaf, i_zone) == 0);
const _: () = assert!(std::mem::offset_of!(FLeaf, i_permeating) == 4);
const _: () = assert!(std::mem::offset_of!(FLeaf, i_volumetric) == 8);
const _: () = assert!(std::mem::offset_of!(FLeaf, visible_zones) == 16);

/// `FZoneProperties` (`UnObj.h`). 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FZoneProperties {
    pub zone_actor: *mut c_void,
    pub last_render_time: f32,
    pub connectivity: u64,
    pub visibility: u64,
}

const _: () = assert!(size_of::<FZoneProperties>() == 32);
const _: () = assert!(std::mem::offset_of!(FZoneProperties, zone_actor) == 0);
const _: () = assert!(std::mem::offset_of!(FZoneProperties, last_render_time) == 8);
const _: () = assert!(std::mem::offset_of!(FZoneProperties, connectivity) == 16);
const _: () = assert!(std::mem::offset_of!(FZoneProperties, visibility) == 24);

/// `FActorLink { AActor* Actor; FActorLink* Next; }` (`UnActor.h`). 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FActorLink {
    pub actor: *mut c_void,
    pub next: *mut FActorLink,
}

const _: () = assert!(size_of::<FActorLink>() == 16);
const _: () = assert!(std::mem::offset_of!(FActorLink, actor) == 0);
const _: () = assert!(std::mem::offset_of!(FActorLink, next) == 8);

/// `FVolActorLink` (`Render.h`). 40 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FVolActorLink {
    pub location: FVector,
    pub actor: *mut c_void,
    pub next: *mut FVolActorLink,
    pub volumetric: i32,
}

const _: () = assert!(size_of::<FVolActorLink>() == 40);
const _: () = assert!(std::mem::offset_of!(FVolActorLink, location) == 0);
const _: () = assert!(std::mem::offset_of!(FVolActorLink, actor) == 16);
const _: () = assert!(std::mem::offset_of!(FVolActorLink, next) == 24);
const _: () = assert!(std::mem::offset_of!(FVolActorLink, volumetric) == 32);

/// `FSavedPoly` (`UnRender.h`). The C++ `FTransform* Pts[ZEROARRAY]` tail is
/// a zero-length array here, so the 32-byte header matches the probed size.
/// Index `Pts` with `pts_ptr()` plus `num_pts` entries.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSavedPoly {
    pub next: *mut FSavedPoly,
    pub i_node: i32,
    pub user: *mut c_void,
    pub num_pts: i32,
    pub pts: [*mut FTransform; 0],
}

const _: () = assert!(size_of::<FSavedPoly>() == 32);
const _: () = assert!(std::mem::offset_of!(FSavedPoly, next) == 0);
const _: () = assert!(std::mem::offset_of!(FSavedPoly, i_node) == 8);
const _: () = assert!(std::mem::offset_of!(FSavedPoly, user) == 16);
const _: () = assert!(std::mem::offset_of!(FSavedPoly, num_pts) == 24);
const _: () = assert!(std::mem::offset_of!(FSavedPoly, pts) == 32);

/// `FSurfaceInfo` (`UnRender.h`). 56 bytes. The `FTextureInfo*` members are
/// opaque `*mut c_void`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSurfaceInfo {
    pub poly_flags: u32,
    pub flat_color: FColor,
    pub level: *mut c_void,
    pub texture: *mut c_void,
    pub light_map: *mut c_void,
    pub macro_texture: *mut c_void,
    pub detail_texture: *mut c_void,
    pub fog_map: *mut c_void,
}

const _: () = assert!(size_of::<FSurfaceInfo>() == 56);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, poly_flags) == 0);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, flat_color) == 4);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, level) == 8);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, texture) == 16);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, light_map) == 24);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, macro_texture) == 32);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, detail_texture) == 40);
const _: () = assert!(std::mem::offset_of!(FSurfaceInfo, fog_map) == 48);

/// `FSurfaceFacet` (`UnRender.h`). 112 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSurfaceFacet {
    pub map_coords: FCoords,
    pub map_uncoords: FCoords,
    pub span: *mut FSpanBuffer,
    pub polys: *mut FSavedPoly,
}

const _: () = assert!(size_of::<FSurfaceFacet>() == 112);
const _: () = assert!(std::mem::offset_of!(FSurfaceFacet, map_coords) == 0);
const _: () = assert!(std::mem::offset_of!(FSurfaceFacet, map_uncoords) == 48);
const _: () = assert!(std::mem::offset_of!(FSurfaceFacet, span) == 96);
const _: () = assert!(std::mem::offset_of!(FSurfaceFacet, polys) == 104);

/// `FScreenBounds` (`UnRender.h`). 24 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FScreenBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

const _: () = assert!(size_of::<FScreenBounds>() == 24);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, min_x) == 0);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, min_y) == 4);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, max_x) == 8);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, max_y) == 12);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, min_z) == 16);
const _: () = assert!(std::mem::offset_of!(FScreenBounds, max_z) == 20);

/// `FDynamicItem` (`Render.h`): polymorphic base. `vptr` is the C++ vtable
/// pointer and is never dereferenced by Rust. 24 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FDynamicItem {
    pub vptr: *const c_void,
    pub filter_next: *mut FDynamicItem,
    pub min_z: f32,
    pub max_z: f32,
}

const _: () = assert!(size_of::<FDynamicItem>() == 24);
const _: () = assert!(std::mem::offset_of!(FDynamicItem, vptr) == 0);
const _: () = assert!(std::mem::offset_of!(FDynamicItem, filter_next) == 8);
const _: () = assert!(std::mem::offset_of!(FDynamicItem, min_z) == 16);
const _: () = assert!(std::mem::offset_of!(FDynamicItem, max_z) == 20);

/// `FDynamicSprite : FDynamicItem` (`Render.h`). 224 bytes. The base is laid
/// out first, so the derived fields start at offset 24.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FDynamicSprite {
    pub base: FDynamicItem,
    pub span_buffer: *mut FSpanBuffer,
    pub render_next: *mut FDynamicSprite,
    pub proxy_verts: [FTransform; 4],
    pub actor: *mut c_void,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub screen_x: f32,
    pub screen_y: f32,
    pub persp: f32,
    pub volumetrics: *mut FActorLink,
    pub leaf_lights: *mut FVolActorLink,
}

const _: () = assert!(size_of::<FDynamicSprite>() == 224);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, base) == 0);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, span_buffer) == 24);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, render_next) == 32);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, proxy_verts) == 40);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, actor) == 168);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, x1) == 176);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, y1) == 180);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, x2) == 184);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, y2) == 188);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, screen_x) == 192);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, screen_y) == 196);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, persp) == 200);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, volumetrics) == 208);
const _: () = assert!(std::mem::offset_of!(FDynamicSprite, leaf_lights) == 216);

/// `FBspDrawList` (`Render.h`). 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FBspDrawList {
    pub i_node: i32,
    pub i_surf: i32,
    pub i_zone: i32,
    pub key: i32,
    pub poly_flags: u32,
    pub span: FSpanBuffer,
    pub zone: *mut c_void,
    pub next: *mut FBspDrawList,
    pub surf_next: *mut FBspDrawList,
    pub volumetrics: *mut FActorLink,
    pub polys: *mut FSavedPoly,
    pub surf_lights: *mut FActorLink,
}

const _: () = assert!(size_of::<FBspDrawList>() == 128);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, i_node) == 0);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, i_surf) == 4);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, i_zone) == 8);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, key) == 12);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, poly_flags) == 16);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, span) == 24);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, zone) == 80);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, next) == 88);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, surf_next) == 96);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, volumetrics) == 104);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, polys) == 112);
const _: () = assert!(std::mem::offset_of!(FBspDrawList, surf_lights) == 120);

/// `URender::FStampedPoint { FTransform* Point; DWORD Stamp; }` (`Render.h`).
/// 16 bytes. Element type of the `PointCache` static.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FStampedPoint {
    pub point: *mut FTransform,
    pub stamp: u32,
}

const _: () = assert!(size_of::<FStampedPoint>() == 16);
const _: () = assert!(std::mem::offset_of!(FStampedPoint, point) == 0);
const _: () = assert!(std::mem::offset_of!(FStampedPoint, stamp) == 8);

/// `URender::FDynamicsCache { FDynamicItem* Dynamics[2]; }` (`Render.h`).
/// 16 bytes. Element type of the `DynamicsCache` static, indexed by `iNode`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FDynamicsCache {
    pub dynamics: [*mut FDynamicItem; 2],
}

const _: () = assert!(size_of::<FDynamicsCache>() == 16);
const _: () = assert!(std::mem::offset_of!(FDynamicsCache, dynamics) == 0);

/// `FSceneNode` (`UnRender.h`). 408 bytes. Every field `UnRender.cpp` reads
/// or writes, in declaration order. Per-frame scratch lives in the frame
/// itself, so Rust reads and writes these fields directly.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSceneNode {
    pub viewport: *mut c_void,
    pub level: *mut c_void,
    pub parent: *mut FSceneNode,
    pub sibling: *mut FSceneNode,
    pub child: *mut FSceneNode,
    pub i_surf: i32,
    pub zone_number: i32,
    pub recursion: i32,
    pub mirror: f32,
    pub near_clip: FPlane,
    pub coords: FCoords,
    pub uncoords: FCoords,
    pub span: *mut FSpanBuffer,
    pub draw: [*mut FBspDrawList; 3],
    pub sprite: *mut FDynamicSprite,
    pub x: i32,
    pub y: i32,
    pub xb: i32,
    pub yb: i32,
    pub fx: f32,
    pub fy: f32,
    pub fx15: f32,
    pub fy15: f32,
    pub fx2: f32,
    pub fy2: f32,
    pub zoom: f32,
    pub effective_fov_angle: f32,
    pub proj: FVector,
    pub r_proj: FVector,
    pub prj_xm: f32,
    pub prj_ym: f32,
    pub prj_xp: f32,
    pub prj_yp: f32,
    pub view_sides: [FVector; 4],
    pub view_planes: [FPlane; 4],
}

const _: () = assert!(size_of::<FSceneNode>() == 408);
const _: () = assert!(std::mem::offset_of!(FSceneNode, viewport) == 0);
const _: () = assert!(std::mem::offset_of!(FSceneNode, level) == 8);
const _: () = assert!(std::mem::offset_of!(FSceneNode, parent) == 16);
const _: () = assert!(std::mem::offset_of!(FSceneNode, sibling) == 24);
const _: () = assert!(std::mem::offset_of!(FSceneNode, child) == 32);
const _: () = assert!(std::mem::offset_of!(FSceneNode, i_surf) == 40);
const _: () = assert!(std::mem::offset_of!(FSceneNode, zone_number) == 44);
const _: () = assert!(std::mem::offset_of!(FSceneNode, recursion) == 48);
const _: () = assert!(std::mem::offset_of!(FSceneNode, mirror) == 52);
const _: () = assert!(std::mem::offset_of!(FSceneNode, near_clip) == 56);
const _: () = assert!(std::mem::offset_of!(FSceneNode, coords) == 72);
const _: () = assert!(std::mem::offset_of!(FSceneNode, uncoords) == 120);
const _: () = assert!(std::mem::offset_of!(FSceneNode, span) == 168);
const _: () = assert!(std::mem::offset_of!(FSceneNode, draw) == 176);
const _: () = assert!(std::mem::offset_of!(FSceneNode, sprite) == 200);
const _: () = assert!(std::mem::offset_of!(FSceneNode, x) == 208);
const _: () = assert!(std::mem::offset_of!(FSceneNode, y) == 212);
const _: () = assert!(std::mem::offset_of!(FSceneNode, xb) == 216);
const _: () = assert!(std::mem::offset_of!(FSceneNode, yb) == 220);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fx) == 224);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fy) == 228);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fx15) == 232);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fy15) == 236);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fx2) == 240);
const _: () = assert!(std::mem::offset_of!(FSceneNode, fy2) == 244);
const _: () = assert!(std::mem::offset_of!(FSceneNode, zoom) == 248);
const _: () = assert!(std::mem::offset_of!(FSceneNode, effective_fov_angle) == 252);
const _: () = assert!(std::mem::offset_of!(FSceneNode, proj) == 256);
const _: () = assert!(std::mem::offset_of!(FSceneNode, r_proj) == 268);
const _: () = assert!(std::mem::offset_of!(FSceneNode, prj_xm) == 280);
const _: () = assert!(std::mem::offset_of!(FSceneNode, prj_ym) == 284);
const _: () = assert!(std::mem::offset_of!(FSceneNode, prj_xp) == 288);
const _: () = assert!(std::mem::offset_of!(FSceneNode, prj_yp) == 292);
const _: () = assert!(std::mem::offset_of!(FSceneNode, view_sides) == 296);
const _: () = assert!(std::mem::offset_of!(FSceneNode, view_planes) == 344);
