//! Port of `Engine/Src/UnCamera.cpp` render-frame math: `FSceneNode::ComputeRenderCoords`
//! and `ComputeRenderSize`, `FCoords::Transpose`, and the `URenderDevice` line paths
//! (`Draw2DClippedLine`, `Draw3DLine`).
//!
//! Engine objects arrive as raw pointers. Their layouts are read through the
//! `hp2_abi` generated mirrors. Virtual calls into `URenderDevice` and the
//! `GMath` tables go through [`CameraServices`], which `Hp2CameraServices.cpp`
//! fills in.
//!
//! Throw exposure: the `Hp2CameraServices` thunks that can raise `appErrorf`, `appThrowf`, or
//! `appFailAssert` are listed in `Hp2CameraServices.h`. A C++ throw unwinding through these Rust
//! frames is undefined behaviour, the same exposure as the committed console and input ports.

use std::ffi::c_void;

use hp2_abi::engine::generated::{APlayerPawn, FRotator, FSceneNode, UViewport};
use hp2_abi::{FCoords, FPlane, FVector};

use crate::cam_mgr::get_effective_fov_angle;
use crate::camera_math::{LINE_NEAR_CLIP_Z, clip_2d_line, perspective_point};
use crate::coords::{self, Trig};
use crate::prim;
use crate::vec;

/// `PI` (UnMath.h:30) as a float literal.
const PI_F32: f32 = std::f32::consts::PI;
/// `REN_OrthXY` / `REN_OrthXZ` / `REN_OrthYZ` (UnCamera.h:150-152).
pub(crate) const REN_ORTH_XY: i32 = 13;
pub(crate) const REN_ORTH_XZ: i32 = 14;
pub(crate) const REN_ORTH_YZ: i32 = 15;
/// `ORTHO_LOW_DETAIL` (UnRender.h:40).
const ORTHO_LOW_DETAIL: f32 = 40000.0;
/// `LINE_None` (UnRender.h:415).
pub const LINE_NONE: u32 = 0;
/// `LINE_PreTransformed` (UnRender.h:418).
pub const LINE_PRE_TRANSFORMED: u32 = 4;

