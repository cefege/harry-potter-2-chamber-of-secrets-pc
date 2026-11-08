//! Reachability group of `APawn` (`Engine/Src/UnPawn.cpp`): `actorReachable`,
//! `pointReachable`, `Reachable`, `flyReachable`, `swimReachable`,
//! `walkReachable`, `jumpReachable`, `jumpLanding`, `FindJumpUp`,
//! `SuggestJumpVelocity` and `FindBestJump`.
//!
//! Calls to the other public APawn methods (`walkMove`, `flyMove`,
//! `swimMove`, the `*Reachable` family and the jump helpers) are direct Rust
//! calls under [`guarded`] with the original guard name, so unwind history
//! matches the C++ shells they replace. Calls outside [`Hp2PawnServices`]
//! (`FarMoveActor` with other flags, `SingleLineCheck`, `PointRegion`,
//! `FastLineCheck`, `IsA`, `GIsEditor`) come from [`Hp2PawnReachServices`].
//! `FindJumpUp` never reads `Dest`, `Landing` or `moveActor` in the original
//! body, so its port takes only `vel`.
//!
//! FMA sites follow the objdump of the original `UnPawn.o`; see the
//! `mul_add` calls below. Quirks kept on purpose: `Hit` in `walkReachable` is
//! one object across loop iterations, `!R_FLY` is logical not, and
//! `MoveSizeSquared` in `walkReachable` is computed once before the loop.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APawn, AWarpZoneMarker, AZoneInfo, FCheckResult, FPointRegion,
};

use crate::math::app_sqrt;
use crate::native_guard::guarded;
use crate::pawn::{Hp2PawnServices, PawnResult, checked, with_hit};
use crate::prim::{self, max_f, min_f};
use crate::reach::{R_FLY, R_SWIM};
use crate::vec;

/// `R_WALK`, `R_JUMP` (`route.rs` keeps its copies private).
const R_WALK: i32 = 1;
const R_JUMP: i32 = 8;

/// `EPhysics` values (`EngineClasses.h`).
const PHYS_WALKING: u8 = 1;
const PHYS_SWIMMING: u8 = 3;
const PHYS_FLYING: u8 = 4;

/// `Object->IsA(...)` kinds accepted by [`Hp2PawnReachServices::is_a`].
const CLASS_PAWN: i32 = 0;
const CLASS_INVENTORY: i32 = 1;
const CLASS_TRIGGER: i32 = 2;
const CLASS_WARP_ZONE_MARKER: i32 = 3;

/// Checked native leaves. Published once by `Hp2PawnReachServices.cpp` for
/// the callers outside this module (path, route, native move).
#[repr(C)]
pub struct Hp2PawnReachServices {
    pub pawn: Hp2PawnServices,
    pub far_move_actor_flags:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, i32, i32, *mut i32) -> i32,
    pub single_line_check: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
    ) -> i32,
    pub point_region:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void) -> i32,
    pub fast_line_check:
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, *mut i32) -> i32,
    pub is_a: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub is_editor: unsafe extern "C" fn(*mut i32) -> i32,
}

static mut PAWN_REACH_SERVICES: Option<&'static Hp2PawnReachServices> = None;

/// Store the C++ table.
///
/// # Safety
/// `services` is the program-lifetime `GHp2PawnReachServices`. Publication
/// happens once during static initialization, before any engine call.
pub unsafe fn publish_services(services: &'static Hp2PawnReachServices) {
    unsafe { (&raw mut PAWN_REACH_SERVICES).write(Some(services)) };
}

/// The table stored by [`publish_services`]; `Err(1)` before publication.
///
/// # Safety
/// No concurrent [`publish_services`].
pub unsafe fn published_services() -> PawnResult<&'static Hp2PawnReachServices> {
    unsafe { (&raw const PAWN_REACH_SERVICES).read().ok_or(1) }
}

/// `Zone->bWaterZone`. The zone is dereferenced unchecked, as in the original.
fn water(zone: *mut c_void) -> bool {
    // SAFETY: `zone` is the live `Region.Zone` of a pawn (C++ dereferences it unchecked).
    unsafe { (*zone.cast::<AZoneInfo>()).b_water_zone() }
}

/// `Zone->bPainZone`.
fn pain(zone: *mut c_void) -> bool {
    // SAFETY: same contract as [`water`].
    unsafe { (*zone.cast::<AZoneInfo>()).b_pain_zone() }
}

/// `Zone->DamageType`.
fn damage(zone: *mut c_void) -> [u8; 4] {
    // SAFETY: same contract as [`water`].
    unsafe { (*zone.cast::<AZoneInfo>()).damage_type }
}

/// Subject pawn plus its services. Every read of pawn state goes through the
/// raw pointer again, because moves change `Location`, `Region` and
/// `FootRegion` under the body.
struct Reach<'a> {
    svc: &'a Hp2PawnReachServices,
    pawn: *mut APawn,
}

