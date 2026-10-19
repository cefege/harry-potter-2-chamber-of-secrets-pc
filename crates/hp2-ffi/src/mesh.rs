//! C ABI for the `UMesh` and `ULodMesh` ports (`hp2_engine::mesh`,
//! `hp2_engine::lodmesh`). The C++ shells in `UnMesh.cpp` and `ULodMesh.cpp`
//! fill `Hp2MeshArg`, `Hp2LodArg` and `Hp2OwnerArg` from the objects and pass
//! `Hp2MeshServices` (the `GCache` and `GMath` entry points). The layouts are
//! mirrored in `Hp2MeshServices.h` and checked there with `static_assert`.
//!
//! Null policy: a null services, input or output pointer makes the export
//! return its default without writing. C++ callers always pass live objects,
//! so this only avoids UB.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::engine::generated::FRotator;
use hp2_abi::{FCoords, FVector};
use hp2_engine::coords::Trig;
use hp2_engine::lodmesh::{self, LodView};
use hp2_engine::mesh::{
    self, ActorPose, AnimSeqRaw, AnimState, CacheBlock, CacheKey, FBoxRaw, FrameCache, MeshView,
    OwnerView,
};
use hp2_ffi_support::guard;

/// `Hp2MeshServices` in `Hp2MeshServices.h`: the `GCache` and `GMath` entry
/// points the core calls through. Filled by C++ once; never written by Rust.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2MeshServices {
    /// `GCache.Get(id, Item)`: the block or null; `*item` receives the `FCacheItem*`.
    pub cache_get: unsafe extern "C" fn(id: u64, item: *mut *mut c_void) -> *mut u8,
    /// `GCache.Create(id, Item, len)`.
    pub cache_create: unsafe extern "C" fn(id: u64, len: i32, item: *mut *mut c_void) -> *mut u8,
    /// `GCache.Flush(id)`.
    pub cache_flush: unsafe extern "C" fn(id: u64),
    /// `Item->Unlock()`.
    pub cache_unlock: unsafe extern "C" fn(item: *mut c_void),
    /// `GMath.SinTab(i)`; the C++ side applies the table masking.
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GMath.CosTab(i)`.
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
}

/// `Hp2MeshArg` in `Hp2MeshServices.h`: the `UMesh` fields the bounds and
/// frame code reads. C++ fills it after `Verts.Load()`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2MeshArg {
    pub frame_verts: i32,
    /// `Verts.GetData()`: `num_verts` packed `FMeshVert` words.
    pub verts: *const u32,
    pub num_verts: i32,
    /// `AnimSeqs.GetData()`: `num_anim_seqs` `FMeshAnimSeq` records.
    pub anim_seqs: *const AnimSeqRaw,
    pub num_anim_seqs: i32,
    /// `BoundingBoxes.GetData()`: `num_bounding_boxes` `FBox` records.
    pub bounding_boxes: *const FBoxRaw,
    pub num_bounding_boxes: i32,
    pub bounding_box: FBoxRaw,
    pub scale: FVector,
    pub origin: FVector,
    pub rot_origin: FRotator,
    /// `this` (the `UMesh*`). The cache block stores it to detect a changed mesh.
    pub mesh_key: u64,
}

/// `Hp2LodArg` in `Hp2MeshServices.h`: a `Hp2MeshArg` plus the `ULodMesh` fields.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LodArg {
    pub mesh: Hp2MeshArg,
    /// `SpecialVerts`.
    pub special_verts: i32,
    /// `LODRequest` on entry (regular vertices requested).
    pub lod_request: i32,
}

/// `Hp2OwnerArg` in `Hp2MeshServices.h`: the `AActor` fields the mesh code
/// reads. Booleans and `UBOOL` are nonzero for true.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2OwnerArg {
    pub location: FVector,
    pub pre_pivot: FVector,
    pub rotation: FRotator,
    pub draw_scale: f32,
    /// `Owner->AnimSequence` and `Owner->AnimFrame`.
    pub anim_sequence: i32,
    pub anim_frame: f32,
    /// Nonzero iff `Owner->Owner != NULL`; the parent fields are read only then.
    pub has_parent: i32,
    pub parent_sequence: i32,
    pub parent_frame: f32,
    /// `Owner->bAnimByOwner`.
    pub anim_by_owner: i32,
    /// `MakeCacheID(CID_TweenAnim, Owner, NULL)`, computed in C++.
    pub cache_id: u64,
}

/// Borrows the `T` at `p`, or `None` when `p` is null.
///
/// # Safety
/// A non-null `p` must point to a live `T` for the chosen lifetime.
unsafe fn at<'a, T>(p: *const c_void) -> Option<&'a T> {
    unsafe { p.cast::<T>().as_ref() }
}

/// `n` elements at `p`; a null pointer or negative count yields an empty slice.
///
/// # Safety
/// A non-null `p` must point to `n` live `T` values for the chosen lifetime.
unsafe fn slice<'a, T>(p: *const T, n: i32) -> &'a [T] {
    if p.is_null() || n <= 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, n as usize) }
    }
}

/// Views the `UMesh` argument as the core's [`MeshView`].
///
/// # Safety
/// The array pointers in `m` must be valid for their counts while the view lives.
unsafe fn mesh_view(m: &Hp2MeshArg) -> MeshView<'_> {
    unsafe {
        MeshView {
            frame_verts: m.frame_verts,
            verts: slice(m.verts, m.num_verts),
            anim_seqs: slice(m.anim_seqs, m.num_anim_seqs),
            bounding_boxes: slice(m.bounding_boxes, m.num_bounding_boxes),
            bounding_box: m.bounding_box,
            scale: m.scale,
            origin: m.origin,
            rot_origin: m.rot_origin,
        }
    }
}

/// The owner as the core's [`OwnerView`]. Reads only the argument.
fn owner_view(o: &Hp2OwnerArg) -> OwnerView {
    OwnerView {
        pose: ActorPose {
            location: o.location,
            pre_pivot: o.pre_pivot,
            rotation: o.rotation,
            draw_scale: o.draw_scale,
        },
        anim: AnimState {
            sequence: o.anim_sequence,
            frame: o.anim_frame,
        },
        parent_anim: (o.has_parent != 0).then_some(AnimState {
            sequence: o.parent_sequence,
            frame: o.parent_frame,
        }),
        anim_by_owner: o.anim_by_owner != 0,
    }
}

/// `GCache` through the C++ services, as the core's [`FrameCache`].
struct ServiceCache<'a> {
    svc: &'a Hp2MeshServices,
}

impl FrameCache for ServiceCache<'_> {
    fn get(&mut self, id: u64, len: usize) -> Option<CacheBlock> {
        let mut item = ptr::null_mut();
        // SAFETY: the services are C++ functions, valid for the whole export.
        let ptr = unsafe { (self.svc.cache_get)(id, &mut item) };
        (!ptr.is_null()).then_some(CacheBlock { ptr, len, item })
    }

    fn create(&mut self, id: u64, len: usize) -> CacheBlock {
        let size = i32::try_from(len).expect("GCache block size fits INT");
        let mut item = ptr::null_mut();
        // SAFETY: as in `get`.
        let ptr = unsafe { (self.svc.cache_create)(id, size, &mut item) };
        assert!(!ptr.is_null(), "GCache.Create returned NULL");
        CacheBlock { ptr, len, item }
    }

    fn flush(&mut self, id: u64) {
        // SAFETY: as in `get`.
        unsafe { (self.svc.cache_flush)(id) }
    }

    fn unlock(&mut self, block: CacheBlock) {
        // SAFETY: as in `get`; `item` came from `get` or `create`.
        unsafe { (self.svc.cache_unlock)(block.item) }
    }
}

/// Runs `f` with the `GMath` sine and cosine tables from the services.
fn with_trig<R>(svc: &Hp2MeshServices, f: impl FnOnce(&Trig) -> R) -> R {
    let sin = |i: i32| unsafe { (svc.sin_tab)(i) };
    let cos = |i: i32| unsafe { (svc.cos_tab)(i) };
    f(&Trig {
        sin: &sin,
        cos: &cos,
    })
}

/// Writes output vertex `i` as an `FVector` at `out + i * stride` (the
/// `ResultVerts` walk in `GetFrame`). A null `out` writes nothing.
///
/// # Safety
/// Non-null `out` must be valid for an `FVector` write at every emitted index.
unsafe fn write_vert(out: *mut c_void, stride: i32, i: usize, v: FVector) {
    if out.is_null() {
        return;
    }
    unsafe {
        out.cast::<u8>()
            .offset(i as isize * stride as isize)
            .cast::<FVector>()
            .write_unaligned(v);
    }
}

/// Writes the render bounding box (`UMesh::GetRenderBoundingBox`) as `FCoords`.
///
/// # Safety
/// `svc`, `mesh`, `owner`, `out` must be null or live with the layouts above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_render_bounding_box(
    svc: *const c_void,
    mesh: *const c_void,
    owner: *const c_void,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_mesh_render_bounding_box",
        (),
        move || unsafe {
            let (Some(svc), Some(m), Some(o), Some(out)) = (
                at::<Hp2MeshServices>(svc),
                at::<Hp2MeshArg>(mesh),
                at::<Hp2OwnerArg>(owner),
                out.cast::<FCoords>().as_mut(),
            ) else {
                return;
            };
            let mv = mesh_view(m);
            let ov = owner_view(o);
            *out = with_trig(svc, |t| mesh::render_bounding_box(&mv, &ov, t));
        },
    )
}

/// Writes the collision `FBox` (`UMesh::GetCollisionBoundingBox`).
/// `world != 0` transforms by the owner's location.
///
/// # Safety
/// As [`hp2_rs_engine_mesh_render_bounding_box`]; `out` is an `FBox`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_collision_bounding_box(
    svc: *const c_void,
    mesh: *const c_void,
    owner: *const c_void,
    world: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_mesh_collision_bounding_box",
        (),
        move || unsafe {
            let (Some(svc), Some(m), Some(o), Some(out)) = (
                at::<Hp2MeshServices>(svc),
                at::<Hp2MeshArg>(mesh),
                at::<Hp2OwnerArg>(owner),
                out.cast::<FBoxRaw>().as_mut(),
            ) else {
                return;
            };
            let mv = mesh_view(m);
            let ov = owner_view(o);
            let b = with_trig(svc, |t| {
                mesh::collision_bounding_box(&mv, &ov, world != 0, t)
            });
            *out = FBoxRaw {
                min: b.min,
                max: b.max,
                is_valid: i32::from(b.valid),
            };
        },
    )
}

/// Writes the collision extent (`UMesh::GetCollisionExtent`) as `FVector`.
///
/// # Safety
/// As [`hp2_rs_engine_mesh_render_bounding_box`]; `out` is an `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_collision_extent(
    svc: *const c_void,
    mesh: *const c_void,
    owner: *const c_void,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_mesh_collision_extent", (), move || unsafe {
        let (Some(svc), Some(m), Some(o), Some(out)) = (
            at::<Hp2MeshServices>(svc),
            at::<Hp2MeshArg>(mesh),
            at::<Hp2OwnerArg>(owner),
            out.cast::<FVector>().as_mut(),
        ) else {
            return;
        };
        let mv = mesh_view(m);
        let ov = owner_view(o);
        *out = with_trig(svc, |t| mesh::collision_extent(&mv, &ov, t));
    })
}

