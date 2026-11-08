//! Port of the `UViewport` command, input, and capture family in `Engine/Src/UnCamera.cpp`:
//! `Exec` (with `ExecMacro`), `Lock`/`Unlock`, `ScreenShot`, `SNAP`/`THUMBNAIL`/`SAVE*`,
//! `SetDrag`, `IsWire`, `ReadInput`, `PushHit`/`PopHit`/`ExecuteHits`, and the base
//! `URenderDevice::Exec`.
//!
//! Rust never raises. Native failures propagate immediately as checked statuses. Bitmap
//! faults return `FAULT_*` codes to the enclosing native lifetime/guard, which raises the
//! real `appErrorf` / `appFailAssert` with the original source location.
//! Virtual calls, `Console`/`UPlayer` dispatch, log output, and UObject lifetime go through
//! [`CameraServices`], which `Hp2CameraServices.cpp` fills in.

use std::ffi::c_void;
use std::ptr::{self, from_mut};

use hp2_abi::FColor;
use hp2_abi::engine::generated::{
    AActor, APlayerPawn, TArrayRaw, ULevel, UModel, UNetConnection, UNetDriver, URenderDevice,
    UViewport,
};

use crate::camera::{
    CameraServices, REN_ORTH_XY, REN_ORTH_XZ, REN_ORTH_YZ, checked_unit, checked_value,
};
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

pub(crate) const REN_NONE: i32 = 0;
const REN_WIRE: i32 = 1;
const REN_ZONES: i32 = 2;
const REN_POLY_CUTS: i32 = 4;
pub(crate) const REN_MAX: i32 = 21;
/// `SHOW_Actors` / `SHOW_Bounds` / `SHOW_Bones` (UnRender.h).
pub(crate) const SHOW_ACTORS: i32 = 0x0008;
pub(crate) const SHOW_BOUNDS: i32 = 0x8000;
pub(crate) const SHOW_BONES: i32 = 0x10000;
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
pub(crate) fn clamp_speed(x: i32, max: i32) -> i32 {
    if x < 500 {
        500
    } else if x < max {
        x
    } else {
        max
    }
}
/// Shared `NETSPEED`/`LANSPEED` configuration, filter, clamp, and send ordering.
pub(crate) fn net_speed_with<E>(
    rate: i32,
    want_lan: bool,
    driver_present: bool,
    mut save_config: impl FnMut() -> Result<(), E>,
    mut server_connection: impl FnMut() -> Result<(*mut c_void, i32), E>,
    mut connection_is_lan: impl FnMut(*mut c_void) -> Result<bool, E>,
    mut apply_speed: impl FnMut(*mut c_void, i32) -> Result<(), E>,
    mut send_rate: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    save_config()?;
    if rate < 500 || !driver_present { return Ok(()); }
    let (connection, max_rate) = server_connection()?;
    if connection.is_null() || connection_is_lan(connection)? != want_lan { return Ok(()); }
    let speed = clamp_speed(rate, max_rate);
    apply_speed(connection, speed)?;
    send_rate(connection, rate)
}

/// `appDiv` for `INT`. C++ `/` with a zero divisor is UB; arm64 `sdiv` yields 0, which is
/// what this returns. `wrapping_div` matches the `INT_MIN / -1` wrap.
pub(crate) fn sdiv(x: i32, y: i32) -> i32 {
    if y == 0 { 0 } else { x.wrapping_div(y) }
}

/// `ParseCommand(&Cmd, Token)`: advances `cursor` only on a match.
#[inline]
unsafe fn match_command(
    svc: &CameraServices,
    cursor: &mut *const c_void,
    token: *const c_void,
) -> Res<bool> {
    Ok(unsafe {
        checked_value!((svc.parse_command)(
            from_mut(cursor).cast::<c_void>(),
            token
        )) != 0
    })
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
    let mut context = (svc, ar);
    let handled = unsafe {
        viewport_exec_dispatch_with(
            &mut context,
            vp,
            cmd.cast(),
            |context, input, command| {
                let (svc, ar) = *context;
                Ok(checked_value!((svc.input_exec)(input, command.cast(), ar)) != 0)
            },
            |context, device, command| {
                let (svc, ar) = *context;
                Ok(checked_value!((svc.render_dev_exec)(device, command.cast(), ar)) != 0)
            },
            |context, viewport, command| {
                let (svc, ar) = *context;
                unsafe { exec_viewport_commands(svc, viewport, command.cast(), ar) }
            },
            |context, viewport, command| {
                let (svc, ar) = *context;
                Ok(checked_value!((svc.viewport_script_console_exec)(viewport.cast(), command.cast(), ar)) != 0)
            },
            |context, viewport, command| {
                let (svc, ar) = *context;
                Ok(checked_value!((svc.viewport_player_exec)(viewport.cast(), command.cast(), ar)) != 0)
            },
        )
    }?;
    Ok(i32::from(handled))
}

