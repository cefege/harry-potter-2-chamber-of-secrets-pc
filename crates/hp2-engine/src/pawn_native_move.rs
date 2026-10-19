//! Port of the move group of `Engine/Src/UnPawn.cpp`: `APawn::execactorReachable`,
//! `execFindPathTo`, `execFindPathToward`, `execFindRandomDest`,
//! `AActor::execMakeNoise`, the `MoveTo`, `MoveToward`, `StrafeTo`,
//! `StrafeFacing`, `TurnToward` and `TurnTo` latent actions with their `Poll*`
//! continuations, and `APawn::ShowSelf`.
//!
//! Path search, reach checks, `moveToward`/`rotateToward`, the event
//! dispatchers, level-info lists and `Rotation.Vector()` go through
//! [`Hp2PawnNativeMoveServices`], filled in by `Hp2PawnNativeMoveServices.cpp`.
//! Pawn, actor, inventory and navigation-point fields are read and written
//! through the `hp2_abi` mirrors, as the C++ did. The C++ shells keep
//! `guard`/`unguard`, `clock`/`unclock`, `P_GET_*`, `P_FINISH` and the
//! `*(T*)Result` writes.
//!
//! Latent actions: the thunks pass their `AI_Poll*` value in as `latent`. Rust
//! stores it in `LatentAction` through the services and stores `0` when a poll
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
use hp2_abi::engine::generated::{AActor, AInventory, ANavigationPoint, APawn, AZoneInfo};

use crate::vec;

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

/// Engine services used by the move group. Filled in by
/// `Hp2PawnNativeMoveServices.cpp`. Vectors cross the boundary by pointer.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeMoveServices {
    /// `UObject::IsA` for the `CLASS_*` kind. Returns `UBOOL`.
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, kind: u32) -> i32,
    /// `UObject::IsProbing` for the `NAME_*` kind. Returns `UBOOL`.
    pub is_probing: unsafe extern "C" fn(obj: *mut c_void, name: u32) -> i32,
    /// `GetLevel()->GetLevelInfo()->NetMode` of `actor`.
    pub net_mode: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `GetLevel()->ReachSpecs.Num()` of `pawn`.
    pub reach_spec_count: unsafe extern "C" fn(pawn: *mut c_void) -> i32,
    /// `GetLevel()->GetLevelInfo()->PawnList` of `pawn`.
    pub level_pawn_list: unsafe extern "C" fn(pawn: *mut c_void) -> *mut c_void,
    /// `GetLevel()->GetLevelInfo()->NavigationPointList` of `pawn`.
    pub level_nav_list: unsafe extern "C" fn(pawn: *mut c_void) -> *mut c_void,
    /// `pawn->actorReachable(actor)` with the default `bKnowVisible = 0`. Returns `INT`.
    pub actor_reachable: unsafe extern "C" fn(pawn: *mut c_void, actor: *mut c_void) -> i32,
    /// `pawn->CanMoveTo(anchor, dest)`. Returns `INT`.
    pub can_move_to:
        unsafe extern "C" fn(pawn: *mut c_void, anchor: *mut c_void, dest: *mut c_void) -> i32,
    /// `pawn->findPathTo(*point, single, newPath, clear)`. Writes `newPath` to `best`.
    /// Returns `INT`.
    pub find_path_to: unsafe extern "C" fn(
        pawn: *mut c_void,
        point: *const c_void,
        single: i32,
        clear: i32,
        best: *mut *mut c_void,
    ) -> i32,
    /// `pawn->findPathToward(goal, single, newPath, clear)`. Writes `newPath` to `best`.
    /// Returns `INT`.
    pub find_path_toward: unsafe extern "C" fn(
        pawn: *mut c_void,
        goal: *mut c_void,
        single: i32,
        clear: i32,
        best: *mut *mut c_void,
    ) -> i32,
    /// `pawn->findRandomDest(newPath)`. Writes `newPath` to `best`. Returns `INT`.
    pub find_random_dest: unsafe extern "C" fn(pawn: *mut c_void, best: *mut *mut c_void) -> i32,
    /// `pawn->clearPaths()`.
    pub clear_paths: unsafe extern "C" fn(pawn: *mut c_void),
    /// `pawn->HandleSpecial(*best)`. Writes the updated actor back to `best`.
    pub handle_special: unsafe extern "C" fn(pawn: *mut c_void, best: *mut *mut c_void),
    /// `pawn->setMoveTimer(move_size)`.
    pub set_move_timer: unsafe extern "C" fn(pawn: *mut c_void, move_size: f32),
    /// `pawn->rotateToward(*focal)`. `focal` points at the live `Focus` field. Returns `INT`.
    pub rotate_toward: unsafe extern "C" fn(pawn: *mut c_void, focal: *const c_void) -> i32,
    /// `pawn->moveToward(*dest)`. `dest` points at the live `Destination` field. Returns `INT`.
    pub move_toward: unsafe extern "C" fn(pawn: *mut c_void, dest: *const c_void) -> i32,
    /// `GetStateFrame()->LatentAction = latent`.
    pub set_latent_action: unsafe extern "C" fn(pawn: *mut c_void, latent: i32),
    /// `pawn->eventAlterDestination()`.
    pub event_alter_destination: unsafe extern "C" fn(pawn: *mut c_void),
    /// `pawn->eventSeePlayer(seer)`.
    pub event_see_player: unsafe extern "C" fn(pawn: *mut c_void, seer: *mut c_void),
    /// `pawn->LineOfSightTo(other, show_self)`. Returns `DWORD`.
    pub line_of_sight_to:
        unsafe extern "C" fn(pawn: *mut c_void, other: *mut c_void, show_self: i32) -> u32,
    /// `actor->CheckNoiseHearing(loudness)`.
    pub check_noise_hearing: unsafe extern "C" fn(actor: *mut c_void, loudness: f32),
    /// `actor->Rotation.Vector()`, written to `out`.
    pub rotation_vector: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
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
unsafe fn set_latent(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn, latent: i32) {
    unsafe { (svc.set_latent_action)(pawn.cast(), latent) }
}

