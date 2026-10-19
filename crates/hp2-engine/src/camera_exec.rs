//! Port of the `UViewport` command, input, and capture family in `Engine/Src/UnCamera.cpp`:
//! `Exec` (with `ExecMacro`), `Lock`/`Unlock`, `ScreenShot`, `SNAP`/`THUMBNAIL`/`SAVE*`,
//! `SetDrag`, `IsWire`, `ReadInput`, `PushHit`/`PopHit`/`ExecuteHits`, and the base
//! `URenderDevice::Exec`.
//!
//! Rust never raises. A fault comes back as a `FAULT_*` code and the C++ shell raises the
//! real `appErrorf` / `appFailAssert`, with the original `__FILE__`/`__LINE__` literals.
//! Virtual calls, `Console`/`UPlayer` dispatch, log output, and UObject lifetime go through
//! [`CameraServices`], which `Hp2CameraServices.cpp` fills in.

use std::ffi::c_void;
use std::ptr::from_mut;

use hp2_abi::FColor;
use hp2_abi::engine::generated::{APlayerPawn, UViewport};

use crate::camera::{CameraServices, REN_ORTH_XY, REN_ORTH_XZ, REN_ORTH_YZ};
use crate::params::TChar;

/// `Exec` result codes returned to the shell: `0` not handled, `1` handled.
/// Codes `>= 2` are faults (see below); the shell raises them and returns `1`.
pub const FAULT_NEGATIVE_DIMENSION: i32 = 2;
/// `appErrorf("Bitmap dimensions exceed the signed 32-bit buffer limit")`.
pub const FAULT_PIXEL_COUNT: i32 = 3;
/// `appErrorf("Bitmap dimensions exceed the 32-bit file format")`.
pub const FAULT_FILE_FORMAT: i32 = 4;
/// `check(!appIsNan(ShrinkageValue) && ShrinkageValue>=1.0 && ShrinkageValue<=MAXINT)` (UnCamera.cpp:702).
pub const FAULT_SNAP_SHRINKAGE: i32 = 5;
/// `check(widthFactor<31)` (UnCamera.cpp:888).
pub const FAULT_SAVESNAP_WIDTH: i32 = 6;
/// `check(heightFactor<31)` (UnCamera.cpp:900).
pub const FAULT_SAVESNAP_HEIGHT: i32 = 7;

type Fault = i32;
type Res<T> = Result<T, Fault>;

/// `REN_None` / `REN_Wire` / `REN_Zones` / `REN_PolyCuts` / `REN_MAX` (UnCamera.h:150-160).
const REN_NONE: i32 = 0;
const REN_WIRE: i32 = 1;
const REN_ZONES: i32 = 2;
const REN_POLY_CUTS: i32 = 4;
const REN_MAX: i32 = 21;
/// `SHOW_Actors` / `SHOW_Bounds` / `SHOW_Bones` (UnRender.h).
const SHOW_ACTORS: i32 = 0x0008;
const SHOW_BOUNDS: i32 = 0x8000;
const SHOW_BONES: i32 = 0x10000;
/// `PF_NoMerge` (UnRender.h).
const PF_NO_MERGE: u32 = 0x10000;
/// `MOUSE_FirstHit` / `MOUSE_LastRelease` (UnCamera.h:195-196).
const MOUSE_FIRST_HIT: i32 = 0x0008;
const MOUSE_LAST_RELEASE: i32 = 0x0010;

/// `tstr!("text")` yields a NUL-terminated `TCHAR` string as `*const c_void`, the form the
/// services thunks take.
macro_rules! tstr {
    ($s:literal) => {{
        const LEN: usize = $s.len() + 1;
        const fn encode(src: &[u8]) -> [TChar; LEN] {
            let mut out = [0 as TChar; LEN];
            let mut i = 0;
            while i < src.len() {
                out[i] = src[i] as TChar;
                i += 1;
            }
            out
        }
        static S: [TChar; LEN] = encode($s.as_bytes());
        S.as_ptr().cast::<c_void>()
    }};
}

/// `Clamp(X, Min, Max)` (UnTemplate.h:123).
fn clamp_speed(x: i32, max: i32) -> i32 {
    if x < 500 {
        500
    } else if x < max {
        x
    } else {
        max
    }
}

/// `appDiv` for `INT`. C++ `/` with a zero divisor is UB; arm64 `sdiv` yields 0, which is
/// what this returns. `wrapping_div` matches the `INT_MIN / -1` wrap.
fn sdiv(x: i32, y: i32) -> i32 {
    if y == 0 { 0 } else { x.wrapping_div(y) }
}

