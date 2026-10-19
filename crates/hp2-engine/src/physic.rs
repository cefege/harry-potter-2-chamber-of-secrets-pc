//! Port of `Engine/Src/UnPhysic.cpp`: movement physics. Ported so far:
//! `AActor::fixedTurn`, `APawn::calcVelocity`, `APawn::Mount`,
//! `APawn::stepUp`, `AActor::processHitWall`, `AActor::processLanded`.
//! Field access goes through the generated `hp2_abi` mirrors. Engine calls go
//! through `Hp2PhysicServices` (filled by `Hp2PhysicServices.cpp`). The
//! bitfield flags for `fixedTurn` are passed in by the C++ shell.
pub mod dispatch;
pub mod fall;
pub mod interp;
pub mod motion;
pub mod swim;
pub mod walk;

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, ACarcass, ADecoration, APawn, AZoneInfo, FCheckResult, FRotator,
};

/// Class ids for `Hp2PhysicServices::is_a` (`Hp2PhysicClass` in C++).
const CLASS_PAWN: i32 = 0;
const CLASS_PLAYER_PAWN: i32 = 1;
const CLASS_DECORATION: i32 = 2;
const CLASS_CARCASS: i32 = 3;

/// Trace kinds for `Hp2PhysicServices::single_line_check` (`Hp2PhysicTrace`).
const TRACE_VIS_BLOCKING: i32 = 0;
const TRACE_ALL_BLOCKING: i32 = 1;

/// `EPhysics` values (`EngineClasses.h`).
const PHYS_NONE: u8 = 0;
const PHYS_WALKING: u8 = 1;
const PHYS_FALLING: u8 = 2;

/// `SMALL_NUMBER` from `UnMath.h`.
const SMALL_NUMBER: f32 = 1.0e-8;