/// Shared source-ordered Input, RenderDevice, viewport command, console, and player dispatch.
///
/// # Safety
/// `vp` must be live and `cmd` must point to a NUL-terminated TCHAR command.
pub unsafe fn viewport_exec_dispatch_with<C, E>(
    context: &mut C,
    vp: *mut UViewport,
    cmd: *const u32,
    mut input_exec: impl FnMut(&mut C, *mut c_void, *const u32) -> Result<bool, E>,
    mut render_exec: impl FnMut(&mut C, *mut c_void, *const u32) -> Result<bool, E>,
    mut viewport_commands: impl FnMut(&mut C, *mut UViewport, *const u32) -> Result<bool, E>,
    mut script_console_exec: impl FnMut(&mut C, *mut UViewport, *const u32) -> Result<bool, E>,
    mut player_exec: impl FnMut(&mut C, *mut UViewport, *const u32) -> Result<bool, E>,
) -> Result<bool, E> {
    let input = unsafe { (*vp).input };
    if !input.is_null() && input_exec(context, input, cmd)? { return Ok(true); }
    let render_device = unsafe { (*vp).ren_dev };
    if !render_device.is_null() && render_exec(context, render_device, cmd)? { return Ok(true); }
    if viewport_commands(context, vp, cmd)? { return Ok(true); }
    if script_console_exec(context, vp, cmd)? { return Ok(true); }
    player_exec(context, vp, cmd)
}

#[derive(Clone, Copy)]
pub(crate) enum ViewportCommand {
    GetPing,
    Inject,
    NetSpeed,
    LanSpeed,
    ShowAll,
    Report,
    Shot,
    Snap,
    Thumbnail,
    SaveThumbnail,
    SaveSnap,
    ShowActors,
    HideActors,
    RenderMode,
    ExecMacro,
    ShowBones,
    ShowBounds,
}
/// Shared `appPow(2, Factor)` snapshot shrinkage and source range predicate.
pub(crate) fn snap_shrinkage(factor: i32) -> (i32, bool) {
    let value = 2.0f64.powf(f64::from(factor));
    let in_range = !value.is_nan() && value >= 1.0 && value <= f64::from(i32::MAX);
    (value as i32, in_range)
}
/// Source THUMBNAIL default and zero-substitution rules shared by both backends.
pub(crate) fn thumbnail_dimensions(x: Option<i32>, y: Option<i32>) -> (i32, i32) {
    let x = x.unwrap_or(160);
    let y = y.unwrap_or(120);
    (if x == 0 { 160 } else { x }, if y == 0 { 120 } else { y })
}

/// The source right-shift loop for SAVESNAP dimensions. Negative sizes never
/// reach zero in the C++ loop; `None` makes that invalid state explicit.
pub(crate) fn shared_power_of_two_size(size: i32) -> Option<(i32, u32)> {
    if size < 0 { return None; }
    let mut destination = size;
    let mut factor = 0u32;
    loop {
        if destination == 0 {
            return Some(((1u32 << factor) as i32, factor));
        }
        destination >>= 1;
        factor += 1;
    }
}

/// Shared source-ordered `UViewport::Exec` command parser.
pub(crate) fn viewport_commands_with<E>(
    command: *const TChar,
    mut parse: impl FnMut(&mut *const TChar, *const TChar) -> Result<bool, E>,
    mut execute: impl FnMut(ViewportCommand, *const TChar) -> Result<bool, E>,
) -> Result<bool, E> {
    let mut cursor = command;
    macro_rules! branch {
        ($text:literal, $kind:ident) => {{
            if parse(&mut cursor, tstr!($text).cast::<TChar>())? {
                return execute(ViewportCommand::$kind, cursor);
            }
        }};
    }
    branch!("GETPING", GetPing);
    branch!("INJECT", Inject);
    branch!("NETSPEED", NetSpeed);
    branch!("LANSPEED", LanSpeed);
    branch!("SHOWALL", ShowAll);
    branch!("REPORT", Report);
    branch!("SHOT", Shot);
    branch!("SNAP", Snap);
    branch!("THUMBNAIL", Thumbnail);
    branch!("SAVETHUMBNAIL", SaveThumbnail);
    branch!("SAVESNAP", SaveSnap);
    branch!("SHOWACTORS", ShowActors);
    branch!("HIDEACTORS", HideActors);
    branch!("RMODE", RenderMode);
    branch!("EXEC", ExecMacro);
    branch!("SHOWBONES", ShowBones);
    branch!("SHOWBOUNDS", ShowBounds);
    Ok(false)
}
/// Shared first-free `Shot%04i.bmp` scan used by Native and Core owners.
pub(crate) fn unique_shot_index_with<E>(
    mut file_is_missing: impl FnMut(usize) -> Result<bool, E>,
) -> Result<Option<usize>, E> {
    for index in 0..256 {
        if file_is_missing(index)? { return Ok(Some(index)); }
    }
    Ok(None)
}

