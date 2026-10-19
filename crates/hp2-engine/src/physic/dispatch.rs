//! Port of the dispatch bodies in `Engine/Src/UnPhysic.cpp`: `AActor::FindBase`,
//! `AActor::setPhysics`, `AActor::performPhysics` and `APawn::performPhysics`,
//! the static `StepUpDownStairs`, and `AActor::physicsRotation` and
//! `APawn::physicsRotation`. Family bodies (`physWalking`, `physFalling`, ...)
//! are called as Rust functions; other engine calls go through
//! `Hp2PhysicServices`.

use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, FRotator};

use super::{
    CLASS_PLAYER_PAWN, Hp2PhysicServices, PHYS_FALLING, PHYS_NONE, PHYS_WALKING,
    TRACE_ALL_BLOCKING, ZERO, add, blank_hit, div, fixed_turn, is_a, max_i, min_i, size_sq, sub,
    v3,
};
use crate::coords::{self, Trig};
use crate::math::app_round;
use crate::prim;

/// `EPhysics` values (`EngineClasses.h`) not used by the other physics families.
const PHYS_SWIMMING: u8 = 3;
const PHYS_FLYING: u8 = 4;
const PHYS_ROTATING: u8 = 5;
const PHYS_PROJECTILE: u8 = 6;
const PHYS_ROLLING: u8 = 7;
const PHYS_INTERPOLATING: u8 = 8;
const PHYS_MOVING_BRUSH: u8 = 9;
const PHYS_SPIDER: u8 = 10;
const PHYS_TRAILER: u8 = 11;

/// `Size2D()`: `appSqrt(X*X + Y*Y)`. The clang build fuses `X*X` into the sum.
fn size_2d(a: FVector) -> f32 {
    super::app_sqrt(a.x.mul_add(a.x, a.y * a.y) as f64) as f32
}

/// `FRotator::operator==`.
fn rot_eq(a: FRotator, b: FRotator) -> bool {
    a.pitch == b.pitch && a.yaw == b.yaw && a.roll == b.roll
}

/// `FRotator::IsZero()`: each component is masked with `65535` first.
fn rot_is_zero(r: FRotator) -> bool {
    (r.pitch & 65535) == 0 && (r.yaw & 65535) == 0 && (r.roll & 65535) == 0
}

/// `FRotator::operator*(FLOAT)`: each component is `appRound(c * scale)`.
fn rot_scale(r: FRotator, scale: f32) -> FRotator {
    FRotator {
        pitch: app_round(r.pitch as f32 * scale),
        yaw: app_round(r.yaw as f32 * scale),
        roll: app_round(r.roll as f32 * scale),
    }
}

/// The roll smoothing used by the walking branch and the default branch of
/// `APawn::physicsRotation`. The `65536 - roll` term is an integer; the
/// product with `smooth` fuses into the add.
fn smooth_roll(roll: i32, smooth: f32) -> i32 {
    if roll < 32768 {
        app_round(roll as f32 * (1.0 - smooth))
    } else {
        app_round(((65536i32).wrapping_sub(roll) as f32).mul_add(smooth, roll as f32))
    }
}

/// `AActor::FindBase()`: trace 8 units down and rebase onto the hit actor.
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn find_base(this: *mut AActor, svc: &Hp2PhysicServices) {
    unsafe {
        let mut hit = blank_hit(1.0);
        let location = (*this).location;
        let end = add(location, v3(0.0, 0.0, -8.0));
        let mut extent = ZERO;
        (svc.cylinder_extent)(this as *mut c_void, &mut extent);
        (svc.single_line_check)(
            this as *mut c_void,
            &mut hit,
            &end,
            &location,
            TRACE_ALL_BLOCKING,
            &extent,
        );
        if (*this).base != hit.actor {
            (svc.set_base)(this as *mut c_void, hit.actor);
        }
    }
}

/// `AActor::setPhysics(NewPhysics, NewFloor)`.
///
/// # Safety
/// `this` must point to a live `AActor`; `new_floor` is null or a live `AActor`.
pub unsafe fn set_physics(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    new_physics: u8,
    new_floor: *mut AActor,
) {
    unsafe {
        if (*this).physics == new_physics {
            return;
        }
        (*this).physics = new_physics;
        let physics = (*this).physics;

        if physics == PHYS_WALKING
            || physics == PHYS_NONE
            || physics == PHYS_ROLLING
            || physics == PHYS_ROTATING
            || physics == PHYS_SPIDER
        {
            if !new_floor.is_null() {
                if (*this).base != new_floor as *mut c_void {
                    (svc.set_base)(this as *mut c_void, new_floor as *mut c_void);
                }
            } else {
                find_base(this, svc);
            }
        } else if !(*this).base.is_null() {
            (svc.set_base)(this as *mut c_void, null_mut());
        }

        let final_physics = (*this).physics;
        if final_physics == PHYS_NONE || final_physics == PHYS_ROTATING {
            (*this).velocity = ZERO;
            (*this).acceleration = ZERO;
        }
    }
}