/// C++ services (`Hp2CameraServices` in `Hp2CameraServices.cpp`), same field order.
#[repr(C)]
pub struct CameraServices {
    /// `UViewport::GetOuterUClient()->MaintainVerticalFOV`.
    pub viewport_maintain_vertical_fov: unsafe extern "C" fn(viewport: *mut c_void) -> i32,
    /// `URenderDevice::SetSceneNode(Frame)` (virtual).
    pub render_dev_set_scene_node:
        unsafe extern "C" fn(render_dev: *mut c_void, frame: *mut c_void),
    /// `&GMath.ViewCoords`.
    pub gmath_view_coords: unsafe extern "C" fn() -> *const FCoords,
    /// `GMath.SinTab(i)`.
    pub gmath_sin_tab: unsafe extern "C" fn(index: i32) -> f32,
    /// `GMath.CosTab(i)`.
    pub gmath_cos_tab: unsafe extern "C" fn(index: i32) -> f32,
    /// `URenderDevice::Draw2DLine(Frame, Color, LineFlags, P1, P2)` (virtual).
    pub render_dev_draw_2d_line: unsafe extern "C" fn(
        render_dev: *mut c_void,
        frame: *mut c_void,
        color: *const FPlane,
        line_flags: u32,
        p1: *const FVector,
        p2: *const FVector,
    ),
    /// `URenderDevice::Draw2DPoint(Frame, Color, LineFlags, X1, Y1, X2, Y2, Z)` (virtual).
    pub render_dev_draw_2d_point: unsafe extern "C" fn(
        render_dev: *mut c_void,
        frame: *mut c_void,
        color: *const FPlane,
        line_flags: u32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        z: f32,
    ),
    /// `ParseCommand(&cursor, token)`; advances `cursor` on match.
    pub parse_command: unsafe extern "C" fn(cursor: *mut c_void, token: *const c_void) -> i32,
    /// `ParseToken(cursor, buf, len, 0)`.
    pub parse_token: unsafe extern "C" fn(cursor: *mut c_void, buf: *mut c_void, len: i32) -> i32,
    /// `ParseLine(cursor, buf, len)`.
    pub parse_line: unsafe extern "C" fn(cursor: *mut c_void, buf: *mut c_void, len: i32) -> i32,
    /// `Parse(stream, match, &value)`.
    pub parse_int:
        unsafe extern "C" fn(stream: *const c_void, matched: *const c_void, value: *mut i32) -> i32,
    /// `appAtoi(s)`.
    pub app_atoi: unsafe extern "C" fn(s: *const c_void) -> i32,
    /// `appUserDir()`.
    pub app_user_dir: unsafe extern "C" fn() -> *const c_void,
    /// `DO_CHECK` as compiled into the adapter TU (nonzero = `check` live).
    pub check_enabled: unsafe extern "C" fn() -> i32,
    /// `GFileManager->FileSize(path)`.
    pub file_size: unsafe extern "C" fn(path: *const c_void) -> i32,
    /// `CreateFileWriter(path)`.
    pub create_file_writer: unsafe extern "C" fn(path: *const c_void) -> *mut c_void,
    /// `CreateFileWriter(*(GSys->SaveSlotPath * file))`.
    pub create_slot_file_writer: unsafe extern "C" fn(file: *const c_void) -> *mut c_void,
    /// `CreateFileWriter(*(FString(appUserDir()) * file))`.
    pub create_user_dir_file_writer: unsafe extern "C" fn(file: *const c_void) -> *mut c_void,
    /// `Ar->Serialize(data, count)`.
    pub archive_serialize: unsafe extern "C" fn(ar: *mut c_void, data: *const c_void, count: i32),
    /// `delete Ar`.
    pub archive_delete: unsafe extern "C" fn(ar: *mut c_void),
    /// `Ar.Logf(fmt)`.
    pub log_format: unsafe extern "C" fn(ar: *mut c_void, fmt: *const c_void),
    /// `Ar.Logf(fmt, str)`.
    pub log_str: unsafe extern "C" fn(ar: *mut c_void, fmt: *const c_void, arg: *const c_void),
    /// `Ar.Logf(fmt, int)`.
    pub log_int: unsafe extern "C" fn(ar: *mut c_void, fmt: *const c_void, arg: i32),
    /// `Ar.Logf(NAME_ExecWarning, fmt, str)`.
    pub log_exec_warning:
        unsafe extern "C" fn(ar: *mut c_void, fmt: *const c_void, arg: *const c_void),
    /// `debugf(fmt, str)`.
    pub debugf_str: unsafe extern "C" fn(fmt: *const c_void, arg: *const c_void),
    /// `LocalizeError("FileNotFound", TEXT("Core"))`.
    pub localize_file_not_found: unsafe extern "C" fn() -> *const c_void,
    /// `GIsRunning`.
    pub is_running: unsafe extern "C" fn() -> i32,
    /// `viewport->CurrentTime = appSeconds()`.
    pub viewport_stamp_current_time: unsafe extern "C" fn(viewport: *mut c_void),
    /// `viewport->CurrentNetSpeed = speed`.
    pub viewport_set_current_net_speed: unsafe extern "C" fn(viewport: *mut c_void, speed: i32),
    /// SHOWALL loop: `Level->Actors(i)->bHidden = 0` for every actor.
    pub viewport_show_all_actors: unsafe extern "C" fn(viewport: *mut c_void),
    /// REPORT branch: `FStringOutputDevice` report, `Ar.Logf(*Str, Cmd)`, clipboard copy.
    pub viewport_report:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const c_void, ar: *mut c_void),
    /// `Engine->MouseDelta(viewport, kind, 0, 0)`.
    pub viewport_mouse_delta: unsafe extern "C" fn(viewport: *mut c_void, kind: i32),
    /// `UpdateInput(flags)`.
    pub viewport_update_input: unsafe extern "C" fn(viewport: *mut c_void, flags: i32),
    /// `Console && Console->ScriptConsoleExec(cmd, ar, Actor)`.
    pub viewport_script_console_exec:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const c_void, ar: *mut c_void) -> i32,
    /// `UPlayer::Exec(cmd, ar)` (base implementation, non-virtual call).
    pub viewport_player_exec:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const c_void, ar: *mut c_void) -> i32,
    /// `this->Exec(cmd, ar)` (virtual; reaches `USDLViewport::Exec` overrides).
    pub viewport_exec:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const c_void, ar: *mut c_void) -> i32,
    /// `Actor->GetLevel()->NetDriver`.
    pub viewport_net_driver: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `Actor->GetLevel()->Model->Nodes.Num()`.
    pub viewport_model_node_count: unsafe extern "C" fn(viewport: *mut c_void) -> i32,
    /// `SnapBuff.Empty()`.
    pub viewport_snap_buff_empty: unsafe extern "C" fn(viewport: *mut c_void),
    /// `SnapBuff.Add(count)`.
    pub viewport_snap_buff_add: unsafe extern "C" fn(viewport: *mut c_void, count: i32),
    /// `HitSizes.AddItem(size)`.
    pub viewport_hit_sizes_add: unsafe extern "C" fn(viewport: *mut c_void, size: i32),
    /// `HitSizes.Pop()`.
    pub viewport_hit_sizes_pop: unsafe extern "C" fn(viewport: *mut c_void) -> i32,
    /// `Input->Exec(cmd, ar)` (virtual).
    pub input_exec:
        unsafe extern "C" fn(input: *mut c_void, cmd: *const c_void, ar: *mut c_void) -> i32,
    /// `Input->ReadInput(delta, *GLog)` (virtual).
    pub input_read_input: unsafe extern "C" fn(input: *mut c_void, delta: f32),
    /// `RenDev->Exec(cmd, ar)` (virtual; reaches the base shell for plain devices).
    pub render_dev_exec:
        unsafe extern "C" fn(render_dev: *mut c_void, cmd: *const c_void, ar: *mut c_void) -> i32,
    /// `RenDev->ReadPixels(dest)` (virtual).
    pub render_dev_read_pixels: unsafe extern "C" fn(render_dev: *mut c_void, dest: *mut c_void),
    /// `RenDev->Lock(FlashScale, FlashFog, ScreenClear, flags, HitData, HitSize)` (virtual).
    pub render_dev_lock: unsafe extern "C" fn(
        render_dev: *mut c_void,
        flash_scale: *const FPlane,
        flash_fog: *const FPlane,
        screen_clear: *const FPlane,
        flags: u32,
        hit_data: *mut c_void,
        hit_size: *mut i32,
    ),
    /// `RenDev->Unlock(blit)` (virtual).
    pub render_dev_unlock: unsafe extern "C" fn(render_dev: *mut c_void, blit: i32),
    /// `RenDev->PushHit(hit, size)` (virtual).
    pub render_dev_push_hit:
        unsafe extern "C" fn(render_dev: *mut c_void, hit: *const c_void, size: i32),
    /// `RenDev->PopHit(size, force)` (virtual).
    pub render_dev_pop_hit: unsafe extern "C" fn(render_dev: *mut c_void, size: i32, force: i32),
    /// `URenderDevice::DetailTextures`.
    pub render_dev_detail_textures: unsafe extern "C" fn(render_dev: *mut c_void) -> i32,
    /// `DetailTextures ^= 1; SaveConfig();`.
    pub render_dev_toggle_detail_textures: unsafe extern "C" fn(render_dev: *mut c_void),
    /// `HHitProxy::Size`.
    pub hit_proxy_size: unsafe extern "C" fn(hit: *const c_void) -> i32,
    /// `HHitProxy::Size = size`.
    pub hit_proxy_set_size: unsafe extern "C" fn(hit: *mut c_void, size: i32),
    /// `HHitProxy::Parent = parent`.
    pub hit_proxy_set_parent: unsafe extern "C" fn(hit: *mut c_void, parent: *mut c_void),
    /// `NetDriver->ServerConnection`.
    pub net_driver_server_connection: unsafe extern "C" fn(driver: *mut c_void) -> *mut c_void,
    /// `NetDriver->MaxClientRate`.
    pub net_driver_max_client_rate: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    /// `URL.HasOption("LAN")` on the connection's URL (NETSPEED/LANSPEED).
    pub connection_has_lan_option: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    /// `conn->CurrentNetSpeed = speed`.
    pub connection_set_current_net_speed: unsafe extern "C" fn(conn: *mut c_void, speed: i32),
    /// `conn->Logf(fmt)`.
    pub connection_logf_format: unsafe extern "C" fn(conn: *mut c_void, fmt: *const c_void),
    /// `conn->Logf(fmt, int)`.
    pub connection_logf_int: unsafe extern "C" fn(conn: *mut c_void, fmt: *const c_void, arg: i32),
    /// `GetDefault<UPlayer>()->ConfiguredInternetSpeed = rate; SaveConfig();`.
    pub player_set_configured_internet_speed: unsafe extern "C" fn(rate: i32),
    /// `ImportObject<UTextBuffer>(GetTransientPackage(), NAME_None, 0, file)`; may be null.
    pub import_text_buffer: unsafe extern "C" fn(file: *const c_void) -> *mut c_void,
    /// `text->Text` (the `FString` data pointer).
    pub text_buffer_text: unsafe extern "C" fn(text: *mut c_void) -> *const c_void,
    /// `text->AddToRoot()`.
    pub text_buffer_add_to_root: unsafe extern "C" fn(text: *mut c_void),
    /// `text->RemoveFromRoot(); delete text;`.
    pub text_buffer_release: unsafe extern "C" fn(text: *mut c_void),
}

