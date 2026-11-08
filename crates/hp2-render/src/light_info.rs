//! `FLightManager` global light types (`global_*` in `UnLight.cpp`), the control
//! flow of `FLightInfo::ComputeFromActor`, and `LightInfoDistCompare`. The C++
//! shell keeps statics, the callback table, and marshalling. Every engine read
//! or write that depends on a condition goes through `Hp2LightServices`.
//! Fused `a*b+c` sites follow `/tmp/RenLight/fma_info.md`.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FColor;
use hp2_abi::engine::generated::{AActor, ALevelInfo, UPalette, UTexture};

use crate::light::{GMath, LD_AMBIENT, LD_PLANE};
use crate::services::Hp2RenderServices;
use crate::services_light::Hp2LightServices;

/// `ELightType` (`Engine/Inc/EngineClasses.h`).
pub const LT_NONE: u32 = 0;
pub const LT_STEADY: u32 = 1;
pub const LT_PULSE: u32 = 2;
pub const LT_BLINK: u32 = 3;
pub const LT_FLICKER: u32 = 4;
pub const LT_STROBE: u32 = 5;
pub const LT_BACKDROP_LIGHT: u32 = 6;
pub const LT_SUBTLE_PULSE: u32 = 7;
pub const LT_TEXTURE_PALETTE_ONCE: u32 = 8;
pub const LT_TEXTURE_PALETTE_LOOP: u32 = 9;
pub const LT_MAX: u32 = 10;

/// `LE_MAX` (`Engine/Inc/EngineClasses.h:450`).
pub const LE_MAX: u32 = 20;
/// `SHOW_PlayerCtrl` (`Engine/Inc/UnCamera.h:181`).
pub const SHOW_PLAYER_CTRL: u32 = 0x0000_0800;

/// `sizeof(FVector) + 256*sizeof(FColor)`: lighting palette cache item.
pub const LIGHT_PALETTE_BYTES: u32 = 12 + 256 * 4;
/// `sizeof(FPlane) + 256*sizeof(FColor)`: volumetric palette cache item.
pub const VOLUMETRIC_PALETTE_BYTES: u32 = 16 + 256 * 4;
/// Byte offset of the lighting palette inside its cache item (`Color+1`, FVector).
const LIGHT_PALETTE_OFFSET: usize = 12;
/// Byte offset of the volumetric palette inside its cache item (`Color+1`, FPlane).
const VOLUMETRIC_PALETTE_OFFSET: usize = 16;

/// `FGlobalRandomsBase::RAND_MASK` (`Render/Inc/Render.h`): 256 entries.
const RAND_MASK: usize = 255;
/// `SMALL_NUMBER` (`Core/Inc/UnMath.h`).
const SMALL_NUMBER: f32 = 1.0e-8;

// `UnTemplate.h` forms of Max/Min/Abs/Clamp. `appRound`/`appFloor` are
// `static_cast<INT>` on the non-PSX path (truncation).

#[inline]
fn app_round(f: f32) -> i32 {
    f as i32
}

#[inline]
fn app_floor(f: f32) -> i32 {
    f as i32
}

#[inline]
fn max_i(a: i32, b: i32) -> i32 {
    if a >= b { a } else { b }
}

#[inline]
fn min_i(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

#[inline]
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

#[inline]
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `Clamp(X, 0.0f, 1.0f)`: `X<Min ? Min : X<Max ? X : Max`.
#[inline]
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x < 1.0 {
        x
    } else {
        1.0
    }
}

/// `appSqrt(DOUBLE)` on a FLOAT argument, returned as FLOAT.
#[inline]
fn sqrt_plain(f: f32) -> f32 {
    (f as f64).sqrt() as f32
}

/// `FVector | FVector` (UnMath.h:688). The chain is `z*z + (x*x + y*y)` with
/// the two outer products fused and `y*y` unfused.
#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[2].mul_add(b[2], a[0].mul_add(b[0], a[1] * b[1]))
}

