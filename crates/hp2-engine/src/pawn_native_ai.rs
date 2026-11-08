//! Port of the `ai` group of `Engine/Src/UnPawn.cpp`: `AActor::execPlayerCanSeeMe`
//! and `AActor::TestCanSeeMe`, `APawn::execPickTarget`, `execPickAnyTarget`,
//! `execAddPawn`, `execRemovePawn`, `execStopWaiting`, `execWaitForLanding`,
//! `execPollWaitForLanding`, `execFindStairRotation`, `execCanSee`,
//! `execLineOfSightTo` and `ANavigationPoint::execdescribeSpec`.
//!
//! Engine calls (out-of-line or virtual calls, inline reads of fields that are
//! not mirrored here, level-info access) go through [`Hp2PawnNativeAiServices`],
//! filled in by `Hp2PawnNativeAiServices.cpp`, published once
//! (`crates/hp2-ffi/src/engine_pawn_native_ai.rs`'s
//! `hp2_rs_engine_pawn_native_ai_publish_services`) and read per call through
//! [`published_services`]. `guard`/`unguard`, `P_GET_*`/`P_FINISH` argument
//! unpacking and `*(T*)Result = ...` now live in the Rust native handlers
//! (`engine_natives_pawn_ai.rs`), not a C++ thunk.
//!
//! `FCheckResult Hit(1.f)` (`execFindStairRotation`'s local) is now owned by its
//! Rust handler and passed by pointer, so `SingleLineCheck` writes and `Hit.Time`
//! reads see one object, as in the original.
//!
//! FP contraction: [`prim::dot`] is the fused `FVector::operator|` /
//! `SizeSquared` shape. `mul_add` marks the other sites the original object file
//! fuses (`fmadd`). Every other product and sum is a separate rounded operation;
//! `operator*`/`operator+` are inline calls, which clang does not contract across.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, APlayerPawn, FCheckResult, FRotator};

use crate::pawn::{PawnResult, checked};
use crate::prim;

// Read native outputs only after their callback has succeeded.
macro_rules! native_value {
    ($service:expr, $($arg:expr),* $(,)?) => {{
        let mut value = std::mem::MaybeUninit::uninit();
        checked(($service)($($arg,)* value.as_mut_ptr()))?;
        value.assume_init()
    }};
}

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

