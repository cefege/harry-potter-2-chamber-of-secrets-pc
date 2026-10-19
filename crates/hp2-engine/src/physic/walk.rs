//! Port of `APawn::physWalking` and the free function `Predict` (`UnPhysic.cpp`).
//! Engine calls go through `Hp2PhysicServices`. The falling, flying, swimming and
//! `setPhysics` transitions call the sibling families directly. The `LEGEND`
//! `CheckSurfaces` block is dead and is not ported.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, APlayerPawn, AZoneInfo, FCheckResult};

use super::{
    CLASS_PAWN, CLASS_PLAYER_PAWN, Hp2PhysicServices, PHYS_FALLING, PHYS_WALKING,
    TRACE_ALL_BLOCKING, TRACE_VIS_BLOCKING, ZERO, add, app_sqrt, blank_hit, calc_velocity, div,
    dot, is_a, is_zero, max_f, max_val, move_actor, neg, safe_normal, scale, size, size_sq,
    step_up, sub, v3,
};
use super::{dispatch, fall, swim};

/// `EPhysics` values not defined in `physic.rs` (`EngineClasses.h`).
const PHYS_FLYING: u8 = 4;
const PHYS_SWIMMING: u8 = 3;
const PHYS_PROJECTILE: u8 = 6;

/// `ENetRole::ROLE_Authority` (`EngineClasses.h`).
const ROLE_AUTHORITY: u8 = 4;

/// `KINDA_SMALL_NUMBER` from `UnMath.h`.
const KINDA_SMALL_NUMBER: f32 = 1.0e-4;

/// `Predict` loop bounds (`UnPhysic.cpp`).
const PREDICT_MAX_ITER: f32 = 0.2;
const PREDICT_MAX_ITERS: i32 = 10;

/// `physWalking` auto-jump thresholds (function-local statics).
const MIN_AUTO_JUMP_COS: f32 = 0.25;
const AUTO_JUMP_THRESHOLD: f32 = 10.0;

/// `Min(A, B)` from `UnTemplate.h`: `A<=B ? A : B`.
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `IsNearlyZero()`: every component below `KINDA_SMALL_NUMBER` in magnitude.
fn is_nearly_zero(a: FVector) -> bool {
    a.x.abs() < KINDA_SMALL_NUMBER
        && a.y.abs() < KINDA_SMALL_NUMBER
        && a.z.abs() < KINDA_SMALL_NUMBER
}

/// `Size2D()`: `appSqrt(X*X + Y*Y)` narrowed to `FLOAT`.
fn size_2d(a: FVector) -> f32 {
    // `-ffp-contract=on`: `X*X + Y*Y` fuses the first product into an FMA.
    app_sqrt(a.x.mul_add(a.x, a.y * a.y) as f64) as f32
}

/// `Region.Zone` as an `AZoneInfo`. Re-read at each use: engine calls such as
/// `MoveActor` can move the actor into another region.
unsafe fn zone(pawn: *mut APawn) -> *const AZoneInfo {
    unsafe { (*pawn).base.region.zone as *const AZoneInfo }
}

/// `GetLevel()->SingleLineCheck(hit, this, end, start, trace, extent)`.
unsafe fn line_check(
    svc: &Hp2PhysicServices,
    this: *mut c_void,
    hit: &mut FCheckResult,
    end: FVector,
    start: FVector,
    trace: i32,
    extent: &FVector,
) -> i32 {
    unsafe { (svc.single_line_check)(this, hit, &end, &start, trace, extent) }
}

/// Wall/floor test used after `MoveActor`: `Hit.Time < 1 && Normal.Z > 0.01 && Normal.Z < 0.7`.
fn is_slope(hit: &FCheckResult) -> bool {
    hit.time < 1.0 && hit.normal.z > 0.01 && hit.normal.z < 0.7
}

