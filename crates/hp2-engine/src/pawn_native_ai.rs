//! Port of the `ai` group of `Engine/Src/UnPawn.cpp`: `AActor::execPlayerCanSeeMe`
//! and `AActor::TestCanSeeMe`, `APawn::execPickTarget`, `execPickAnyTarget`,
//! `execAddPawn`, `execRemovePawn`, `execStopWaiting`, `execWaitForLanding`,
//! `execPollWaitForLanding` and `execFindStairRotation`.
//!
//! Engine calls (out-of-line or virtual calls, inline reads of fields that are
//! not mirrored here, level-info access) go through [`Hp2PawnNativeAiServices`],
//! filled in by `Hp2PawnNativeAiServices.cpp`. `guard`/`unguard`, `P_GET_*`,
//! `P_FINISH` and `*(T*)Result = ...` stay in the C++ thunks.
//!
//! `FCheckResult Hit(1.f)` is owned by the C++ thunk of `execFindStairRotation`
//! and passed by pointer, so `SingleLineCheck` writes and `Hit.Time` reads see one
//! object, as in the original.
//!
//! FP contraction: [`prim::dot`] is the fused `FVector::operator|` /
//! `SizeSquared` shape. `mul_add` marks the other sites the original object file
//! fuses (`fmadd`). Every other product and sum is a separate rounded operation;
//! `operator*`/`operator+` are inline calls, which clang does not contract across.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, APlayerPawn, FCheckResult, FRotator};

use crate::prim;

/// `ENetMode`: `NM_Standalone` and `NM_Client`.
const NM_STANDALONE: i32 = 0;
const NM_CLIENT: i32 = 3;
/// `ENetRole`: `ROLE_Authority`.
const ROLE_AUTHORITY: u8 = 4;
/// `EPhysics`: `PHYS_Falling`.
const PHYS_FALLING: u8 = 2;
/// Latent action `EPOLL_Sleep`.
const EPOLL_SLEEP: i32 = 384;
/// Latent action `AI_PollWaitForLanding` (`UnPawn.cpp` enum).
const AI_POLL_WAIT_FOR_LANDING: i32 = 528;
/// `TRACE_VisBlocking` (UnLevel.h): `TRACE_Level | TRACE_Movers`.
const TRACE_VIS_BLOCKING: u32 = 0x06;

/// Engine services used by the `ai` group. Filled in by `Hp2PawnNativeAiServices.cpp`.
/// Object arguments are raw engine pointers; `APawn` is its own `AActor` base at
/// offset 0, so either pointer type may be passed where the adapter casts.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeAiServices {
    /// Inline `FRotator::Vector()`; `rot` points at `FRotator`, `out` at `FVector`.
    pub rotation_vector: unsafe extern "C" fn(rot: *const c_void, out: *mut c_void),
    /// `GetStateFrame()->LatentAction`.
    pub state_latent_action: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `GetStateFrame()->LatentAction = action`.
    pub set_state_latent_action: unsafe extern "C" fn(actor: *mut c_void, action: i32),
    /// `GetLevelInfo()->NetMode`.
    pub level_net_mode: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `GetLevelInfo()->PawnList`.
    pub level_pawn_list: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `GetLevelInfo()->PawnList = pawn`.
    pub set_level_pawn_list: unsafe extern "C" fn(level: *mut c_void, pawn: *mut c_void),
    /// `GetLevelInfo()->Game->bTeamGame`.
    pub level_team_game: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Actors.Num()`.
    pub level_actor_count: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Actors(index)`.
    pub level_actor: unsafe extern "C" fn(level: *mut c_void, index: i32) -> *mut c_void,
    /// `Engine->Client->Viewports.Num()`.
    pub viewport_count: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Engine->Client->Viewports(index)->Actor`.
    pub viewport_actor: unsafe extern "C" fn(level: *mut c_void, index: i32) -> *mut c_void,
    /// `bNetTemporary`.
    pub actor_net_temporary: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `bProjTarget`.
    pub actor_proj_target: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `IsA(APawn::StaticClass())`.
    pub actor_is_pawn: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Health`.
    pub pawn_health: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    /// `bBehindView`.
    pub pawn_behind_view: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    /// `PlayerReplicationInfo->Team`.
    pub replication_team: unsafe extern "C" fn(info: *mut c_void) -> i32,
    /// `APawn::LineOfSightTo(other)`: out-of-line member, forwards to pawn sense.
    pub line_of_sight_to: unsafe extern "C" fn(pawn: *mut c_void, other: *mut c_void) -> i32,
    /// `ULevel::SingleLineCheck(hit, source, end, start, trace, extent)`.
    /// `hit` is `FCheckResult`; `end`, `start` and `extent` point at `FVector`.
    pub single_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        source: *mut c_void,
        hit: *mut c_void,
        end: *const c_void,
        start: *const c_void,
        trace: u32,
        extent: *const c_void,
    ),
    /// `eventLongFall()`.
    pub event_long_fall: unsafe extern "C" fn(pawn: *mut c_void),
}

