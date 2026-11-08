//! Port of the dispatch bodies in `Engine/Src/UnPhysic.cpp`: `AActor::FindBase`,
//! `AActor::setPhysics`, `AActor::performPhysics` and `APawn::performPhysics`,
//! the static `StepUpDownStairs`, and `AActor::physicsRotation` and
//! `APawn::physicsRotation`. Family bodies (`physWalking`, `physFalling`, ...)
//! reenter their qualified native member guards through checked
//! `Hp2PhysicServices` routes; numerical policy remains in Rust.

use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};
use std::sync::LazyLock;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, AMover, APawn, AZoneInfo, FRotator};

use super::{
    CLASS_PLAYER_PAWN, Hp2PhysicServices, PHYS_FALLING, PHYS_NONE, PHYS_WALKING, PhysicsError,
    TRACE_ALL_BLOCKING, ZERO, add, blank_hit, cylinder_extent, div, dot, fixed_turn, is_a, max_i,
    min_i, scale, set_base, size_sq, sub, v3,
};
use crate::coords::{self, Trig};
use crate::math::app_round;
use crate::native_guard::guarded;
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

/// `FRotator::operator+(FRotator)`.
fn rot_add(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_add(b.pitch),
        yaw: a.yaw.wrapping_add(b.yaw),
        roll: a.roll.wrapping_add(b.roll),
    }
}

/// `FRotator::operator-(FRotator)`.
fn rot_sub(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_sub(b.pitch),
        yaw: a.yaw.wrapping_sub(b.yaw),
        roll: a.roll.wrapping_sub(b.roll),
    }
}

/// `FRotator::operator*(FLOAT)` (`UnMath.h`): `hp2_rs_core_math_rotator_scale`,
/// a truncating cast per component, unlike this file's `rot_scale` (which
/// matches the hand-rolled `appRound` formula used by `physicsRotation`).
fn rot_mul(a: FRotator, s: f32) -> FRotator {
    FRotator {
        pitch: (a.pitch as f32 * s) as i32,
        yaw: (a.yaw as f32 * s) as i32,
        roll: (a.roll as f32 * s) as i32,
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
pub unsafe fn find_base(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
) -> crate::physic::PhysicsResult<()> {
    let actor_svc = crate::actor::published_services().map_err(PhysicsError)?;
    unsafe { crate::actor_physics_owner::find_base_with_ops(&mut NativeFindBaseOps { svc, actor_svc: &actor_svc }, this) }
}

struct NativeFindBaseOps<'a> {
    svc: &'a Hp2PhysicServices,
    actor_svc: &'a crate::actor::Hp2ActorServices,
}

impl crate::actor_physics_owner::FindBaseOps for NativeFindBaseOps<'_> {
    type Error = PhysicsError;

    unsafe fn cylinder_extent(&mut self, actor: *mut AActor) -> Result<FVector, Self::Error> {
        let mut extent = ZERO;
        unsafe { cylinder_extent(Some(self.actor_svc), actor.cast(), &mut extent)?; }
        Ok(extent)
    }

    unsafe fn single_line_check(
        &mut self,
        actor: *mut AActor,
        hit: *mut hp2_abi::engine::generated::FCheckResult,
        end: FVector,
        start: FVector,
        trace: u32,
        extent: FVector,
    ) -> Result<(), Self::Error> {
        let native_trace = if trace == crate::levact::TRACE_ALL_BLOCKING { TRACE_ALL_BLOCKING } else { trace as i32 };
        let _ = self.svc.single_line_check(actor.cast(), hit, &end, &start, native_trace, &extent)?;
        Ok(())
    }

    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> Result<(), Self::Error> {
        unsafe { set_base(Some(self.actor_svc), actor.cast(), new_base.cast()) }
    }
}

/// Operations needed by `AActor::setPhysics`, shared by native and Core paths.
pub(crate) trait SetPhysicsOps {
    type Error;
    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn find_base(&mut self, actor: *mut AActor) -> Result<(), Self::Error>;
}

