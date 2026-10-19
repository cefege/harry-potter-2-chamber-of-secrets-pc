//! Skeletal mesh evaluation: animation blending, frame skinning, bounds and
//! root motion (port of `UnSkeletalMesh.cpp`).
//!
//! Engine state stays in the generated mirrors (`hp2_abi::engine::generated`)
//! and is reached through live pointers. Services owned by the C++ side (GCache,
//! trig tables, debug output, actor removal, `check` failures) arrive through
//! [`SkelHost`].
//!
//! Float evaluation follows the C++ source. Under clang `-ffp-contract=on` a
//! fused multiply-add forms only inside one expression: `a*b + c*d` fuses the
//! left product and `(sum) + c*d` the right one. Statements and operator-function
//! boundaries do not fuse. Fused sites are marked with `mul_add`.

use std::ffi::{CStr, c_void};
use std::ptr::null_mut;

use hp2_abi::engine::generated::{
    AActor, CFSkelHeader, FAnalogTrack, FBox as FBoxRaw, FMeshAnimSeq, FMeshBone, FMeshFace,
    FNamedBone, FPlace, MotionChunk, TArrayRaw, TRefArrayRaw, UAnimation, USkeletalMesh,
    VBoneInfIndex, VBoneInfluence,
};
use hp2_abi::{FCoords, FVector};

use crate::coords::{self, FScale, SHEER_NONE, Trig};
use crate::math::{app_round, app_sqrt};
use crate::prim::{self, Aabb, INDEX_NONE};

/// `PHYS_None`.
const PHYS_NONE: u8 = 0;
/// `PHYS_Trailer`.
const PHYS_TRAILER: u8 = 11;
/// `CT_Shape`.
const CT_SHAPE: u8 = 3;
/// `DELTA` in `FQuat::Normalize`.
const DELTA: f32 = 0.00001;
/// `BASE_SCALE` in `FAnimVec`.
const BASE_SCALE: f32 = 32767.0;
/// `static float Scale = BASE_SCALE / 1.57079633;` (double division, then float).
#[allow(clippy::approx_constant)] // Original literal; FRAC_PI_2 would change the bits.
const ANIM_ENCODE_SCALE: f32 = (32767.0_f64 / 1.57079633_f64) as f32;
/// `static float Scale = 1.57079633 / BASE_SCALE;`
#[allow(clippy::approx_constant)] // Original literal; FRAC_PI_2 would change the bits.
const ANIM_DECODE_SCALE: f32 = (1.57079633_f64 / 32767.0_f64) as f32;
/// `sizeof(CFSkelHeader)`.
const CFSKEL_HEADER_SIZE: usize = 168;
/// Cache bytes per bone: one `INT` link plus one `FPlace`.
const CACHE_BYTES_PER_BONE: usize = 4 + 28;
/// `NAME_None` as stored in `[u8; 4]` name fields.
const NAME_NONE: [u8; 4] = [0; 4];
const ZERO_V: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

/// Engine services the skeletal code needs and cannot express as mirrors.
pub trait SkelHost {
    /// `GMath.SinTab(index)`.
    fn sin_tab(&self, index: i32) -> f32;
    /// `GMath.CosTab(index)`.
    fn cos_tab(&self, index: i32) -> f32;
    /// `GCache.Get(MakeCacheID(CID_SkeletalData, owner, NULL), item)`.
    /// Returns the block (null on miss) and sets `item` only on a hit.
    fn cache_get(&self, owner: *const AActor, item: &mut *mut c_void) -> *mut u8;
    /// `GCache.Create(CacheID, item, appCheckedIntSize(bytes))`; sets `item`.
    fn cache_create(&self, owner: *const AActor, bytes: usize, item: &mut *mut c_void) -> *mut u8;
    /// `GCache.Flush(CacheID)`.
    fn cache_flush(&self, owner: *const AActor);
    /// `Item->Unlock()`; null-safe.
    fn cache_unlock(&self, item: *mut c_void);
    /// `debugf("Warning: missing bone/animation link for %s  index %i", ...)`.
    fn warn_missing_link(&self, mesh: *const USkeletalMesh, bone: usize);
    /// `Owner->GetLevel()->DestroyActor(AuxAnims(index))` followed by `AuxAnims.Remove(index)`.
    fn remove_aux(&self, owner: *mut AActor, index: usize);
    /// `DisplayBones` debug output; the host fills `DebugPivots` from `bases`.
    fn debug_pivots(&self, mesh: *mut USkeletalMesh, bases: &[FCoords]);
    /// `DisplayInfluences` debug output (`DebugVerts` / `VertInfluences`).
    fn debug_influences(&self, mesh: *mut USkeletalMesh);
    /// `check()` failure (`appFailAssert`). The caller aborts afterwards.
    fn fail_check(&self, expr: &CStr);
}

/// Aborts after reporting a failed `check()`.
fn check_failed<H: SkelHost>(host: &H, expr: &CStr) -> ! {
    host.fail_check(expr);
    std::process::abort()
}

/// `TCacheItem<CFSkelHeader>`: holds the GCache lock for the scope and unlocks on drop.
pub struct SkelCache<'h, H: SkelHost> {
    host: &'h H,
    item: *mut c_void,
    hdr: *mut CFSkelHeader,
}

impl<'h, H: SkelHost> SkelCache<'h, H> {
    /// `FSkelCache(Owner)`. No Rust-side cache: this reads the same GCache entry the
    /// original reads (in-game `SkelCacheGet` = `MakeCacheID(CID_SkeletalData, Owner, NULL)`,
    /// keyed by the owner's object index). Stale pool memory the original would see
    /// (e.g. `bGotRoot`/`PrevRootFrame` on a reused entry) is therefore seen identically
    /// in-game, so replay parity implies in-game parity. The replay harness zeroes the
    /// pool per case (`flush_case`) only so fixtures do not depend on earlier cases.
    pub fn new(host: &'h H, owner: *const AActor) -> Self {
        let mut item: *mut c_void = null_mut();
        let mem = host.cache_get(owner, &mut item);
        SkelCache {
            host,
            item,
            hdr: mem.cast::<CFSkelHeader>(),
        }
    }

    /// Cache header, or null on a miss.
    pub fn hdr(&self) -> *mut CFSkelHeader {
        self.hdr
    }
}

impl<H: SkelHost> Drop for SkelCache<'_, H> {
    fn drop(&mut self) {
        if !self.item.is_null() {
            self.host.cache_unlock(self.item);
        }
    }
}

/// Runs `f` with `Trig` views over the host's sine and cosine tables.
fn with_trig<H: SkelHost, R>(host: &H, f: impl FnOnce(&Trig) -> R) -> R {
    let sin = |i: i32| host.sin_tab(i);
    let cos = |i: i32| host.cos_tab(i);
    f(&Trig {
        sin: &sin,
        cos: &cos,
    })
}