/// Writes the bone coords (`UMesh::GetBoneCoords`) as `FCoords`.
///
/// # Safety
/// As [`hp2_rs_engine_mesh_render_bounding_box`]; `out` is an `FCoords`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_bone_coords(
    svc: *const c_void,
    mesh: *const c_void,
    owner: *const c_void,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_mesh_bone_coords", (), move || unsafe {
        let (Some(svc), Some(m), Some(o), Some(out)) = (
            at::<Hp2MeshServices>(svc),
            at::<Hp2MeshArg>(mesh),
            at::<Hp2OwnerArg>(owner),
            out.cast::<FCoords>().as_mut(),
        ) else {
            return;
        };
        let mv = mesh_view(m);
        let ov = owner_view(o);
        *out = with_trig(svc, |t| mesh::bone_coords(&mv, &ov, t));
    })
}

/// `UMesh::GetAnimSeq`: index of the first sequence named `name`, or `-1`.
///
/// # Safety
/// `seqs` must be null or point to `num` live `FMeshAnimSeq` records.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_anim_seq_index(
    seqs: *const c_void,
    num: i32,
    name: i32,
) -> i32 {
    guard("hp2_rs_engine_mesh_anim_seq_index", -1, move || unsafe {
        let seqs = slice(seqs.cast::<AnimSeqRaw>(), num);
        mesh::anim_seq_index(seqs, name).map_or(-1, |i| i as i32)
    })
}

