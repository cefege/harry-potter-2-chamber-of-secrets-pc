//! Port of the `ULodMesh` non-serialization code in
//! `Engine/Src/ULodMesh.cpp`: `GetFrame` (`ULodMesh.cpp:154-358`) and the
//! `MeshScaleMax` computation from `SetScale` (`ULodMesh.cpp:91-101`).
//! Bounds, the coords chain, and `GetAnimSeq` are shared with `mesh.rs`.
//!
//! Cache layout: the `GCache` block this module creates has a 24-byte
//! `CFLodHeader`: `CachedMesh` (u64) at 0, `CachedFrame` (f32) at 8,
//! `CachedSeq` (i32) at 12, `CachedLodVerts` (i32) at 16, `TweenIndicator`
//! (f32) at 20. The 8-byte align hint puts `CachedVerts` at 32. No C++ code
//! reads this block.

use hp2_abi::{FCoords, FVector};

use crate::coords::Trig;
use crate::mesh::{
    self, CacheKey, FrameCache, MeshView, NAME_NONE, OwnerView, VEC_LEN, anim_seq_index,
    block_bytes, interp_offsets, pose_coords, rd_f32, rd_i32, rd_vec, wr_f32, wr_i32, wr_u64,
    wr_vec,
};
use crate::prim;

/// `sizeof(CFLodHeader)`.
const LOD_HEADER_LEN: usize = 24;
/// `Align(sizeof(CFLodHeader), 16) - sizeof(CFLodHeader)`.
const LOD_ALIGN_HINT: usize = 8;
const LOD_VERTS_AT: usize = LOD_HEADER_LEN + LOD_ALIGN_HINT;
const OFF_CACHED_MESH: usize = 0;
const OFF_CACHED_FRAME: usize = 8;
const OFF_CACHED_SEQ: usize = 12;
const OFF_CACHED_LOD_VERTS: usize = 16;
const OFF_TWEEN_INDICATOR: usize = 20;

/// The loaded `ULodMesh` fields used by `GetFrame`.
pub struct LodView<'a> {
    pub base: MeshView<'a>,
    pub special_verts: i32,
    /// `LODRequest` on entry: regular vertices requested.
    pub lod_request: i32,
}