/// Element view of an engine `TArray` (empty when unallocated).
///
/// # Safety
/// `a.data` must hold `a.num` live elements of `T` when `a.num > 0`.
unsafe fn tarr<'a, T>(a: &TArrayRaw) -> &'a [T] {
    match usize::try_from(a.num) {
        Ok(n) if n > 0 && !a.data.is_null() => unsafe {
            std::slice::from_raw_parts(a.data.cast::<T>(), n)
        },
        _ => &[],
    }
}

/// Element view of an engine `TRefArray` (`T* Data; INT ArrayNum;`, empty when unallocated).
///
/// # Safety
/// `a.data` must hold `a.num` live elements of `T` when `a.num > 0`.
unsafe fn tref<'a, T>(a: &TRefArrayRaw) -> &'a [T] {
    match usize::try_from(a.num) {
        Ok(n) if n > 0 && !a.data.is_null() => unsafe {
            std::slice::from_raw_parts(a.data.cast::<T>(), n)
        },
        _ => &[],
    }
}

/// Mutable element view of an engine `TArray` (empty when unallocated).
///
/// # Safety
/// `a.data` must hold `a.num` live elements of `T` when `a.num > 0`, and no other
/// reference to those elements may be live.
unsafe fn tarr_mut<'a, T>(a: &TArrayRaw) -> &'a mut [T] {
    match usize::try_from(a.num) {
        Ok(n) if n > 0 && !a.data.is_null() => unsafe {
            std::slice::from_raw_parts_mut(a.data.cast::<T>(), n)
        },
        _ => &mut [],
    }
}

/// `USkeletalMesh::RefSkeleton`.
///
/// # Safety
/// `m` must point to a live `USkeletalMesh`.
unsafe fn ref_skel<'a>(m: *const USkeletalMesh) -> &'a [FMeshBone] {
    unsafe { tarr(&(*m).ref_skeleton) }
}

/// `CachedLinks` of a cache block for `n` bones.
///
/// # Safety
/// `h` must be a live cache header created for `n` bones.
unsafe fn cached_links<'a>(h: *mut CFSkelHeader, n: usize) -> &'a mut [i32] {
    unsafe { std::slice::from_raw_parts_mut((*h).cached_links.cast::<i32>(), n) }
}

/// `CachedPlaces[b]`.
///
/// # Safety
/// `h` must be a live cache header whose block holds more than `b` places.
unsafe fn cached_place(h: *mut CFSkelHeader, b: usize) -> *mut FPlace {
    unsafe { (*h).cached_places.cast::<FPlace>().add(b) }
}

/// Unit quaternion as stored by engine `FQuat` (`X, Y, Z, W`).
#[derive(Clone, Copy, Debug)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

const QUAT_IDENTITY: Quat = Quat {
    x: 0.0,
    y: 0.0,
    z: 0.0,
    w: 1.0,
};

impl Quat {
    /// `FQuat(X, Y, Z)`: W is rebuilt from the unit-length constraint.
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Quat {
        // Max(0, 1 - X*X - Y*Y - Z*Z), fused left to right.
        let sq = (-z).mul_add(z, (-y).mul_add(y, (-x).mul_add(x, 1.0)));
        Quat {
            x,
            y,
            z,
            w: app_sqrt(prim::max_f(0.0, sq) as f64) as f32,
        }
    }

    /// `FQuat::Vector()`.
    fn vector(self) -> FVector {
        if self.w < 0.0 {
            prim::v3(-self.x, -self.y, -self.z)
        } else {
            prim::v3(self.x, self.y, self.z)
        }
    }

    /// `FQuat::operator|` (dot product), fused left to right.
    fn dot(self, q: Quat) -> f32 {
        self.w
            .mul_add(q.w, self.z.mul_add(q.z, self.x.mul_add(q.x, self.y * q.y)))
    }

    /// `FQuat::Normalize`; returns false when the quaternion collapsed.
    fn normalize(&mut self) -> bool {
        let sq = self.w.mul_add(
            self.w,
            self.z
                .mul_add(self.z, self.x.mul_add(self.x, self.y * self.y)),
        );
        if (sq - 1.0).abs() > DELTA {
            if sq >= DELTA {
                let scale = sq.mul_add(-0.5, 1.5);
                self.x *= scale;
                self.y *= scale;
                self.z *= scale;
                self.w *= scale;
            } else {
                self.x = 0.0;
                self.y = 0.0;
                self.z = 0.1;
                self.w = 0.0;
                return false;
            }
        }
        true
    }
}

/// `FQuat operator<<` (UnMath.h 2913–2922).
fn quat_shl(a: Quat, b: Quat) -> Quat {
    Quat {
        x: (-a.y).mul_add(b.z, (-a.w).mul_add(b.x, a.z.mul_add(b.y, -(a.x * b.w)))),
        y: (-a.z).mul_add(b.x, (-a.w).mul_add(b.y, a.x.mul_add(b.z, -(a.y * b.w)))),
        z: (-a.x).mul_add(b.y, (-a.w).mul_add(b.z, a.y.mul_add(b.x, -(a.z * b.w)))),
        w: (-a.w).mul_add(b.w, a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))),
    }
}

/// `SlerpQuat` (UnMath.cpp 275–302).
fn slerp_quat(q1: Quat, q2: Quat, slerp: f32) -> Quat {
    let cosom = q1.dot(q2);
    let abs_cosom = cosom.abs();
    if abs_cosom < 1.0 {
        let omega = acos_d(abs_cosom as f64) as f32;
        let sininv = (1.0 / sin_d(omega as f64)) as f32;
        let scale0 = (sin_d(((1.0 - slerp) * omega) as f64) * sininv as f64) as f32;
        let mut scale1 = (sin_d((slerp * omega) as f64) * sininv as f64) as f32;
        if cosom < 0.0 {
            scale1 = -scale1;
        }
        let mut r = Quat {
            x: scale0.mul_add(q1.x, scale1 * q2.x),
            y: scale0.mul_add(q1.y, scale1 * q2.y),
            z: scale0.mul_add(q1.z, scale1 * q2.z),
            w: scale0.mul_add(q1.w, scale1 * q2.w),
        };
        r.normalize();
        r
    } else {
        q1
    }
}

/// Engine `FPlace`: a rotation and a translation.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub q: Quat,
    pub pos: FVector,
}

const PLACE_IDENTITY: Place = Place {
    q: QUAT_IDENTITY,
    pos: ZERO_V,
};

fn place_of(p: &FPlace) -> Place {
    Place {
        q: Quat {
            x: p.quat[0],
            y: p.quat[1],
            z: p.quat[2],
            w: p.quat[3],
        },
        pos: p.pos,
    }
}

fn raw_place(p: Place) -> FPlace {
    FPlace {
        quat: [p.q.x, p.q.y, p.q.z, p.q.w],
        pos: p.pos,
    }
}