/// `UMesh::GetFrame`: writes `FrameVerts` vertices at `out + i * stride`.
///
/// # Safety
/// As [`hp2_rs_engine_mesh_render_bounding_box`]; `coords` is an `FCoords`;
/// `out` must be valid for `FrameVerts` strided `FVector` writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_mesh_get_frame(
    svc: *const c_void,
    mesh: *const c_void,
    owner: *const c_void,
    coords: *const c_void,
    out: *mut c_void,
    stride: i32,
) {
    guard("hp2_rs_engine_mesh_get_frame", (), move || unsafe {
        let (Some(svc), Some(m), Some(o), Some(c)) = (
            at::<Hp2MeshServices>(svc),
            at::<Hp2MeshArg>(mesh),
            at::<Hp2OwnerArg>(owner),
            at::<FCoords>(coords),
        ) else {
            return;
        };
        let mv = mesh_view(m);
        let ov = owner_view(o);
        let key = CacheKey {
            id: o.cache_id,
            mesh: m.mesh_key,
        };
        let mut cache = ServiceCache { svc };
        let mut emit = |i: usize, v: FVector| write_vert(out, stride, i, v);
        with_trig(svc, |t| {
            mesh::get_frame(&mv, &ov, *c, t, &mut cache, key, &mut emit)
        });
    })
}

