//! Level-actor helper exports (`crates/hp2-engine/src/levact.rs`). The C++
//! free functions in `Engine/Src/UnLevAct.cpp` forward here. `FVector`
//! crosses as `void*`: the C++ side passes a pointer to its own `FVector`.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::FCheckResult;
use hp2_engine::levact;
use hp2_ffi_support::guard;

/// Reads an `FVector` by value; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `FVector`.
unsafe fn read_vec(p: *const c_void) -> Option<FVector> {
    unsafe { p.cast::<FVector>().as_ref().copied() }
}

/// Writes `v` through `out`; a null `out` is a no-op.
///
/// # Safety
/// `out`, when non-null, must be valid for writes of an `FVector`.
unsafe fn write_vec(out: *mut c_void, v: FVector) {
    unsafe {
        if let Some(dst) = out.cast::<FVector>().as_mut() {
            *dst = v;
        }
    }
}

/// `FindDeltaFromRotationYaw(origin, point, angle)` writing into `out`.
///
/// # Safety
/// `origin`, `point` and `out` must be valid `FVector` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_find_delta_from_rotation_yaw(
    origin: *const c_void,
    point: *const c_void,
    angle: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_levact_find_delta_from_rotation_yaw",
        (),
        move || unsafe {
            if let (Some(o), Some(p)) = (read_vec(origin), read_vec(point)) {
                write_vec(out, levact::find_delta_from_rotation_yaw(o, p, angle));
            }
        },
    )
}

/// `FindDeltaFromRotationRoll(origin, point, angle)` writing into `out`.
///
/// # Safety
/// `origin`, `point` and `out` must be valid `FVector` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_find_delta_from_rotation_roll(
    origin: *const c_void,
    point: *const c_void,
    angle: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_levact_find_delta_from_rotation_roll",
        (),
        move || unsafe {
            if let (Some(o), Some(p)) = (read_vec(origin), read_vec(point)) {
                write_vec(out, levact::find_delta_from_rotation_roll(o, p, angle));
            }
        },
    )
}

/// `FindDeltaFromRotationPitch(origin, point, angle)` writing into `out`.
///
/// # Safety
/// `origin`, `point` and `out` must be valid `FVector` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_find_delta_from_rotation_pitch(
    origin: *const c_void,
    point: *const c_void,
    angle: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_levact_find_delta_from_rotation_pitch",
        (),
        move || unsafe {
            if let (Some(o), Some(p)) = (read_vec(origin), read_vec(point)) {
                write_vec(out, levact::find_delta_from_rotation_pitch(o, p, angle));
            }
        },
    )
}

/// `FindDeltaFromRotation(origin, point, yaw, roll, pitch)` writing into `out`.
///
/// # Safety
/// `origin`, `point` and `out` must be valid `FVector` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_find_delta_from_rotation(
    origin: *const c_void,
    point: *const c_void,
    yaw: i32,
    roll: i32,
    pitch: i32,
    out: *mut c_void,
) {
    guard(
        "hp2_rs_engine_levact_find_delta_from_rotation",
        (),
        move || unsafe {
            if let (Some(o), Some(p)) = (read_vec(origin), read_vec(point)) {
                write_vec(
                    out,
                    levact::find_delta_from_rotation(o, p, yaw, roll, pitch),
                );
            }
        },
    )
}

/// `DoMultiLineCheck`: `is_mover` is the `IsA(AMover)` result; `encroach_type`
/// is read by the C++ shell only when `is_mover` is set. Returns the UBOOL.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_levact_do_multi_line_check(
    is_mover: i32,
    encroach_type: u8,
) -> i32 {
    guard("hp2_rs_engine_levact_do_multi_line_check", 1, move || {
        i32::from(levact::do_multi_line_check(is_mover != 0, encroach_type))
    })
}

/// Builds the level context. `svc` is the C++ `Hp2LevActServices` table, `gmem`
/// is the global `FMemStack`, and `hash` is the `FCollisionHashBase*` (may be null).
///
/// # Safety
/// `svc` must point to a live `LevActServices`, and `gmem` must be a live `FMemStack`.
unsafe fn level_ctx<'a>(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
) -> Option<levact::LevelCtx<'a>> {
    unsafe {
        let svc = svc.cast::<levact::LevActServices>().as_ref()?;
        if gmem.is_null() {
            return None;
        }
        Some(levact::LevelCtx { svc, gmem, hash })
    }
}

