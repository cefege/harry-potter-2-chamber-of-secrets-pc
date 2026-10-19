//! `APawn` swimming and flying bodies (`UnPhysic.cpp`): `startSwimming`,
//! `physFlying`, `Swim`, `findWaterLine`, `physSwimming`.

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult};

use super::{
    CLASS_PLAYER_PAWN, Hp2PhysicServices, PHYS_FALLING, ZERO, abs_f, add, app_sqrt, blank_hit,
    calc_velocity, div, dot, is_a, is_zero, move_actor, process_hit_wall, safe_normal, scale, size,
    size_sq, step_up, sub, two_wall_adjust, v3,
};

/// `EPhysics` values (`EngineClasses.h`) not used by the shared constants.
const PHYS_SWIMMING: u8 = 3;
const PHYS_FLYING: u8 = 4;

/// `Size2D()`: `appSqrt(X*X+Y*Y)`. Clang fuses `X*X` into the `Y*Y` addend.
fn size2d(a: FVector) -> f32 {
    app_sqrt(a.x.mul_add(a.x, a.y * a.y) as f64) as f32
}

/// `FVector::operator!=`: true when any component differs.
fn vec_ne(a: FVector, b: FVector) -> bool {
    a.x != b.x || a.y != b.y || a.z != b.z
}

/// `Region.Zone` of `actor`, read fresh (`MoveActor` may change the region).
unsafe fn region_zone(actor: *mut AActor) -> *mut AZoneInfo {
    unsafe { (*actor).region.zone.cast::<AZoneInfo>() }
}

/// `APawn::startSwimming(OldVelocity, timeTick, remainingTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `APawn` with a live `Region.Zone`.
pub unsafe fn start_swimming(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    old_velocity: FVector,
    time_tick: f32,
    mut remaining_time: f32,
    iterations: i32,
) {
    unsafe {
        let actor = this.cast::<AActor>();
        let mut end = (*actor).location;
        find_water_line(this, svc, (*actor).old_location, &mut end);
        let mut water_time = 0.0f32;
        if vec_ne(end, (*actor).location) {
            water_time = time_tick * size(sub(end, (*actor).location))
                / size(sub((*actor).location, (*actor).old_location));
            remaining_time += water_time;
            let mut hit = blank_hit(1.0);
            move_actor(
                this,
                svc,
                sub(end, (*actor).location),
                &mut hit as *mut FCheckResult,
            );
        }
        if !(*actor).b_bounce() && !(*actor).b_just_teleported() {
            // actual average velocity
            (*actor).velocity = div(
                sub((*actor).location, (*actor).old_location),
                time_tick - water_time,
            );
            // end velocity has 2* accel of avg
            (*actor).velocity = sub(scale((*actor).velocity, 2.0), old_velocity);
            if size_sq((*actor).velocity) > 16_000_000.0 {
                (*actor).velocity = safe_normal((*actor).velocity);
                (*actor).velocity = scale((*actor).velocity, 4000.0);
            }
        }
        if ((*actor).velocity.z > -160.0) && ((*actor).velocity.z < 0.0) {
            // allow for falling out of water; smooth bobbing
            (*actor).velocity.z = size2d((*actor).velocity).mul_add(-0.7, -80.0);
        }
        if remaining_time > 0.01 {
            phys_swimming(this, svc, remaining_time, iterations);
        }
    }
}