#[inline]
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn mul_s(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `FVector::UnsafeNormal` (UnMath.h:827-830). `1.f/appSqrt(sum)` divides in
/// double, then the scale is rounded to FLOAT.
fn unsafe_normal(v: [f32; 3]) -> [f32; 3] {
    let scale = (1.0f64 / (dot(v, v) as f64).sqrt()) as f32;
    mul_s(v, scale)
}

/// `FVector::SafeNormal` (UnMath.cpp:259-268). Returns zero below `SMALL_NUMBER`.
/// `appSqrt` narrows to FLOAT before `1.f/Size`, and the double sqrt rounded to
/// FLOAT equals the FLOAT sqrt, so the scale is computed in f32.
fn safe_normal(v: [f32; 3]) -> [f32; 3] {
    let sum = dot(v, v);
    if sum < SMALL_NUMBER {
        return [0.0; 3];
    }
    mul_s(v, 1.0f32 / sum.sqrt())
}

/// `FColor::FBrightness` (UnTex.h:94): `(2.f*R + 3.f*G + 1.f*B) / (6.f*256.f)`.
/// `2R + 3G` fuses as `fma(3, G, 2R)`.
#[inline]
fn fbrightness(r: u8, g: u8, b: u8) -> f32 {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    (3.0 * g + 2.0 * r + b) / (6.0 * 256.0)
}

#[inline]
fn period_max(period: u32) -> i32 {
    max_i(period as i32, 1)
}

/// `FVector` (`#[repr(C)]` triple) with `FCoords` layout as four of them.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LightCoords {
    pub origin: [f32; 3],
    pub x_axis: [f32; 3],
    pub y_axis: [f32; 3],
    pub z_axis: [f32; 3],
}

#[inline]
fn write_xyz(dst: &mut [f32; 4], v: [f32; 3]) {
    dst[0] = v[0];
    dst[1] = v[1];
    dst[2] = v[2];
}

/// `Owner->Level->TimeSeconds * 35.0f * 65536.0f) / Max((INT)Period,1) + (Phase<<8)`.
fn pulse_time(ts: f32, period: u32, phase: u32) -> f32 {
    (ts * 35.0 * 65536.0) / (period_max(period) as f32) + ((phase << 8) as f32)
}

/// Mirror of C++ `Hp2LightGlobalParams` (#[repr(C)], 64 bytes). Order and types must match.
/// `brightness`, `color`, `strobe_last`, and `strobe_toggle` are in/out.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GlobalParams {
    /// `AActor*` (`Owner`). Callbacks read Level time and Skin/palette through it.
    pub owner: *mut c_void,
    /// Nonzero when `GlobalLighting` was called with `Realtime`. Otherwise the type is `LT_STEADY`.
    pub realtime: u32,
    /// `Owner->LightType`.
    pub light_type: u32,
    /// `Owner->LightPeriod`.
    pub light_period: u32,
    /// `Owner->LightPhase`.
    pub light_phase: u32,
    /// `Owner->LightHue`.
    pub light_hue: u32,
    /// `Owner->LightSaturation`.
    pub light_saturation: u32,
    /// `Brightness` in/out.
    pub brightness: f32,
    /// `Color` (`FPlane`) in/out. Written by `global_hsv`, then XYZ by palette types.
    pub color: [f32; 4],
    /// Strobe `LastUpdateTime` (file-scope static in the shell).
    pub strobe_last: f32,
    /// Strobe `Toggle` (file-scope static in the shell).
    pub strobe_toggle: i32,
}