/// Runs the shared `AActor::setPhysics` state transition.
///
/// # Safety
/// `actor`, `new_floor`, and all objects reached by Ops must remain live.
pub(crate) unsafe fn set_physics_with_ops<O: SetPhysicsOps>(
    ops: &mut O,
    actor: *mut AActor,
    new_physics: u8,
    new_floor: *mut AActor,
) -> Result<(), O::Error> {
    if unsafe { (*actor).physics } == new_physics {
        return Ok(());
    }
    unsafe { (*actor).physics = new_physics; }
    let physics = unsafe { (*actor).physics };
    if physics == PHYS_WALKING
        || physics == PHYS_NONE
        || physics == PHYS_ROLLING
        || physics == PHYS_ROTATING
        || physics == PHYS_SPIDER
    {
        if !new_floor.is_null() {
            if unsafe { (*actor).base } != new_floor.cast() {
                unsafe { ops.set_base(actor, new_floor)?; }
            }
        } else {
            unsafe { ops.find_base(actor)?; }
        }
    } else if !unsafe { (*actor).base }.is_null() {
        unsafe { ops.set_base(actor, null_mut())?; }
    }
    let final_physics = unsafe { (*actor).physics };
    if final_physics == PHYS_NONE || final_physics == PHYS_ROTATING {
        unsafe {
            (*actor).velocity = ZERO;
            (*actor).acceleration = ZERO;
        }
    }
    Ok(())
}

struct NativeSetPhysicsOps<'a> {
    physics: &'a Hp2PhysicServices,
    actor: &'a crate::actor::Hp2ActorServices,
}

impl SetPhysicsOps for NativeSetPhysicsOps<'_> {
    type Error = PhysicsError;
    unsafe fn set_base(
        &mut self,
        actor: *mut AActor,
        new_base: *mut AActor,
    ) -> Result<(), Self::Error> {
        unsafe { super::set_base(Some(self.actor), actor.cast(), new_base.cast()) }
    }
    unsafe fn find_base(&mut self, actor: *mut AActor) -> Result<(), Self::Error> {
        unsafe { self.physics.call_find_base(actor) }
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
) -> crate::physic::PhysicsResult<()> {
    unsafe {
        guarded(c"AActor::setPhysics", || {
            if (*this).physics == new_physics {
                return Ok(());
            }
            let actor_svc = crate::actor::published_services().map_err(PhysicsError)?;
            let mut ops = NativeSetPhysicsOps { physics: svc, actor: &actor_svc };
            set_physics_with_ops(&mut ops, this, new_physics, new_floor)
        })
    }
}

/// `AActor::physTrailer(deltaTime)`.
///
/// # Safety
/// `this` and any non-null owner/mesh returned by `trailer_state` must be live.
pub unsafe fn phys_trailer(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    _delta_time: f32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
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
            svc.trailer_state(this.cast(), (&mut state as *mut super::TrailerState).cast())?;
            let owner = state.owner.cast::<AActor>();
            if owner.is_null() {
                return Ok(());
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
                    svc.rotation_vector(&owner_rotation, &mut offset)?;
                    sub(owner_location, super::scale(offset, state.mass))
                } else {
                    owner_location
                };
                svc.trailer_far_move_actor(this.cast(), &location)?;
                return Ok(());
            }

            let (location, rotation) = if state.anim_bone != 0 && !state.mesh.is_null() {
                let mut coords = hp2_abi::FCoords {
                    origin: ZERO,
                    x_axis: v3(1.0, 0.0, 0.0),
                    y_axis: v3(0.0, 1.0, 0.0),
                    z_axis: v3(0.0, 0.0, 1.0),
                };
                svc.mesh_bone_coords(
                    state.mesh,
                    owner.cast(),
                    state.anim_bone - 1,
                    (&mut coords as *mut hp2_abi::FCoords).cast(),
                )?;
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
                        svc.rotation_from_vector(&direction, &mut result)?;
                        result
                    }
                };
                (location, rotation)
            };

            // GetBoneCoords may reenter script and change collision flags.
            let block_or_collide = (*this).b_block_actors() || (*this).b_collide_world();
            if !block_or_collide {
                let current = core::ptr::addr_of!((*this).location).read_unaligned();
                let delta = sub(location, current);
                let mut hit = blank_hit(1.0);
                svc.move_actor(this.cast(), &delta, &rotation, &mut hit)?;
            } else {
                svc.trailer_far_move_actor(this.cast(), &location)?;
                let mut hit = blank_hit(1.0);
                svc.move_actor(this.cast(), &v3(0.01, 0.0, 0.0), &rotation, &mut hit)?;
            }
        }
    })
}

