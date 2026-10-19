//! Port of AActor::physProjectile, physRolling; APawn::physSpider, checkFloor, findNewFloor (UnPhysic.cpp).

use super::{
    Hp2PhysicServices, PHYS_FALLING, TRACE_VIS_BLOCKING, ZERO, add, div, dot, is_zero, neg,
    safe_normal, scale, size, size_sq, sub,
};
use hp2_abi::{
    FVector,
    engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, FRotator},
};
use std::ffi::c_void;

const CLASS_PAWN: i32 = 0;
const PHYS_ROLLING: u8 = 7;
const PHYS_SPIDER: u8 = 10;
const CLASS_PROJECTILE: i32 = 4;
const CLASS_MOVER: i32 = 5;

// APawn::Floor is the first FVector in the ABI's opaque _pad_1184 block.
const PAWN_FLOOR_OFFSET: usize = 1184;
unsafe fn floor_ptr(pawn: *mut APawn) -> *mut FVector {
    unsafe { pawn.cast::<u8>().add(PAWN_FLOOR_OFFSET).cast() }
}

unsafe fn zone(actor: *mut AActor) -> *mut AZoneInfo {
    unsafe { (*actor).region.zone.cast() }
}

/// AActor::physProjectile.
///
/// # Safety
/// `this`, its zone/level links, and `svc` callbacks must be live.
pub unsafe fn phys_projectile(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    mut iterations: i32,
) {
    unsafe {
        let actor = this.cast::<c_void>();
        let mut remaining = delta_time;
        let mut bounces = 0;
        if (*this).region.zone_number == 0 {
            (svc.destroy_actor)(actor);
            return;
        }
        (*this).old_location = (*this).location;
        (*this).set_b_just_teleported(false);
        let mut hit = super::blank_hit(1.0);
        while remaining > 0.0 && iterations < 8 {
            iterations += 1;
            let z = zone(this);
            // `-ffp-contract=on`: `1 - 0.2f*F*T` fuses the final product into an FMA.
            if (*z).b_water_zone() {
                (*this).velocity = scale(
                    (*this).velocity,
                    (-(0.2 * (*z).zone_fluid_friction)).mul_add(remaining, 1.0),
                );
            }
            (*this).velocity = add((*this).velocity, scale((*this).acceleration, remaining));
            let time_tick = remaining;
            remaining = 0.0;
            if (svc.is_a)(actor, CLASS_PROJECTILE) != 0 {
                let max_speed = (svc.projectile_max_speed)(actor);
                if size_sq((*this).velocity) > max_speed * max_speed {
                    (*this).velocity = scale(safe_normal((*this).velocity), max_speed);
                }
            }
            let adjusted = scale((*this).velocity, delta_time);
            hit.time = 1.0;
            (svc.move_actor)(actor, &adjusted, &(*this).rotation, &mut hit);
            if hit.time < 1.0 && !(*this).b_delete_me() && !(*this).b_just_teleported() {
                (svc.event_hit_wall)(actor, &hit.normal, hit.actor);
                if (*this).b_bounce() && (svc.is_a)(hit.actor, CLASS_MOVER) == 0 {
                    if bounces < 2 {
                        remaining = time_tick * (1.0 - hit.time);
                    }
                    bounces += 1;
                    if (*this).physics == 2 {
                        super::fall::phys_falling(this, svc, remaining, iterations);
                    }
                }
            }
        }
        if !(*this).b_bounce() && !(*this).b_just_teleported() {
            (*this).velocity = div(sub((*this).location, (*this).old_location), delta_time);
        }
    }
}