/// Engine calls the physics bodies make. Field order matches
/// `Hp2PhysicServices` in `Hp2PhysicServices.h`.
#[repr(C)]
pub struct Hp2PhysicServices {
    /// `UObject::IsA(X::StaticClass())`; `class` is a `CLASS_*` id.
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, class: i32) -> i32,
    /// `IsProbing(NAME_HitWall)`.
    pub is_probing_hit_wall: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `appFrand()`.
    pub app_frand: unsafe extern "C" fn() -> f32,
    /// `GetCylinderExtent()`.
    pub cylinder_extent: unsafe extern "C" fn(actor: *mut c_void, out: *mut FVector),
    /// Non-null `Hit.BspSurf(radius)` with `PF_SpecialPoly` set.
    pub bsp_surf_special: unsafe extern "C" fn(hit: *const FCheckResult, radius: f32) -> i32,
    /// `FRotator::Vector()`.
    pub rotation_vector: unsafe extern "C" fn(rot: *const FRotator, out: *mut FVector),
    /// `GetLevel()->MoveActor(actor, delta, rot, hit)` with default flags.
    pub move_actor: unsafe extern "C" fn(
        actor: *mut c_void,
        delta: *const FVector,
        rot: *const FRotator,
        hit: *mut FCheckResult,
    ) -> i32,
    /// `GetLevel()->SingleLineCheck(hit, actor, end, start, trace, extent)`.
    pub single_line_check: unsafe extern "C" fn(
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
    ) -> i32,
    /// `SetBase(base)` with the default notify flag.
    pub set_base: unsafe extern "C" fn(actor: *mut c_void, base: *mut c_void),
    /// `APawn::eventMount(dir)`.
    pub event_mount: unsafe extern "C" fn(actor: *mut c_void, dir: *const FVector),
    /// `AActor::eventHitWall(normal, hit_actor)`.
    pub event_hit_wall:
        unsafe extern "C" fn(actor: *mut c_void, normal: *const FVector, hit_actor: *mut c_void),
    /// `AActor::eventLanded(normal)`.
    pub event_landed: unsafe extern "C" fn(actor: *mut c_void, normal: *const FVector),
    /// `GetLevel()->Model->PointRegion(Level, *location).Zone` (`AActor::findWaterLine`).
    pub point_region_zone:
        unsafe extern "C" fn(actor: *mut c_void, location: *const FVector) -> *mut c_void,
    /// `debugf(TEXT("%s flew out of the world!"), GetName())`.
    pub debugf_flew_out: unsafe extern "C" fn(actor: *mut c_void),
    /// `GetLevel()->DestroyActor(actor)`.
    pub destroy_actor: unsafe extern "C" fn(actor: *mut c_void),
    /// `AActor::eventFalling()`.
    pub event_falling: unsafe extern "C" fn(actor: *mut c_void),
    /// `AProjectile::MaxSpeed` read through the class (`physProjectile`).
    pub projectile_max_speed: unsafe extern "C" fn(actor: *mut c_void) -> f32,
    /// `debugf(TEXT("%s fell out of the world!"), GetName())`.
    pub log_fell_out_of_world: unsafe extern "C" fn(actor: *mut c_void),
    /// `AActor::eventFellOutOfWorld()`.
    pub event_fell_out_of_world: unsafe extern "C" fn(actor: *mut c_void),
    /// `FVector::Rotation()`.
    pub rotation_from_vector: unsafe extern "C" fn(vector: *const FVector, out: *mut FRotator),
    /// First live actor in the level whose `Tag` matches the requested tag.
    pub find_view_target:
        unsafe extern "C" fn(actor: *mut c_void, tag: *const [u8; 4]) -> *mut c_void,
    /// `AInterpolationManager::eventUpdateCamera(old_alpha)`.
    pub event_update_camera: unsafe extern "C" fn(manager: *mut c_void, old_alpha: f32),
    /// `AInterpolationPoint::eventInterpolateEnd(manager, forward)`.
    pub event_interpolate_end:
        unsafe extern "C" fn(point: *mut c_void, manager: *mut c_void, forward: i32),
    /// `AInterpolationManager::eventFinishedInterpolation(NULL)`.
    pub event_finished_interpolation: unsafe extern "C" fn(manager: *mut c_void),
    /// `AActor::physTrailer(delta_time)`.
    pub phys_trailer: unsafe extern "C" fn(actor: *mut c_void, delta_time: f32),
    /// `AActor::physMovingBrush(delta_time)`.
    pub phys_moving_brush: unsafe extern "C" fn(actor: *mut c_void, delta_time: f32),
    /// `PendingTouch->eventPostTouch(this)`; `touched` is the receiver.
    pub event_post_touch: unsafe extern "C" fn(touched: *mut c_void, toucher: *mut c_void),
    /// `IsProbing(NAME_EndedRotation)`.
    pub is_probing_ended_rotation: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `AActor::eventEndedRotation()`.
    pub event_ended_rotation: unsafe extern "C" fn(actor: *mut c_void),
    /// `GMath.SinTab(i)`; the C++ side applies the table masking.
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GMath.CosTab(i)`.
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `IsProbing(NAME_MayFall)`.
    pub is_probing_may_fall: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `APawn::eventMayFall()`.
    pub event_may_fall: unsafe extern "C" fn(actor: *mut c_void),
    /// `APawn::eventDoJump(1.f)`.
    pub event_do_jump: unsafe extern "C" fn(actor: *mut c_void, value: f32),
    /// `GetLevel()->FarMoveActor(actor, old_location, 0, 0)`.
    pub far_move_actor: unsafe extern "C" fn(actor: *mut c_void, location: *const FVector),
    /// `debugf(TEXT("%s fell out of the world!"), GetName())`.
    pub debugf_fell_out: unsafe extern "C" fn(actor: *mut c_void),
    /// `GetLevel()->SingleLineCheck(..., NULL, ...)`; owner only locates level.
    pub single_line_check_no_source: unsafe extern "C" fn(
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
    ) -> i32,

    /// Snapshot fields needed by the original `AActor::physTrailer` body.
    pub trailer_state: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    /// `UMesh::GetBoneCoords(owner, bone)` for trailer attachment points.
    pub mesh_bone_coords:
        unsafe extern "C" fn(mesh: *mut c_void, owner: *mut c_void, bone: i32, out: *mut c_void),
    /// `FarMoveActor(actor, location, 0, 1)` used by physTrailer.
    pub trailer_far_move_actor: unsafe extern "C" fn(actor: *mut c_void, location: *const FVector),
}
/// Plain snapshot returned by the C++ adapter for `AActor::physTrailer`.
#[repr(C)]
struct TrailerState {
    owner: *mut c_void,
    mesh: *mut c_void,
    pre_pivot: [f32; 3],
    mass: f32,
    draw_type: i32,
    anim_bone: i32,
    flags: u32,
}

/// `FVector(x, y, z)`.
const fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

const ZERO: FVector = v3(0.0, 0.0, 0.0);

/// `A + B`.
fn add(a: FVector, b: FVector) -> FVector {
    v3(a.x + b.x, a.y + b.y, a.z + b.z)
}

/// `A - B`.
fn sub(a: FVector, b: FVector) -> FVector {
    v3(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// `A * S` (`FVector::operator*(FLOAT)`).
fn scale(a: FVector, s: f32) -> FVector {
    v3(a.x * s, a.y * s, a.z * s)
}

/// `A / S` (`FVector::operator/(FLOAT)` multiplies by `1.f/S`).
fn div(a: FVector, s: f32) -> FVector {
    scale(a, 1.0 / s)
}

/// `-A`.
fn neg(a: FVector) -> FVector {
    v3(-a.x, -a.y, -a.z)
}

/// `A | B`.
fn dot(a: FVector, b: FVector) -> f32 {
    // `-ffp-contract=on`: clang fuses the second product into the first FMA.
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `A ^ B` (`UnMath.h` cross product).
fn cross(a: FVector, b: FVector) -> FVector {
    // `-ffp-contract=on`: each `P*Q - R*S` fuses the left product into an FMA.
    v3(
        a.y.mul_add(b.z, -(a.z * b.y)),
        a.z.mul_add(b.x, -(a.x * b.z)),
        a.x.mul_add(b.y, -(a.y * b.x)),
    )
}

/// `SizeSquared()`.
fn size_sq(a: FVector) -> f32 {
    a.z.mul_add(a.z, a.x.mul_add(a.x, a.y * a.y))
}

/// `appSqrt(DOUBLE)` from `UnFile.h`.
fn app_sqrt(v: f64) -> f64 {
    v.sqrt()
}

/// `Size()`: `appSqrt` takes and returns `DOUBLE`, narrowed to `FLOAT`.
fn size(a: FVector) -> f32 {
    app_sqrt(size_sq(a) as f64) as f32
}

/// `IsZero()`.
fn is_zero(a: FVector) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

/// `SafeNormal()` (`UnMath.cpp`): zero below `SMALL_NUMBER`.
fn safe_normal(a: FVector) -> FVector {
    let sq = size_sq(a);
    if sq < SMALL_NUMBER {
        return ZERO;
    }
    let len = app_sqrt(sq as f64) as f32;
    scale(a, 1.0 / len)
}

/// `Normalize()` (`UnMath.h`): in place, unchanged below `SMALL_NUMBER`.
fn normalize(a: &mut FVector) {
    let sq = size_sq(*a);
    if sq >= SMALL_NUMBER {
        let s = (1.0 / app_sqrt(sq as f64)) as f32;
        *a = scale(*a, s);
    }
}

/// `Abs(A)` from `UnTemplate.h`: `A >= 0 ? A : -A`.
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `Max(A, B)` from `UnTemplate.h`.
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `Clamp(X, Min, Max)` from `UnTemplate.h`.
fn clamp_f(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// `FVector::MaxVal()`.
fn max_val(a: FVector) -> f32 {
    max_f(max_f(abs_f(a.x), abs_f(a.y)), abs_f(a.z))
}

/// `FCheckResult Hit(time)`. Only the fields the physics bodies read are set;
/// the rest is zero.
fn blank_hit(time: f32) -> FCheckResult {
    // SAFETY: FCheckResult is plain old data (pointers, floats, ints); a zero
    // bit pattern is a valid value and every field the bodies read is set here
    // or written by the engine call that fills it.
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = time;
    hit.item = -1;
    hit
}

/// `Obj->IsA(class)` through the services table.
unsafe fn is_a(svc: &Hp2PhysicServices, obj: *mut c_void, class: i32) -> bool {
    unsafe { (svc.is_a)(obj, class) != 0 }
}

/// `GetLevel()->MoveActor(this, delta, Rotation, hit)`.
unsafe fn move_actor(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    delta: FVector,
    hit: *mut FCheckResult,
) -> i32 {
    unsafe { (svc.move_actor)(pawn as *mut c_void, &delta, &(*pawn).base.rotation, hit) }
}

/// `APawn::calcVelocity(AccelDir, deltaTime, maxSpeed, friction, bFluid, bBrake, bBuoyant)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`. When `b_buoyant` is nonzero,
/// `Region.Zone` must point to a live `AZoneInfo` (the C++ dereferences it).
#[allow(clippy::too_many_arguments)]
pub unsafe fn calc_velocity(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    accel_dir: FVector,
    delta_time: f32,
    mut max_speed: f32,
    friction: f32,
    b_fluid: i32,
    b_brake: i32,
    b_buoyant: i32,
) {
    unsafe {
        let this = pawn as *mut c_void;
        let mut velocity = (*pawn).base.velocity;
        let mut acceleration = (*pawn).base.acceleration;
        let accel_rate = (*pawn).accel_rate;
        let effective_friction = max_f(b_fluid as f32, friction);
        let walking_player = is_a(svc, this, CLASS_PLAYER_PAWN) && (*pawn).b_is_walking();

        if b_brake != 0 && is_zero(acceleration) {
            let old_vel = velocity;
            let mut sum_vel = ZERO;
            let mut remaining_time = delta_time;
            // Subdivide braking to keep results consistent at low frame rates.
            while remaining_time > 0.03 {
                let brake = scale(scale(scale(velocity, 2.0), 0.03), effective_friction);
                velocity = sub(velocity, brake);
                if dot(velocity, old_vel) > 0.0 {
                    sum_vel = add(sum_vel, div(scale(velocity, 0.03), delta_time));
                }
                remaining_time -= 0.03;
            }
            let brake = scale(
                scale(scale(velocity, 2.0), remaining_time),
                effective_friction,
            );
            velocity = sub(velocity, brake);
            if dot(velocity, old_vel) > 0.0 {
                sum_vel = add(sum_vel, div(scale(velocity, remaining_time), delta_time));
            }
            velocity = sum_vel;
            // Brake to a stop, not backwards.
            if dot(old_vel, velocity) < 0.0 || size_sq(velocity) < 100.0 {
                velocity = ZERO;
            }
        } else {
            let vel_size = size(velocity);
            if walking_player {
                if size_sq(acceleration) > 0.09 * accel_rate * accel_rate {
                    acceleration = scale(scale(accel_dir, accel_rate), 0.3);
                }
            } else if size_sq(acceleration) > accel_rate * accel_rate {
                acceleration = scale(accel_dir, accel_rate);
            }
            let drift = sub(velocity, scale(accel_dir, vel_size));
            velocity = sub(
                velocity,
                scale(scale(drift, delta_time), effective_friction),
            );
        }

        let damp = (-(b_fluid as f32 * friction)).mul_add(delta_time, 1.0);
        velocity = add(scale(velocity, damp), scale(acceleration, delta_time));

        if !is_a(svc, this, CLASS_PLAYER_PAWN) {
            max_speed *= (*pawn).desired_speed;
        }

        if b_buoyant != 0 {
            let zone = (*pawn).base.region.zone as *const AZoneInfo;
            let buoyancy_factor = 1.0 - (*pawn).base.buoyancy / (*pawn).base.mass;
            let lift = scale((*zone).zone_gravity, delta_time);
            velocity = add(velocity, scale(lift, buoyancy_factor));
        }

        if walking_player && size_sq(velocity) > 0.09 * max_speed * max_speed {
            let speed = size(velocity);
            velocity = div(velocity, speed);
            let cap = max_f(
                0.3 * max_speed,
                speed * (-(delta_time * 2.0)).mul_add(effective_friction, 1.0),
            );
            velocity = scale(velocity, cap);
        } else if size_sq(velocity) > max_speed * max_speed {
            velocity = safe_normal(velocity);
            velocity = scale(velocity, max_speed);
        }

        (*pawn).base.velocity = velocity;
        (*pawn).base.acceleration = acceleration;
    }
}

/// `APawn::Mount(MountDir, Hit)`. Returns true when the mount is taken.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `hit` must point to a live `FCheckResult`.
pub unsafe fn mount(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    mount_dir: FVector,
    hit: *mut FCheckResult,
) -> bool {
    unsafe {
        let this = pawn as *mut c_void;
        let max_mount = (*pawn).max_mount_height;
        let hit_normal = (*hit).normal;
        if max_mount > 0.0 && (hit_normal.z as f64) > -0.1 && (hit_normal.z as f64) < 0.7 {
            let mut rotation_vector = ZERO;
            (svc.rotation_vector)(&(*pawn).base.rotation, &mut rotation_vector);
            if dot(hit_normal, rotation_vector) < 0.0 {
                let mut extent = ZERO;
                (svc.cylinder_extent)(this, &mut extent);
                if (svc.bsp_surf_special)(hit, max_val(extent)) != 0 {
                    let mut move_dir = neg(hit_normal);
                    move_dir.z = 0.0;
                    normalize(&mut move_dir);

                    let loc = (*pawn).base.location;
                    let cr = (*pawn).base.collision_radius;
                    let mut loc1 = add(loc, scale(mount_dir, max_mount));
                    let loc2 = add(loc1, scale(move_dir, 2.0 * cr + hit_normal.z * max_mount));
                    let loc3 = sub(
                        sub(loc2, scale(mount_dir, max_mount)),
                        scale(move_dir, hit_normal.z * max_mount),
                    );

                    let mut hit2 = blank_hit(0.0);
                    if (svc.single_line_check)(
                        this,
                        &mut hit2,
                        &loc3,
                        &loc2,
                        TRACE_VIS_BLOCKING,
                        &extent,
                    ) == 0
                    {
                        let threshold = if (*pawn).base.physics == PHYS_FALLING {
                            0.0
                        } else {
                            (*pawn).max_step_height
                        };
                        if hit2.location.z - loc.z >= threshold {
                            let mut mount_loc = hit2.location;
                            mount_loc.z += 2.0;
                            loc1.z = mount_loc.z;
                            if (svc.single_line_check)(
                                this,
                                &mut hit2,
                                &loc1,
                                &loc,
                                TRACE_VIS_BLOCKING,
                                &extent,
                            ) != 0
                                && (svc.single_line_check)(
                                    this,
                                    &mut hit2,
                                    &mount_loc,
                                    &loc1,
                                    TRACE_VIS_BLOCKING,
                                    &extent,
                                ) != 0
                            {
                                (svc.set_base)(this, (*hit).actor);
                                (svc.event_mount)(this, &sub(mount_loc, loc));
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

/// `TwoWallAdjust(DesiredDir, Delta, HitNormal, OldHitNormal, HitTime)`
/// (`AActor.h`).
pub(crate) fn two_wall_adjust(
    desired_dir: FVector,
    delta: &mut FVector,
    hit_normal: FVector,
    old_hit_normal: FVector,
    hit_time: f32,
) {
    if dot(old_hit_normal, hit_normal) <= 0.0 {
        let new_dir = safe_normal(cross(hit_normal, old_hit_normal));
        let along = dot(*delta, new_dir) * (1.0 - hit_time);
        *delta = scale(new_dir, along);
        if dot(desired_dir, *delta) < 0.0 {
            *delta = scale(*delta, -1.0);
        }
    } else {
        let slide = sub(*delta, scale(hit_normal, dot(*delta, hit_normal)));
        *delta = scale(slide, 1.0 - hit_time);
        if dot(*delta, desired_dir) <= 0.0 {
            *delta = ZERO;
        }
    }
}

/// `APawn::stepUp(GravDir, DesiredDir, Delta, Hit)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `hit` must point to a live `FCheckResult`
/// whose `actor` is a live object whenever `time < 1` (the C++ dereferences it).
pub unsafe fn step_up(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    grav_dir: FVector,
    desired_dir: FVector,
    mut delta: FVector,
    hit: *mut FCheckResult,
) {
    unsafe {
        if mount(pawn, svc, neg(grav_dir), hit) {
            return;
        }
        let this = pawn as *mut c_void;
        let actor = pawn as *mut AActor;
        let down = scale(grav_dir, (*pawn).max_step_height);
        let up = scale(down, -1.0);
        move_actor(pawn, svc, up, hit);
        move_actor(pawn, svc, delta, hit);

        if (*hit).time < 1.0 {
            let hit_actor = (*hit).actor;
            let hit_normal = (*hit).normal;
            if is_a(svc, this, CLASS_PLAYER_PAWN)
                && is_a(svc, hit_actor, CLASS_DECORATION)
                && (*(hit_actor as *mut ADecoration)).b_pushable()
                && (dot(hit_normal, desired_dir) as f64) < -0.9
            {
                (*pawn).base.set_b_just_teleported(true);
                let mass = (*pawn).base.mass;
                let hit_mass = (*(hit_actor as *mut AActor)).mass;
                (*pawn).base.velocity = scale((*pawn).base.velocity, mass / (mass + hit_mass));
                process_hit_wall(actor, svc, hit_normal, hit_actor);
                if (*pawn).base.physics == PHYS_FALLING {
                    return;
                }
            } else if abs_f(hit_normal.z) < 0.2 && (*hit).time * size_sq(delta) > 144.0 {
                step_up(
                    pawn,
                    svc,
                    grav_dir,
                    desired_dir,
                    scale(delta, 1.0 - (*hit).time),
                    hit,
                );
                if (*pawn).base.physics == PHYS_FALLING {
                    return;
                }
            } else {
                process_hit_wall(actor, svc, hit_normal, hit_actor);
                let original_delta = delta;
                let old_hit_normal = hit_normal;
                let slide = sub(delta, scale(hit_normal, dot(delta, hit_normal)));
                delta = scale(slide, 1.0 - (*hit).time);
                if dot(delta, original_delta) >= 0.0 {
                    move_actor(pawn, svc, delta, hit);
                    if (*hit).time < 1.0 {
                        process_hit_wall(actor, svc, (*hit).normal, (*hit).actor);
                        if (*pawn).base.physics == PHYS_FALLING {
                            return;
                        }
                        two_wall_adjust(
                            desired_dir,
                            &mut delta,
                            (*hit).normal,
                            old_hit_normal,
                            (*hit).time,
                        );
                        move_actor(pawn, svc, delta, hit);
                    }
                }
            }
        }

        move_actor(pawn, svc, down, hit);
        if (*hit).time < 1.0 && (*hit).normal.z < 0.5 {
            let hit_normal = (*hit).normal;
            let slide = sub(down, scale(hit_normal, dot(down, hit_normal)));
            delta = scale(slide, 1.0 - (*hit).time);
            if dot(delta, down) >= 0.0 {
                move_actor(pawn, svc, delta, hit);
            }
        }
    }
}