/// Typed engine leaves for shared global and per-actor light computation.
pub trait ComputeOps {
    type Error;
    fn global_hsv(&mut self, hue: u8, saturation: u8, out: &mut [f32; 4]) -> Result<(), Self::Error>;
    fn owner_time_seconds(&mut self, owner: *mut c_void) -> Result<f32, Self::Error>;
    fn owner_life_fraction(&mut self, owner: *mut c_void) -> Result<f32, Self::Error>;
    fn owner_palette(&mut self, owner: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn palette_color(&mut self, palette: *mut c_void, index: i32) -> Result<(u8, u8, u8), Self::Error>;
    fn world_light_radius(&mut self, actor: *mut c_void) -> Result<f32, Self::Error>;
    fn copy_effect(&mut self, effect: u32, out: *mut c_void) -> Result<(), Self::Error>;
    fn view_vector(&mut self, actor: *mut c_void) -> Result<[f32; 3], Self::Error>;
    fn stat_clock(&mut self) -> Result<(), Self::Error>;
    fn stat_unclock(&mut self) -> Result<(), Self::Error>;
    fn cache_get(&mut self, id: u32, actor: *mut c_void) -> Result<Option<*mut c_void>, Self::Error>;
    fn cache_create(&mut self, id: u32, actor: *mut c_void, size: u32) -> Result<*mut c_void, Self::Error>;
    fn map_uncoords(&mut self, coords: &LightCoords) -> Result<LightCoords, Self::Error>;
    fn world_volumetric_radius(&mut self, actor: *mut c_void) -> Result<f32, Self::Error>;
}

/// Typed shared `GlobalLighting` policy.
pub fn global_with_ops<O: ComputeOps>(
    gm: &GMath<'_>,
    ops: &mut O,
    random_base: &[f32; 256],
    p: &mut GlobalParams,
) -> Result<(), O::Error> {
    ops.global_hsv(p.light_hue as u8, p.light_saturation as u8, &mut p.color)?;
    let ty = if p.realtime != 0 { p.light_type } else { LT_STEADY };
    if ty < LT_MAX {
        match ty {
            LT_NONE => p.brightness = 0.0,
            LT_STEADY | LT_BACKDROP_LIGHT => {}
            LT_PULSE => {
                let t = ops.owner_time_seconds(p.owner)?;
                let sin = gm.sin_tab(app_round(pulse_time(t, p.light_period, p.light_phase)));
                p.brightness *= 0.39f32.mul_add(sin, 0.6);
            }
            LT_BLINK => {
                let t = ops.owner_time_seconds(p.owner)?;
                let t = (t * 35.0 * 65536.0) / ((p.light_period + 1) as f32)
                    + ((p.light_phase << 8) as f32);
                if app_round(t) & 1 != 0 { p.brightness = 0.0; }
            }
            LT_FLICKER => {
                let random = random_base[(p.owner as usize) & RAND_MASK];
                if random < 0.5 { p.brightness = 0.0; } else { p.brightness *= random; }
            }
            LT_STROBE => {
                let t = ops.owner_time_seconds(p.owner)?;
                if p.strobe_last != t {
                    p.strobe_last = t;
                    p.strobe_toggle ^= 1;
                }
                if p.strobe_toggle != 0 { p.brightness = 0.0; }
            }
            LT_SUBTLE_PULSE => {
                let t = ops.owner_time_seconds(p.owner)?;
                let sin = gm.sin_tab(pulse_time(t, p.light_period, p.light_phase) as i32);
                p.brightness *= 0.09f32.mul_add(sin, 0.9);
            }
            LT_TEXTURE_PALETTE_ONCE => {
                let palette = ops.owner_palette(p.owner)?;
                if !palette.is_null() {
                    let index = app_floor(255.0f32 * ops.owner_life_fraction(p.owner)?);
                    let (r, g, b) = ops.palette_color(palette, index)?;
                    write_xyz(&mut p.color, safe_normal([r as f32, g as f32, b as f32]));
                    p.brightness *= fbrightness(r, g, b) * 2.8f32;
                }
            }
            LT_TEXTURE_PALETTE_LOOP => {
                let palette = ops.owner_palette(p.owner)?;
                if !palette.is_null() {
                    let t = ops.owner_time_seconds(p.owner)?;
                    let time = t * 35.0 / (period_max(p.light_period) as f32) + (p.light_phase as f32);
                    let index = (app_round(time * 256.0) & 255) % 255;
                    let (r, g, b) = ops.palette_color(palette, index)?;
                    write_xyz(&mut p.color, unsafe_normal([r as f32, g as f32, b as f32]));
                    p.brightness *= fbrightness(r, g, b) * 2.8f32;
                }
            }
            _ => {}
        }
    }
    p.brightness = clamp01(p.brightness);
    Ok(())
}

/// Native adapter retained for the SDK callback ABI.
pub fn global(
    render: &Hp2RenderServices,
    light: &Hp2LightServices,
    random_base: &[f32; 256],
    p: &mut GlobalParams,
) {
    let mut ops = NativeComputeOps { render, light };
    let gm = GMath::new(render);
    match global_with_ops(&gm, &mut ops, random_base, p) {
        Ok(()) => (),
        Err(never) => match never {},
    }
}

/// Mirror of C++ `Hp2LightComputeParams` (#[repr(C)], 536 bytes). Order and types must match.
///
/// The shell copies every `FLightInfo`, `FLightManager`, and `FTextureInfo`
/// field that `ComputeFromActor` reads into the inputs, and copies every field in
/// the "in/out" group back afterward. The shell reads `Map`-derived fields only
/// when `Map` is non-null (`has_map`). Fields that are written only under a
/// condition are copied in first, so an unconditional copy-back is a no-op for
/// them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ComputeParams {
    // Inputs.
    /// `AActor*` (`Actor`).
    pub actor: *mut c_void,
    /// `Actor->Location`.
    pub actor_location: [f32; 3],
    /// `(FLOAT)Actor->LightRadiusInner`.
    pub light_radius_inner: f32,
    /// `(FLOAT)Actor->LightBrightness`.
    pub light_brightness: f32,
    /// `Actor->Level->Brightness`.
    pub level_brightness: f32,
    /// `Actor->LightSource` (`LD_*`).
    pub light_source: u32,
    /// `Actor->LightEffect`.
    pub light_effect: u32,
    /// `Actor->LightType`.
    pub light_type: u32,
    /// `Actor->LightPeriod`.
    pub light_period: u32,
    /// `Actor->LightPhase`.
    pub light_phase: u32,
    /// `Actor->LightHue`.
    pub light_hue: u32,
    /// `Actor->LightSaturation`.
    pub light_saturation: u32,
    /// `(FLOAT)Actor->VolumeFog`.
    pub volume_fog: f32,
    /// `(FLOAT)Actor->VolumeBrightness`.
    pub volume_brightness: f32,
    /// `Actor->bLightChanged` (nonzero when set).
    pub light_changed: u32,
    /// `Frame->Viewport->Actor->ShowFlags`.
    pub show_flags: u32,
    /// `IsVolumetric` (nonzero when set).
    pub is_volumetric: u32,
    /// `Frame->Coords`.
    pub frame_coords: LightCoords,
    /// Nonzero when `Map` is non-null. Gates the surface block.
    pub has_map: u32,
    /// `FLightManager::MapCoords` (valid when `has_map`).
    pub map_coords: LightCoords,
    /// `Map->Pan.X` (valid when `has_map`).
    pub map_pan_x: f32,
    /// `Map->Pan.Y` (valid when `has_map`).
    pub map_pan_y: f32,
    /// `Map->UScale` (valid when `has_map`).
    pub map_uscale: f32,
    /// `Map->VScale` (valid when `has_map`).
    pub map_vscale: f32,
    /// `FLightManager::LightMap.Pan.X`.
    pub light_pan_x: f32,
    /// `FLightManager::LightMap.Pan.Y`.
    pub light_pan_y: f32,
    /// `FLightManager::LightMap.UScale`.
    pub light_uscale: f32,
    /// `FLightManager::LightMap.VScale`.
    pub light_vscale: f32,
    /// `FLightManager::LightMap.UClamp`.
    pub light_uclamp: i32,
    /// `FLightManager::LightMap.VClamp`.
    pub light_vclamp: i32,
    /// `FLightManager::LightDist` (table pointer, not dereferenced by Rust).
    pub light_dist: *const c_void,
    /// `FLightManager::LightDistInc` (table pointer, not dereferenced by Rust).
    pub light_dist_inc: *const c_void,
    /// `CID_LightPalette`.
    pub cid_palette: u32,
    /// `CID_VolumetricScaler`.
    pub cid_volumetric: u32,
    /// `&FLightInfo::Effect` (written by `light_manager::copy_effect`).
    pub effect_out: *mut c_void,