/// AActor::physRolling.
///
/// # Safety
/// `this`, its zone/level links, and `svc` callbacks must be live.
pub unsafe fn phys_rolling(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    mut iterations: i32,
) {
    unsafe {
        let actor = this.cast::<c_void>();
        let z = zone(this);
        let vel_dir = safe_normal((*this).velocity);
        let accel_dir = safe_normal((*this).acceleration);
        (*this).velocity = sub(
            (*this).velocity,
            scale(
                scale(
                    scale(sub(vel_dir, accel_dir), size((*this).velocity)),
                    delta_time,
                ),
                (*z).zone_ground_friction,
            ),
        );
        (*this).velocity = add(
            scale(
                (*this).velocity,
                (-(*z).zone_fluid_friction).mul_add(delta_time, 1.0),
            ),
            scale((*this).acceleration, delta_time),
        );
        let desired_move = add((*this).velocity, (*z).zone_velocity);
        (*this).old_location = (*this).location;
        (*this).set_b_just_teleported(false);
        let mut remaining = delta_time;
        let mut time_tick;
        let grav_dir = if (*z).zone_gravity.z > 0.0 {
            v3(0.0, 0.0, 1.0)
        } else {
            v3(0.0, 0.0, -1.0)
        };
        let down = scale(grav_dir, 16.0);
        let mut hit = super::blank_hit(1.0);
        let mut bounces = 0;
        while remaining > 0.0 && iterations < 8 {
            iterations += 1;
            time_tick = if remaining > 0.1 {
                min_f(0.1, remaining * 0.5)
            } else {
                remaining
            };
            remaining -= time_tick;
            let mut delta = scale(desired_move, time_tick);
            let sub_move = delta;
            let sub_loc = (*this).location;
            if !near_zero(delta) {
                (svc.move_actor)(actor, &delta, &(*this).rotation, &mut hit);
                if hit.time < 1.0 {
                    (svc.event_hit_wall)(actor, &hit.normal, hit.actor);
                    if (*this).b_bounce() && (svc.is_a)(hit.actor, CLASS_MOVER) == 0 {
                        if bounces < 2 {
                            remaining = time_tick.mul_add(1.0 - hit.time, remaining);
                        }
                        bounces += 1;
                    } else {
                        let original = delta;
                        let old_normal = hit.normal;
                        delta = scale(
                            sub(delta, scale(hit.normal, dot(delta, hit.normal))),
                            1.0 - hit.time,
                        );
                        if dot(delta, original) >= 0.0 {
                            (svc.move_actor)(actor, &delta, &(*this).rotation, &mut hit);
                            if hit.time < 1.0 {
                                (svc.event_hit_wall)(actor, &hit.normal, hit.actor);
                                let desired_dir = safe_normal(desired_move);
                                super::two_wall_adjust(
                                    desired_dir,
                                    &mut delta,
                                    hit.normal,
                                    old_normal,
                                    hit.time,
                                );
                                (svc.move_actor)(actor, &delta, &(*this).rotation, &mut hit);
                            }
                        }
                    }
                }
            }
            (svc.move_actor)(actor, &down, &(*this).rotation, &mut hit);
            let drop_time = hit.time;
            let mut drop_hit_z = hit.normal.z;
            if drop_time < 1.0
                && hit.normal.z < 1.0
                && hit.normal.z * (*z).zone_ground_friction < 3.3
            {
                let slide = scale(
                    scale(
                        scale((*z).zone_gravity, delta_time),
                        1.0 / (2.0 * super::max_f(0.5, (*z).zone_ground_friction)),
                    ),
                    delta_time,
                );
                delta = sub(slide, scale(hit.normal, dot(slide, hit.normal)));
                if dot(delta, slide) >= 0.0 {
                    (svc.move_actor)(actor, &delta, &(*this).rotation, &mut hit);
                    drop_hit_z = super::max_f(drop_hit_z, hit.normal.z);
                }
            }
            if drop_time == 1.0 || drop_hit_z < 0.7 {
                let adjust = scale(neg(down), drop_time);
                (svc.move_actor)(actor, &adjust, &(*this).rotation, &mut hit);
                let desired_dist = size(sub_move);
                let actual_dist = size2d(sub((*this).location, sub_loc));
                remaining =
                    time_tick.mul_add(1.0 - min_f(1.0, actual_dist / desired_dist), remaining);
                (svc.event_falling)(actor);
                if (*this).physics == PHYS_ROLLING {
                    super::dispatch::set_physics(this, svc, PHYS_FALLING, std::ptr::null_mut());
                }
                if (*this).physics == PHYS_FALLING {
                    if !(*this).b_just_teleported() && delta_time > remaining {
                        (*this).velocity = div(
                            sub((*this).location, (*this).old_location),
                            delta_time - remaining,
                        );
                    }
                    (*this).velocity.z = 0.0;
                    if remaining > 0.005 {
                        super::fall::phys_falling(this, svc, remaining, iterations);
                    }
                    return;
                }
                delta = scale(desired_move, remaining);
                (svc.move_actor)(actor, &delta, &(*this).rotation, &mut hit);
            } else if hit.actor != (*this).base {
                (svc.set_base)(actor, hit.actor);
            }
        }
        if !(*this).b_just_teleported() {
            (*this).velocity = div(sub((*this).location, (*this).old_location), delta_time);
        }
    }
}

