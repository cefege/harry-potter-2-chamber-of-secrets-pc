//! Level-actor helper exports (`crates/hp2-engine/src/levact.rs`). The C++
//! free functions in `Engine/Src/UnLevAct.cpp` forward here. `FVector`
//! crosses as `void*`: the C++ side passes a pointer to its own `FVector`.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{FCheckResult, FRotator};
use hp2_engine::levact;
use hp2_ffi_support::guard;

/// Reads an `FVector` by value; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `FVector`.
unsafe fn read_vec(p: *const c_void) -> Option<FVector> {
    unsafe { p.cast::<FVector>().as_ref().copied() }
}

/// Builds the level context. `svc` is the C++ `Hp2LevActServices` table, `gmem`
/// is the global `FMemStack`, `hash` is the `FCollisionHashBase*` (may be null),
/// and `ulevel` is the owning `ULevel*`. Paths that read the actor array
/// (`level_info`, `find_spot`, `set_actor_zone`, `move_actor`,
/// `check_encroachment`) need a non-null `ulevel`; the four check exports pass
/// null because their paths never read it.
///
/// # Safety
/// `svc` must point to a live `LevActServices`, `gmem` must be a live `FMemStack`,
/// and `ulevel`, when non-null, must be a live `ULevel`.
unsafe fn level_ctx<'a>(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
) -> Option<levact::LevelCtx<'a>> {
    unsafe {
        let svc = svc.cast::<levact::LevActServices>().as_ref()?;
        if gmem.is_null() {
            return None;
        }
        Some(levact::LevelCtx {
            svc,
            gmem,
            hash,
            ulevel: ulevel.cast(),
        })
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
                level_ctx(svc, gmem, hash, std::ptr::null_mut()),
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
                level_ctx(svc, gmem, hash, std::ptr::null_mut()),
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
                level_ctx(svc, gmem, hash, std::ptr::null_mut()),
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
                level_ctx(svc, gmem, hash, std::ptr::null_mut()),
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

/// `ULevel::AdjustSpot` body. Updates `adjusted` and `hit` in place.
///
/// # Safety
/// See [`level_ctx`]. `adjusted` and `hit` must be valid `FVector` and
/// `FCheckResult` pointers; `trace_dest` must be a valid `FVector` pointer.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_adjust_spot(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    adjusted: *mut c_void,
    trace_dest: *const c_void,
    trace_len: f32,
    hit: *mut c_void,
) {
    guard("hp2_rs_engine_levact_adjust_spot", (), move || unsafe {
        let (Some(ctx), Some(adjusted), Some(dest), Some(hit)) = (
            level_ctx(svc, gmem, hash, ulevel),
            adjusted.cast::<FVector>().as_mut(),
            read_vec(trace_dest),
            hit.cast::<FCheckResult>().as_mut(),
        ) else {
            return;
        };
        levact::adjust_spot(ctx, adjusted, dest, trace_len, hit);
    })
}

/// `ULevel::FindSpot` body. Writes `location` on success. Fallback: `0`.
///
/// # Safety
/// See [`level_ctx`]. `extent` and `location` must be valid `FVector` pointers.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_find_spot(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    extent: *const c_void,
    location: *mut c_void,
    check_actors: i32,
    assume_fit: i32,
) -> i32 {
    guard("hp2_rs_engine_levact_find_spot", 0, move || unsafe {
        let (Some(ctx), Some(ext), Some(loc)) = (
            level_ctx(svc, gmem, hash, ulevel),
            read_vec(extent),
            location.cast::<FVector>().as_mut(),
        ) else {
            return 0;
        };
        i32::from(levact::find_spot(
            ctx,
            ext,
            loc,
            check_actors != 0,
            assume_fit != 0,
        ))
    })
}

/// `ULevel::FarMoveActor` body. Fallback: `0`.
///
/// # Safety
/// See [`level_ctx`]. `actor` must be a live `AActor`; `destination` must be a
/// valid `FVector` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_far_move_actor(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    actor: *mut c_void,
    destination: *const c_void,
    test: i32,
    no_check: i32,
) -> i32 {
    guard("hp2_rs_engine_levact_far_move_actor", 0, move || unsafe {
        let (Some(ctx), Some(dest)) = (level_ctx(svc, gmem, hash, ulevel), read_vec(destination))
        else {
            return 0;
        };
        i32::from(levact::far_move_actor(
            ctx,
            actor,
            dest,
            test != 0,
            no_check != 0,
        ))
    })
}

/// `ULevel::MoveActor` body. Writes `hit`. Fallback: `0`.
///
/// # Safety
/// See [`level_ctx`]. `actor` must be a live `AActor`; `delta`, `new_rotation`
/// and `hit` must be valid `FVector`, `FRotator` and `FCheckResult` pointers.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_move_actor(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    actor: *mut c_void,
    delta: *const c_void,
    new_rotation: *const c_void,
    hit: *mut c_void,
    test: i32,
    ignore_pawns: i32,
    ignore_bases: i32,
    no_fail: i32,
) -> i32 {
    guard("hp2_rs_engine_levact_move_actor", 0, move || unsafe {
        let (Some(ctx), Some(delta), Some(rot), Some(hit)) = (
            level_ctx(svc, gmem, hash, ulevel),
            read_vec(delta),
            new_rotation.cast::<FRotator>().as_ref().copied(),
            hit.cast::<FCheckResult>().as_mut(),
        ) else {
            return 0;
        };
        i32::from(levact::move_actor(
            ctx,
            actor,
            delta,
            rot,
            hit,
            test != 0,
            ignore_pawns != 0,
            ignore_bases != 0,
            no_fail != 0,
        ))
    })
}

/// `ULevel::CheckEncroachment` body. Fallback: `0` (allow the move).
///
/// # Safety
/// See [`level_ctx`]. `actor` must be a live `AActor`; `test_location` and
/// `test_rotation` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_check_encroachment(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    actor: *mut c_void,
    test_location: *const c_void,
    test_rotation: *const c_void,
    touch_notify: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_levact_check_encroachment",
        0,
        move || unsafe {
            let (Some(ctx), Some(loc), Some(rot)) = (
                level_ctx(svc, gmem, hash, ulevel),
                read_vec(test_location),
                test_rotation.cast::<FRotator>().as_ref().copied(),
            ) else {
                return 0;
            };
            i32::from(levact::check_encroachment(
                ctx,
                actor,
                loc,
                rot,
                touch_notify != 0,
            ))
        },
    )
}

/// `ULevel::SetActorZone` body.
///
/// # Safety
/// See [`level_ctx`]. `actor` must be a live `AActor`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_levact_set_actor_zone(
    svc: *const c_void,
    gmem: *mut c_void,
    hash: *mut c_void,
    ulevel: *mut c_void,
    actor: *mut c_void,
    test: i32,
    force_refresh: i32,
) {
    guard("hp2_rs_engine_levact_set_actor_zone", (), move || unsafe {
        if let Some(ctx) = level_ctx(svc, gmem, hash, ulevel) {
            levact::set_actor_zone(ctx, actor, test != 0, force_refresh != 0);
        }
    })
}
