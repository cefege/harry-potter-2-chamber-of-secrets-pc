//! `FLightManager::SetupForSurf` and its per-light fog pass (`UnLight.cpp`).
//! The Rust driver owns the light/link walks, cache decisions, allocation and
//! merge order. C++ supplies only raw engine leaf callbacks through
//! `Hp2LightServices`.
//!
//! Float contraction follows the arm64 clang `-ffp-contract=on` build as in
//! `light.rs`: `a*b + c` fuses, and in `a*b ± c*d` the left product fuses. The
//! helpers below repeat the private `light.rs` helpers (that file belongs to the
//! kernels group).

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, UModel};

use crate::light::{GMath, MergeParams, VolumetricParams, merge, volumetric};
use crate::services::Hp2RenderServices;
use crate::services_light::Hp2LightServices;

/// Typed engine leaves used by the surface setup and merge policies.
pub trait SurfSetupOps {
    /// A failed leaf stops setup before its output is consumed.
    type Error;

    /// Adds an actor to the current light list.
    unsafe fn add_light(&self, mover: *mut c_void, actor: *mut c_void) -> Result<u32, Self::Error>;
    /// Initializes surface map state for the requested phase.
    unsafe fn surf_map_setup(&self, params: *mut SurfSetupParams, which: u32) -> Result<(), Self::Error>;
    /// Marks a temporary-memory allocation scope.
    unsafe fn surf_gmem_mark(&self, slot: u32, pop: u32) -> Result<(), Self::Error>;
    /// Allocates temporary engine memory.
    unsafe fn surf_gmem(&self, bytes: u64) -> Result<*mut c_void, Self::Error>;
    /// Computes one light for the requested phase.
    unsafe fn surf_light_compute(&self, i: u32, which: u32) -> Result<(), Self::Error>;
    /// Looks up a cached surface allocation.
    unsafe fn surf_cache_get(&self, id: u64) -> Result<*mut c_void, Self::Error>;
    /// Creates a cached surface allocation.
    unsafe fn surf_cache_create(&self, id: u64, size: u64, pad: u64, check: u32) -> Result<*mut c_void, Self::Error>;
    /// Reads the zone ambient color.
    unsafe fn surf_zone_ambient(&self, out: *mut c_void) -> Result<(), Self::Error>;
    /// Generates a light shadow map.
    unsafe fn surf_shadow_gen(&self, i: u32, use_bits: u32, dest: *mut c_void) -> Result<(), Self::Error>;
    /// Applies the light's spatial effect.
    unsafe fn surf_light_spatial_fx(&self, i: u32, shadow: *mut c_void) -> Result<(), Self::Error>;
    /// Returns the random-base table used by surface merge.
    unsafe fn random_base_data(&self) -> Result<*const f32, Self::Error>;
    /// Returns the random table used by surface merge.
    unsafe fn random_data(&self) -> Result<*const f32, Self::Error>;
}

struct NativeSurfSetupOps<'a>(&'a Hp2LightServices);

impl SurfSetupOps for NativeSurfSetupOps<'_> {
    type Error = std::convert::Infallible;

    unsafe fn add_light(&self, mover: *mut c_void, actor: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.add_light)(mover, actor)) })
    }
    unsafe fn surf_map_setup(&self, params: *mut SurfSetupParams, which: u32) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_map_setup)(params.cast(), which)) })
    }
    unsafe fn surf_gmem_mark(&self, slot: u32, pop: u32) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_gmem_mark)(slot, pop)) })
    }
    unsafe fn surf_gmem(&self, bytes: u64) -> Result<*mut c_void, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_gmem)(bytes)) })
    }
    unsafe fn surf_light_compute(&self, i: u32, which: u32) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_light_compute)(i, which)) })
    }
    unsafe fn surf_cache_get(&self, id: u64) -> Result<*mut c_void, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_cache_get)(id)) })
    }
    unsafe fn surf_cache_create(&self, id: u64, size: u64, pad: u64, check: u32) -> Result<*mut c_void, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_cache_create)(id, size, pad, check)) })
    }
    unsafe fn surf_zone_ambient(&self, out: *mut c_void) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_zone_ambient)(out)) })
    }
    unsafe fn surf_shadow_gen(&self, i: u32, use_bits: u32, dest: *mut c_void) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_shadow_gen)(i, use_bits, dest)) })
    }
    unsafe fn surf_light_spatial_fx(&self, i: u32, shadow: *mut c_void) -> Result<(), Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.surf_light_spatial_fx)(i, shadow)) })
    }
    unsafe fn random_base_data(&self) -> Result<*const f32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.random_base_data)()) })
    }
    unsafe fn random_data(&self) -> Result<*const f32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.random_data)()) })
    }
}

/// `ELightKind` (`ALO_*`), as stored in `FLightInfo::Opt`.
const ALO_STATIC_LIGHT: u32 = 0;
const ALO_DYNAMIC_LIGHT: u32 = 1;
const ALO_MOVING_LIGHT: u32 = 2;

/// Per-light path chosen in `SetupForSurf`. `KIND_NONE` covers `ALO_NotLight`
/// and unknown codes.
pub const KIND_NONE: u32 = 0;
pub const KIND_STATIC: u32 = 1;
pub const KIND_MOVING: u32 = 2;
pub const KIND_SPATIAL_DYNAMIC: u32 = 3;
pub const KIND_CACHED_DYNAMIC: u32 = 4;

/// Dynamic-map cache outcome (`GCache.Get` plus the FTime test).
pub const CACHE_CREATE: u32 = 0;
pub const CACHE_REFRESH: u32 = 1;
pub const CACHE_SKIP: u32 = 2;