/// `APawn::base` is the first base, so the `AActor` view of `pawn` has the same address.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn actor_of(pawn: *mut APawn) -> *mut AActor {
    unsafe { &raw mut (*pawn).base }
}

/// `FRotator::Vector()` through the service.
///
/// # Safety
/// The service must be a valid table.
unsafe fn rotation_vector(svc: &Hp2PawnNativeAiServices, rot: &FRotator) -> FVector {
    let mut out = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    unsafe {
        (svc.rotation_vector)(
            (rot as *const FRotator).cast(),
            (&mut out as *mut FVector).cast(),
        );
    }
    out
}

/// `origin + scale * dir`: inline `operator*` then `operator+`, so unfused.
fn along(origin: FVector, scale: f32, dir: FVector) -> FVector {
    FVector {
        x: origin.x + scale * dir.x,
        y: origin.y + scale * dir.y,
        z: origin.z + scale * dir.z,
    }
}

/// `spot - FVector(0,0,down)`: inline `operator-`.
fn drop_z(spot: FVector, down: f32) -> FVector {
    FVector {
        x: spot.x - 0.0,
        y: spot.y - 0.0,
        z: spot.z - down,
    }
}

/// `GetLevel()->SingleLineCheck(Hit, this, end, start, TRACE_VisBlocking, extent)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level; `hit` must point to a live `FCheckResult`.
unsafe fn trace(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    hit: *mut FCheckResult,
    end: FVector,
    start: FVector,
    extent: FVector,
) {
    unsafe {
        let level = (*pawn).base.x_level;
        (svc.single_line_check)(
            level,
            pawn.cast(),
            hit.cast(),
            (&end as *const FVector).cast(),
            (&start as *const FVector).cast(),
            TRACE_VIS_BLOCKING,
            (&extent as *const FVector).cast(),
        );
    }
}

/// `AActor::TestCanSeeMe(Viewer)`. `viewer` is null or a live `APlayerPawn`.
///
/// # Safety
/// `actor` must point to a live `AActor`; `viewer` must be null or point to a live `APlayerPawn`.
// The `!(a < b)` / `!(a >= b)` forms are deliberate: C++ treats NaN as "not less" and "not greater-or-equal".
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub unsafe fn test_can_see_me(
    svc: &Hp2PawnNativeAiServices,
    actor: *mut AActor,
    viewer: *mut APlayerPawn,
) -> bool {
    unsafe {
        if viewer.is_null() {
            return false;
        }
        if (*viewer).view_target == actor.cast() {
            return true;
        }
        let delta = prim::sub((*actor).location, (*viewer).base.base.location);
        let dist_sq = prim::dot(delta, delta);
        // `100000.f * (CollisionRadius + 3.6)`: `3.6` is a double literal, so the threshold is double.
        if !((dist_sq as f64) < 100_000.0_f64 * ((*actor).collision_radius as f64 + 3.6_f64)) {
            return false;
        }
        let behind = (svc.pawn_behind_view)(viewer.cast()) != 0;
        if !behind {
            let view = rotation_vector(svc, &(*viewer).base.view_rotation);
            let along_view = prim::dot(view, delta);
            if !(along_view * along_view >= 0.25_f32 * dist_sq) {
                return false;
            }
        }
        (svc.line_of_sight_to)(viewer.cast(), actor.cast()) != 0
    }
}

/// `AActor::execPlayerCanSeeMe`: some viewport or pawn can see `actor`.
///
/// # Safety
/// `actor` must point to a live `AActor` in a live level.
pub unsafe fn player_can_see_me(svc: &Hp2PawnNativeAiServices, actor: *mut AActor) -> bool {
    unsafe {
        let level = (*actor).x_level;
        let net_mode = (svc.level_net_mode)(level);
        if net_mode == NM_STANDALONE
            || (net_mode == NM_CLIENT
                && ((svc.actor_net_temporary)(actor.cast()) != 0
                    || (*actor).role == ROLE_AUTHORITY))
        {
            let mut i = 0;
            while i < (svc.viewport_count)(level) {
                if test_can_see_me(svc, actor, (svc.viewport_actor)(level, i).cast()) {
                    return true;
                }
                i += 1;
            }
            false
        } else {
            let mut next = (svc.level_pawn_list)(level).cast::<APlayerPawn>();
            while !next.is_null() {
                if test_can_see_me(svc, actor, next) {
                    return true;
                }
                next = (*next).base.next_pawn.cast();
            }
            false
        }
    }
}