    // In/out: `FLightManager` statics.
    /// `FLightManager::TemporaryTablesBuilt`.
    pub temporary_tables_built: u32,
    /// `FLightManager::MapUncoords`.
    pub map_uncoords: LightCoords,
    /// `FLightManager::VertexBase`.
    pub vertex_base: [f32; 3],
    /// `FLightManager::VertexDU`.
    pub vertex_du: [f32; 3],
    /// `FLightManager::VertexDV`.
    pub vertex_dv: [f32; 3],
    /// Strobe `LastUpdateTime` (file-scope static in the shell).
    pub strobe_last: f32,
    /// Strobe `Toggle` (file-scope static in the shell).
    pub strobe_toggle: i32,
    /// Byte offsets of `FColor` B, G, R, A.
    pub palette_offsets: [u32; 4],

    // Outputs and in/out: `FLightInfo` members.
    pub radius: f32,
    pub radius_inner: f32,
    pub rradius: f32,
    pub rradius_mult: f32,
    /// `Location`.
    pub location: [f32; 3],
    /// `Brightness`.
    pub brightness: f32,
    /// `FloatColor` (`FPlane`).
    pub float_color: [f32; 4],
    /// `MaxSpecDir` (zeroed).
    pub max_spec_dir: [f32; 3],
    /// `LightDir`.
    pub light_dir: [f32; 3],
    /// `Diffuse`.
    pub diffuse: f32,
    /// `LightDistTab` (selected table pointer).
    pub light_dist_tab: *mut c_void,
    pub min_u: i32,
    pub max_u: i32,
    pub min_v: i32,
    pub max_v: i32,
    /// `Palette` (`FColor*`).
    pub palette: *mut c_void,
    /// `VolPalette` (`FColor*`).
    pub vol_palette: *mut c_void,
    /// `VolumetricColor` (`FPlane`).
    pub volumetric_color: [f32; 4],
    pub vol_radius: f32,
    pub vol_radius_squared: f32,
    pub rvol_radius: f32,
    pub rvol_radius_squared: f32,
    pub location_size_squared: f32,
    pub vol_brightness: f32,
    /// `VolInside` (nonzero when set).
    pub vol_inside: u32,
}