/// `rotateToward(Focus)`. Passes the live `Focus` field, as the C++ passes it by reference.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn rotate_focus(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> i32 {
    unsafe { (svc.rotate_toward)(pawn.cast(), (&raw const (*pawn).focus).cast()) }
}

/// `moveToward(Destination)`. Passes the live `Destination` field, as the C++ passes it by reference.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn move_destination(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) -> i32 {
    unsafe { (svc.move_toward)(pawn.cast(), (&raw const (*pawn).destination).cast()) }
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
unsafe fn accelerate_if_airborne(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        if (*pawn).b_can_strafe() {
            return;
        }
        let physics = (*pawn).base.physics;
        if physics != PHYS_FLYING && physics != PHYS_SWIMMING {
            return;
        }
        let mut dir = vec::zero();
        (svc.rotation_vector)(pawn.cast(), (&raw mut dir).cast());
        (*pawn).base.acceleration = vec::scale(dir, (*pawn).accel_rate);
    }
}

/// Tail shared by `execFindPathTo` and `execFindPathToward`: `SpecialPause = 0`,
/// `bShootSpecial = 0`, `HandleSpecial` for a probing best path, and clearing
/// `SpecialGoal` when it is the chosen path.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `best` must be null or a live `AActor`.
unsafe fn settle_best_path(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    best: *mut c_void,
) -> *mut c_void {
    unsafe {
        (*pawn).special_pause = 0.0;
        (*pawn).set_b_shoot_special(false);
        let mut best = best;
        if !best.is_null() && (svc.is_probing)(best, NAME_SPECIAL_HANDLING) != 0 {
            (svc.handle_special)(pawn.cast(), &raw mut best);
        }
        if best == (*pawn).special_goal {
            (*pawn).special_goal = ptr::null_mut();
        }
        best
    }
}