/// `AActor::processHitWall(HitNormal, HitActor)`.
///
/// # Safety
/// `this` must point to a live `AActor`; `hit_actor` must be a live object.
pub unsafe fn process_hit_wall(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    mut hit_normal: FVector,
    hit_actor: *mut c_void,
) {
    unsafe {
        let this_v = this as *mut c_void;
        if is_a(svc, hit_actor, CLASS_PAWN) {
            return;
        }
        if is_a(svc, this_v, CLASS_PAWN) {
            let pawn = this as *mut APawn;
            if is_zero((*this).acceleration) {
                return;
            }
            let mut dir = safe_normal(sub((*pawn).destination, (*this).location));
            if (*this).physics == PHYS_WALKING {
                hit_normal.z = 0.0;
                dir.z = 0.0;
            }
            if (*pawn).min_hit_wall < dot(dir, hit_normal) {
                return;
            }
            if (svc.is_probing_hit_wall)(this_v) == 0 && (*this).physics != PHYS_FALLING {
                (*pawn).set_b_from_wall(true);
                return;
            }
        } else if (svc.is_probing_hit_wall)(this_v) == 0 {
            return;
        }
        (svc.event_hit_wall)(this_v, &hit_normal, hit_actor);
    }
}

/// `(Delta - Normal * D) * Scale` with `D = Delta|Normal`, the `moveSmooth`
/// slide. Not contracted: the original emits `fmul` then `fsub` (verified by
/// objdump of `UnPhysic.o`).
fn slide_scaled(delta: FVector, normal: FVector, d: f32, scale_by: f32) -> FVector {
    v3(
        (delta.x - normal.x * d) * scale_by,
        (delta.y - normal.y * d) * scale_by,
        (delta.z - normal.z * d) * scale_by,
    )
}

