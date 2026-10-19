//! Port of `AActor::physFalling` (`UnPhysic.cpp`): falling bodies with air
//! control, water entry, bounces, apex adjustment and terminal velocity.
//! Ported line by line; the fused-multiply-add sites are the ones the `-O3`
//! disassembly of the original object shows (see `tick_velocity`).

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, ADecoration, APawn, AZoneInfo};

use super::swim::start_swimming;
use super::{
    CLASS_DECORATION, CLASS_PAWN, CLASS_PLAYER_PAWN, Hp2PhysicServices, PHYS_NONE,
    TRACE_VIS_BLOCKING, ZERO, abs_f, add, app_sqrt, blank_hit, div, dot, is_a, max_f, max_i, mount,
    process_hit_wall, process_landed, safe_normal, scale, size_sq, sub, two_wall_adjust, v3,
};

/// `AInventory` class id for `Hp2PhysicServices::is_a` (`HP2_PHYSIC_CLASS_INVENTORY`).
const CLASS_INVENTORY: i32 = 6;
/// `AMover` class id for `Hp2PhysicServices::is_a`.
const CLASS_MOVER: i32 = 5;
/// `ROLE_Authority` (`EngineClasses.h`).
const ROLE_AUTHORITY: u8 = 4;
/// `PHYS_Swimming` (`EngineClasses.h`).
const PHYS_SWIMMING: u8 = 3;

/// `Min(A, B)` from `UnTemplate.h` for `FLOAT`.
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `FVector::Size2D()`: `appSqrt(X*X + Y*Y)`, with the same fusion as `size_sq`.
fn size2d(a: FVector) -> f32 {
    app_sqrt(a.x.mul_add(a.x, a.y * a.y) as f64) as f32
}

/// `Region.Zone`, read fresh on each access (`MoveActor` may change it).
///
/// # Safety
/// `this` must point to a live `AActor`.
unsafe fn region_zone(this: *const AActor) -> *const AZoneInfo {
    unsafe { (*this).region.zone as *const AZoneInfo }
}

/// `((APawn*)this)->FootRegion.Zone`. The caller has checked `IsA(APlayerPawn)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn foot_zone(pawn: *const APawn) -> *const AZoneInfo {
    unsafe { (*pawn).foot_region.zone as *const AZoneInfo }
}

/// `GetLevel()->MoveActor(this, Delta, Rotation, Hit)`.
///
/// # Safety
/// `this` must point to a live `AActor`; `hit` must point to a live `FCheckResult`.
unsafe fn move_this(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta: FVector,
    hit: *mut hp2_abi::engine::generated::FCheckResult,
) {
    unsafe {
        (svc.move_actor)(this as *mut c_void, &delta, &(*this).rotation, hit);
    }
}

/// One velocity update over `timeTick` from `OldVelocity`. Shared by the main
/// step and the apex-adjusted step; both C++ sites are the same expression.
///
/// FMA sites (`-O3` on arm64): `1 - F*t` becomes `fmsub`, `1 - 2F*t` becomes
/// `fmadd` in the water branches. The bobbing and plain branches do not fuse.
///
/// # Safety
/// `this` must point to a live `AActor` with a live `Region.Zone`. A pawn
/// `IsA(APlayerPawn)` must have a live `FootRegion.Zone` (the C++ dereferences it).
unsafe fn tick_velocity(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    old_velocity: FVector,
    time_tick: f32,
) -> FVector {
    unsafe {
        let this_v = this as *mut c_void;
        let zone = region_zone(this);
        let acc = (*this).acceleration;
        if !(*zone).b_water_zone() {
            if is_a(svc, this_v, CLASS_DECORATION) && (*(this as *mut ADecoration)).b_bobbing() {
                // OldVelocity + 0.5 * (Acceleration + 0.5 * ZoneGravity) * timeTick
                let inner = add(acc, scale((*zone).zone_gravity, 0.5));
                add(old_velocity, scale(scale(inner, 0.5), time_tick))
            } else if is_a(svc, this_v, CLASS_PLAYER_PAWN)
                && (*foot_zone(this as *const APawn)).b_water_zone()
                && old_velocity.z < 0.0
            {
                // OldVelocity * (1 - F*t) + 0.5 * (Acceleration + ZoneGravity) * timeTick
                let f = (*foot_zone(this as *const APawn)).zone_fluid_friction;
                let k = (-f).mul_add(time_tick, 1.0);
                let inner = add(acc, (*zone).zone_gravity);
                add(scale(old_velocity, k), scale(scale(inner, 0.5), time_tick))
            } else {
                // OldVelocity + 0.5 * (Acceleration + ZoneGravity) * timeTick
                let inner = add(acc, (*zone).zone_gravity);
                add(old_velocity, scale(scale(inner, 0.5), time_tick))
            }
        } else {
            // OldVelocity * (1 - 2*F*t) + 0.5 * (Acceleration + ZoneGravity*(1 - Buoyancy/Max(1, Mass))) * timeTick
            let f = (*zone).zone_fluid_friction;
            let k = (-(2.0 * f)).mul_add(time_tick, 1.0);
            let buoy_ratio = (*this).buoyancy / max_f(1.0, (*this).mass);
            let g = scale((*zone).zone_gravity, 1.0 - buoy_ratio);
            add(
                scale(old_velocity, k),
                scale(scale(add(acc, g), 0.5), time_tick),
            )
        }
    }
}