/// `ALO_*` to the path taken in the static and dynamic loops of `SetupForSurf`.
pub fn light_kind(opt: u32, spatial_dynamic: bool) -> u32 {
    match opt {
        ALO_STATIC_LIGHT => KIND_STATIC,
        ALO_DYNAMIC_LIGHT if spatial_dynamic => KIND_SPATIAL_DYNAMIC,
        ALO_DYNAMIC_LIGHT => KIND_CACHED_DYNAMIC,
        ALO_MOVING_LIGHT => KIND_MOVING,
        _ => KIND_NONE,
    }
}

/// `appCeilLogTwo` (UnMisc.cpp): the smallest N with `(1 << N) >= arg`, and 0
/// for `arg == 0` (the C++ `--Arg == MAXDWORD` early return).
pub fn ceil_log_two(arg: u32) -> u32 {
    u32::from(hp2_core::bits::ceil_log_two(arg))
}

/// Sizes and strides that `SetupForSurf` derives from `UClamp`/`VClamp` and the
/// mover-stamp size. Field order matches the C++ mirror `Hp2LightSurfDims` in
/// `UnLight.cpp`; the C++ side applies `appCheckedIntSize` to the `u64` fields
/// exactly where the original code called it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfDims {
    /// `appCeilLogTwo(UClamp)` and `appCeilLogTwo(VClamp)`. LightMip and FogMip
    /// use the same values.
    pub u_bits: i32,
    pub v_bits: i32,
    /// `1 << UBits`, `1 << VBits`: `LightMap`/`FogMap` `USize`/`VSize`.
    pub u_size: i32,
    pub v_size: i32,
    /// `ShadowMaskU = (UClamp+7)>>3`, `ShadowMaskSpace = ShadowMaskU*VClamp`,
    /// `ShadowSkip = ShadowMaskU*8 - UClamp`.
    pub shadow_mask_u: i32,
    pub shadow_mask_space: i32,
    pub shadow_skip: i32,
    /// Physical shadow-map bytes. Source convolutions with `VClamp==1` also
    /// write their third channel into a following row, so this exceeds the
    /// logical `ShadowMaskSpace*8` map in that case.
    pub shadow_alloc: i32,
    /// `UClamp*VClamp`: illumination-map bytes.
    pub illum_alloc: i32,
    /// `USize*VSize+1`: DWORD count of the merged dynamic map.
    pub merged_words: i32,
    /// `sizeof(FMoverStamp)/sizeof(DWORD)`: stamp words at the head of a static map.
    pub stamp_words: i32,
    /// `USize*VSize*sizeof(DWORD) + sizeof(FColor)`: fog map bytes.
    pub fog_alloc: u64,
    /// `USize*VClamp*sizeof(DWORD) + sizeof(FColor) + sizeof(FMoverStamp)`: static map.
    pub static_alloc: u64,
    /// `(USize*VClamp+3)*sizeof(DWORD)`: dynamic map (FTime, 2 words, FColor, rows).
    pub dyn_alloc: u64,
    /// `(UClamp*(VClamp+1)+1)*sizeof(BYTE)`: cached illumination map.
    pub illum_cache_alloc: u64,
    /// `USize*(VSize-VClamp)`: the `skip` argument of `GCache.Create` (static and dynamic).
    pub tail_skip: u64,
}

/// Sizing for one surface. The `i32` fields use wrapping int math as the C++
/// does; the `u64` fields widen before multiplying, so no `i32` overflow.
pub fn dims(uclamp: i32, vclamp: i32, stamp_bytes: u64) -> SurfDims {
    let u_bits = ceil_log_two(uclamp as u32);
    let v_bits = ceil_log_two(vclamp as u32);
    let u_size = 1i32.wrapping_shl(u_bits);
    let v_size = 1i32.wrapping_shl(v_bits);
    let shadow_mask_u = uclamp.wrapping_add(7) >> 3;
    let shadow_mask_space = shadow_mask_u.wrapping_mul(vclamp);
    let shadow_skip = shadow_mask_u.wrapping_mul(8).wrapping_sub(uclamp);
    let shadow_alloc = if vclamp == 1 {
        shadow_mask_u.wrapping_mul(16)
    } else {
        shadow_mask_space.wrapping_mul(8)
    };
    let illum_alloc = uclamp.wrapping_mul(vclamp);
    let merged_words = u_size.wrapping_mul(v_size).wrapping_add(1);

    let uc = uclamp as u64;
    let vc = vclamp as u64;
    let us = u_size as u64;
    let vs = v_size as u64;
    SurfDims {
        u_bits: u_bits as i32,
        v_bits: v_bits as i32,
        u_size,
        v_size,
        shadow_mask_u,
        shadow_mask_space,
        shadow_skip,
        shadow_alloc,
        illum_alloc,
        merged_words,
        stamp_words: (stamp_bytes / 4) as i32,
        fog_alloc: us * vs * 4 + 4,
        static_alloc: us * vc * 4 + 4 + stamp_bytes,
        dyn_alloc: (us * vc + 3) * 4,
        illum_cache_alloc: uc * (vc + 1) + 1,
        tail_skip: us * (vs - vc),
    }
}

/// `Clamp(appFloor(P*256), 0, 255)` for one `FColor` channel. `appFloor` is the
/// portable `static_cast<INT>`, which truncates.
#[inline]
fn color_channel(p: f32) -> u8 {
    ((p * 256.0) as i32).clamp(0, 255) as u8
}