#[inline]
fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

/// `UViewport::IsOrtho()` (UnCamera.h:307): reads `Actor->RendMap`.
#[inline]
fn is_ortho(actor: &APlayerPawn) -> bool {
    matches!(actor.rend_map, REN_ORTH_XY | REN_ORTH_XZ | REN_ORTH_YZ)
}

/// # Safety
/// `frame` must point to a live `FSceneNode` whose `viewport` is a live `UViewport`
/// with a live `Actor` (APlayerPawn).
#[inline]
unsafe fn frame_actor<'a>(frame: *const FSceneNode) -> (&'a UViewport, &'a APlayerPawn) {
    unsafe {
        let vp = &*(*frame).viewport.cast::<UViewport>();
        let actor = &*vp.actor.cast::<APlayerPawn>();
        (vp, actor)
    }
}

/// `FCoords::Transpose()` (UnMath.h:2226).
pub fn transpose(c: &FCoords) -> FCoords {
    let o = prim::transform_vector(c.origin, c);
    FCoords {
        origin: v3(-o.x, -o.y, -o.z),
        x_axis: v3(c.x_axis.x, c.y_axis.x, c.z_axis.x),
        y_axis: v3(c.x_axis.y, c.y_axis.y, c.z_axis.y),
        z_axis: v3(c.x_axis.z, c.y_axis.z, c.z_axis.z),
    }
}

