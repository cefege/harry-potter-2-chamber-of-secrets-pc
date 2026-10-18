//! Lighting exports (`hp2-render::light`). The C++ shells in `UnLight.cpp`
//! keep the `FLightManager`/`FLightInfo` state, the actor and level setup, the
//! `Effects[]` table, and the UnrealScript natives. They forward the table
//! builders, the shadow-map generator, the merge blend, the volumetric term,
//! and the spatial kernels here. Vectors cross as `[f32; 3]` inside `#[repr(C)]`
//! params (the `FVector` layout) and as `void*` for direct pointers (hp2-abi
//! mirror, not parsed by cbindgen).

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_ffi_support::guard;
use hp2_render::light::{
    self, BYTE_FOG_LEN, FILTER_TAB_LEN, FilterTab, GMath, LIGHT_DIST_LEN, MergeParams, Spatial,
    SpatialParams, VolumetricParams,
};
use hp2_render::services::Hp2RenderServices;

/// `FLightManager::Merge` scalar inputs: `Info->Actor->LightEffect`, the
/// random-stream start `Key`, the `FTextureInfo` strides, and the
/// `Info->MinU..MaxU` / `MinV..MaxV` window.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightMergeParams {
    /// `Info->Actor->LightEffect` (`LE_*`).
    pub effect: u8,
    /// Random-stream start index.
    pub key: i32,
    /// `Tex.UClamp`: stride of the illumination map.
    pub uclamp: i32,
    /// `Tex.USize`: stride of the stream and destination words.
    pub usize_: i32,
    pub min_u: i32,
    pub max_u: i32,
    pub min_v: i32,
    pub max_v: i32,
}

/// `FLightManager::Volumetric` inputs read from `FLightInfo`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightVolumetricParams {
    /// `Info->Location`.
    pub location: [f32; 3],
    pub vol_radius_squared: f32,
    pub rvol_radius: f32,
    pub rvol_radius_squared: f32,
    pub vol_brightness: f32,
    pub location_size_squared: f32,
    /// Nonzero when `Info->VolInside`.
    pub vol_inside: u32,
    /// Nonzero when `Info->Actor->LightSource == LD_Ambient`.
    pub ambient: u32,
}

/// `FLightInfo` and `FTextureInfo` fields read by the `spatial_*` kernels.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightSpatialParams {
    /// `Map.UClamp`.
    pub uclamp: i32,
    /// `FLightManager::ShadowMaskU`.
    pub shadow_mask_u: i32,
    /// `FLightManager::ShadowSkip`.
    pub shadow_skip: i32,
    pub min_u: i32,
    pub max_u: i32,
    pub min_v: i32,
    pub max_v: i32,
    pub radius: f32,
    pub rradius: f32,
    pub radius_inner: f32,
    pub diffuse: f32,
    /// `Info->Actor->Location`.
    pub actor_location: [f32; 3],
    /// `Info->Actor->GetViewRotation().Vector()` (Spotlight only).
    pub view: [f32; 3],
    /// `Info->Actor->LightPeriod`.
    pub light_period: u8,
    /// `Info->Actor->LightPhase`.
    pub light_phase: u8,
    /// `Info->Actor->LightCone`.
    pub light_cone: u8,
    /// `LevelInfo->TimeSeconds`.
    pub time_seconds: f32,
    /// `FLightManager::VertexBase`.
    pub vertex_base: [f32; 3],
    /// `FLightManager::VertexDU`.
    pub vertex_du: [f32; 3],
    /// `FLightManager::VertexDV`.
    pub vertex_dv: [f32; 3],
}