/// `FCoords(const FPlace&)` (UnMath.cpp 305–326).
fn coords_from_place(p: Place) -> FCoords {
    let q = p.q;
    let x2 = q.x + q.x;
    let y2 = q.y + q.y;
    let z2 = q.z + q.z;
    let xx = q.x * x2;
    let xy = q.x * y2;
    let xz = q.x * z2;
    let yy = q.y * y2;
    let yz = q.y * z2;
    let zz = q.z * z2;
    let wx = q.w * x2;
    let wy = q.w * y2;
    let wz = q.w * z2;
    FCoords {
        origin: p.pos,
        x_axis: prim::v3(1.0 - (yy + zz), xy - wz, xz + wy),
        y_axis: prim::v3(xy + wz, 1.0 - (xx + zz), yz - wx),
        z_axis: prim::v3(xz - wy, yz + wx, 1.0 - (xx + yy)),
    }
}

/// `FPlace operator<<` (UnMath.h 2929–2932).
fn place_shl(a: Place, b: Place) -> Place {
    Place {
        q: quat_shl(a.q, b.q),
        pos: coords::vec_times_coords(a.pos, &coords_from_place(b)),
    }
}

/// `FTriple` (UnMath.h 2196–2202).
fn triple(x: FVector, y: FVector, z: FVector) -> f32 {
    let inner1 = y.y.mul_add(z.z, -(y.z * z.y));
    let inner2 = y.z.mul_add(z.x, -(y.x * z.z));
    let inner3 = y.x.mul_add(z.y, -(y.y * z.x));
    x.z.mul_add(inner3, x.x.mul_add(inner1, x.y * inner2))
}

/// `FCoords::Inverse` (UnMath.cpp 123–143).
fn inverse(c: &FCoords) -> FCoords {
    let (x, y, z) = (c.x_axis, c.y_axis, c.z_axis);
    let rdet = 1.0 / triple(x, y, z);
    let c0 = prim::v3(
        y.y.mul_add(z.z, -(y.z * z.y)),
        z.y.mul_add(x.z, -(z.z * x.y)),
        x.y.mul_add(y.z, -(x.z * y.y)),
    );
    let c1 = prim::v3(
        y.z.mul_add(z.x, -(z.z * y.x)),
        z.z.mul_add(x.x, -(x.z * z.x)),
        x.z.mul_add(y.x, -(x.x * y.z)),
    );
    let c2 = prim::v3(
        y.x.mul_add(z.y, -(y.y * z.x)),
        z.x.mul_add(x.y, -(z.y * x.x)),
        x.x.mul_add(y.y, -(x.y * y.x)),
    );
    let mut inv = FCoords {
        origin: ZERO_V,
        x_axis: prim::scale(c0, rdet),
        y_axis: prim::scale(c1, rdet),
        z_axis: prim::scale(c2, rdet),
    };
    inv.origin = coords::vec_times_coords(prim::neg(c.origin), &inv);
    inv
}

/// `FVector operator^` (UnMath.h 677–685), fused left to right.
fn cross_fused(a: FVector, b: FVector) -> FVector {
    prim::v3(
        a.y.mul_add(b.z, -(a.z * b.y)),
        a.z.mul_add(b.x, -(a.x * b.z)),
        a.x.mul_add(b.y, -(a.y * b.x)),
    )
}

/// `FCoords << FScale` (UnMath.h 2896–2900).
fn coords_times_scale(c: FCoords, s: &FScale) -> FCoords {
    FCoords {
        origin: coords::mul_comp(c.origin, s.scale),
        x_axis: coords::mul_comp(c.x_axis, s.scale),
        y_axis: coords::mul_comp(c.y_axis, s.scale),
        z_axis: coords::mul_comp(c.z_axis, s.scale),
    }
}

/// `Interp(FVector)` (UnMath.h 908–917), fused left to right.
fn interp_vec(a: FVector, b: FVector, param: f32) -> FVector {
    let marap = 1.0 - param;
    prim::v3(
        a.x.mul_add(marap, b.x * param),
        a.y.mul_add(marap, b.y * param),
        a.z.mul_add(marap, b.z * param),
    )
}

/// `Interp(const FPlace&, FPlace&, float)` (UnSkeletalMesh.cpp 350–356).
fn interp_place(p1: Place, p2: Place, alpha: f32) -> Place {
    Place {
        q: slerp_quat(p1.q, p2.q, alpha),
        pos: prim::add(prim::scale(p1.pos, 1.0 - alpha), prim::scale(p2.pos, alpha)),
    }
}

/// Engine `FBox(0)`: zero extents, invalid.
fn zero_box() -> Aabb {
    Aabb {
        min: ZERO_V,
        max: ZERO_V,
        valid: false,
    }
}

/// Mirror `FBox` to `Aabb`.
pub fn box_from_raw(b: &FBoxRaw) -> Aabb {
    Aabb {
        min: b.min,
        max: b.max,
        valid: b.is_valid != 0,
    }
}

/// `Aabb` to mirror `FBox`.
pub fn box_to_raw(b: Aabb) -> FBoxRaw {
    // SAFETY: FBox is plain old data; its padding is zero.
    let mut r: FBoxRaw = unsafe { std::mem::zeroed() };
    r.min = b.min;
    r.max = b.max;
    r.is_valid = u8::from(b.valid);
    r
}

/// `FBox(const FCoords&)` (UnMath.cpp 212–228).
fn box_from_coords(c: &FCoords) -> Aabb {
    let mut b = Aabb {
        min: c.origin,
        max: c.origin,
        valid: true,
    };
    for axis in [c.x_axis, c.y_axis, c.z_axis] {
        if axis.x >= 0.0 {
            b.max.x += axis.x;
        } else {
            b.min.x += axis.x;
        }
        if axis.y >= 0.0 {
            b.max.y += axis.y;
        } else {
            b.min.y += axis.y;
        }
        if axis.z >= 0.0 {
            b.max.z += axis.z;
        } else {
            b.min.z += axis.z;
        }
    }
    b
}

/// `FBox::operator+=(const FBox&)` (UnMath.h 1413–1454).
fn box_add_box(b: &mut Aabb, o: Aabb) {
    if !o.valid {
        return;
    }
    if b.valid {
        b.min = prim::v3(
            prim::min_f(b.min.x, o.min.x),
            prim::min_f(b.min.y, o.min.y),
            prim::min_f(b.min.z, o.min.z),
        );
        b.max = prim::v3(
            prim::max_f(b.max.x, o.max.x),
            prim::max_f(b.max.y, o.max.y),
            prim::max_f(b.max.z, o.max.z),
        );
    } else {
        *b = o;
    }
}

/// Engine `FAnimVec`: ANIM48 compressed 3-vector (`i16` components).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimVec {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

unsafe extern "C" {
    #[link_name = "asinf"]
    fn libm_asinf(x: f32) -> f32;
    #[link_name = "sinf"]
    fn libm_sinf(x: f32) -> f32;
    #[link_name = "acos"]
    fn libm_acos(x: f64) -> f64;
    #[link_name = "sin"]
    fn libm_sin(x: f64) -> f64;
}