unsafe fn exec_viewport_commands(
    svc: &CameraServices,
    vp: *mut UViewport,
    cmd: *const c_void,
    ar: *mut c_void,
) -> Res<bool> {
    unsafe {
        viewport_commands_with(
            cmd.cast(),
            |cursor, token| unsafe {
                let cursor = &mut *(cursor as *mut *const TChar).cast::<*const c_void>();
                match_command(svc, cursor, token.cast())
            },
            |branch, cursor| {
                let mut cur = cursor.cast::<c_void>();
                match branch {
                    ViewportCommand::GetPing => {
                        checked_unit!((svc.log_format)(ar, tstr!("0")));
                        Ok(true)
                    }
                    ViewportCommand::Inject => { inject(svc, vp, cur, ar)?; Ok(true) }
                    ViewportCommand::NetSpeed => { net_speed(svc, vp, cur, false)?; Ok(true) }
                    ViewportCommand::LanSpeed => { net_speed(svc, vp, cur, true)?; Ok(true) }
                    ViewportCommand::ShowAll => {
                        checked_unit!((svc.viewport_show_all_actors)(vp.cast()));
                        Ok(true)
                    }
                    ViewportCommand::Report => {
                        checked_unit!((svc.viewport_report)(vp.cast(), cur, ar));
                        Ok(true)
                    }
                    ViewportCommand::Shot => Ok(shot(svc, vp, &mut cur)? != 0),
                    ViewportCommand::Snap => Ok(snap(svc, vp, cur)? != 0),
                    ViewportCommand::Thumbnail => Ok(thumbnail(svc, vp, cur)? != 0),
                    ViewportCommand::SaveThumbnail => Ok(save_thumbnail_cmd(svc, vp, &mut cur)? != 0),
                    ViewportCommand::SaveSnap => Ok(save_snap_cmd(svc, vp, &mut cur)? != 0),
                    ViewportCommand::ShowActors => {
                        (*pawn(vp)).show_flags |= SHOW_ACTORS;
                        Ok(true)
                    }
                    ViewportCommand::HideActors => {
                        (*pawn(vp)).show_flags &= !SHOW_ACTORS;
                        Ok(true)
                    }
                    ViewportCommand::RenderMode => {
                        let mode = checked_value!((svc.app_atoi)(cur));
                        if mode > REN_NONE && mode < REN_MAX { (*pawn(vp)).rend_map = mode; }
                        Ok(true)
                    }
                    ViewportCommand::ExecMacro => {
                        let mut filename = [0 as TChar; 64];
                        if checked_value!((svc.parse_token)(cursor_ptr(&mut cur), filename.as_mut_ptr().cast(), 64)) != 0 {
                            checked_unit!((svc.viewport_exec_macro)(vp.cast(), filename.as_ptr().cast(), ar));
                        }
                        Ok(true)
                    }
                    ViewportCommand::ShowBones => {
                        set_show_flag(pawn(vp), checked_value!((svc.app_atoi)(cur)) != 0, SHOW_BONES);
                        Ok(true)
                    }
                    ViewportCommand::ShowBounds => {
                        set_show_flag(pawn(vp), checked_value!((svc.app_atoi)(cur)) != 0, SHOW_BOUNDS);
                        Ok(true)
                    }
                }
            },
        )
    }
}

/// `ParseToken`/`ParseLine` cursor: the C++ `const TCHAR*&` lives in a local.
#[inline]
fn cursor_ptr(cursor: &mut *const c_void) -> *mut c_void {
    from_mut(cursor).cast::<c_void>()
}

/// `ShowFlags |= Flag` / `&= ~Flag` (UnCamera.cpp, SHOWBONES / SHOWBOUNDS).
#[inline]
pub(crate) unsafe fn set_show_flag(actor: *mut APlayerPawn, on: bool, flag: i32) {
    unsafe {
        if on {
            (*actor).show_flags |= flag;
        } else {
            (*actor).show_flags &= !flag;
        }
    }
}

/// `INJECT` (UnCamera.cpp:603-611): `Driver && Driver->ServerConnection`, log, then inject.
unsafe fn inject(
    svc: &CameraServices,
    vp: *mut UViewport,
    cmd: *const c_void,
    ar: *mut c_void,
) -> Res<()> {
    unsafe {
        let driver = (*(*(*vp).actor.cast::<AActor>()).x_level.cast::<ULevel>()).base.net_driver;
        if driver.is_null() {
            return Ok(());
        }
        let conn = (*driver.cast::<UNetDriver>()).server_connection;
        if conn.is_null() {
            return Ok(());
        }
        checked_unit!((svc.log_str)(ar, tstr!("Injecting: %s"), cmd));
        // `Driver->ServerConnection->Logf(Cmd)`: the command text is used as the format.
        checked_unit!((svc.connection_logf_format)(
            (*driver.cast::<UNetDriver>()).server_connection,
            cmd
        ));
    }

    Ok(())
}