/// `APawn::physFlying(deltaTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `APawn` with a live `Region.Zone`.
pub unsafe fn phys_flying(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    iterations: i32,
) {
    unsafe {
        // C++ increments `Iterations` but never reads it again.
        let _ = iterations;
        let actor = this.cast::<AActor>();

        if (*actor).b_collide_world() && (*actor).region.zone_number == 0 {
            // not in valid spot
            if !(*this).b_is_player() {
                (svc.debugf_flew_out)(actor.cast());
                (svc.destroy_actor)(actor.cast());
            }
            return;
        }
        let acceleration = (*actor).acceleration;
        let accel_dir = if is_zero(acceleration) {
            acceleration
        } else {
            safe_normal(acceleration)
        };
        calc_velocity(
            this,
            svc,
            accel_dir,
            delta_time,
            (*this).air_speed,
            (*region_zone(actor)).zone_fluid_friction,
            1,
            0,
            0,
        );

        (*actor).old_location = (*actor).location;
        (*actor).set_b_just_teleported(false);
        let zone_vel = if is_a(svc, this.cast(), CLASS_PLAYER_PAWN)
            || size_sq((*region_zone(actor)).zone_velocity) > 90000.0
        {
            (*region_zone(actor)).zone_velocity
        } else {
            ZERO
        };
        let adjusted = scale(add((*actor).velocity, zone_vel), delta_time);
        let mut hit = blank_hit(1.0);
        move_actor(this, svc, adjusted, &mut hit as *mut FCheckResult);
        if hit.time < 1.0 {
            let mut grav_dir = v3(0.0, 0.0, -1.0);
            if (*region_zone(actor)).zone_gravity.z > 0.0 {
                grav_dir.z = 1.0;
            }
            let desired_dir = safe_normal(adjusted);
            let vel_dir = safe_normal((*actor).velocity);
            let up_down = dot(grav_dir, vel_dir);
            if abs_f(hit.normal.z) < 0.2 && up_down < 0.5 && up_down > -0.2 {
                let step_z = (*actor).location.z;
                step_up(
                    this,
                    svc,
                    grav_dir,
                    desired_dir,
                    scale(adjusted, 1.0 - hit.time),
                    &mut hit as *mut FCheckResult,
                );
                (*actor).old_location.z = (*actor).location.z + ((*actor).old_location.z - step_z);
            } else {
                process_hit_wall(actor, svc, hit.normal, hit.actor);
                // adjust and try again
                let old_hit_normal = hit.normal;
                let mut delta = scale(
                    sub(adjusted, scale(hit.normal, dot(adjusted, hit.normal))),
                    1.0 - hit.time,
                );
                if dot(delta, adjusted) >= 0.0 {
                    move_actor(this, svc, delta, &mut hit as *mut FCheckResult);
                    if hit.time < 1.0 {
                        // hit second wall
                        process_hit_wall(actor, svc, hit.normal, hit.actor);
                        two_wall_adjust(
                            desired_dir,
                            &mut delta,
                            hit.normal,
                            old_hit_normal,
                            hit.time,
                        );
                        move_actor(this, svc, delta, &mut hit as *mut FCheckResult);
                    }
                }
            }
        }

        if !(*actor).b_just_teleported() {
            (*actor).velocity = div(sub((*actor).location, (*actor).old_location), delta_time);
        }
    }
}

/// `APawn::Swim(Delta, Hit)`: returns the fraction of `Delta` spent in air.
///
/// # Safety
/// `this` must point to a live `APawn`; `hit` must point to a live `FCheckResult`.
pub unsafe fn swim(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta: FVector,
    hit: *mut FCheckResult,
) -> f32 {
    unsafe {
        let actor = this.cast::<AActor>();
        let start = (*actor).location;
        let mut air_time = 0.0f32;
        move_actor(this, svc, delta, hit);
        let mut end = (*actor).location;
        if !(*region_zone(actor)).b_water_zone() {
            // then left water
            find_water_line(this, svc, start, &mut end);
            if vec_ne(end, (*actor).location) {
                air_time = size(sub(end, (*actor).location)) / size(delta);
                move_actor(this, svc, sub(end, (*actor).location), hit);
            }
        }
        air_time
    }
}

/// `APawn::findWaterLine(Start, End)`, with the recursion as a loop.
///
/// # Safety
/// `this` must point to a live `APawn` with a live `Region.Zone`; `end` must be
/// a live `FVector`.
pub unsafe fn find_water_line(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    start: FVector,
    end: *mut FVector,
) {
    unsafe {
        let actor = this.cast::<AActor>();
        let mut start = start;
        loop {
            if size_sq(sub(*end, start)) < 1.0 {
                // current value of End is acceptable
                return;
            }
            let mid_point = scale(add(start, *end), 0.5);
            // `GetLevel()->Model->PointRegion(Level, MidPoint).Zone`, via the thunk.
            let zone = (svc.point_region_zone)(actor.cast(), &mid_point);
            if (*zone.cast::<AZoneInfo>()).b_water_zone() != (*region_zone(actor)).b_water_zone() {
                start = mid_point;
            } else {
                *end = mid_point;
            }
        }
    }
}

