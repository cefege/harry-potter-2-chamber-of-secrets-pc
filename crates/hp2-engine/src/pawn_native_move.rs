//! Port of the move group of `Engine/Src/UnPawn.cpp`: `APawn::execactorReachable`,
//! `execFindPathTo`, `execFindPathToward`, `execFindRandomDest`,
//! `AActor::execMakeNoise`, the `MoveTo`, `MoveToward`, `StrafeTo`,
//! `StrafeFacing`, `TurnToward` and `TurnTo` latent actions with their `Poll*`
//! continuations, and `APawn::ShowSelf`.
//!
//! Route, reach and pawn-sense logic (`CanMoveTo`, `findPathTo`,
//! `findPathToward`, `findRandomDest`, `clearPaths`, `actorReachable`,
//! `pointReachable`, `findPath`, `HandleSpecial`, `setMoveTimer`,
//! `rotateToward`, `moveToward`, `LineOfSightTo`, `CheckNoiseHearing`,
//! `PickWallAdjust`) call `crate::route`/`crate::pawn_reach`/`crate::pawn_sense`
//! directly instead of round-tripping through an `APawn`/`AActor` C++ shell —
//! those shells stay, called by other leaf tables, but this group no longer
//! bounces through them. `Rotation.Vector()` uses the shared Core math policy.
//! Level-info lists, `NetMode`, `ReachSpecs.Num()`,
//! `GetStateFrame()->LatentAction` and `FindPathCycles` are read and written
//! through the `hp2_abi` mirrors, as the C++ did. `appCycles()` (`UnUnix.cpp`)
//! is itself Rust-owned (`hp2_core::unix_clock::cycles`), so `clock`/`unclock`
//! call it directly too. `IsA`/`IsProbing` reflection and the
//! `eventAlterDestination`/`eventSeePlayer` dispatchers are the only
//! operations routed through [`Hp2PawnNativeMoveServices`], supplied by the
//! canonical owner or the legacy C++ oracle's leaf table.
//! The C++ shells keep `guard`/`unguard`, `clock`/`unclock`, `P_GET_*`,
//! `P_FINISH` and the `*(T*)Result` writes only after checked success. Every
//! native leaf returns status separately from its value, and every policy
//! frame stops at first error.
//!
//! Latent actions: the thunks pass their `AI_Poll*` value in as `latent`. Rust
//! stores it in `LatentAction` through the mirror and stores `0` when a poll
//! finishes.
//!
//! FP contraction: each `mul_add` marks a site the original object file fuses
//! (`fmadd`/`fmsub`). The evidence is recorded on each function. Every other
//! product is a separate rounded multiply.
//!
//! Oracle UB kept as is: `MoveTarget`, `FaceTarget` and `Region.Zone` are
//! dereferenced without a null check where the C++ does so, including after
//! engine events that can clear them. Nothing is guarded.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, AInventory, ALevelInfo, ANavigationPoint, APawn, AZoneInfo, FStateFrame, ULevel,
    UObject,
};
use hp2_core::unix_clock;

use crate::pawn::{PawnResult, checked};
use crate::{pawn_reach, pawn_sense, route, vec};

// A native output is read only after success; failure returns from this frame.
macro_rules! native_value {
    ($service:expr, $($arg:expr),* $(,)?) => {{
        let mut value = std::mem::MaybeUninit::uninit();
        checked(($service)($($arg,)* value.as_mut_ptr()))?;
        value.assume_init()
    }};
}

/// `EPhysics` values (`EngineClasses.h`).
const PHYS_WALKING: u8 = 1;
const PHYS_SWIMMING: u8 = 3;
const PHYS_FLYING: u8 = 4;
const PHYS_SPIDER: u8 = 10;

/// `NM_Client` (`EngineClasses.h`).
const NM_CLIENT: i32 = 3;

/// `MAXCOMMONRADIUS` (`UnPath.h`).
const MAX_COMMON_RADIUS: f32 = 18.0;

/// `IsA` kinds accepted by [`Hp2PawnNativeMoveServices::is_a`].
pub const CLASS_PAWN: u32 = 0;
/// `AInventory`.
pub const CLASS_INVENTORY: u32 = 1;
/// `ANavigationPoint`.
pub const CLASS_NAVIGATION_POINT: u32 = 2;