/// `ULevel::SinglePointCheck` body. The C++ shell owns the `FMemMark`. Returns
/// `1` when there is no hit (`UBOOL`) and `0` on a hit. Fallback: `1`.
///
/// # Safety
/// See [`level_ctx`]. `hit` must be a valid `FCheckResult*`, `level` may be null,
/// and `location`/`extent` must be valid `FVector` pointers.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_single_point_check(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    hit: *mut c_void,
    level: *mut c_void,
    location: *const c_void,
    extent: *const c_void,
    flags: u32,
    check_actors: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_levact_single_point_check",
        1,
        move || unsafe {
            let (Some(ctx), Some(hit), Some(loc), Some(ext)) = (
                level_ctx(svc, gmem, hash),
                hit.cast::<FCheckResult>().as_mut(),
                read_vec(location),
                read_vec(extent),
            ) else {
                return 1;
            };
            i32::from(levact::single_point_check(
                ctx,
                hit,
                level,
                loc,
                ext,
                flags,
                check_actors != 0,
            ))
        },
    )
}

/// `ULevel::MultiPointCheck` body. Returns the result list (`FCheckResult*`
/// chain allocated from `mem`/`GMem` as in C++), or null. Fallback: null.
///
/// # Safety
/// See [`level_ctx`]. `mem` must be a live `FMemStack`; `level` may be null.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_multi_point_check(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    mem: *mut c_void,
    level: *mut c_void,
    location: *const c_void,
    extent: *const c_void,
    flags: u32,
    check_actors: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_levact_multi_point_check",
        std::ptr::null_mut(),
        move || unsafe {
            let (Some(ctx), Some(loc), Some(ext)) = (
                level_ctx(svc, gmem, hash),
                read_vec(location),
                read_vec(extent),
            ) else {
                return std::ptr::null_mut();
            };
            levact::multi_point_check(ctx, mem, level, loc, ext, flags, check_actors != 0).cast()
        },
    )
}

/// `ULevel::SingleLineCheck` body. The C++ shell owns the `FMemMark`. Returns
/// `1` when nothing blocks (`UBOOL`), `0` otherwise. Fallback: `1`.
///
/// # Safety
/// See [`level_ctx`]. `hit` must be a valid `FCheckResult*`; `source` and
/// `level_info` may be null; `end`/`start`/`extent` must be valid `FVector` pointers.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_single_line_check(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    hit: *mut c_void,
    source: *mut c_void,
    level_info: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
    trace_flags: u32,
    extra: u8,
) -> i32 {
    guard(
        "hp2_rs_engine_levact_single_line_check",
        1,
        move || unsafe {
            let (Some(ctx), Some(hit), Some(end), Some(start), Some(extent)) = (
                level_ctx(svc, gmem, hash),
                hit.cast::<FCheckResult>().as_mut(),
                read_vec(end),
                read_vec(start),
                read_vec(extent),
            ) else {
                return 1;
            };
            let seg = levact::LineQuery { end, start, extent };
            i32::from(levact::single_line_check(
                ctx,
                hit,
                source,
                level_info,
                seg,
                trace_flags,
                extra,
            ))
        },
    )
}

/// `ULevel::MultiLineCheck` body. Returns the sorted result list allocated from
/// `mem`, or null when there are no hits. Fallback: null.
///
/// # Safety
/// See [`level_ctx`]. `mem` must be a live `FMemStack`; `level` may be null;
/// `end`/`start`/`extent` must be valid `FVector` pointers.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_multi_line_check(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    mem: *mut c_void,
    level: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
    check_actors: i32,
    extra: u8,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_levact_multi_line_check",
        std::ptr::null_mut(),
        move || unsafe {
            let (Some(ctx), Some(end), Some(start), Some(extent)) = (
                level_ctx(svc, gmem, hash),
                read_vec(end),
                read_vec(start),
                read_vec(extent),
            ) else {
                return std::ptr::null_mut();
            };
            let seg = levact::LineQuery { end, start, extent };
            levact::multi_line_check(ctx, mem, level, seg, check_actors != 0, u32::from(extra))
                .cast()
        },
    )
}
