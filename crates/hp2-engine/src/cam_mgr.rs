//! Rust port of `HarryPotter2/Unreal/Engine/Src/UnCamMgr.cpp`: FOV and
//! post-edit math, BRIGHTNESS/Flush policy, plus UClient construction,
//! non-reflection defaults, destruction, initialization, and viewport
//! serialization sequencing.
//!
//! Object access goes through [`CamMgrServices`], filled in by
//! `Hp2CamMgrServices.cpp`. Native leaves retain reflection macros, C++ object
//! fields, typed archive operations, virtual calls, guard scopes, and exception
//! transport. Rust owns the policy order and publications across those leaves.
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
    /// `UBitmap::__Client = client`.
    pub client_set_bitmap_client: unsafe extern "C" fn(client: *mut c_void) -> i32,
    /// `UBitmap::__Client = NULL`.
    pub client_clear_bitmap_client: unsafe extern "C" fn() -> i32,
    /// Assign `UClient::MaintainVerticalFOV`.
    pub client_set_maintain_vertical_fov:
        unsafe extern "C" fn(client: *mut c_void, value: i32) -> i32,
    /// Assign `UClient::NativeText`.
    pub client_set_native_text: unsafe extern "C" fn(client: *mut c_void, value: i32) -> i32,
    /// Assign `UClient::ShowFPS`.
    pub client_set_show_fps: unsafe extern "C" fn(client: *mut c_void, value: i32) -> i32,
    /// Assign `UClient::Engine`.
    pub client_set_engine: unsafe extern "C" fn(client: *mut c_void, engine: *mut c_void) -> i32,
    /// Serialize `UClient::Viewports` through the native typed archive operator.
    pub client_serialize_viewports:
        unsafe extern "C" fn(client: *mut c_void, archive: *mut c_void) -> i32,
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
    match unsafe {
        exec_with::<std::convert::Infallible>(
            client,
            brightness,
            cmd,
            |command| Ok((svc.parse_brightness_command)(command) != 0),
            |client, allow| { (svc.client_engine_flush)(client, allow); Ok(()) },
            |client| { (svc.client_save_config)(client); Ok(()) },
            |client| Ok((svc.client_viewports_num)(client)),
            |client| Ok((svc.client_viewport0_actor)(client)),
            |client, level| { (svc.client_brightness_message)(client, level); Ok(()) },
        )
    } {
        Ok(result) => result,
        Err(never) => match never {},
    }
}

/// Shared `UClient::Exec` transition with typed action adapters.
///
/// # Safety
/// `client`, `brightness`, and `command` must remain live throughout the
/// synchronous source-order callback sequence.
pub(crate) unsafe fn exec_with<E>(
    client: *mut c_void,
    brightness: *mut f32,
    command: *const TChar,
    mut parse_brightness: impl FnMut(*const TChar) -> Result<bool, E>,
    mut engine_flush: impl FnMut(*mut c_void, i32) -> Result<(), E>,
    mut save_config: impl FnMut(*mut c_void) -> Result<(), E>,
    mut viewport_count: impl FnMut(*mut c_void) -> Result<i32, E>,
    mut viewport0_actor: impl FnMut(*mut c_void) -> Result<*mut c_void, E>,
    mut brightness_message: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<i32, E> {
    unsafe {
        if !parse_brightness(command)? { return Ok(0); }
        *brightness += 0.1_f32;
        if *brightness >= 1.0_f32 { *brightness = 0.0_f32; }
        engine_flush(client, 1)?;
        save_config(client)?;
        if viewport_count(client)? != 0 && !viewport0_actor(client)?.is_null() {
            let level = app_round(f32::mul_add(*brightness, 10.0_f32, 1.0_f32));
            brightness_message(client, level)?;
        }
    }
    Ok(1)
}

/// `UClient::Flush(UBOOL AllowPrecache)` (UnCamMgr.cpp:131-140).
///
/// # Safety
/// `client` must be a live `UClient` as far as `svc` is concerned.
pub unsafe fn flush(svc: &CamMgrServices, client: *mut c_void, allow_precache: i32) {
    match unsafe {
        flush_with::<std::convert::Infallible>(
            client,
            allow_precache,
            |client| Ok((svc.client_viewports_num)(client)),
            |client, index| Ok((svc.client_viewport_render_dev)(client, index)),
            |render_dev, allow| { (svc.render_dev_flush)(render_dev, allow); Ok(()) },
        )
    } {
        Ok(()) => (),
        Err(never) => match never {},
    }
}

/// Shared `UClient::Flush` viewport traversal for Native and Core clients.
pub(crate) unsafe fn flush_with<E>(
    client: *mut c_void,
    allow_precache: i32,
    mut viewport_count: impl FnMut(*mut c_void) -> Result<i32, E>,
    mut viewport_render_device: impl FnMut(*mut c_void, i32) -> Result<*mut c_void, E>,
    mut render_flush: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    let mut index = 0;
    while index < viewport_count(client)? {
        let render_device = viewport_render_device(client, index)?;
        if !render_device.is_null() { render_flush(render_device, allow_precache)?; }
        index += 1;
    }
    Ok(())
}

type Res<T> = Result<T, i32>;

#[inline]
fn checked(status: i32) -> Res<()> {
    if status == 0 { Ok(()) } else { Err(-status) }
}

/// Execute the non-reflection `UClient` constructor work.
///
/// # Safety
/// `client` must be a live UClient and `svc` must be its matching native service table.
pub unsafe fn construct(svc: &CamMgrServices, client: *mut c_void) -> Res<()> {
    checked(unsafe { (svc.client_set_bitmap_client)(client) })
}

/// Apply non-reflection UClient class defaults in declaration order.
///
/// # Safety
/// `client` must be a live UClient and `svc` must be its matching native service table.
pub unsafe fn set_static_defaults(svc: &CamMgrServices, client: *mut c_void) -> Res<()> {
    checked(unsafe { (svc.client_set_maintain_vertical_fov)(client, 1) })?;
    checked(unsafe { (svc.client_set_native_text)(client, 1) })?;
    checked(unsafe { (svc.client_set_show_fps)(client, 0) })
}

/// Execute the non-superclass UClient destruction work.
///
/// # Safety
/// `svc` must be the matching native service table.
pub unsafe fn destroy(svc: &CamMgrServices) -> Res<()> {
    checked(unsafe { (svc.client_clear_bitmap_client)() })
}

/// Publish the engine pointer during UClient initialization.
///
/// # Safety
/// `client`, `engine`, and `svc` must remain live for the native service call.
pub unsafe fn init(svc: &CamMgrServices, client: *mut c_void, engine: *mut c_void) -> Res<()> {
    checked(unsafe { (svc.client_set_engine)(client, engine) })
}

/// Serialize the UClient viewport object array after the native superclass archive work.
///
/// # Safety
/// `client`, `archive`, and `svc` must remain live for the native service call.
pub unsafe fn serialize_viewports(
    svc: &CamMgrServices,
    client: *mut c_void,
    archive: *mut c_void,
) -> Res<()> {
    checked(unsafe { (svc.client_serialize_viewports)(client, archive) })
}