/// `IsProbing` names accepted by [`Hp2PawnNativeMoveServices::is_probing`].
/// `NAME_SpecialHandling`.
pub const NAME_SPECIAL_HANDLING: u32 = 0;
/// `NAME_SeePlayer`.
pub const NAME_SEE_PLAYER: u32 = 1;

/// Checked native services used by the move group. Every function returns a
/// status (zero succeeds); documented native return values use the final out
/// pointer. Field reads/writes, route/reach/sense logic and `Rotation.Vector()`
/// have moved to direct mirror reads and direct calls into their owning
/// modules (see the module doc); only `UObject` reflection and UnrealScript
/// event dispatch remain leaves here. The shared cycle counter
/// (`appCycles()`, `UnUnix.cpp`) is itself Rust-owned
/// (`hp2_core::unix_clock::cycles`), so `clock`/`unclock` call it directly
/// instead of bouncing through a leaf.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeMoveServices {
    /// `UObject::IsA` for the `CLASS_*` kind. Returns `UBOOL`.
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, kind: u32, out: *mut i32) -> i32,
    /// `UObject::IsProbing` for the `NAME_*` kind. Returns `UBOOL`.
    pub is_probing: unsafe extern "C" fn(obj: *mut c_void, name: u32, out: *mut i32) -> i32,
    /// `pawn->eventAlterDestination()`.
    pub event_alter_destination: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    /// `pawn->eventSeePlayer(seer)`.
    pub event_see_player: unsafe extern "C" fn(pawn: *mut c_void, seer: *mut c_void) -> i32,
}

static mut PAWN_MOVE_SERVICES: Option<Hp2PawnNativeMoveServices> = None;

/// Publishes the table read by every native in
/// `crates/hp2-ffi/src/engine_natives_pawn_move.rs` through
/// [`published_services`], mirroring [`crate::stat_log::publish_services`].
/// Called once from `Hp2PawnNativeMoveServices.cpp`'s static initializer.
///
/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2PawnNativeMoveServices) {
    unsafe {
        (&raw mut PAWN_MOVE_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2PawnNativeMoveServices, i32> {
    unsafe { (&raw const PAWN_MOVE_SERVICES).read().ok_or(1) }
}

/// `Abs` (`UnTemplate.h`): `(A >= 0) ? A : -A`.
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `Max` (`UnTemplate.h`): `(A >= B) ? A : B`.
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `Min` (`UnTemplate.h`): `(A <= B) ? A : B`.
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `::Max(0.f, Min(MaxDesiredSpeed, speed))`, the `DesiredSpeed` clamp of the
/// move latent actions.
fn clamped_desired_speed(max_desired_speed: f32, speed: f32) -> f32 {
    max_f(0.0, min_f(max_desired_speed, speed))
}

/// `AActor::Location` of the actor at `actor`.
///
/// # Safety
/// `actor` must point to a live `AActor`.
unsafe fn location_of(actor: *mut c_void) -> FVector {
    unsafe { (*actor.cast::<AActor>()).location }
}

/// `Dir = Other - Location; Dir.Z = 0; (Dir | Dir)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn planar_dist_sq(pawn: *mut APawn, other: FVector) -> f32 {
    unsafe {
        let mut dir = vec::sub(other, (*pawn).base.location);
        dir.z = 0.0;
        vec::size_squared(dir)
    }
}

/// `GetStateFrame()->LatentAction = latent`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn set_latent(pawn: *mut APawn, latent: i32) -> PawnResult<()> {
    unsafe {
        let frame = (*pawn.cast::<UObject>()).state_frame.cast::<FStateFrame>();
        (*frame).latent_action = latent;
    }
    Ok(())
}

/// `rotateToward(Focus)`. Passes the live `Focus` field, as the C++ passes it by reference.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn rotate_focus(sense: &pawn_sense::Hp2PawnSenseServices, pawn: *mut APawn) -> PawnResult<i32> {
    unsafe {
        pawn_sense::rotate_toward(sense, pawn, (*pawn).focus)
    }
}

/// `moveToward(Destination)`. Passes the live `Destination` field, as the C++ passes it by reference.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn move_destination(sense: &pawn_sense::Hp2PawnSenseServices, pawn: *mut APawn) -> PawnResult<i32> {
    unsafe {
        pawn_sense::move_toward(
            sense,
            pawn,
            &raw const (*pawn).destination,
        )
    }
}