/// `asinf` as resolved by the original TU (UnSkeletalMesh.o calls `_asinf`).
/// Calls the real libm symbol through a `black_box`ed pointer so LLVM cannot
/// substitute the `llvm.asin` intrinsic. No mismatch observed on a 200k random
/// sample; routed this way for the same reason as `sin_anim`.
#[inline(never)]
fn asin_anim(x: f32) -> f32 {
    let f: unsafe extern "C" fn(f32) -> f32 =
        std::hint::black_box(libm_asinf as unsafe extern "C" fn(f32) -> f32);
    unsafe { f(x) }
}

/// `sinf` as resolved by the original TU (UnSkeletalMesh.o calls `_sinf`).
/// Same routing as `asin_anim`. `f32::sin()` gives a 1-ulp diff at -O on the
/// fixture input `-14662 * (1.57079633 / 32767)` (math case 4, lane 0).
#[inline(never)]
fn sin_anim(x: f32) -> f32 {
    let f: unsafe extern "C" fn(f32) -> f32 =
        std::hint::black_box(libm_sinf as unsafe extern "C" fn(f32) -> f32);
    unsafe { f(x) }
}

/// `sin` (double) as resolved by the original `appSin` (UnFile.h: `DOUBLE appSin(DOUBLE)`).
/// Same `black_box`ed libm routing as `sin_anim`; `f64::sin` would lower to `llvm.sin`.
#[inline(never)]
fn sin_d(x: f64) -> f64 {
    let f: unsafe extern "C" fn(f64) -> f64 =
        std::hint::black_box(libm_sin as unsafe extern "C" fn(f64) -> f64);
    unsafe { f(x) }
}

/// `acos` (double) as resolved by the original `appAcos` (UnFile.h: `DOUBLE appAcos(DOUBLE)`).
#[inline(never)]
fn acos_d(x: f64) -> f64 {
    let f: unsafe extern "C" fn(f64) -> f64 =
        std::hint::black_box(libm_acos as unsafe extern "C" fn(f64) -> f64);
    unsafe { f(x) }
}

impl AnimVec {
    /// `FAnimVec::Init(const FVector&, float InvScale)`.
    pub fn from_vector(v: FVector, inv_scale: f32) -> AnimVec {
        let inv = inv_scale * BASE_SCALE;
        AnimVec {
            x: app_round(v.x * inv) as i16,
            y: app_round(v.y * inv) as i16,
            z: app_round(v.z * inv) as i16,
        }
    }

    /// `FAnimVec(const FQuat&)` (ANIM48 path).
    pub fn from_quat(q: Quat) -> AnimVec {
        let v = q.vector();
        AnimVec {
            x: app_round(asin_anim(prim::min_f(v.x, 1.0)) * ANIM_ENCODE_SCALE) as i16,
            y: app_round(asin_anim(prim::min_f(v.y, 1.0)) * ANIM_ENCODE_SCALE) as i16,
            z: app_round(asin_anim(prim::min_f(v.z, 1.0)) * ANIM_ENCODE_SCALE) as i16,
        }
    }

    /// `FAnimVec::Quat()` (ANIM48 path).
    pub fn quat(self) -> Quat {
        Quat::from_xyz(
            sin_anim(f32::from(self.x) * ANIM_DECODE_SCALE),
            sin_anim(f32::from(self.y) * ANIM_DECODE_SCALE),
            sin_anim(f32::from(self.z) * ANIM_DECODE_SCALE),
        )
    }

    /// `FAnimVec::Vector(float Scale)`: `Scale /= BASE_SCALE`, then `X * Scale` per component.
    pub fn vector(self, scale: f32) -> FVector {
        let s = scale / BASE_SCALE;
        prim::v3(
            f32::from(self.x) * s,
            f32::from(self.y) * s,
            f32::from(self.z) * s,
        )
    }
}

/// `FAnalogTrack::GetKeyPos` helpers: key `i` of a track, zero on a miss.
///
/// # Safety
/// The track's arrays must hold their declared element counts.
unsafe fn key_pos(track: &FAnalogTrack, i: i32) -> FVector {
    unsafe {
        let keys = tref::<AnimVec>(&track.key_pos);
        usize::try_from(i)
            .ok()
            .and_then(|i| keys.get(i))
            .map_or(ZERO_V, |k| k.vector(track.pos_scale))
    }
}

/// `KeyQuat(i).Quat()`, identity on a miss.
///
/// # Safety
/// The track's arrays must hold their declared element counts.
unsafe fn key_quat(track: &FAnalogTrack, i: i32) -> Quat {
    unsafe {
        let keys = tref::<AnimVec>(&track.key_quat);
        usize::try_from(i)
            .ok()
            .and_then(|i| keys.get(i))
            .map_or(QUAT_IDENTITY, |k| k.quat())
    }
}

/// `KeyPos.Num()`.
///
/// # Safety
/// The track's arrays must hold their declared element counts.
unsafe fn key_pos_len(track: &FAnalogTrack) -> usize {
    unsafe { tref::<AnimVec>(&track.key_pos).len() }
}

/// One step up the animation's bone chain: `RefBones(t).ParentIndex`, or `None` at the root.
fn parent_step(named: &[FNamedBone], t: i32) -> Option<i32> {
    let p = named.get(usize::try_from(t).ok()?)?.parent_index;
    (p != t && p >= 0).then_some(p)
}

/// `GetPlace(ThisMove, t, FrameTime)`: samples track `t` of `mv` at `frame_time`.
///
/// # Safety
/// `mv` must be a live motion chunk of a live animation.
unsafe fn get_place(mv: &MotionChunk, t: usize, frame_time: f32) -> Place {
    unsafe {
        let Some(track) = tarr::<FAnalogTrack>(&mv.anim_tracks).get(t) else {
            return PLACE_IDENTITY;
        };
        let deltas = tref::<u8>(&track.key_delta);
        let num = deltas.len() as i32;
        if num == 1 {
            return Place {
                q: key_quat(track, 0),
                pos: key_pos(track, 0),
            };
        }
        let mut key_a_time = 0.0_f32;
        let mut key_b_time = 0.0_f32;
        let mut key_b: i32 = 1;
        while key_b < num {
            key_b_time = (f32::from(deltas[key_b as usize])).mul_add(track.time_scale, key_b_time);
            if key_b_time > frame_time {
                break;
            }
            key_a_time = key_b_time;
            key_b += 1;
        }
        let mut key_a = key_b - 1;
        if key_a_time == frame_time {
            key_b = key_a;
        } else if key_b >= num {
            key_b = 0;
            key_b_time = mv.track_time;
            key_a = num - 1;
        }
        let pos_single = key_pos_len(track) == 1;
        if key_a == key_b {
            Place {
                q: key_quat(track, key_a),
                pos: if pos_single {
                    key_pos(track, 0)
                } else {
                    key_pos(track, key_a)
                },
            }
        } else {
            let alpha = (frame_time - key_a_time) / (key_b_time - key_a_time);
            Place {
                q: slerp_quat(key_quat(track, key_a), key_quat(track, key_b), alpha),
                pos: if pos_single {
                    key_pos(track, 0)
                } else {
                    interp_vec(key_pos(track, key_a), key_pos(track, key_b), alpha)
                },
            }
        }
    }
}