/// `AMover` class id for `Hp2PhysicServices::is_a`.
const CLASS_MOVER: i32 = 5;

/// `EMoverGlideType` (`EngineClasses.h`).
const MV_MOVE_BY_TIME: u8 = 0;
const MV_GLIDE_BY_TIME: u8 = 1;
const MV_SPRING_BY_TIME: u8 = 2;

/// `ARRAY_COUNT(Mover->KeyPos)`: `physMovingBrush`'s `KeyNum` clamp uses this
/// as the upper bound, not `ARRAY_COUNT - 1`, so `KeyNum == 8` is reachable.
/// The original C++ then indexes `KeyPos[8]`/`KeyRot[8]`, one past each
/// array, which silently aliases the next field (`KeyRot[0]`/`BasePos`) with
/// no crash. [`mover_key_pos`]/[`mover_key_pos_set`]/[`mover_key_rot`]
/// replicate that exact byte-level read instead of letting Rust's bounds
/// check panic, which would diverge (an unguard log line the original never
/// produced) on this reachable out-of-range `KeyNum`.
const MOVER_KEY_ARRAY_COUNT: i32 = 8;

/// `Clamp(X, Min, Max)` (`UnTemplate.h`), for `INT`.
fn clamp_i(x: i32, min: i32, max: i32) -> i32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// Reads `Mover->KeyPos[idx]`, including `idx == 8` (one past the array,
/// aliasing `KeyRot[0]`), matching the original's unchecked array decay.
unsafe fn mover_key_pos(mv: *const AMover, idx: usize) -> FVector {
    unsafe {
        let base = mv as *const u8;
        let offset = std::mem::offset_of!(AMover, key_pos) + idx * size_of::<FVector>();
        base.add(offset).cast::<FVector>().read_unaligned()
    }
}

/// Writes `Mover->KeyPos[idx]`, including `idx == 8` (one past the array,
/// aliasing `KeyRot[0]`), matching the original's unchecked array decay.
unsafe fn mover_key_pos_set(mv: *mut AMover, idx: usize, value: FVector) {
    unsafe {
        let base = mv as *mut u8;
        let offset = std::mem::offset_of!(AMover, key_pos) + idx * size_of::<FVector>();
        base.add(offset).cast::<FVector>().write_unaligned(value);
    }
}

/// Reads `Mover->KeyRot[idx]`, including `idx == 8` (one past the array,
/// aliasing `BasePos`), matching the original's unchecked array decay.
unsafe fn mover_key_rot(mv: *const AMover, idx: usize) -> FRotator {
    unsafe {
        let base = mv as *const u8;
        let offset = std::mem::offset_of!(AMover, key_rot) + idx * size_of::<FRotator>();
        base.add(offset).cast::<FRotator>().read_unaligned()
    }
}

/// `MoverDebugEnabled()` (`UnScript.cpp`): `HP2_MOVER_DEBUG=1` enables mover
/// freeze logging. Cached after the first read, like the original's static.
static MOVER_DEBUG: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("HP2_MOVER_DEBUG")
        .map(|v| v.starts_with('1'))
        .unwrap_or(false)
});

fn mover_debug_enabled() -> bool {
    *MOVER_DEBUG
}