/// `AActor::physFalling(deltaTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `AActor`. Its `Region.Zone` must be live
/// whenever it is read (the C++ dereferences it unconditionally).
pub unsafe fn phys_falling(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    iterations: i32,
) {
    unsafe {
        let this_v = this as *mut c_void;
        // APawn *ThisPawn = IsA(APawn) ? (APawn*)this : NULL;
        let this_pawn: *mut APawn = if is_a(svc, this_v, CLASS_PAWN) {
            this as *mut APawn
        } else {
            null_mut()
        };

        // Outside every zone: fell out of the world.
        if (*this).region.zone_number == 0 {
            if (*this).role == ROLE_AUTHORITY
                && (is_a(svc, this_v, CLASS_INVENTORY)
                    || is_a(svc, this_v, CLASS_DECORATION)
                    || is_a(svc, this_v, CLASS_PAWN))
            {
                (svc.log_fell_out_of_world)(this_v);
            }
            (svc.event_fell_out_of_world)(this_v);
            return;
        }

        let real_acceleration = (*this).acceleration;
        let mut bound_speed: f32 = 0.0;

        // Air control: limit the acceleration while airborne.
        if !this_pawn.is_null() {
            let pawn = this_pawn;
            let mut air_control = (*pawn).air_control;
            if air_control > 0.15 {
                // TestWalk = (AirControl * AccelRate * Acceleration.SafeNormal() + Velocity) * deltaTime
                let mut test_walk = scale(
                    add(
                        scale(
                            safe_normal((*this).acceleration),
                            air_control * (*pawn).accel_rate,
                        ),
                        (*this).velocity,
                    ),
                    delta_time,
                );
                test_walk.z = 0.0;
                let mut hit = blank_hit(1.0);
                let start = (*this).location;
                let end = add(start, test_walk);
                let extent = v3(
                    (*this).collision_radius,
                    (*this).collision_radius,
                    (*this).collision_height,
                );
                (svc.single_line_check)(
                    this_v,
                    &mut hit,
                    &end,
                    &start,
                    TRACE_VIS_BLOCKING,
                    &extent,
                );
                if !hit.actor.is_null() {
                    air_control = 0.05;
                }
            }
            let mut max_accel = (*pawn).accel_rate * air_control;
            let mut vel2d = (*this).velocity;
            vel2d.z = 0.0;
            (*this).acceleration.z = 0.0;
            let speed2d = size2d(vel2d);
            if speed2d < 10.0 {
                max_accel += (10.0 - speed2d) / delta_time;
            } else if speed2d >= (*pawn).ground_speed {
                if air_control <= 0.05 {
                    max_accel = 1.0;
                } else {
                    bound_speed = speed2d;
                }
            }
            if size_sq((*this).acceleration) > max_accel * max_accel {
                let n = safe_normal((*this).acceleration);
                (*this).acceleration = scale(n, max_accel);
            }
        }

        let mut remaining_time = delta_time;
        let mut time_tick: f32;
        let mut num_bounces: i32 = 0;
        let mut hit = blank_hit(1.0);
        let mut adjust_apex = false;
        let mut iterations = iterations;

        while remaining_time > 0.0 && iterations < 8 {
            iterations += 1;
            time_tick = if remaining_time > 0.1 {
                min_f(0.1, remaining_time * 0.5)
            } else {
                remaining_time
            };
            remaining_time -= time_tick;
            (*this).old_location = (*this).location;
            (*this).set_b_just_teleported(false);

            let mut old_velocity = (*this).velocity;
            (*this).velocity = tick_velocity(this, svc, old_velocity, time_tick);

            // Apex: vertical velocity crossed zero; split the step at the apex.
            let apex = !adjust_apex
                && ((old_velocity.z > 0.0) != ((*this).velocity.z > 0.0))
                && abs_f(old_velocity.z) > 5.0
                && abs_f((*this).velocity.z) > 5.0;
            if apex {
                adjust_apex = true;
                let part =
                    abs_f(old_velocity.z) / (abs_f(old_velocity.z) + abs_f((*this).velocity.z));
                // Fused in the original apex-split remainder update.
                if part * time_tick > 0.015 && (1.0 - part) * time_tick > 0.015 {
                    remaining_time = time_tick.mul_add(1.0 - part, remaining_time);
                    time_tick *= part;
                    (*this).velocity = tick_velocity(this, svc, old_velocity, time_tick);
                }
            } else {
                adjust_apex = false;
            }

            // Keep the horizontal speed at the bound set by air control.
            if bound_speed != 0.0 {
                let mut vel2d = (*this).velocity;
                vel2d.z = 0.0;
                if size_sq(vel2d) > bound_speed * bound_speed {
                    vel2d = safe_normal(vel2d);
                    vel2d = scale(vel2d, bound_speed);
                    vel2d.z = (*this).velocity.z;
                    (*this).velocity = vel2d;
                }
            }

            let mut zone_vel = ZERO;
            if !(*this).b_is_pawn()
                || is_a(svc, this_v, CLASS_PLAYER_PAWN)
                || size_sq((*region_zone(this)).zone_velocity) > 40000.0
            {
                zone_vel = (*region_zone(this)).zone_velocity;
            }
            let adjusted = scale(add((*this).velocity, zone_vel), time_tick);

            move_this(this, svc, adjusted, &mut hit);
            if (*this).b_delete_me() {
                return;
            } else if !this_pawn.is_null() && (*this).physics == PHYS_SWIMMING {
                remaining_time = time_tick.mul_add(1.0 - hit.time, remaining_time);
                start_swimming(
                    this_pawn,
                    svc,
                    old_velocity,
                    time_tick,
                    remaining_time,
                    iterations,
                );
                return;
            } else if hit.time < 1.0 {
                if is_a(svc, hit.actor, CLASS_PLAYER_PAWN) && is_a(svc, this_v, CLASS_DECORATION) {
                    // Hit.Actor is dereferenced unchecked, as in the C++ (undefined if null).
                    let dec = this as *mut ADecoration;
                    (*dec).num_landings = max_i(0, (*dec).num_landings - 1);
                }
                if (*this).b_bounce() && !is_a(svc, hit.actor, CLASS_MOVER) {
                    (svc.event_hit_wall)(this_v, &hit.normal, hit.actor);
                    if (*this).physics == PHYS_NONE {
                        return;
                    } else if num_bounces < 2 {
                        remaining_time = time_tick.mul_add(1.0 - hit.time, remaining_time);
                    }
                    num_bounces += 1;
                } else if hit.normal.z as f64 > 0.7 {
                    remaining_time = time_tick.mul_add(1.0 - hit.time, remaining_time);
                    if !(*this).b_just_teleported()
                        && hit.time > 0.1
                        && hit.time * time_tick > 0.003
                    {
                        (*this).velocity = div(
                            sub((*this).location, (*this).old_location),
                            time_tick * hit.time,
                        );
                    }
                    process_landed(this, svc, hit.normal, hit.actor, remaining_time, iterations);
                    return;
                } else {
                    if !this_pawn.is_null() {
                        mount(this_pawn, svc, v3(0.0, 0.0, 1.0), &mut hit);
                    }
                    process_hit_wall(this, svc, hit.normal, hit.actor);
                    let old_hit_normal = hit.normal;
                    let mut delta = scale(
                        sub(adjusted, scale(hit.normal, dot(adjusted, hit.normal))),
                        1.0 - hit.time,
                    );
                    if dot(delta, adjusted) >= 0.0 {
                        move_this(this, svc, delta, &mut hit);
                        if hit.time < 1.0 {
                            if hit.normal.z > 0.7 {
                                remaining_time = 0.0;
                                process_landed(
                                    this,
                                    svc,
                                    hit.normal,
                                    hit.actor,
                                    remaining_time,
                                    iterations,
                                );
                                return;
                            } else {
                                process_hit_wall(this, svc, hit.normal, hit.actor);
                            }
                            let desired_dir = safe_normal(adjusted);
                            two_wall_adjust(
                                desired_dir,
                                &mut delta,
                                hit.normal,
                                old_hit_normal,
                                hit.time,
                            );
                            let b_ditch = old_hit_normal.z > 0.0
                                && hit.normal.z > 0.0
                                && delta.z == 0.0
                                && dot(hit.normal, old_hit_normal) < 0.0;
                            move_this(this, svc, delta, &mut hit);
                            if b_ditch || hit.normal.z as f64 > 0.7 {
                                remaining_time = 0.0;
                                process_landed(
                                    this,
                                    svc,
                                    hit.normal,
                                    hit.actor,
                                    remaining_time,
                                    iterations,
                                );
                                return;
                            }
                        }
                    }
                    let old_z = old_velocity.z;
                    old_velocity = div(sub((*this).location, (*this).old_location), time_tick);
                    old_velocity.z = old_z;
                }
            }

            // Velocity refinement from the realised move, minus the zone current.
            if !(*this).b_bounce() && !(*this).b_just_teleported() {
                (*this).velocity = sub(
                    div(sub((*this).location, (*this).old_location), time_tick),
                    zone_vel,
                );
                if (*this).velocity.z < old_velocity.z || old_velocity.z >= 0.0 {
                    (*this).velocity = sub(scale((*this).velocity, 2.0), old_velocity);
                }
                let zt = (*region_zone(this)).zone_terminal_velocity;
                if size_sq((*this).velocity) > zt * zt {
                    let v = safe_normal((*this).velocity);
                    (*this).velocity = scale(v, zt);
                }
            }
        }

        (*this).acceleration = real_acceleration;
    }
}