/// `NETSPEED` (`want_lan == false`) and `LANSPEED` (`want_lan == true`), UnCamera.cpp:614-638.
/// `Driver` is read before the configured speed is saved; the connection is read after.
unsafe fn net_speed(
    svc: &CameraServices,
    vp: *mut UViewport,
    cmd: *const c_void,
    want_lan: bool,
) -> Res<()> {
    unsafe {
        let rate = checked_value!((svc.app_atoi)(cmd));
        let driver = (*(*(*vp).actor.cast::<AActor>()).x_level.cast::<ULevel>()).base.net_driver;
        net_speed_with(
            rate,
            want_lan,
            !driver.is_null(),
            || unsafe {
                checked_unit!((svc.player_set_configured_internet_speed)(rate));
                Ok(())
            },
            || unsafe {
                let driver = driver.cast::<UNetDriver>();
                Ok(((*driver).server_connection, (*driver).max_client_rate))
            },
            |connection| unsafe {
                Ok(checked_value!((svc.connection_has_lan_option)(connection)) != 0)
            },
            |connection, speed| unsafe {
                (*connection.cast::<UNetConnection>()).current_net_speed = speed;
                (*vp).current_net_speed = speed;
                Ok(())
            },
            |connection, rate| unsafe {
                checked_unit!((svc.connection_logf_int)(connection, tstr!("NETSPEED %i"), rate));
                Ok(())
            },
        )
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
) -> Res<()> {
    unsafe {
        exec_macro_with(
            filename.cast(),
            |file| {
                let text = checked_value!((svc.import_text_buffer)(file.cast()));
                Ok(if text.is_null() { None } else { Some(text) })
            },
            |file| {
                checked_unit!((svc.log_exec_warning)(
                    ar,
                    checked_value!((svc.localize_file_not_found)()),
                    file.cast(),
                ));
                Ok(())
            },
            |text| { checked_unit!((svc.text_buffer_add_to_root)(*text)); Ok(()) },
            |file| { checked_unit!((svc.debugf_str)(tstr!("Execing %s"), file.cast())); Ok(()) },
            |text| Ok(checked_value!((svc.text_buffer_text)(*text)).cast()),
            |cursor, line| {
                Ok(checked_value!((svc.parse_line)(
                    cursor as *mut *const TChar as *mut c_void,
                    line.as_mut_ptr().cast(),
                    line.len() as i32,
                )) != 0)
            },
            |line| { checked_value!((svc.viewport_exec)(vp.cast(), line.cast(), ar)); Ok(()) },
            |text| { checked_unit!((svc.text_buffer_release)(text)); Ok(()) },
        )
    }
}

/// Shared `ExecMacro` import/root/line-execution/release algorithm for Native and Core.
///
/// # Safety
/// The callbacks must retain each imported text buffer and command line through its call.
pub(crate) unsafe fn exec_macro_with<T, E>(
    filename: *const TChar,
    mut import: impl FnMut(*const TChar) -> Result<Option<T>, E>,
    mut log_missing: impl FnMut(*const TChar) -> Result<(), E>,
    mut add_root: impl FnMut(&mut T) -> Result<(), E>,
    mut debug_exec: impl FnMut(*const TChar) -> Result<(), E>,
    mut text: impl FnMut(&mut T) -> Result<*const TChar, E>,
    mut parse_line: impl FnMut(&mut *const TChar, &mut [TChar; 256]) -> Result<bool, E>,
    mut execute: impl FnMut(*const TChar) -> Result<(), E>,
    mut release: impl FnMut(T) -> Result<(), E>,
) -> Result<(), E> {
    let Some(mut buffer) = import(filename)? else {
        log_missing(filename)?;
        return Ok(());
    };
    add_root(&mut buffer)?;
    debug_exec(filename)?;
    let mut cursor = text(&mut buffer)?;
    let mut line = [0 as TChar; 256];
    while parse_line(&mut cursor, &mut line)? {
        execute(line.as_ptr())?;
    }
    release(buffer)
}

/// Shared `UViewport::Lock` state changes and source-ordered renderer dispatch.
///
/// # Safety
/// `vp` and all pointer arguments remain live through the callbacks.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn lock_with<E>(
    vp: *mut UViewport,
    flash_scale: *const hp2_abi::FPlane,
    flash_fog: *const hp2_abi::FPlane,
    screen_clear: *const hp2_abi::FPlane,
    render_lock_flags: u32,
    hit_data: *mut c_void,
    hit_size: *mut i32,
    mut stamp_current_time: impl FnMut(*mut UViewport) -> Result<(), E>,
    mut render_lock: impl FnMut(*mut c_void, *const hp2_abi::FPlane, *const hp2_abi::FPlane, *const hp2_abi::FPlane, u32, *mut c_void, *mut i32) -> Result<(), E>,
) -> Result<i32, E> {
    unsafe {
        stamp_current_time(vp)?;
        (*vp).hit_testing = i32::from(!hit_data.is_null());
        (*vp).render_flags = render_lock_flags;
        let actor = (*vp).actor;
        let poly_cuts_or_zones = !actor.is_null() && {
            let rend = (*actor.cast::<APlayerPawn>()).rend_map;
            rend == REN_POLY_CUTS || rend == REN_ZONES
        };
        (*vp).extra_poly_flags = if poly_cuts_or_zones { PF_NO_MERGE } else { 0 };
        (*vp).frame_count = (*vp).frame_count.wrapping_add(1);
        render_lock(
            (*vp).ren_dev,
            flash_scale,
            flash_fog,
            screen_clear,
            render_lock_flags,
            hit_data,
            hit_size,
        )?;
    }
    Ok(1)
}

/// `Lock` adapter over the shared viewport state policy.
///
/// # Safety
/// `vp` must be live and all pointer arguments remain live through callbacks.
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
) -> Res<i32> {
    unsafe {
        lock_with(
            vp,
            flash_scale,
            flash_fog,
            screen_clear,
            render_lock_flags,
            hit_data,
            hit_size,
            |viewport| { checked_unit!((svc.viewport_stamp_current_time)(viewport.cast())); Ok(()) },
            |device, scale, fog, clear, flags, hit_data, hit_size| {
                checked_unit!((svc.render_dev_lock)(device, scale, fog, clear, flags, hit_data, hit_size));
                Ok(())
            },
        )
    }
}

/// Shared `UViewport::Unlock` renderer call and blit-time update.
///
/// # Safety
/// `vp` and the renderer callback's device argument must remain live.
pub(crate) unsafe fn unlock_with<E>(
    vp: *mut UViewport,
    blit: i32,
    mut render_unlock: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        render_unlock((*vp).ren_dev, blit)?;
        if blit != 0 {
            (*vp).last_update_time = (*vp).current_time;
        }
    }
    Ok(())
}

/// `Unlock(Blit)` adapter over the shared viewport policy.
///
/// # Safety
/// `vp` must be live with a live render device.
pub unsafe fn unlock(svc: &CameraServices, vp: *mut UViewport, blit: i32) -> Res<()> {
    unsafe {
        unlock_with(vp, blit, |device, blit| {
            checked_unit!((svc.render_dev_unlock)(device, blit));
            Ok(())
        })
    }
}