/// `FColor(AmbientVector*0.25f)`, then `Exchange(R, B)` and `A = 127`, as the
/// little-endian DWORD that `GET_COLOR_DWORD` reads (bytes `R, G, B, A`).
/// `ambient` holds the X, Y, Z of the `FGetHSV` result.
pub fn ambient_word(ambient: [f32; 3]) -> u32 {
    let r = color_channel(ambient[0] * 0.25);
    let g = color_channel(ambient[1] * 0.25);
    let b = color_channel(ambient[2] * 0.25);
    u32::from_le_bytes([b, g, r, 127])
}

/// Fills the ambient rows of a static map: `vclamp` rows of `uclamp` DWORDs at
/// stride `u_size`, every word `ambient_word(ambient)`.
///
/// # Safety
/// `dest` must address `u_size * (vclamp - 1) + uclamp` writable DWORDs.
pub unsafe fn fill_ambient(
    ambient: [f32; 3],
    uclamp: i32,
    vclamp: i32,
    u_size: i32,
    dest: *mut u32,
) {
    let word = ambient_word(ambient);
    let stride = u_size as usize;
    let uc = uclamp.max(0) as usize;
    for i in 0..vclamp.max(0) as usize {
        for j in 0..uc {
            // SAFETY: the caller guarantees the rows are addressable.
            unsafe { *dest.add(i * stride + j) = word };
        }
    }
}

/// Copies `vclamp` rows of `uclamp` DWORDs (stride `u_size`) from the static map
/// to the dynamic map, as the `appMemcpy` loop in `SetupForSurf` does.
///
/// # Safety
/// Both `src` and `dst` must address `u_size * (vclamp - 1) + uclamp` DWORDs,
/// and the two regions must not overlap.
pub unsafe fn copy_static_rows(
    src: *const u32,
    dst: *mut u32,
    uclamp: i32,
    vclamp: i32,
    u_size: i32,
) {
    let stride = u_size as usize;
    let uc = uclamp.max(0) as usize;
    for i in 0..vclamp.max(0) as usize {
        // SAFETY: the caller guarantees the rows are addressable and disjoint.
        unsafe { std::ptr::copy_nonoverlapping(src.add(i * stride), dst.add(i * stride), uc) };
    }
}

/// `FMoverStamp` as stored at the head of a static map: `iLeaf`, `Location`,
/// `Rotation`. Same 28 bytes as the C++ struct (`sizeof(FMoverStamp)`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverStamp {
    pub i_leaf: i32,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
}

/// C++ actor fields needed while walking a model's static/permeating list.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SurfActorGeom {
    pub actor: *mut c_void,
    pub location: FVector,
    pub special_lit: u32,
}

/// `Model->Lights(Index)`: unchecked `UModel` mirror index, as the oracle's
/// `TArray::operator()` is in a release (non-debug) build. Returns `None`
/// when the slot is null, matching `LightSurfModelLight`'s `*Out != NULL`.
#[inline]
pub unsafe fn surf_model_light(model: *mut c_void, index: i32) -> Option<SurfActorGeom> {
    unsafe {
        let lights = std::ptr::addr_of!((*(model as *const UModel)).lights);
        let data = std::ptr::addr_of!((*lights).data).read_unaligned();
        let actor = *(data as *const *mut c_void).add(index as usize);
        if actor.is_null() {
            None
        } else {
            Some(surf_actor_geom(actor))
        }
    }
}

/// `Actor->Location`/`Actor->bSpecialLit`: direct `AActor` mirror read,
/// matching `LightSurfActorGeom`.
#[inline]
pub unsafe fn surf_actor_geom(actor: *mut c_void) -> SurfActorGeom {
    unsafe {
        let a = &*(actor as *const AActor);
        SurfActorGeom {
            actor,
            location: a.location,
            special_lit: a.b_special_lit() as u32,
        }
    }
}

/// Snapshot of one `FLightInfo`. `surf_light_info` only copies this structure;
/// it must not inspect another light or alter list state.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SurfLightInfo {
    pub actor: *mut c_void,
    pub actor_index: u32,
    pub opt: u32,
    pub spatial_dynamic: u32,
    pub volumetric: u32,
    pub shadow_bits: *mut c_void,
    pub location_z: f32,
    pub light_changed: u32,
    pub light_effect: u8,
    pub _pad: [u8; 3],
    pub vol_palette: *const [u8; 4],
    pub vol_location: FVector,
    pub vol_radius_squared: f32,
    pub rvol_radius: f32,
    pub rvol_radius_squared: f32,
    pub vol_brightness: f32,
    pub location_size_squared: f32,
    pub vol_inside: u32,
    pub ambient: u32,
}

impl SurfLightInfo {
    #[inline]
    fn volumetric_params(self) -> VolumetricParams {
        VolumetricParams {
            location: self.vol_location,
            vol_radius_squared: self.vol_radius_squared,
            rvol_radius: self.rvol_radius,
            rvol_radius_squared: self.rvol_radius_squared,
            vol_brightness: self.vol_brightness,
            location_size_squared: self.location_size_squared,
            vol_inside: self.vol_inside != 0,
            ambient: self.ambient != 0,
        }
    }
}

