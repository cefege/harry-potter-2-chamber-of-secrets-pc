//! `UnCamera.cpp` exports (`crates/hp2-engine/src/camera.rs`). The C++ shells in
//! `Engine/Src/UnCamera.cpp` forward here. Each export takes the C++ `Hp2CameraServices`
//! table as `*const c_void` and casts it inside.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FRotator, FSceneNode, UViewport};
use hp2_abi::{FPlane, FVector};
use hp2_engine::camera::{self, CameraServices};
use hp2_engine::camera_exec;
use hp2_ffi_support::guard;

/// `FSceneNode::ComputeRenderCoords(FVector&, FRotator&)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `frame` must be a live `FSceneNode` whose
/// `Viewport` and `Viewport->Actor` are live. `location` and `rotation` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_compute_render_coords(
    svc: *const c_void,
    frame: *mut c_void,
    location: *const c_void,
    rotation: *const c_void,
) {
    guard(
        "hp2_rs_engine_camera_compute_render_coords",
        (),
        move || unsafe {
            let (Some(svc), Some(location), Some(rotation)) = (
                svc.cast::<CameraServices>().as_ref(),
                location.cast::<FVector>().as_ref(),
                rotation.cast::<FRotator>().as_ref(),
            ) else {
                return;
            };
            let Some(frame) = frame.cast::<FSceneNode>().as_mut() else {
                return;
            };
            camera::compute_render_coords(svc, frame, *location, *rotation);
        },
    )
}

/// `FSceneNode::ComputeRenderSize()` forward.
///
/// # Safety
/// Same contract as [`hp2_rs_engine_camera_compute_render_coords`] for `svc` and `frame`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_compute_render_size(
    svc: *const c_void,
    frame: *mut c_void,
) {
    guard(
        "hp2_rs_engine_camera_compute_render_size",
        (),
        move || unsafe {
            let (Some(svc), Some(frame)) = (
                svc.cast::<CameraServices>().as_ref(),
                frame.cast::<FSceneNode>().as_mut(),
            ) else {
                return;
            };
            camera::compute_render_size(svc, frame);
        },
    )
}

/// `URenderDevice::Draw2DClippedLine(Frame, Color, LineFlags, P1, P2)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `render_dev` and `frame` must be live engine
/// objects. `color`, `p1` and `p2` must be valid for reads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_draw_2d_clipped_line(
    svc: *const c_void,
    render_dev: *mut c_void,
    frame: *mut c_void,
    color: *const c_void,
    line_flags: u32,
    p1: *const c_void,
    p2: *const c_void,
) {
    guard(
        "hp2_rs_engine_camera_draw_2d_clipped_line",
        (),
        move || unsafe {
            let (Some(svc), Some(frame), Some(color), Some(p1), Some(p2)) = (
                svc.cast::<CameraServices>().as_ref(),
                frame.cast::<FSceneNode>().as_mut(),
                color.cast::<FPlane>().as_ref(),
                p1.cast::<FVector>().as_ref(),
                p2.cast::<FVector>().as_ref(),
            ) else {
                return;
            };
            camera::draw_2d_clipped_line(svc, render_dev, frame, *color, line_flags, *p1, *p2);
        },
    )
}

/// `URenderDevice::Draw3DLine(Frame, Color, LineFlags, P1, P2)` forward.
///
/// # Safety
/// Same contract as [`hp2_rs_engine_camera_draw_2d_clipped_line`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_draw_3d_line(
    svc: *const c_void,
    render_dev: *mut c_void,
    frame: *mut c_void,
    color: *const c_void,
    line_flags: u32,
    p1: *const c_void,
    p2: *const c_void,
) {
    guard("hp2_rs_engine_camera_draw_3d_line", (), move || unsafe {
        let (Some(svc), Some(frame), Some(color), Some(p1), Some(p2)) = (
            svc.cast::<CameraServices>().as_ref(),
            frame.cast::<FSceneNode>().as_mut(),
            color.cast::<FPlane>().as_ref(),
            p1.cast::<FVector>().as_ref(),
            p2.cast::<FVector>().as_ref(),
        ) else {
            return;
        };
        camera::draw_3d_line(svc, render_dev, frame, *color, line_flags, *p1, *p2);
    })
}

/// `UViewport::Exec(Cmd, Ar)` forward. Returns `0` (not handled), `1` (handled), or a fault code
/// (`>= 2`, see `camera_exec::FAULT_*`) that the shell raises with `appErrorf`/`appFailAssert`.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport`. `cmd` must be a
/// NUL-terminated `TCHAR` string. `ar` must be a live `FOutputDevice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_exec(
    svc: *const c_void,
    vp: *mut c_void,
    cmd: *const c_void,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_camera_exec", 1, move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return 1;
        };
        match camera_exec::exec(svc, vp, cmd, ar) {
            Ok(rc) => rc,
            Err(fault) => fault,
        }
    })
}

