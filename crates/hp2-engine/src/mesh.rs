//! Port of the `UMesh` non-serialization code in `Engine/Src/UnMesh.cpp`:
//! bounds (`GetRenderBoundingBox`, `GetCollisionBoundingBox`,
//! `GetCollisionExtent`, `GetBoneCoords`), the `GetAnimSeq` lookup, and the
//! generic `GetFrame` (`UnMesh.cpp:498-619`). The C++ shell keeps the
//! constructors, `Serialize`, and the `TLazyArray::Load()` calls, and passes
//! the loaded arrays in as slices.
//!
//! Cache layout: this module owns the `GCache` block it creates. Header
//! (16 bytes): `CachedMesh` (u64 key) at 0, `CachedFrame` (f32) at 8,
//! `CachedSeq` (i32 `FName`) at 12. `CachedVerts` (`FVector[FrameVerts]`)
//! starts at 16. No C++ code reads this block.
//!
//! Float fidelity: `*=`/`<<`/`TransformPointBy` use the fused dot product in
//! [`prim::dot`]. Lerps and the `CachedVerts` updates are plain multiply and
//! add (no `mul_add`), matching the disassembly of `UnMesh.cpp`.

use std::ffi::c_void;

use hp2_abi::{FCoords, FVector};

use crate::coords::{self, FRotator, FScale, SHEER_NONE, Trig, mul_comp};
use crate::math::app_floor;
use crate::prim::{self, Aabb};

/// `NAME_None`.
pub const NAME_NONE: i32 = 0;

/// Header bytes before `CachedVerts` in the `GCache` block.
const HEADER_LEN: usize = 16;
const OFF_CACHED_MESH: usize = 0;
const OFF_CACHED_FRAME: usize = 8;
const OFF_CACHED_SEQ: usize = 12;
/// `Align(HeaderSize, 16) - HeaderSize`.
const ALIGN_HINT: usize = 0;
/// `sizeof(FVector)`.
pub(crate) const VEC_LEN: usize = 12;

/// `FBox` (28 bytes): `Min`, `Max`, `INT IsValid`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FBoxRaw {
    pub min: FVector,
    pub max: FVector,
    pub is_valid: i32,
}

impl From<FBoxRaw> for Aabb {
    fn from(b: FBoxRaw) -> Aabb {
        Aabb {
            min: b.min,
            max: b.max,
            valid: b.is_valid != 0,
        }
    }
}

/// `FMeshAnimSeq` (40 bytes). `notifys` is the opaque `TArray<FMeshNotify>`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimSeqRaw {
    pub name: i32,
    pub group: i32,
    pub start_frame: i32,
    pub num_frames: i32,
    pub rate: f32,
    pub notifys: [u64; 2],
}

/// `AActor::AnimSequence` and `AActor::AnimFrame`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimState {
    pub sequence: i32,
    pub frame: f32,
}

/// The `AActor` pose fields read by the mesh code.
#[derive(Clone, Copy)]
pub struct ActorPose {
    pub location: FVector,
    pub pre_pivot: FVector,
    pub rotation: FRotator,
    pub draw_scale: f32,
}

/// The owner actor as seen by `UMesh`: its pose and animation state, plus
/// `Owner->Owner` for `bAnimByOwner`.
#[derive(Clone, Copy)]
pub struct OwnerView {
    pub pose: ActorPose,
    pub anim: AnimState,
    /// `Owner->Owner` animation state, `Some` iff `Owner->Owner != NULL`.
    pub parent_anim: Option<AnimState>,
    pub anim_by_owner: bool,
}

impl OwnerView {
    /// `AnimOwner` (`UnMesh.cpp:514`): `Owner->Owner` when `bAnimByOwner` and
    /// it is non-null, otherwise `Owner`.
    pub fn anim_owner(&self) -> AnimState {
        match self.parent_anim {
            Some(p) if self.anim_by_owner => p,
            _ => self.anim,
        }
    }
}

/// The loaded `UMesh` fields used by the bounds and frame code.
pub struct MeshView<'a> {
    pub frame_verts: i32,
    /// `Verts`: packed `FMeshVert` words (see [`decode_vert`]).
    pub verts: &'a [u32],
    pub anim_seqs: &'a [AnimSeqRaw],
    pub bounding_boxes: &'a [FBoxRaw],
    pub bounding_box: FBoxRaw,
    pub scale: FVector,
    pub origin: FVector,
    pub rot_origin: FRotator,
}

/// One `GCache` block, as returned by the C++ cache services. `ptr` addresses
/// `len` writable bytes until `unlock` is called.
#[derive(Clone, Copy)]
pub struct CacheBlock {
    pub ptr: *mut u8,
    pub len: usize,
    /// The `FMemCache::FCacheItem*` to unlock.
    pub item: *mut c_void,
}