impl Reach<'_> {
    fn x_level(&self) -> *mut c_void {
        // SAFETY: `pawn` is live (see the entry points' `# Safety`).
        unsafe { (*self.pawn).base.x_level }
    }

    fn loc(&self) -> FVector {
        unsafe { (*self.pawn).base.location }
    }

    fn velocity(&self) -> FVector {
        unsafe { (*self.pawn).base.velocity }
    }

    fn set_velocity(&self, v: FVector) {
        unsafe { (*self.pawn).base.velocity = v }
    }

    fn region_zone(&self) -> *mut c_void {
        unsafe { (*self.pawn).base.region.zone }
    }

    fn region_number(&self) -> u8 {
        unsafe { (*self.pawn).base.region.zone_number }
    }

    fn foot_zone(&self) -> *mut c_void {
        unsafe { (*self.pawn).foot_region.zone }
    }

    fn physics(&self) -> u8 {
        unsafe { (*self.pawn).base.physics }
    }

    fn cr(&self) -> f32 {
        unsafe { (*self.pawn).base.collision_radius }
    }

    fn ch(&self) -> f32 {
        unsafe { (*self.pawn).base.collision_height }
    }

    fn max_step(&self) -> f32 {
        unsafe { (*self.pawn).max_step_height }
    }

    fn set_max_step(&self, v: f32) {
        unsafe { (*self.pawn).max_step_height = v }
    }

    fn base_eye_height(&self) -> f32 {
        unsafe { (*self.pawn).base_eye_height }
    }

    fn melee_range(&self) -> f32 {
        unsafe { (*self.pawn).melee_range }
    }

    fn ground_speed(&self) -> f32 {
        unsafe { (*self.pawn).ground_speed }
    }

    fn jump_z(&self) -> f32 {
        unsafe { (*self.pawn).jump_z }
    }

    fn reduced(&self) -> [u8; 4] {
        unsafe { (*self.pawn).reduced_damage_type }
    }

    fn can_swim(&self) -> bool {
        unsafe { (*self.pawn).b_can_swim() }
    }

    fn can_walk(&self) -> bool {
        unsafe { (*self.pawn).b_can_walk() }
    }

    fn can_fly(&self) -> bool {
        unsafe { (*self.pawn).b_can_fly() }
    }

    fn can_jump(&self) -> bool {
        unsafe { (*self.pawn).b_can_jump() }
    }

    fn is_editor(&self) -> PawnResult<bool> {
        let mut out = 0;
        checked(unsafe { (self.svc.is_editor)(&mut out) })?;
        Ok(out != 0)
    }
    fn is_a(&self, object: *mut c_void, kind: i32) -> PawnResult<bool> {
        let mut out = 0;
        checked(unsafe { (self.svc.is_a)(object, kind, &mut out) })?;
        Ok(out != 0)
    }
    fn far_move_flags(&self, dest: FVector, test: i32, no_check: i32) -> PawnResult<i32> {
        let mut out = 0;
        checked(unsafe {
            (self.svc.far_move_actor_flags)(
                self.x_level(),
                self.pawn.cast(),
                (&dest as *const FVector).cast(),
                test,
                no_check,
                &mut out,
            )
        })?;
        Ok(out)
    }
    fn far_move(&self, dest: FVector) -> PawnResult<()> {
        let mut out = 0;
        checked(unsafe {
            (self.svc.pawn.far_move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&dest as *const FVector).cast(),
                &mut out,
            )
        })
    }
    fn move_actor(&self, delta: FVector, hit: *mut FCheckResult) -> PawnResult<i32> {
        let mut out = 0;
        checked(unsafe {
            (self.svc.pawn.move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&delta as *const FVector).cast(),
                std::ptr::addr_of!((*self.pawn).base.rotation).cast(),
                hit.cast(),
                &mut out,
            )
        })?;
        Ok(out)
    }
    fn call_walk_move(
        &self,
        delta: FVector,
        hit: *mut FCheckResult,
        goal: *mut AActor,
        threshold: f32,
        adjust: i32,
    ) -> PawnResult<i32> {
        unsafe { crate::pawn::walk_move(&self.svc.pawn, self.pawn, delta, hit, goal, threshold, adjust) }
    }
    fn call_fly_move(
        &self,
        delta: FVector,
        goal: *mut AActor,
        threshold: f32,
        adjust: i32,
    ) -> PawnResult<i32> {
        unsafe { crate::pawn::fly_move(&self.svc.pawn, self.pawn, delta, goal, threshold, adjust) }
    }
    fn call_swim_move(
        &self,
        delta: FVector,
        goal: *mut AActor,
        threshold: f32,
        adjust: i32,
    ) -> PawnResult<i32> {
        unsafe { crate::pawn::swim_move(&self.svc.pawn, self.pawn, delta, goal, threshold, adjust) }
    }
    fn call_jump_landing(
        &self,
        test_vel: FVector,
        landing: *mut FVector,
        move_pawn: i32,
    ) -> PawnResult<()> {
        unsafe { jump_landing(self.svc, self.pawn, test_vel, landing, move_pawn) }
    }
    fn call_find_jump_up(&self, vel: FVector) -> PawnResult<i32> {
        unsafe { find_jump_up(self.svc, self.pawn, vel) }
    }
    fn call_suggest_jump_velocity(&self, dest: FVector, vel: *mut FVector) -> PawnResult<()> {
        unsafe { suggest_jump_velocity(self.pawn, dest, vel) }
    }
    fn call_find_best_jump(
        &self,
        dest: FVector,
        vel: FVector,
        landing: *mut FVector,
        move_pawn: i32,
    ) -> PawnResult<i32> {
        unsafe { find_best_jump(self.svc, self.pawn, dest, vel, landing, move_pawn) }
    }
    fn call_reachable(&self, dest: FVector, threshold: f32, goal: *mut AActor) -> PawnResult<i32> {
        unsafe { reachable(self.svc, self.pawn, dest, threshold, goal) }
    }
    fn call_walk_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe { walk_reachable(self.svc, self.pawn, dest, threshold, flags, goal) }
    }
    fn call_fly_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe { fly_reachable(self.svc, self.pawn, dest, threshold, flags, goal) }
    }
    fn call_swim_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe { swim_reachable(self.svc, self.pawn, dest, threshold, flags, goal) }
    }

    /// `if (!success && GoalActor && GoalActor->IsA(AWarpZoneMarker)) success = (Region.Zone == markedWarpZone)`.
    fn warp_zone_success(&self, success: i32, goal: *mut AActor) -> PawnResult<i32> {
        unsafe {
            if success == 0
                && !goal.is_null()
                && self.is_a(goal.cast::<c_void>(), CLASS_WARP_ZONE_MARKER)?
            {
                // SAFETY: `goal` passed `IsA(AWarpZoneMarker)`, so it is one.
                let marked = (*goal.cast::<AWarpZoneMarker>()).marked_warp_zone;
                Ok(i32::from(self.region_zone() == marked))
            } else {
                Ok(success)
            }
        }
    }

    /// `APawn::actorReachable(Other, bKnowVisible)`.
    fn actor_reachable(&self, other: *mut AActor, know_visible: i32) -> PawnResult<i32> {
        unsafe {
            if other.is_null() {
                return Ok(0);
            }
            // SAFETY: `other` is a live actor (the C++ dereferences it unchecked).
            unsafe {
                let other_loc = (*other).location;
                let dir = vec::sub(other_loc, self.loc());
                let distsq = vec::size_squared(dir);
                let is_pawn = self.is_a(other.cast::<c_void>(), CLASS_PAWN)?;

                if !is_pawn {
                    if !self.is_editor()? {
                        // only look past 800 for pawns
                        if distsq > 640000.0 {
                            return Ok(0);
                        }
                        if pain((*other).region.zone)
                            && damage((*other).region.zone) != self.reduced()
                        {
                            return Ok(0);
                        }
                    }
                } else {
                    let foot = (*other.cast::<APawn>()).foot_region.zone;
                    if pain(foot) && damage(foot) != self.reduced() {
                        return Ok(0);
                    }
                }

                if water((*other).region.zone) && !self.can_swim() {
                    return Ok(0);
                }

                // check other visible
                if know_visible == 0 {
                    let visible = with_hit(&self.svc.pawn, |hit| {
                        let mut view = self.loc();
                        view.z += self.base_eye_height(); // look from eyes
                        checked((self.svc.single_line_check)(
                            self.x_level(),
                            self.pawn.cast::<c_void>(),
                            hit.cast(),
                            std::ptr::addr_of!((*other).location).cast(),
                            (&view as *const FVector).cast(),
                        ))?;
                        if (*hit).time != 1.0 && (*hit).actor != other.cast::<c_void>() {
                            return Ok(false);
                        }
                        Ok(true)
                    })?;
                    if !visible {
                        return Ok(0);
                    }
                }

                if self.is_a(other.cast::<c_void>(), CLASS_PAWN)? {
                    let cr = self.cr();
                    let mut threshold =
                        cr + min_f(1.5 * cr, self.melee_range()) + (*other).collision_radius;
                    let thresholdsq = threshold * threshold;
                    if distsq <= thresholdsq {
                        return Ok(1);
                    } else if distsq > 640000.0 {
                        let dist = vec::size(dir);
                        threshold = max_f(dist - 800.0, threshold);
                    }
                    let real_loc = self.loc();
                    let mut a_point = (*other).location; // adjust destination
                    if self.far_move_flags((*other).location, 1, 0)? != 0 {
                        a_point = self.loc();
                        self.far_move(real_loc)?;
                    }
                    Ok(self.call_reachable(a_point, threshold, other)?)
                } else {
                    let mut threshold = 15.0f32;
                    if self.is_a(other.cast::<c_void>(), CLASS_INVENTORY)?
                        || self.is_a(other.cast::<c_void>(), CLASS_TRIGGER)?
                    {
                        threshold = self.cr() + (*other).collision_radius - 2.0;
                    }
                    let real_loc = self.loc();
                    let mut a_point = (*other).location;
                    if self.far_move_flags((*other).location, 1, 0)? != 0 {
                        a_point = self.loc(); // adjust destination
                        self.far_move(real_loc)?;
                    }
                    if (*other).b_block_actors()
                        || self.is_a(other.cast::<c_void>(), CLASS_WARP_ZONE_MARKER)?
                    {
                        Ok(self.call_reachable(a_point, threshold, other)?)
                    } else {
                        Ok(self.call_reachable(a_point, threshold, null_mut())?)
                    }
                }
            }
        }
    }

    /// `APawn::pointReachable(aPoint, bKnowVisible)`.
    fn point_reachable(&self, a_point: FVector, know_visible: i32) -> PawnResult<i32> {
        unsafe {
            let mut a_point = a_point;
            if !self.is_editor()? {
                let mut dir2d = vec::sub(a_point, self.loc());
                dir2d.z = 0.0;
                if vec::size_squared(dir2d) > 640000.0 {
                    // points must be within 800.f
                    return Ok(0);
                }
            }

            // SAFETY: `FPointRegion` is plain old data; all-zero is a valid value and
            // the service overwrites it.
            let mut new_region: FPointRegion = unsafe { std::mem::zeroed() };
            // SAFETY: the out-pointer and `a_point` live for the call.
            unsafe {
                checked((self.svc.point_region)(
                    self.x_level(),
                    self.pawn.cast::<c_void>(),
                    (&a_point as *const FVector).cast(),
                    (&mut new_region as *mut FPointRegion).cast(),
                ))?;
            }
            let new_zone = new_region.zone;
            if !water(self.region_zone()) && !self.can_swim() && water(new_zone) {
                return Ok(0);
            }
            if !pain(self.foot_zone()) && pain(new_zone) && damage(new_zone) != self.reduced() {
                return Ok(0);
            }

            // check aPoint visible
            if know_visible == 0 {
                let visible = with_hit(&self.svc.pawn, |_hit| {
                    let mut view = self.loc();
                    view.z += self.base_eye_height(); // look from eyes
                    // SAFETY: both vectors live for the call.
                    let mut visible = 0;
                    unsafe {
                        checked((self.svc.fast_line_check)(
                            self.x_level(),
                            (&a_point as *const FVector).cast(),
                            (&view as *const FVector).cast(),
                            &mut visible,
                        ))?;
                    };
                    Ok(visible)
                })?;
                if visible == 0 {
                    return Ok(0);
                }
            }

            let real_loc = self.loc();
            if self.far_move_flags(a_point, 1, 0)? != 0 {
                a_point = self.loc(); // adjust destination
                self.far_move(real_loc)?;
            }
            Ok(self.call_reachable(a_point, 15.0, null_mut())?)
        }
    }

    /// `APawn::Reachable(aPoint, Threshold, GoalActor)`.
    fn reachable(&self, a_point: FVector, threshold: f32, goal: *mut AActor) -> PawnResult<i32> {
        unsafe {
            if water(self.region_zone()) {
                return Ok(self.call_swim_reachable(a_point, threshold, 0, goal)?);
            }
            let phys = self.physics();
            if phys == PHYS_WALKING || phys == PHYS_SWIMMING {
                Ok(self.call_walk_reachable(a_point, threshold, 0, goal)?)
            } else if phys == PHYS_FLYING {
                Ok(self.call_fly_reachable(a_point, threshold, 0, goal)?)
            } else {
                // PHYS_Falling: jumpReachable is commented out in the original.
                Ok(0)
            }
        }
    }

    /// `APawn::flyReachable(Dest, Threshold, reachFlags, GoalActor)`.
    fn fly_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        reach_flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe {
            let mut reach_flags = reach_flags | R_FLY;
            let mut success = 0;
            let original_pos = self.loc();
            let real_vel = self.velocity();
            let mut stillmoving = 1;
            let close_sq = threshold * threshold;
            let movesize = max_f(200.0, self.cr());
            let movesize_sq = movesize * movesize;
            let mut ticks = 100;

            while stillmoving != 0 {
                let mut direction = vec::sub(dest, self.loc());
                let dist_sq = vec::size_squared(direction);
                if dist_sq > close_sq || direction.z.abs() > self.ch() {
                    // move not too small to do
                    if dist_sq < movesize_sq {
                        stillmoving = self.call_fly_move(direction, goal, 8.0, 0)?;
                    } else {
                        direction = prim::safe_normal(direction);
                        stillmoving =
                            self.call_fly_move(vec::scale(direction, movesize), goal, 4.1, 0)?;
                    }
                    if stillmoving == 5 {
                        // bumped into goal
                        stillmoving = 0;
                        success = 1;
                    }
                    if stillmoving != 0 && water(self.region_zone()) {
                        stillmoving = 0;
                        let rz = self.region_zone();
                        if self.can_swim() && (!pain(rz) || damage(rz) == self.reduced()) {
                            reach_flags =
                                self.call_swim_reachable(dest, threshold, reach_flags, goal)?;
                            success = reach_flags;
                        }
                    }
                } else {
                    stillmoving = 0;
                    success = 1;
                }
                ticks -= 1;
                if ticks < 0 {
                    stillmoving = 0;
                }
            }

            success = self.warp_zone_success(success, goal)?;
            self.far_move(original_pos)?; // move actor back to starting point
            self.set_velocity(real_vel);
            Ok(if success != 0 { reach_flags } else { 0 })
        }
    }

    /// `APawn::swimReachable(Dest, Threshold, reachFlags, GoalActor)`.
    fn swim_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        reach_flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe {
            let mut reach_flags = reach_flags | R_SWIM;
            let mut success = 0;
            let original_pos = self.loc();
            let real_vel = self.velocity();
            let mut stillmoving = 1;
            let close_sq = threshold * threshold;
            let movesize = max_f(200.0, self.cr());
            let movesize_sq = movesize * movesize;
            let mut ticks = 100;

            while stillmoving != 0 {
                let mut direction = vec::sub(dest, self.loc());
                let dist_sq = vec::size_squared(direction);
                if dist_sq > close_sq || direction.z.abs() > self.ch() {
                    // move not too small to do
                    if dist_sq < movesize_sq {
                        stillmoving = self.call_swim_move(direction, goal, 8.0, 0)?;
                    } else {
                        direction = prim::safe_normal(direction);
                        stillmoving =
                            self.call_swim_move(vec::scale(direction, movesize), goal, 4.1, 0)?;
                    }
                    if stillmoving == 5 {
                        // bumped into goal
                        stillmoving = 0;
                        success = 1;
                    }
                    if !water(self.region_zone()) {
                        stillmoving = 0;
                        if self.can_fly() {
                            reach_flags =
                                self.call_fly_reachable(dest, threshold, reach_flags, goal)?;
                            success = reach_flags;
                        } else if self.can_walk()
                        // 50 = default navigationpoint height
                        && dest.z < self.loc().z + 50.0 + self.max_step()
                        {
                            with_hit(&self.svc.pawn, |hit| {
                                let mut delta = vec::zero();
                                delta.z = max_f(self.ch() + self.max_step(), dest.z - self.loc().z);
                                self.move_actor(delta, hit)?;
                                if (*hit).time == 1.0 {
                                    success = self.call_fly_reachable(
                                        dest,
                                        threshold,
                                        reach_flags,
                                        goal,
                                    )?;
                                    // `!R_FLY` is logical not, so the masked term is always 0.
                                    reach_flags = R_WALK | (success & i32::from(R_FLY == 0));
                                }
                                Ok(())
                            })?;
                        }
                    } else {
                        let rz = self.region_zone();
                        if pain(rz) && damage(rz) != self.reduced() {
                            stillmoving = 0;
                            success = 0;
                        }
                    }
                } else {
                    stillmoving = 0;
                    success = 1;
                }
                ticks -= 1;
                if ticks < 0 {
                    stillmoving = 0;
                }
            }

            success = self.warp_zone_success(success, goal)?;
            self.far_move(original_pos)?; // move actor back to starting point
            self.set_velocity(real_vel);
            Ok(if success != 0 { reach_flags } else { 0 })
        }
    }

    /// `APawn::walkReachable(Dest, Threshold, reachFlags, GoalActor)`.
    fn walk_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        reach_flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe {
            let mut reach_flags = reach_flags | R_WALK;
            let mut success = 0;
            let original_pos = self.loc();
            let real_vel = self.velocity();
            let mut stillmoving = 1;
            let close_sq = threshold * threshold;
            let mut movesize = 16.0f32;
            if !self.is_editor()? {
                movesize = if self.can_jump() {
                    max_f(128.0, self.cr())
                } else {
                    self.cr()
                };
            }
            let mut ticks = 100;
            let movesize_sq = movesize * movesize;
            let mut max_zdiff = self.ch();
            if !goal.is_null() {
                // SAFETY: `goal` is a live actor (the caller's contract).
                max_zdiff = max_f(self.ch(), unsafe { (*goal).collision_height });
            }
            // One `Hit` for the whole loop, as in the original.
            with_hit(&self.svc.pawn, |hit| {
                while stillmoving == 1 {
                    let mut direction = vec::sub(dest, self.loc());
                    let zdiff = direction.z;
                    direction.z = 0.0; // this is a 2D move
                    let dist_sq = vec::size_squared(direction); // 2D size

                    // compared in double, with `0.8 * (Zdiff - MaxZDiff)^2` in double
                    let steep = (zdiff - max_zdiff) as f64;
                    if zdiff > max_zdiff && (dist_sq as f64) < 0.8 * steep * steep {
                        stillmoving = 0; // too steep to get there
                    } else {
                        if dist_sq > close_sq {
                            // move not too small to do
                            if dist_sq < movesize_sq {
                                stillmoving = self.call_walk_move(direction, hit, goal, 8.0, 0)?;
                            } else {
                                direction = prim::safe_normal(direction);
                                stillmoving = self.call_walk_move(
                                    vec::scale(direction, movesize),
                                    hit,
                                    goal,
                                    4.1,
                                    0,
                                )?;
                            }
                            if stillmoving != 1 {
                                if stillmoving == 5 {
                                    // bumped into goal
                                    stillmoving = 0;
                                    success = 1;
                                } else if self.region_number() == 0 {
                                    stillmoving = 0;
                                    success = 0;
                                } else if self.can_fly() {
                                    stillmoving = 0;
                                    reach_flags = self.call_fly_reachable(
                                        dest,
                                        threshold,
                                        reach_flags,
                                        goal,
                                    )?;
                                    success = reach_flags;
                                } else if self.can_jump() {
                                    reach_flags |= R_JUMP;
                                    if stillmoving == -1 {
                                        let mut landing = vec::zero();
                                        direction = prim::safe_normal(direction);
                                        stillmoving = self.call_find_best_jump(
                                            dest,
                                            vec::scale(direction, self.ground_speed()),
                                            &mut landing,
                                            1,
                                        )?;
                                    } else if stillmoving == 0 {
                                        direction = prim::safe_normal(direction);
                                        stillmoving = self.call_find_jump_up(vec::scale(
                                            direction,
                                            self.ground_speed(),
                                        ))?;
                                    }
                                } else if stillmoving == -1 && movesize > self.max_step() {
                                    // try smaller
                                    stillmoving = 1;
                                    movesize = self.max_step();
                                }
                            }
                            // FIXME in the original: "make sure fully on path" check is commented out.
                            let fz = self.foot_zone();
                            if pain(fz) && damage(fz) != self.reduced() {
                                stillmoving = 0;
                                success = 0;
                            }
                            if water(self.region_zone()) {
                                // swim from walk
                                stillmoving = 0;
                                let rz = self.region_zone();
                                if self.can_swim() && (!pain(rz) || damage(rz) == self.reduced()) {
                                    reach_flags = self.call_swim_reachable(
                                        dest,
                                        threshold,
                                        reach_flags,
                                        goal,
                                    )?;
                                    success = reach_flags;
                                }
                            }
                        } else {
                            // close enough to the destination (2D)
                            stillmoving = 0;
                            if zdiff.abs() < max_zdiff {
                                success = 1;
                            } else if ((*hit).normal.z as f64) < 0.95
                                && ((*hit).normal.z as f64) > 0.7
                            {
                                // check if above because of slope
                                let nz = (*hit).normal.z;
                                let slope = app_sqrt(f64::from(1.0f32 / (nz * nz) - 1.0f32));
                                if zdiff < 0.0
                                    && f64::from(-zdiff)
                                        < f64::from(self.cr()).mul_add(slope, f64::from(self.ch()))
                                {
                                    success = 1;
                                } else {
                                    // might be below because on slope
                                    let mut adj_rad = 46.0f32; // Navigation point default
                                    if !goal.is_null() {
                                        // SAFETY: `goal` is a live actor.
                                        adj_rad = unsafe { (*goal).collision_radius };
                                    }
                                    let rhs = f64::from(adj_rad + 15.0 - self.cr())
                                        .mul_add(slope, f64::from(max_zdiff));
                                    if self.cr() < adj_rad && f64::from(zdiff) < rhs {
                                        success = 1;
                                    }
                                }
                            }
                        }
                        ticks -= 1;
                        if ticks < 0 {
                            stillmoving = 0;
                        }
                    }
                }

                success = self.warp_zone_success(success, goal)?;
                self.far_move(original_pos)?; // move actor back to starting point
                self.set_velocity(real_vel);
                Ok(if success != 0 { reach_flags } else { 0 })
            })
        }
    }

    /// `APawn::jumpReachable(Dest, Threshold, reachFlags, GoalActor)`.
    fn jump_reachable(
        &self,
        dest: FVector,
        threshold: f32,
        reach_flags: i32,
        goal: *mut AActor,
    ) -> PawnResult<i32> {
        unsafe {
            let reach_flags = reach_flags | R_JUMP;
            let original_pos = self.loc();
            let mut landing = vec::zero();
            self.call_jump_landing(self.velocity(), &mut landing, 1)?;
            if landing == original_pos {
                return Ok(0);
            }
            let success = self.call_walk_reachable(dest, threshold, reach_flags, goal)?;
            self.far_move(original_pos)?; // move actor back to starting point
            Ok(success)
        }
    }

    /// `APawn::jumpLanding(testVel, Landing, movePawn)`.
    fn jump_landing(
        &self,
        test_vel: FVector,
        landing: *mut FVector,
        move_pawn: i32,
    ) -> PawnResult<()> {
        unsafe {
            let mut test_vel = test_vel;
            let original_pos = self.loc();
            let mut landed = false;
            let mut ticks = 0;
            let tick_time = 0.1f32;
            while !landed {
                // SAFETY: `Region.Zone` is live (re-read every tick, moves change it).
                let zone = self.region_zone().cast::<AZoneInfo>();
                let (friction, gravity, zone_vel) = unsafe {
                    (
                        (*zone).zone_fluid_friction,
                        (*zone).zone_gravity,
                        (*zone).zone_velocity,
                    )
                };
                // `1 - ZFF*tickTime` contracts to fma(-ZFF, tickTime, 1).
                let k = (-friction).mul_add(tick_time, 1.0);
                test_vel = vec::add(vec::scale(test_vel, k), vec::scale(gravity, tick_time));
                let adjusted = vec::scale(vec::add(test_vel, zone_vel), tick_time);
                with_hit(&self.svc.pawn, |hit| {
                    self.move_actor(adjusted, hit)?;
                    if water(self.region_zone()) {
                        landed = true;
                    } else if (*hit).time < 1.0 {
                        if (*hit).normal.z > 0.7f32 {
                            landed = true;
                        } else {
                            let old_hit_normal = (*hit).normal;
                            let mut delta = vec::scale(
                                vec::sub(
                                    adjusted,
                                    vec::scale((*hit).normal, prim::dot(adjusted, (*hit).normal)),
                                ),
                                1.0 - (*hit).time,
                            );
                            if prim::dot(delta, adjusted) >= 0.0 {
                                self.move_actor(delta, hit)?;
                                if (*hit).time < 1.0 {
                                    // hit second wall
                                    if ((*hit).normal.z as f64) > 0.7 {
                                        landed = true;
                                    }
                                    // Inline `AActor::TwoWallAdjust` (`AActor.h`): pure math, only `Delta` is written.
                                    crate::physic::two_wall_adjust(
                                        prim::safe_normal(adjusted),
                                        &mut delta,
                                        (*hit).normal,
                                        old_hit_normal,
                                        (*hit).time,
                                    );
                                    self.move_actor(delta, hit)?;
                                    if ((*hit).normal.z as f64) > 0.7 {
                                        landed = true;
                                    }
                                }
                            }
                        }
                    }
                    ticks += 1;
                    if self.region_number() == 0
                        || ticks > 35
                        || vec::size_squared(test_vel) > 2500000.0
                    {
                        self.far_move(original_pos)?; // move actor back to starting point
                        landed = true;
                    }
                    Ok(())
                })?;
            }

            *landing = self.loc();
            if move_pawn == 0 {
                self.far_move(original_pos)?; // move actor back to starting point
            }
            Ok(())
        }
    }

    /// `APawn::FindJumpUp(Dest, vel, Landing, moveActor)`.
    fn find_jump_up(&self, vel: FVector) -> PawnResult<i32> {
        unsafe {
            let real_step = self.max_step();
            self.set_max_step(48.0);
            let direction = prim::safe_normal(vel);
            with_hit(&self.svc.pawn, |hit| {
                let mut success =
                    self.call_walk_move(vec::scale(direction, real_step), hit, null_mut(), 4.1, 1)?;
                if success == 5 {
                    success = 1;
                }
                self.set_max_step(real_step);
                Ok(success)
            })
        }
    }


    /// `APawn::FindBestJump(Dest, vel, Landing, movePawn)`.
    fn find_best_jump(
        &self,
        dest: FVector,
        vel: FVector,
        landing: *mut FVector,
        move_pawn: i32,
    ) -> PawnResult<i32> {
        unsafe {
            let mut vel = vel;
            let real_location = self.loc();
            vel.z = self.jump_z();
            self.call_suggest_jump_velocity(dest, &mut vel)?;

            // Now imagine jump
            self.call_jump_landing(vel, landing, 1)?;
            let olddist = vec::sub(dest, real_location);
            let dist = vec::sub(dest, self.loc());
            // Same short-circuit order as the C++ `if / else if` pair, both giving 0.
            let success = if (pain(self.foot_zone()) && damage(self.foot_zone()) != self.reduced())
                || (!self.can_swim() && water(self.region_zone()))
            {
                0
            } else {
                let mut netchange = vec::size(olddist);
                netchange -= vec::size(dist);
                i32::from(netchange > 8.0)
            };
            if move_pawn == 0 {
                self.far_move(real_location)?; // move actor back to starting point
            }
            Ok(success)
        }
    }
}