/// `SetDrag(NewDrag)` (UnCamera.cpp:298-312). Returns the previous drag value.
///
/// # Safety
/// `vp` must be a live `UViewport`.
pub unsafe fn set_drag(svc: &CameraServices, vp: *mut UViewport, new_drag: i32) -> Res<i32> {
    Ok(unsafe {
        let result = (*vp).dragging;
        (*vp).dragging = new_drag;
        if checked_value!((svc.is_running)()) != 0 {
            if new_drag != 0 && result == 0 {
                checked_unit!((svc.viewport_mouse_delta)(vp.cast(), MOUSE_FIRST_HIT));
            } else if result != 0 && new_drag == 0 {
                checked_unit!((svc.viewport_mouse_delta)(vp.cast(), MOUSE_LAST_RELEASE));
            }
        }
        result
    })
}

/// `IsWire()` (UnCamera.cpp:485-496). `Actor &&` short-circuits before the model read.
///
/// # Safety
/// `vp` must be a live `UViewport`.
pub unsafe fn is_wire(_svc: &CameraServices, vp: *mut UViewport) -> Res<i32> {
    Ok(unsafe {
        let actor = (*vp).actor;
        if actor.is_null() {
            return Ok(0);
        }
        let level = (*actor.cast::<AActor>()).x_level;
        let model = (*level.cast::<ULevel>()).model;
        let nodes = ptr::addr_of!((*model.cast::<UModel>()).nodes)
            .cast::<TArrayRaw>()
            .read_unaligned()
            .num;
        let rend = (*actor.cast::<APlayerPawn>()).rend_map;
        i32::from(
            nodes == 0
                || rend == REN_ORTH_XY
                || rend == REN_ORTH_XZ
                || rend == REN_ORTH_YZ
                || rend == REN_WIRE,
        )
    })
}

/// `ReadInput(DeltaSeconds)` after the shell's `check(Input)`.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live input object.
pub unsafe fn read_input(svc: &CameraServices, vp: *mut UViewport, delta_seconds: f32) -> Res<()> {
    unsafe {
        if delta_seconds != -1.0 {
            checked_unit!((svc.viewport_update_input)(vp.cast(), 0));
        }
        checked_unit!((svc.input_read_input)((*vp).input, delta_seconds));
    }

    Ok(())
}

/// `PushHit(Hit, Size)` (UnCamera.cpp:1067-1072).
///
/// # Safety
/// `vp` must be a live `UViewport`; `hit` a live `HHitProxy`.
pub unsafe fn push_hit(
    svc: &CameraServices,
    vp: *mut UViewport,
    hit: *mut c_void,
    size: i32,
) -> Res<()> {
    unsafe {
        // `Size`/`Parent` is a union at byte offset 8: `HHitProxy` has a vtable pointer
        // (8 bytes on this ABI) occupying bytes 0-7 before the union.
        *(hit.byte_add(8) as *mut i32) = size;
        checked_unit!((svc.viewport_hit_sizes_add)(vp.cast(), size));
        checked_unit!((svc.render_dev_push_hit)((*vp).ren_dev, hit, size));
    }
    Ok(())
}

/// `PopHit(bForce)` (UnCamera.cpp:1080-1088) after the shell's `check` lines.
///
/// # Safety
/// `vp` must be a live `UViewport` with a live render device.
pub unsafe fn pop_hit(svc: &CameraServices, vp: *mut UViewport, force: i32) -> Res<()> {
    unsafe {
        let mut size = 0;
        checked_unit!((svc.viewport_hit_sizes_pop)(vp.cast(), &mut size));
        checked_unit!((svc.render_dev_pop_hit)((*vp).ren_dev, size, force));
    }
    Ok(())
}

/// The `ExecuteHits` link loop (UnCamera.cpp:1099-1107). Each hit's `Parent` points to the
/// previous one; `*top` is the innermost (last) hit. `*hit_size` ends as the leftover size,
/// which the shell `check`s is zero.
///
/// # Safety
/// `hit_data` must point at `*hit_size` bytes of packed `HHitProxy` records. `top` must be
/// writable.
pub unsafe fn link_hits(
    _svc: &CameraServices,
    mut hit_data: *mut c_void,
    hit_size: *mut i32,
    top: *mut *mut c_void,
) -> Res<()> {
    unsafe {
        *top = std::ptr::null_mut();
        while *hit_size > 0 {
            let this_hit = hit_data;
            let size = *(this_hit.byte_add(8) as *const i32);
            hit_data = hit_data.wrapping_offset(size as isize);
            *hit_size = (*hit_size).wrapping_sub(size);
            *(this_hit.byte_add(8) as *mut *mut c_void) = *top;
            *top = this_hit;
        }
    }

    Ok(())
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
) -> Res<i32> {
    unsafe {
        render_dev_exec_with(
            rd.cast(),
            cmd.cast(),
            |cursor, token| {
                Ok(unsafe {
                    checked_value!((svc.parse_command)(cursor.cast(), token.cast())) != 0
                })
            },
            |value| {
                checked_unit!((svc.log_int)(ar, tstr!("%i"), value));
                Ok(())
            },
            |device| {
                checked_unit!((svc.render_dev_toggle_detail_textures)(device.cast()));
                Ok(())
            },
        )
    }
}