/// `GCache` operations used by `GetFrame`.
pub trait FrameCache {
    /// `GCache.Get(id, Item)`: `None` when the block is not cached. `len` is the
    /// size the caller expects; `GCache` does not report it, so the returned block carries it.
    fn get(&mut self, id: u64, len: usize) -> Option<CacheBlock>;
    /// `GCache.Create(id, Item, len)`.
    fn create(&mut self, id: u64, len: usize) -> CacheBlock;
    /// `GCache.Flush(id)`.
    fn flush(&mut self, id: u64);
    /// `Item->Unlock()`.
    fn unlock(&mut self, block: CacheBlock);
}

/// Cache identity: `CacheID` (from `MakeCacheID`, computed in C++) and the
/// mesh key (`this`) stored in the header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CacheKey {
    pub id: u64,
    pub mesh: u64,
}

/// `FMeshVert` (4 bytes): `INT X:11, Y:11, Z:10`, little-endian bit order
/// (`__INTEL_BYTE_ORDER__`). Each field is sign-extended.
pub fn decode_vert(raw: u32) -> FVector {
    let x = ((raw << 21) as i32) >> 21;
    let y = (((raw >> 11) << 21) as i32) >> 21;
    let z = (raw as i32) >> 22;
    FVector {
        x: x as f32,
        y: y as f32,
        z: z as f32,
    }
}

/// `UMesh::GetAnimSeq` (`UnMesh.h:255`): the first sequence named `name`.
pub fn anim_seq_index(seqs: &[AnimSeqRaw], name: i32) -> Option<usize> {
    seqs.iter().position(|s| s.name == name)
}

/// `UMesh::GetRenderBoundingBox` (`UnMesh.cpp:72`). The `Exact` argument is
/// unused in the original and is not carried over.
pub fn render_bounding_box(mesh: &MeshView, owner: &OwnerView, trig: &Trig) -> FCoords {
    let seq = anim_seq_index(mesh.anim_seqs, owner.anim.sequence).map(|i| &mesh.anim_seqs[i]);
    let bound = match seq {
        Some(s) if owner.anim.frame >= 0.0 => {
            // Animating: the box enclosing two frames' bounds.
            let i_frame = app_floor((owner.anim.frame + 1.0) * s.num_frames as f32);
            let f1 = s.start_frame.wrapping_add(frame_mod(i_frame, s.num_frames));
            let f2 = s
                .start_frame
                .wrapping_add(frame_mod(i_frame.wrapping_add(1), s.num_frames));
            let mut b = Aabb::from(bounding_box_at(mesh, f1));
            b.add_box(&Aabb::from(bounding_box_at(mesh, f2)));
            b
        }
        // Interpolating: be pessimistic and use the whole-mesh bound.
        _ => Aabb::from(mesh.bounding_box),
    };

    // FCoords(-Origin) << (Scale*DrawScale) << RotOrigin << PrePivot << Rotation << Location.
    // `<< FVector` is `/= FVector` (Origin += P), so the scaled vector is added, not multiplied.
    let mut c = coords::coords_from_origin(prim::neg(mesh.origin));
    coords::div_vec(&mut c, prim::scale(mesh.scale, owner.pose.draw_scale));
    coords::div_rot(&mut c, mesh.rot_origin, trig);
    coords::div_vec(&mut c, owner.pose.pre_pivot);
    coords::div_rot(&mut c, owner.pose.rotation, trig);
    coords::div_vec(&mut c, owner.pose.location);
    coords::box_times_coords(&bound, &c)
}

/// `UMesh::GetCollisionBoundingBox` (`UnMesh.cpp:104`).
pub fn collision_bounding_box(
    mesh: &MeshView,
    owner: &OwnerView,
    world: bool,
    trig: &Trig,
) -> Aabb {
    let box_scale = prim::scale(mesh.scale, owner.pose.draw_scale);
    let bb = mesh.bounding_box;
    let bound = Aabb::from_min_max(
        mul_comp(box_scale, prim::sub(bb.min, mesh.origin)),
        mul_comp(box_scale, prim::sub(bb.max, mesh.origin)),
    );
    if !world {
        return bound;
    }
    // GMath.UnitCoords / RotOrigin / Owner->Rotation, then the actor location.
    let mut c = coords::unit_coords();
    coords::div_rot(&mut c, mesh.rot_origin, trig);
    coords::div_rot(&mut c, owner.pose.rotation, trig);
    c.origin = prim::add(owner.pose.location, owner.pose.pre_pivot);
    bound.transform_by(&prim::transpose(&c))
}