/// `APawn::physSwimming(deltaTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `APawn` with live `Region.Zone` and `HeadRegion.Zone`.
pub unsafe fn phys_swimming(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    iterations: i32,
) {
    unsafe {
        let actor = this.cast::<AActor>();
        // C++ signed overflow is UB; `Iterations` is < 8 in practice.
        let iterations = iterations.wrapping_add(1);

        if !(*((*this).head_region.zone.cast::<AZoneInfo>())).b_water_zone()
            && (*actor).velocity.z > 100.0
        {
            // damp positive Z out of water
            (*actor).velocity.z *= 1.0 - delta_time;
        }

        (*actor).old_location = (*actor).location;
        (*actor).set_b_just_teleported(false);
        let acceleration = (*actor).acceleration;
        let accel_dir = if is_zero(acceleration) {
            acceleration
        } else {
            safe_normal(acceleration)
        };
        calc_velocity(
            this,
            svc,
            accel_dir,
            delta_time,
            (*this).water_speed,
            (*region_zone(actor)).zone_fluid_friction,
            1,
            0,
            1,
        );
        let mut vel_z = (*actor).velocity.z;
        let zone_vel = if is_a(svc, this.cast(), CLASS_PLAYER_PAWN)
            || size_sq((*region_zone(actor)).zone_velocity) > 90000.0
        {
            // Add effect of velocity zone (scaled by 25 and the frame time, so
            // clamped swimming does not amplify it at fast frame rates).
            scale(scale((*region_zone(actor)).zone_velocity, 25.0), delta_time)
        } else {
            ZERO
        };
        let adjusted = scale(add((*actor).velocity, zone_vel), delta_time);
        let mut hit = blank_hit(1.0);
        let mut remaining_time =
            delta_time * swim(this, svc, adjusted, &mut hit as *mut FCheckResult);

        if hit.time < 1.0 {
            let mut grav_dir = v3(0.0, 0.0, -1.0);
            if (*region_zone(actor)).zone_gravity.z > 0.0 {
                grav_dir.z = 1.0;
            }
            let desired_dir = safe_normal(adjusted);
            let vel_dir = safe_normal((*actor).velocity);
            let up_down = dot(grav_dir, vel_dir);
            if abs_f(hit.normal.z) < 0.2 && up_down < 0.5 && up_down > -0.2 {
                let step_z = (*actor).location.z;
                step_up(
                    this,
                    svc,
                    grav_dir,
                    desired_dir,
                    scale(adjusted, 1.0 - hit.time),
                    &mut hit as *mut FCheckResult,
                );
                (*actor).old_location.z = (*actor).location.z + ((*actor).old_location.z - step_z);
            } else {
                process_hit_wall(actor, svc, hit.normal, hit.actor);
                // adjust and try again
                let old_hit_normal = hit.normal;
                let mut delta = scale(
                    sub(adjusted, scale(hit.normal, dot(adjusted, hit.normal))),
                    1.0 - hit.time,
                );
                if dot(delta, adjusted) >= 0.0 {
                    // `remainingTime * (1 - Hit.Time)` is evaluated before `Swim`.
                    let scaled = remaining_time * (1.0 - hit.time);
                    remaining_time = scaled * swim(this, svc, delta, &mut hit as *mut FCheckResult);
                    if hit.time < 1.0 {
                        // hit second wall
                        process_hit_wall(actor, svc, hit.normal, hit.actor);
                        two_wall_adjust(
                            desired_dir,
                            &mut delta,
                            hit.normal,
                            old_hit_normal,
                            hit.time,
                        );
                        let scaled = remaining_time * (1.0 - hit.time);
                        remaining_time =
                            scaled * swim(this, svc, delta, &mut hit as *mut FCheckResult);
                    }
                }
            }
        }

        if !(*actor).b_just_teleported() && remaining_time < delta_time {
            let water_jump = vel_z != (*actor).velocity.z; // changed by script
            if water_jump {
                vel_z = (*actor).velocity.z;
            }
            (*actor).velocity = div(
                sub((*actor).location, (*actor).old_location),
                delta_time - remaining_time,
            );
            if water_jump {
                (*actor).velocity.z = vel_z;
            }
        }

        if !(*region_zone(actor)).b_water_zone() {
            if (*actor).physics == PHYS_SWIMMING {
                // in case script didn't change it (w/ zone change)
                super::dispatch::set_physics(actor, svc, PHYS_FALLING, std::ptr::null_mut());
            }
            if (*actor).velocity.z < 160.0 && (*actor).velocity.z > 0.0 {
                // allow for falling out of water; smooth bobbing
                (*actor).velocity.z = size2d((*actor).velocity).mul_add(0.4, 40.0);
            }
        }

        // may have left water - if so, script might have set new physics mode
        if remaining_time > 0.01 {
            if (*actor).physics == PHYS_FALLING {
                super::fall::phys_falling(actor, svc, remaining_time, iterations);
            } else if (*actor).physics == PHYS_FLYING {
                phys_flying(this, svc, remaining_time, iterations);
            }
        }
    }
}