/// `ParseCommand(&Cmd, Token)`: advances `cursor` only on a match.
#[inline]
unsafe fn match_command(
    svc: &CameraServices,
    cursor: &mut *const c_void,
    token: *const c_void,
) -> bool {
    unsafe { (svc.parse_command)(from_mut(cursor).cast::<c_void>(), token) != 0 }
}

/// `UViewport::Actor` as `APlayerPawn`, where `ShowFlags` / `RendMap` live.
#[inline]
fn pawn(vp: *mut UViewport) -> *mut APlayerPawn {
    unsafe { (*vp).actor.cast::<APlayerPawn>() }
}

/// `UViewport::Exec` (UnCamera.cpp:581-...). Input and render device dispatch first, then the
/// `ParseCommand` chain, then console and player fallbacks.
///
/// # Safety
/// `vp` must be a live `UViewport`. `cmd` must be a NUL-terminated `TCHAR` string. `ar` must
/// be a live `FOutputDevice`.
pub unsafe fn exec(
    svc: &CameraServices,
    vp: *mut UViewport,
    cmd: *const c_void,
    ar: *mut c_void,
) -> Res<i32> {
    unsafe {
        let input = (*vp).input;
        if !input.is_null() && (svc.input_exec)(input, cmd, ar) != 0 {
            return Ok(1);
        }
        let rd = (*vp).ren_dev;
        if !rd.is_null() && (svc.render_dev_exec)(rd, cmd, ar) != 0 {
            return Ok(1);
        }

        let mut cur = cmd;
        if match_command(svc, &mut cur, tstr!("GETPING")) {
            (svc.log_format)(ar, tstr!("0"));
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("INJECT")) {
            inject(svc, vp, cur, ar);
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("NETSPEED")) {
            net_speed(svc, vp, cur, false);
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("LANSPEED")) {
            net_speed(svc, vp, cur, true);
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("SHOWALL")) {
            (svc.viewport_show_all_actors)(vp.cast());
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("REPORT")) {
            (svc.viewport_report)(vp.cast(), cur, ar);
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("SHOT")) {
            return shot(svc, vp, &mut cur);
        }
        if match_command(svc, &mut cur, tstr!("SNAP")) {
            return snap(svc, vp, cur);
        }
        if match_command(svc, &mut cur, tstr!("THUMBNAIL")) {
            return thumbnail(svc, vp, cur);
        }
        if match_command(svc, &mut cur, tstr!("SAVETHUMBNAIL")) {
            return save_thumbnail_cmd(svc, vp, &mut cur);
        }
        if match_command(svc, &mut cur, tstr!("SAVESNAP")) {
            return save_snap_cmd(svc, vp, &mut cur);
        }
        if match_command(svc, &mut cur, tstr!("SHOWACTORS")) {
            (*pawn(vp)).show_flags |= SHOW_ACTORS;
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("HIDEACTORS")) {
            (*pawn(vp)).show_flags &= !SHOW_ACTORS;
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("RMODE")) {
            let mode = (svc.app_atoi)(cur);
            if mode > REN_NONE && mode < REN_MAX {
                (*pawn(vp)).rend_map = mode;
            }
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("EXEC")) {
            let mut filename = [0 as TChar; 64];
            if (svc.parse_token)(cursor_ptr(&mut cur), filename.as_mut_ptr().cast(), 64) != 0 {
                exec_macro(svc, vp, filename.as_ptr().cast(), ar);
            }
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("SHOWBONES")) {
            set_show_flag(pawn(vp), (svc.app_atoi)(cur) != 0, SHOW_BONES);
            return Ok(1);
        }
        if match_command(svc, &mut cur, tstr!("SHOWBOUNDS")) {
            set_show_flag(pawn(vp), (svc.app_atoi)(cur) != 0, SHOW_BOUNDS);
            return Ok(1);
        }
        if (svc.viewport_script_console_exec)(vp.cast(), cur, ar) != 0 {
            return Ok(1);
        }
        if (svc.viewport_player_exec)(vp.cast(), cur, ar) != 0 {
            return Ok(1);
        }
        Ok(0)
    }
}

/// `ParseToken`/`ParseLine` cursor: the C++ `const TCHAR*&` lives in a local.
#[inline]
fn cursor_ptr(cursor: &mut *const c_void) -> *mut c_void {
    from_mut(cursor).cast::<c_void>()
}

/// `ShowFlags |= Flag` / `&= ~Flag` (UnCamera.cpp, SHOWBONES / SHOWBOUNDS).
#[inline]
unsafe fn set_show_flag(actor: *mut APlayerPawn, on: bool, flag: i32) {
    unsafe {
        if on {
            (*actor).show_flags |= flag;
        } else {
            (*actor).show_flags &= !flag;
        }
    }
}