/// `URenderDevice::Exec` base forward (`GetDetailTextures` / `ToggleDetailTextures`).
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `rd` must be a live `URenderDevice`. `cmd` and
/// `ar` as in [`hp2_rs_engine_camera_exec`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_render_dev_exec(
    svc: *const c_void,
    rd: *mut c_void,
    cmd: *const c_void,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_camera_render_dev_exec", 0, move || unsafe {
        let Some(svc) = svc.cast::<CameraServices>().as_ref() else {
            return 0;
        };
        camera_exec::render_dev_exec(svc, rd, cmd, ar)
    })
}

/// `UViewport::Lock(FPlane, FPlane, FPlane, DWORD, HHitProxy*, INT*)` forward. Returns `1`.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport` with a live render
/// device. The `FPlane` pointers must be valid for reads. `hit_data`/`hit_size` as in the shell.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_engine_camera_lock(
    svc: *const c_void,
    vp: *mut c_void,
    flash_scale: *const c_void,
    flash_fog: *const c_void,
    screen_clear: *const c_void,
    render_lock_flags: u32,
    hit_data: *mut c_void,
    hit_size: *mut i32,
) -> i32 {
    guard("hp2_rs_engine_camera_lock", 1, move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return 1;
        };
        camera_exec::lock(
            svc,
            vp,
            flash_scale.cast::<FPlane>(),
            flash_fog.cast::<FPlane>(),
            screen_clear.cast::<FPlane>(),
            render_lock_flags,
            hit_data,
            hit_size,
        )
    })
}

/// `UViewport::Unlock(UBOOL Blit)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport` with a live render
/// device.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_unlock(
    svc: *const c_void,
    vp: *mut c_void,
    blit: i32,
) {
    guard("hp2_rs_engine_camera_unlock", (), move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return;
        };
        camera_exec::unlock(svc, vp, blit);
    })
}

/// `UViewport::ReadInput(FLOAT DeltaSeconds)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport` with a live input.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_read_input(
    svc: *const c_void,
    vp: *mut c_void,
    delta_seconds: f32,
) {
    guard("hp2_rs_engine_camera_read_input", (), move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return;
        };
        camera_exec::read_input(svc, vp, delta_seconds);
    })
}

/// `UViewport::PushHit(HHitProxy&, INT Size)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport`; `hit` a live
/// `HHitProxy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_push_hit(
    svc: *const c_void,
    vp: *mut c_void,
    hit: *mut c_void,
    size: i32,
) {
    guard("hp2_rs_engine_camera_push_hit", (), move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return;
        };
        camera_exec::push_hit(svc, vp, hit, size);
    })
}

/// `UViewport::PopHit(UBOOL bForce)` forward, after the shell's `check` lines.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport` with a live render
/// device.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_pop_hit(
    svc: *const c_void,
    vp: *mut c_void,
    force: i32,
) {
    guard("hp2_rs_engine_camera_pop_hit", (), move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return;
        };
        camera_exec::pop_hit(svc, vp, force);
    })
}

/// `UViewport::ExecuteHits` link loop. Writes the leftover size to `hit_size` and the innermost
/// hit to `top`. The shell checks the leftover size and calls `Click`.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `hit_data` must point at `*hit_size` bytes of
/// packed `HHitProxy` records. `hit_size` and `top` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_link_hits(
    svc: *const c_void,
    hit_data: *mut c_void,
    hit_size: *mut i32,
    top: *mut *mut c_void,
) {
    guard("hp2_rs_engine_camera_link_hits", (), move || unsafe {
        let (Some(svc), Some(hit_size), Some(top)) = (
            svc.cast::<CameraServices>().as_ref(),
            hit_size.as_mut(),
            top.as_mut(),
        ) else {
            return;
        };
        camera_exec::link_hits(svc, hit_data, hit_size, top);
    })
}

/// `UViewport::ExecMacro(Filename, Ar)` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport`. `filename` must
/// be a NUL-terminated `TCHAR` string. `ar` must be a live `FOutputDevice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_exec_macro(
    svc: *const c_void,
    vp: *mut c_void,
    filename: *const c_void,
    ar: *mut c_void,
) {
    guard("hp2_rs_engine_camera_exec_macro", (), move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return;
        };
        camera_exec::exec_macro(svc, vp, filename, ar);
    })
}

/// `UViewport::SetDrag(UBOOL NewDrag)` forward. Returns the previous drag value.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_set_drag(
    svc: *const c_void,
    vp: *mut c_void,
    new_drag: i32,
) -> i32 {
    guard("hp2_rs_engine_camera_set_drag", 0, move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return 0;
        };
        camera_exec::set_drag(svc, vp, new_drag)
    })
}

/// `UViewport::IsWire()` forward.
///
/// # Safety
/// `svc` must point to a live `CameraServices`. `vp` must be a live `UViewport`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_camera_is_wire(svc: *const c_void, vp: *mut c_void) -> i32 {
    guard("hp2_rs_engine_camera_is_wire", 0, move || unsafe {
        let (Some(svc), Some(vp)) = (
            svc.cast::<CameraServices>().as_ref(),
            vp.cast::<UViewport>().as_mut(),
        ) else {
            return 0;
        };
        camera_exec::is_wire(svc, vp)
    })
}