/// `FPlane(InBase, InNormal)` (UnMath.h:1012): `W = InBase | InNormal`.
#[inline]
fn plane_from_base_normal(base: FVector, normal: FVector) -> FPlane {
    FPlane {
        x: normal.x,
        y: normal.y,
        z: normal.z,
        w: prim::dot(base, normal),
    }
}

/// `FSceneNode::ComputeRenderCoords(Location, Rotation)` (UnCamera.cpp:327-339).
///
/// # Safety
/// `frame` must satisfy the contract of [`frame_actor`]; `services` must be valid.
pub unsafe fn compute_render_coords(
    svc: &CameraServices,
    frame: *mut FSceneNode,
    location: FVector,
    rotation: FRotator,
) {
    unsafe {
        let (_, actor) = frame_actor(frame);
        let mut c = match actor.rend_map {
            REN_ORTH_XY => FCoords {
                origin: v3(0.0, 0.0, 0.0),
                x_axis: v3(1.0, 0.0, 0.0),
                y_axis: v3(0.0, 1.0, 0.0),
                z_axis: v3(0.0, 0.0, 1.0),
            },
            REN_ORTH_XZ => FCoords {
                origin: v3(0.0, 0.0, 0.0),
                x_axis: v3(1.0, 0.0, 0.0),
                y_axis: v3(0.0, 0.0, -1.0),
                z_axis: v3(0.0, 1.0, 0.0),
            },
            REN_ORTH_YZ => FCoords {
                origin: v3(0.0, 0.0, 0.0),
                x_axis: v3(0.0, 1.0, 0.0),
                y_axis: v3(0.0, 0.0, -1.0),
                z_axis: v3(1.0, 0.0, 0.0),
            },
            _ => {
                // `GMath.ViewCoords / Rotation`.
                let mut v = *(svc.gmath_view_coords)();
                let sin = |i: i32| (svc.gmath_sin_tab)(i);
                let cos = |i: i32| (svc.gmath_cos_tab)(i);
                let trig = Trig {
                    sin: &sin,
                    cos: &cos,
                };
                coords::div_rot(&mut v, rotation, &trig);
                v
            }
        };
        coords::div_vec(&mut c, location);
        (*frame).coords = c;
        (*frame).uncoords = transpose(&c);
        compute_render_size(svc, frame);
    }
}