/// `UAnimation::GetAnimSeq(name)`.
///
/// # Safety
/// `anim` must point to a live `UAnimation`.
unsafe fn find_anim_seq<'a>(anim: *const UAnimation, name: [u8; 4]) -> Option<&'a FMeshAnimSeq> {
    unsafe {
        tarr::<FMeshAnimSeq>(&(*anim).anim_seqs)
            .iter()
            .find(|s| s.name == name)
    }
}

/// `UAnimation::GetMovement(name)`: the motion at the index of the matching sequence.
///
/// # Safety
/// `anim` must point to a live `UAnimation`.
unsafe fn find_movement<'a>(anim: *const UAnimation, name: [u8; 4]) -> Option<&'a MotionChunk> {
    unsafe {
        let i = tarr::<FMeshAnimSeq>(&(*anim).anim_seqs)
            .iter()
            .position(|s| s.name == name)?;
        tarr::<MotionChunk>(&(*anim).moves).get(i)
    }
}

/// `USkeletalMesh::GetMeshCoords(Owner)`.
///
/// # Safety
/// `owner` and `mesh` must point to live objects.
pub unsafe fn get_mesh_coords<H: SkelHost>(
    host: &H,
    owner: *const AActor,
    mesh: *const USkeletalMesh,
) -> FCoords {
    unsafe {
        let o = &*owner;
        let m = &*mesh;
        let mut ss = FScale {
            scale: prim::scale(m.scale, o.draw_scale),
            sheer_rate: 0.0,
            sheer_axis: SHEER_NONE,
        };
        let wideness = f32::from(o.wideness) / 128.0;
        ss.scale.x *= wideness;
        ss.scale.y *= -wideness;
        let pivot = prim::add(
            prim::add(o.pre_pivot, mesh_adjust(owner, mesh)),
            prim::v3(0.0, 0.0, o.saved_pre_pivot_z),
        );
        let mut c = coords_times_scale(coords::coords_from_origin(prim::neg(m.origin)), &ss);
        with_trig(host, |t| {
            coords::div_rot(&mut c, m.rot_origin, t);
            coords::div_vec(&mut c, pivot);
            coords::div_rot(&mut c, o.rotation, t);
            coords::div_vec(&mut c, o.location);
        });
        c
    }
}

/// `USkeletalMesh::MeshAdjust(Owner)`.
///
/// # Safety
/// `owner` and `mesh` must point to live objects.
pub unsafe fn mesh_adjust(owner: *const AActor, mesh: *const USkeletalMesh) -> FVector {
    unsafe {
        let o = &*owner;
        let m = &*mesh;
        if o.b_align_bottom()
            && o.b_collide_world()
            && o.physics != PHYS_NONE
            && o.collide_type != CT_SHAPE
        {
            let inner = ((m.origin.z - m.bounding_box.min.z) * m.scale.z) * o.draw_scale;
            let z = ((-o.collision_height) as f64 - 2.5 + f64::from(inner)) as f32;
            prim::v3(0.0, 0.0, z)
        } else {
            ZERO_V
        }
    }
}

/// `USkeletalMesh::SetScale(NewScale)`.
///
/// # Safety
/// `mesh` must point to a live `USkeletalMesh`.
pub unsafe fn set_scale(mesh: *mut USkeletalMesh, new_scale: FVector) {
    unsafe {
        (*mesh).scale = new_scale;
        let extent = prim::max_f(
            new_scale.x.abs(),
            prim::max_f(new_scale.y.abs(), new_scale.z.abs()),
        );
        (*mesh).mesh_scale_max = (5.0_f32 * 1.0 / 128.0) * (*mesh).bounding_sphere.w * extent;
    }
}

/// Swaps the first two wedge indices of every face (`USkeletalMesh::FlipFaces`).
///
/// # Safety
/// `mesh` must point to a live `USkeletalMesh`.
pub unsafe fn flip_faces(mesh: *mut USkeletalMesh) {
    unsafe {
        for f in tarr_mut::<FMeshFace>(&(*mesh).faces) {
            f.i_wedge.swap(0, 1);
        }
    }
}

/// `USkeletalMesh::BoneIndex(name)`; `INDEX_NONE` when absent.
///
/// # Safety
/// `mesh` must point to a live `USkeletalMesh`.
pub unsafe fn bone_index(mesh: *const USkeletalMesh, name: [u8; 4]) -> i32 {
    unsafe {
        ref_skel(mesh)
            .iter()
            .position(|b| b.name == name)
            .map_or(INDEX_NONE, |i| i as i32)
    }
}

/// `USkeletalMesh::BoneName(index)`; `NAME_None` when out of range.
///
/// # Safety
/// `mesh` must point to a live `USkeletalMesh`.
pub unsafe fn bone_name(mesh: *const USkeletalMesh, index: i32) -> [u8; 4] {
    unsafe {
        usize::try_from(index)
            .ok()
            .and_then(|i| ref_skel(mesh).get(i))
            .map_or(NAME_NONE, |b| b.name)
    }
}

/// `USkeletalMesh::AnimBox(Owner)`: bounds of the playing animation and its aux anims.
///
/// # Safety
/// `owner` must point to a live actor.
pub unsafe fn anim_box<H: SkelHost>(host: &H, owner: *const AActor) -> Aabb {
    unsafe {
        let o = &*owner;
        let mut bound = zero_box();
        let skel_anim = o.skel_anim.cast::<UAnimation>();
        if !skel_anim.is_null() && o.anim_sequence != NAME_NONE {
            let mesh = o.mesh.cast::<USkeletalMesh>();
            let seqs = tarr::<FMeshAnimSeq>(&(*skel_anim).anim_seqs);
            if let Some(s) = seqs.iter().position(|s| s.name == o.anim_sequence) {
                let raw = if mesh.is_null() {
                    None
                } else {
                    tarr::<FBoxRaw>(&(*mesh).bounding_boxes).get(s)
                };
                bound = raw.map_or_else(zero_box, box_from_raw);
            }
        }
        let skel = SkelCache::new(host, owner);
        let hdr = skel.hdr();
        if !hdr.is_null() {
            if o.tween_alpha != 1.0 {
                box_add_box(&mut bound, box_from_raw(&(*hdr).prev_bound));
            }
            (*hdr).prev_bound = box_to_raw(bound);
        }
        let aux = o.aux_anims;
        for &chan in tarr::<*mut AActor>(&aux) {
            box_add_box(&mut bound, anim_box(host, chan.cast_const()));
        }
        bound
    }
}