/// `Predict(Hit, Pawn, Start, Vel, Extent, Time)` (`UnPhysic.cpp`, free function).
/// Returns `false` when the path lands or mounts; `Hit.Location` and `Hit.Time`
/// then hold the stop point. Returns `true` when the whole time is traced.
///
/// # Safety
/// `pawn` must point to a live `APawn` whose `Region.Zone` is a live `AZoneInfo`.
pub unsafe fn predict(
    hit: &mut FCheckResult,
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    mut start: FVector,
    mut vel: FVector,
    extent: &FVector,
    time: f32,
) -> bool {
    unsafe {
        let this = pawn as *mut c_void;
        let mut t = 0.0f32;
        hit.normal = ZERO;
        let mut i = 0;
        while i < PREDICT_MAX_ITERS && t < time {
            let mut iter = min_f(PREDICT_MAX_ITER, time - t);

            // Conform gravity to last hit surface.
            let mut grav = (*zone(pawn)).zone_gravity;
            grav = sub(grav, scale(hit.normal, dot(grav, hit.normal)));

            let mut end = add(add(start, scale(vel, iter)), scale(grav, iter * iter * 0.5));
            (svc.single_line_check)(this, hit, &end, &start, TRACE_ALL_BLOCKING, extent);
            iter *= hit.time;
            t += iter;
            vel = add(vel, scale(grav, iter));

            if hit.time < 1.0 {
                end = add(scale(start, 1.0 - hit.time), scale(end, hit.time));
                if hit.normal.z > 0.7 {
                    // Landed.
                    hit.location = end;
                    hit.time = t / time;
                    return false;
                } else {
                    // Hit wall. Test for mount.
                    if (*pawn).max_mount_height > 0.0 && (hit.normal.z as f64) > -0.1 {
                        // Make sure pawn is facing in direction of wall.
                        let mut facing = ZERO;
                        (svc.rotation_vector)(&(*pawn).base.rotation, &mut facing);
                        if dot(hit.normal, facing) < 0.0
                            && (svc.bsp_surf_special)(hit, max_val(*extent)) != 0
                        {
                            // Shortcut: any hit on a mounting surface results in a mount.
                            hit.location = end;
                            hit.time = t / time;
                            return false;
                        }
                    }

                    // Conform velocity to hit.
                    vel = sub(vel, scale(hit.normal, dot(vel, hit.normal)));
                }
            } else {
                hit.normal = ZERO;
            }
            start = end;
            i += 1;
        }
        hit.location = start;
        hit.time = 1.0;
        true
    }
}