/// `AActor::moveSmooth(Delta)`. Returns the first `MoveActor` result.
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn move_smooth(this: *mut AActor, svc: &Hp2PhysicServices, delta: FVector) -> i32 {
    unsafe {
        let this_v = this as *mut c_void;
        let mut hit = blank_hit(1.0);
        let did_hit = (svc.move_actor)(this_v, &delta, &(*this).rotation, &mut hit);
        if hit.time < 1.0 {
            let mut adjusted =
                slide_scaled(delta, hit.normal, dot(delta, hit.normal), 1.0 - hit.time);
            if dot(delta, adjusted) >= 0.0 {
                let old_hit_normal = hit.normal;
                let desired_dir = safe_normal(delta);
                (svc.event_hit_wall)(this_v, &hit.normal, hit.actor);
                (svc.move_actor)(this_v, &adjusted, &(*this).rotation, &mut hit);
                if hit.time < 1.0 {
                    two_wall_adjust(
                        desired_dir,
                        &mut adjusted,
                        hit.normal,
                        old_hit_normal,
                        hit.time,
                    );
                    (svc.move_actor)(this_v, &adjusted, &(*this).rotation, &mut hit);
                }
            }
        }
        did_hit
    }
}

/// One of the four `SingleLineCheck` probes `processLanded` runs under a
/// decoration. `signs` is the (x, y) quadrant; `half` is `0.5f*CollisionRadius`.
unsafe fn landing_quad(
    svc: &Hp2PhysicServices,
    this: *mut c_void,
    hit: *mut FCheckResult,
    loc: FVector,
    signs: (f32, f32),
    half: f32,
    extent: &FVector,
) -> i32 {
    let (sx, sy) = signs;
    let end = v3(loc.x + sx * half, loc.y + sy * half, loc.z + -8.0);
    let start = v3(loc.x + sx * half, loc.y + sy * half, loc.z + 0.0);
    unsafe { (svc.single_line_check)(this, hit, &end, &start, TRACE_ALL_BLOCKING, extent) }
}