/// All non-engine data read or written by `setup_for_surf`.
///
/// This is the exact C++ mirror contract. Pointer members remain raw engine
/// pointers. `surf_map_setup(params, 0)` initializes LightMap/LightMip from
/// this snapshot, `surf_map_setup(params, 1)` does the same for FogMap/FogMip,
/// mode 2 commits their final `data`, cache ID and realtime state, and mode 3
/// lazily writes `current_time` from `Frame->Viewport->CurrentTime`. None of
/// those modes may walk a light list or make a control-flow decision.
#[repr(C)]
pub struct SurfSetupParams {
    pub manager: *mut c_void,
    pub frame: *mut c_void,
    pub draw: *mut c_void,
    pub input_coords: *const c_void,
    pub level_model: *mut c_void,
    pub active_model: *mut c_void,
    pub mover: *mut c_void,
    pub zone: *mut c_void,
    pub light_map: *mut c_void,
    pub fog_map: *mut c_void,
    pub light_mip: *mut c_void,
    pub fog_mip: *mut c_void,
    pub out_light_map: *mut *mut c_void,
    pub out_fog_map: *mut *mut c_void,
    pub i_surf: i32,
    pub i_light_map: i32,
    pub zone_id: u32,
    pub poly_flags: u32,
    pub merged: u32,
    pub uclamp: i32,
    pub vclamp: i32,
    pub dims: SurfDims,
    pub pan: FVector,
    pub uscale: f32,
    pub vscale: f32,
    pub mover_dynamic_light: u32,
    pub mover_leaf: i32,
    pub mover_special_lit: u32,
    pub model_has_leaves: u32,
    pub leaf_permeating: i32,
    pub zone_fog: u32,
    pub static_light_index: i32,
    pub static_light_bits: *mut u8,
    pub surf_lights: *mut c_void,
    pub volumetrics: *mut c_void,
    pub light_capacity: u32,
    pub model_cache_index: u32,
    pub current_time: i64,
    pub mover_stamp: MoverStamp,
    pub frame_coords: [FVector; 4],
    pub vertex_base: FVector,
    pub vertex_du: FVector,
    pub vertex_dv: FVector,
    pub byte_fog: *const u8,
    pub fog_rejection: *mut i32,
    /// Rust writes these before invoking the map metadata leaf callbacks.
    pub light_data: *mut c_void,
    pub fog_data: *mut c_void,
    pub light_cache_id: u64,
    pub fog_cache_id: u64,
    pub light_realtime_changed: u32,
    pub fog_realtime_changed: u32,
}

const PF_TRANSLUCENT: u32 = 0x0000_0004;
const CID_SHADOW_MAP: u64 = 0x15;
const CID_ILLUMINATION_MAP: u64 = 0x16;
const CID_STATIC_MAP: u64 = 0x18;
const CID_RENDER_FOG_MAP: u64 = 0x26;
const CID_DYNAMIC_MAP: u64 = 0x31;
const COUNTER_DYNAMIC_LIGHTS: u32 = 0;
const COUNTER_MOVING_LIGHTS: u32 = 1;
const COUNTER_STATIC_LIGHTING_CHANGED: u32 = 2;

/// `StaticLightingChanged |= ...` for the stored stamp against the mover's
/// current `iLeaf`, `Location` and `Rotation`. The float compare is `!=` per
/// component (`FVector::operator!=`), so NaN compares unequal.
pub fn mover_stamp_changed(stored: &MoverStamp, current: &MoverStamp) -> bool {
    stored.i_leaf != current.i_leaf
        || stored.location[0] != current.location[0]
        || stored.location[1] != current.location[1]
        || stored.location[2] != current.location[2]
        || stored.rotation != current.rotation
}

/// Dynamic-map cache outcome. `has_stream` is `GCache.Get` non-null; `stored` is
/// the FTime at its head (ignored when there is no stream); `now` is the frame's
/// CurrentTime.
pub fn dyn_cache_outcome(has_stream: bool, stored: i64, now: i64) -> u32 {
    if !has_stream {
        CACHE_CREATE
    } else if stored != now {
        CACHE_REFRESH
    } else {
        CACHE_SKIP
    }
}

/// `((Location - Origin) | ZAxis) > 0.0f` in the permeating-light test.
pub fn dot_positive(location: FVector, origin: FVector, z_axis: FVector) -> bool {
    dot(v_sub(location, origin), z_axis) > 0.0
}

/// `FCoords` (`Origin`, `XAxis`, `YAxis`, `ZAxis`) for the fog transform.
#[derive(Clone, Copy, Debug)]
pub struct Coords {
    pub origin: FVector,
    pub x_axis: FVector,
    pub y_axis: FVector,
    pub z_axis: FVector,
}

/// Inputs for one light's `MergeVolumetrics` pass: the three vertex vectors
/// (`VertexBase`, `VertexDU`, `VertexDV`), the frame coords, the volumetric
/// light fields and the map extents.
#[derive(Clone, Copy, Debug)]
pub struct FogLightInputs {
    pub coords: Coords,
    pub vertex_base: FVector,
    pub vertex_du: FVector,
    pub vertex_dv: FVector,
    pub vol: VolumetricParams,
    pub uclamp: i32,
    pub vclamp: i32,
    pub u_size: i32,
}

/// `FVector - FVector`.
#[inline]
fn v_sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

/// `FVector + FVector`.
#[inline]
fn v_add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

/// `FVector::operator|` (dot product): `X*V.X + Y*V.Y + Z*V.Z`. The oracle fuses the `Z` and
/// `X` products into `fmadd` and leaves `Y*V.Y` unfused (fma_surf.md).
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `FVector::TransformPointBy` (portable branch): `Temp = *this - Origin`, then
/// the dot with each axis.
#[inline]
fn transform_point(v: FVector, c: &Coords) -> FVector {
    let t = v_sub(v, c.origin);
    FVector {
        x: dot(t, c.x_axis),
        y: dot(t, c.y_axis),
        z: dot(t, c.z_axis),
    }
}

/// `FVector::TransformVectorBy` (portable branch): the dot with each axis.
#[inline]
fn transform_vector(v: FVector, c: &Coords) -> FVector {
    FVector {
        x: dot(v, c.x_axis),
        y: dot(v, c.y_axis),
        z: dot(v, c.z_axis),
    }
}