/// `INJECT` (UnCamera.cpp:603-611): `Driver && Driver->ServerConnection`, log, then inject.
unsafe fn inject(svc: &CameraServices, vp: *mut UViewport, cmd: *const c_void, ar: *mut c_void) {
    unsafe {
        let driver = (svc.viewport_net_driver)(vp.cast());
        if driver.is_null() {
            return;
        }
        let conn = (svc.net_driver_server_connection)(driver);
        if conn.is_null() {
            return;
        }
        (svc.log_str)(ar, tstr!("Injecting: %s"), cmd);
        // `Driver->ServerConnection->Logf(Cmd)`: the command text is used as the format.
        (svc.connection_logf_format)(conn, cmd);
    }
}

/// `NETSPEED` (`want_lan == false`) and `LANSPEED` (`want_lan == true`), UnCamera.cpp:614-638.
/// `Driver` is read before the configured speed is saved; the connection is read after.
unsafe fn net_speed(svc: &CameraServices, vp: *mut UViewport, cmd: *const c_void, want_lan: bool) {
    unsafe {
        let rate = (svc.app_atoi)(cmd);
        let driver = (svc.viewport_net_driver)(vp.cast());
        (svc.player_set_configured_internet_speed)(rate);
        if rate < 500 || driver.is_null() {
            return;
        }
        let conn = (svc.net_driver_server_connection)(driver);
        if conn.is_null() {
            return;
        }
        if ((svc.connection_has_lan_option)(conn) != 0) != want_lan {
            return;
        }
        let speed = clamp_speed(rate, (svc.net_driver_max_client_rate)(driver));
        (svc.viewport_set_current_net_speed)(vp.cast(), speed);
        (svc.connection_set_current_net_speed)(conn, speed);
        (svc.connection_logf_int)(conn, tstr!("NETSPEED %i"), rate);
    }
}

/// `ExecMacro(Filename, Ar)` (UnCamera.cpp:996-...). Runs each line of a text buffer through
/// the virtual `Exec`; a missing file logs `FileNotFound` as an exec warning.
///
/// # Safety
/// `vp` must be a live `UViewport`; `filename` a NUL-terminated `TCHAR` string; `ar` a live
/// `FOutputDevice`.
pub unsafe fn exec_macro(
    svc: &CameraServices,
    vp: *mut UViewport,
    filename: *const c_void,
    ar: *mut c_void,
) {
    unsafe {
        let text = (svc.import_text_buffer)(filename);
        if text.is_null() {
            (svc.log_exec_warning)(ar, (svc.localize_file_not_found)(), filename);
            return;
        }
        (svc.text_buffer_add_to_root)(text);
        (svc.debugf_str)(tstr!("Execing %s"), filename);
        let mut temp = [0 as TChar; 256];
        let mut data = (svc.text_buffer_text)(text);
        // A null `data` is passed to `parse_line` unchanged. The C++ crashes inside ParseLine
        // on the same input, so the crash is preserved rather than substituted.
        while (svc.parse_line)(cursor_ptr(&mut data), temp.as_mut_ptr().cast(), 256) != 0 {
            (svc.viewport_exec)(vp.cast(), temp.as_ptr().cast(), ar);
        }
        (svc.text_buffer_release)(text);
    }
}

/// `Lock` (UnCamera.cpp:506-522). Returns `1`.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live render device. Pointer arguments are forwarded
/// unchanged to `URenderDevice::Lock`.
#[allow(clippy::too_many_arguments)]
pub unsafe fn lock(
    svc: &CameraServices,
    vp: *mut UViewport,
    flash_scale: *const hp2_abi::FPlane,
    flash_fog: *const hp2_abi::FPlane,
    screen_clear: *const hp2_abi::FPlane,
    render_lock_flags: u32,
    hit_data: *mut c_void,
    hit_size: *mut i32,
) -> i32 {
    unsafe {
        (svc.viewport_stamp_current_time)(vp.cast());
        (*vp).hit_testing = i32::from(!hit_data.is_null());
        (*vp).render_flags = render_lock_flags;
        let actor = (*vp).actor;
        let poly_cuts_or_zones = !actor.is_null() && {
            let rend = (*actor.cast::<APlayerPawn>()).rend_map;
            rend == REN_POLY_CUTS || rend == REN_ZONES
        };
        (*vp).extra_poly_flags = if poly_cuts_or_zones { PF_NO_MERGE } else { 0 };
        (*vp).frame_count = (*vp).frame_count.wrapping_add(1);
        (svc.render_dev_lock)(
            (*vp).ren_dev,
            flash_scale,
            flash_fog,
            screen_clear,
            render_lock_flags,
            hit_data,
            hit_size,
        );
        1
    }
}