/// `AActor::processLanded(HitNormal, HitActor, remainingTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `AActor` (a pawn when `bIsPawn` is set, a
/// decoration or carcass when the class checks pass). `hit_actor` must be a live
/// object. `Region.Zone` must be live when `bIsPawn` is clear.
pub unsafe fn process_landed(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    hit_normal: FVector,
    hit_actor: *mut c_void,
    remaining_time: f32,
    iterations: i32,
) {
    unsafe {
        let this_v = this as *mut c_void;
        if !(*this).b_is_pawn() {
            let zone = (*this).region.zone as *const AZoneInfo;
            if (*zone).b_bounce_velocity() && !is_zero((*zone).zone_velocity) {
                (*this).velocity = add((*zone).zone_velocity, v3(0.0, 0.0, 80.0));
                return;
            }
        }

        if is_a(svc, this_v, CLASS_DECORATION) {
            let dec = this as *mut ADecoration;
            if (*dec).num_landings < 5 {
                let loc = (*this).location;
                let ch = (*this).collision_height;
                let cr = (*this).collision_radius;
                let mut hit = blank_hit(1.0);
                let end = sub(loc, v3(0.0, 0.0, ch + cr + 8.0));
                let start = sub(loc, v3(0.0, 0.0, 0.8 * ch));
                (svc.single_line_check)(this_v, &mut hit, &end, &start, TRACE_ALL_BLOCKING, &ZERO);
                if hit.actor.is_null() {
                    let mut extent = ZERO;
                    (svc.cylinder_extent)(this_v, &mut extent);
                    let mut part = scale(extent, 0.5);
                    part.z *= 2.0;
                    let half = 0.5 * cr;
                    let hit_ptr: *mut FCheckResult = &mut hit;
                    let q1 = landing_quad(svc, this_v, hit_ptr, loc, (1.0, 1.0), half, &part);
                    let q2 = landing_quad(svc, this_v, hit_ptr, loc, (-1.0, 1.0), half, &part);
                    let q3 = landing_quad(svc, this_v, hit_ptr, loc, (-1.0, -1.0), half, &part);
                    let q4 = landing_quad(svc, this_v, hit_ptr, loc, (1.0, -1.0), half, &part);
                    if q1 + q2 + q3 + q4 > 1 && q1 + q3 != 0 && q2 + q4 != 0 {
                        (*dec).num_landings += 1;
                        let speed = 2.0 * clamp_f(-(*this).velocity.z, 30.0, 30.0 + cr);
                        let dir = v3((q1 + q4 - q2 - q3) as f32, (q1 + q2 - q3 - q4) as f32, 0.5);
                        (*this).velocity = scale(dir, speed);
                        return;
                    }
                }
                if is_a(svc, this_v, CLASS_CARCASS)
                    && (hit_normal.z as f64) < 0.9
                    && (*(this as *mut ACarcass)).b_sliding_carcass()
                {
                    if (svc.app_frand)() < 0.2 {
                        (*dec).num_landings += 1;
                    }
                    (*this).velocity = scale(hit_normal, 120.0);
                    (*this).velocity.z = 70.0;
                    return;
                }
                (*dec).num_landings = 0;
            } else {
                (*dec).num_landings = 0;
            }
        }

        (svc.event_landed)(this_v, &hit_normal);

        if (*this).physics == PHYS_FALLING {
            if is_a(svc, this_v, CLASS_PAWN) {
                dispatch::set_physics(this, svc, PHYS_WALKING, hit_actor as *mut AActor);
            } else {
                dispatch::set_physics(this, svc, PHYS_NONE, hit_actor as *mut AActor);
                (*this).velocity = ZERO;
            }
        }

        if (*this).physics == PHYS_WALKING && is_a(svc, this_v, CLASS_PAWN) {
            (*this).acceleration = safe_normal((*this).acceleration);
            if remaining_time > 0.01 {
                walk::phys_walking(this as *mut APawn, svc, remaining_time, iterations);
            }
        }
    }
}