/// Mirror of C++ `Hp2LightPaletteParams` (#[repr(C)], 36 bytes). Order and types must match.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PaletteParams {
    /// `FloatColor` (lighting) or `VolumetricColor` (volumetric); X, Y, Z, W.
    pub color: [f32; 4],
    /// Byte offsets of B, G, R, A inside `FColor`.
    pub offsets: [u32; 4],
    /// Nonzero: A = min(unfix(W), 127). Zero: A = 255.
    pub volumetric: u32,
}

const _: () = assert!(std::mem::size_of::<PaletteParams>() == 36);
const _: () = assert!(std::mem::size_of::<GlobalParams>() == 64);
const _: () = assert!(std::mem::size_of::<ComputeParams>() == 536);

/// Fill the 256-entry `FColor` palette at `dst`. Channel X feeds B, Y feeds G,
/// and Z feeds R. The oracle pairs them this way (`Palette[i].B` from `FixR`,
/// which is driven by X). The pairing is kept on purpose.
///
/// # Safety
/// `dst` must point to 256 * 4 writable bytes.
unsafe fn fill_palette(p: &PaletteParams, dst: *mut u8) {
    let c = p.color;
    let fix_d = [
        app_floor(c[0] * 65536.0),
        app_floor(c[1] * 65536.0),
        app_floor(c[2] * 65536.0),
        app_floor(c[3] * 65536.0),
    ];
    let mut fix = [0i32; 4];
    for i in 0..256usize {
        let entry = unsafe { dst.add(i * 4) };
        let b = min_i(fix[0] >> 16, 127) as u8;
        let g = min_i(fix[1] >> 16, 127) as u8;
        let r = min_i(fix[2] >> 16, 127) as u8;
        let a = if p.volumetric != 0 {
            min_i(fix[3] >> 16, 127) as u8
        } else {
            255
        };
        unsafe {
            *entry.add(p.offsets[0] as usize) = b;
            *entry.add(p.offsets[1] as usize) = g;
            *entry.add(p.offsets[2] as usize) = r;
            *entry.add(p.offsets[3] as usize) = a;
        }
        for k in 0..4 {
            fix[k] = fix[k].wrapping_add(fix_d[k]);
        }
    }
}