/// `ULodMesh::GetFrame` (`ULodMesh.cpp:154`). Calls `emit(i, p)` for each
/// output vertex `i` in `0..VertexNum`, in order. Returns the new `LODRequest`
/// (regular vertices returned).
pub fn get_frame(
    lod: &LodView,
    owner: &OwnerView,
    coords: FCoords,
    trig: &Trig,
    cache: &mut dyn FrameCache,
    key: CacheKey,
    emit: &mut dyn FnMut(usize, FVector),
) -> i32 {
    let mesh = &lod.base;
    let anim_owner = owner.anim_owner();
    // VertsRequested = Min(LODRequest + SpecialVerts, FrameVerts).
    let verts_requested = min_i32(lod.lod_request + lod.special_verts, mesh.frame_verts);
    let mut vertex_num = verts_requested;

    let body = mesh.frame_verts as usize * VEC_LEN;
    let (block, was_cached) = mesh::acquire(cache, key, LOD_VERTS_AT + body);
    // SAFETY: `acquire` returns a locked block of `block.len` bytes, valid until `unlock`.
    let buf = unsafe { block_bytes(block) };
    if !was_cached {
        // Created fresh: a new cache starts with full tween indicator.
        wr_f32(buf, OFF_TWEEN_INDICATOR, 1.0);
        wr_u64(buf, OFF_CACHED_MESH, key.mesh);
        wr_i32(buf, OFF_CACHED_SEQ, NAME_NONE);
        wr_f32(buf, OFF_CACHED_FRAME, 0.0);
        wr_i32(buf, OFF_CACHED_LOD_VERTS, 0);
    }

    let c = pose_coords(coords, owner, mesh, trig);
    let seq = anim_seq_index(mesh.anim_seqs, anim_owner.sequence).map(|i| &mesh.anim_seqs[i]);
    // Integrate the origin offset into the screen-space transform.
    let mut local = c;
    local.origin = prim::add(local.origin, mesh.origin);

    let mut lod_request;
    if anim_owner.frame >= 0.0 || !was_cached {
        // Interpolate two frames.
        lod_request = vertex_num - lod.special_verts;
        wr_i32(buf, OFF_CACHED_LOD_VERTS, vertex_num);

        let (alpha, off1, off2) = interp_offsets(seq, anim_owner.frame, mesh.frame_verts);
        if alpha <= 0.0 {
            for i in 0..count(vertex_num) {
                let v1 = mesh::decode_vert(mesh.verts[off1 + i]);
                wr_vec(buf, LOD_VERTS_AT + i * VEC_LEN, v1);
                emit(i, prim::transform_point(v1, &local));
            }
        } else {
            for i in 0..count(vertex_num) {
                let v1 = mesh::decode_vert(mesh.verts[off1 + i]);
                let v2 = mesh::decode_vert(mesh.verts[off2 + i]);
                let cur = prim::add(v1, prim::scale(prim::sub(v2, v1), alpha));
                wr_vec(buf, LOD_VERTS_AT + i * VEC_LEN, cur);
                emit(i, prim::transform_point(cur, &local));
            }
        }
    } else {
        // Tween: cache present and AnimFrame < 0.
        // The cache can only tween as far as it was filled.
        vertex_num = min_i32(vertex_num, rd_i32(buf, OFF_CACHED_LOD_VERTS));
        wr_i32(buf, OFF_CACHED_LOD_VERTS, vertex_num);
        lod_request = vertex_num - lod.special_verts;

        let start_frame = match seq {
            Some(s) => -1.0 / s.num_frames as f32,
            None => 0.0,
        };
        let off = match seq {
            Some(s) => s.start_frame.wrapping_mul(mesh.frame_verts) as usize,
            None => 0,
        };
        let cached_frame = rd_f32(buf, OFF_CACHED_FRAME);
        let mut alpha = 1.0 - anim_owner.frame / cached_frame;

        if rd_i32(buf, OFF_CACHED_SEQ) != anim_owner.sequence {
            wr_f32(buf, OFF_TWEEN_INDICATOR, 0.0);
        }
        if rd_i32(buf, OFF_CACHED_SEQ) != anim_owner.sequence || mesh::alpha_outside_unit(alpha) {
            wr_f32(buf, OFF_CACHED_FRAME, start_frame);
            alpha = 0.0;
            wr_i32(buf, OFF_CACHED_SEQ, anim_owner.sequence);
        }

        // ULodMesh.cpp:310 `TweenIndicator += (1 - TweenIndicator) * Alpha` contracts to one fmadd.
        let t0 = rd_f32(buf, OFF_TWEEN_INDICATOR);
        let t = (1.0 - t0).mul_add(alpha, t0);
        wr_f32(buf, OFF_TWEEN_INDICATOR, t);

        if t > 0.97 {
            // Destination (practically) reached: skip the tween.
            alpha = 0.0;
            // The cache may hold fewer vertices than requested: fill the rest from the target frame.
            if vertex_num < verts_requested {
                for i in count(vertex_num)..count(verts_requested) {
                    let v = mesh::decode_vert(mesh.verts[off + i]);
                    wr_vec(buf, LOD_VERTS_AT + i * VEC_LEN, v);
                }
                vertex_num = verts_requested;
                lod_request = vertex_num - lod.special_verts;
                wr_i32(buf, OFF_CACHED_LOD_VERTS, vertex_num);
            }
        }

        if alpha <= 0.0 {
            for i in 0..count(vertex_num) {
                let cv = rd_vec(buf, LOD_VERTS_AT + i * VEC_LEN);
                emit(i, prim::transform_point(cv, &local));
            }
        } else {
            for i in 0..count(vertex_num) {
                let v2 = mesh::decode_vert(mesh.verts[off + i]);
                let cv = rd_vec(buf, LOD_VERTS_AT + i * VEC_LEN);
                let cur = prim::add(cv, prim::scale(prim::sub(v2, cv), alpha));
                wr_vec(buf, LOD_VERTS_AT + i * VEC_LEN, cur);
                emit(i, prim::transform_point(cur, &local));
            }
        }
        wr_f32(buf, OFF_CACHED_FRAME, anim_owner.frame);
    }

    cache.unlock(block);
    lod_request
}

/// `ULodMesh::SetScale`'s `MeshScaleMax` (`ULodMesh.cpp:97`):
/// `(1/128) * BoundingSphere.W * Max(Abs(X), Max(Abs(Y), Abs(Z)))`.
pub fn mesh_scale_max(bounding_sphere_w: f32, scale: FVector) -> f32 {
    let m = prim::max_f(abs_f(scale.y), abs_f(scale.z));
    (1.0 / 128.0) * bounding_sphere_w * prim::max_f(abs_f(scale.x), m)
}

/// `Abs` (`UnTemplate.h:103`): `(A >= 0) ? A : -A`.
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `Min(A, B)` for `INT` (`UnTemplate.h`): `(A <= B) ? A : B`.
fn min_i32(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

/// Loop count for a C++ `for (i = 0; i < n; i++)`: zero for `n <= 0`.
fn count(n: i32) -> usize {
    n.max(0) as usize
}