/// `USkeletalMesh::ApplyAnim(AnimOwner, ParentHdr, bRootOnly) const`.
///
/// Evaluates the owner's animation into the cache block, blends tween, applies
/// root motion, and recurses into aux anims.
///
/// # Safety
/// `this` and `owner` must point to live objects; `parent_hdr` must be null or a
/// live cache header of the same mesh.
pub unsafe fn apply_anim<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    owner: *mut AActor,
    parent_hdr: *mut CFSkelHeader,
    root_only: bool,
) {
    unsafe {
        let skel = ref_skel(this);
        let n = skel.len();
        let aux = (*owner).anim_bone > 0 && (*owner).physics != PHYS_TRAILER;
        let mut item: *mut c_void = null_mut();
        let mut mem = host.cache_get(owner.cast_const(), &mut item);
        if mem.is_null() || (*mem.cast::<CFSkelHeader>()).cached_mesh != this.cast() {
            if !mem.is_null() {
                host.cache_unlock(item);
                host.cache_flush(owner.cast_const());
            }
            mem = host.cache_create(
                owner.cast_const(),
                CFSKEL_HEADER_SIZE + CACHE_BYTES_PER_BONE * n,
                &mut item,
            );
            init_cache(mem, this, n);
        }
        let frame = mem.cast::<CFSkelHeader>();
        let anim_seq = (*owner).anim_sequence;
        let af = (*owner).anim_frame;
        let tween_alpha = (*owner).tween_alpha;
        if !root_only && (*frame).cached_seq != anim_seq {
            (*frame).b_got_root = 0;
            (*frame).prev_root_frame = 0.0;
        }
        (*frame).mesh_coords = get_mesh_coords(host, owner.cast_const(), this.cast_const());

        if aux
            || (*frame).cached_seq != anim_seq
            || (*frame).cached_frame != af
            || (*frame).cached_alpha != tween_alpha
        {
            if (*owner).skel_anim.is_null() && !(*this).default_animation.is_null() {
                (*owner).skel_anim = (*this).default_animation;
            }
            let skel_anim = (*owner).skel_anim.cast::<UAnimation>();
            let seq = if skel_anim.is_null() {
                None
            } else {
                find_anim_seq(skel_anim, anim_seq)
            };
            let this_move = if skel_anim.is_null() {
                None
            } else {
                find_movement(skel_anim, anim_seq)
            };
            if seq.is_none() && aux {
                host.cache_unlock(item);
                return;
            }

            // Skeleton-to-animation link-up.
            if let Some(mv) = this_move
                .filter(|_| !skel_anim.is_null() && (*frame).cached_anim != skel_anim.cast())
            {
                let links = cached_links(frame, n);
                let tracks = tarr::<FAnalogTrack>(&mv.anim_tracks);
                let named = tarr::<FNamedBone>(&(*skel_anim).ref_bones);
                let ab = i64::from((*owner).anim_bone);
                let last = skel
                    .get(usize::from((*owner).anim_bone))
                    .map_or(ab, |b| ab + i64::from(b.num_children));
                for (p, bone) in skel.iter().enumerate() {
                    links[p] = -1;
                    if aux && ((p as i64) < ab || (p as i64) > last) {
                        continue;
                    }
                    let found = (0..tracks.len())
                        .find(|&b| named.get(b).is_some_and(|r| r.name == bone.name));
                    match found {
                        Some(b) => links[p] = b as i32,
                        None => host.warn_missing_link(this.cast_const(), p),
                    }
                }
                (*frame).cached_anim = skel_anim.cast();
            }

            let parent = if parent_hdr.is_null() {
                frame
            } else {
                parent_hdr
            };
            let num_bones = if root_only { n.min(1) } else { n };

            if skel_anim.is_null() || seq.is_none() {
                // Reference pose.
                for (b, bone) in skel.iter().take(num_bones).enumerate() {
                    let place = bone.bone_pos.place;
                    *cached_place(frame, b) = place;
                    if frame != parent {
                        *cached_place(parent, b) = place;
                    }
                }
                if !root_only {
                    (*frame).cached_anim = null_mut();
                }
            } else if let Some(mv) =
                this_move.filter(|mv| !tarr::<FAnalogTrack>(&mv.anim_tracks).is_empty())
            {
                let frame_time = prim::min_f(1.0, af) * mv.track_time;
                let alpha = if (*frame).cache_loaded != 0 && (*owner).tween_rate != 0.0 {
                    1.0 - tween_alpha
                } else {
                    0.0
                };
                let links = cached_links(frame, n);
                let tracks = tarr::<FAnalogTrack>(&mv.anim_tracks);
                let named = tarr::<FNamedBone>(&(*skel_anim).ref_bones);
                for (b, bone) in skel.iter().take(num_bones).enumerate() {
                    let link = links[b];
                    let mut place = if link < 0 {
                        if aux {
                            continue;
                        }
                        place_of(&bone.bone_pos.place)
                    } else {
                        let t_orig = link as usize;
                        let mut place = get_place(mv, t_orig, frame_time);
                        if b == 0
                            && let Some(track) = tracks.get(t_orig)
                        {
                            // Root motion: bone 0 only.
                            let mut t = link;
                            for _ in 0..named.len() {
                                let Some(p) = parent_step(named, t) else {
                                    break;
                                };
                                t = p;
                                place = place_shl(place, get_place(mv, t as usize, frame_time));
                            }
                            if (*owner).b_anim_move() {
                                if (*frame).b_got_root == 0 {
                                    (*frame).prev_root = key_pos(track, 0);
                                    let mut t = link;
                                    for _ in 0..named.len() {
                                        let Some(p) = parent_step(named, t) else {
                                            break;
                                        };
                                        t = p;
                                        (*frame).prev_root = coords::vec_times_coords(
                                            (*frame).prev_root,
                                            &coords_from_place(get_place(mv, t as usize, 0.0)),
                                        );
                                    }
                                    (*frame).movement_surplus = ZERO_V;
                                    (*frame).b_got_root = 1;
                                }
                                let delta = prim::sub(place.pos, (*frame).prev_root);
                                (*frame).prev_root = place.pos;
                                place.pos = skel[0].bone_pos.place.pos;
                                (*frame).movement = prim::add((*frame).movement, delta);
                                if (*frame).cached_seq == anim_seq && af != (*frame).prev_root_frame
                                {
                                    if !is_zero((*frame).movement_surplus) {
                                        let kp = key_pos_len(track) as i32 - 1;
                                        let mut max_rate =
                                            abs_v(prim::sub(key_pos(track, kp), key_pos(track, 0)));
                                        let amount = (if af != 0.0 { af } else { 1.0 })
                                            - (*frame).prev_root_frame;
                                        max_rate = prim::scale(max_rate, amount);
                                        let surplus = (*frame).movement_surplus;
                                        let adjust = prim::v3(
                                            clamp(surplus.x, -max_rate.x, max_rate.x),
                                            clamp(surplus.y, -max_rate.y, max_rate.y),
                                            clamp(surplus.z, -max_rate.z, max_rate.z),
                                        );
                                        (*frame).movement = prim::add((*frame).movement, adjust);
                                        (*frame).movement_surplus = prim::sub(surplus, adjust);
                                    }
                                    (*frame).prev_root_frame = af;
                                }
                            }
                        }
                        place
                    };
                    if alpha != 0.0 {
                        place = interp_place(place, place_of(&*cached_place(frame, b)), alpha);
                    }
                    *cached_place(frame, b) = raw_place(place);
                    if frame != parent {
                        *cached_place(parent, b) = raw_place(place);
                    }
                }
            }
        }

        if !root_only {
            (*frame).cache_loaded = 1;
            (*frame).cached_seq = anim_seq;
            (*frame).cached_frame = af;
            (*frame).cached_alpha = tween_alpha;
            let mut a = 0usize;
            loop {
                let aux = (*owner).aux_anims;
                let Some(&chan) = tarr::<*mut AActor>(&aux).get(a) else {
                    break;
                };
                apply_anim(host, this, chan, frame, false);
                let c = &*chan;
                if c.b_anim_transient() && !c.b_anim_loop() && c.anim_frame >= c.anim_last {
                    host.remove_aux(owner, a);
                } else {
                    a += 1;
                }
            }
        }
        host.cache_unlock(item);
    }
}

