//! Rust port of the logic in `HarryPotter2/Unreal/Engine/Src/UnCamMgr.cpp` (pinned
//! `5782e7f`): `UClient::GetEffectiveFovAngle`, `UClient::PostEditChange`,
//! `UClient::Exec` (BRIGHTNESS command) and `UClient::Flush`.
//!
//! Object access goes through [`CamMgrServices`], filled in by `Hp2CamMgrServices.cpp`.
//! Those thunks own the C++-only work: `ParseCommand` with the literal "BRIGHTNESS",
//! `UEngine::Flush`, `SaveConfig`, the `Viewports` array, `FString::Printf` with
//! `eventClientMessage`, and the `URenderDevice::Flush` virtual call. `Super::PostEditChange`,
//! `guard`/`unguard`, the constructors, `Destroy`, `Init`, `Serialize`,
//! `StaticConstructor` and `IMPLEMENT_CLASS` stay in the C++ shell.
//!
//! Divergences, all intentional:
//! - `GetEffectiveFovAngle` uses libm `tan`/`atan` through `f64`, as `appTan`/`appAtan` do.
//! - FMA: the only fused site is `appRound(Brightness*10+1)` in `Exec`, which the original
//!   compiles to `fmadd` (`UnCamMgr.o`, -O3 -g). It is `f32::mul_add`.

use std::ffi::c_void;

use crate::math::app_round;
use crate::params::TChar;

/// Unreal `PI` (UnMath.h, `3.1415926535897932`) as a float. It rounds to the same
/// f32 bits (`0x40490FDB`) as `std::f32::consts::PI`.
const PI_F32: f32 = std::f32::consts::PI;

/// C++ services (`Hp2CamMgrServices` in `Hp2CamMgrServices.cpp`), same field order.
#[repr(C)]
pub struct CamMgrServices {
    /// `ParseCommand(&Cmd, TEXT("BRIGHTNESS"))` (nonzero when the command matches).
    pub parse_brightness_command: unsafe extern "C" fn(cmd: *const TChar) -> i32,
    /// `UClient::Engine->Flush(allow)` (virtual).
    pub client_engine_flush: unsafe extern "C" fn(client: *mut c_void, allow: i32),
    /// `UClient::SaveConfig()`.
    pub client_save_config: unsafe extern "C" fn(client: *mut c_void),
    /// `UClient::Viewports.Num()`.
    pub client_viewports_num: unsafe extern "C" fn(client: *mut c_void) -> i32,
    /// `UClient::Viewports(0)->Actor` (APlayerPawn*). Only called when `Viewports.Num()` is nonzero.
    pub client_viewport0_actor: unsafe extern "C" fn(client: *mut c_void) -> *mut c_void,
    /// `Viewports(0)->Actor->eventClientMessage(FString::Printf("Brightness level %i/10", level), NAME_None, 0)`.
    pub client_brightness_message: unsafe extern "C" fn(client: *mut c_void, level: i32),
    /// `Viewports(index)->RenDev` (URenderDevice*, may be null).
    pub client_viewport_render_dev:
        unsafe extern "C" fn(client: *mut c_void, index: i32) -> *mut c_void,
    /// `URenderDevice::Flush(allow)` (virtual).
    pub render_dev_flush: unsafe extern "C" fn(render_dev: *mut c_void, allow: i32),
}

/// `UClient::GetEffectiveFovAngle` (UnCamMgr.cpp:37-45).
pub fn get_effective_fov_angle(
    authored_hfov: f32,
    vx: i32,
    vy: i32,
    maintain_vertical: i32,
) -> f32 {
    if maintain_vertical == 0 || vx <= 0 || vy <= 0 || 3.0 * f64::from(vx) <= 4.0 * f64::from(vy) {
        return authored_hfov;
    }
    let aspect_scale = (3.0 * f64::from(vx)) / (4.0 * f64::from(vy));
    // `AuthoredHFov * PI` is float*float; `/360.0` then promotes to double.
    let half = f64::from(authored_hfov * PI_F32) / 360.0;
    let effective = (2.0 * (half.tan() * aspect_scale).atan() * 180.0 / f64::from(PI_F32)) as f32;
    // Unreal `Min(A, B)` = `(A <= B) ? A : B`.
    if effective <= 169.999_f32 {
        effective
    } else {
        169.999_f32
    }
}

/// `Clamp(X, Min, Max)` (UnTemplate.h): `X < Min ? Min : X < Max ? X : Max`.
fn clamp_f32(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// `UClient::PostEditChange` field clamps (UnCamMgr.cpp:30-32).
///
/// # Safety
/// Each pointer must be valid for reads and writes.
pub unsafe fn post_edit_change(brightness: *mut f32, mip_factor: *mut f32, density: *mut i32) {
    unsafe {
        *brightness = clamp_f32(*brightness, 0.0, 1.0);
        *mip_factor = clamp_f32(*mip_factor, -3.0, 3.0);
        // `Max(ParticleDensity, 0)` = `(A >= B) ? A : B`.
        *density = if *density >= 0 { *density } else { 0 };
    }
}

/// `UClient::Exec` (UnCamMgr.cpp:101-116). Returns `UBOOL`.
///
/// # Safety
/// `client` must be a live `UClient` as far as `svc` is concerned. `brightness` must point
/// to the client's `Brightness` field. `cmd` must be NUL-terminated.
pub unsafe fn exec(
    svc: &CamMgrServices,
    client: *mut c_void,
    brightness: *mut f32,
    cmd: *const TChar,
) -> i32 {
    unsafe {
        if (svc.parse_brightness_command)(cmd) == 0 {
            return 0;
        }
        *brightness += 0.1_f32;
        if *brightness >= 1.0_f32 {
            *brightness = 0.0_f32;
        }
        (svc.client_engine_flush)(client, 1);
        (svc.client_save_config)(client);
        if (svc.client_viewports_num)(client) != 0
            && !(svc.client_viewport0_actor)(client).is_null()
        {
            // `appRound(Brightness*10+1)`: one fused multiply-add in the original object.
            let level = app_round(f32::mul_add(*brightness, 10.0_f32, 1.0_f32));
            (svc.client_brightness_message)(client, level);
        }
        1
    }
}

/// `UClient::Flush(UBOOL AllowPrecache)` (UnCamMgr.cpp:131-140).
///
/// # Safety
/// `client` must be a live `UClient` as far as `svc` is concerned.
pub unsafe fn flush(svc: &CamMgrServices, client: *mut c_void, allow_precache: i32) {
    unsafe {
        let mut i = 0;
        while i < (svc.client_viewports_num)(client) {
            let render_dev = (svc.client_viewport_render_dev)(client, i);
            if !render_dev.is_null() {
                (svc.render_dev_flush)(render_dev, allow_precache);
            }
            i += 1;
        }
    }
}