/// `Unlock(Blit)` (UnCamera.cpp:529-543), after the shell's `check` lines.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live render device.
pub unsafe fn unlock(svc: &CameraServices, vp: *mut UViewport, blit: i32) {
    unsafe {
        (svc.render_dev_unlock)((*vp).ren_dev, blit);
        if blit != 0 {
            (*vp).last_update_time = (*vp).current_time;
        }
    }
}

/// `SetDrag(NewDrag)` (UnCamera.cpp:298-312). Returns the previous drag value.
///
/// # Safety
/// `vp` must be a live `UViewport`.
pub unsafe fn set_drag(svc: &CameraServices, vp: *mut UViewport, new_drag: i32) -> i32 {
    unsafe {
        let result = (*vp).dragging;
        (*vp).dragging = new_drag;
        if (svc.is_running)() != 0 {
            if new_drag != 0 && result == 0 {
                (svc.viewport_mouse_delta)(vp.cast(), MOUSE_FIRST_HIT);
            } else if result != 0 && new_drag == 0 {
                (svc.viewport_mouse_delta)(vp.cast(), MOUSE_LAST_RELEASE);
            }
        }
        result
    }
}

/// `IsWire()` (UnCamera.cpp:485-496). `Actor &&` short-circuits before the model read.
///
/// # Safety
/// `vp` must be a live `UViewport`.
pub unsafe fn is_wire(svc: &CameraServices, vp: *mut UViewport) -> i32 {
    unsafe {
        let actor = (*vp).actor;
        if actor.is_null() {
            return 0;
        }
        let nodes = (svc.viewport_model_node_count)(vp.cast());
        let rend = (*actor.cast::<APlayerPawn>()).rend_map;
        i32::from(
            nodes == 0
                || rend == REN_ORTH_XY
                || rend == REN_ORTH_XZ
                || rend == REN_ORTH_YZ
                || rend == REN_WIRE,
        )
    }
}

/// `ReadInput(DeltaSeconds)` after the shell's `check(Input)`.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live input object.
pub unsafe fn read_input(svc: &CameraServices, vp: *mut UViewport, delta_seconds: f32) {
    unsafe {
        if delta_seconds != -1.0 {
            (svc.viewport_update_input)(vp.cast(), 0);
        }
        (svc.input_read_input)((*vp).input, delta_seconds);
    }
}

/// `PushHit(Hit, Size)` (UnCamera.cpp:1067-1072).
///
/// # Safety
/// `vp` must be a live `UViewport`; `hit` a live `HHitProxy`.
pub unsafe fn push_hit(svc: &CameraServices, vp: *mut UViewport, hit: *mut c_void, size: i32) {
    unsafe {
        (svc.hit_proxy_set_size)(hit, size);
        (svc.viewport_hit_sizes_add)(vp.cast(), size);
        (svc.render_dev_push_hit)((*vp).ren_dev, hit, size);
    }
}

/// `PopHit(bForce)` (UnCamera.cpp:1080-1088) after the shell's `check` lines.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live render device.
pub unsafe fn pop_hit(svc: &CameraServices, vp: *mut UViewport, force: i32) {
    unsafe {
        let size = (svc.viewport_hit_sizes_pop)(vp.cast());
        (svc.render_dev_pop_hit)((*vp).ren_dev, size, force);
    }
}

/// The `ExecuteHits` link loop (UnCamera.cpp:1099-1107). Each hit's `Parent` points to the
/// previous one; `*top` is the innermost (last) hit. `*hit_size` ends as the leftover size,
/// which the shell `check`s is zero.
///
/// # Safety
/// `hit_data` must point at `*hit_size` bytes of packed `HHitProxy` records. `top` must be
/// writable.
pub unsafe fn link_hits(
    svc: &CameraServices,
    mut hit_data: *mut c_void,
    hit_size: &mut i32,
    top: &mut *mut c_void,
) {
    unsafe {
        *top = std::ptr::null_mut();
        while *hit_size > 0 {
            let this_hit = hit_data;
            let size = (svc.hit_proxy_size)(this_hit);
            hit_data = hit_data.wrapping_offset(size as isize);
            *hit_size = hit_size.wrapping_sub(size);
            (svc.hit_proxy_set_parent)(this_hit, *top);
            *top = this_hit;
        }
    }
}

/// `URenderDevice::Exec` base (UnCamera.cpp:171-190).
///
/// # Safety
/// `rd` must be a live `URenderDevice`; `cmd` a NUL-terminated `TCHAR` string; `ar` a live
/// `FOutputDevice`.
pub unsafe fn render_dev_exec(
    svc: &CameraServices,
    rd: *mut c_void,
    cmd: *const c_void,
    ar: *mut c_void,
) -> i32 {
    unsafe {
        let mut cur = cmd;
        if match_command(svc, &mut cur, tstr!("GetDetailTextures")) {
            (svc.log_int)(ar, tstr!("%i"), (svc.render_dev_detail_textures)(rd));
            return 1;
        }
        if match_command(svc, &mut cur, tstr!("ToggleDetailTextures")) {
            (svc.render_dev_toggle_detail_textures)(rd);
            return 1;
        }
        0
    }
}