/// `APawn::execactorReachable` after the `!actor` check. The clock stays in C++.
/// Returns the `DWORD` result.
///
/// # Safety
/// `pawn` must point to a live `APawn` and `actor` to a live `AActor`, on the game thread.
pub unsafe fn actor_reachable(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    actor: *mut c_void,
) -> u32 {
    unsafe {
        let raw_pawn = pawn.cast::<c_void>();
        let mut actor = actor;
        let mut real_actor = ptr::null_mut::<c_void>();
        if (svc.is_a)(actor, CLASS_INVENTORY) != 0 {
            let marker = (*actor.cast::<AInventory>()).my_marker;
            if !marker.is_null() {
                real_actor = actor;
                actor = marker;
            }
        }
        if (svc.is_a)(actor, CLASS_NAVIGATION_POINT) != 0
            && (svc.reach_spec_count)(raw_pawn) != 0
            && (*pawn).base.collision_radius <= MAX_COMMON_RADIUS
        {
            let mut max_dist_sq = max_f(48.0, (*pawn).base.collision_radius);
            let mut on_path = false;
            max_dist_sq = max_dist_sq * max_dist_sq;
            let move_target = (*pawn).move_target;
            if !move_target.is_null()
                && (svc.is_a)(move_target, CLASS_NAVIGATION_POINT) != 0
                && abs_f(location_of(move_target).z - (*pawn).base.location.z)
                    < (*pawn).base.collision_height
                && planar_dist_sq(pawn, location_of(move_target)) < max_dist_sq
            {
                on_path = true;
                if move_target == actor || (svc.can_move_to)(raw_pawn, move_target, actor) != 0 {
                    return 1;
                }
            }
            let mut nav = (svc.level_nav_list)(raw_pawn);
            while !nav.is_null() {
                let nav_location = location_of(nav);
                if abs_f(nav_location.z - (*pawn).base.location.z) < (*pawn).base.collision_height
                    && planar_dist_sq(pawn, nav_location) < max_dist_sq
                {
                    on_path = true;
                    if nav == actor || (svc.can_move_to)(raw_pawn, nav, actor) != 0 {
                        return 1;
                    }
                }
                nav = (*nav.cast::<ANavigationPoint>())
                    .next_navigation_point
                    .cast::<c_void>();
            }
            if on_path && (*pawn).base.physics != PHYS_FLYING {
                return 0;
            }
        }
        if !real_actor.is_null() {
            actor = real_actor;
        }
        (svc.actor_reachable)(raw_pawn, actor) as u32
    }
}

/// `APawn::execFindPathTo` after `P_FINISH`. Returns `bestPath`. The clock stays in C++.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `point` must point to a live `FVector`.
pub unsafe fn find_path_to(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    point: *const c_void,
    single: i32,
    clear: i32,
) -> *mut c_void {
    unsafe {
        let mut new_path = ptr::null_mut::<c_void>();
        let found = (svc.find_path_to)(pawn.cast(), point, single, clear, &raw mut new_path);
        let best = if found != 0 {
            new_path
        } else {
            ptr::null_mut()
        };
        settle_best_path(svc, pawn, best)
    }
}

/// `APawn::execFindPathToward` after the `!goal` check. Returns `bestPath`. The clock
/// stays in C++.
///
/// # Safety
/// `pawn` must point to a live `APawn` and `goal` to a live `AActor`.
pub unsafe fn find_path_toward(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    single: i32,
    clear: i32,
) -> *mut c_void {
    unsafe {
        let mut new_path = ptr::null_mut::<c_void>();
        let found = (svc.find_path_toward)(pawn.cast(), goal, single, clear, &raw mut new_path);
        let best = if found != 0 {
            new_path
        } else {
            ptr::null_mut()
        };
        settle_best_path(svc, pawn, best)
    }
}

/// `APawn::execFindRandomDest` body between the clock calls. `clearPaths()` runs
/// first when `clear` is set. Returns `bestPath`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn find_random_dest(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    clear: i32,
) -> *mut c_void {
    unsafe {
        if clear != 0 {
            (svc.clear_paths)(pawn.cast());
        }
        let mut new_path = ptr::null_mut::<c_void>();
        if (svc.find_random_dest)(pawn.cast(), &raw mut new_path) != 0 {
            new_path
        } else {
            ptr::null_mut()
        }
    }
}