/// `APawn::actorReachable(Other, bKnowVisible)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` with live level, region and foot-region
/// zones. `other` must be null or a live actor.
pub unsafe fn actor_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    other: *mut AActor,
    know_visible: i32,
) -> PawnResult<i32> {
    unsafe { guarded(c"AActor::actorReachable", || Reach { svc, pawn }.actor_reachable(other, know_visible)) }
}

/// `APawn::pointReachable(aPoint, bKnowVisible)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` with live level, region and foot-region zones.
pub unsafe fn point_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    a_point: FVector,
    know_visible: i32,
) -> PawnResult<i32> {
    unsafe { guarded(c"AActor::pointReachable", || Reach { svc, pawn }.point_reachable(a_point, know_visible)) }
}

/// `APawn::Reachable(aPoint, Threshold, GoalActor)`, under its original
/// `guard(AActor::Reachable)` (sic).
///
/// # Safety
/// `pawn` must point to a live `APawn` with live level and region zones. `goal`
/// must be null or a live actor.
pub unsafe fn reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    a_point: FVector,
    threshold: f32,
    goal: *mut AActor,
) -> PawnResult<i32> {
    unsafe { guarded(c"AActor::Reachable", || Reach { svc, pawn }.reachable(a_point, threshold, goal)) }
}