/// Fresh cache block layout and reset state (the `GCache.Create` branch of `ApplyAnim`).
///
/// # Safety
/// `mem` must point to a block of at least `CFSKEL_HEADER_SIZE + 4*n + 28*n` bytes.
unsafe fn init_cache(mem: *mut u8, this: *mut USkeletalMesh, n: usize) {
    unsafe {
        let h = mem.cast::<CFSkelHeader>();
        (*h).cached_mesh = this.cast();
        (*h).cached_links = mem.add(CFSKEL_HEADER_SIZE).cast();
        (*h).cached_places = mem.add(CFSKEL_HEADER_SIZE + 4 * n).cast();
        (*h).cache_loaded = 0;
        (*h).movement = ZERO_V;
        (*h).prev_root = ZERO_V;
        (*h).movement_surplus = ZERO_V;
        (*h).cached_anim = null_mut();
        (*h).cached_seq = NAME_NONE;
        (*h).cached_frame = -1.0;
        (*h).cached_alpha = -1.0;
        (*h).prev_bound = box_to_raw(zero_box());
    }
}

/// `Clamp(X, Min, Max)` (UnTemplate.h).
fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

fn abs_v(v: FVector) -> FVector {
    prim::v3(v.x.abs(), v.y.abs(), v.z.abs())
}

fn is_zero(v: FVector) -> bool {
    v.x == 0.0 && v.y == 0.0 && v.z == 0.0
}

/// `USkeletalMesh::GetFrame(ResultVerts, Size, Coords, Owner, LODRequest)`.
///
/// Writes `LODRequest` (clamped to `FrameVerts`) vertices at byte stride `size`.
///
/// # Safety
/// `this` and `owner` must point to live objects; `result` must be writable for
/// `min(LODRequest, FrameVerts)` vertices at stride `size`.
pub unsafe fn get_frame<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    result: *mut u8,
    size: i32,
    coords: FCoords,
    owner: *mut AActor,
    lod: &mut i32,
) {
    unsafe {
        let anim_owner = if (*owner).b_anim_by_owner() && !(*owner).owner.is_null() {
            (*owner).owner.cast::<AActor>()
        } else {
            owner
        };
        let vertex_num = (*lod).min((*this).frame_verts);
        apply_anim(host, this, anim_owner, null_mut(), false);
        let skel_cache = SkelCache::new(host, anim_owner.cast_const());
        let frame = skel_cache.hdr();
        if frame.is_null() {
            check_failed(host, c"FrameHdr");
        }
        let mut mesh_coords = (*frame).mesh_coords;
        coords::div_coords(&mut mesh_coords, &inverse(&coords));

        let stride = usize::try_from(size).unwrap_or(0);
        let vcount = usize::try_from(vertex_num).unwrap_or(0);
        let bones = ref_skel(this);
        let n = bones.len();

        // SpaceBases: bone-space to mesh-space coords, parents before children.
        let mut bases = vec![coords::unit_coords(); n];
        if n > 0 {
            let mut c = coords_from_place(place_of(&*cached_place(frame, 0)));
            coords::div_coords(&mut c, &mesh_coords);
            bases[0] = c;
        }
        for (s, bone) in bones.iter().enumerate().skip(1) {
            let parent = bone.parent_index;
            if parent == s as i32 {
                check_failed(host, c"Parent != s");
            }
            let pb = usize::try_from(parent)
                .ok()
                .and_then(|p| bases.get(p).copied())
                .unwrap_or_else(coords::unit_coords);
            let mut c = coords_from_place(place_of(&*cached_place(frame, s)));
            coords::div_coords(&mut c, &pb);
            bases[s] = c;
        }

        if (*this).display_bones != 0 {
            host.debug_pivots(this, &bases);
        }
        if (*this).display_influences != 0 && (*this).debug_verts.num == 0 {
            host.debug_influences(this);
        }

        let base0 = bases.first().copied().unwrap_or_else(coords::unit_coords);
        let bw_idx = tarr::<VBoneInfIndex>(&(*this).bone_weight_idx);
        let bone_weights = tarr::<VBoneInfluence>(&(*this).bone_weights);
        let local_points = tarr::<FVector>(&(*this).local_points);
        if bw_idx.len() == 1 {
            let number = usize::try_from(vertex_num.min(i32::from(bw_idx[0].number))).unwrap_or(0);
            for (b, lp) in local_points.iter().take(number).enumerate() {
                write_vert(result, b, stride, coords::vec_times_coords(*lp, &base0));
            }
        } else {
            let mut out = vec![ZERO_V; vcount];
            for (w, idx) in bw_idx.iter().enumerate() {
                let base_w = bases.get(w).copied().unwrap_or_else(coords::unit_coords);
                let first = usize::from(idx.weight_index);
                let count = usize::from(idx.number);
                for b in first..first + count {
                    let (Some(bw), Some(lp)) = (bone_weights.get(b), local_points.get(b)) else {
                        break;
                    };
                    let vi = i32::from(bw.point_index);
                    if vi >= vertex_num {
                        break;
                    }
                    let weight = f32::from(bw.bone_weight) * (1.0_f32 / 65535.0_f32);
                    let v = coords::vec_times_coords(*lp, &base_w);
                    if let Some(o) = out.get_mut(vi as usize) {
                        *o = prim::add(*o, prim::scale(v, weight));
                    }
                }
            }
            for (v, p) in out.iter().enumerate() {
                write_vert(result, v, stride, *p);
            }
        }
        *lod = vertex_num;

        let wbi = (*this).weapon_bone_index;
        if wbi >= 0 {
            let wb = bases
                .get(wbi as usize)
                .copied()
                .unwrap_or_else(coords::unit_coords);
            let mut orient = (*this).weapon_adjust;
            coords::div_coords(&mut orient, &wb);
            orient.x_axis = prim::safe_normal(orient.x_axis);
            orient.y_axis = prim::safe_normal(cross_fused(orient.x_axis, orient.z_axis));
            orient.z_axis = cross_fused(orient.x_axis, orient.y_axis);
            orient.y_axis = prim::neg(orient.y_axis);
            let mut u = coords::unit_coords();
            coords::mul_coords(&mut u, &orient);
            (*this).classic_weapon_coords = u;
        }
    }
}