/// `!PlayerReplicationInfo || !next->PlayerReplicationInfo || !bTeamGame || Team != next->Team`.
///
/// # Safety
/// `pawn` and `next` must point to live `APawn`s in a live level.
unsafe fn team_allows(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn, next: *mut APawn) -> bool {
    unsafe {
        let info = (*pawn).player_replication_info;
        let next_info = (*next).player_replication_info;
        info.is_null()
            || next_info.is_null()
            || (svc.level_team_game)((*pawn).base.x_level) == 0
            || (svc.replication_team)(info) != (svc.replication_team)(next_info)
    }
}

/// Aim test shared by `execPickTarget` and `execPickAnyTarget`. Returns `true`
/// and updates `best_aim`/`best_dist` when `target` is a new best aim.
///
/// # Safety
/// `pawn` and `target` must point to live engine actors; `best_aim` and `best_dist` must be valid.
unsafe fn consider_target(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    target: *mut AActor,
    fire_dir: FVector,
    proj_start: FVector,
    best_aim: *mut f32,
    best_dist: *mut f32,
) -> bool {
    unsafe {
        let delta = prim::sub((*target).location, proj_start);
        let mut new_aim = prim::dot(fire_dir, delta);
        if new_aim > 0.0 {
            let mut fire_dist = prim::dot(delta, delta);
            if fire_dist < 4_000_000.0 {
                // `appSqrt` takes and returns double.
                fire_dist = (fire_dist as f64).sqrt() as f32;
                new_aim /= fire_dist;
                if new_aim > *best_aim && (svc.line_of_sight_to)(pawn.cast(), target.cast()) != 0 {
                    *best_aim = new_aim;
                    *best_dist = fire_dist;
                    return true;
                }
            }
        }
        false
    }
}

/// `APawn::execPickTarget`: the best-aimed visible pawn. Returns null when none.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `best_aim` and `best_dist` must be valid.
pub unsafe fn pick_target(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    best_aim: *mut f32,
    best_dist: *mut f32,
    fire_dir: FVector,
    proj_start: FVector,
) -> *mut APawn {
    unsafe {
        let level = (*pawn).base.x_level;
        let mut pick = null_mut::<APawn>();
        let mut next = (svc.level_pawn_list)(level).cast::<APawn>();
        while !next.is_null() {
            if next != pawn
                && (svc.pawn_health)(next.cast()) > 0
                && (svc.actor_proj_target)(actor_of(next).cast()) != 0
                && team_allows(svc, pawn, next)
                && consider_target(
                    svc,
                    pawn,
                    actor_of(next),
                    fire_dir,
                    proj_start,
                    best_aim,
                    best_dist,
                )
            {
                pick = next;
            }
            next = (*next).next_pawn.cast();
        }
        pick
    }
}

/// `APawn::execPickAnyTarget`: the best-aimed non-pawn projectile target. Returns null when none.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `best_aim` and `best_dist` must be valid.
pub unsafe fn pick_any_target(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    best_aim: *mut f32,
    best_dist: *mut f32,
    fire_dir: FVector,
    proj_start: FVector,
) -> *mut AActor {
    unsafe {
        let level = (*pawn).base.x_level;
        let mut pick = null_mut::<AActor>();
        let mut i = 0;
        while i < (svc.level_actor_count)(level) {
            let next = (svc.level_actor)(level, i).cast::<AActor>();
            if !next.is_null()
                && (svc.actor_proj_target)(next.cast()) != 0
                && (svc.actor_is_pawn)(next.cast()) == 0
                && consider_target(svc, pawn, next, fire_dir, proj_start, best_aim, best_dist)
            {
                pick = next;
            }
            i += 1;
        }
        pick
    }
}

/// `APawn::execAddPawn`: push `pawn` onto the level's pawn list.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level.
pub unsafe fn add_pawn(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) {
    unsafe {
        let level = (*pawn).base.x_level;
        (*pawn).next_pawn = (svc.level_pawn_list)(level);
        (svc.set_level_pawn_list)(level, pawn.cast());
    }
}

/// `APawn::execRemovePawn`: unlink `pawn` from the level's pawn list.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level.
pub unsafe fn remove_pawn(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) {
    unsafe {
        let level = (*pawn).base.x_level;
        let head = (svc.level_pawn_list)(level).cast::<APawn>();
        if head == pawn {
            (svc.set_level_pawn_list)(level, (*head).next_pawn);
        } else {
            let mut next = head;
            while !next.is_null() {
                if (*next).next_pawn == pawn.cast() {
                    (*next).next_pawn = (*pawn).next_pawn;
                    break;
                }
                next = (*next).next_pawn.cast();
            }
        }
    }
}