/// `APawn::walkReachable(Dest, Threshold, reachFlags, GoalActor)`, under its
/// original `guard(APawn::walkReachable)`.
///
/// # Safety
/// As [`reachable`].
pub unsafe fn walk_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    dest: FVector,
    threshold: f32,
    reach_flags: i32,
    goal: *mut AActor,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::walkReachable", || {
            Reach { svc, pawn }.walk_reachable(dest, threshold, reach_flags, goal)
        })
    }
}

/// `APawn::flyReachable(Dest, Threshold, reachFlags, GoalActor)`, under its
/// original `guard(APawn::flyReachable)`.
///
/// # Safety
/// As [`reachable`].
pub unsafe fn fly_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    dest: FVector,
    threshold: f32,
    reach_flags: i32,
    goal: *mut AActor,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::flyReachable", || {
            Reach { svc, pawn }.fly_reachable(dest, threshold, reach_flags, goal)
        })
    }
}

/// `APawn::swimReachable(Dest, Threshold, reachFlags, GoalActor)`, under its
/// original `guard(APawn::swimReachable)`.
///
/// # Safety
/// As [`reachable`].
pub unsafe fn swim_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    dest: FVector,
    threshold: f32,
    reach_flags: i32,
    goal: *mut AActor,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::swimReachable", || {
            Reach { svc, pawn }.swim_reachable(dest, threshold, reach_flags, goal)
        })
    }
}