/// `AActor::physTrailer(deltaTime)`.
///
/// # Safety
/// `this` and any non-null owner/mesh returned by `trailer_state` must be live.
pub unsafe fn phys_trailer(this: *mut AActor, svc: &Hp2PhysicServices, _delta_time: f32) {
    unsafe {
        let mut state = super::TrailerState {
            owner: null_mut(),
            mesh: null_mut(),
            pre_pivot: [0.0; 3],
            mass: 0.0,
            draw_type: 0,
            anim_bone: 0,
            flags: 0,
        };
        (svc.trailer_state)(this.cast(), (&mut state as *mut super::TrailerState).cast());
        let owner = state.owner.cast::<AActor>();
        if owner.is_null() {
            return;
        }

        let owner_location = core::ptr::addr_of!((*owner).location).read_unaligned();
        let prepivot = v3(state.pre_pivot[0], state.pre_pivot[1], state.pre_pivot[2]);
        let same_rotation = state.flags & 2 != 0;
        let prepivot_enabled = state.flags & 1 != 0;
        if state.draw_type == 1 {
            let location = if prepivot_enabled {
                add(owner_location, prepivot)
            } else if same_rotation {
                let owner_rotation = core::ptr::addr_of!((*owner).rotation).read_unaligned();
                let mut offset = ZERO;
                (svc.rotation_vector)(&owner_rotation, &mut offset);
                sub(owner_location, super::scale(offset, state.mass))
            } else {
                owner_location
            };
            (svc.trailer_far_move_actor)(this.cast(), &location);
            return;
        }

        let (location, rotation) = if state.anim_bone != 0 && !state.mesh.is_null() {
            let mut coords = hp2_abi::FCoords {
                origin: ZERO,
                x_axis: v3(1.0, 0.0, 0.0),
                y_axis: v3(0.0, 1.0, 0.0),
                z_axis: v3(0.0, 0.0, 1.0),
            };
            (svc.mesh_bone_coords)(
                state.mesh,
                owner.cast(),
                state.anim_bone - 1,
                (&mut coords as *mut hp2_abi::FCoords).cast(),
            );
            (coords.origin, super::interp::ortho_rotation(coords, svc))
        } else {
            let mut location = owner_location;
            let owner_rotation = core::ptr::addr_of!((*owner).rotation).read_unaligned();
            if prepivot_enabled || same_rotation {
                let mut frame = coords::unit_coords();
                let trig = Trig {
                    sin: &|i| (svc.sin_tab)(i),
                    cos: &|i| (svc.cos_tab)(i),
                };
                coords::mul_rot(&mut frame, owner_rotation, &trig);
                location = add(location, prim::transform_vector(prepivot, &frame));
            }
            let rotation = if same_rotation {
                owner_rotation
            } else {
                let velocity = core::ptr::addr_of!((*owner).velocity).read_unaligned();
                if velocity.x.abs() < 1.0e-4
                    && velocity.y.abs() < 1.0e-4
                    && velocity.z.abs() < 1.0e-4
                {
                    FRotator {
                        pitch: 16384,
                        yaw: 0,
                        roll: 0,
                    }
                } else {
                    let direction = super::scale(velocity, -1.0);
                    let mut result = FRotator {
                        pitch: 0,
                        yaw: 0,
                        roll: 0,
                    };
                    (svc.rotation_from_vector)(&direction, &mut result);
                    result
                }
            };
            (location, rotation)
        };

        let block_or_collide = state.flags & (4 | 8) != 0;
        if !block_or_collide {
            let current = core::ptr::addr_of!((*this).location).read_unaligned();
            let delta = sub(location, current);
            let mut hit = blank_hit(1.0);
            (svc.move_actor)(this.cast(), &delta, &rotation, &mut hit);
        } else {
            (svc.trailer_far_move_actor)(this.cast(), &location);
            let mut hit = blank_hit(1.0);
            (svc.move_actor)(this.cast(), &v3(0.01, 0.0, 0.0), &rotation, &mut hit);
        }
    }
}

