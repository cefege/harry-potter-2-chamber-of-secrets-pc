//! C ABI for the `USkeletalMesh` / `UAnimation` port (`hp2_engine::skeletal`).
//! The C++ forwards in `UnSkeletalMesh.cpp` pass live object pointers and
//! `Hp2SkeletalServices` (the `GCache`, `GMath` and debug/check host hooks).
//! The layouts are mirrored in `Hp2SkeletalServices.h` and checked there.
//!
//! Null policy: a null services, input or output pointer makes the export
//! return without writing. C++ callers always pass live objects, so this only
//! avoids UB.

use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use hp2_abi::engine::generated::{AActor, CFSkelHeader, FBox, USkeletalMesh};
use hp2_abi::{FCoords, FVector};
use hp2_engine::skeletal::{self, AnimVec, Quat, SkelHost, box_to_raw};
use hp2_ffi_support::guard;

/// `Hp2SkeletalServices` in `Hp2SkeletalServices.h`: the `GCache`, `GMath`
/// and host hooks the skeletal core calls through. Filled by C++ once; never
/// written by Rust. Cache calls take the owning actor; C++ computes the
/// `CID_SkeletalData` cache id from it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2SkeletalServices {
    /// `GMath.SinTab(i)`.
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GMath.CosTab(i)`.
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GCache.Get(MakeCacheID(CID_SkeletalData, owner, NULL), item)`.
    pub cache_get: unsafe extern "C" fn(owner: *const c_void, item: *mut *mut c_void) -> *mut u8,
    /// `GCache.Create(MakeCacheID(...), item, appCheckedIntSize(len))`.
    pub cache_create:
        unsafe extern "C" fn(owner: *const c_void, len: usize, item: *mut *mut c_void) -> *mut u8,
    /// `GCache.Flush(MakeCacheID(...))`.
    pub cache_flush: unsafe extern "C" fn(owner: *const c_void),
    /// `Item->Unlock()`.
    pub cache_unlock: unsafe extern "C" fn(item: *mut c_void),
    /// Host debugf for a missing bone/animation link.
    pub warn_missing_link: unsafe extern "C" fn(mesh: *const c_void, bone: i32),
    /// `Owner->GetLevel()->DestroyActor(AuxAnims(i))` and `AuxAnims.Remove(i)`.
    pub remove_aux: unsafe extern "C" fn(owner: *mut c_void, index: i32),
    /// `DisplayBones` debug output; fills `DebugPivots` from `bases`.
    pub debug_pivots: unsafe extern "C" fn(mesh: *mut c_void, bases: *const c_void, num: i32),
    /// `DisplayInfluences` debug output (`DebugVerts` / `VertInfluences`).
    pub debug_influences: unsafe extern "C" fn(mesh: *mut c_void),
    /// `check()` failure; the Rust caller aborts afterwards.
    pub fail_check: unsafe extern "C" fn(expr: *const c_char),
}

/// Adapter from the C services table to the core's `SkelHost` trait.
struct ServiceHost {
    svc: Hp2SkeletalServices,
}

impl SkelHost for ServiceHost {
    fn sin_tab(&self, index: i32) -> f32 {
        // SAFETY: the table is filled by C++ with live functions.
        unsafe { (self.svc.sin_tab)(index) }
    }

    fn cos_tab(&self, index: i32) -> f32 {
        // SAFETY: as above.
        unsafe { (self.svc.cos_tab)(index) }
    }

    fn cache_get(&self, owner: *const AActor, item: &mut *mut c_void) -> *mut u8 {
        // SAFETY: as above; `owner` is a live actor or null.
        unsafe { (self.svc.cache_get)(owner.cast(), item) }
    }

    fn cache_create(&self, owner: *const AActor, bytes: usize, item: &mut *mut c_void) -> *mut u8 {
        // SAFETY: as above.
        unsafe { (self.svc.cache_create)(owner.cast(), bytes, item) }
    }

    fn cache_flush(&self, owner: *const AActor) {
        // SAFETY: as above.
        unsafe { (self.svc.cache_flush)(owner.cast()) }
    }

    fn cache_unlock(&self, item: *mut c_void) {
        // SAFETY: as above; the C++ side treats null as a no-op.
        unsafe { (self.svc.cache_unlock)(item) }
    }

    fn warn_missing_link(&self, mesh: *const USkeletalMesh, bone: usize) {
        let bone = i32::try_from(bone).unwrap_or(i32::MAX);
        // SAFETY: as above.
        unsafe { (self.svc.warn_missing_link)(mesh.cast(), bone) }
    }

    fn remove_aux(&self, owner: *mut AActor, index: usize) {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        // SAFETY: as above.
        unsafe { (self.svc.remove_aux)(owner.cast(), index) }
    }

    fn debug_pivots(&self, mesh: *mut USkeletalMesh, bases: &[FCoords]) {
        let num = i32::try_from(bases.len()).unwrap_or(i32::MAX);
        // SAFETY: as above; `bases` is a live slice of `FCoords`.
        unsafe { (self.svc.debug_pivots)(mesh.cast(), bases.as_ptr().cast(), num) }
    }