/// `APawn::jumpReachable(Dest, Threshold, reachFlags, GoalActor)`.
///
/// # Safety
/// As [`reachable`].
pub unsafe fn jump_reachable(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    dest: FVector,
    threshold: f32,
    reach_flags: i32,
    goal: *mut AActor,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::jumpReachable", || {
            Reach { svc, pawn }.jump_reachable(dest, threshold, reach_flags, goal)
        })
    }
}

/// `APawn::jumpLanding(testVel, Landing, movePawn)`, under its original
/// `guard(APawn::jumpLanding)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` with live level and region zones.
pub unsafe fn jump_landing(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    test_vel: FVector,
    landing: *mut FVector,
    move_pawn: i32,
) -> PawnResult<()> {
    unsafe {
        guarded(c"APawn::jumpLanding", || {
            Reach { svc, pawn }.jump_landing(test_vel, landing, move_pawn)
        })
    }
}

/// `APawn::FindJumpUp(Dest, vel, Landing, moveActor)`, under its original
/// `guard(APawn::FindJumpUp)`. The original never reads `Dest`, `Landing` or
/// `moveActor`, so only `vel` is taken.
///
/// # Safety
/// As [`jump_landing`].
pub unsafe fn find_jump_up(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    vel: FVector,
) -> PawnResult<i32> {
    unsafe { guarded(c"APawn::FindJumpUp", || Reach { svc, pawn }.find_jump_up(vel)) }
}