/// `DesiredSpeed = bIsPlayer ? MaxDesiredSpeed : 0.8f*MaxDesiredSpeed` for the
/// strafe latent actions. The product stands alone, so it is never fused.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn strafe_desired_speed(pawn: *mut APawn) -> f32 {
    unsafe {
        if (*pawn).b_is_player() {
            (*pawn).max_desired_speed
        } else {
            0.8_f32 * (*pawn).max_desired_speed
        }
    }
}

/// `if (!bCanStrafe && (Physics == PHYS_Flying || Physics == PHYS_Swimming))
/// Acceleration = Rotation.Vector() * AccelRate;`. Each component is a single
/// multiply, so it is never fused.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn accelerate_if_airborne(pawn: *mut APawn) -> PawnResult<()> {
    unsafe {
        if (*pawn).b_can_strafe() {
            return Ok(());
        }
        let physics = (*pawn).base.physics;
        if physics != PHYS_FLYING && physics != PHYS_SWIMMING {
            return Ok(());
        }
        let rotation = (*pawn).base.rotation;
        let dir = hp2_core::math::rotator_vector(
            hp2_core::math::FRotator { pitch: rotation.pitch, yaw: rotation.yaw, roll: rotation.roll },
            hp2_core::math::trig_table(),
        );
        (*pawn).base.acceleration = vec::scale(dir, (*pawn).accel_rate);
    }
    Ok(())
}

/// Tail shared by `execFindPathTo` and `execFindPathToward`: `SpecialPause = 0`,
/// `bShootSpecial = 0`, `HandleSpecial` for a probing best path, and clearing
/// `SpecialGoal` when it is the chosen path.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `best` must be null or a live `AActor`.
unsafe fn settle_best_path(
    svc: &Hp2PawnNativeMoveServices,
    sense: &crate::pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    best: *mut c_void,
) -> PawnResult<*mut c_void> {
    unsafe {
        (*pawn).special_pause = 0.0;
        (*pawn).set_b_shoot_special(false);
        let mut best = best;
        if !best.is_null() && native_value!(svc.is_probing, best, NAME_SPECIAL_HANDLING) != 0 {
            let mut best_path = best.cast::<AActor>();
            pawn_sense::handle_special(sense, pawn, &mut best_path)?;
            best = best_path.cast();
        }
        if best == (*pawn).special_goal {
            (*pawn).special_goal = ptr::null_mut();
        }
        Ok(best)
    }
}

/// `GetLevel()->GetLevelInfo()->NetMode` of `actor`.
///
/// # Safety
/// `actor` must point to a live `AActor`.
unsafe fn net_mode(actor: *mut c_void) -> i32 {
    unsafe { (*(*actor.cast::<AActor>()).level.cast::<ALevelInfo>()).net_mode as i32 }
}

/// `GetLevel()->ReachSpecs.Num()` of `pawn`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn reach_spec_count(pawn: *mut APawn) -> i32 {
    unsafe { (*(*pawn.cast::<AActor>()).x_level.cast::<ULevel>()).reach_specs.num }
}

/// `GetLevel()->GetLevelInfo()->NavigationPointList` of `pawn`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn level_nav_list(pawn: *mut APawn) -> *mut c_void {
    unsafe { (*(*pawn.cast::<AActor>()).level.cast::<ALevelInfo>()).navigation_point_list }
}

/// `GetLevel()->GetLevelInfo()->PawnList` of `pawn`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn level_pawn_list(pawn: *mut APawn) -> *mut c_void {
    unsafe { (*(*pawn.cast::<AActor>()).level.cast::<ALevelInfo>()).pawn_list }
}

/// `clock(T)` (`UnFile.h`): `T -= appCycles()`. `appCycles()` is itself
/// Rust-owned (`UnUnix.cpp`'s `appCycles()` forwards to
/// `hp2_rs_core_unix_cycles`/[`unix_clock::cycles`]), so this calls it
/// directly instead of bouncing through a C++ leaf.
fn clock(t: i32) -> i32 {
    t.wrapping_sub(unix_clock::cycles() as i32)
}

