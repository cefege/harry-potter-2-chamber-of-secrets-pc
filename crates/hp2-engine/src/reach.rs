//! Port of `Engine/Src/UnReach.cpp`: reach-spec queries and the
//! `findBestReachable` collision-size search.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, FReachSpec};

use crate::math::app_round;
use crate::services::Hp2EngineServices;
use crate::vec;

/// `APawn::R_FLY` (`APawn.h`).
pub const R_FLY: i32 = 2;
/// `APawn::R_SWIM` (`APawn.h`).
pub const R_SWIM: i32 = 4;

/// `UnPath.h` collision constants.
const MAXCOMMONRADIUS: i32 = 18;
const MAXCOMMONHEIGHT: i32 = 20;
const MINCOMMONHEIGHT: i32 = 39;
const MINCOMMONRADIUS: i32 = 18;
const COMMONRADIUS: i32 = 18;
const HUMANRADIUS: i32 = 18;
const HUMANHEIGHT: i32 = 39;

/// `PHYS_Walking` (`EngineClasses.h`).
const PHYS_WALKING: u8 = 1;

/// `Min(A, B)` from `UnTemplate.h` for `INT`.
fn min_i(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

/// `FReachSpec::MonsterPath()`.
///
/// # Safety
/// `spec` must point to a live `FReachSpec`.
pub unsafe fn monster_path(spec: *const FReachSpec) -> i32 {
    unsafe {
        ((*spec).collision_radius >= COMMONRADIUS
            && (*spec).collision_height >= MINCOMMONHEIGHT
            && ((*spec).reach_flags & R_FLY) == 0) as i32
    }
}

/// `FReachSpec::BotOnlyPath()`.
///
/// # Safety
/// `spec` must point to a live `FReachSpec`.
pub unsafe fn bot_only_path(spec: *const FReachSpec) -> i32 {
    unsafe { ((*spec).collision_radius < MINCOMMONRADIUS) as i32 }
}

/// `FReachSpec::operator+`. The C++ result leaves `Start`, `End` and
/// `bPruned` uninitialized. This port sets `Start` and `End` to null and
/// `bPruned` to 0.
///
/// # Safety
/// `a`, `b` must point to live `FReachSpec`s. `out` must be writable.
pub unsafe fn combine(a: *const FReachSpec, b: *const FReachSpec, out: *mut FReachSpec) {
    unsafe {
        (*out).distance = (*a).distance + (*b).distance;
        (*out).collision_radius = min_i((*a).collision_radius, (*b).collision_radius);
        (*out).collision_height = min_i((*a).collision_height, (*b).collision_height);
        (*out).reach_flags = (*a).reach_flags | (*b).reach_flags;
        (*out).start = ptr::null_mut();
        (*out).end = ptr::null_mut();
        (*out).b_pruned = 0;
    }
}

/// `FReachSpec::operator<=`.
///
/// # Safety
/// `a`, `b` must point to live `FReachSpec`s.
pub unsafe fn less_equal(a: *const FReachSpec, b: *const FReachSpec) -> i32 {
    unsafe {
        ((*a).collision_radius >= (*b).collision_radius
            && (*a).collision_height >= (*b).collision_height
            && ((*a).reach_flags | (*b).reach_flags) == (*b).reach_flags) as i32
    }
}

/// `FReachSpec::operator==`.
///
/// # Safety
/// `a`, `b` must point to live `FReachSpec`s.
pub unsafe fn equal(a: *const FReachSpec, b: *const FReachSpec) -> i32 {
    unsafe {
        ((*a).distance == (*b).distance
            && (*a).collision_radius == (*b).collision_radius
            && (*a).collision_height == (*b).collision_height
            && (*a).reach_flags == (*b).reach_flags) as i32
    }
}

/// `FReachSpec::defineFor(AActor* Begin, AActor* Dest, APawn* Scout)`.
///
/// # Safety
/// `svc` must be valid. `spec`, `begin`, `dest` and `scout` must point to
/// live objects.
pub unsafe fn define_for(
    svc: &Hp2EngineServices,
    spec: *mut FReachSpec,
    begin: *mut AActor,
    dest: *mut AActor,
    scout: *mut APawn,
) -> i32 {
    unsafe {
        (*spec).start = begin.cast::<c_void>();
        (*spec).end = dest.cast::<c_void>();

        (*scout).base.physics = PHYS_WALKING;
        (*scout).jump_z = 320.0;
        (*scout).set_b_can_walk(true);
        (*scout).set_b_can_jump(true);
        (*scout).set_b_can_swim(true);
        (*scout).set_b_can_fly(false);
        (*scout).ground_speed = 320.0;
        (*scout).max_step_height = 25.0;

        find_best_reachable(
            svc,
            spec,
            ptr::addr_of!((*begin).location),
            ptr::addr_of!((*dest).location),
            scout,
        )
    }
}

/// `FReachSpec::findBestReachable(FVector& Begin, FVector& Destination, APawn* Scout)`.
///
/// # Safety
/// `svc` must be valid. `spec`, `begin`, `destination` and `scout` must point
/// to live objects. `spec.start` and `spec.end` must point to live `AActor`s.
pub unsafe fn find_best_reachable(
    svc: &Hp2EngineServices,
    spec: *mut FReachSpec,
    begin: *const FVector,
    destination: *const FVector,
    scout: *mut APawn,
) -> i32 {
    unsafe {
        let level = (*scout).base.x_level;
        let scout_v = scout.cast::<c_void>();
        let set_size = |radius: f32, height: f32| {
            (svc.set_collision_size)(scout_v, radius, height);
        };

        set_size(HUMANRADIUS as f32, HUMANHEIGHT as f32);

        let mut result = 0;
        let mut stepsize: f32 = MAXCOMMONRADIUS as f32 - (*scout).base.collision_radius;
        let mut stilltrying = true;
        let mut best_radius: f32 = 0.0;
        let mut best_height: f32 = 0.0;

        // Find the largest radius that works.
        while stilltrying {
            let mut success = (svc.far_move_actor)(level, scout_v, begin.cast::<c_void>());
            if success != 0 {
                success = (svc.point_reachable)(scout_v, destination.cast::<c_void>());
            }

            if success != 0 {
                (*spec).reach_flags = success;
                result = 1;
                best_radius = (*scout).base.collision_radius;
                best_height = (*scout).base.collision_height;
                set_size(
                    (*scout).base.collision_radius + stepsize,
                    MINCOMMONHEIGHT as f32,
                );
                stepsize *= 0.5;
                if stepsize < 2.0 || (*scout).base.collision_radius > MAXCOMMONRADIUS as f32 {
                    stilltrying = false;
                }
            } else {
                set_size(
                    (*scout).base.collision_radius - stepsize,
                    (*scout).base.collision_height,
                );
                stepsize *= 0.5;
                if stepsize < 2.0 || (*scout).base.collision_radius < HUMANRADIUS as f32 {
                    stilltrying = false;
                }
            }
        }

        if result != 0 {
            // Find the largest height that works at the best radius.
            set_size(best_radius, (*scout).base.collision_height + 4.0);
            stilltrying = true;
            stepsize = MAXCOMMONHEIGHT as f32 - (*scout).base.collision_height;

            while stilltrying {
                let mut success = (svc.far_move_actor)(level, scout_v, begin.cast::<c_void>());
                if success != 0 {
                    success = (svc.point_reachable)(scout_v, destination.cast::<c_void>());
                }

                if success != 0 {
                    (*spec).reach_flags = success;
                    best_height = (*scout).base.collision_height;
                    set_size(
                        (*scout).base.collision_radius,
                        (*scout).base.collision_height + stepsize,
                    );
                    stepsize *= 0.5;
                    if stepsize < 1.0 || (*scout).base.collision_height > MAXCOMMONHEIGHT as f32 {
                        stilltrying = false;
                    }
                } else {
                    set_size(
                        (*scout).base.collision_radius,
                        (*scout).base.collision_height - stepsize,
                    );
                    stepsize *= 0.5;
                    if stepsize < 1.0 || (*scout).base.collision_height < (HUMANHEIGHT + 1) as f32 {
                        stilltrying = false;
                    }
                }
            }

            (*spec).collision_radius = app_round((*scout).base.collision_radius);
            (*spec).collision_height = app_round(best_height);

            let end = (*spec).end.cast::<AActor>();
            let start = (*spec).start.cast::<AActor>();
            let path = vec::sub((*end).location, (*start).location);
            (*spec).distance = app_round(vec::size(path));
            if ((*spec).reach_flags & R_SWIM) != 0 {
                (*spec).distance *= 2;
            }
        }

        result
    }
}