/// `APawn::SuggestJumpVelocity(Dest, vel)`, under its original
/// `guard(APawn::SuggestJumpVelocity)`.
///
/// # Safety
/// `pawn` and its Region.Zone are live on the game thread; `vel` points to a
/// writable FVector. No published services or native-object callbacks are used.
pub unsafe fn suggest_jump_velocity(
    pawn: *mut APawn,
    dest: FVector,
    vel: *mut FVector,
) -> PawnResult<()> {
    unsafe {
        guarded(c"APawn::SuggestJumpVelocity", || {
            let mut grav_z = (*(*pawn).base.region.zone.cast::<AZoneInfo>()).zone_gravity.z;
            if grav_z >= 0.0 {
                // negative gravity - pretend its low gravity
                grav_z = -100.0;
            }
            let start_vel_z = (*vel).z;
            let floor = dest.z - (*pawn).base.location.z;
            let mut current_z = 0.0f32;
            let mut ticks = 0.0f32;
            while current_z > floor || (*vel).z > 0.0 {
                (*vel).z = grav_z.mul_add(0.05, (*vel).z);
                ticks += 0.05;
                current_z = (*vel).z.mul_add(0.05, current_z);
            }
            if (*vel).z.abs() > 1.0 {
                ticks -= (current_z - floor) / (*vel).z; // correct overshoot
            }
            *vel = vec::sub(dest, (*pawn).base.location);
            (*vel).z = 0.0;
            if ticks > 0.0 {
                let mut velsize = vec::size(*vel);
                if velsize > 0.0 {
                    *vel = vec::div(*vel, velsize);
                }
                velsize = min_f(1.0 * (*pawn).ground_speed, velsize / ticks);
                *vel = vec::scale(*vel, velsize);
            } else {
                *vel = prim::safe_normal(*vel);
                *vel = vec::scale(*vel, (*pawn).ground_speed);
            }
            (*vel).z = start_vel_z;
            Ok(())
        })
    }
}

/// `APawn::FindBestJump(Dest, vel, Landing, movePawn)`, under its original
/// `guard(APawn::FindBestJump)`.
///
/// # Safety
/// As [`jump_landing`].
pub unsafe fn find_best_jump(
    svc: &Hp2PawnReachServices,
    pawn: *mut APawn,
    dest: FVector,
    vel: FVector,
    landing: *mut FVector,
    move_pawn: i32,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::FindBestJump", || {
            Reach { svc, pawn }.find_best_jump(dest, vel, landing, move_pawn)
        })
    }
}