/// Checked native services: zero status succeeds, values use final output pointers.
/// Native adapters catch before returning through Rust, and public wrappers rethrow
/// only after all Rust frames have returned. Object arguments remain raw pointers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeAiServices {
    /// Inline `FRotator::Vector()`; `rot` points at `FRotator`, `out` at `FVector`.
    pub rotation_vector: unsafe extern "C" fn(rot: *const c_void, out: *mut c_void) -> i32,
    /// `GetStateFrame()->LatentAction`.
    pub state_latent_action: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `GetStateFrame()->LatentAction = action`.
    pub set_state_latent_action: unsafe extern "C" fn(actor: *mut c_void, action: i32) -> i32,
    /// `GetLevelInfo()->NetMode`.
    pub level_net_mode: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    /// `GetLevelInfo()->PawnList`.
    pub level_pawn_list: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `GetLevelInfo()->PawnList = pawn`.
    pub set_level_pawn_list: unsafe extern "C" fn(level: *mut c_void, pawn: *mut c_void) -> i32,
    /// `GetLevelInfo()->Game->bTeamGame`.
    pub level_team_game: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    /// `Actors.Num()`.
    pub level_actor_count: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    /// `Actors(index)`.
    pub level_actor:
        unsafe extern "C" fn(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    /// `Engine->Client->Viewports.Num()`.
    pub viewport_count: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    /// `Engine->Client->Viewports(index)->Actor`.
    pub viewport_actor:
        unsafe extern "C" fn(level: *mut c_void, index: i32, out: *mut *mut c_void) -> i32,
    /// `bNetTemporary`.
    pub actor_net_temporary: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `bProjTarget`.
    pub actor_proj_target: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `IsA(APawn::StaticClass())`.
    pub actor_is_pawn: unsafe extern "C" fn(actor: *mut c_void, out: *mut i32) -> i32,
    /// `Health`.
    pub pawn_health: unsafe extern "C" fn(pawn: *mut c_void, out: *mut i32) -> i32,
    /// `bBehindView`.
    pub pawn_behind_view: unsafe extern "C" fn(pawn: *mut c_void, out: *mut i32) -> i32,
    /// `PlayerReplicationInfo->Team`.
    pub replication_team: unsafe extern "C" fn(info: *mut c_void, out: *mut i32) -> i32,
    /// `APawn::LineOfSightTo(other, show_self)`: out-of-line member, forwards to
    /// pawn sense. `show_self` is the C++ `bShowSelf` default-argument value, passed
    /// explicitly since the leaf has no default.
    pub line_of_sight_to: unsafe extern "C" fn(
        pawn: *mut c_void,
        other: *mut c_void,
        show_self: i32,
        out: *mut i32,
    ) -> i32,
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
    ) -> i32,
    /// `eventLongFall()`.
    pub event_long_fall: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    /// Public `AActor::TestCanSeeMe(viewer)`, retaining its nested native guard.
    pub test_can_see_me:
        unsafe extern "C" fn(actor: *mut c_void, viewer: *mut c_void, out: *mut i32) -> i32,
    /// `GetLevel()->ReachSpecs.Num()`.
    pub reach_spec_count: unsafe extern "C" fn(level: *mut c_void, out: *mut i32) -> i32,
    /// `GetLevel()->ReachSpecs(index)`: `start`/`end` receive `FReachSpec::Start`/`End`,
    /// `reach_flags`/`distance` receive `reachFlags`/`distance`. `CollisionRadius`,
    /// `CollisionHeight` and `bPruned` are not read by `describeSpec`.
    pub reach_spec_at: unsafe extern "C" fn(
        level: *mut c_void,
        index: i32,
        start: *mut *mut c_void,
        end: *mut *mut c_void,
        reach_flags: *mut i32,
        distance: *mut i32,
    ) -> i32,
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
unsafe fn rotation_vector(
    svc: &Hp2PawnNativeAiServices,
    rot: *const FRotator,
) -> PawnResult<FVector> {
    let mut out = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    unsafe {
        checked((svc.rotation_vector)(rot.cast(), (&raw mut out).cast()))?;
    }
    Ok(out)
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
) -> PawnResult<()> {
    unsafe {
        let level = (*pawn).base.x_level;
        checked((svc.single_line_check)(
            level,
            pawn.cast(),
            hit.cast(),
            (&end as *const FVector).cast(),
            (&start as *const FVector).cast(),
            TRACE_VIS_BLOCKING,
            (&extent as *const FVector).cast(),
        ))?;
        Ok(())
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
) -> PawnResult<bool> {
    unsafe {
        if viewer.is_null() {
            return Ok(false);
        }
        if (*viewer).view_target == actor.cast() {
            return Ok(true);
        }
        let delta = prim::sub((*actor).location, (*viewer).base.base.location);
        let dist_sq = prim::dot(delta, delta);
        // `100000.f * (CollisionRadius + 3.6)`: `3.6` is a double literal, so the threshold is double.
        if !((dist_sq as f64) < 100_000.0_f64 * ((*actor).collision_radius as f64 + 3.6_f64)) {
            return Ok(false);
        }
        let behind = native_value!(svc.pawn_behind_view, viewer.cast()) != 0;
        if !behind {
            let view = rotation_vector(svc, &raw const (*viewer).base.view_rotation)?;
            let along_view = prim::dot(view, delta);
            if !(along_view * along_view >= 0.25_f32 * dist_sq) {
                return Ok(false);
            }
        }
        Ok(native_value!(svc.line_of_sight_to, viewer.cast(), actor.cast(), 0) != 0)
    }
}

/// `AActor::execPlayerCanSeeMe`: some viewport or pawn can see `actor`.
///
/// # Safety
/// `actor` must point to a live `AActor` in a live level.
pub unsafe fn player_can_see_me(
    svc: &Hp2PawnNativeAiServices,
    actor: *mut AActor,
) -> PawnResult<bool> {
    unsafe {
        let level = (*actor).x_level;
        let net_mode = native_value!(svc.level_net_mode, level);
        if net_mode == NM_STANDALONE
            || (net_mode == NM_CLIENT
                && (native_value!(svc.actor_net_temporary, actor.cast()) != 0
                    || (*actor).role == ROLE_AUTHORITY))
        {
            let mut i = 0;
            while i < native_value!(svc.viewport_count, (*actor).x_level) {
                if native_value!(
                    svc.test_can_see_me,
                    actor.cast(),
                    native_value!(svc.viewport_actor, (*actor).x_level, i)
                ) != 0
                {
                    return Ok(true);
                }
                i += 1;
            }
            Ok(false)
        } else {
            let mut next = native_value!(svc.level_pawn_list, level).cast::<APlayerPawn>();
            while !next.is_null() {
                if native_value!(svc.test_can_see_me, actor.cast(), next.cast()) != 0 {
                    return Ok(true);
                }
                next = (*next).base.next_pawn.cast();
            }
            Ok(false)
        }
    }
}

/// Live queries needed by target selection; canonical hosts do not publish a native table.
///
/// # Safety
/// Actor/level identities and referenced outputs remain live through synchronous reentry.
pub trait TargetOps {
    type Error;
    unsafe fn pawn_list(&self, level: *mut c_void) -> Result<*mut APawn, Self::Error>;
    unsafe fn team_game(&self, level: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn team(&self, info: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn health(&self, pawn: *mut APawn) -> Result<i32, Self::Error>;
    unsafe fn projectile_target(&self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn is_pawn(&self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn line_of_sight(&self, pawn: *mut APawn, target: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn actor_count(&self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_at(&self, level: *mut c_void, index: i32) -> Result<*mut AActor, Self::Error>;
}

impl TargetOps for Hp2PawnNativeAiServices {
    type Error = i32;
    unsafe fn pawn_list(&self, level: *mut c_void) -> PawnResult<*mut APawn> {
        unsafe { Ok(native_value!(self.level_pawn_list, level).cast()) }
    }
    unsafe fn team_game(&self, level: *mut c_void) -> PawnResult<bool> {
        unsafe { Ok(native_value!(self.level_team_game, level) != 0) }
    }
    unsafe fn team(&self, info: *mut c_void) -> PawnResult<i32> {
        unsafe { Ok(native_value!(self.replication_team, info)) }
    }
    unsafe fn health(&self, pawn: *mut APawn) -> PawnResult<i32> {
        unsafe { Ok(native_value!(self.pawn_health, pawn.cast())) }
    }
    unsafe fn projectile_target(&self, actor: *mut AActor) -> PawnResult<bool> {
        unsafe { Ok(native_value!(self.actor_proj_target, actor.cast()) != 0) }
    }
    unsafe fn is_pawn(&self, actor: *mut AActor) -> PawnResult<bool> {
        unsafe { Ok(native_value!(self.actor_is_pawn, actor.cast()) != 0) }
    }
    unsafe fn line_of_sight(&self, pawn: *mut APawn, target: *mut AActor) -> PawnResult<bool> {
        unsafe { Ok(native_value!(self.line_of_sight_to, pawn.cast(), target.cast(), 0) != 0) }
    }
    unsafe fn actor_count(&self, level: *mut c_void) -> PawnResult<i32> {
        unsafe { Ok(native_value!(self.level_actor_count, level)) }
    }
    unsafe fn actor_at(&self, level: *mut c_void, index: i32) -> PawnResult<*mut AActor> {
        unsafe { Ok(native_value!(self.level_actor, level, index).cast()) }
    }
}

/// `!PlayerReplicationInfo || !next->PlayerReplicationInfo || !bTeamGame || Team != next->Team`.
///
/// # Safety
/// `pawn` and `next` must point to live `APawn`s in a live level.
unsafe fn team_allows<O: TargetOps>(
    svc: &O,
    pawn: *mut APawn,
    next: *mut APawn,
) -> Result<bool, O::Error> {
    unsafe {
        let info = (*pawn).player_replication_info;
        let next_info = (*next).player_replication_info;
        Ok(info.is_null()
            || next_info.is_null()
            || !svc.team_game((*pawn).base.x_level)?
            || svc.team(info)? != svc.team(next_info)?)
    }
}

/// Aim test shared by `execPickTarget` and `execPickAnyTarget`. Returns `true`
/// and updates `best_aim`/`best_dist` when `target` is a new best aim.
///
/// # Safety
/// `pawn` and `target` must point to live engine actors; `best_aim` and `best_dist` must be valid.
unsafe fn consider_target<O: TargetOps>(
    svc: &O,
    pawn: *mut APawn,
    target: *mut AActor,
    fire_dir: FVector,
    proj_start: FVector,
    best_aim: *mut f32,
    best_dist: *mut f32,
) -> Result<bool, O::Error> {
    unsafe {
        let delta = prim::sub((*target).location, proj_start);
        let mut new_aim = prim::dot(fire_dir, delta);
        if new_aim > 0.0 {
            let mut fire_dist = prim::dot(delta, delta);
            if fire_dist < 4_000_000.0 {
                // `appSqrt` takes and returns double.
                fire_dist = (fire_dist as f64).sqrt() as f32;
                new_aim /= fire_dist;
                if new_aim > *best_aim
                    && svc.line_of_sight(pawn, target)?
                {
                    *best_aim = new_aim;
                    *best_dist = fire_dist;
                    return Ok(true);
                }
            }
        }
        Ok(false)
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
) -> PawnResult<*mut APawn> {
    unsafe { pick_target_with(svc, pawn, best_aim, best_dist, fire_dir, proj_start) }
}

/// Canonical/source-shared PickTarget policy.
///
/// # Safety
/// Same live actor/output contract as `pick_target`; query operations are genuine.
pub unsafe fn pick_target_with<O: TargetOps>(
    svc: &O, pawn: *mut APawn, best_aim: *mut f32, best_dist: *mut f32,
    fire_dir: FVector, proj_start: FVector,
) -> Result<*mut APawn, O::Error> {
    unsafe {
        let level = (*pawn).base.x_level;
        let mut pick = null_mut::<APawn>();
        let mut next = svc.pawn_list(level)?;
        while !next.is_null() {
            if next != pawn
                && svc.health(next)? > 0
                && svc.projectile_target(actor_of(next))?
                && team_allows(svc, pawn, next)?
                && consider_target(
                    svc,
                    pawn,
                    actor_of(next),
                    fire_dir,
                    proj_start,
                    best_aim,
                    best_dist,
                )?
            {
                pick = next;
            }
            next = (*next).next_pawn.cast();
        }
        Ok(pick)
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
) -> PawnResult<*mut AActor> {
    unsafe { pick_any_target_with(svc, pawn, best_aim, best_dist, fire_dir, proj_start) }
}

/// Canonical/source-shared PickAnyTarget policy.
///
/// # Safety
/// Same live actor/output contract as `pick_any_target`; query operations are genuine.
pub unsafe fn pick_any_target_with<O: TargetOps>(
    svc: &O, pawn: *mut APawn, best_aim: *mut f32, best_dist: *mut f32,
    fire_dir: FVector, proj_start: FVector,
) -> Result<*mut AActor, O::Error> {
    unsafe {
        let mut pick = null_mut::<AActor>();
        let mut i = 0;
        while i < svc.actor_count((*pawn).base.x_level)? {
            let next = svc.actor_at((*pawn).base.x_level, i)?;
            if !next.is_null()
                && svc.projectile_target(next)?
                && !svc.is_pawn(next)?
                && consider_target(svc, pawn, next, fire_dir, proj_start, best_aim, best_dist)?
            {
                pick = next;
            }
            i += 1;
        }
        Ok(pick)
    }
}

/// `APawn::execAddPawn`: push `pawn` onto the level's pawn list.
/// UE1 never checked for an existing membership, so a pawn already in the list was
/// pushed again, linking the list into a cycle that hung every later walk (the
/// harness fixture records that UE1 behavior). Rust policy (LeadFindings
/// "Duplicate AddPawn"): a pawn that is already listed is a no-op.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level pawn list.
pub unsafe fn add_pawn(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe {
        add_pawn_with(pawn, |level| Ok(native_value!(svc.level_pawn_list, level)),
            |level, head| checked((svc.set_level_pawn_list)(level, head)))
    }
}

/// Source AddPawn policy with canonical or SDK list publication.
///
/// # Safety
/// Pawn and every reached list node remain live; callbacks implement the actual level list.
pub unsafe fn add_pawn_with<E>(
    pawn: *mut APawn,
    read_head: impl FnOnce(*mut c_void) -> Result<*mut c_void, E>,
    write_head: impl FnOnce(*mut c_void, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let level = (*pawn).base.x_level;
        let head = read_head(level)?;
        let mut next = head.cast::<APawn>();
        while !next.is_null() {
            if next == pawn {
                return Ok(());
            }
            next = (*next).next_pawn.cast();
        }
        (*pawn).next_pawn = head;
        write_head(level, pawn.cast())
    }
}

/// `APawn::execRemovePawn`: unlink `pawn` from the level's pawn list.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level.
pub unsafe fn remove_pawn(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe {
        remove_pawn_with(pawn, |level| Ok(native_value!(svc.level_pawn_list, level)),
            |level, head| checked((svc.set_level_pawn_list)(level, head)))
    }
}

/// Source RemovePawn policy; the removed pawn's own NextPawn is intentionally retained.
///
/// # Safety
/// Pawn and every reached list node remain live; callbacks implement the actual level list.
pub unsafe fn remove_pawn_with<E>(
    pawn: *mut APawn,
    read_head: impl FnOnce(*mut c_void) -> Result<*mut c_void, E>,
    write_head: impl FnOnce(*mut c_void, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let level = (*pawn).base.x_level;
        let head = read_head(level)?.cast::<APawn>();
        if head == pawn {
            write_head(level, (*head).next_pawn)?;
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
        Ok(())
    }
}

/// `APawn::execStopWaiting`: a sleeping latent action is cancelled.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn stop_waiting(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { stop_waiting_with(pawn, || Ok(native_value!(svc.state_latent_action, actor_of(pawn).cast()))) }
}

/// Source StopWaiting with a genuine current latent-action query.
///
/// # Safety
/// Pawn and its execution frame remain live.
pub unsafe fn stop_waiting_with<E>(
    pawn: *mut APawn, latent_action: impl FnOnce() -> Result<i32, E>,
) -> Result<(), E> {
    unsafe {
        if latent_action()? == EPOLL_SLEEP {
            (*actor_of(pawn)).latent_float = -1.0;
        }
        Ok(())
    }
}

/// `APawn::execWaitForLanding`: arm the 2.5 s landing timer and poll while falling.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn wait_for_landing(svc: &Hp2PawnNativeAiServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { wait_for_landing_with(pawn, |actor, action| checked((svc.set_state_latent_action)(actor, action))) }
}

/// Source landing timer initialization and conditional latent poll publication.
///
/// # Safety
/// Pawn is live; publication writes its actual execution frame.
pub unsafe fn wait_for_landing_with<E>(
    pawn: *mut APawn, set_latent: impl FnOnce(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let actor = actor_of(pawn);
        (*actor).latent_float = 2.5;
        if (*actor).physics == PHYS_FALLING {
            set_latent(actor.cast(), AI_POLL_WAIT_FOR_LANDING)?;
        }
        Ok(())
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
) -> PawnResult<()> {
    unsafe {
        poll_wait_for_landing_with(pawn, result,
            |actor, action| checked((svc.set_state_latent_action)(actor, action)),
            |pawn| checked((svc.event_long_fall)(pawn)))
    }
}

/// Source landing poll, including negative timer event and ground-only latent clear.
///
/// # Safety
/// Pawn/frame are live; delta is readable only when physics is Falling.
pub unsafe fn poll_wait_for_landing_with<E>(
    pawn: *mut APawn, result: *const f32,
    set_latent: impl FnOnce(*mut c_void, i32) -> Result<(), E>,
    long_fall: impl FnOnce(*mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let actor = actor_of(pawn);
        if (*actor).physics != PHYS_FALLING {
            set_latent(actor.cast(), 0)?;
        } else {
            (*actor).latent_float -= *result;
            if (*actor).latent_float < 0.0 {
                long_fall(pawn.cast())?;
            }
        }
        Ok(())
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
) -> PawnResult<i32> {
    unsafe {
        find_stair_rotation_with(
            pawn, delta_time, hit, |rot| rotation_vector(svc, &raw const rot),
            |pawn, hit, end, start, extent| trace(svc, pawn, hit, end, start, extent),
        )
    }
}

/// Source stair pitch adjustment with actual host rotation and visibility trace operations.
///
/// # Safety
/// Pawn/world and writable hit storage remain live through all queries.
pub unsafe fn find_stair_rotation_with<E>(
    pawn: *mut APawn, delta_time: f32, hit: *mut FCheckResult,
    rotation_vector: impl FnOnce(FRotator) -> Result<FVector, E>,
    mut trace: impl FnMut(*mut APawn, *mut FCheckResult, FVector, FVector, FVector) -> Result<(), E>,
) -> Result<i32, E> {
    unsafe {
        if delta_time > 0.33_f32 {
            return Ok((*pawn).view_rotation.pitch);
        }
        if (*pawn).view_rotation.pitch > 32768 {
            (*pawn).view_rotation.pitch = ((*pawn).view_rotation.pitch & 65535) - 65536;
        }
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
        let dir = rotation_vector(look)?;
        let eye = FVector {
            x: location.x + 0.0,
            y: location.y + 0.0,
            z: location.z + base_eye,
        };

        trace(pawn, hit, along(eye, 2.0 * height, dir), eye, slice)?;
        let dist = 2.0 * height * (*hit).time;
        let mut stair_rot: i32 = 0;
        if (dist as f64) > 0.8_f64 * (height as f64) {
            let spot = along(eye, 0.5 * dist, dir);
            let down = 3.0 * height;
            trace(pawn, hit, drop_z(spot, down), spot, slice)?;
            if (*hit).time < 1.0 {
                let first_down = down * (*hit).time;
                if first_down < height.mul_add(0.7, -6.0) {
                    let spot = along(eye, dist, dir);
                    trace(pawn, hit, drop_z(spot, down), spot, slice)?;
                    stair_rot = (*pawn).view_rotation.pitch.max(0);
                    if down * (*hit).time < first_down - 10.0 {
                        stair_rot = 5400;
                    }
                } else if first_down > height.mul_add(0.7, 6.0) {
                    trace(
                        pawn,
                        hit,
                        along((*actor).location, 0.9 * dist, dir),
                        (*actor).location,
                        no_extent,
                    )?;
                    if (*hit).time == 1.0 {
                        let spot = along(eye, dist, dir);
                        trace(pawn, hit, drop_z(spot, down), spot, slice)?;
                        stair_rot = (*pawn).view_rotation.pitch.min(0);
                        if down * (*hit).time > first_down + 10.0 {
                            stair_rot = -5000;
                        }
                    }
                }
            }
        }

        // `Abs(ViewRotation.Pitch - stairRot)`; the subtraction wraps, as clang emits it.
        let diff = {
            let d = (*pawn).view_rotation.pitch.wrapping_sub(stair_rot);
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
            stair_rot = ((*pawn).view_rotation.pitch as f32)
                .mul_add(1.0 - rot_rate, stair_rot as f32 * rot_rate)
                as i32;
        }
        Ok(stair_rot)
    }
}

static mut AI_SERVICES: Option<Hp2PawnNativeAiServices> = None;

/// Publishes the `Hp2PawnNativeAiServices` table every native below reads through
/// [`published_services`]. Called once from `Hp2PawnNativeAiServices.cpp`'s static
/// initializer, mirroring `hp2_rs_core_vm_object_publish_services`.
///
/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2PawnNativeAiServices) {
    unsafe {
        (&raw mut AI_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2PawnNativeAiServices, i32> {
    unsafe { (&raw const AI_SERVICES).read().ok_or(1) }
}

/// `APawn::LineOfSightTo(Other, bShowSelf)`, the shared computation behind
/// `execCanSee` (`bShowSelf = 1`) and `execLineOfSightTo` (`bShowSelf = 0`). The C++
/// member itself (`UnPawn.cpp`) stays: `Hp2PawnNativeMoveServices` and
/// `Hp2PawnNativeSenseServices` leaves also call through it, and its own
/// `guard(APawn::LineOfSightTo)` frame must keep appearing in unwind history.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `other` must point to a live `AActor`.
pub unsafe fn line_of_sight_to(
    svc: &Hp2PawnNativeAiServices,
    pawn: *mut APawn,
    other: *mut AActor,
    show_self: i32,
) -> PawnResult<bool> {
    unsafe { Ok(native_value!(svc.line_of_sight_to, pawn.cast(), other.cast(), show_self) != 0) }
}

/// `ANavigationPoint::execdescribeSpec`: reads `GetLevel()->ReachSpecs(iSpec)`.
/// Rust policy (unchanged from the exec body this replaces): an index outside
/// `ReachSpecs` (UE1 read it unchecked, via `TArray::operator()`) yields an empty
/// spec rather than reproducing the out-of-bounds read.
///
/// # Safety
/// `level` must point to a live `ULevel`.
pub unsafe fn describe_spec(
    svc: &Hp2PawnNativeAiServices,
    level: *mut c_void,
    i_spec: i32,
) -> PawnResult<(*mut c_void, *mut c_void, i32, i32)> {
    unsafe {
        // C++ short-circuits `iSpec >= 0 && iSpec < GetLevel()->ReachSpecs.Num()`:
        // a negative index never evaluates `ReachSpecs.Num()`.
        if i_spec < 0 {
            return Ok((null_mut(), null_mut(), 0, 0));
        }
        let count = native_value!(svc.reach_spec_count, level);
        if i_spec >= count {
            return Ok((null_mut(), null_mut(), 0, 0));
        }
        let mut start = null_mut();
        let mut end = null_mut();
        let mut reach_flags = 0;
        let mut distance = 0;
        checked((svc.reach_spec_at)(
            level,
            i_spec,
            &raw mut start,
            &raw mut end,
            &raw mut reach_flags,
            &raw mut distance,
        ))?;
        Ok((start, end, reach_flags, distance))
    }
}
