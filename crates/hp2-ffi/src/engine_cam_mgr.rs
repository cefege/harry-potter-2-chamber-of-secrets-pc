//! `UnCamMgr.cpp` exports (`crates/hp2-engine/src/cam_mgr.rs`). The C++
//! `UClient` shells in `Engine/Src/UnCamMgr.cpp` forward here. Exec and Flush take
//! `svc`, the C++ `Hp2CamMgrServices` table. FOV and PostEditChange are pure.

use std::ffi::c_void;

use hp2_engine::cam_mgr::{self, CamMgrServices};
use hp2_engine::params::TChar;
use hp2_ffi_support::guard;

/// `UClient::GetEffectiveFovAngle(FLOAT, INT, INT, UBOOL)` forward. No `Super`, no
/// callbacks. The original has no `guard` either. The wrapper only protects against
/// panics, and this body cannot panic.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_cam_mgr_get_effective_fov_angle(
    authored_hfov: f32,
    viewport_x: i32,
    viewport_y: i32,
    maintain_vertical: i32,
) -> f32 {
    guard(
        "hp2_rs_engine_cam_mgr_get_effective_fov_angle",
        authored_hfov,
        move || {
            cam_mgr::get_effective_fov_angle(
                authored_hfov,
                viewport_x,
                viewport_y,
                maintain_vertical,
            )
        },
    )
}

/// `UClient::PostEditChange` field clamps, after the C++ `Super::PostEditChange()`.
///
/// # Safety
/// `brightness`, `mip_factor` and `density` must point to live `FLOAT`, `FLOAT` and `INT`
/// fields of one `UClient`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_cam_mgr_post_edit_change(
    brightness: *mut c_void,
    mip_factor: *mut c_void,
    density: *mut c_void,
) {
    guard(
        "hp2_rs_engine_cam_mgr_post_edit_change",
        (),
        move || unsafe {
            let (Some(brightness), Some(mip_factor), Some(density)) = (
                brightness.cast::<f32>().as_mut(),
                mip_factor.cast::<f32>().as_mut(),
                density.cast::<i32>().as_mut(),
            ) else {
                return;
            };
            cam_mgr::post_edit_change(brightness, mip_factor, density);
        },
    )
}

/// `UClient::Exec(const TCHAR* Cmd, FOutputDevice& Ar)` forward.
///
/// # Safety
/// `svc` must point to a live `CamMgrServices`. `client` must be a live `UClient`.
/// `brightness` must point to its `Brightness` field. `cmd` must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_cam_mgr_exec(
    svc: *const c_void,
    client: *mut c_void,
    brightness: *mut c_void,
    cmd: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_cam_mgr_exec", 0, move || unsafe {
        let Some(svc) = svc.cast::<CamMgrServices>().as_ref() else {
            return 0;
        };
        let Some(brightness) = brightness.cast::<f32>().as_mut() else {
            return 0;
        };
        cam_mgr::exec(svc, client, brightness, cmd.cast::<TChar>())
    })
}

/// `UClient::Flush(UBOOL AllowPrecache)` forward.
///
/// # Safety
/// `svc` must point to a live `CamMgrServices`. `client` must be a live `UClient`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_cam_mgr_flush(
    svc: *const c_void,
    client: *mut c_void,
    allow_precache: i32,
) {
    guard("hp2_rs_engine_cam_mgr_flush", (), move || unsafe {
        let Some(svc) = svc.cast::<CamMgrServices>().as_ref() else {
            return;
        };
        cam_mgr::flush(svc, client, allow_precache);
    })
}
