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
/// `AActor::physFalling(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_falling(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_falling", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::fall::phys_falling(actor.cast::<AActor>(), svc, delta_time, iterations);
        }
    })
}

/// `AActor::FindBase()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_find_base(actor: *mut c_void, svc: *const c_void) {
    guard("hp2_rs_engine_physic_find_base", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::dispatch::find_base(actor.cast::<AActor>(), svc);
        }
    })
}

/// `AActor::setPhysics(NewPhysics, NewFloor)`. `new_floor` is null or an `AActor*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_set_physics(
    actor: *mut c_void,
    new_physics: u8,
    new_floor: *mut c_void,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_set_physics", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::dispatch::set_physics(
                actor.cast::<AActor>(),
                svc,
                new_physics,
                new_floor.cast::<AActor>(),
            );
        }
    })
}

/// `AActor::performPhysics(DeltaSeconds)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_perform_physics_actor(
    actor: *mut c_void,
    delta_time: f32,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_perform_physics_actor",
        (),
        move || {
            let Some(svc) = (unsafe { services(svc) }) else {
                return;
            };
            if actor.is_null() {
                return;
            }
            unsafe {
                physic::dispatch::perform_physics_actor(actor.cast::<AActor>(), svc, delta_time);
            }
        },
    )
}

/// `APawn::performPhysics(DeltaSeconds)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_perform_physics_pawn(
    actor: *mut c_void,
    delta_time: f32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_perform_physics_pawn", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::dispatch::perform_physics_pawn(actor.cast::<APawn>(), svc, delta_time);
        }
    })
}

/// `AActor::physicsRotation(DeltaSeconds)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_physics_rotation_actor(
    actor: *mut c_void,
    delta_time: f32,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_physics_rotation_actor",
        (),
        move || {
            let Some(svc) = (unsafe { services(svc) }) else {
                return;
            };
            if actor.is_null() {
                return;
            }
            unsafe {
                physic::dispatch::physics_rotation_actor(actor.cast::<AActor>(), svc, delta_time);
            }
        },
    )
}

/// `APawn::physicsRotation(DeltaSeconds, OldVelocity)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_physics_rotation_pawn(
    actor: *mut c_void,
    delta_time: f32,
    old_velocity: *const c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_physics_rotation_pawn",
        (),
        move || {
            let (Some(old_velocity), Some(svc)) =
                (unsafe { vec_arg(old_velocity) }, unsafe { services(svc) })
            else {
                return;
            };
            if actor.is_null() {
                return;
            }
            unsafe {
                physic::dispatch::physics_rotation_pawn(
                    actor.cast::<APawn>(),
                    svc,
                    delta_time,
                    old_velocity,
                );
            }
        },
    )
}

/// `APawn::startSwimming(OldVelocity, timeTick, remainingTime, Iterations)`.
/// `old_velocity` is an `FVector*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_start_swimming(
    actor: *mut c_void,
    old_velocity: *const c_void,
    time_tick: f32,
    remaining_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_start_swimming", (), move || {
        let (Some(old), Some(svc)) = (unsafe { vec_arg(old_velocity) }, unsafe { services(svc) })
        else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::swim::start_swimming(
                actor.cast::<APawn>(),
                svc,
                old,
                time_tick,
                remaining_time,
                iterations,
            );
        }
    })
}

/// `APawn::physFlying(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_flying(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_flying", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::swim::phys_flying(actor.cast::<APawn>(), svc, delta_time, iterations);
        }
    })
}

/// `APawn::physSwimming(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_swimming(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_swimming", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::swim::phys_swimming(actor.cast::<APawn>(), svc, delta_time, iterations);
        }
    })
}

/// `APawn::Swim(Delta, Hit)`. `delta` is an `FVector*`, `hit` an `FCheckResult*`.
/// Returns the air fraction; 0 when an argument is null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_swim(
    actor: *mut c_void,
    delta: *const c_void,
    hit: *mut c_void,
    svc: *const c_void,
) -> f32 {
    guard("hp2_rs_engine_physic_swim", 0.0, move || {
        let (Some(delta), Some(svc)) = (unsafe { vec_arg(delta) }, unsafe { services(svc) }) else {
            return 0.0;
        };
        if actor.is_null() || hit.is_null() {
            return 0.0;
        }
        unsafe { physic::swim::swim(actor.cast::<APawn>(), svc, delta, hit.cast()) }
    })
}

/// `APawn::findWaterLine(Start, End)`. `start` is an `FVector*`; `end` is an
/// in/out `FVector*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_find_water_line(
    actor: *mut c_void,
    start: *const c_void,
    end: *mut c_void,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_find_water_line", (), move || {
        let (Some(start), Some(svc)) = (unsafe { vec_arg(start) }, unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() || end.is_null() {
            return;
        }
        unsafe {
            physic::swim::find_water_line(actor.cast::<APawn>(), svc, start, end.cast());
        }
    })
}
/// `APawn::physWalking(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_walking(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_walking", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe { physic::walk::phys_walking(actor.cast::<APawn>(), svc, delta_time, iterations) }
    })
}
/// `AActor::physProjectile(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_projectile(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_projectile", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe {
            physic::motion::phys_projectile(actor.cast::<AActor>(), svc, delta_time, iterations)
        }
    })
}