/// `AActor::execMakeNoise` body: `CheckNoiseHearing` unless the level runs as a client.
///
/// # Safety
/// `actor` must point to a live `AActor`.
pub unsafe fn make_noise(svc: &Hp2PawnNativeMoveServices, actor: *mut c_void, loudness: f32) {
    unsafe {
        if (svc.net_mode)(actor) != NM_CLIENT {
            (svc.check_noise_hearing)(actor, loudness);
        }
    }
}

/// `APawn::execMoveTo` body after `P_GET_*`/`P_FINISH`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn move_to(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    dest: FVector,
    speed: f32,
    latent: i32,
) {
    unsafe {
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).move_target = ptr::null_mut();
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = clamped_desired_speed((*pawn).max_desired_speed, speed);
        (svc.set_move_timer)(pawn.cast(), move_size);
        set_latent(svc, pawn, latent);
        (*pawn).destination = dest;
        (*pawn).focus = dest;
        rotate_focus(svc, pawn);
        move_destination(svc, pawn);
    }
}

/// `APawn::execPollMoveTo` and `APawn::execPollStrafeTo` (identical bodies).
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn poll_destination(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        rotate_focus(svc, pawn);
        if move_destination(svc, pawn) != 0 {
            set_latent(svc, pawn, 0);
        }
    }
}

/// `APawn::execMoveToward` body after `P_GET_*`/`P_FINISH`. A null `goal` returns
/// before anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn move_toward(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    speed: f32,
    latent: i32,
) {
    unsafe {
        if goal.is_null() {
            return;
        }
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = clamped_desired_speed((*pawn).max_desired_speed, speed);
        if (svc.is_a)(goal, CLASS_PAWN) != 0 {
            (*pawn).move_timer = 1.2;
        } else {
            let move_size = vec::size(vec::sub(location_of(goal), (*pawn).base.location));
            (svc.set_move_timer)(pawn.cast(), move_size);
        }
        (*pawn).move_target = goal;
        (*pawn).destination = location_of(goal);
        (*pawn).focus = (*pawn).destination;
        set_latent(svc, pawn, latent);
        rotate_focus(svc, pawn);
        move_destination(svc, pawn);
    }
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
pub unsafe fn poll_move_toward(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        let target = (*pawn).move_target;
        if target.is_null() {
            set_latent(svc, pawn, 0);
            return;
        }
        (*pawn).destination = location_of(target);
        let physics = (*pawn).base.physics;
        if physics == PHYS_FLYING && (svc.is_a)(target, CLASS_PAWN) != 0 {
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
        rotate_focus(svc, pawn);
        let old_desired_speed = (*pawn).desired_speed;
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            (svc.event_alter_destination)(pawn.cast());
        }
        if move_destination(svc, pawn) != 0 {
            set_latent(svc, pawn, 0);
        }
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            (*pawn).destination = location_of((*pawn).move_target);
        }
        let current = (*pawn).move_target;
        if (svc.is_a)(current, CLASS_PAWN) != 0 {
            (*pawn).desired_speed = old_desired_speed;
            if !(*pawn).b_can_swim() {
                let zone = (*current.cast::<AActor>()).region.zone.cast::<AZoneInfo>();
                if (*zone).b_water_zone() {
                    (*pawn).move_timer = -1.0;
                }
            }
        }
    }
}

/// `APawn::execStrafeTo` body after `P_GET_VECTOR` x2.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn strafe_to(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    dest: FVector,
    focal: FVector,
    latent: i32,
) {
    unsafe {
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).move_target = ptr::null_mut();
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = strafe_desired_speed(pawn);
        (svc.set_move_timer)(pawn.cast(), move_size);
        set_latent(svc, pawn, latent);
        (*pawn).destination = dest;
        (*pawn).focus = focal;
        rotate_focus(svc, pawn);
        move_destination(svc, pawn);
    }
}

/// `APawn::execStrafeFacing` body after `P_FINISH`. A null `goal` returns before
/// anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn strafe_facing(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    dest: FVector,
    goal: *mut c_void,
    latent: i32,
) {
    unsafe {
        if goal.is_null() {
            return;
        }
        let move_size = vec::size(vec::sub(dest, (*pawn).base.location));
        (*pawn).set_b_reduced_speed(false);
        (*pawn).desired_speed = strafe_desired_speed(pawn);
        (svc.set_move_timer)(pawn.cast(), move_size);
        (*pawn).destination = dest;
        (*pawn).face_target = goal;
        (*pawn).focus = location_of(goal);
        set_latent(svc, pawn, latent);
        rotate_focus(svc, pawn);
        move_destination(svc, pawn);
    }
}