/// `unclock(T)` (`UnFile.h`): `T += appCycles()-34`.
fn unclock(t: i32) -> i32 {
    t.wrapping_add(unix_clock::cycles().wrapping_sub(34) as i32)
}

/// `clock(GetLevel()->FindPathCycles)` (`UnFile.h`). Called by
/// `engine_natives_pawn_paths.rs` before any fallible path-finding work.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level.
pub unsafe fn clock_find_path_begin(pawn: *mut c_void) -> PawnResult<()> {
    unsafe {
        let level = (*pawn.cast::<AActor>()).x_level.cast::<ULevel>();
        (*level).find_path_cycles = clock((*level).find_path_cycles);
    }
    Ok(())
}

/// `unclock(GetLevel()->FindPathCycles)` (`UnFile.h`). Called by
/// `engine_natives_pawn_paths.rs` only once the rest of the native has already
/// succeeded, matching the original leaving it unexecuted on the exception path.
///
/// # Safety
/// `pawn` must point to a live `APawn` in a live level.
pub unsafe fn clock_find_path_end(pawn: *mut c_void) -> PawnResult<()> {
    unsafe {
        let level = (*pawn.cast::<AActor>()).x_level.cast::<ULevel>();
        (*level).find_path_cycles = unclock((*level).find_path_cycles);
    }
    Ok(())
}

/// `APawn::execactorReachable` after the `!actor` check. The `GetLevel()->FindPathCycles`
/// clock/unclock is done by the caller in `engine_natives_pawn_paths.rs` via the
/// `clock_find_path_*` functions, not here.
/// Returns the `DWORD` result.
///
/// # Safety
/// `pawn` must point to a live `APawn` and `actor` to a live `AActor`, on the game thread.
pub unsafe fn actor_reachable(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    actor: *mut c_void,
) -> PawnResult<u32> {
    unsafe { actor_reachable_with(svc, route::published_services()?, pawn_reach::published_services()?, pawn, actor) }
}

/// Shared script reachability policy with graph-owned route/reach services.
///
/// # Safety
/// The same world/actor lifetime contract as `actor_reachable` applies.
pub unsafe fn actor_reachable_with(
    svc: &Hp2PawnNativeMoveServices,
    route: &crate::route::Hp2RouteServices,
    reach: &crate::pawn_reach::Hp2PawnReachServices,
    pawn: *mut APawn,
    actor: *mut c_void,
) -> PawnResult<u32> {
    unsafe {
        let mut actor = actor;
        let mut real_actor = ptr::null_mut::<c_void>();
        if native_value!(svc.is_a, actor, CLASS_INVENTORY) != 0 {
            let marker = (*actor.cast::<AInventory>()).my_marker;
            if !marker.is_null() {
                real_actor = actor;
                actor = marker;
            }
        }
        if native_value!(svc.is_a, actor, CLASS_NAVIGATION_POINT) != 0
            && reach_spec_count(pawn) != 0
            && (*pawn).base.collision_radius <= MAX_COMMON_RADIUS
        {
            let mut max_dist_sq = max_f(48.0, (*pawn).base.collision_radius);
            let mut on_path = false;
            max_dist_sq = max_dist_sq * max_dist_sq;
            let move_target = (*pawn).move_target;
            if !move_target.is_null()
                && native_value!(svc.is_a, move_target, CLASS_NAVIGATION_POINT) != 0
                && abs_f(location_of(move_target).z - (*pawn).base.location.z)
                    < (*pawn).base.collision_height
                && planar_dist_sq(pawn, location_of(move_target)) < max_dist_sq
            {
                on_path = true;
                if move_target == actor
                    || route::can_move_to(
                        route,
                        pawn,
                        move_target.cast(),
                        actor.cast(),
                    )? != 0
                {
                    return Ok(1);
                }
            }
            let mut nav = level_nav_list(pawn);
            while !nav.is_null() {
                let nav_location = location_of(nav);
                if abs_f(nav_location.z - (*pawn).base.location.z) < (*pawn).base.collision_height
                    && planar_dist_sq(pawn, nav_location) < max_dist_sq
                {
                    on_path = true;
                    if nav == actor
                        || route::can_move_to(
                            route,
                            pawn,
                            nav.cast(),
                            actor.cast(),
                        )? != 0
                    {
                        return Ok(1);
                    }
                }
                nav = (*nav.cast::<ANavigationPoint>())
                    .next_navigation_point
                    .cast::<c_void>();
            }
            if on_path && (*pawn).base.physics != PHYS_FLYING {
                return Ok(0);
            }
        }
        if !real_actor.is_null() {
            actor = real_actor;
        }
        Ok(pawn_reach::actor_reachable(
            reach,
            pawn,
            actor.cast(),
            0,
        )? as u32)
    }
}