/// APawn::checkFloor.
///
/// # Safety
/// The pawn, its zone/level links, and `svc` callbacks must be live.
pub unsafe fn check_floor(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    dir: FVector,
    hit: *mut FCheckResult,
) -> i32 {
    unsafe {
        let end = sub((*this).base.location, scale(dir, (*this).max_step_height));
        let mut extent = ZERO;
        (svc.cylinder_extent)(this.cast(), &mut extent);
        (svc.single_line_check_no_source)(
            this.cast(),
            hit,
            &end,
            &(*this).base.location,
            TRACE_VIS_BLOCKING,
            &extent,
        );
        if (*hit).time < 1.0 {
            (*floor_ptr(this)) = (*hit).normal;
            1
        } else {
            0
        }
    }
}

/// APawn::findNewFloor.
///
/// # Safety
/// The pawn, its zone/level links, and `svc` callbacks must be live.
pub unsafe fn find_new_floor(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    old_location: FVector,
    delta_time: f32,
    remaining_time: f32,
    iterations: i32,
) -> i32 {
    unsafe {
        let mut hit = super::blank_hit(1.0);
        let dirs = [
            v3(0.0, 0.0, 1.0),
            v3(0.0, 1.0, 0.0),
            v3(0.0, -1.0, 0.0),
            v3(1.0, 0.0, 0.0),
            v3(-1.0, 0.0, 0.0),
        ];
        for dir in dirs {
            if check_floor(this, svc, dir, &mut hit) != 0 {
                return 1;
            }
        }
        let actor = this.cast::<AActor>();
        (svc.event_falling)(actor.cast());
        if (*actor).physics == PHYS_SPIDER {
            super::dispatch::set_physics(actor, svc, PHYS_FALLING, std::ptr::null_mut());
        }
        if (*actor).physics == PHYS_FALLING {
            let vel_z = (*actor).velocity.z;
            if !(*actor).b_just_teleported() && delta_time > remaining_time {
                (*actor).velocity = div(
                    sub((*actor).location, old_location),
                    delta_time - remaining_time,
                );
            }
            (*actor).velocity.z = vel_z;
            if remaining_time > 0.005 {
                super::fall::phys_falling(actor, svc, remaining_time, iterations);
            }
        }
        0
    }
}

