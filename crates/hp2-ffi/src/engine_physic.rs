//! Engine exports from `crates/hp2-engine/src/physic.rs` (`UnPhysic.cpp`).

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn};
use hp2_engine::physic::{self, Hp2PhysicServices};
use hp2_ffi_support::guard;

/// `AActor::fixedTurn`. `fixed_dir` is `bFixedRotationDir`, `rotate_to_desired`
/// is `bRotateToDesired` (nonzero = set).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_fixed_turn(
    current: i32,
    desired: i32,
    delta_rate: i32,
    fixed_dir: i32,
    rotate_to_desired: i32,
) -> i32 {
    guard("hp2_rs_engine_physic_fixed_turn", 0, move || {
        physic::fixed_turn(
            current,
            desired,
            delta_rate,
            fixed_dir != 0,
            rotate_to_desired != 0,
        )
    })
}

/// Borrows the `Hp2PhysicServices` table the C++ shell passes; `None` if null.
///
/// # Safety
/// A non-null `svc` must point at a live `Hp2PhysicServices`.
unsafe fn services<'a>(svc: *const c_void) -> Option<&'a Hp2PhysicServices> {
    unsafe { svc.cast::<Hp2PhysicServices>().as_ref() }
}

/// Reads an `FVector` argument; `None` if null.
///
/// # Safety
/// A non-null `v` must point at a live `FVector`.
unsafe fn vec_arg(v: *const c_void) -> Option<FVector> {
    unsafe { v.cast::<FVector>().as_ref().copied() }
}

/// `APawn::calcVelocity(AccelDir, deltaTime, maxSpeed, friction, bFluid, bBrake,
/// bBuoyant)`. `actor` is the `APawn*`, `accel_dir` an `FVector*`. Flags are C
/// ints (nonzero = set).
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_physic_calc_velocity(
    actor: *mut c_void,
    accel_dir: *const c_void,
    delta_time: f32,
    max_speed: f32,
    friction: f32,
    b_fluid: i32,
    b_brake: i32,
    b_buoyant: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_calc_velocity", (), move || {
        let (Some(dir), Some(svc)) = (unsafe { vec_arg(accel_dir) }, unsafe { services(svc) })
        else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::calc_velocity(
                actor.cast::<APawn>(),
                svc,
                dir,
                delta_time,
                max_speed,
                friction,
                b_fluid,
                b_brake,
                b_buoyant,
            );
        }
    })
}

/// `APawn::Mount(MountDir, Hit)`. Returns nonzero when the mount is taken.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_mount(
    actor: *mut c_void,
    mount_dir: *const c_void,
    hit: *mut c_void,
    svc: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_physic_mount", 0, move || {
        let (Some(dir), Some(svc)) = (unsafe { vec_arg(mount_dir) }, unsafe { services(svc) })
        else {
            return 0;
        };
        if actor.is_null() || hit.is_null() {
            return 0;
        }
        i32::from(unsafe { physic::mount(actor.cast::<APawn>(), svc, dir, hit.cast()) })
    })
}

/// `APawn::stepUp(GravDir, DesiredDir, Delta, Hit)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_step_up(
    actor: *mut c_void,
    grav_dir: *const c_void,
    desired_dir: *const c_void,
    delta: *const c_void,
    hit: *mut c_void,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_step_up", (), move || {
        let (Some(grav), Some(desired), Some(step), Some(svc)) = (
            unsafe { vec_arg(grav_dir) },
            unsafe { vec_arg(desired_dir) },
            unsafe { vec_arg(delta) },
            unsafe { services(svc) },
        ) else {
            return;
        };
        if actor.is_null() || hit.is_null() {
            return;
        }
        unsafe {
            physic::step_up(actor.cast::<APawn>(), svc, grav, desired, step, hit.cast());
        }
    })
}

/// `AActor::processHitWall(HitNormal, HitActor)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_process_hit_wall(
    actor: *mut c_void,
    hit_normal: *const c_void,
    hit_actor: *mut c_void,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_process_hit_wall", (), move || {
        let (Some(normal), Some(svc)) = (unsafe { vec_arg(hit_normal) }, unsafe { services(svc) })
        else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::process_hit_wall(actor.cast::<AActor>(), svc, normal, hit_actor);
        }
    })
}

/// `AActor::processLanded(HitNormal, HitActor, remainingTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_process_landed(
    actor: *mut c_void,
    hit_normal: *const c_void,
    hit_actor: *mut c_void,
    remaining_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_process_landed", (), move || {
        let (Some(normal), Some(svc)) = (unsafe { vec_arg(hit_normal) }, unsafe { services(svc) })
        else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::process_landed(
                actor.cast::<AActor>(),
                svc,
                normal,
                hit_actor,
                remaining_time,
                iterations,
            );
        }
    })
}