/// `APawn::execFindPathTo` after `P_FINISH`. Returns `bestPath`. The
/// `GetLevel()->FindPathCycles` clock/unclock is done by the caller in
/// `engine_natives_pawn_paths.rs` via the `clock_find_path_*` functions, not here.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `point` must point to a live `FVector`.
pub unsafe fn find_path_to(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    point: *const c_void,
    single: i32,
    clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe { find_path_to_with(svc, route::published_services()?, pawn_sense::published_services()?, pawn, point, single, clear) }
}

/// Shared path policy supplied by the active graph rather than process globals.
///
/// # Safety
/// Same world/point lifetime contract as `find_path_to`.
pub unsafe fn find_path_to_with(
    svc: &Hp2PawnNativeMoveServices,
    route: &crate::route::Hp2RouteServices,
    sense: &crate::pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    point: *const c_void,
    single: i32,
    clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe {
        let mut new_path: *mut AActor = ptr::null_mut();
        let found = route::find_path_to(
            route,
            pawn,
            *point.cast::<FVector>(),
            single,
            &mut new_path,
            clear,
        )?;
        let best = if found != 0 {
            new_path.cast::<c_void>()
        } else {
            ptr::null_mut()
        };
        settle_best_path(svc, sense, pawn, best)
    }
}

/// `APawn::execFindPathToward` after the `!goal` check. Returns `bestPath`. The
/// `GetLevel()->FindPathCycles` clock/unclock is done by the caller in
/// `engine_natives_pawn_paths.rs` via the `clock_find_path_*` functions, not here.
///
/// # Safety
/// `pawn` must point to a live `APawn` and `goal` to a live `AActor`.
pub unsafe fn find_path_toward(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    single: i32,
    clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe { find_path_toward_with(svc, route::published_services()?, pawn_sense::published_services()?, pawn, goal, single, clear) }
}

/// Shared goal-path policy with graph-owned services.
///
/// # Safety
/// Same world/goal lifetime contract as `find_path_toward`.
pub unsafe fn find_path_toward_with(
    svc: &Hp2PawnNativeMoveServices,
    route: &crate::route::Hp2RouteServices,
    sense: &crate::pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    single: i32,
    clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe {
        let mut new_path: *mut AActor = ptr::null_mut();
        let found = route::find_path_toward(
            route,
            pawn,
            goal.cast(),
            single,
            &mut new_path,
            clear,
        )?;
        let best = if found != 0 {
            new_path.cast::<c_void>()
        } else {
            ptr::null_mut()
        };
        settle_best_path(svc, sense, pawn, best)
    }
}

/// `APawn::execFindRandomDest` body between the clock calls. `clearPaths()` runs
/// first when `clear` is set. Returns `bestPath`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn find_random_dest(
    _svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe { find_random_dest_with(route::published_services()?, pawn, clear) }
}

/// Shared random-destination policy with graph-owned route services.
///
/// # Safety
/// Pawn and its navigation world remain live through synchronous callbacks.
pub unsafe fn find_random_dest_with(
    s: &crate::route::Hp2RouteServices, pawn: *mut APawn, clear: i32,
) -> PawnResult<*mut c_void> {
    unsafe {
        if clear != 0 {
            route::clear_paths(s, pawn)?;
        }
        let mut new_path: *mut AActor = ptr::null_mut();
        Ok(if route::find_random_dest(s, pawn, &mut new_path)? != 0 {
            new_path.cast::<c_void>()
        } else {
            ptr::null_mut()
        })
    }
}


/// `AActor::execMakeNoise` body: `CheckNoiseHearing` unless the level runs as a client.
///
/// # Safety
/// `actor` must point to a live `AActor`.
pub unsafe fn make_noise_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    actor: *mut c_void,
    loudness: f32,
) -> PawnResult<()> {
    unsafe {
        if net_mode(actor) != NM_CLIENT {
            check_noise_hearing(sense, actor, loudness)?;
        }
    }
    Ok(())
}