/// `ScreenShot(Filename)` target. `Plain` = `CreateFileWriter(path)`; `Slot` = the
/// `SaveSlotPath`-prefixed writer used by `SHOT <name>`.
#[derive(Clone, Copy)]
enum ShotTarget {
    Plain,
    Slot,
}

/// `CheckedBitmapDimension` (UnCamera.cpp:42-47).
fn checked_bitmap_dimension(dimension: i32) -> Res<u32> {
    if dimension < 0 {
        return Err(FAULT_NEGATIVE_DIMENSION);
    }
    Ok(dimension as u32)
}

/// `CheckedBitmapPixelCount` (UnCamera.cpp:48-53).
fn checked_bitmap_pixel_count(width: i32, height: i32) -> Res<i32> {
    if width < 0 || height < 0 || (width > 0 && height > i32::MAX / width) {
        return Err(FAULT_PIXEL_COUNT);
    }
    Ok(width * height)
}

/// `WriteUInt16LE` / `WriteUInt32LE` (UnCamera.cpp:20-40): one `Serialize` per field.
unsafe fn write_bytes(svc: &CameraServices, ar: *mut c_void, bytes: &[u8]) {
    unsafe { (svc.archive_serialize)(ar, bytes.as_ptr().cast(), bytes.len() as i32) }
}

/// `WriteBitmapHeaders` (UnCamera.cpp:55-83): 54-byte BMP header, 24 bpp, with the
/// 32-bit file-size check. The pixel and file sizes are computed in 64-bit before the check.
unsafe fn write_bitmap_headers(
    svc: &CameraServices,
    ar: *mut c_void,
    width: u32,
    height: u32,
) -> Res<()> {
    const FILE_HEADER_SIZE: u32 = 14;
    const INFO_HEADER_SIZE: u32 = 40;
    let pixel_bytes64 = 3u64 * u64::from(width) * u64::from(height);
    let file_bytes64 = u64::from(FILE_HEADER_SIZE) + u64::from(INFO_HEADER_SIZE) + pixel_bytes64;
    if pixel_bytes64 > u64::from(u32::MAX) || file_bytes64 > u64::from(u32::MAX) {
        return Err(FAULT_FILE_FORMAT);
    }
    let pixel_bytes = pixel_bytes64 as u32;
    unsafe {
        write_bytes(svc, ar, &0x4d42u16.to_le_bytes());
        write_bytes(svc, ar, &(file_bytes64 as u32).to_le_bytes());
        write_bytes(svc, ar, &0u16.to_le_bytes());
        write_bytes(svc, ar, &0u16.to_le_bytes());
        write_bytes(
            svc,
            ar,
            &(FILE_HEADER_SIZE + INFO_HEADER_SIZE).to_le_bytes(),
        );
        write_bytes(svc, ar, &INFO_HEADER_SIZE.to_le_bytes());
        write_bytes(svc, ar, &width.to_le_bytes());
        write_bytes(svc, ar, &height.to_le_bytes());
        write_bytes(svc, ar, &1u16.to_le_bytes());
        write_bytes(svc, ar, &24u16.to_le_bytes());
        write_bytes(svc, ar, &0u32.to_le_bytes());
        write_bytes(svc, ar, &pixel_bytes.to_le_bytes());
        write_bytes(svc, ar, &0u32.to_le_bytes());
        write_bytes(svc, ar, &0u32.to_le_bytes());
        write_bytes(svc, ar, &0u32.to_le_bytes());
        write_bytes(svc, ar, &0u32.to_le_bytes());
    }
    Ok(())
}

/// `Ar->Serialize(&Buf[i], 3)`: the first three bytes of a pixel in memory order (R, G, B).
/// The C++ writes R,G,B despite BMP expecting B,G,R; kept as-is.
#[inline]
unsafe fn write_pixel(svc: &CameraServices, ar: *mut c_void, pixel: *const FColor) {
    unsafe { (svc.archive_serialize)(ar, pixel.cast(), 3) }
}