/// `AActor::performPhysics(DeltaSeconds)`.
///
/// # Safety
/// `this` must point to a live `AActor`. A non-null `pending_touch` must point
/// to a live `AActor` whose `pending_touch` link is valid.
pub unsafe fn perform_physics_actor(this: *mut AActor, svc: &Hp2PhysicServices, delta_time: f32) {
    unsafe {
        // change position
        match (*this).physics {
            PHYS_PROJECTILE => super::motion::phys_projectile(this, svc, delta_time, 0),
            PHYS_FALLING => super::fall::phys_falling(this, svc, delta_time, 0),
            PHYS_ROTATING | PHYS_INTERPOLATING => {}
            PHYS_MOVING_BRUSH => {
                (*this).old_location = (*this).location;
                (svc.phys_moving_brush)(this as *mut c_void, delta_time);
                (*this).velocity = div(sub((*this).location, (*this).old_location), delta_time);
            }
            PHYS_TRAILER => phys_trailer(this, svc, delta_time),
            PHYS_ROLLING => super::motion::phys_rolling(this, svc, delta_time, 0),
            _ => {}
        }

        // rotate
        if !rot_is_zero((*this).rotation_rate) {
            physics_rotation_actor(this, svc, delta_time);
        }

        // allow touched actors to impact physics
        if !(*this).pending_touch.is_null() {
            (svc.event_post_touch)((*this).pending_touch, this as *mut c_void);
            let old_touch = (*this).pending_touch as *mut AActor;
            (*this).pending_touch = (*old_touch).pending_touch;
            (*old_touch).pending_touch = null_mut();
        }
    }
}

/// Rounds `Acceleration` to replicated precision (`execAutonomousPhysics`).
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn round_acceleration(this: *mut AActor) {
    unsafe {
        let a = &mut (*this).acceleration;
        a.x = 0.1_f32 * app_round(10.0_f32 * a.x) as f32;
        a.y = 0.1_f32 * app_round(10.0_f32 * a.y) as f32;
        a.z = 0.1_f32 * app_round(10.0_f32 * a.z) as f32;
    }
}

/// `StepUpDownStairs(APawn *const ap)`.
///
/// # Safety
/// `ap` must point to a live `APawn`.
unsafe fn step_up_down_stairs(ap: *mut APawn) {
    unsafe {
        let epsilon = 1.0_f32;

        // do it just if pawn is WALKING up/down stairs
        if (*ap).base.physics == PHYS_WALKING {
            let step_height = (*ap).step_height;
            let step_width = (*ap).step_width;
            let mut delta_z = size_2d(sub((*ap).base.location, (*ap).base.old_location))
                * step_height
                / step_width;
            if delta_z > step_height {
                delta_z = step_height;
            }

            // if going UP ....
            if (*ap).base.saved_pre_pivot_z < 0.0 {
                (*ap).base.saved_pre_pivot_z += delta_z;
                if (*ap).base.saved_pre_pivot_z > 0.0 {
                    (*ap).base.saved_pre_pivot_z = 0.0;
                }
            }
            // if going DOWN ....
            else if (*ap).base.saved_pre_pivot_z > 0.0 {
                (*ap).base.saved_pre_pivot_z -= delta_z;
                if (*ap).base.saved_pre_pivot_z < 0.0 {
                    (*ap).base.saved_pre_pivot_z = 0.0;
                }
            }

            let mut delta = (*ap).base.old_location.z - (*ap).base.location.z;

            // Start to go Down ...
            if delta < step_height + epsilon && delta > step_height - epsilon {
                (*ap).base.saved_pre_pivot_z = step_height - delta_z;
            }

            delta = (*ap).base.location.z - (*ap).base.old_location.z;

            // Start to go UP ...
            if delta < step_height + epsilon && delta > step_height - epsilon {
                (*ap).base.saved_pre_pivot_z = -(step_height - delta_z);
            }
        }
        // if this is not a walking phisics anymore, reset PrePivot back
        else {
            (*ap).base.saved_pre_pivot_z = 0.0;
        }
    }
}