/// `APawn::execStopWaiting`: a sleeping latent action is cancelled.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn stop_waiting(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) {
    unsafe {
        if (svc.state_latent_action)(actor_of(pawn).cast()) == EPOLL_SLEEP {
            (*actor_of(pawn)).latent_float = -1.0;
        }
    }
}

/// `APawn::execWaitForLanding`: arm the 2.5 s landing timer and poll while falling.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn wait_for_landing(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) {
    unsafe {
        let actor = actor_of(pawn);
        (*actor).latent_float = 2.5;
        if (*actor).physics == PHYS_FALLING {
            (svc.set_state_latent_action)(actor.cast(), AI_POLL_WAIT_FOR_LANDING);
        }
    }
}

/// `APawn::execPollWaitForLanding` body. `result` is the thunk's `Result`; the
/// `FLOAT` delta is read from it only on the falling path, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `result` must point to a `FLOAT` when the actor is falling.
pub unsafe fn poll_wait_for_landing(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    result: *const f32,
) {
    unsafe {
        let actor = actor_of(pawn);
        if (*actor).physics != PHYS_FALLING {
            (svc.set_state_latent_action)(actor.cast(), 0);
        } else {
            (*actor).latent_float -= *result;
            if (*actor).latent_float < 0.0 {
                (svc.event_long_fall)(pawn.cast());
            }
        }
    }
}

/// `APawn::execFindStairRotation` body, returning the pitch to ease toward.
/// `hit` is the thunk's `FCheckResult Hit(1.f)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level; `hit` must point to a live `FCheckResult`.
pub unsafe fn find_stair_rotation(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    delta_time: f32,
    hit: *mut FCheckResult,
) -> i32 {
    unsafe {
        if delta_time > 0.33_f32 {
            return (*pawn).view_rotation.pitch;
        }
        if (*pawn).view_rotation.pitch > 32768 {
            (*pawn).view_rotation.pitch = ((*pawn).view_rotation.pitch & 65535) - 65536;
        }
        let pitch = (*pawn).view_rotation.pitch;
        let actor = actor_of(pawn);
        let location = (*actor).location;
        let base_eye = (*pawn).base_eye_height;
        let height = (*actor).collision_height + base_eye;
        let radius = (*actor).collision_radius;
        let slice = FVector {
            x: radius,
            y: radius,
            z: 1.0,
        };
        let no_extent = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let look = FRotator {
            pitch: 0,
            yaw: (*pawn).view_rotation.yaw,
            roll: (*pawn).view_rotation.roll,
        };
        let dir = rotation_vector(svc, &look);
        let eye = FVector {
            x: location.x + 0.0,
            y: location.y + 0.0,
            z: location.z + base_eye,
        };

        trace(svc, pawn, hit, along(eye, 2.0 * height, dir), eye, slice);
        let dist = 2.0 * height * (*hit).time;
        let mut stair_rot: i32 = 0;
        if (dist as f64) > 0.8_f64 * (height as f64) {
            let spot = along(eye, 0.5 * dist, dir);
            let down = 3.0 * height;
            trace(svc, pawn, hit, drop_z(spot, down), spot, slice);
            if (*hit).time < 1.0 {
                let first_down = down * (*hit).time;
                if first_down < height.mul_add(0.7, -6.0) {
                    let spot = along(eye, dist, dir);
                    trace(svc, pawn, hit, drop_z(spot, down), spot, slice);
                    stair_rot = pitch.max(0);
                    if down * (*hit).time < first_down - 10.0 {
                        stair_rot = 5400;
                    }
                } else if first_down > height.mul_add(0.7, 6.0) {
                    trace(
                        svc,
                        pawn,
                        hit,
                        along(location, 0.9 * dist, dir),
                        location,
                        no_extent,
                    );
                    if (*hit).time == 1.0 {
                        let spot = along(eye, dist, dir);
                        trace(svc, pawn, hit, drop_z(spot, down), spot, slice);
                        stair_rot = pitch.min(0);
                        if down * (*hit).time > first_down + 10.0 {
                            stair_rot = -5000;
                        }
                    }
                }
            }
        }

        // `Abs(ViewRotation.Pitch - stairRot)`; the subtraction wraps, as clang emits it.
        let diff = {
            let d = pitch.wrapping_sub(stair_rot);
            if d >= 0 { d } else { d.wrapping_neg() }
        };
        if diff > 0 {
            let mut rot_rate: f32 = 8.0;
            if diff < 1000 {
                rot_rate = (8000 / diff) as f32;
            }
            let scaled = rot_rate * delta_time;
            // `::Min(1.f, RotRate*deltaTime)`: `fcsel ge`, so NaN keeps `scaled`.
            rot_rate = if 1.0 <= scaled { 1.0 } else { scaled };
            stair_rot = (pitch as f32).mul_add(1.0 - rot_rate, stair_rot as f32 * rot_rate) as i32;
        }
        stair_rot
    }
}