    fn debug_influences(&self, mesh: *mut USkeletalMesh) {
        // SAFETY: as above.
        unsafe { (self.svc.debug_influences)(mesh.cast()) }
    }

    fn fail_check(&self, expr: &CStr) {
        // SAFETY: as above; `expr` is NUL-terminated.
        unsafe { (self.svc.fail_check)(expr.as_ptr()) }
    }
}

/// Copies the services table; null yields `None`.
///
/// # Safety
/// A non-null `p` must point to a live `Hp2SkeletalServices`.
unsafe fn services(p: *const c_void) -> Option<ServiceHost> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a live table.
    Some(ServiceHost {
        svc: unsafe { ptr::read(p.cast::<Hp2SkeletalServices>()) },
    })
}

/// Input object pointer; null yields `None`.
fn obj<T>(p: *const c_void) -> Option<*mut T> {
    (!p.is_null()).then(|| p.cast_mut().cast::<T>())
}

/// Output pointer; null yields `None`.
fn out<T>(p: *mut c_void) -> Option<*mut T> {
    (!p.is_null()).then(|| p.cast::<T>())
}

/// Reads an `FQuat` (X, Y, Z, W floats).
///
/// # Safety
/// `p` must point to 16 readable bytes.
unsafe fn read_quat(p: *const c_void) -> Quat {
    // SAFETY: the caller guarantees 16 readable bytes.
    let a = unsafe { ptr::read_unaligned(p.cast::<[f32; 4]>()) };
    Quat {
        x: a[0],
        y: a[1],
        z: a[2],
        w: a[3],
    }
}

/// Writes an `FQuat` (X, Y, Z, W floats).
///
/// # Safety
/// `p` must point to 16 writable bytes.
unsafe fn write_quat(p: *mut c_void, q: Quat) {
    // SAFETY: the caller guarantees 16 writable bytes.
    unsafe { ptr::write_unaligned(p.cast::<[f32; 4]>(), [q.x, q.y, q.z, q.w]) }
}

/// `USkeletalMesh::ApplyAnim(AnimOwner, ParentHdr, bRootOnly)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_apply_anim(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    parent: *mut c_void,
    root_only: i32,
) {
    guard("hp2_rs_engine_skeletal_apply_anim", (), move || unsafe {
        let (Some(host), Some(this), Some(owner)) = (
            services(svc),
            obj::<USkeletalMesh>(this),
            obj::<AActor>(owner),
        ) else {
            return;
        };
        skeletal::apply_anim(
            &host,
            this,
            owner,
            parent.cast::<CFSkelHeader>(),
            root_only != 0,
        );
    })
}

/// `USkeletalMesh::GetFrame(ResultVerts, Size, Coords, Owner, LODRequest)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_frame(
    svc: *const c_void,
    this: *const c_void,
    result: *mut c_void,
    size: i32,
    coords: *const c_void,
    owner: *const c_void,
    lod: *mut c_void,
) {
    guard("hp2_rs_engine_skeletal_get_frame", (), move || unsafe {
        let (Some(host), Some(this), Some(result), Some(coords), Some(owner), Some(lod)) = (
            services(svc),
            obj::<USkeletalMesh>(this),
            out::<u8>(result),
            obj::<FCoords>(coords),
            obj::<AActor>(owner),
            out::<i32>(lod),
        ) else {
            return;
        };
        skeletal::get_frame(
            &host,
            this,
            result,
            size,
            ptr::read(coords),
            owner,
            &mut *lod,
        );
    })
}

/// `USkeletalMesh::GetRootMovement(Owner)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_root_movement(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_get_root_movement",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FVector>(result),
            ) else {
                return;
            };
            ptr::write(result, skeletal::get_root_movement(&host, this, owner));
        },
    )
}

/// `USkeletalMesh::AdjustRootMovement(Owner, Adjust)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_adjust_root_movement(
    svc: *const c_void,
    owner: *const c_void,
    adjust: *const c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_adjust_root_movement",
        (),
        move || unsafe {
            let (Some(host), Some(owner), Some(adjust)) =
                (services(svc), obj::<AActor>(owner), obj::<FVector>(adjust))
            else {
                return;
            };
            skeletal::adjust_root_movement(&host, owner, ptr::read(adjust));
        },
    )
}

/// `USkeletalMesh::AnimCycleMovement(Owner)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_anim_cycle_movement(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_anim_cycle_movement",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FVector>(result),
            ) else {
                return;
            };
            ptr::write(result, skeletal::anim_cycle_movement(&host, this, owner));
        },
    )
}

/// `USkeletalMesh::GetBoneCoords(Owner, Bone)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_bone_coords(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    bone: i32,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_get_bone_coords",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FCoords>(result),
            ) else {
                return;
            };
            ptr::write(result, skeletal::get_bone_coords(&host, this, owner, bone));
        },
    )
}