/// `MV_SpringByTime` branch of `physMovingBrush`'s alpha computation
/// (`UnScript.cpp`). The source mixes `float` locals with unsuffixed
/// (`double`) literals; per C++'s usual arithmetic conversions every
/// sub-expression touching a literal computes in `f64` and narrows to `f32`
/// only where the source assigns into a `float`, exactly as replicated here.
fn spring_render_alpha(mv: &AMover, new_alpha: f32) -> f32 {
    let move_time = mv.move_time;
    let mut f_start_spring = (move_time - mv.mover_spring_time) / move_time;
    let mut f_max_amplitude = mv.mover_max_amplitude;
    let mut i_mover_fluctuations = mv.mover_fluctuations as i32;

    // set reasonable iMoverFluctuations, if level editor made a zero
    if i_mover_fluctuations == 0 {
        i_mover_fluctuations = 1;
    }
    // set reasonable fStartSpring, if level editor made a mistake
    if f_start_spring <= 0.0 || f_start_spring >= 1.0 {
        f_start_spring = 0.75;
    }
    // set reasonable fMaxAmplitude, if level editor made a mistake
    if f_max_amplitude <= 0.0 || f_max_amplitude >= 1.0 {
        f_max_amplitude = 0.1;
    }

    let mut render_alpha: f32 = 1.0; // just for initialization
    if new_alpha <= f_start_spring {
        render_alpha = new_alpha / f_start_spring;
    } else {
        let delta = ((1.0 - f_start_spring as f64) / (2.0 * i_mover_fluctuations as f64)) as f32;

        for k in 0..i_mover_fluctuations {
            // Oracle arm64 fuses both `2.0*k+1.0` and the outer
            // `fStartSpring + delta*(...)` into chained `fmadd`.
            let two_k_plus_1 = (k as f64).mul_add(2.0, 1.0);
            let current =
                (delta as f64).mul_add(two_k_plus_1, f_start_spring as f64) as f32;
            let previous = current - delta;
            let next = current + delta;
            let curr_ampl = f_max_amplitude / (1i32.wrapping_shl(k as u32) as f32);
            let coeff = curr_ampl / delta;

            if new_alpha > previous && new_alpha <= current {
                render_alpha = ((-coeff * new_alpha) as f64 + 1.0 - curr_ampl as f64
                    + (coeff * current) as f64) as f32;
                break;
            } else if new_alpha > current && new_alpha <= next {
                render_alpha = ((coeff * new_alpha) as f64 + 1.0 - curr_ampl as f64
                    - (coeff * current) as f64) as f32;
                break;
            }
        }
    }
    render_alpha
}