/// `APawn::physWalking(deltaTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `APawn` whose `Region.Zone` is a live `AZoneInfo`.
/// Engine calls made through `svc` can move the actor, so fields are re-read
/// after each call, as the C++ does.
pub unsafe fn phys_walking(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    mut iterations: i32,
) {
    unsafe {
        let actor = this as *mut AActor;
        let this_v = this as *mut c_void;

        if (*actor).region.zone_number == 0 {
            // not in valid spot
            if (*actor).role == ROLE_AUTHORITY {
                (svc.debugf_fell_out)(this_v);
            }
            (svc.event_fell_out_of_world)(this_v);
            return;
        }

        let mut ground_normal = v3(0.0, 0.0, 1.0);

        (*actor).acceleration.z = 0.0;
        let accel = (*actor).acceleration;
        let accel_dir = if is_zero(accel) {
            accel
        } else {
            safe_normal(accel)
        };

        let ground_friction = (*zone(this)).zone_ground_friction
            * if (*this).b_no_zone_friction() {
                0.0
            } else {
                1.0
            };
        calc_velocity(
            this,
            svc,
            accel_dir,
            delta_time,
            (*this).ground_speed,
            ground_friction,
            0,
            0,
            0,
        );

        let mut desired_move = (*actor).velocity;
        if is_a(svc, this_v, CLASS_PLAYER_PAWN) || size_sq((*zone(this)).zone_velocity) > 90000.0 {
            // Add effect of velocity zone. Rather than constant velocity, hacked to make sure
            // that velocity being clamped when walking doesn't cause the zone velocity to have
            // too much of an effect at fast frame rates.
            desired_move = add(
                desired_move,
                scale(scale((*zone(this)).zone_velocity, 25.0), delta_time),
            );
        }

        //-------------------------------------------------------------------------------------------
        //Perform the move
        let mut grav_dir = v3(0.0, 0.0, -1.0);
        if (*zone(this)).zone_gravity.z > 0.0 {
            grav_dir.z = 1.0;
        }
        let down = scale(grav_dir, (*this).max_step_height + 2.0);
        let mut extent = ZERO;
        (svc.cylinder_extent)(this_v, &mut extent);
        let mut hit = blank_hit(1.0);
        (*actor).old_location = (*actor).location;
        (*actor).set_b_just_teleported(false);
        let mut checked_fall = false;
        let mut must_jump = false;

        let mut remaining_time = delta_time;
        while remaining_time > 0.0 && iterations < 8 {
            iterations += 1;
            let time_tick = if remaining_time > 0.05
                && (is_a(svc, this_v, CLASS_PLAYER_PAWN)
                    || size_sq(desired_move) * remaining_time * remaining_time > 400.0)
            {
                min_f(0.05, remaining_time * 0.5)
            } else {
                remaining_time
            };
            remaining_time -= time_tick;
            let mut delta = scale(desired_move, time_tick);
            let sub_loc = (*actor).location;
            let mut sub_move = delta;
            let mut b_zero_move = is_nearly_zero(delta);
            if b_zero_move {
                remaining_time = 0.0;
                (*this).set_b_hit_sloped_wall(false);
            } else {
                let mut forward_check = scale(accel_dir, (*actor).collision_radius);
                if !(*this).b_avoid_ledges() {
                    forward_check = scale(forward_check, 0.5);
                }
                // if AI controlled, check for fall by doing trace forward
                // try to find reasonable walk along ledge
                if (!is_a(svc, this_v, CLASS_PLAYER_PAWN) || (*this).b_is_walking())
                    && !(*this).b_can_fly()
                {
                    // check if clear in front
                    let mut destn = add(add((*actor).location, delta), forward_check);
                    line_check(
                        svc,
                        this_v,
                        &mut hit,
                        destn,
                        (*actor).location,
                        TRACE_VIS_BLOCKING,
                        &ZERO,
                    );
                    if hit.time == 1.0 {
                        // clear in front - see if there is footing at walk destination
                        let desired_dist = size(delta);
                        // check down enough to catch either step or slope
                        let test_down = max_f(
                            4.0 + (*this).max_step_height + (*actor).collision_height,
                            4.0 + (*actor).collision_height
                                + (*actor).collision_radius
                                + desired_dist,
                        );
                        // try a point trace
                        line_check(
                            svc,
                            this_v,
                            &mut hit,
                            add(destn, scale(grav_dir, test_down)),
                            destn,
                            TRACE_VIS_BLOCKING,
                            &ZERO,
                        );
                        let max_radius = min_f(14.0, 0.5 * (*actor).collision_radius);
                        // if point trace hit nothing, or hit a slope, do a trace with extent
                        let ch = (*actor).collision_height;
                        let cr = (*actor).collision_radius;
                        let msh = (*this).max_step_height;
                        if hit.time == 1.0
                            || ((hit.normal.z as f64) > 0.7
                                && hit.time * test_down > ch + msh + 4.0
                                && ((hit.time * test_down) as f64)
                                    > ((ch + 4.0) as f64)
                                        + app_sqrt(
                                            // `1 - Z*Z` fuses into an FMA.
                                            (-hit.normal.z).mul_add(hit.normal.z, 1.0) as f64,
                                        ) * ((cr + desired_dist) as f64)
                                            / (hit.normal.z as f64))
                        {
                            line_check(
                                svc,
                                this_v,
                                &mut hit,
                                add(destn, scale(grav_dir, msh + 4.0)),
                                destn,
                                TRACE_VIS_BLOCKING,
                                &v3(max_radius, max_radius, ch),
                            );
                        }
                        if hit.time == 1.0 {
                            // We have a ledge!
                            destn = add(
                                add((*actor).location, scale(accel_dir, desired_dist)),
                                forward_check,
                            );
                            // first, try tracing back to get the ledge direction
                            let desired_dir = div(delta, desired_dist);
                            let mut ledge_down = scale(grav_dir, ch + 6.0);
                            line_check(
                                svc,
                                this_v,
                                &mut hit,
                                sub(
                                    add((*actor).location, ledge_down),
                                    scale(accel_dir, 2.0 * cr),
                                ),
                                add(destn, ledge_down),
                                TRACE_VIS_BLOCKING,
                                &ZERO,
                            );
                            ledge_down = scale(grav_dir, msh + 6.0);
                            let mut ledge_dir = ZERO;
                            let mut move_forward: bool;
                            let mut good_move = false;
                            if hit.time < 1.0 {
                                // found a ledge
                                if (*this).b_avoid_ledges() {
                                    ledge_dir = hit.normal;
                                    ledge_dir.z = 0.0;
                                    delta = scale(
                                        ledge_dir,
                                        -(*this).desired_speed * (*this).ground_speed * time_tick,
                                    );
                                    move_forward = false;
                                    if (*this).b_stop_at_ledges() {
                                        (*this).move_timer = -1.0;
                                    } else {
                                        (*this).move_timer -= 0.25;
                                    }
                                } else {
                                    ledge_dir.x = hit.normal.y;
                                    ledge_dir.y = -hit.normal.x;
                                    ledge_dir.z = 0.0;
                                    ledge_dir = safe_normal(ledge_dir);
                                    if dot(ledge_dir, accel_dir) < 0.0 {
                                        ledge_dir = scale(ledge_dir, -1.0);
                                    }
                                    let dp = dot(ledge_dir, accel_dir);
                                    move_forward = (dp as f64) < 0.5
                                        || ((*this).b_can_jump() && (dp as f64) < 0.7);
                                    if (dp as f64) < 0.7 {
                                        delta = scale(
                                            ledge_dir,
                                            min_f(0.8, (*this).desired_speed)
                                                * (*this).ground_speed
                                                * time_tick,
                                        );
                                    } else {
                                        delta = scale(
                                            ledge_dir,
                                            (*this).desired_speed
                                                * (*this).ground_speed
                                                * time_tick,
                                        );
                                    }
                                }
                            } else {
                                // `Destn = Location + Delta + ForwardCheck` is overwritten
                                // below before any use, so it is not computed here.
                                ledge_dir = v3(desired_dir.y, -desired_dir.x, 0.0);
                                move_forward = true;
                                delta = scale(
                                    ledge_dir,
                                    min_f(0.8, (*this).desired_speed)
                                        * (*this).ground_speed
                                        * time_tick,
                                );
                                destn = add((*actor).location, delta);
                                line_check(
                                    svc,
                                    this_v,
                                    &mut hit,
                                    destn,
                                    (*actor).location,
                                    TRACE_VIS_BLOCKING,
                                    &extent,
                                );
                                if hit.time == 1.0 {
                                    line_check(
                                        svc,
                                        this_v,
                                        &mut hit,
                                        add(destn, ledge_down),
                                        destn,
                                        TRACE_VIS_BLOCKING,
                                        &extent,
                                    );
                                    if hit.time == 1.0 {
                                        // reflect delta about desiredir
                                        delta = neg(delta);
                                    } else {
                                        good_move = true;
                                    }
                                } else {
                                    good_move = true;
                                }
                            }
                            if is_a(svc, this_v, CLASS_PLAYER_PAWN) {
                                move_forward = false;
                                if !good_move {
                                    destn = add(add((*actor).location, delta), forward_check);
                                    line_check(
                                        svc,
                                        this_v,
                                        &mut hit,
                                        destn,
                                        (*actor).location,
                                        TRACE_VIS_BLOCKING,
                                        &extent,
                                    );
                                    if hit.time == 1.0 {
                                        line_check(
                                            svc,
                                            this_v,
                                            &mut hit,
                                            add(destn, ledge_down),
                                            destn,
                                            TRACE_VIS_BLOCKING,
                                            &v3(max_radius, max_radius, ch),
                                        );
                                    }
                                    if hit.time == 1.0 {
                                        (*this).base.acceleration = ZERO;
                                        delta = ZERO;
                                    }
                                }
                            }
                            if (*this).b_can_jump() && move_forward {
                                if (svc.is_probing_may_fall)(this_v) == 0 {
                                    delta = scale(accel_dir, desired_dist);
                                } else if !checked_fall {
                                    checked_fall = true;
                                    move_forward = false;
                                    (svc.event_may_fall)(this_v);
                                    if (*this).b_can_jump() {
                                        must_jump = true;
                                        delta = scale(accel_dir, desired_dist);
                                    }
                                }
                            }
                            if !(*this).b_can_jump() {
                                // if can't jump, make sure this is valid
                                if move_forward {
                                    // check if should just move forward
                                    destn = add(
                                        (*actor).location,
                                        scale(desired_dir, desired_dist + cr),
                                    );
                                    line_check(
                                        svc,
                                        this_v,
                                        &mut hit,
                                        destn,
                                        (*actor).location,
                                        TRACE_VIS_BLOCKING,
                                        &extent,
                                    );
                                    if hit.time == 1.0 {
                                        line_check(
                                            svc,
                                            this_v,
                                            &mut hit,
                                            add(destn, ledge_down),
                                            destn,
                                            TRACE_VIS_BLOCKING,
                                            &extent,
                                        );
                                        if hit.time < 1.0 {
                                            destn = add(
                                                (*actor).location,
                                                scale(desired_dir, desired_dist),
                                            );
                                            line_check(
                                                svc,
                                                this_v,
                                                &mut hit,
                                                add(destn, ledge_down),
                                                destn,
                                                TRACE_VIS_BLOCKING,
                                                &extent,
                                            );
                                        }
                                    }
                                    if hit.time < 1.0 {
                                        delta = scale(desired_dir, desired_dist);
                                    } else {
                                        move_forward = false;
                                        if (svc.app_frand)() < 2.0 * time_tick {
                                            (*this).move_timer = -1.0;
                                        } else {
                                            (*this).move_timer -= 0.1;
                                        }
                                    }
                                }
                                if !move_forward && !good_move {
                                    destn = add(add((*actor).location, delta), forward_check);
                                    line_check(
                                        svc,
                                        this_v,
                                        &mut hit,
                                        destn,
                                        (*actor).location,
                                        TRACE_VIS_BLOCKING,
                                        &extent,
                                    );
                                    if hit.time == 1.0 {
                                        line_check(
                                            svc,
                                            this_v,
                                            &mut hit,
                                            add(destn, ledge_down),
                                            destn,
                                            TRACE_VIS_BLOCKING,
                                            &extent,
                                        );
                                    } else if dot(hit.normal, desired_dir) < (*this).min_hit_wall {
                                        (*this).move_timer = -1.0;
                                    }
                                    if hit.time == 1.0 {
                                        line_check(
                                            svc,
                                            this_v,
                                            &mut hit,
                                            add((*actor).location, ledge_down),
                                            (*actor).location,
                                            TRACE_VIS_BLOCKING,
                                            &v3(max_radius, max_radius, ch),
                                        );
                                        remaining_time = 0.0;
                                        (*this).move_timer = -1.0;
                                        (*this).base.acceleration = ZERO;
                                        if hit.time == 1.0 {
                                            delta = scale(
                                                desired_dir,
                                                -(*this).ground_speed * time_tick,
                                            );
                                        } else {
                                            delta = ZERO;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    sub_move = delta;
                }

                // check if might hit sloped wall, and decide if to change direction before move
                if (*this).b_hit_sloped_wall() {
                    let desired_dist = size(delta);
                    let desired_dir = div(delta, desired_dist);
                    let check_dir = scale(desired_dir, max_f(30.0, desired_dist + 4.0));
                    line_check(
                        svc,
                        this_v,
                        &mut hit,
                        add((*actor).location, check_dir),
                        (*actor).location,
                        TRACE_VIS_BLOCKING,
                        &extent,
                    );
                    let sloped = is_slope(&hit);
                    (*this).set_b_hit_sloped_wall(sloped);
                    if sloped {
                        hit.normal.z = 0.0;
                        hit.normal = safe_normal(hit.normal);
                        delta = sub(delta, scale(hit.normal, dot(delta, hit.normal)));
                    } else if is_a(svc, this_v, CLASS_PLAYER_PAWN) {
                        // make sure really done with sloped wall
                        let mut check_loc = (*actor).location;
                        check_loc.z =
                            check_loc.z - (*actor).collision_height + (*this).max_step_height + 4.0;
                        line_check(
                            svc,
                            this_v,
                            &mut hit,
                            add(check_loc, scale(desired_dir, 100.0)),
                            check_loc,
                            TRACE_VIS_BLOCKING,
                            &ZERO,
                        );
                        let mut hit_slope = is_slope(&hit);
                        if !hit_slope {
                            let left_dir = safe_normal(add(
                                v3(desired_dir.y, -desired_dir.x, 0.0),
                                desired_dir,
                            ));
                            line_check(
                                svc,
                                this_v,
                                &mut hit,
                                add(check_loc, scale(left_dir, 100.0)),
                                check_loc,
                                TRACE_VIS_BLOCKING,
                                &ZERO,
                            );
                            hit_slope = is_slope(&hit);
                        }
                        if !hit_slope {
                            let left_dir = safe_normal(add(
                                v3(-desired_dir.y, desired_dir.x, 0.0),
                                desired_dir,
                            ));
                            line_check(
                                svc,
                                this_v,
                                &mut hit,
                                add(check_loc, scale(left_dir, 100.0)),
                                check_loc,
                                TRACE_VIS_BLOCKING,
                                &ZERO,
                            );
                            hit_slope = is_slope(&hit);
                        }
                        (*this).set_b_hit_sloped_wall(hit_slope);
                    }
                    move_actor(this, svc, delta, &mut hit);
                } else {
                    move_actor(this, svc, delta, &mut hit);
                    (*this).set_b_hit_sloped_wall(is_slope(&hit));
                }

                if hit.time < 1.0 {
                    // try to step up
                    if hit.normal.z as f64 > 0.7 {
                        ground_normal = hit.normal;
                    }
                    let desired_dir = safe_normal(delta);
                    step_up(
                        this,
                        svc,
                        grav_dir,
                        desired_dir,
                        scale(delta, 1.0 - hit.time),
                        &mut hit,
                    );
                    if (*actor).physics == PHYS_FALLING {
                        // pawn decided to jump up
                        let desired_dist = size(sub_move);
                        let actual_dist = size_2d(sub((*actor).location, sub_loc));
                        remaining_time = time_tick
                            .mul_add(1.0 - min_f(1.0, actual_dist / desired_dist), remaining_time);
                        (svc.event_falling)(this_v);
                        if (*actor).physics == PHYS_FALLING {
                            if remaining_time > 0.01 {
                                fall::phys_falling(actor, svc, remaining_time, iterations);
                            }
                        } else if (*actor).physics == PHYS_FLYING {
                            (*actor).velocity = v3(0.0, 0.0, (*this).air_speed);
                            (*actor).acceleration = v3(0.0, 0.0, (*this).accel_rate);
                            if remaining_time > 0.01 {
                                swim::phys_flying(this, svc, remaining_time, iterations);
                            }
                        }
                        return;
                    } else if (*actor).physics == PHYS_PROJECTILE {
                        // pawn is mounting
                        return;
                    }
                }

                if is_a(svc, this_v, CLASS_PAWN) && (*actor).physics == PHYS_SWIMMING {
                    // just entered water
                    swim::start_swimming(
                        this,
                        svc,
                        (*actor).velocity,
                        time_tick,
                        remaining_time,
                        iterations,
                    );
                    return;
                }
            }

            let ppawn = if is_a(svc, this_v, CLASS_PLAYER_PAWN) {
                this_v as *mut APlayerPawn
            } else {
                std::ptr::null_mut()
            };
            if !ppawn.is_null() && (*ppawn).b_auto_jump() {
                // Check whether center has crossed a ledge.
                let ch = (*actor).collision_height;
                let foot_del = v3(0.0, 0.0, -ch);
                let sub_foot_del = v3(0.0, 0.0, -ch - 4.0);
                if !b_zero_move
                    && line_check(
                        svc,
                        this_v,
                        &mut hit,
                        add(add((*actor).location, foot_del), down),
                        (*actor).location,
                        TRACE_ALL_BLOCKING,
                        &ZERO,
                    ) != 0
                {
                    // Find exact location of ledge.
                    let back = sub(
                        (*actor).location,
                        scale(accel_dir, (*actor).collision_radius * 1.5),
                    );
                    if line_check(
                        svc,
                        this_v,
                        &mut hit,
                        add(back, sub_foot_del),
                        add((*actor).location, sub_foot_del),
                        TRACE_ALL_BLOCKING,
                        &ZERO,
                    ) == 0
                    {
                        let cos = dot(hit.normal, accel_dir);
                        if cos > MIN_AUTO_JUMP_COS {
                            // Crossed ledge this tick. See whether we'd really fall on ledge
                            // edge. Move player out to ledge location.
                            let t = hit.time;
                            let edge_loc = add(scale(back, t), scale((*actor).location, 1.0 - t));

                            // Find where we fall, and where we jump.
                            let start = add(
                                edge_loc,
                                scale(accel_dir, (*actor).collision_radius * 1.5 / cos),
                            );
                            predict(&mut hit, this, svc, start, (*actor).velocity, &extent, 2.0);
                            let fall_loc = hit.location;

                            predict(
                                &mut hit,
                                this,
                                svc,
                                edge_loc,
                                add((*actor).velocity, v3(0.0, 0.0, (*this).jump_z)),
                                &extent,
                                2.0,
                            );
                            let jump_loc = hit.location;

                            if jump_loc.z - fall_loc.z > AUTO_JUMP_THRESHOLD {
                                // Jumping is better. Move back to edge location, then jump.
                                move_actor(this, svc, sub(edge_loc, (*actor).location), &mut hit);
                                (svc.event_do_jump)(this_v, 1.0);
                                remaining_time = time_tick.mul_add(t, remaining_time);
                                fall::phys_falling(actor, svc, remaining_time, iterations);
                                return;
                            }
                        }
                    }
                }
            }

            //drop to floor
            if b_zero_move {
                let foot = sub((*actor).location, v3(0.0, 0.0, (*actor).collision_height));
                line_check(
                    svc,
                    this_v,
                    &mut hit,
                    sub(foot, v3(0.0, 0.0, 20.0)),
                    foot,
                    TRACE_VIS_BLOCKING,
                    &ZERO,
                );
                let floor_dist = hit.time * 20.0;
                b_zero_move = (*actor).base == hit.actor && (4.1..=4.6).contains(&floor_dist);
            }
            if !b_zero_move {
                line_check(
                    svc,
                    this_v,
                    &mut hit,
                    add((*actor).location, down),
                    (*actor).location,
                    TRACE_ALL_BLOCKING,
                    &extent,
                );
                let floor_dist = hit.time * ((*this).max_step_height + 2.0);

                if hit.time < 1.0 && hit.normal.z > 0.7 {
                    ground_normal = hit.normal;
                }
                if hit.time < 1.0 && (hit.actor != (*actor).base || floor_dist > 2.4) {
                    move_actor(this, svc, down, &mut hit);
                    if hit.actor != (*actor).base {
                        (svc.set_base)(this_v, hit.actor);
                    }
                    if is_a(svc, this_v, CLASS_PAWN) && (*actor).physics == PHYS_SWIMMING {
                        // just entered water
                        swim::start_swimming(
                            this,
                            svc,
                            (*actor).velocity,
                            time_tick,
                            remaining_time,
                            iterations,
                        );
                        return;
                    }
                } else if (floor_dist as f64) < 1.9 {
                    let real_norm = hit.normal;
                    move_actor(this, svc, v3(0.0, 0.0, 2.1 - floor_dist), &mut hit);
                    hit.time = 0.0;
                    hit.normal = real_norm;
                }

                if !must_jump && hit.time < 1.0 && hit.normal.z >= 0.7 {
                    // slide down slope, depending on friction and gravity
                    if hit.normal.z < 1.0
                        && (hit.normal.z * (*zone(this)).zone_ground_friction) < 3.3
                    {
                        let slide = scale(
                            div(
                                scale((*zone(this)).zone_gravity, delta_time),
                                2.0 * max_f(0.5, (*zone(this)).zone_ground_friction),
                            ),
                            delta_time,
                        );
                        delta = sub(slide, scale(hit.normal, dot(slide, hit.normal)));
                        if dot(delta, slide) >= 0.0 {
                            move_actor(this, svc, delta, &mut hit);
                        }
                        if is_a(svc, this_v, CLASS_PAWN) && (*actor).physics == PHYS_SWIMMING {
                            // just entered water
                            swim::start_swimming(
                                this,
                                svc,
                                (*actor).velocity,
                                time_tick,
                                remaining_time,
                                iterations,
                            );
                            return;
                        }
                    }
                } else {
                    if !must_jump
                        && (*this).b_can_jump()
                        && !checked_fall
                        && (svc.is_probing_may_fall)(this_v) != 0
                    {
                        checked_fall = true;
                        (svc.event_may_fall)(this_v);
                    }
                    if !must_jump && (!(*this).b_can_jump() || (*this).b_is_walking()) {
                        (*actor).velocity = ZERO;
                        (*actor).acceleration = ZERO;
                        let old_loc = (*actor).old_location;
                        (svc.far_move_actor)(this_v, &old_loc);
                        (*this).move_timer = -1.0;
                        return;
                    } else {
                        // falling
                        if hit.time < 1.0 {
                            (*this).set_b_hit_sloped_wall(true);
                        }
                        let desired_dist = size(sub_move);
                        let actual_dist = size_2d(sub((*actor).location, sub_loc));
                        if desired_dist == 0.0 {
                            remaining_time = 0.0;
                        } else {
                            remaining_time = time_tick.mul_add(
                                1.0 - min_f(1.0, actual_dist / desired_dist),
                                remaining_time,
                            );
                        }
                        (*actor).velocity.z = 0.0;
                        (svc.event_falling)(this_v);
                        if (*actor).physics == PHYS_WALKING {
                            // default if script didn't change physics
                            dispatch::set_physics(actor, svc, PHYS_FALLING, std::ptr::null_mut());
                        }
                        if !must_jump && (*actor).physics == PHYS_FALLING {
                            let vel_z = (*actor).velocity.z;
                            if !(*actor).b_just_teleported() && delta_time > remaining_time {
                                (*actor).velocity = div(
                                    sub((*actor).location, (*actor).old_location),
                                    delta_time - remaining_time,
                                );
                            }
                            (*actor).velocity.z = vel_z;
                            if remaining_time > 0.01 {
                                fall::phys_falling(actor, svc, remaining_time, iterations);
                            }
                            return;
                        } else {
                            delta = scale(desired_move, remaining_time);
                            move_actor(this, svc, delta, &mut hit);
                            remaining_time = 0.0;
                        }
                    }
                }
            }
        }

        // make velocity reflect actual move
        if !(*actor).b_just_teleported() {
            (*actor).velocity = div(sub((*actor).location, (*actor).old_location), delta_time);

            // Conform velocity to ground.
            let vel = size((*actor).velocity);
            if vel > 0.0 && ground_normal.z > 0.0 {
                (*actor).velocity.z -= dot(ground_normal, (*actor).velocity) / ground_normal.z;
                let vel2 = size((*actor).velocity);
                if vel2 > 0.0 {
                    (*actor).velocity = scale((*actor).velocity, vel / vel2);
                }
            }
        }
    }
}