/// `FSceneNode::ComputeRenderSize()` (UnCamera.cpp:344-393).
///
/// # Safety
/// `frame` must satisfy the contract of [`frame_actor`]; `services` must be valid.
pub unsafe fn compute_render_size(svc: &CameraServices, frame: *mut FSceneNode) {
    unsafe {
        let (vp, actor) = frame_actor(frame);
        let node = &mut *frame;

        let fx = node.x as f32;
        let fy = node.y as f32;
        let fx2 = fx * 0.5;
        let fy2 = fy * 0.5;
        let fx15 = (fx + 1.0001) * 0.5;
        let fy15 = (fy + 1.0001) * 0.5;
        let mut effective_fov = actor.base.fov_angle;
        if !is_ortho(actor) {
            let maintain_vertical = (svc.viewport_maintain_vertical_fov)(node.viewport);
            effective_fov =
                get_effective_fov_angle(effective_fov, node.x, node.y, maintain_vertical);
        }
        // `0.5f*FX / appTan(EffectiveFovAngle*PI/360.f)`: float divided by a double
        // `appTan`, narrowed to the FLOAT `Proj.Z`.
        let tan = f64::from((effective_fov * PI_F32) / 360.0).tan();
        let proj_z = (f64::from(0.5 * fx) / tan) as f32;
        // `0.5f - 0.5f*FX` contracts to one fused multiply-subtract.
        let proj = v3(
            (-0.5f32).mul_add(fx, 0.5),
            (-0.5f32).mul_add(fy, 0.5),
            proj_z,
        );
        let r_proj = v3(1.0 / proj.x, 1.0 / proj.y, 1.0 / proj.z);
        let zoom = actor.base.ortho_zoom / (fx * 15.0);
        let prj_xm = (0.0 - fx2) * (-r_proj.z);
        let prj_xp = (fx - fx2) * r_proj.z;
        let prj_ym = (0.0 - fy2) * (-r_proj.z);
        let prj_yp = (fy - fy2) * r_proj.z;

        // Side info.
        let signs = [-1.0f32, 1.0];
        let uncoords = node.uncoords;
        let origin = node.coords.origin;
        let mut view_sides = [v3(0.0, 0.0, 0.0); 4];
        let mut view_planes = [FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }; 4];
        for i in 0..2 {
            for j in 0..2 {
                view_sides[i * 2 + j] = prim::transform_vector(
                    vec::unsafe_normal(v3(signs[i] * fx2, signs[j] * fy2, proj.z)),
                    &uncoords,
                );
            }
            view_planes[i] = plane_from_base_normal(
                origin,
                prim::transform_vector(
                    vec::unsafe_normal(v3(0.0, signs[i] / fy2, 1.0 / proj.z)),
                    &uncoords,
                ),
            );
            view_planes[i + 2] = plane_from_base_normal(
                origin,
                prim::transform_vector(
                    vec::unsafe_normal(v3(signs[i] / fx2, 0.0, 1.0 / proj.z)),
                    &uncoords,
                ),
            );
        }

        node.fx = fx;
        node.fy = fy;
        node.fx2 = fx2;
        node.fy2 = fy2;
        node.fx15 = fx15;
        node.fy15 = fy15;
        node.effective_fov_angle = effective_fov;
        node.proj = proj;
        node.r_proj = r_proj;
        node.zoom = zoom;
        node.prj_xm = prj_xm;
        node.prj_xp = prj_xp;
        node.prj_ym = prj_ym;
        node.prj_yp = prj_yp;
        node.view_sides = view_sides;
        node.view_planes = view_planes;

        // Tell the rendering device.
        (svc.render_dev_set_scene_node)(vp.ren_dev, frame.cast::<c_void>());
    }
}