/// `APawn::performPhysics(DeltaSeconds)`.
///
/// # Safety
/// `this` must point to a live `APawn`. A non-null `pending_touch` must point
/// to a live `AActor`.
pub unsafe fn perform_physics_pawn(this: *mut APawn, svc: &Hp2PhysicServices, delta_time: f32) {
    unsafe {
        let actor = addr_of_mut!((*this).base);
        let old_velocity = (*actor).velocity;

        // change position
        match (*actor).physics {
            PHYS_WALKING => super::walk::phys_walking(this, svc, delta_time, 0),
            PHYS_FALLING => super::fall::phys_falling(actor, svc, delta_time, 0),
            PHYS_FLYING => super::swim::phys_flying(this, svc, delta_time, 0),
            PHYS_SWIMMING => super::swim::phys_swimming(this, svc, delta_time, 0),
            PHYS_SPIDER => super::motion::phys_spider(this, svc, delta_time, 0),
            PHYS_INTERPOLATING => {}
            PHYS_PROJECTILE => super::motion::phys_projectile(actor, svc, delta_time, 0),
            PHYS_TRAILER => phys_trailer(actor, svc, delta_time),
            PHYS_ROLLING => super::motion::phys_rolling(actor, svc, delta_time, 0),
            _ => {}
        }

        // check it out if he is WALKING up (or down) stairs
        step_up_down_stairs(this);

        // rotate
        let physics = (*actor).physics;
        if physics != PHYS_SPIDER
            && physics != PHYS_INTERPOLATING
            && (is_a(svc, actor as *mut c_void, CLASS_PLAYER_PAWN)
                || (!rot_eq((*actor).rotation, (*actor).desired_rotation)
                    && (*actor).b_rotate_to_desired())
                || (!(*actor).b_rotate_to_desired() && (*actor).b_fixed_rotation_dir()))
        {
            physics_rotation_pawn(this, svc, delta_time, old_velocity);
        }

        (*this).move_timer -= delta_time;
        (*this).avg_physics_time = (*this).avg_physics_time.mul_add(0.8, 0.2 * delta_time);

        // allow touched actors to impact physics
        if !(*actor).pending_touch.is_null() {
            (svc.event_post_touch)((*actor).pending_touch, actor as *mut c_void);
            if !(*actor).pending_touch.is_null() {
                let old_touch = (*actor).pending_touch as *mut AActor;
                (*actor).pending_touch = (*old_touch).pending_touch;
                (*old_touch).pending_touch = null_mut();
            }
        }
    }
}

/// `AActor::physicsRotation(DeltaSeconds)`.
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn physics_rotation_actor(this: *mut AActor, svc: &Hp2PhysicServices, delta_time: f32) {
    unsafe {
        if (!(*this).b_rotate_to_desired() && !(*this).b_fixed_rotation_dir())
            || ((*this).b_rotate_to_desired() && rot_eq((*this).rotation, (*this).desired_rotation))
        {
            return;
        }

        // Accumulate a desired new rotation.
        let mut new_rot = (*this).rotation;
        let delta_rot = rot_scale((*this).rotation_rate, delta_time);

        //YAW
        if delta_rot.yaw != 0
            && (!(*this).b_rotate_to_desired() || (*this).desired_rotation.yaw != new_rot.yaw)
        {
            new_rot.yaw = fixed_turn(
                new_rot.yaw,
                (*this).desired_rotation.yaw,
                delta_rot.yaw,
                (*this).b_fixed_rotation_dir(),
                (*this).b_rotate_to_desired(),
            );
        }
        //PITCH
        if delta_rot.pitch != 0
            && (!(*this).b_rotate_to_desired() || (*this).desired_rotation.pitch != new_rot.pitch)
        {
            new_rot.pitch = fixed_turn(
                new_rot.pitch,
                (*this).desired_rotation.pitch,
                delta_rot.pitch,
                (*this).b_fixed_rotation_dir(),
                (*this).b_rotate_to_desired(),
            );
        }
        //ROLL
        if delta_rot.roll != 0
            && (!(*this).b_rotate_to_desired() || (*this).desired_rotation.roll != new_rot.roll)
        {
            new_rot.roll = fixed_turn(
                new_rot.roll,
                (*this).desired_rotation.roll,
                delta_rot.roll,
                (*this).b_fixed_rotation_dir(),
                (*this).b_rotate_to_desired(),
            );
        }

        // Set the new rotation.
        if !rot_eq(new_rot, (*this).rotation) {
            let mut hit = blank_hit(1.0);
            (svc.move_actor)(this as *mut c_void, &ZERO, &new_rot, &mut hit);
        }

        if (*this).b_rotate_to_desired()
            && rot_eq((*this).rotation, (*this).desired_rotation)
            && (svc.is_probing_ended_rotation)(this as *mut c_void) != 0
        {
            (svc.event_ended_rotation)(this as *mut c_void); //tell thing rotation ended
        }
    }
}