/// Palette index `appRound(fog * 255.0f)`. `Volumetric` already returns a value
/// in [0, 1] (`MinPositiveFloat`), so the index stays in 0..=255.
#[inline]
fn palette_index(fog: f32) -> usize {
    (fog * 255.0) as i32 as usize
}

/// One light's fog over the fog map. `palette` is `VolPalette` (`FColor[256]`,
/// bytes `R, G, B, A`), `byte_fog` is `ByteFog`, `dest` is the first texel of
/// the fog map (stride `u_size`). `virgin` is the C++ `VirginFog` flag: the
/// first light fills the map from the palette, later lights blend through
/// `ByteFog`. `fog_rejection` is the shared `FogRejectionMethod` state.
///
/// Deliberate C++ behaviour: `ByteFog[128*A + R]` is read with the palette
/// `A` and texel `R` unchecked. When `A >= 128` the index can pass
/// `BYTE_FOG_LEN`, which is an out-of-bounds read in the C++ too. It is kept as a
/// raw read, not clamped.
///
/// # Safety
/// `palette` addresses 256 texels, `byte_fog` addresses at least the reachable
/// `ByteFog` entries, and `dest` addresses `u_size * (vclamp - 1) + uclamp`
/// texels.
pub unsafe fn fog_light(
    g: &GMath<'_>,
    p: &FogLightInputs,
    virgin: bool,
    palette: *const [u8; 4],
    byte_fog: *const u8,
    dest: *mut [u8; 4],
    fog_rejection: &mut i32,
) {
    let mut vertex1 = transform_point(p.vertex_base, &p.coords);
    let du = transform_vector(p.vertex_du, &p.coords);
    let dv = transform_vector(p.vertex_dv, &p.coords);
    let stride = p.u_size as usize;
    let uc = p.uclamp.max(0) as usize;

    for i in 0..p.vclamp.max(0) as usize {
        let mut vertex = vertex1;
        // SAFETY: the caller guarantees the rows are addressable.
        let row = unsafe { dest.add(i * stride) };
        for j in 0..uc {
            let fog = volumetric(g, &p.vol, vertex, fog_rejection);
            // SAFETY: the palette, ByteFog and texel reads follow the C++ indexing,
            // see the function doc.
            unsafe {
                if virgin {
                    *row.add(j) = *palette.add(palette_index(fog));
                } else if fog.to_bits() != 0 {
                    let light = palette_index(fog);
                    let pal = *palette.add(light);
                    let table = byte_fog.add(128 * pal[3] as usize);
                    let texel = &mut *row.add(j);
                    texel[0] =
                        (table.add(texel[0] as usize).read() as i32 + pal[0] as i32).min(127) as u8;
                    texel[1] =
                        (table.add(texel[1] as usize).read() as i32 + pal[1] as i32).min(127) as u8;
                    texel[2] =
                        (table.add(texel[2] as usize).read() as i32 + pal[2] as i32).min(127) as u8;
                    texel[3] = (texel[3] as i32 + pal[3] as i32).min(127) as u8;
                }
            }
            vertex = v_add(vertex, du);
        }
        vertex1 = v_add(vertex1, dv);
    }
}

#[inline]
fn cache_id(base: u64, word: i32, byte: u32, frame_index: u32) -> u64 {
    base + ((byte as u64) << 8) + ((word as u32 as u64) << 16) + ((frame_index as u64) << 32)
}

#[inline]
pub unsafe fn surf_light_info(index: u32) -> SurfLightInfo {
    unsafe {
        let s = crate::light_manager::state();
        let info = &(*s).FirstLight[index as usize];
        let a = &*(info.Actor as *const AActor);
        SurfLightInfo {
            actor: info.Actor,
            actor_index: a.index as u32,
            opt: info.Opt as u32,
            spatial_dynamic: info.Effect.IsSpatialDynamic as u32,
            volumetric: info.IsVolumetric as u32,
            shadow_bits: info.ShadowBits.cast(),
            location_z: info.Location[2],
            light_changed: a.b_light_changed() as u32,
            light_effect: a.light_effect,
            _pad: [0; 3],
            vol_palette: info.VolPalette.cast(),
            vol_location: FVector {
                x: info.Location[0],
                y: info.Location[1],
                z: info.Location[2],
            },
            vol_radius_squared: info.VolRadiusSquared,
            rvol_radius: info.RVolRadius,
            rvol_radius_squared: info.RVolRadiusSquared,
            vol_brightness: info.VolBrightness,
            location_size_squared: info.LocationSizeSquared,
            vol_inside: (info.VolInside != 0) as u32,
            ambient: (a.light_source as u32 == crate::light::LD_AMBIENT) as u32,
        }
    }
}