/// `AActor::physMovingBrush(DeltaTime)` (`Engine/Src/UnScript.cpp`). Only
/// `AMover` actors do anything.
///
/// # Safety
/// `this` must point to a live `AActor`. When it is-a `AMover`, `Region.Zone`
/// must be live whenever `bCollideWorld` is set.
pub unsafe fn phys_moving_brush(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    mut delta_time: f32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            if !is_a(svc, this as *mut c_void, CLASS_MOVER)? {
                return Ok(());
            }
            let mv = this as *mut AMover;
            let key_num = clamp_i((*mv).key_num as i32, 0, MOVER_KEY_ARRAY_COUNT) as usize;

            while (*this).b_interpolating() && delta_time > 0.0 {
                let mut fell = false;
                if (*this).b_collide_world()
                    && !(*this).region.zone.is_null()
                    && delta_time != 0.0
                {
                    // Apply gravity as well.
                    let zone = (*this).region.zone as *mut AZoneInfo;
                    let gravity = (*zone).zone_gravity;
                    let fall_vel = div(
                        scale(gravity, dot((*this).velocity, gravity)),
                        size_sq(gravity),
                    );
                    let fall = add(
                        scale(fall_vel, delta_time),
                        scale(gravity, delta_time * delta_time * 0.5),
                    );
                    (*this).velocity = add((*this).velocity, scale(gravity, delta_time));
                    let mut hit = blank_hit(1.0);
                    let old_loc = (*this).location;
                    svc.move_actor(
                        this as *mut c_void,
                        &fall,
                        &raw const (*this).rotation,
                        &mut hit,
                    )?;
                    if hit.time > 0.0 {
                        let moved = sub((*this).location, old_loc);
                        mover_key_pos_set(mv, key_num, add(mover_key_pos(mv, key_num), moved));
                        (*mv).old_pos = add((*mv).old_pos, moved);
                        fell = true;
                    } else {
                        svc.call_find_base(this)?;
                    }
                }

                // We are moving.
                // Oracle arm64 fuses this into `fmadd`.
                let mut new_alpha = delta_time.mul_add((*mv).phys_rate, (*mv).phys_alpha);
                if new_alpha > 1.0 {
                    if (*mv).phys_alpha < 1.0 {
                        delta_time *= (new_alpha - 1.0) / (new_alpha - (*mv).phys_alpha);
                    } else {
                        delta_time = 0.0;
                    }
                    new_alpha = 1.0;
                } else {
                    delta_time = 0.0;
                }

                // set correct MoverGlideType (just for last or first frame)
                let mut mover_glide_type = (*mv).mover_glide_type;
                if mover_glide_type == MV_SPRING_BY_TIME {
                    if ((*mv).prev_key_num as i32) < (*mv).key_num as i32
                        && (*mv).key_num as i32 != (*mv).num_keys as i32 - 1
                    {
                        mover_glide_type = MV_MOVE_BY_TIME;
                    }
                    if ((*mv).prev_key_num as i32) > (*mv).key_num as i32 && (*mv).key_num != 0 {
                        mover_glide_type = MV_MOVE_BY_TIME;
                    }
                }

                let render_alpha = if mover_glide_type == MV_GLIDE_BY_TIME {
                    // Make alpha time-smooth and time-continuous.
                    // f(0)=0, f(1)=1, f'(0)=f'(1)=0.
                    // Oracle arm64 computes `3.0*a*a` and `2.0*a*a*a` as
                    // plain muls, then fuses the final combination into
                    // `fmadd(3.0*a, a, -(2.0*a*a*a))`.
                    let three_a = 3.0f32 * new_alpha;
                    let neg_two_a3 = -(2.0f32 * new_alpha * new_alpha * new_alpha);
                    three_a.mul_add(new_alpha, neg_two_a3)
                } else if mover_glide_type == MV_SPRING_BY_TIME {
                    spring_render_alpha(&*mv, new_alpha)
                } else {
                    new_alpha
                };

                // Move.
                let mut hit = blank_hit(1.0);
                let delta = sub(
                    add(
                        (*mv).old_pos,
                        scale(
                            sub(add((*mv).base_pos, mover_key_pos(mv, key_num)), (*mv).old_pos),
                            render_alpha,
                        ),
                    ),
                    (*this).location,
                );
                let rot = rot_add(
                    (*mv).old_rot,
                    rot_mul(
                        rot_sub(rot_add((*mv).base_rot, mover_key_rot(mv, key_num)), (*mv).old_rot),
                        render_alpha,
                    ),
                );
                if svc.move_actor(this as *mut c_void, &delta, &rot, &mut hit)? != 0 {
                    // Moved some amount.
                    // Oracle arm64 fuses this into `fmadd`.
                    (*mv).phys_alpha =
                        hit.time.mul_add(new_alpha - (*mv).phys_alpha, (*mv).phys_alpha);
                }

                if !fell {
                    if hit.time < 1.0 {
                        // Hit something. Done now.
                        (*this).set_b_interpolating(false);
                        (*mv).hit_position = hit.location;
                        (*mv).hit_normal = hit.normal;
                        if mover_debug_enabled() {
                            svc.mover_freeze_log(
                                this as *mut c_void,
                                (*mv).phys_alpha,
                                (*mv).prev_key_num as i32,
                                (*mv).key_num as i32,
                                hit.actor,
                            )?;
                        }
                    } else if (*mv).phys_alpha >= 1.0 {
                        // Just finished moving.
                        (*this).set_b_interpolating(false);
                        svc.event_key_frame_reached(this as *mut c_void)?;
                    }
                }
            }
        }
    })
}