/// Shared `URenderDevice::Exec` branches for native and canonical Core adapters.
///
/// # Safety
/// `rd` must be a live canonical or native `URenderDevice`; `cmd` must be NUL-terminated.
pub unsafe fn render_dev_exec_with<E>(
    rd: *mut URenderDevice,
    cmd: *const u32,
    mut parse_command: impl FnMut(*mut *const u32, *const u32) -> Result<bool, E>,
    mut log_detail_textures: impl FnMut(i32) -> Result<(), E>,
    mut toggle_detail_textures: impl FnMut(*mut URenderDevice) -> Result<(), E>,
) -> Result<i32, E> {
    let mut cursor = cmd;
    if parse_command(&mut cursor, tstr!("GetDetailTextures").cast())? {
        let enabled = unsafe { (*rd).detail_textures() };
        log_detail_textures(i32::from(enabled))?;
        return Ok(1);
    }
    if parse_command(&mut cursor, tstr!("ToggleDetailTextures").cast())? {
        toggle_detail_textures(rd)?;
        return Ok(1);
    }
    Ok(0)
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
unsafe fn write_bytes(svc: &CameraServices, ar: *mut c_void, bytes: &[u8]) -> Res<()> {
    unsafe {
        checked_unit!((svc.archive_serialize)(ar, bytes.as_ptr().cast(), bytes.len() as i32))
    }
    Ok(())
}
/// `WriteBitmapHeaders` (UnCamera.cpp:55-83): 54-byte BMP header, 24 bpp, with the
/// 32-bit file-size check. The pixel and file sizes are computed in 64-bit before the check.
pub(crate) fn write_bitmap_headers_with<E>(
    width: u32,
    height: u32,
    mut write: impl FnMut(&[u8]) -> Result<(), E>,
    make_error: impl FnOnce(i32) -> E,
) -> Result<(), E> {
    const FILE_HEADER_SIZE: u32 = 14;
    const INFO_HEADER_SIZE: u32 = 40;
    let pixel_bytes64 = 3u64 * u64::from(width) * u64::from(height);
    let file_bytes64 = u64::from(FILE_HEADER_SIZE) + u64::from(INFO_HEADER_SIZE) + pixel_bytes64;
    if pixel_bytes64 > u64::from(u32::MAX) || file_bytes64 > u64::from(u32::MAX) {
        return Err(make_error(FAULT_FILE_FORMAT));
    }
    let pixel_bytes = pixel_bytes64 as u32;
    for bytes in [
        &0x4d42u16.to_le_bytes()[..],
        &(file_bytes64 as u32).to_le_bytes(),
        &0u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &(FILE_HEADER_SIZE + INFO_HEADER_SIZE).to_le_bytes(),
        &INFO_HEADER_SIZE.to_le_bytes(),
        &width.to_le_bytes(),
        &height.to_le_bytes(),
        &1u16.to_le_bytes(),
        &24u16.to_le_bytes(),
        &0u32.to_le_bytes(),
        &pixel_bytes.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
        &0u32.to_le_bytes(),
    ] {
        write(bytes)?;
    }
    Ok(())
}

pub(crate) unsafe fn write_pixel_with<E>(
    pixel: *const FColor,
    mut write: impl FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), E> {
    let bytes = unsafe { std::slice::from_raw_parts(pixel.cast::<u8>(), 3) };
    write(bytes)
}

unsafe fn write_bitmap_headers(
    svc: &CameraServices,
    ar: *mut c_void,
    width: u32,
    height: u32,
) -> Res<()> {
    write_bitmap_headers_with(
        width,
        height,
        |bytes| unsafe { write_bytes(svc, ar, bytes) },
        |fault| fault,
    )
}
/// `Ar->Serialize(&Buf[i], 3)`: the first three bytes of a pixel in memory order (R, G, B).
/// The C++ writes R,G,B despite BMP expecting B,G,R; kept as-is.
#[inline]
unsafe fn write_pixel(svc: &CameraServices, ar: *mut c_void, pixel: *const FColor) -> Res<()> {
    write_pixel_with(pixel, |bytes| unsafe {
        checked_unit!((svc.archive_serialize)(ar, bytes.as_ptr().cast(), bytes.len() as i32));
        Ok(())
    })
}
unsafe fn with_pixels<F: FnMut(*mut FColor) -> Res<()>>(
    svc: &CameraServices,
    vp: *mut UViewport,
    snap: bool,
    mut run: F,
) -> Res<()> {
    struct Context<F> {
        svc: *const CameraServices,
        vp: *mut UViewport,
        run: *mut F,
    }
    unsafe extern "C" fn invoke<F: FnMut(*mut FColor) -> Res<()>>(context: *mut c_void) -> i32 {
        unsafe {
            let context = context.cast::<Context<F>>();
            let svc = &*(*context).svc;
            let vp = (*context).vp;
            let result = (|| -> Res<()> {
                let count = checked_bitmap_pixel_count((*vp).size_x, (*vp).size_y)?;
                let pixels = checked_value!((svc.pixel_allocate)(count));
                (&mut *(*context).run)(pixels.cast())
            })();
            result.map_or_else(|status| -status, |()| 0)
        }
    }
    let mut context = Context {
        svc,
        vp,
        run: &raw mut run,
    };
    unsafe {
        checked_unit!((svc.pixel_scope)(
            i32::from(snap),
            (&raw mut context).cast(),
            invoke::<F>
        ));
    }
    Ok(())
}

/// `ScreenShot(Filename)` (UnCamera.cpp:545-571). Returns `Ok(1)`. Bitmap faults are checked
/// in the original native mark scope. A failure after writer creation retains the
/// original leaked archive and unpopped mark rather than inventing cleanup.
unsafe fn screen_shot(svc: &CameraServices, vp: *mut UViewport, file: *const c_void) -> Res<i32> {
    unsafe {
        with_pixels(svc, vp, false, |buf| {
            checked_unit!((svc.render_dev_read_pixels)((*vp).ren_dev, buf.cast()));
            let ar = checked_value!((svc.create_file_writer)(file));
            if !ar.is_null() {
                write_frame(svc, vp, ar, buf)?;
                checked_unit!((svc.archive_delete)(ar));
            }
            Ok(())
        })?;
        Ok(1)
    }
}

/// Header plus bottom-up rows for `ScreenShot`.
unsafe fn write_frame(
    svc: &CameraServices,
    vp: *mut UViewport,
    ar: *mut c_void,
    buf: *const FColor,
) -> Res<()> {
    unsafe {
        let width = checked_bitmap_dimension((*vp).size_x)?;
        let height = checked_bitmap_dimension((*vp).size_y)?;
        write_bitmap_headers(svc, ar, width, height)?;
        let mut i = (*vp).size_y.wrapping_sub(1);
        while i >= 0 {
            let mut j = 0i32;
            while j < (*vp).size_x {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul((*vp).size_x).wrapping_add(j) as isize),
                )?;
                j += 1;
            }
            i -= 1;
        }
    }
    Ok(())
}