/// `FLightInfo::ComputeFromActor` General block: radius terms, transformed
/// location, base brightness, the zeroed `MaxSpecDir`, the `Effect` copy, and
/// `GlobalLighting` (with the `FloatColor` scale), then `LightDir`.
fn general<O: ComputeOps>(
    gm: &GMath<'_>,
    ops: &mut O,
    random_base: &[f32; 256],
    p: &mut ComputeParams,
) -> Result<(), O::Error> {
    p.radius = ops.world_light_radius(p.actor)?;
    p.radius_inner = p.radius * p.light_radius_inner / 256.0;
    p.rradius = 1.0 / max_f(1.0, p.radius);
    p.rradius_mult = 1.0 / max_f(1.0, p.radius - p.radius_inner);
    let temp = sub(p.actor_location, p.frame_coords.origin);
    p.location = [dot(temp, p.frame_coords.x_axis), dot(temp, p.frame_coords.y_axis), dot(temp, p.frame_coords.z_axis)];
    p.brightness = p.light_brightness / 255.0;
    p.max_spec_dir = [0.0; 3];
    ops.copy_effect(if p.light_effect < LE_MAX { p.light_effect } else { 0 }, p.effect_out)?;
    let mut global = GlobalParams {
        owner: p.actor,
        realtime: ((p.show_flags & SHOW_PLAYER_CTRL) != 0) as u32,
        light_type: p.light_type,
        light_period: p.light_period,
        light_phase: p.light_phase,
        light_hue: p.light_hue,
        light_saturation: p.light_saturation,
        brightness: p.brightness,
        color: p.float_color,
        strobe_last: p.strobe_last,
        strobe_toggle: p.strobe_toggle,
    };
    global_with_ops(gm, ops, random_base, &mut global)?;
    p.brightness = global.brightness;
    p.float_color = global.color;
    p.strobe_last = global.strobe_last;
    p.strobe_toggle = global.strobe_toggle;
    let scale = p.brightness * p.level_brightness;
    for c in &mut p.float_color { *c *= scale; }
    p.light_dir = if p.light_source == LD_PLANE { ops.view_vector(p.actor)? } else { [0.0; 3] };
    Ok(())
}
fn lighting_palette<O: ComputeOps>(ops: &mut O, p: &mut ComputeParams) -> Result<(), O::Error> {
    ops.stat_clock()?;
    let cached = ops.cache_get(p.cid_palette, p.actor)?;
    let color = match cached {
        Some(color) => color,
        None => ops.cache_create(p.cid_palette, p.actor, LIGHT_PALETTE_BYTES)?,
    };
    let fc = p.float_color;
    let need = match cached {
        None => true,
        Some(_) => {
            let c = unsafe { *(color as *const [f32; 3]) };
            c[0] != fc[0] || c[1] != fc[1] || c[2] != fc[2] || p.light_changed != 0
        }
    };
    if need {
        unsafe { *(color as *mut [f32; 3]) = [fc[0], fc[1], fc[2]] };
        let dst = unsafe { (color as *mut u8).add(LIGHT_PALETTE_OFFSET) };
        let pp = PaletteParams { color: fc, offsets: p.palette_offsets, volumetric: 0 };
        unsafe { fill_palette(&pp, dst) };
    }
    p.palette = unsafe { (color as *mut u8).add(LIGHT_PALETTE_OFFSET) }.cast();
    ops.stat_unclock()?;
    Ok(())
}
fn volumetric_palette<O: ComputeOps>(ops: &mut O, p: &mut ComputeParams) -> Result<(), O::Error> {
    ops.stat_clock()?;
    let cached = ops.cache_get(p.cid_volumetric, p.actor)?;
    let color = match cached {
        Some(color) => color,
        None => ops.cache_create(p.cid_volumetric, p.actor, VOLUMETRIC_PALETTE_BYTES)?,
    };
    let vc = p.volumetric_color;
    let need = match cached {
        None => true,
        Some(_) => {
            let c = unsafe { *(color as *const [f32; 4]) };
            c[0] != vc[0] || c[1] != vc[1] || c[2] != vc[2] || c[3] != vc[3] || p.light_changed != 0
        }
    };
    if need {
        unsafe { *(color as *mut [f32; 4]) = vc };
        let dst = unsafe { (color as *mut u8).add(VOLUMETRIC_PALETTE_OFFSET) };
        let pp = PaletteParams { color: vc, offsets: p.palette_offsets, volumetric: 1 };
        unsafe { fill_palette(&pp, dst) };
    }
    p.vol_palette = unsafe { (color as *mut u8).add(VOLUMETRIC_PALETTE_OFFSET) }.cast();
    ops.stat_unclock()?;
    Ok(())
}
fn surface<O: ComputeOps>(ops: &mut O, p: &mut ComputeParams) -> Result<(), O::Error> {
    if p.temporary_tables_built == 0 {
        p.temporary_tables_built = 1;
        p.map_uncoords = ops.map_uncoords(&p.map_coords)?;
        let ux = p.map_uncoords.x_axis;
        let uy = p.map_uncoords.y_axis;
        p.vertex_base = add(add(p.map_coords.origin, mul_s(ux, p.map_pan_x)), mul_s(uy, p.map_pan_y));
        p.vertex_du = mul_s(ux, p.map_uscale);
        p.vertex_dv = mul_s(uy, p.map_vscale);
    }
    let origin = p.map_coords.origin;
    let x_axis = p.map_coords.x_axis;
    let y_axis = p.map_coords.y_axis;
    let z_axis = p.map_coords.z_axis;
    let rel = sub(p.actor_location, origin);
    let (diffuse, dist) = match p.light_source {
        LD_PLANE => (max_f(-dot(p.light_dir, z_axis), 0.0), p.light_dist),
        LD_AMBIENT => (1.0, p.light_dist),
        _ => (abs_f(dot(rel, z_axis)) * p.rradius, p.light_dist_inc),
    };
    p.diffuse = diffuse;
    p.light_dist_tab = dist as *mut c_void;
    lighting_palette(ops, p)?;
    let plane_dot = dot(rel, z_axis);
    let plane_radius = sqrt_plain(max_f((p.radius * p.radius).mul_add(1.05, -(plane_dot * plane_dot)), 0.0));
    let center = [
        p.actor_location[0] - z_axis[0] * plane_dot,
        p.actor_location[1] - z_axis[1] * plane_dot,
        p.actor_location[2] - z_axis[2] * plane_dot,
    ];
    let cdiff = sub(center, origin);
    let center_u = dot(cdiff, x_axis) - p.light_pan_x;
    let center_v = dot(cdiff, y_axis) - p.light_pan_y;
    let radius_u = plane_radius * sqrt_plain(dot(x_axis, x_axis));
    let radius_v = plane_radius * sqrt_plain(dot(y_axis, y_axis));
    p.min_u = max_i(app_round((center_u - radius_u) / p.light_uscale), 0);
    p.min_v = max_i(app_round((center_v - radius_v) / p.light_vscale), 0);
    p.max_u = min_i(app_round((center_u + radius_u) / p.light_uscale), p.light_uclamp);
    p.max_v = min_i(app_round((center_v + radius_v) / p.light_vscale), p.light_vclamp);
    Ok(())
}
/// Shared `FLightInfo::ComputeFromActor` policy with typed Native/Core leaves.
pub fn compute_from_actor_with_ops<O: ComputeOps>(
    gm: &GMath<'_>,
    ops: &mut O,
    random_base: &[f32; 256],
    p: &mut ComputeParams,
) -> Result<(), O::Error> {
    general(gm, ops, random_base, p)?;
    if p.has_map != 0 { surface(ops, p)?; }
    if p.is_volumetric != 0 {
        let mut vc = p.float_color;
        vc[3] = p.volume_fog * (1.0 / 255.0);
        p.volumetric_color = vc;
        volumetric_palette(ops, p)?;
        p.vol_radius = ops.world_volumetric_radius(p.actor)?;
        p.vol_radius_squared = p.vol_radius * p.vol_radius;
        p.rvol_radius = 1.0 / p.vol_radius;
        p.rvol_radius_squared = p.rvol_radius * p.rvol_radius;
        p.location_size_squared = dot(p.location, p.location);
        p.vol_brightness = p.brightness * p.volume_brightness / 64.0;
        p.vol_inside = (p.location_size_squared < p.vol_radius_squared) as u32;
    }
    Ok(())
}