/// `APawn::physicsRotation(deltaTime, OldVelocity)`.
///
/// # Safety
/// `this` must point to a live `APawn`.
pub unsafe fn physics_rotation_pawn(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
    old_velocity: FVector,
) {
    unsafe {
        let actor = addr_of_mut!((*this).base);

        // Accumulate a desired new rotation.
        let mut new_rot = (*actor).rotation;

        if !(*actor).b_rotate_to_desired() && (*actor).b_fixed_rotation_dir() {
            physics_rotation_actor(actor, svc, delta_time);
            return;
        }

        let fixed_dir = (*actor).b_fixed_rotation_dir();
        let rotate_to_desired = (*actor).b_rotate_to_desired();
        let rate = (*actor).rotation_rate;
        let desired = (*actor).desired_rotation;

        //YAW
        let delta_yaw = app_round(rate.yaw as f32 * delta_time);
        if desired.yaw != new_rot.yaw {
            new_rot.yaw = fixed_turn(
                new_rot.yaw,
                desired.yaw,
                delta_yaw,
                fixed_dir,
                rotate_to_desired,
            );
        }

        //PITCH
        if desired.pitch != new_rot.pitch {
            let delta_pitch = app_round(rate.pitch as f32 * delta_time);
            new_rot.pitch = fixed_turn(
                new_rot.pitch,
                desired.pitch,
                delta_pitch,
                fixed_dir,
                rotate_to_desired,
            );
        }

        //ROLL
        if rate.roll > 0 {
            //pawns roll based on physics
            if (*actor).physics == PHYS_WALKING && size_sq((*actor).velocity) < 40000.0 {
                let smooth = prim::min_f(1.0, 8.0 * delta_time);
                new_rot.roll = smooth_roll(new_rot.roll, smooth);
            } else {
                let mut real_accel = div(sub((*actor).velocity, old_velocity), delta_time);
                if size_sq(real_accel) > 10000.0 {
                    let mut max_roll = 28000.0_f32;
                    if (*actor).physics == PHYS_WALKING {
                        max_roll = 4096.0;
                    }
                    new_rot.roll = 0;

                    // RealAcceleration.TransformVectorBy(GMath.UnitCoords / NewRotation)
                    let sin = |i: i32| (svc.sin_tab)(i);
                    let cos = |i: i32| (svc.cos_tab)(i);
                    let trig = Trig {
                        sin: &sin,
                        cos: &cos,
                    };
                    let mut frame = coords::unit_coords();
                    coords::div_rot(&mut frame, new_rot, &trig);
                    real_accel = prim::transform_vector(real_accel, &frame);

                    let accel_rate = (*this).accel_rate;
                    if real_accel.y > 0.0 {
                        new_rot.roll =
                            min_i(rate.roll, app_round(real_accel.y * max_roll / accel_rate));
                    } else {
                        new_rot.roll = max_i(
                            (65536i32).wrapping_sub(rate.roll),
                            app_round(65536.0 + real_accel.y * max_roll / accel_rate),
                        );
                    }

                    //smoothly change rotation
                    (*actor).rotation.roll &= 65535;
                    if new_rot.roll > 32768 {
                        if (*actor).rotation.roll < 32768 {
                            (*actor).rotation.roll = (*actor).rotation.roll.wrapping_add(65536);
                        }
                    } else if (*actor).rotation.roll > 32768 {
                        (*actor).rotation.roll = (*actor).rotation.roll.wrapping_sub(65536);
                    }

                    let smooth = prim::min_f(1.0, 5.0 * delta_time);
                    new_rot.roll = app_round(
                        (new_rot.roll as f32)
                            .mul_add(smooth, (*actor).rotation.roll as f32 * (1.0 - smooth)),
                    );
                } else {
                    let smooth = prim::min_f(1.0, 8.0 * delta_time);
                    new_rot.roll = smooth_roll(new_rot.roll, smooth);
                }
            }
        } else {
            new_rot.roll = 0;
        }

        // Set the new rotation.
        if !rot_eq(new_rot, (*actor).rotation) {
            let mut hit = blank_hit(1.0);
            (svc.move_actor)(actor as *mut c_void, &ZERO, &new_rot, &mut hit);
        }
    }
}