/// `ScreenShot(Filename)` (UnCamera.cpp:545-571). Returns `Ok(1)`. Bitmap faults are checked
/// before the writer is created. After it is created, faults still delete the writer
/// (the C++ leaks it on the throw path; documented).
unsafe fn screen_shot(
    svc: &CameraServices,
    vp: *mut UViewport,
    target: ShotTarget,
    file: *const c_void,
) -> Res<i32> {
    unsafe {
        let size_x = (*vp).size_x;
        let size_y = (*vp).size_y;
        let count = checked_bitmap_pixel_count(size_x, size_y)?;
        // C++ `new(GMem)FColor` is uninitialized; ReadPixels overwrites every element, so the
        // zero fill has no observable effect.
        let mut buf = vec![
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            };
            count as usize
        ];
        (svc.render_dev_read_pixels)((*vp).ren_dev, buf.as_mut_ptr().cast());
        let ar = match target {
            ShotTarget::Plain => (svc.create_file_writer)(file),
            ShotTarget::Slot => (svc.create_slot_file_writer)(file),
        };
        if !ar.is_null() {
            let res = write_frame(svc, ar, buf.as_ptr(), size_x, size_y);
            // The C++ leaks Ar when a fault throws past its delete; Rust keeps that quirk.
            if res.is_ok() {
                (svc.archive_delete)(ar);
            }
            res?;
        }
        Ok(1)
    }
}

/// Header plus bottom-up rows for `ScreenShot`.
unsafe fn write_frame(
    svc: &CameraServices,
    ar: *mut c_void,
    buf: *const FColor,
    size_x: i32,
    size_y: i32,
) -> Res<()> {
    unsafe {
        let width = checked_bitmap_dimension(size_x)?;
        let height = checked_bitmap_dimension(size_y)?;
        write_bitmap_headers(svc, ar, width, height)?;
        for i in (0..size_y).rev() {
            for j in 0..size_x {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul(size_x).wrapping_add(j) as isize),
                );
            }
        }
        Ok(())
    }
}

/// Appends one `TCHAR` to the fixed 512-element buffer, stopping before the NUL slot.
/// Truncation is the documented divergence for `appSprintf` into `File[512]`.
fn push_tchar(out: &mut [TChar; 512], n: &mut usize, c: TChar) {
    if *n < 511 {
        out[*n] = c;
        *n += 1;
    }
}

/// `appSprintf(File, TEXT("%sShot%04i.bmp"), appUserDir(), i)` (UnCamera.cpp:687).
unsafe fn shot_name(svc: &CameraServices, index: i32, out: &mut [TChar; 512]) {
    unsafe {
        let mut n = 0usize;
        let dir = (svc.app_user_dir)().cast::<TChar>();
        let mut p = 0usize;
        loop {
            let c = *dir.add(p);
            if c == 0 {
                break;
            }
            push_tchar(out, &mut n, c);
            p += 1;
        }
        for b in b"Shot"
            .iter()
            .copied()
            .chain(format!("{index:04}").bytes())
            .chain(b".bmp".iter().copied())
        {
            push_tchar(out, &mut n, TChar::from(b));
        }
        out[n] = 0;
    }
}

/// `SHOT` (UnCamera.cpp:673-695). With a name, the slot writer is used. Without one, the first
/// unused `Shot%04i.bmp` is chosen, or the last name is used.
unsafe fn shot(svc: &CameraServices, vp: *mut UViewport, cursor: &mut *const c_void) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 512];
        if (svc.parse_token)(cursor_ptr(cursor), file.as_mut_ptr().cast(), 512) != 0 {
            return screen_shot(svc, vp, ShotTarget::Slot, file.as_ptr().cast());
        }
        for i in 0..256 {
            shot_name(svc, i, &mut file);
            if (svc.file_size)(file.as_ptr().cast()) < 0 {
                break;
            }
        }
        if (svc.file_size)(file.as_ptr().cast()) < 0 {
            return screen_shot(svc, vp, ShotTarget::Plain, file.as_ptr().cast());
        }
        Ok(1)
    }
}

/// `SNAP` (UnCamera.cpp:696-759). Shrinks the frame by `2^factor` and fills `SnapBuff`.
unsafe fn snap(svc: &CameraServices, vp: *mut UViewport, cmd: *const c_void) -> Res<i32> {
    unsafe {
        let shrink_factor = (svc.app_atoi)(cmd);
        // `appPow(2, shrinkFactor)`: libm `pow` in double precision (UnAnsi.cpp:123-142).
        let shrinkage_value = 2f64.powf(f64::from(shrink_factor));
        let in_range = !shrinkage_value.is_nan()
            && shrinkage_value >= 1.0
            && shrinkage_value <= f64::from(i32::MAX);
        if !in_range && (svc.check_enabled)() != 0 {
            return Err(FAULT_SNAP_SHRINKAGE);
        }
        let shrinkage = shrinkage_value as i32;
        (*vp).snap_u = sdiv((*vp).size_x, shrinkage);
        (*vp).snap_v = sdiv((*vp).size_y, shrinkage);
        fill_snap_buffer(svc, vp, shrinkage)?;
        Ok(1)
    }
}