/// `UMesh::GetCollisionExtent` (`UnMesh.cpp:126`).
pub fn collision_extent(mesh: &MeshView, owner: &OwnerView, trig: &Trig) -> FVector {
    let b = collision_bounding_box(mesh, owner, false, trig);
    prim::v3(
        prim::max_f(-b.min.x, b.max.x),
        prim::max_f(-b.min.y, b.max.y),
        prim::max_f(-b.min.z, b.max.z),
    )
}

/// `UMesh::GetBoneCoords` (`UnMesh.cpp:136`). `Bone` is unused in the original.
pub fn bone_coords(mesh: &MeshView, owner: &OwnerView, trig: &Trig) -> FCoords {
    let mut c = coords::coords_from_origin(prim::neg(mesh.origin));
    coords::div_scale(
        &mut c,
        &FScale {
            scale: prim::scale(mesh.scale, owner.pose.draw_scale),
            sheer_rate: 0.0,
            sheer_axis: SHEER_NONE,
        },
    );
    coords::div_rot(&mut c, mesh.rot_origin, trig);
    coords::div_rot(&mut c, owner.pose.rotation, trig);
    coords::div_vec(&mut c, owner.pose.pre_pivot);
    coords::div_vec(&mut c, owner.pose.location);
    c
}

/// `UMesh::GetFrame` generic path (`UnMesh.cpp:498-619`). Calls `emit(i, p)`
/// for each vertex `i` in `0..FrameVerts` in order.
pub fn get_frame(
    mesh: &MeshView,
    owner: &OwnerView,
    coords: FCoords,
    trig: &Trig,
    cache: &mut dyn FrameCache,
    key: CacheKey,
    emit: &mut dyn FnMut(usize, FVector),
) {
    let anim_owner = owner.anim_owner();
    let n = mesh.frame_verts as usize;
    let (block, was_cached) = acquire(cache, key, HEADER_LEN + ALIGN_HINT + n * VEC_LEN);
    // SAFETY: `acquire` returns a locked block of `block.len` bytes, valid until `unlock`.
    let buf = unsafe { block_bytes(block) };
    if !was_cached {
        wr_u64(buf, OFF_CACHED_MESH, key.mesh);
        wr_f32(buf, OFF_CACHED_FRAME, 0.0);
        wr_i32(buf, OFF_CACHED_SEQ, NAME_NONE);
    }
    let c = pose_coords(coords, owner, mesh, trig);
    let seq = anim_seq_index(mesh.anim_seqs, anim_owner.sequence).map(|i| &mesh.anim_seqs[i]);
    let verts_at = HEADER_LEN + ALIGN_HINT;

    if anim_owner.frame >= 0.0 || !was_cached {
        // Interpolate two frames.
        let (alpha, off1, off2) = interp_offsets(seq, anim_owner.frame, mesh.frame_verts);
        for i in 0..n {
            let v1 = decode_vert(mesh.verts[off1 + i]);
            let v2 = decode_vert(mesh.verts[off2 + i]);
            let cur = prim::add(v1, prim::scale(prim::sub(v2, v1), alpha));
            wr_vec(buf, verts_at + i * VEC_LEN, cur);
            emit(i, prim::transform_point(prim::sub(cur, mesh.origin), &c));
        }
    } else {
        // Tween from the cached pose toward the current frame.
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
        if rd_i32(buf, OFF_CACHED_SEQ) != anim_owner.sequence || alpha_outside_unit(alpha) {
            wr_i32(buf, OFF_CACHED_SEQ, anim_owner.sequence);
            wr_f32(buf, OFF_CACHED_FRAME, start_frame);
            alpha = 0.0;
        }
        for i in 0..n {
            let v2 = decode_vert(mesh.verts[off + i]);
            let cv = rd_vec(buf, verts_at + i * VEC_LEN);
            let cur = prim::add(cv, prim::scale(prim::sub(v2, cv), alpha));
            wr_vec(buf, verts_at + i * VEC_LEN, cur);
            emit(i, prim::transform_point(prim::sub(cur, mesh.origin), &c));
        }
        wr_f32(buf, OFF_CACHED_FRAME, anim_owner.frame);
    }
    cache.unlock(block);
}