struct NativeComputeOps<'a> {
    render: &'a Hp2RenderServices,
    light: &'a Hp2LightServices,
}

impl ComputeOps for NativeComputeOps<'_> {
    type Error = std::convert::Infallible;
    fn global_hsv(&mut self, hue: u8, saturation: u8, out: &mut [f32; 4]) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.light.global_hsv)(hue, saturation, out.as_mut_ptr().cast())) };
        Ok(())
    }
    fn owner_time_seconds(&mut self, owner: *mut c_void) -> Result<f32, Self::Error> {
        unsafe {
            let level = (*(owner as *const AActor)).level;
            Ok((*(level as *const ALevelInfo)).time_seconds)
        }
    }
    fn owner_life_fraction(&mut self, owner: *mut c_void) -> Result<f32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.light.owner_life_fraction)(owner)) })
    }
    fn owner_palette(&mut self, owner: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe {
            let skin = (*(owner as *const AActor)).skin;
            Ok(if skin.is_null() { null_mut() } else { (*(skin as *const UTexture)).base.palette })
        }
    }
    fn palette_color(&mut self, palette: *mut c_void, index: i32) -> Result<(u8, u8, u8), Self::Error> {
        unsafe {
            let colors = &(*(palette as *const UPalette)).colors;
            let c = &*(colors.data as *const FColor).add(index as usize);
            Ok((c.r, c.g, c.b))
        }
    }
    fn world_light_radius(&mut self, actor: *mut c_void) -> Result<f32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.light.world_light_radius)(actor)) })
    }
    fn copy_effect(&mut self, effect: u32, out: *mut c_void) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::copy_effect(effect, out); }
        Ok(())
    }
    fn view_vector(&mut self, actor: *mut c_void) -> Result<[f32; 3], Self::Error> {
        let mut out = [0.0; 3];
        unsafe { crate::light_manager::callback(|| (self.light.view_vector)(actor, out.as_mut_ptr().cast())) };
        Ok(out)
    }
    fn stat_clock(&mut self) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.light.stat_clock)()); }
        Ok(())
    }
    fn stat_unclock(&mut self) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.light.stat_unclock)()); }
        Ok(())
    }
    fn cache_get(&mut self, id: u32, actor: *mut c_void) -> Result<Option<*mut c_void>, Self::Error> {
        let mut color = null_mut();
        let found = unsafe { crate::light_manager::callback(|| (self.light.cache_get)(id, actor, &mut color)) };
        Ok((found != 0).then_some(color))
    }
    fn cache_create(&mut self, id: u32, actor: *mut c_void, size: u32) -> Result<*mut c_void, Self::Error> {
        let mut color = null_mut();
        unsafe { crate::light_manager::callback(|| (self.light.cache_create)(id, actor, size, &mut color)); }
        Ok(color)
    }
    fn map_uncoords(&mut self, coords: &LightCoords) -> Result<LightCoords, Self::Error> {
        let mut out = LightCoords::default();
        unsafe {
            crate::light_manager::callback(|| {
                (self.light.map_uncoords)(
                    std::ptr::from_ref(coords).cast(),
                    std::ptr::from_mut(&mut out).cast(),
                )
            });
        }
        Ok(out)
    }
    fn world_volumetric_radius(&mut self, actor: *mut c_void) -> Result<f32, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.light.world_volumetric_radius)(actor)) })
    }
}

pub fn compute_from_actor(
    render: &Hp2RenderServices,
    light: &Hp2LightServices,
    random_base: &[f32; 256],
    p: &mut ComputeParams,
) {
    let mut ops = NativeComputeOps { render, light };
    let gm = GMath::new(render);
    match compute_from_actor_with_ops(&gm, &mut ops, random_base, p) {
        Ok(()) => (),
        Err(never) => match never {},
    }
}


/// `LightInfoDistCompare`: `2*(A->Location.Z < B->Location.Z) - 1`.
pub fn dist_compare(a: [f32; 3], b: [f32; 3]) -> i32 {
    2 * ((a[2] < b[2]) as i32) - 1
}