/// `THUMBNAIL` (UnCamera.cpp:760-834). `x=` / `y=` default to 160 / 120; a zero value is
/// replaced by the default.
unsafe fn thumbnail(svc: &CameraServices, vp: *mut UViewport, cmd: *const c_void) -> Res<i32> {
    unsafe {
        let mut thumb_x = 0i32;
        if (svc.parse_int)(cmd, tstr!("x="), &mut thumb_x) == 0 {
            thumb_x = 160;
        }
        let mut thumb_y = 0i32;
        if (svc.parse_int)(cmd, tstr!("y="), &mut thumb_y) == 0 {
            thumb_y = 120;
        }
        if thumb_x == 0 {
            thumb_x = 160;
        }
        if thumb_y == 0 {
            thumb_y = 120;
        }
        let shrinkage = sdiv((*vp).size_x, thumb_x);
        (*vp).snap_u = thumb_x;
        (*vp).snap_v = thumb_y;
        fill_snap_buffer(svc, vp, shrinkage)?;
        Ok(1)
    }
}

/// Shared body of `SNAP` and `THUMBNAIL` after the target size is set: read the frame,
/// empty `SnapBuff`, add `snapU*snapV` entries, then box-filter.
unsafe fn fill_snap_buffer(svc: &CameraServices, vp: *mut UViewport, shrinkage: i32) -> Res<()> {
    unsafe {
        let size_x = (*vp).size_x;
        let size_y = (*vp).size_y;
        let count = checked_bitmap_pixel_count(size_x, size_y)?;
        let mut src = vec![
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            };
            count as usize
        ];
        (svc.render_dev_read_pixels)((*vp).ren_dev, src.as_mut_ptr().cast());
        (svc.viewport_snap_buff_empty)(vp.cast());
        let dst_count = checked_bitmap_pixel_count((*vp).snap_u, (*vp).snap_v)?;
        (svc.viewport_snap_buff_add)(vp.cast(), dst_count);
        box_filter(vp, src.as_ptr(), size_x, shrinkage);
        Ok(())
    }
}

/// Box filter from SNAP/THUMBNAIL (UnCamera.cpp:716-745). Each destination pixel averages
/// `shrinkage x shrinkage` source pixels. `dst` is re-read after `Add`.
///
/// Divergence: when `snapV * shrinkage > SizeY` (or `snapU * shrinkage > SizeX`) the
/// C++ reads past the source buffer. Rust reproduces the same offsets with
/// `wrapping_offset` and does not clamp; the read is out of bounds in both.
unsafe fn box_filter(vp: *mut UViewport, src: *const FColor, size_x: i32, shrinkage: i32) {
    unsafe {
        let snap_u = (*vp).snap_u;
        let snap_v = (*vp).snap_v;
        let dst = (*vp).snap_buff.data.cast::<FColor>();
        let div = shrinkage.wrapping_mul(shrinkage);
        for y in 0..snap_v {
            let dst_line = dst.wrapping_offset(y.wrapping_mul(snap_u) as isize);
            let src_top_line =
                src.wrapping_offset(y.wrapping_mul(size_x).wrapping_mul(shrinkage) as isize);
            for x in 0..snap_u {
                let src_top_left = src_top_line.wrapping_offset(x.wrapping_mul(shrinkage) as isize);
                let mut dest = [0i32; 4];
                for src_y in 0..shrinkage {
                    let src_line =
                        src_top_left.wrapping_offset(src_y.wrapping_mul(size_x) as isize);
                    for src_x in 0..shrinkage {
                        let p = *src_line.wrapping_offset(src_x as isize);
                        dest[0] = dest[0].wrapping_add(i32::from(p.r));
                        dest[1] = dest[1].wrapping_add(i32::from(p.g));
                        dest[2] = dest[2].wrapping_add(i32::from(p.b));
                        dest[3] = dest[3].wrapping_add(i32::from(p.a));
                    }
                }
                let out = dst_line.wrapping_offset(x as isize);
                (*out).r = sdiv(dest[0], div) as u8;
                (*out).g = sdiv(dest[1], div) as u8;
                (*out).b = sdiv(dest[2], div) as u8;
                (*out).a = sdiv(dest[3], div) as u8;
            }
        }
    }
}

/// `SAVETHUMBNAIL` (UnCamera.cpp:835-873) after `ParseCommand`.
unsafe fn save_thumbnail_cmd(
    svc: &CameraServices,
    vp: *mut UViewport,
    cursor: &mut *const c_void,
) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 64];
        if (svc.parse_token)(cursor_ptr(cursor), file.as_mut_ptr().cast(), 64) != 0
            && (*vp).snap_buff.num != 0
        {
            let ar = (svc.create_slot_file_writer)(file.as_ptr().cast());
            if !ar.is_null() {
                let res = save_thumbnail(svc, vp, ar);
                // Leaked on fault, as the C++ does (see screen_shot).
                if res.is_ok() {
                    (svc.archive_delete)(ar);
                }
                res?;
            }
        }
        Ok(1)
    }
}