/// `appSprintf(File, TEXT("%sShot%04i.bmp"), appUserDir(), i)` (UnCamera.cpp:687).
unsafe fn shot_name(svc: &CameraServices, index: i32, out: &mut [TChar; 512]) -> Res<()> {
    unsafe {
        checked_unit!((svc.shot_name)(index, out.as_mut_ptr().cast()));
    }
    Ok(())
}

unsafe fn slot_screen_shot(
    svc: &CameraServices,
    vp: *mut UViewport,
    file: *const c_void,
) -> Res<i32> {
    struct Context {
        svc: *const CameraServices,
        vp: *mut UViewport,
    }
    unsafe extern "C" fn run(context: *mut c_void, path: *const c_void) -> i32 {
        unsafe {
            let c = context.cast::<Context>();
            screen_shot(&*(*c).svc, (*c).vp, path).map_or_else(|status| -status, |_| 0)
        }
    }
    let mut context = Context { svc, vp };
    unsafe {
        checked_unit!((svc.shot_path_scope)(file, (&raw mut context).cast(), run));
    }
    Ok(1)
}

/// `SHOT` (UnCamera.cpp:673-695). With a name, the slot writer is used. Without one, the first
/// unused `Shot%04i.bmp` is chosen, or the last name is used.
unsafe fn shot(svc: &CameraServices, vp: *mut UViewport, cursor: &mut *const c_void) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 512];
        if checked_value!((svc.parse_token)(
            cursor_ptr(cursor),
            file.as_mut_ptr().cast(),
            512
        )) != 0
        {
            return slot_screen_shot(svc, vp, file.as_ptr().cast());
        }
        let index = unique_shot_index_with(|index| {
            shot_name(svc, index as i32, &mut file)?;
            Ok(checked_value!((svc.file_size)(file.as_ptr().cast())) < 0)
        })?;
        shot_name(svc, index.unwrap_or(255) as i32, &mut file)?;
        if checked_value!((svc.file_size)(file.as_ptr().cast())) < 0 {
            return screen_shot(svc, vp, file.as_ptr().cast());
        }
        Ok(1)
    }
}

/// `SNAP` (UnCamera.cpp:696-759). Shrinks the frame by `2^factor` and fills `SnapBuff`.
unsafe fn snap(svc: &CameraServices, vp: *mut UViewport, cmd: *const c_void) -> Res<i32> {
    unsafe {
        let shrink_factor = checked_value!((svc.app_atoi)(cmd));
        // `appPow(2, shrinkFactor)`: libm `pow` in double precision (UnAnsi.cpp:123-142).
        let (shrinkage, in_range) = snap_shrinkage(shrink_factor);
        if !in_range && checked_value!((svc.check_enabled)()) != 0 {
            return Err(FAULT_SNAP_SHRINKAGE);
        }
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
        let has_x = checked_value!((svc.parse_int)(cmd, tstr!("x="), &mut thumb_x)) != 0;
        let mut thumb_y = 0i32;
        let has_y = checked_value!((svc.parse_int)(cmd, tstr!("y="), &mut thumb_y)) != 0;
        let (thumb_x, thumb_y) = thumbnail_dimensions(
            has_x.then_some(thumb_x),
            has_y.then_some(thumb_y),
        );
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
        with_pixels(svc, vp, true, |src| {
            checked_unit!((svc.render_dev_read_pixels)((*vp).ren_dev, src.cast()));
            fill_snap_buffer_with(
                vp,
                src,
                (*vp).size_x,
                shrinkage,
                checked_bitmap_pixel_count((*vp).snap_u, (*vp).snap_v)?,
                |viewport| {
                    checked_unit!((svc.viewport_snap_buff_empty)(viewport.cast()));
                    Ok(())
                },
                |viewport, count| {
                    checked_unit!((svc.viewport_snap_buff_add)(viewport.cast(), count));
                    Ok(())
                },
                |pixel| {
                    let mut channels = [0i32; 4];
                    let status = (svc.pixel_channels)(pixel.cast(), channels.as_mut_ptr());
                    if status == 0 { Ok(channels) } else { Err(-status) }
                },
            )
        })
    }
}