/// The owner-relative coords chain (`UnMesh.cpp:513`, `UnMeshLOD` shares it):
/// `Coords * (Location+PrePivot) * Rotation * RotOrigin * FScale(Scale*DrawScale)`.
pub(crate) fn pose_coords(
    coords: FCoords,
    owner: &OwnerView,
    mesh: &MeshView,
    trig: &Trig,
) -> FCoords {
    let mut c = coords;
    coords::mul_vec(&mut c, prim::add(owner.pose.location, owner.pose.pre_pivot));
    coords::mul_rot(&mut c, owner.pose.rotation, trig);
    coords::mul_rot(&mut c, mesh.rot_origin, trig);
    coords::mul_scale(
        &mut c,
        &FScale {
            scale: prim::scale(mesh.scale, owner.pose.draw_scale),
            sheer_rate: 0.0,
            sheer_axis: SHEER_NONE,
        },
    );
    c
}

/// Interpolation numbers for the two frames around `anim_frame`:
/// `(alpha, offset1, offset2)`, offsets in `Verts` units.
pub(crate) fn interp_offsets(
    seq: Option<&AnimSeqRaw>,
    anim_frame: f32,
    frame_verts: i32,
) -> (f32, usize, usize) {
    match seq {
        Some(s) => {
            let frame = prim::max_f(anim_frame, 0.0) * s.num_frames as f32;
            let i_frame = app_floor(frame);
            let alpha = frame - i_frame as f32;
            let off1 = s
                .start_frame
                .wrapping_add(frame_mod(i_frame, s.num_frames))
                .wrapping_mul(frame_verts);
            let off2 = s
                .start_frame
                .wrapping_add(frame_mod(i_frame.wrapping_add(1), s.num_frames))
                .wrapping_mul(frame_verts);
            (alpha, off1 as usize, off2 as usize)
        }
        None => (0.0, 0, 0),
    }
}

/// `i % n` as the arm64 build computes it for the C++ `(Frame + k) % NumFrames`.
/// `NumFrames == 0` gives `sdiv` 0 and `msub` leaves the dividend, so the
/// result is `i`. Rust's `%` would panic on zero, so the guard mirrors the
/// shipped behaviour instead.
fn frame_mod(i: i32, n: i32) -> i32 {
    if n == 0 { i } else { i.wrapping_rem(n) }
}

fn bounding_box_at(mesh: &MeshView, i: i32) -> FBoxRaw {
    mesh.bounding_boxes[i as usize]
}

/// C++ `Alpha<0.f || Alpha>1.f`. Kept in this form: NaN compares false and does not reset.
#[allow(clippy::manual_range_contains)]
pub(crate) fn alpha_outside_unit(alpha: f32) -> bool {
    alpha < 0.0 || alpha > 1.0
}

/// `GCache.Get` plus the `CachedMesh` identity check. A stale block is
/// unlocked and flushed, then recreated. Returns `(block, WasCached)`.
pub(crate) fn acquire(cache: &mut dyn FrameCache, key: CacheKey, len: usize) -> (CacheBlock, bool) {
    if let Some(old) = cache.get(key.id, len) {
        // SAFETY: `get` returns a locked block of `old.len` bytes.
        let stored = rd_u64(unsafe { block_bytes(old) }, OFF_CACHED_MESH);
        if stored == key.mesh {
            return (old, true);
        }
        // Actor's mesh changed.
        cache.unlock(old);
        cache.flush(key.id);
    }
    (cache.create(key.id, len), false)
}

/// Views a locked block as bytes.
///
/// # Safety
/// `block.ptr` must address `block.len` writable bytes that remain valid
/// until the block is unlocked, and no other live reference may alias them.
pub(crate) unsafe fn block_bytes<'a>(block: CacheBlock) -> &'a mut [u8] {
    // SAFETY: upheld by the caller per the contract above.
    unsafe { std::slice::from_raw_parts_mut(block.ptr, block.len) }
}

pub(crate) fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_ne_bytes(rd4(b, o))
}

pub(crate) fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_ne_bytes(rd4(b, o))
}

pub(crate) fn rd_u64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_ne_bytes(a)
}

fn rd4(b: &[u8], o: usize) -> [u8; 4] {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[o..o + 4]);
    a
}

pub(crate) fn wr_f32(b: &mut [u8], o: usize, v: f32) {
    b[o..o + 4].copy_from_slice(&v.to_ne_bytes());
}

pub(crate) fn wr_i32(b: &mut [u8], o: usize, v: i32) {
    b[o..o + 4].copy_from_slice(&v.to_ne_bytes());
}

pub(crate) fn wr_u64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_ne_bytes());
}

pub(crate) fn rd_vec(b: &[u8], o: usize) -> FVector {
    prim::v3(rd_f32(b, o), rd_f32(b, o + 4), rd_f32(b, o + 8))
}

pub(crate) fn wr_vec(b: &mut [u8], o: usize, v: FVector) {
    wr_f32(b, o, v.x);
    wr_f32(b, o + 4, v.y);
    wr_f32(b, o + 8, v.z);
}
