//! Engine services the lighting port (`UnLight.cpp`, class `FLightManager`)
//! reaches through `Hp2LightServices`. Passed per call, like
//! `Hp2RenderServices`; nothing is installed globally. The C++ side defines
//! `LightServicesTable` in `HarryPotter2/Unreal/Render/Src/UnLight.cpp`.
//!
//! `Hp2RenderServices` is shared with other render slices and carries no assert
//! callback, so the lighting checks that must stay live go through this table.

use std::ffi::{c_char, c_void};

/// Engine callbacks used by the lighting kernels. Field order matches
/// `Hp2LightServices` in `UnLight.cpp`; the C++ side pins the count with
/// `static_assert(sizeof(Hp2LightServices) == 30*sizeof(void*))`. Pure
/// `State` reads/writes (candidate storage, vtric list, counters, effect
/// table) are not here: they go straight through
/// `light_manager`/`light_info`'s own accessors onto Rust-owned memory.
///
/// `no_dynamic_lights`, `owner_has_palette`, `owner_palette_color`,
/// `owner_time_seconds`, `light_fill`, `light_write_back`, `model_light`,
/// `surf_model_light`, `surf_actor_geom`, `light_tags`, `fog`, `surf_light_info`,
/// `surf_light_changed` and `surf_merge` were removed: their C++ bodies were
/// plain `AActor`/`UClient`/`UTexture`/`UPalette`/`ALevelInfo`/`ULevel`/
/// `UModel`/`FLightInfo`/`FTextureInfo` field reads and (for `surf_merge`) a
/// pure parameter marshal into `light::merge` (or, for `surf_light_changed`,
/// dead code never called through this table), now done directly off the
/// generated mirrors and `light_manager::State` in
/// `hp2_abi::engine::generated` (`light_info::global`,
/// `light_actor::{fill_rec,write_back,model_light,setup_tail}`,
/// `light_surf::{surf_model_light,surf_actor_geom,surf_light_info,
/// surf_merge}`, `light_manager::{fog_light_at,fog_vtric_at}`,
/// `hp2-ffi::render_light_actor::hp2_rs_light_actor_add_light`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LightServices {
    /// A live `check(expr)` that failed. The C++ side calls
    /// `appFailAssert(expr, __FILE__, line)`, so the expression text, file and
    /// line match the original `check` in `UnLight.cpp`. `expr` is a
    /// NUL-terminated string. `line` is the line in the original `UnLight.cpp`.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32),
    /// `Level->Model->LineCheck(Hit, NULL, light->Location, actor->Location,
    /// FVector(0,0,0), 0)` with `FCheckResult Hit(0)`. Returns 1 when the path
    /// is clear. `actor` and `light` are `AActor*`.
    pub line_check: unsafe extern "C" fn(actor: *mut c_void, light: *mut c_void) -> u32,
    /// `GLightManagerInstance.AddLight(actor, light)`. `actor` and `light` are
    /// `AActor*`. Returns 1 when the light was appended to the light list.
    pub add_light: unsafe extern "C" fn(actor: *mut c_void, light: *mut c_void) -> u32,
    /// Sender cache liveness: `UObject::GetIndexedObject(index)==actor &&
    /// !actor->bDeleteMe`. The raw deref of `actor` stays in C++. Returns 1 when
    /// the sender is still live.
    pub sender_alive: unsafe extern "C" fn(index: i32, actor: *mut c_void) -> u32,
    /// `Owner->LifeFraction()`. Calls `GetClass()->GetDefaultActor()`, which has a
    /// live `check`, so it is only called from the palette-once path.
    pub owner_life_fraction: unsafe extern "C" fn(owner: *mut c_void) -> f32,
    /// `Actor->WorldLightRadius()`.
    pub world_light_radius: unsafe extern "C" fn(actor: *mut c_void) -> f32,
    /// `Actor->WorldVolumetricRadius()`.
    pub world_volumetric_radius: unsafe extern "C" fn(actor: *mut c_void) -> f32,
    /// `Actor->GetViewRotation().Vector()`, written as FVector (12 bytes) to `out`.
    pub view_vector: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    /// `FLightManager::MapCoords->Inverse().Transpose()`, written as FCoords to `out`.
    pub map_uncoords: unsafe extern "C" fn(map_coords: *const c_void, out: *mut c_void),
    /// `FGetHSV(hue, sat, 255)`, written as FPlane (16 bytes) to `out`.
    pub global_hsv: unsafe extern "C" fn(hue: u8, sat: u8, out: *mut c_void),
    /// SDK cache lookup with a slot reserved by `light_manager::reserve_slot`.
    /// Stores the data pointer in `out`. Returns 1 when found.
    pub cache_get:
        unsafe extern "C" fn(kind: u32, actor: *mut c_void, out: *mut *mut c_void) -> u32,
    /// SDK cache creation using `light_manager::previous_slot`, retaining the
    /// preceding miss's reserved slot. Stores the data pointer in `out`.
    pub cache_create:
        unsafe extern "C" fn(kind: u32, actor: *mut c_void, size: u32, out: *mut *mut c_void),
    /// `STAT(clock(GStat.ExtraTime))`.
    pub stat_clock: unsafe extern "C" fn(),
    /// `STAT(unclock(GStat.ExtraTime))`.
    pub stat_unclock: unsafe extern "C" fn(),
    /// C++ `LightSurfMapSetup(ctx, which)`.
    pub surf_map_setup: unsafe extern "C" fn(ctx: *mut c_void, which: u32),
    /// C++ `LightSurfZoneAmbient(out)`.
    pub surf_zone_ambient: unsafe extern "C" fn(out: *mut c_void),
    /// C++ `LightSurfCacheGet(id)`.
    pub surf_cache_get: unsafe extern "C" fn(id: u64) -> *mut c_void,
    /// C++ `LightSurfCacheCreate(id, size, pad, check)`.
    pub surf_cache_create:
        unsafe extern "C" fn(id: u64, size: u64, pad: u64, check: u32) -> *mut c_void,
    /// C++ `LightSurfGmem(bytes)`: `New<BYTE>(GMem, appCheckedIntSize(bytes))`.
    pub surf_gmem: unsafe extern "C" fn(bytes: u64) -> *mut c_void,
    /// C++ `LightSurfGmemMark(slot, pop)`.
    pub surf_gmem_mark: unsafe extern "C" fn(slot: u32, pop: u32),
    /// C++ `LightSurfLightCompute(i, which)`.
    pub surf_light_compute: unsafe extern "C" fn(i: u32, which: u32),
    /// C++ `LightSurfLightSpatialFx(i, shadow)`.
    pub surf_light_spatial_fx: unsafe extern "C" fn(i: u32, shadow: *mut c_void),
    /// C++ `LightSurfShadowGen(i, use_bits, dest)`.
    pub surf_shadow_gen: unsafe extern "C" fn(i: u32, use_bits: u32, dest: *mut c_void),
    /// `FGlobalRandomsBase::RandomBaseData()`: the `FLOAT[256]` random-base table.
    pub random_base_data: unsafe extern "C" fn() -> *const f32,
    /// `FGlobalRandomsBase::RandomData()`: the `FLOAT[256]` random table.
    pub random_data: unsafe extern "C" fn() -> *const f32,
    /// C++ `LightSvcCompute(i, frame)`: `FirstLight[i].ComputeFromActor(NULL, frame)`.
    pub light_compute: unsafe extern "C" fn(index: u32, frame: *mut c_void),
    /// C++ `LightSvcStatVtricInc()`: `STAT(GStat.MeshVtricCount++)`.
    pub stat_vtric_inc: unsafe extern "C" fn(),
    /// C++ `LightSvcStatMeshLightAdd(n)`: `STAT(GStat.MeshLightCount += n)`.
    pub stat_mesh_light_add: unsafe extern "C" fn(n: u32),
    /// Native SDK `Unlock` for the item selected by `light_manager::pop_unlock`.
    pub unlock_one: unsafe extern "C" fn(),
    /// C++ `LightSvcMarkPop()`: `FLightManager::Mark.Pop()`.
    pub mark_pop: unsafe extern "C" fn(),
}