/// `AActor::CheckNoiseHearing(Loudness)`: the `Instigator` guard and
/// `clock`/`unclock(GetLevel()->SeePlayer)` the C++ method wraps
/// `pawn_sense::check_noise_hearing` in.
///
/// # Safety
/// `actor` must point to a live `AActor`.
unsafe fn check_noise_hearing(sense: &pawn_sense::Hp2PawnSenseServices, actor: *mut c_void, loudness: f32) -> PawnResult<()> {
    unsafe {
        let this = actor.cast::<AActor>();
        if (*this).instigator.is_null() {
            return Ok(());
        }
        let level = (*this).x_level.cast::<ULevel>();
        (*level).see_player = clock((*level).see_player);
        pawn_sense::check_noise_hearing(sense, this, loudness)?;
        (*level).see_player = unclock((*level).see_player);
    }
    Ok(())
}

/// `APawn::execMoveTo` body after `P_GET_*`/`P_FINISH`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn move_to_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    dest: FVector,
    speed: f32,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).move_target = ptr::null_mut();
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = clamped_desired_speed((*pawn).max_desired_speed, speed);
        pawn_sense::set_move_timer(pawn, move_size)?;
        set_latent(pawn, latent)?;
        (*pawn).destination = dest;
        (*pawn).focus = dest;
        rotate_focus(sense, pawn)?;
        move_destination(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execPollMoveTo` and `APawn::execPollStrafeTo` (identical bodies).
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn poll_destination_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
) -> PawnResult<()> {
    unsafe {
        rotate_focus(sense, pawn)?;
        if move_destination(sense, pawn)? != 0 {
            set_latent(pawn, 0)?;
        }
    }
    Ok(())
}

/// `APawn::execMoveToward` body after `P_GET_*`/`P_FINISH`. A null `goal` returns
/// before anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn move_toward_with(
    svc: &Hp2PawnNativeMoveServices,
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    speed: f32,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        if goal.is_null() {
            return Ok(());
        }
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = clamped_desired_speed((*pawn).max_desired_speed, speed);
        if native_value!(svc.is_a, goal, CLASS_PAWN) != 0 {
            (*pawn).move_timer = 1.2;
        } else {
            let move_size = vec::size(vec::sub(location_of(goal), (*pawn).base.location));
            pawn_sense::set_move_timer(pawn, move_size)?;
        }
        (*pawn).move_target = goal;
        (*pawn).destination = location_of(goal);
        (*pawn).focus = (*pawn).destination;
        set_latent(pawn, latent)?;
        rotate_focus(sense, pawn)?;
        move_destination(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execPollMoveToward` body after `P_FINISH`.
///
/// Flying pawns aim at the head of a pawn target (`Destination.Z += 0.7f *
/// CollisionHeight`, fused by clang into one `mul_add`). Spider pawns aim at the
/// surface (`Destination -= CollisionRadius * Floor`). Clang emits a separate
/// `fmul` and `fsub` there (`operator*` and `operator-` are different inline
/// functions, so no contraction), so that subtraction stays unfused.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `move_target` must be null or a live `AActor`.
pub unsafe fn poll_move_toward_with(
    svc: &Hp2PawnNativeMoveServices,
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
) -> PawnResult<()> {
    unsafe {
        let target = (*pawn).move_target;
        if target.is_null() {
            set_latent(pawn, 0)?;
            return Ok(());
        }
        (*pawn).destination = location_of(target);
        let physics = (*pawn).base.physics;
        if physics == PHYS_FLYING && native_value!(svc.is_a, target, CLASS_PAWN) != 0 {
            let height = (*target.cast::<AActor>()).collision_height;
            (*pawn).destination.z = 0.7_f32.mul_add(height, (*pawn).destination.z);
        } else if physics == PHYS_SPIDER {
            let radius = (*target.cast::<AActor>()).collision_radius;
            let floor = (*pawn).floor;
            let dest = (*pawn).destination;
            (*pawn).destination = FVector {
                x: dest.x - radius * floor.x,
                y: dest.y - radius * floor.y,
                z: dest.z - radius * floor.z,
            };
        }
        (*pawn).focus = (*pawn).destination;
        rotate_focus(sense, pawn)?;
        let old_desired_speed = (*pawn).desired_speed;
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            checked((svc.event_alter_destination)(pawn.cast()))?;
        }
        if move_destination(sense, pawn)? != 0 {
            set_latent(pawn, 0)?;
        }
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            (*pawn).destination = location_of((*pawn).move_target);
        }
        let current = (*pawn).move_target;
        if native_value!(svc.is_a, current, CLASS_PAWN) != 0 {
            (*pawn).desired_speed = old_desired_speed;
            if !(*pawn).b_can_swim() {
                let zone = (*current.cast::<AActor>()).region.zone.cast::<AZoneInfo>();
                if (*zone).b_water_zone() {
                    (*pawn).move_timer = -1.0;
                }
            }
        }
    }
    Ok(())
}