/// Unaligned store of vertex `index` at byte stride `stride`.
///
/// # Safety
/// `base + index * stride` must be writable for an `FVector`.
unsafe fn write_vert(base: *mut u8, index: usize, stride: usize, v: FVector) {
    unsafe { std::ptr::write_unaligned(base.add(index * stride).cast::<FVector>(), v) }
}

/// `USkeletalMesh::GetRootMovement(Owner)`: takes and clears accumulated root motion.
///
/// # Safety
/// `this` and `owner` must point to live objects.
pub unsafe fn get_root_movement<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    owner: *mut AActor,
) -> FVector {
    unsafe {
        apply_anim(host, this, owner, null_mut(), true);
        let skel = SkelCache::new(host, owner.cast_const());
        let hdr = skel.hdr();
        if hdr.is_null() {
            return ZERO_V;
        }
        let mut vc = (*hdr).mesh_coords;
        vc.origin = ZERO_V;
        let mv = coords::vec_times_coords((*hdr).movement, &vc);
        (*hdr).movement = ZERO_V;
        mv
    }
}

/// `USkeletalMesh::AdjustRootMovement(Owner, Adjust)`.
///
/// # Safety
/// `owner` must point to a live actor.
pub unsafe fn adjust_root_movement<H: SkelHost>(host: &H, owner: *mut AActor, adjust: FVector) {
    unsafe {
        if is_zero(adjust) {
            return;
        }
        let skel = SkelCache::new(host, owner.cast_const());
        let hdr = skel.hdr();
        if !hdr.is_null() {
            (*hdr).movement_surplus = prim::add(
                (*hdr).movement_surplus,
                prim::transform_vector(adjust, &(*hdr).mesh_coords),
            );
        }
    }
}

/// `USkeletalMesh::AnimCycleMovement(Owner)`: root displacement over one cycle.
///
/// # Safety
/// `this` and `owner` must point to live objects.
pub unsafe fn anim_cycle_movement<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    owner: *mut AActor,
) -> FVector {
    unsafe {
        apply_anim(host, this, owner, null_mut(), true);
        let skel = SkelCache::new(host, owner.cast_const());
        let hdr = skel.hdr();
        let n = ref_skel(this).len();
        if hdr.is_null() || n == 0 {
            return ZERO_V;
        }
        let link0 = cached_links(hdr, n)[0];
        if link0 < 0 {
            return ZERO_V;
        }
        let anim = (*owner).skel_anim.cast::<UAnimation>();
        if anim.is_null() {
            return ZERO_V;
        }
        let Some(mv) = find_movement(anim, (*owner).anim_sequence) else {
            return ZERO_V;
        };
        let tracks = tarr::<FAnalogTrack>(&mv.anim_tracks);
        if tracks.is_empty() {
            return ZERO_V;
        }
        let Some(track) = tracks.get(link0 as usize) else {
            return ZERO_V;
        };
        let last = key_pos_len(track) as i32 - 1;
        prim::sub(key_pos(track, last), key_pos(track, 0))
    }
}

/// `USkeletalMesh::GetBoneCoords(Owner, Bone)`: bone `bone` in mesh-space coords.
///
/// # Safety
/// `this` and `owner` must point to live objects.
pub unsafe fn get_bone_coords<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    owner: *mut AActor,
    bone: i32,
) -> FCoords {
    unsafe {
        // UnSkeletalMesh.cpp:966: `ApplyAnim( Owner, NULL, Bone==0 )`.
        apply_anim(host, this, owner, null_mut(), bone == 0);
        let skel = SkelCache::new(host, owner.cast_const());
        let hdr = skel.hdr();
        let refs = ref_skel(this);
        let n = refs.len();
        if !hdr.is_null()
            && let Some(mut b) = usize::try_from(bone).ok().filter(|&b| b < n)
        {
            let mut c = coords_from_place(place_of(&*cached_place(hdr, b)));
            while b != 0 {
                let parent = refs[b].parent_index;
                if parent < 0 || parent as usize >= n {
                    break;
                }
                b = parent as usize;
                let step = coords_from_place(place_of(&*cached_place(hdr, b)));
                coords::div_coords(&mut c, &step);
            }
            coords::div_coords(&mut c, &(*hdr).mesh_coords);
            return c;
        }
        get_mesh_coords(host, owner.cast_const(), this.cast_const())
    }
}

/// `USkeletalMesh::GetRenderBoundingBox(Owner, Exact)`: mesh-space bounds as coords.
///
/// # Safety
/// `this` and `owner` must point to live objects.
pub unsafe fn get_render_bounding_box<H: SkelHost>(
    host: &H,
    this: *mut USkeletalMesh,
    owner: *mut AActor,
) -> FCoords {
    unsafe {
        apply_anim(host, this, owner, null_mut(), true);
        let skel = SkelCache::new(host, owner.cast_const());
        let hdr = skel.hdr();
        let bound = anim_box(host, owner.cast_const());
        if hdr.is_null() {
            // Unreachable: `apply_anim` always creates the cache (UnSkeletalMesh.cpp:480-499).
            check_failed(host, c"FrameHdr");
        }
        let mc = (*hdr).mesh_coords;
        let bb = box_from_raw(&(*this).bounding_box);
        if !bound.valid {
            return coords::box_times_coords(&bb, &mc);
        }
        let mut c0 = coords_from_place(place_of(&*cached_place(hdr, 0)));
        coords::div_coords(&mut c0, &mc);
        coords::box_times_coords(&bound, &c0)
    }
}

/// `USkeletalMesh::GetCollisionBoundingBox(Owner, bWorld)`.
///
/// # Safety
/// `this` and `owner` must point to live objects.
pub unsafe fn get_collision_bounding_box<H: SkelHost>(
    host: &H,
    this: *const USkeletalMesh,
    owner: *const AActor,
    world: bool,
) -> Aabb {
    unsafe {
        let m = &*this;
        if world {
            let mc = get_mesh_coords(host, owner, this);
            return box_from_coords(&coords::box_times_coords(
                &box_from_raw(&m.bounding_box),
                &mc,
            ));
        }
        let box_scale = prim::scale(m.scale, (*owner).draw_scale);
        let bb = &m.bounding_box;
        Aabb {
            min: coords::mul_comp(box_scale, prim::sub(bb.min, m.origin)),
            max: coords::mul_comp(box_scale, prim::sub(bb.max, m.origin)),
            valid: true,
        }
    }
}