/// `AActor::physRolling(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_rolling(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_rolling", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe { physic::motion::phys_rolling(actor.cast::<AActor>(), svc, delta_time, iterations) }
    })
}

/// `APawn::physSpider(deltaTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_spider(
    actor: *mut c_void,
    delta_time: f32,
    iterations: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_spider", (), move || {
        let Some(svc) = (unsafe { services(svc) }) else {
            return;
        };
        if actor.is_null() {
            return;
        }
        unsafe { physic::motion::phys_spider(actor.cast::<APawn>(), svc, delta_time, iterations) }
    })
}

/// `APawn::checkFloor(Dir, Hit)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_check_floor(
    actor: *mut c_void,
    dir: *const c_void,
    hit: *mut c_void,
    svc: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_physic_check_floor", 0, move || {
        let (Some(dir), Some(svc)) = (unsafe { vec_arg(dir) }, unsafe { services(svc) }) else {
            return 0;
        };
        if actor.is_null() || hit.is_null() {
            return 0;
        }
        unsafe { physic::motion::check_floor(actor.cast::<APawn>(), svc, dir, hit.cast()) }
    })
}

/// `APawn::findNewFloor(OldLocation, deltaTime, remainingTime, Iterations)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_find_new_floor(
    actor: *mut c_void,
    old_location: *const c_void,
    delta_time: f32,
    remaining_time: f32,
    iterations: i32,
    svc: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_physic_find_new_floor", 0, move || {
        let (Some(old_location), Some(svc)) =
            (unsafe { vec_arg(old_location) }, unsafe { services(svc) })
        else {
            return 0;
        };
        if actor.is_null() {
            return 0;
        }
        unsafe {
            physic::motion::find_new_floor(
                actor.cast::<APawn>(),
                svc,
                old_location,
                delta_time,
                remaining_time,
                iterations,
            )
        }
    })
}
/// `AInterpolationPoint::GetDesiredRotationAtPosition`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_get_desired_rotation_at_position(
    point: *mut c_void,
    pos_offset: i32,
    pause_num: i32,
    new_location: *const c_void,
    out: *mut c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_get_desired_rotation_at_position",
        (),
        move || {
            let (Some(location), Some(svc)) =
                (unsafe { vec_arg(new_location) }, unsafe { services(svc) })
            else {
                return;
            };
            if point.is_null() || out.is_null() {
                return;
            }
            let rotation = unsafe {
                physic::interp::get_desired_rotation_at_position(
                    point.cast(),
                    svc,
                    pos_offset,
                    pause_num,
                    location,
                )
            };
            unsafe {
                out.cast::<hp2_abi::engine::generated::FRotator>()
                    .write_unaligned(rotation)
            };
        },
    )
}

/// `AInterpolationPoint::GetDesiredRotationAtPause`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_get_desired_rotation_at_pause(
    point: *mut c_void,
    pause_num: i32,
    new_location: *const c_void,
    out: *mut c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_get_desired_rotation_at_pause",
        (),
        move || {
            let (Some(location), Some(svc)) =
                (unsafe { vec_arg(new_location) }, unsafe { services(svc) })
            else {
                return;
            };
            if point.is_null() || out.is_null() {
                return;
            }
            let rotation = unsafe {
                physic::interp::get_desired_rotation_at_pause(
                    point.cast(),
                    svc,
                    pause_num,
                    location,
                )
            };
            unsafe {
                out.cast::<hp2_abi::engine::generated::FRotator>()
                    .write_unaligned(rotation)
            };
        },
    )
}

/// `AInterpolationPoint::GetInterpolatedPosition`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_get_interpolated_position(
    point: *mut c_void,
    old_coords: *const c_void,
    start_control: *const c_void,
    alpha: f32,
    pause_num: i32,
    out: *mut c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_get_interpolated_position",
        (),
        move || {
            let (Some(svc), Some(old), Some(start)) = (
                unsafe { services(svc) },
                unsafe { old_coords.cast::<hp2_abi::FCoords>().as_ref().copied() },
                unsafe { vec_arg(start_control) },
            ) else {
                return;
            };
            if point.is_null() || out.is_null() {
                return;
            }
            let coords = unsafe {
                physic::interp::get_interpolated_position(
                    point.cast(),
                    svc,
                    old,
                    start,
                    alpha,
                    pause_num,
                )
            };
            unsafe { out.cast::<hp2_abi::FCoords>().write_unaligned(coords) };
        },
    )
}

/// `AInterpolationManager::performPhysics(delta_time)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_interpolation_perform_physics(
    manager: *mut c_void,
    delta_time: f32,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_physic_interpolation_perform_physics",
        (),
        move || {
            let Some(svc) = (unsafe { services(svc) }) else {
                return;
            };
            if manager.is_null() {
                return;
            }
            unsafe { physic::interp::perform_physics(manager.cast(), svc, delta_time) }
        },
    )
}

/// `AActor::physTrailer(deltaTime)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_physic_phys_trailer(
    actor: *mut c_void,
    delta_time: f32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_physic_phys_trailer", (), move || unsafe {
        let (Some(actor), Some(svc)) = (actor.cast::<AActor>().as_mut(), services(svc)) else {
            return;
        };
        physic::dispatch::phys_trailer(actor, svc, delta_time);
    })
}