fn vec3(a: [f32; 3]) -> FVector {
    FVector {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}

/// Spatial kind codes for `hp2_rs_light_spatial`'s `which` argument.
fn spatial_kind(which: u32) -> Option<Spatial> {
    Some(match which {
        0 => Spatial::None,
        1 => Spatial::SearchLight,
        2 => Spatial::Rotor,
        3 => Spatial::SlowWave,
        4 => Spatial::FastWave,
        5 => Spatial::Shock,
        6 => Spatial::Disco,
        7 => Spatial::Interference,
        8 => Spatial::Spotlight,
        9 => Spatial::Cylinder,
        10 => Spatial::NonIncidence,
        11 => Spatial::Shell,
        _ => return None,
    })
}

/// `FLightManager::Init` table builders. Fills `LightDist[4096]`,
/// `LightDistInc[4096]`, `FilterTab[128][4]` (flattened to 512 words), and
/// `ByteFog[0x4000]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_light_init_tables(
    light_dist: *mut f32,
    light_dist_inc: *mut f32,
    filter_tab: *mut u32,
    byte_fog: *mut u8,
) {
    guard("hp2_rs_light_init_tables", (), move || unsafe {
        if light_dist.is_null()
            || light_dist_inc.is_null()
            || filter_tab.is_null()
            || byte_fog.is_null()
        {
            return;
        }
        light::init_light_dist(
            &mut *(light_dist as *mut [f32; LIGHT_DIST_LEN]),
            &mut *(light_dist_inc as *mut [f32; LIGHT_DIST_LEN]),
        );
        let table = light::init_filter_tab();
        let flat = &mut *(filter_tab as *mut [u32; FILTER_TAB_LEN * 4]);
        for (i, row) in table.iter().enumerate() {
            flat[i * 4..i * 4 + 4].copy_from_slice(row);
        }
        light::init_byte_fog(&mut *(byte_fog as *mut [u8; BYTE_FOG_LEN]));
    })
}

/// `FLightManager::ShadowMapGen`. `src` null takes the no-source branch and
/// fills `dest` with 0x7F bytes. `filter_tab` is `FilterTab[128][4]`
/// flattened to 512 words. `src_len` is `ShadowMaskSpace` (one byte per shadow
/// bit). `dest_words` is `ShadowMaskSpace*2` (`ShadowMaskSpace*8` bytes).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_light_shadow_map_gen(
    filter_tab: *const u32,
    shadow_mask_u: usize,
    v_clamp: usize,
    src: *const u8,
    src_len: usize,
    dest: *mut u32,
    dest_words: usize,
) {
    guard("hp2_rs_light_shadow_map_gen", (), move || unsafe {
        if filter_tab.is_null() || dest.is_null() {
            return;
        }
        let flat = &*(filter_tab as *const [u32; FILTER_TAB_LEN * 4]);
        let mut table: FilterTab = [[0; 4]; FILTER_TAB_LEN];
        for (i, row) in table.iter_mut().enumerate() {
            row.copy_from_slice(&flat[i * 4..i * 4 + 4]);
        }
        let src = if src.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts(src, src_len))
        };
        let dest = std::slice::from_raw_parts_mut(dest, dest_words);
        light::shadow_map_gen(&table, shadow_mask_u, v_clamp, src, dest);
    })
}

/// `FLightManager::Merge`. `src` is the illumination map (`UClamp*VClamp`
/// bytes), `palette` is `FColor[256]` as DWORDs, and `random_base`/`random`
/// are the `GRandoms` tables (`FLOAT[256]`). `stream` and `dest` are the
/// lightmap words (`USize` stride). When `stream == dest` the C++ call is
/// `Merge(..., Stream, Stream)` and the blend runs in place.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_light_merge(
    params: *const LightMergeParams,
    src: *const u8,
    src_len: usize,
    palette: *const u32,
    stream: *const u32,
    stream_len: usize,
    dest: *mut u32,
    dest_len: usize,
    random_base: *const f32,
    random: *const f32,
) {
    guard("hp2_rs_light_merge", (), move || unsafe {
        if params.is_null()
            || src.is_null()
            || palette.is_null()
            || stream.is_null()
            || dest.is_null()
            || random_base.is_null()
            || random.is_null()
        {
            return;
        }
        let p = *params;
        let p = MergeParams {
            effect: p.effect,
            key: p.key,
            uclamp: p.uclamp,
            usize_: p.usize_,
            min_u: p.min_u,
            max_u: p.max_u,
            min_v: p.min_v,
            max_v: p.max_v,
        };
        let in_place = std::ptr::eq(stream, dest);
        let stream = if in_place {
            None
        } else {
            Some(std::slice::from_raw_parts(stream, stream_len))
        };
        let dest = std::slice::from_raw_parts_mut(dest, dest_len);
        light::merge(
            &p,
            std::slice::from_raw_parts(src, src_len),
            &*(palette as *const [u32; 256]),
            stream,
            dest,
            &*(random_base as *const [f32; 256]),
            &*(random as *const [f32; 256]),
        );
    })
}