/// `USkeletalMesh::GetMeshCoords(Owner)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_mesh_coords(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_get_mesh_coords",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FCoords>(result),
            ) else {
                return;
            };
            ptr::write(result, skeletal::get_mesh_coords(&host, owner, this));
        },
    )
}

/// `USkeletalMesh::MeshAdjust(Owner)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_mesh_adjust(
    this: *const c_void,
    owner: *const c_void,
    result: *mut c_void,
) {
    guard("hp2_rs_engine_skeletal_mesh_adjust", (), move || unsafe {
        let (Some(this), Some(owner), Some(result)) = (
            obj::<USkeletalMesh>(this),
            obj::<AActor>(owner),
            out::<FVector>(result),
        ) else {
            return;
        };
        ptr::write(result, skeletal::mesh_adjust(owner, this));
    })
}

/// `USkeletalMesh::SetScale(NewScale)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_set_scale(
    this: *const c_void,
    scale: *const c_void,
) {
    guard("hp2_rs_engine_skeletal_set_scale", (), move || unsafe {
        let (Some(this), Some(scale)) = (obj::<USkeletalMesh>(this), obj::<FVector>(scale)) else {
            return;
        };
        skeletal::set_scale(this, ptr::read(scale));
    })
}

/// `USkeletalMesh::FlipFaces()`, faces only; the debugf stays in C++.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_flip_faces(this: *const c_void) {
    guard("hp2_rs_engine_skeletal_flip_faces", (), move || unsafe {
        let Some(this) = obj::<USkeletalMesh>(this) else {
            return;
        };
        skeletal::flip_faces(this);
    })
}

/// `USkeletalMesh::GetRenderBoundingBox(Owner, Exact)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_render_bounding_box(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_get_render_bounding_box",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FCoords>(result),
            ) else {
                return;
            };
            ptr::write(
                result,
                skeletal::get_render_bounding_box(&host, this, owner),
            );
        },
    )
}

/// `USkeletalMesh::GetCollisionBoundingBox(Owner, bWorld)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_get_collision_bounding_box(
    svc: *const c_void,
    this: *const c_void,
    owner: *const c_void,
    world: i32,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_get_collision_bounding_box",
        (),
        move || unsafe {
            let (Some(host), Some(this), Some(owner), Some(result)) = (
                services(svc),
                obj::<USkeletalMesh>(this),
                obj::<AActor>(owner),
                out::<FBox>(result),
            ) else {
                return;
            };
            let bb = skeletal::get_collision_bounding_box(&host, this, owner, world != 0);
            ptr::write(result, box_to_raw(bb));
        },
    )
}

/// `USkeletalMesh::BoneIndex(FName)`; `name` points to 4 name bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_bone_index(
    this: *const c_void,
    name: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_skeletal_bone_index", -1, move || unsafe {
        let (Some(this), false) = (obj::<USkeletalMesh>(this), name.is_null()) else {
            return -1;
        };
        // SAFETY: C++ passes a live 4-byte FName.
        let n = ptr::read_unaligned(name.cast::<[u8; 4]>());
        skeletal::bone_index(this, n)
    })
}

/// `USkeletalMesh::BoneName(int)`; writes 4 name bytes to `result`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_bone_name(
    this: *const c_void,
    index: i32,
    result: *mut c_void,
) {
    guard("hp2_rs_engine_skeletal_bone_name", (), move || unsafe {
        let (Some(this), Some(result)) = (obj::<USkeletalMesh>(this), out::<[u8; 4]>(result))
        else {
            return;
        };
        ptr::write_unaligned(result, skeletal::bone_name(this, index));
    })
}

/// `FAnimVec::Init(const FVector& V, float InvScale)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_anim_vec_init(
    v: *const c_void,
    inv_scale: f32,
    result: *mut c_void,
) {
    guard("hp2_rs_engine_skeletal_anim_vec_init", (), move || unsafe {
        let (Some(v), Some(result)) = (obj::<FVector>(v), out::<AnimVec>(result)) else {
            return;
        };
        ptr::write_unaligned(result, AnimVec::from_vector(ptr::read(v), inv_scale));
    })
}

/// `FAnimVec::FAnimVec(const FQuat& Q)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_anim_vec_from_quat(
    q: *const c_void,
    result: *mut c_void,
) {
    guard(
        "hp2_rs_engine_skeletal_anim_vec_from_quat",
        (),
        move || unsafe {
            if q.is_null() || result.is_null() {
                return;
            }
            let quat = read_quat(q);
            ptr::write_unaligned(result.cast::<AnimVec>(), AnimVec::from_quat(quat));
        },
    )
}

/// `FAnimVec::Quat() const`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_skeletal_anim_vec_quat(
    v: *const c_void,
    result: *mut c_void,
) {
    guard("hp2_rs_engine_skeletal_anim_vec_quat", (), move || unsafe {
        let (Some(v), false) = (obj::<AnimVec>(v), result.is_null()) else {
            return;
        };
        let q = ptr::read_unaligned(v).quat();
        write_quat(result, q);
    })
}