/// APawn::physSpider.
///
/// # Safety
/// The pawn, its zone/level links, and `svc` callbacks must be live.
pub unsafe fn phys_spider(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    mut iterations: i32,
) {
    unsafe {
        let actor = this.cast::<AActor>();
        let cv = actor.cast::<c_void>();
        let accel_dir;
        if is_zero((*actor).acceleration) {
            accel_dir = (*actor).acceleration;
            let old_vel = (*actor).velocity;
            (*actor).velocity = sub(
                (*actor).velocity,
                scale(
                    scale(scale((*actor).velocity, 2.0), delta_time),
                    (*zone(actor)).zone_ground_friction,
                ),
            );
            if dot(old_vel, (*actor).velocity) < 0.0 {
                (*actor).velocity = (*actor).acceleration;
            }
        } else {
            accel_dir = safe_normal((*actor).acceleration);
            let vel_size = size((*actor).velocity);
            if size_sq((*actor).acceleration) > (*this).accel_rate * (*this).accel_rate {
                (*actor).acceleration = scale(accel_dir, (*this).accel_rate);
            }
            let velocity_difference = sub((*actor).velocity, scale(accel_dir, vel_size));
            let friction_adjust = scale(
                scale(velocity_difference, delta_time),
                (*zone(actor)).zone_ground_friction,
            );
            (*actor).velocity = sub((*actor).velocity, friction_adjust);
        }
        (*actor).velocity = add((*actor).velocity, scale((*actor).acceleration, delta_time));
        let max_speed = (*this).ground_speed * (*this).desired_speed;
        iterations += 1;
        if size_sq((*actor).velocity) > max_speed * max_speed {
            (*actor).velocity = scale(safe_normal((*actor).velocity), max_speed);
        }
        let zone_vel = if size_sq((*zone(actor)).zone_velocity) > 90000.0 {
            (*zone(actor)).zone_velocity
        } else {
            ZERO
        };
        let desired_move = add((*actor).velocity, zone_vel);
        let move_size = size(desired_move);
        let desired_dir = div(desired_move, move_size);
        let mut floor = *floor_ptr(this);
        let mut find_new = near_zero(floor);
        let mut hit = super::blank_hit(1.0);
        let mut grav_dir = neg(floor);
        let mut down = scale(grav_dir, (*this).max_step_height + 4.0);
        (*actor).desired_rotation = (*actor).rotation;
        if !find_new {
            let end = add((*actor).location, down);
            let mut extent = ZERO;
            (svc.cylinder_extent)(cv, &mut extent);
            (svc.single_line_check_no_source)(
                cv,
                &mut hit,
                &end,
                &(*actor).location,
                TRACE_VIS_BLOCKING,
                &extent,
            );
            find_new = hit.time == 1.0;
        }
        if find_new {
            if find_new_floor(
                this,
                svc,
                (*actor).location,
                delta_time,
                delta_time,
                iterations,
            ) == 0
            {
                return;
            }
            floor = *floor_ptr(this);
            grav_dir = neg(floor);
            down = scale(grav_dir, (*this).max_step_height + 4.0);
        }
        let mut rotation = FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        };
        (svc.rotation_from_vector)(&floor, &mut rotation);
        rotation.pitch = rotation.pitch.wrapping_sub(16384);
        rotation.roll = 0;
        if floor == v3(0.0, 0.0, 1.0) {
            rotation.yaw = (*actor).rotation.yaw;
        }
        (*actor).desired_rotation = rotation;
        let dotp = dot(accel_dir, floor);
        let real_dir = desired_dir;
        let mut desired_dir = desired_dir;
        if floor.z < 0.6 && dotp > 0.9 {
            *floor_ptr(this) = ZERO;
            (svc.event_falling)(cv);
            super::dispatch::set_physics(actor, svc, PHYS_FALLING, std::ptr::null_mut());
            super::fall::phys_falling(actor, svc, delta_time, iterations);
            return;
        } else {
            desired_dir = safe_normal(sub(desired_dir, scale(floor, dot(desired_dir, floor))));
        }
        (*actor).old_location = (*actor).location;
        (*actor).set_b_just_teleported(false);
        let mut remaining = delta_time;
        let mut time_tick;
        let desired_move = scale(desired_dir, move_size);
        while remaining > 0.0 && iterations < 8 {
            iterations += 1;
            time_tick = if remaining > 0.05 {
                min_f(0.05, remaining * 0.5)
            } else {
                remaining
            };
            remaining -= time_tick;
            let mut delta = scale(desired_move, time_tick);
            let sub_loc = (*actor).location;
            let sub_move = delta;
            if !near_zero(delta) {
                (svc.move_actor)(cv, &delta, &(*actor).desired_rotation, &mut hit);
                if hit.time < 1.0 {
                    if hit.normal.z >= 0.0 {
                        if dot(hit.normal, real_dir) < 0.0 && dot(floor, real_dir) < 0.0 {
                            (svc.event_hit_wall)(cv, &hit.normal, hit.actor);
                        } else {
                            let combo = safe_normal(add(hit.normal, floor));
                            if dot(real_dir, combo) > 0.9 {
                                (svc.event_hit_wall)(cv, &hit.normal, hit.actor);
                            }
                        }
                        floor = hit.normal;
                        *floor_ptr(this) = floor;
                        grav_dir = neg(floor);
                        down = scale(grav_dir, (*this).max_step_height + 4.0);
                    } else if dot(hit.normal, real_dir) < 0.0 {
                        (svc.event_hit_wall)(cv, &hit.normal, hit.actor);
                    } else if dot(floor, real_dir) > 0.7 {
                        (svc.event_falling)(cv);
                        if (*actor).physics == PHYS_SPIDER {
                            super::dispatch::set_physics(
                                actor,
                                svc,
                                PHYS_FALLING,
                                std::ptr::null_mut(),
                            );
                        }
                        if (*actor).physics == PHYS_FALLING {
                            let vel_z = (*actor).velocity.z;
                            if !(*actor).b_just_teleported() && delta_time > remaining {
                                (*actor).velocity = div(
                                    sub((*actor).location, (*actor).old_location),
                                    delta_time - remaining,
                                );
                            }
                            (*actor).velocity.z = vel_z;
                            if remaining > 0.005 {
                                super::fall::phys_falling(actor, svc, remaining, iterations);
                            }
                            return;
                        }
                    }
                    let desired_dir2 = safe_normal(delta);
                    delta = scale(delta, 1.0 - hit.time);
                    super::step_up(this, svc, grav_dir, desired_dir2, delta, &mut hit);
                    if (*actor).physics == PHYS_FALLING {
                        if remaining > 0.005 {
                            super::fall::phys_falling(actor, svc, remaining, iterations);
                        }
                        return;
                    }
                }
            }
            (svc.move_actor)(cv, &down, &(*actor).rotation, &mut hit);
            if hit.time == 1.0 {
                if find_new_floor(
                    this,
                    svc,
                    (*actor).old_location,
                    delta_time,
                    remaining,
                    iterations,
                ) != 0
                {
                    floor = *floor_ptr(this);
                    grav_dir = neg(floor);
                    down = scale(grav_dir, (*this).max_step_height + 4.0);
                } else {
                    return;
                }
            } else {
                floor = hit.normal;
                *floor_ptr(this) = floor;
                if hit.actor != (*actor).base && (svc.is_a)(hit.actor, CLASS_PAWN) == 0 {
                    (svc.set_base)(cv, hit.actor);
                }
            }
            let _ = (sub_loc, sub_move);
        }
        if !(*actor).b_just_teleported() {
            (*actor).velocity = div(sub((*actor).location, (*actor).old_location), delta_time);
        }
    }
}

#[inline]
fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
#[inline]
fn near_zero(v: FVector) -> bool {
    v.x.abs() < 1.0e-4 && v.y.abs() < 1.0e-4 && v.z.abs() < 1.0e-4
}
#[inline]
fn size2d(v: FVector) -> f32 {
    super::app_sqrt(v.x.mul_add(v.x, v.y * v.y) as f64) as f32
}
#[inline]
fn min_f(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