/// `URenderDevice::Draw2DClippedLine(Frame, Color, LineFlags, P1, P2)` (UnCamera.cpp:108-169).
///
/// # Safety
/// `render_dev` and `frame` must be valid engine objects; `services` must be valid.
pub unsafe fn draw_2d_clipped_line(
    svc: &CameraServices,
    render_dev: *mut c_void,
    frame: *mut FSceneNode,
    color: FPlane,
    line_flags: u32,
    p1: FVector,
    p2: FVector,
) {
    unsafe {
        let node = &*frame;
        let Some((a, b)) = clip_2d_line(node.fx, node.fy, p1, p2) else {
            return;
        };
        (svc.render_dev_draw_2d_line)(
            render_dev,
            frame.cast::<c_void>(),
            &color,
            line_flags,
            &a,
            &b,
        );
    }
}

/// `URenderDevice::Draw3DLine(Frame, Color, LineFlags, P1, P2)` (UnCamera.cpp:195-263).
///
/// # Safety
/// `render_dev` and `frame` must be valid engine objects; `services` must be valid.
pub unsafe fn draw_3d_line(
    svc: &CameraServices,
    render_dev: *mut c_void,
    frame: *mut FSceneNode,
    color: FPlane,
    line_flags: u32,
    mut p1: FVector,
    mut p2: FVector,
) {
    unsafe {
        let (_, actor) = frame_actor(frame);
        let (sx2, sy2) = ((*frame).fx2, (*frame).fy2);

        // Transform.
        if line_flags & LINE_PRE_TRANSFORMED == 0 {
            p1 = prim::transform_point(p1, &(*frame).coords);
            p2 = prim::transform_point(p2, &(*frame).coords);
        }
        if is_ortho(actor) {
            // `P / Zoom + FVector(SX2,SY2,0)`: no contraction (static fused count is 18).
            let rz = 1.0 / (*frame).zoom;
            p1 = v3(p1.x * rz + sx2, p1.y * rz + sy2, p1.z * rz + 0.0);
            p2 = v3(p2.x * rz + sx2, p2.y * rz + sy2, p2.z * rz + 0.0);

            // Parallel to the line of sight: the line appears as a dot.
            if (p2.x - p1.x).abs() + (p1.y - p2.y).abs() < 0.2 {
                if actor.base.ortho_zoom < ORTHO_LOW_DETAIL {
                    (svc.render_dev_draw_2d_point)(
                        render_dev,
                        frame.cast::<c_void>(),
                        &color,
                        LINE_NONE,
                        p1.x - 1.0,
                        p1.y - 1.0,
                        p1.x + 1.0,
                        p1.y + 1.0,
                        1.0,
                    );
                }
                return;
            }
        } else {
            // Discard the line if its points are identical.
            let d = prim::sub(p2, p1);
            if vec::size_squared(d) < 0.01f32 * 0.01f32 {
                return;
            }

            // Clip to the near clipping plane.
            let ncz = LINE_NEAR_CLIP_Z;
            if p1.z <= ncz {
                if p2.z < (ncz - 0.01) {
                    return;
                }
                p1.x += (ncz - p1.z) * d.x / d.z;
                p1.y += (ncz - p1.z) * d.y / d.z;
                p1.z = ncz;
            } else if p2.z < (ncz - 0.01) {
                p2.x += (ncz - p2.z) * d.x / d.z;
                p2.y += (ncz - p2.z) * d.y / d.z;
                p2.z = ncz;
            }

            // Perspective.
            let proj_z = (*frame).proj.z;
            p1 = perspective_point(p1, proj_z, sx2, sy2);
            p2 = perspective_point(p2, proj_z, sx2, sy2);
        }

        draw_2d_clipped_line(svc, render_dev, frame, color, line_flags, p1, p2);
    }
}