/// `ULodMesh::GetFrame`: writes the vertices and returns the new `LODRequest`.
///
/// # Safety
/// As [`hp2_rs_engine_mesh_get_frame`]; `lod` is a live `Hp2LodArg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_lod_mesh_get_frame(
    svc: *const c_void,
    lod: *const c_void,
    owner: *const c_void,
    coords: *const c_void,
    out: *mut c_void,
    stride: i32,
) -> i32 {
    guard("hp2_rs_engine_lod_mesh_get_frame", 0, move || unsafe {
        let (Some(svc), Some(l), Some(o), Some(c)) = (
            at::<Hp2MeshServices>(svc),
            at::<Hp2LodArg>(lod),
            at::<Hp2OwnerArg>(owner),
            at::<FCoords>(coords),
        ) else {
            return 0;
        };
        let view = LodView {
            base: mesh_view(&l.mesh),
            special_verts: l.special_verts,
            lod_request: l.lod_request,
        };
        let ov = owner_view(o);
        let key = CacheKey {
            id: o.cache_id,
            mesh: l.mesh.mesh_key,
        };
        let mut cache = ServiceCache { svc };
        let mut emit = |i: usize, v: FVector| write_vert(out, stride, i, v);
        with_trig(svc, |t| {
            lodmesh::get_frame(&view, &ov, *c, t, &mut cache, key, &mut emit)
        })
    })
}

/// `ULodMesh::SetScale`'s `MeshScaleMax` (`ULodMesh.cpp:97`). Returns `0`
/// when `scale` is null.
///
/// # Safety
/// `scale` must be null or a live `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_lod_mesh_scale_max(
    bounding_sphere_w: f32,
    scale: *const c_void,
) -> f32 {
    guard("hp2_rs_engine_lod_mesh_scale_max", 0.0, move || unsafe {
        match at::<FVector>(scale) {
            Some(s) => lodmesh::mesh_scale_max(bounding_sphere_w, *s),
            None => 0.0,
        }
    })
}