/// `AActor::performPhysics(DeltaSeconds)`.
///
/// # Safety
/// `this` must point to a live `AActor`. A non-null `pending_touch` must point
/// to a live `AActor` whose `pending_touch` link is valid.
pub unsafe fn perform_physics_actor(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta_time: f32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            // change position
            match (*this).physics {
                PHYS_PROJECTILE => svc.call_phys_projectile(this, delta_time, 0)?,
                PHYS_FALLING => svc.call_phys_falling(this, delta_time, 0)?,
                PHYS_ROTATING | PHYS_INTERPOLATING => {}
                PHYS_MOVING_BRUSH => {
                    (*this).old_location = (*this).location;
                    svc.call_phys_moving_brush(this, delta_time)?;
                    (*this).velocity = div(sub((*this).location, (*this).old_location), delta_time);
                }
                PHYS_TRAILER => svc.call_phys_trailer(this, delta_time)?,
                PHYS_ROLLING => svc.call_phys_rolling(this, delta_time, 0)?,
                _ => {}
            }

            // rotate
            if !rot_is_zero((*this).rotation_rate) {
                svc.call_physics_rotation_actor(this, delta_time)?;
            }

            // allow touched actors to impact physics
            if !(*this).pending_touch.is_null() {
                svc.event_post_touch((*this).pending_touch, this as *mut c_void)?;
                let old_touch = (*this).pending_touch as *mut AActor;
                (*this).pending_touch = (*old_touch).pending_touch;
                (*old_touch).pending_touch = null_mut();
            }
        }
    })
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
pub unsafe fn perform_physics_pawn(
    this: *mut APawn,
    svc: &Hp2PhysicServices,
    delta_time: f32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            let actor = addr_of_mut!((*this).base);
            let old_velocity = (*actor).velocity;

            // change position
            match (*actor).physics {
                PHYS_WALKING => svc.call_phys_walking(this, delta_time, 0)?,
                PHYS_FALLING => svc.call_phys_falling(actor, delta_time, 0)?,
                PHYS_FLYING => svc.call_phys_flying(this, delta_time, 0)?,
                PHYS_SWIMMING => svc.call_phys_swimming(this, delta_time, 0)?,
                PHYS_SPIDER => svc.call_phys_spider(this, delta_time, 0)?,
                PHYS_INTERPOLATING => {}
                PHYS_PROJECTILE => svc.call_phys_projectile(actor, delta_time, 0)?,
                PHYS_TRAILER => svc.call_phys_trailer(actor, delta_time)?,
                PHYS_ROLLING => svc.call_phys_rolling(actor, delta_time, 0)?,
                _ => {}
            }

            // check it out if he is WALKING up (or down) stairs
            step_up_down_stairs(this);

            // rotate
            let physics = (*actor).physics;
            if physics != PHYS_SPIDER
                && physics != PHYS_INTERPOLATING
                && (is_a(svc, actor as *mut c_void, CLASS_PLAYER_PAWN)?
                    || (!rot_eq((*actor).rotation, (*actor).desired_rotation)
                        && (*actor).b_rotate_to_desired())
                    || (!(*actor).b_rotate_to_desired() && (*actor).b_fixed_rotation_dir()))
            {
                svc.call_physics_rotation_pawn(this, delta_time, old_velocity)?;
            }

            (*this).move_timer -= delta_time;
            (*this).avg_physics_time = (*this).avg_physics_time.mul_add(0.8, 0.2 * delta_time);

            // allow touched actors to impact physics
            if !(*actor).pending_touch.is_null() {
                svc.event_post_touch((*actor).pending_touch, actor as *mut c_void)?;
                if !(*actor).pending_touch.is_null() {
                    let old_touch = (*actor).pending_touch as *mut AActor;
                    (*actor).pending_touch = (*old_touch).pending_touch;
                    (*old_touch).pending_touch = null_mut();
                }
            }
        }
    })
}

/// `AActor::physicsRotation(DeltaSeconds)`.
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn physics_rotation_actor(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta_time: f32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            if (!(*this).b_rotate_to_desired() && !(*this).b_fixed_rotation_dir())
                || ((*this).b_rotate_to_desired()
                    && rot_eq((*this).rotation, (*this).desired_rotation))
            {
                return Ok(());
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
                && (!(*this).b_rotate_to_desired()
                    || (*this).desired_rotation.pitch != new_rot.pitch)
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
                svc.move_actor(this as *mut c_void, &ZERO, &new_rot, &mut hit)?;
            }

            if (*this).b_rotate_to_desired()
                && rot_eq((*this).rotation, (*this).desired_rotation)
                && svc.is_probing_ended_rotation(this as *mut c_void)? != 0
            {
                svc.event_ended_rotation(this as *mut c_void)?; //tell thing rotation ended
            }
        }
    })
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
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            let actor = addr_of_mut!((*this).base);

            // Accumulate a desired new rotation.
            let mut new_rot = (*actor).rotation;

            if !(*actor).b_rotate_to_desired() && (*actor).b_fixed_rotation_dir() {
                svc.call_physics_rotation_actor(actor, delta_time)?;
                return Ok(());
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
                svc.move_actor(actor as *mut c_void, &ZERO, &new_rot, &mut hit)?;
            }
        }
    })
}