/// `FLightManager::Volumetric`. `vertex` is an `FVector*`. `fog_rejection`
/// is the C++ function-static `FogRejectionMethod`, read and written in place.
/// Returns the volumetric fog contribution.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_light_volumetric(
    services: *const Hp2RenderServices,
    info: *const LightVolumetricParams,
    vertex: *const c_void,
    fog_rejection: *mut i32,
) -> f32 {
    guard("hp2_rs_light_volumetric", 0.0, move || unsafe {
        if services.is_null() || info.is_null() || vertex.is_null() || fog_rejection.is_null() {
            return 0.0;
        }
        let services = *services;
        let g = GMath::new(&services);
        let info = *info;
        let params = VolumetricParams {
            location: vec3(info.location),
            vol_radius_squared: info.vol_radius_squared,
            rvol_radius: info.rvol_radius,
            rvol_radius_squared: info.rvol_radius_squared,
            vol_brightness: info.vol_brightness,
            location_size_squared: info.location_size_squared,
            vol_inside: info.vol_inside != 0,
            ambient: info.ambient != 0,
        };
        light::volumetric(
            &g,
            &params,
            *(vertex as *const FVector),
            &mut *fog_rejection,
        )
    })
}

/// `FLightManager::spatial_*` dispatch. `which` is the spatial kind:
/// 0 None, 1 SearchLight, 2 Rotor, 3 SlowWave, 4 FastWave, 5 Shock, 6 Disco,
/// 7 Interference, 8 Spotlight, 9 Cylinder, 10 NonIncidence, 11 Shell. An
/// unknown code is logged through the services sink and nothing is written.
/// `light_dist_tab` is `Info->LightDistTab`, `light_dist` is
/// `FLightManager::LightDist`. `src` is the shadow map (`ShadowMaskSpace*8`
/// bytes) and `dest` is the illumination map (`UClamp*VClamp` bytes).
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_light_spatial(
    services: *const Hp2RenderServices,
    which: u32,
    params: *const LightSpatialParams,
    light_dist_tab: *const f32,
    light_dist: *const f32,
    src: *const u8,
    src_len: usize,
    dest: *mut u8,
    dest_len: usize,
) {
    guard("hp2_rs_light_spatial", (), move || unsafe {
        if services.is_null()
            || params.is_null()
            || light_dist_tab.is_null()
            || light_dist.is_null()
            || src.is_null()
            || dest.is_null()
        {
            return;
        }
        let services = *services;
        let Some(kind) = spatial_kind(which) else {
            let msg = b"hp2_rs_light_spatial: unknown spatial kind";
            (services.log_warning)(msg.as_ptr(), msg.len());
            return;
        };
        let p = *params;
        let sp = SpatialParams {
            uclamp: p.uclamp,
            shadow_mask_u: p.shadow_mask_u,
            shadow_skip: p.shadow_skip,
            min_u: p.min_u,
            max_u: p.max_u,
            min_v: p.min_v,
            max_v: p.max_v,
            radius: p.radius,
            rradius: p.rradius,
            radius_inner: p.radius_inner,
            diffuse: p.diffuse,
            actor_location: vec3(p.actor_location),
            view: vec3(p.view),
            light_period: p.light_period,
            light_phase: p.light_phase,
            light_cone: p.light_cone,
            time_seconds: p.time_seconds,
            vertex_base: vec3(p.vertex_base),
            vertex_du: vec3(p.vertex_du),
            vertex_dv: vec3(p.vertex_dv),
        };
        let g = GMath::new(&services);
        light::spatial(
            &g,
            kind,
            &sp,
            &*(light_dist_tab as *const [f32; LIGHT_DIST_LEN]),
            &*(light_dist as *const [f32; LIGHT_DIST_LEN]),
            std::slice::from_raw_parts(src, src_len),
            std::slice::from_raw_parts_mut(dest, dest_len),
        );
    })
}