/// `APawn::execPollStrafeFacing` body. `Destination` is restored after the poll,
/// so the advanced-tactics `eventAlterDestination` change does not persist.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `face_target` must be null or a live `AActor`.
pub unsafe fn poll_strafe_facing(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        let face = (*pawn).face_target;
        if face.is_null() {
            set_latent(svc, pawn, 0);
            return;
        }
        (*pawn).focus = location_of(face);
        let real_dest = (*pawn).destination;
        rotate_focus(svc, pawn);
        if (*pawn).b_advanced_tactics() && (*pawn).base.physics == PHYS_WALKING {
            (svc.event_alter_destination)(pawn.cast());
        }
        if move_destination(svc, pawn) != 0 {
            set_latent(svc, pawn, 0);
        }
        (*pawn).destination = real_dest;
    }
}

/// `APawn::execTurnToward` body after `P_FINISH`. A null `goal` returns before
/// anything changes, as the C++ does.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `goal` must be null or a live `AActor`.
pub unsafe fn turn_toward(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    goal: *mut c_void,
    latent: i32,
) {
    unsafe {
        if goal.is_null() {
            return;
        }
        (*pawn).face_target = goal;
        set_latent(svc, pawn, latent);
        accelerate_if_airborne(svc, pawn);
        (*pawn).focus = location_of(goal);
        rotate_focus(svc, pawn);
    }
}

/// `APawn::execPollTurnToward` body.
///
/// # Safety
/// `pawn` must point to a live `APawn`. `face_target` must be null or a live `AActor`.
pub unsafe fn poll_turn_toward(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        let face = (*pawn).face_target;
        if face.is_null() {
            set_latent(svc, pawn, 0);
            return;
        }
        accelerate_if_airborne(svc, pawn);
        (*pawn).focus = location_of(face);
        if rotate_focus(svc, pawn) != 0 {
            set_latent(svc, pawn, 0);
        }
    }
}

/// `APawn::execTurnTo` body after `P_GET_VECTOR`/`P_FINISH`. On walking pawns
/// the focus stays at the current height.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn turn_to(
    svc: &Hp2PawnNativeMoveServices,
    pawn: *mut APawn,
    focal: FVector,
    latent: i32,
) {
    unsafe {
        (*pawn).move_target = ptr::null_mut();
        set_latent(svc, pawn, latent);
        (*pawn).focus = focal;
        if (*pawn).base.physics == PHYS_WALKING {
            (*pawn).focus.z = (*pawn).base.location.z;
        }
        accelerate_if_airborne(svc, pawn);
        rotate_focus(svc, pawn);
    }
}

/// `APawn::execPollTurnTo` body.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn poll_turn_to(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        accelerate_if_airborne(svc, pawn);
        if rotate_focus(svc, pawn) != 0 {
            set_latent(svc, pawn, 0);
        }
    }
}

/// `APawn::ShowSelf` body between the clock calls. Every other pawn on the level
/// that is probing `NAME_SeePlayer`, has a clear line of sight to this pawn, and
/// is not in its sight cooldown gets `eventSeePlayer(this)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` on the game thread.
pub unsafe fn show_self(svc: &Hp2PawnNativeMoveServices, pawn: *mut APawn) {
    unsafe {
        let raw_pawn = pawn.cast::<c_void>();
        let mut other = (svc.level_pawn_list)(raw_pawn).cast::<APawn>();
        while !other.is_null() {
            let raw_other = other.cast::<c_void>();
            if other != pawn
                && (*other).sight_counter < 0.0
                && (svc.is_probing)(raw_other, NAME_SEE_PLAYER) != 0
                && (svc.line_of_sight_to)(raw_other, raw_pawn, 1) != 0
            {
                (svc.event_see_player)(raw_other, raw_pawn);
            }
            other = (*other).next_pawn.cast::<APawn>();
        }
    }
}