/// `FLightManager::Merge` + `LightSurfMerge`, ported straight into Rust.
/// `Tex` is the shared `LightMap`; `UClamp`/`USize`/`VClamp` sit at byte
/// offsets 68/60/72 of `FTextureInfo` (verified via `offsetof` against the
/// real engine header), inside the opaque `State.LightMap` blob. `Info` is
/// `FirstLight[index]`; `Effect` is `Info.Actor->LightEffect`. `stream`/
/// `dest` are the lightmap words (`USize*VClamp` each); `stream == dest`
/// (always true at both call sites today) runs the in-place blend, matching
/// `Merge(.., Stream, Stream)`.
///
/// `stream` and `dest` each address `USize*VClamp` light-map words. Source
/// callers currently merge in place, but the distinct-pointer case is retained.
///
/// # Safety
/// All engine-state pointers and the buffers implied by the source dimensions
/// must be live and writable for this merge.
#[inline]
pub unsafe fn surf_merge(
    services: &Hp2LightServices,
    index: u32,
    stream: *mut c_void,
    dest: *mut c_void,
) {
    match unsafe { surf_merge_with(&NativeSurfSetupOps(services), index, stream, dest) } {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

/// Merges a light using caller-provided source operations.
///
/// # Safety
/// All engine-state pointers and the buffers implied by the source dimensions
/// must be live and writable for this merge.
#[inline]
pub unsafe fn surf_merge_with<O: SurfSetupOps + ?Sized>(
    ops: &O,
    index: u32,
    stream: *mut c_void,
    dest: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let s = crate::light_manager::state();
        let info = &(*s).FirstLight[index as usize];
        let actor = &*(info.Actor as *const AActor);
        let texture = (*s).LightMap.as_ptr().cast::<u8>();
        let uclamp = *texture.add(68).cast::<i32>();
        let u_size = *texture.add(60).cast::<i32>();
        let vclamp = *texture.add(72).cast::<i32>();
        let params = MergeParams {
            effect: actor.light_effect,
            key: 0,
            uclamp,
            usize_: u_size,
            min_u: info.MinU,
            max_u: info.MaxU,
            min_v: info.MinV,
            max_v: info.MaxV,
        };
        let illumination_len = (uclamp * vclamp).max(0) as usize;
        let word_len = (u_size * vclamp).max(0) as usize;
        let stream_ptr = stream.cast::<u32>();
        let dest_ptr = dest.cast::<u32>();
        let stream_slice = if std::ptr::eq(stream_ptr, dest_ptr) {
            None
        } else {
            Some(std::slice::from_raw_parts(stream_ptr, word_len))
        };
        let dest_slice = std::slice::from_raw_parts_mut(dest_ptr, word_len);
        let random_base = ops.random_base_data()?;
        let random = ops.random_data()?;
        merge(
            &params,
            std::slice::from_raw_parts(info.IlluminationMap.cast::<u8>(), illumination_len),
            &*(info.Palette as *const [u32; 256]),
            stream_slice,
            dest_slice,
            &*(random_base.cast::<[f32; 256]>()),
            &*(random.cast::<[f32; 256]>()),
        );
    }
    Ok(())
}

// `SurfLights` and `Volumetrics` are FActorLink chains; `link_read` is for leaf FVolActorLink nodes.
unsafe fn add_link_lights<O: SurfSetupOps + ?Sized>(
    ops: &O,
    mover: *mut c_void,
    mut link: *mut c_void,
    special_lit: Option<u32>,
) -> Result<(), O::Error> {
    while !link.is_null() {
        let (actor, next) = unsafe { crate::light_manager::read_actor_link(link) };
        let acceptable = match special_lit {
            None => true,
            Some(expected) => unsafe { surf_actor_geom(actor).special_lit == expected },
        };
        if acceptable {
            unsafe { ops.add_light(mover, actor)? };
        }
        link = next;
    }
    Ok(())
}

unsafe fn mark_volumetrics(mut link: *mut c_void, capacity: u32) {
    while !link.is_null() {
        let (actor, next) = unsafe { crate::light_manager::read_actor_link(link) };
        if unsafe { crate::light_manager::light_count() } < capacity as usize {
            unsafe { crate::light_manager::mark_volumetric(actor) };
        }
        link = next;
    }
}

/// Native callback-table entrypoint retained for existing callers.
///
/// # Safety
/// `params` and pointers reachable through the service tables must satisfy the
/// raw dereference requirements of the source routine.
pub unsafe fn setup_for_surf(
    params: &mut SurfSetupParams,
    services: &Hp2LightServices,
    render: &Hp2RenderServices,
) {
    match unsafe { setup_for_surf_with(params, &NativeSurfSetupOps(services), &GMath::new(render)) } {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

/// Full surface setup policy using caller-provided typed engine operations.
///
/// # Safety
/// `params` and pointers reachable through the selected operations
/// must satisfy the raw dereference requirements of the source routine.
pub unsafe fn setup_for_surf_with<O: SurfSetupOps + ?Sized>(
    params: &mut SurfSetupParams,
    ops: &O,
    g: &GMath<'_>,
) -> Result<(), O::Error> {
    params.light_data = std::ptr::null_mut();
    params.fog_data = std::ptr::null_mut();
    params.light_realtime_changed = 0;
    params.fog_realtime_changed = 0;
    let dims = dims(
        params.uclamp,
        params.vclamp,
        std::mem::size_of::<MoverStamp>() as u64,
    );
    params.dims = dims;
    unsafe {
        ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 0)?
    };
    unsafe {
        ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 1)?
    };
    unsafe { ops.surf_gmem_mark(0, 0)? };
    unsafe { crate::light_manager::counter_set(COUNTER_DYNAMIC_LIGHTS, 0) };
    unsafe { crate::light_manager::counter_set(COUNTER_MOVING_LIGHTS, 0) };
    unsafe { crate::light_manager::counter_set(COUNTER_STATIC_LIGHTING_CHANGED, 0) };
    let dynamic_mover = !params.mover.is_null() && params.mover_dynamic_light != 0;

    if !dynamic_mover {
        let mut shadow = params.static_light_bits;
        if params.static_light_index != -1 {
            let mut i = params.static_light_index;
            loop {
                let Some(geom) = (unsafe { surf_model_light(params.active_model, i) }) else {
                    break;
                };
                if unsafe {
                    ops.add_light(std::ptr::null_mut(), geom.actor)?
                } != 0
                {
                    unsafe {
                        let count = crate::light_manager::light_count() as u32;
                        crate::light_manager::set_shadow_bits(count - 1, shadow.cast())
                    };
                }
                shadow = unsafe { shadow.add(dims.shadow_mask_space as usize) };
                i = i.wrapping_add(1);
            }
        }
        unsafe { add_link_lights(ops, std::ptr::null_mut(), params.surf_lights, None)? };
    } else if params.mover_leaf != -1 && params.model_has_leaves != 0 {
        if params.leaf_permeating != -1 {
            let mut i = params.leaf_permeating;
            loop {
                let Some(geom) = (unsafe { surf_model_light(params.level_model, i) }) else {
                    break;
                };
                if dot_positive(
                    geom.location,
                    params.frame_coords[0],
                    params.frame_coords[3],
                ) && geom.special_lit == params.mover_special_lit
                {
                    unsafe {
                        ops.add_light(std::ptr::null_mut(), geom.actor)?
                    };
                }
                i = i.wrapping_add(1);
            }
        }
        unsafe {
            add_link_lights(ops, params.mover, params.surf_lights, Some(params.mover_special_lit))?
        };
        unsafe { mark_volumetrics(params.volumetrics, params.light_capacity) };
    }

    if !params.zone.is_null()
        && params.zone_fog != 0
        && !params.volumetrics.is_null()
        && (params.poly_flags & PF_TRANSLUCENT) == 0
    {
        unsafe { mark_volumetrics(params.volumetrics, params.light_capacity) };
        params.fog_cache_id = cache_id(
            CID_RENDER_FOG_MAP,
            params.i_light_map,
            params.zone_id,
            params.model_cache_index,
        );
        params.fog_realtime_changed = 1;
        let fog_base = unsafe {
            ops.surf_gmem(dims.fog_alloc)? as *mut u32
        };
        unsafe { *fog_base = u32::MAX };
        params.fog_data = unsafe { fog_base.add(1).cast() };
        unsafe {
            ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 1)?
        };

        let total = unsafe { crate::light_manager::light_count() } as u32;
        let indices = unsafe {
            ops.surf_gmem((total as u64) * 4)? as *mut u32
        };
        let mut count = 0usize;
        for i in 0..total {
            if unsafe { surf_light_info(i) }.volumetric != 0 {
                unsafe { ops.surf_light_compute(i, 1)? };
                let info = unsafe { surf_light_info(i) };
                let mut insert = count;
                while insert != 0 {
                    let previous = unsafe { *indices.add(insert - 1) };
                    if info
                        .location_z
                        .partial_cmp(&unsafe { surf_light_info(previous) }.location_z)
                        != Some(std::cmp::Ordering::Less)
                    {
                        break;
                    }
                    unsafe { *indices.add(insert) = previous };
                    insert -= 1;
                }
                unsafe { *indices.add(insert) = i };
                count += 1;
            }
        }
        unsafe { crate::light_manager::vtric_commit(indices, count as u32) };
        let coords = Coords {
            origin: params.frame_coords[0],
            x_axis: params.frame_coords[1],
            y_axis: params.frame_coords[2],
            z_axis: params.frame_coords[3],
        };
        for order in 0..count {
            let info = unsafe { surf_light_info(*indices.add(order)) };
            let fog = FogLightInputs {
                coords,
                vertex_base: params.vertex_base,
                vertex_du: params.vertex_du,
                vertex_dv: params.vertex_dv,
                vol: info.volumetric_params(),
                uclamp: params.uclamp,
                vclamp: params.vclamp,
                u_size: dims.u_size,
            };
            unsafe {
                fog_light(
                    g,
                    &fog,
                    order == 0,
                    info.vol_palette,
                    params.byte_fog,
                    params.fog_data.cast(),
                    &mut *params.fog_rejection,
                )
            };
        }
    }

    params.light_cache_id = cache_id(
        CID_STATIC_MAP,
        params.i_light_map,
        params.zone_id,
        params.model_cache_index,
    );
    let mut stream = unsafe {
        ops.surf_cache_get(params.light_cache_id)?
            as *mut u32
    };
    let static_changed =
        unsafe { crate::light_manager::counter_get(COUNTER_STATIC_LIGHTING_CHANGED) } != 0
            || (dynamic_mover
                && !stream.is_null()
                && mover_stamp_changed(
                    unsafe { &*(stream as *const MoverStamp) },
                    &params.mover_stamp,
                ));
    if stream.is_null() || static_changed {
        unsafe { crate::light_manager::counter_set(COUNTER_STATIC_LIGHTING_CHANGED, 1) };
        if stream.is_null() {
            stream = unsafe {
                ops.surf_cache_create(params.light_cache_id,
                dims.static_alloc,
                dims.tail_skip,
                1,)? as *mut u32
            };
        }
        if dynamic_mover {
            unsafe { *(stream as *mut MoverStamp) = params.mover_stamp };
        }
        stream = unsafe { stream.add(dims.stamp_words as usize) };
        unsafe { *stream = u32::MAX };
        stream = unsafe { stream.add(1) };
        params.light_data = stream.cast();
        unsafe {
            ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 0)?
        };
        let mut ambient = [0.0; 3];
        unsafe {
            ops.surf_zone_ambient(ambient.as_mut_ptr().cast())?
        };
        unsafe { fill_ambient(ambient, params.uclamp, params.vclamp, dims.u_size, stream) };
        for i in 0..unsafe { crate::light_manager::light_count() } as u32 {
            let info = unsafe { surf_light_info(i) };
            if light_kind(info.opt, info.spatial_dynamic != 0) == KIND_STATIC {
                unsafe { ops.surf_gmem_mark(1, 0)? };
                unsafe { ops.surf_light_compute(i, 0)? };
                let shadow = unsafe {
                    ops.surf_gmem(dims.shadow_alloc as u64)?
                };
                unsafe {
                    ops.surf_shadow_gen(i, 1, shadow)?
                };
                let illum = unsafe {
                    ops.surf_gmem(dims.illum_alloc as u64)?
                };
                unsafe { crate::light_manager::set_illum(i, illum) };
                unsafe {
                    ops.surf_light_spatial_fx(i, shadow)?
                };
                unsafe { surf_merge_with(ops, i, stream.cast(), stream.cast())? };
                unsafe { ops.surf_gmem_mark(1, 1)? };
            }
        }
    } else {
        stream = unsafe { stream.add(dims.stamp_words as usize + 1) };
        params.light_data = stream.cast();
        unsafe {
            ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 0)?
        };
    }

    if unsafe { crate::light_manager::counter_get(COUNTER_DYNAMIC_LIGHTS) } != 0
        || unsafe { crate::light_manager::counter_get(COUNTER_MOVING_LIGHTS) } != 0
    {
        let static_stream = stream;
        params.light_cache_id = cache_id(
            CID_DYNAMIC_MAP,
            params.i_light_map,
            params.zone_id,
            params.model_cache_index,
        );
        let skip_dynamic;
        if params.merged != 0 {
            stream = unsafe {
                ops.surf_gmem((dims.merged_words as u64) * 4)? as *mut u32
            };
            unsafe { *stream = u32::MAX };
            stream = unsafe { stream.add(1) };
            skip_dynamic = false;
        } else {
            stream = unsafe {
                ops.surf_cache_get(params.light_cache_id)?
                    as *mut u32
            };
            unsafe {
                ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 3)?
            };
            let outcome = if stream.is_null() {
                CACHE_CREATE
            } else {
                dyn_cache_outcome(
                    true,
                    unsafe { *(stream as *const i64) },
                    params.current_time,
                )
            };
            if outcome == CACHE_CREATE {
                stream = unsafe {
                    ops.surf_cache_create(params.light_cache_id,
                    dims.dyn_alloc,
                    dims.tail_skip,
                    1,)? as *mut u32
                };
            }
            unsafe { *(stream as *mut i64) = params.current_time };
            stream = unsafe { stream.add(2) };
            let max_color = stream;
            stream = unsafe { stream.add(1) };
            skip_dynamic = outcome == CACHE_SKIP;
            if !skip_dynamic {
                unsafe { *max_color = u32::MAX };
            }
        }
        params.light_data = stream.cast();
        params.light_realtime_changed = u32::from(!skip_dynamic);
        unsafe {
            ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 2)?
        };
        if !skip_dynamic {
            unsafe {
                copy_static_rows(
                    static_stream,
                    stream,
                    params.uclamp,
                    params.vclamp,
                    dims.u_size,
                )
            };
            for i in 0..unsafe { crate::light_manager::light_count() } as u32 {
                let info = unsafe { surf_light_info(i) };
                let kind = light_kind(info.opt, info.spatial_dynamic != 0);
                if kind == KIND_MOVING
                    || kind == KIND_SPATIAL_DYNAMIC
                    || kind == KIND_CACHED_DYNAMIC
                {
                    unsafe { ops.surf_gmem_mark(1, 0)? };
                    unsafe {
                        ops.surf_light_compute(i, 0)?
                    };
                    if kind == KIND_MOVING {
                        let shadow = unsafe {
                            ops.surf_gmem(dims.shadow_alloc as u64)?
                        };
                        unsafe {
                            ops.surf_shadow_gen(i, 0, shadow)?
                        };
                        let illum = unsafe {
                            ops.surf_gmem(dims.illum_alloc as u64)?
                        };
                        unsafe { crate::light_manager::set_illum(i, illum) };
                        unsafe {
                            ops.surf_light_spatial_fx(i, shadow)?
                        };
                    } else if kind == KIND_SPATIAL_DYNAMIC {
                        let id = cache_id(CID_SHADOW_MAP, params.i_surf, 0, info.actor_index);
                        let mut shadow = unsafe {
                            ops.surf_cache_get(id)?
                        };
                        if shadow.is_null() {
                            shadow = unsafe {
                                ops.surf_cache_create(id, dims.shadow_alloc as u64, 0, 0)?
                            };
                            unsafe {
                                ops.surf_shadow_gen(i, 1, shadow)?
                            };
                        }
                        let illum = unsafe {
                            ops.surf_gmem(dims.illum_alloc as u64)?
                        };
                        unsafe { crate::light_manager::set_illum(i, illum) };
                        unsafe {
                            ops.surf_light_spatial_fx(i, shadow)?
                        };
                    } else {
                        let id = cache_id(CID_ILLUMINATION_MAP, params.i_surf, 0, info.actor_index);
                        let mut illum = unsafe {
                            ops.surf_cache_get(id)?
                        };
                        unsafe { crate::light_manager::set_illum(i, illum) };
                        if illum.is_null() || info.light_changed != 0 {
                            let shadow = unsafe {
                                ops.surf_gmem(dims.shadow_alloc as u64)?
                            };
                            unsafe {
                                ops.surf_shadow_gen(i, 1, shadow)?
                            };
                            if illum.is_null() {
                                illum = unsafe {
                                    ops.surf_cache_create(id,
                                    dims.illum_cache_alloc,
                                    0,
                                    1,)?
                                };
                                unsafe { crate::light_manager::set_illum(i, illum) };
                            }
                            unsafe {
                                ops.surf_light_spatial_fx(i, shadow)?
                            };
                        }
                    }
                    unsafe { surf_merge_with(ops, i, stream.cast(), stream.cast())? };
                    unsafe { ops.surf_gmem_mark(1, 1)? };
                }
            }
        }
    }
    params.light_data = stream.cast();
    unsafe {
        ops.surf_map_setup((params as *mut SurfSetupParams).cast(), 2)?
    };
    Ok(())
}