/// `APawn::execStrafeTo` body after `P_GET_VECTOR` x2.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn strafe_to_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    dest: FVector,
    focal: FVector,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).move_target = ptr::null_mut();
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = strafe_desired_speed(pawn);
        pawn_sense::set_move_timer(pawn, move_size)?;
        set_latent(pawn, latent)?;
        (*pawn).destination = dest;
        (*pawn).focus = focal;
        rotate_focus(sense, pawn)?;
        move_destination(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execStrafeFacing` body after `P_FINISH`. A null `goal` returns before
/// anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn strafe_facing_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    dest: FVector,
    goal: *mut c_void,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        if goal.is_null() {
            return Ok(());
        }
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = strafe_desired_speed(pawn);
        pawn_sense::set_move_timer(pawn, move_size)?;
        (*pawn).destination = dest;
        (*pawn).face_target = goal;
        (*pawn).focus = location_of(goal);
        set_latent(pawn, latent)?;
        rotate_focus(sense, pawn)?;
        move_destination(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execPollStrafeFacing` body. `Destination` is restored after the poll,
/// so the advanced-tactics `eventAlterDestination` change does not persist.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `face_target` must be null or a live `AActor`.
pub unsafe fn poll_strafe_facing_with(
    svc: &Hp2PawnNativeMoveServices,
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
) -> PawnResult<()> {
    unsafe {
        let face = (*pawn).face_target;
        if face.is_null() {
            set_latent(pawn, 0)?;
            return Ok(());
        }
        (*pawn).focus = location_of(face);
        let real_dest = (*pawn).destination;
        rotate_focus(sense, pawn)?;
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            checked((svc.event_alter_destination)(pawn.cast()))?;
        }
        if move_destination(sense, pawn)? != 0 {
            set_latent(pawn, 0)?;
        }
        (*pawn).destination = real_dest;
    }
    Ok(())
}