/// Shared snap-buffer reset, allocation, and box-filter path for Native and Core.
///
/// # Safety
/// `viewport` and `source` must remain valid through the synchronous callbacks.
pub(crate) unsafe fn fill_snap_buffer_with<E>(
    viewport: *mut UViewport,
    source: *const FColor,
    source_width: i32,
    shrinkage: i32,
    destination_count: i32,
    mut empty: impl FnMut(*mut UViewport) -> Result<(), E>,
    mut add: impl FnMut(*mut UViewport, i32) -> Result<(), E>,
    pixel_channels: impl FnMut(*const FColor) -> Result<[i32; 4], E>,
) -> Result<(), E> {
    empty(viewport)?;
    add(viewport, destination_count)?;
    unsafe { box_filter_with(viewport, source, source_width, shrinkage, pixel_channels) }
}


/// Shared source box-filter kernel for Native and Core capture paths.
///
/// # Safety
/// `vp` and the full source area addressed by the source-compatible offsets must be live.
pub(crate) unsafe fn box_filter_with<E>(
    vp: *mut UViewport,
    src: *const FColor,
    size_x: i32,
    shrinkage: i32,
    mut pixel_channels: impl FnMut(*const FColor) -> Result<[i32; 4], E>,
) -> Result<(), E> {
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
                    let src_line = src_top_left.wrapping_offset(src_y.wrapping_mul(size_x) as isize);
                    for src_x in 0..shrinkage {
                        let channels = pixel_channels(src_line.wrapping_offset(src_x as isize))?;
                        for channel in 0..4 {
                            dest[channel] = dest[channel].wrapping_add(channels[channel]);
                        }
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
    Ok(())
}

/// `SAVETHUMBNAIL` (UnCamera.cpp:835-873) after `ParseCommand`.
unsafe fn save_thumbnail_cmd(
    svc: &CameraServices,
    vp: *mut UViewport,
    cursor: &mut *const c_void,
) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 64];
        if checked_value!((svc.parse_token)(
            cursor_ptr(cursor),
            file.as_mut_ptr().cast(),
            64
        )) != 0
            && (*vp).snap_buff.num != 0
        {
            let ar = checked_value!((svc.create_slot_file_writer)(file.as_ptr().cast()));
            if !ar.is_null() {
                let res = save_thumbnail(svc, vp, ar);
                // Leaked on fault, as the C++ does (see screen_shot).
                if res.is_ok() {
                    checked_unit!((svc.archive_delete)(ar));
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
        let dest_width = (*vp).snap_u;
        let dest_height = (*vp).snap_v;
        write_bitmap_headers(
            svc,
            ar,
            checked_bitmap_dimension(dest_width)?,
            checked_bitmap_dimension(dest_height)?,
        )?;
        let buf = (*vp).snap_buff.data.cast::<FColor>();
        let mut i = (*vp).snap_v.wrapping_sub(1);
        while i >= 0 {
            let mut j = 0i32;
            while j < (*vp).snap_u {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul((*vp).snap_u).wrapping_add(j) as isize),
                )?;
                j += 1;
            }
            while j < dest_width {
                write_pixel(svc, ar, buf)?;
                j += 1;
            }
            i -= 1;
        }
    }
    Ok(())
}

/// `SAVESNAP` (UnCamera.cpp:874-940) after `ParseCommand`.
unsafe fn save_snap_cmd(
    svc: &CameraServices,
    vp: *mut UViewport,
    cursor: &mut *const c_void,
) -> Res<i32> {
    unsafe {
        let mut file = [0 as TChar; 64];
        if checked_value!((svc.parse_token)(
            cursor_ptr(cursor),
            file.as_mut_ptr().cast(),
            64
        )) != 0
            && (*vp).snap_buff.num != 0
        {
            let ar = checked_value!((svc.create_user_dir_file_writer)(file.as_ptr().cast()));
            if !ar.is_null() {
                let res = save_snap(svc, vp, ar);
                // Leaked on fault, as the C++ does (see screen_shot).
                if res.is_ok() {
                    checked_unit!((svc.archive_delete)(ar));
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
        let dest_width = power_of_two_size(svc, (*vp).snap_u, FAULT_SAVESNAP_WIDTH)?;
        let dest_height = power_of_two_size(svc, (*vp).snap_v, FAULT_SAVESNAP_HEIGHT)?;
        write_bitmap_headers(
            svc,
            ar,
            checked_bitmap_dimension(dest_width)?,
            checked_bitmap_dimension(dest_height)?,
        )?;
        let buf = (*vp).snap_buff.data.cast::<FColor>();
        let mut i = dest_height.wrapping_sub(1);
        while i >= (*vp).snap_v {
            for _ in 0..dest_width {
                write_pixel(svc, ar, buf)?;
            }
            i -= 1;
        }
        i = (*vp).snap_v.wrapping_sub(1);
        while i >= 0 {
            let mut j = 0i32;
            while j < (*vp).snap_u {
                write_pixel(
                    svc,
                    ar,
                    buf.wrapping_offset(i.wrapping_mul((*vp).snap_u).wrapping_add(j) as isize),
                )?;
                j += 1;
            }
            while j < dest_width {
                write_pixel(svc, ar, buf)?;
                j += 1;
            }
            i -= 1;
        }
    }
    Ok(())
}

/// The `widthFactor` / `heightFactor` loops (UnCamera.cpp:884-905): right-shift until zero,
/// then substitute `1 << factor`. `check(factor < 31)` fires only when the check is enabled.
unsafe fn power_of_two_size(svc: &CameraServices, size: i32, fault: i32) -> Res<i32> {
    unsafe {
        let Some((value, factor)) = shared_power_of_two_size(size) else {
            return Err(fault);
        };
        if factor >= 31 && checked_value!((svc.check_enabled)()) != 0 {
            return Err(fault);
        }
        Ok(value)
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