/// Body of `SAVETHUMBNAIL`: header from the snap size, then rows bottom-up. `destWidth` is
/// `snapU`, so the width padding loop is empty.
unsafe fn save_thumbnail(svc: &CameraServices, vp: *mut UViewport, ar: *mut c_void) -> Res<()> {
    unsafe {
        let snap_u = (*vp).snap_u;
        let snap_v = (*vp).snap_v;
        let width = checked_bitmap_dimension(snap_u)?;
        let height = checked_bitmap_dimension(snap_v)?;
        write_bitmap_headers(svc, ar, width, height)?;
        let buf = (*vp).snap_buff.data.cast::<FColor>();
        for i in (0..snap_v).rev() {
            for j in 0..snap_u {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul(snap_u).wrapping_add(j) as isize),
                );
            }
        }
        Ok(())
    }
}

/// `SAVESNAP` (UnCamera.cpp:874-940) after `ParseCommand`.
unsafe fn save_snap_cmd(
    svc: &CameraServices,
    vp: *mut UViewport,
    cursor: &mut *const c_void,
) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 64];
        if (svc.parse_token)(cursor_ptr(cursor), file.as_mut_ptr().cast(), 64) != 0
            && (*vp).snap_buff.num != 0
        {
            let ar = (svc.create_user_dir_file_writer)(file.as_ptr().cast());
            if !ar.is_null() {
                let res = save_snap(svc, vp, ar);
                // Leaked on fault, as the C++ does (see screen_shot).
                if res.is_ok() {
                    (svc.archive_delete)(ar);
                }
                res?;
            }
        }
        Ok(1)
    }
}

/// Body of `SAVESNAP`: pads the snap to a power-of-two width and height, then writes top
/// padding, the rows, and any side padding.
unsafe fn save_snap(svc: &CameraServices, vp: *mut UViewport, ar: *mut c_void) -> Res<()> {
    unsafe {
        let snap_u = (*vp).snap_u;
        let snap_v = (*vp).snap_v;
        let dest_width = power_of_two_size(svc, snap_u, FAULT_SAVESNAP_WIDTH)?;
        let dest_height = power_of_two_size(svc, snap_v, FAULT_SAVESNAP_HEIGHT)?;
        let width = checked_bitmap_dimension(dest_width)?;
        let height = checked_bitmap_dimension(dest_height)?;
        write_bitmap_headers(svc, ar, width, height)?;
        let buf = (*vp).snap_buff.data.cast::<FColor>();
        for _ in (snap_v..dest_height).rev() {
            for _ in 0..dest_width {
                write_pixel(svc, ar, buf);
            }
        }
        for i in (0..snap_v).rev() {
            let mut j = 0;
            while j < snap_u {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul(snap_u).wrapping_add(j) as isize),
                );
                j += 1;
            }
            while j < dest_width {
                write_pixel(svc, ar, buf);
                j += 1;
            }
        }
        Ok(())
    }
}

/// The `widthFactor` / `heightFactor` loops (UnCamera.cpp:884-905): right-shift until zero,
/// then substitute `1 << factor`. `check(factor < 31)` fires only when the check is enabled.
unsafe fn power_of_two_size(svc: &CameraServices, size: i32, fault: i32) -> Res<i32> {
    unsafe {
        let mut dest = size;
        let mut factor: u32 = 0;
        loop {
            if dest == 0 {
                if factor >= 31 && (svc.check_enabled)() != 0 {
                    return Err(fault);
                }
                return Ok((1u32 << factor) as i32);
            }
            dest >>= 1;
            factor += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdiv_zero_divisor_is_zero() {
        assert_eq!(sdiv(7, 0), 0);
        assert_eq!(sdiv(-7, 2), -3);
        assert_eq!(sdiv(i32::MIN, -1), i32::MIN);
    }

    #[test]
    fn bitmap_pixel_count_limits() {
        assert_eq!(checked_bitmap_pixel_count(3, 4), Ok(12));
        assert_eq!(checked_bitmap_pixel_count(-1, 4), Err(FAULT_PIXEL_COUNT));
        assert_eq!(
            checked_bitmap_pixel_count(65536, 65536),
            Err(FAULT_PIXEL_COUNT)
        );
        assert_eq!(checked_bitmap_dimension(-1), Err(FAULT_NEGATIVE_DIMENSION));
    }

    #[test]
    fn clamp_speed_matches_template() {
        assert_eq!(clamp_speed(100, 20000), 500);
        assert_eq!(clamp_speed(1000, 20000), 1000);
        assert_eq!(clamp_speed(30000, 20000), 20000);
    }
}