/// `APawn::execTurnToward` body after `P_FINISH`. A null `goal` returns before
/// anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn turn_toward_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        if goal.is_null() {
            return Ok(());
        }
        (*pawn).face_target = goal;
        set_latent(pawn, latent)?;
        accelerate_if_airborne(pawn)?;
        (*pawn).focus = location_of(goal);
        rotate_focus(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execPollTurnToward` body.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `face_target` must be null or a live `AActor`.
pub unsafe fn poll_turn_toward_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
) -> PawnResult<()> {
    unsafe {
        let face = (*pawn).face_target;
        if face.is_null() {
            set_latent(pawn, 0)?;
            return Ok(());
        }
        accelerate_if_airborne(pawn)?;
        (*pawn).focus = location_of(face);
        if rotate_focus(sense, pawn)? != 0 {
            set_latent(pawn, 0)?;
        }
    }
    Ok(())
}

/// `APawn::execTurnTo` body after `P_GET_VECTOR`/`P_FINISH`. On walking pawns
/// the focus stays at the current height.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn turn_to_with(
    sense: &pawn_sense::Hp2PawnSenseServices,
    pawn: *mut APawn,
    focal: FVector,
    latent: i32,
) -> PawnResult<()> {
    unsafe {
        (*pawn).move_target = ptr::null_mut();
        set_latent(pawn, latent)?;
        (*pawn).focus = focal;
        if (*pawn).base.physics == PHYS_WALKING {
            (*pawn).focus.z = (*pawn).base.location.z;
        }
        accelerate_if_airborne(pawn)?;
        rotate_focus(sense, pawn)?;
    }
    Ok(())
}

/// `APawn::execPollTurnTo` body.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn poll_turn_to_with(sense: &pawn_sense::Hp2PawnSenseServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe {
        accelerate_if_airborne(pawn)?;
        if rotate_focus(sense, pawn)? != 0 {
            set_latent(pawn, 0)?;
        }
    }
    Ok(())
}

/// `APawn::ShowSelf` body between the clock calls. Every other pawn on the level
/// that is probing `NAME_SeePlayer`, has a clear line of sight to this pawn, and
/// is not in its sight cooldown gets `eventSeePlayer(this)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` on the game thread.
pub unsafe fn show_self(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe {
        let raw_pawn = pawn.cast::<c_void>();
        let mut other = level_pawn_list(pawn).cast::<APawn>();
        while !other.is_null() {
            let raw_other = other.cast::<c_void>();
            if other != pawn
                && (*other).sight_counter < 0.0
                && native_value!(svc.is_probing, raw_other, NAME_SEE_PLAYER) != 0
                && pawn_sense::line_of_sight_to(
                    pawn_sense::published_services()?,
                    other,
                    pawn.cast(),
                    1,
                )? != 0
            {
                checked((svc.event_see_player)(raw_other, raw_pawn))?;
            }
            other = (*other).next_pawn.cast::<APawn>();
        }
    }
    Ok(())
}

// Legacy service publication stays only while the C++ product is the oracle.
// Both graphs execute the same policy bodies above.
/// # Safety
/// Actor and its world remain live through noise callbacks.
pub unsafe fn make_noise(_: &Hp2PawnNativeMoveServices, actor: *mut c_void, loudness: f32) -> PawnResult<()> {
    unsafe { make_noise_with(pawn_sense::published_services()?, actor, loudness) }
}
/// # Safety
/// Pawn/world/state-frame and referenced event objects remain live.
pub unsafe fn move_to(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn, dest: FVector, speed: f32, latent: i32) -> PawnResult<()> {
    unsafe { move_to_with(pawn_sense::published_services()?, pawn, dest, speed, latent) }
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn poll_destination(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { poll_destination_with(pawn_sense::published_services()?, pawn) }
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn move_toward(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn, goal: *mut c_void, speed: f32, latent: i32) -> PawnResult<()> {
    unsafe { move_toward_with(svc, pawn_sense::published_services()?, pawn, goal, speed, latent) }
}
/// # Safety
/// Pawn/targets/world/state-frame remain live through event reentry.
pub unsafe fn poll_move_toward(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { poll_move_toward_with(svc, pawn_sense::published_services()?, pawn) }
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn strafe_to(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn, dest: FVector, focal: FVector, latent: i32) -> PawnResult<()> {
    unsafe { strafe_to_with(pawn_sense::published_services()?, pawn, dest, focal, latent) }
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn strafe_facing(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn, dest: FVector, goal: *mut c_void, latent: i32) -> PawnResult<()> {
    unsafe { strafe_facing_with(pawn_sense::published_services()?, pawn, dest, goal, latent) }
}
/// # Safety
/// Pawn/targets/world/state-frame remain live through event reentry.
pub unsafe fn poll_strafe_facing(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { poll_strafe_facing_with(svc, pawn_sense::published_services()?, pawn) }
}
/// # Safety
/// Pawn/goal/world/state-frame remain live.
pub unsafe fn turn_toward(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn, goal: *mut c_void, latent: i32) -> PawnResult<()> {
    unsafe { turn_toward_with(pawn_sense::published_services()?, pawn, goal, latent) }
}
/// # Safety
/// Pawn/targets/world/state-frame remain live.
pub unsafe fn poll_turn_toward(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { poll_turn_toward_with(pawn_sense::published_services()?, pawn) }
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn turn_to(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn, focal: FVector, latent: i32) -> PawnResult<()> {
    unsafe { turn_to_with(pawn_sense::published_services()?, pawn, focal, latent) }
}
/// # Safety
/// Pawn/world/state-frame remain live.
pub unsafe fn poll_turn_to(_: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> PawnResult<()> {
    unsafe { poll_turn_to_with(pawn_sense::published_services()?, pawn) }
}