/// `Min(A, B)` from `UnTemplate.h` for `INT`.
fn min_i(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

/// `Max(A, B)` from `UnTemplate.h` for `INT`.
fn max_i(a: i32, b: i32) -> i32 {
    if a >= b { a } else { b }
}

/// `AActor::fixedTurn(current, desired, deltaRate)`. Angles are 16-bit
/// `FRotator` components carried in `int`. `fixed_dir` is
/// `bFixedRotationDir`, `rotate_to_desired` is `bRotateToDesired`.
pub fn fixed_turn(
    current: i32,
    desired: i32,
    delta_rate: i32,
    fixed_dir: bool,
    rotate_to_desired: bool,
) -> i32 {
    if delta_rate == 0 {
        return current & 65535;
    }

    let mut result = current & 65535;
    let mut current = result;
    let mut desired = desired & 65535;

    if fixed_dir {
        if rotate_to_desired {
            if delta_rate > 0 {
                if current > desired {
                    desired += 65536;
                }
                result = result.wrapping_add(min_i(delta_rate, desired - current));
            } else {
                if current < desired {
                    current += 65536;
                }
                result = result.wrapping_add(max_i(delta_rate, desired - current));
            }
        } else {
            result = result.wrapping_add(delta_rate);
        }
    } else if rotate_to_desired {
        if current > desired {
            if current - desired < 32768 {
                result = result.wrapping_sub(min_i(current - desired, delta_rate.wrapping_abs()));
            } else {
                result = result
                    .wrapping_add(min_i(desired + 65536 - current, delta_rate.wrapping_abs()));
            }
        } else if desired - current < 32768 {
            result = result.wrapping_add(min_i(desired - current, delta_rate.wrapping_abs()));
        } else {
            result =
                result.wrapping_sub(min_i(current + 65536 - desired, delta_rate.wrapping_abs()));
        }
    }

    result & 65535
}

#[cfg(test)]
mod tests {
    use super::fixed_turn;

    #[test]
    fn zero_rate_masks_current() {
        assert_eq!(fixed_turn(0x1_0005, 7, 0, true, true), 5);
    }

    #[test]
    fn shortest_way_wraps_down() {
        // 65000 -> 100 is 636 steps up through zero, capped by the rate.
        assert_eq!(fixed_turn(65000, 100, 1000, false, true), 100);
    }
}
