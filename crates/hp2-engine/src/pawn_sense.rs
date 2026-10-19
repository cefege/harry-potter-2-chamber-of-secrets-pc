//! Port of the `sense` group of `Engine/Src/UnPawn.cpp`: `APawn::setMoveTimer`,
//! `moveToward`, `rotateToward`, `LineOfSightTo`, `CanHear`,
//! `AActor::CheckNoiseHearing`, `CheckEnemyVisible`, `PickWallAdjust` and
//! `GetNetPriority`.
//!
//! Engine calls made by these bodies (out-of-line, virtual, or inline reads of
//! engine objects that are not mirrored here) go through [`Hp2PawnSenseServices`],
//! filled in by `Hp2PawnSenseServices.cpp`. `clock`/`unclock`/`check` stay in the
//! C++ shells.
//!
//! FP contraction: each `mul_add` marks a site the original object file fuses
//! (`fmadd`/`fmsub`). Every other product is a separate rounded multiply.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, FRotator};

use crate::pawn::fresh_hit;
use crate::vec;

/// `EPhysics` values (EngineClasses.h).
const PHYS_WALKING: u8 = 1;
const PHYS_FALLING: u8 = 2;
const PHYS_SWIMMING: u8 = 3;
const PHYS_FLYING: u8 = 4;
const PHYS_SPIDER: u8 = 10;

/// `TRACE_VisBlocking` (UnLevel.h): `TRACE_Level | TRACE_Movers`.
const TRACE_VIS_BLOCKING: u32 = 0x06;

/// Class tests answered by [`Hp2PawnSenseServices::is_a`] (`IsA(X::StaticClass())`).
pub const CLASS_PAWN: u32 = 1;
pub const CLASS_PLAYER_PAWN: u32 = 2;
pub const CLASS_INVENTORY: u32 = 3;

/// `FName` probes answered by [`Hp2PawnSenseServices::is_probing`].
pub const NAME_HEAR_NOISE: u32 = 0;
pub const NAME_SPECIAL_HANDLING: u32 = 1;

/// Engine services used by the sense group. Filled in by `Hp2PawnSenseServices.cpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnSenseServices {
    /// `ULevel::Model->FastLineCheck(a, b)`; `a` and `b` point at `FVector`. Returns `UBOOL`.
    pub fast_line_check:
        unsafe extern "C" fn(level: *mut c_void, a: *const c_void, b: *const c_void) -> i32,
    /// `ULevel::SingleLineCheck(hit, source, end, start, trace, extent)` (virtual).
    /// `hit` is `FCheckResult`; `end`, `start` and `extent` point at `FVector`.
    pub single_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        source: *mut c_void,
        hit: *mut c_void,
        end: *const c_void,
        start: *const c_void,
        trace: u32,
        extent: *const c_void,
    ),
    /// `appFrand()`.
    pub app_frand: unsafe extern "C" fn() -> f32,
    /// `AActor::setPhysics(phys)`.
    pub set_physics: unsafe extern "C" fn(actor: *mut c_void, phys: u8),
    /// `AActor::GetCylinderExtent()`; `out` points at `FVector`.
    pub get_cylinder_extent: unsafe extern "C" fn(actor: *mut c_void, out: *mut c_void),
    /// `UObject::IsProbing(name)` with `name` one of the `NAME_*` values above.
    pub is_probing: unsafe extern "C" fn(obj: *mut c_void, name: u32) -> i32,
    /// `pawn->eventHearNoise(loudness, noise)`.
    pub event_hear_noise:
        unsafe extern "C" fn(pawn: *mut c_void, loudness: f32, noise: *mut c_void),
    /// `pawn->eventEnemyNotVisible()`.
    pub event_enemy_not_visible: unsafe extern "C" fn(pawn: *mut c_void),
    /// `actor->eventTouch(other)`.
    pub event_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    /// `FVector::SafeNormal()`; `v` and `out` point at `FVector`.
    pub safe_normal: unsafe extern "C" fn(v: *const c_void, out: *mut c_void),
    /// `FVector::Rotation()`; `v` points at `FVector`, `out` at `FRotator`.
    pub vector_rotation: unsafe extern "C" fn(v: *const c_void, out: *mut c_void),
    /// Inline `FRotator::Vector()`; `rot` points at `FRotator`, `out` at `FVector`.
    pub rotation_vector: unsafe extern "C" fn(rot: *const c_void, out: *mut c_void),
    /// Inline `obj->IsA(X::StaticClass())` for the `CLASS_*` values above.
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, class_kind: u32) -> i32,
    /// Inline `obj->IsA(other->GetClass())`.
    pub is_a_class_of: unsafe extern "C" fn(obj: *mut c_void, other: *mut c_void) -> i32,
    /// Inline `GetLevelInfo()->PawnList`.
    pub level_pawn_head: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// Inline `GetLevelInfo()->Game->bTeamGame`.
    pub level_team_game: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// Inline `TimeSeconds.GetFloat()`.
    pub level_time: unsafe extern "C" fn(level: *mut c_void) -> f32,
    /// Inline `pri_a->Team != pri_b->Team`.
    pub teams_differ: unsafe extern "C" fn(pri_a: *mut c_void, pri_b: *mut c_void) -> i32,
    /// Inline `((AZoneInfo*)zone->GetClass()->GetDefaultObject())->ZoneGravity.Z`.
    pub zone_default_gravity_z: unsafe extern "C" fn(zone: *mut c_void) -> f32,
    /// `APawn::actorReachable(other, know_visible)`; out-of-line member (reach port).
    pub actor_reachable:
        unsafe extern "C" fn(pawn: *mut c_void, other: *mut c_void, know_visible: i32) -> i32,
    /// `APawn::findPathToward(goal, single, bestPath&, clear)`; `best_path` points at `AActor*`.
    pub find_path_toward: unsafe extern "C" fn(
        pawn: *mut c_void,
        goal: *mut c_void,
        single: i32,
        best_path: *mut *mut c_void,
        clear: i32,
    ) -> i32,
    /// `actor->eventSpecialHandling(pawn)` (UnrealScript event); returns `AActor*`.
    pub event_special_handling:
        unsafe extern "C" fn(actor: *mut c_void, pawn: *mut c_void) -> *mut c_void,
}

fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

fn abs_i(a: i32) -> i32 {
    if a >= 0 { a } else { -a }
}

/// `Max(A,B)` (UnTemplate.h).
fn max_t<T: PartialOrd>(a: T, b: T) -> T {
    if a >= b { a } else { b }
}

/// `Min(A,B)` (UnTemplate.h).
fn min_t<T: PartialOrd>(a: T, b: T) -> T {
    if a <= b { a } else { b }
}

/// `A|B`, contracted the way the original fuses the 3-term sum.
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `Base + V * S`, contracted: one fused multiply-add per component.
fn add_scaled(base: FVector, v: FVector, s: f32) -> FVector {
    FVector {
        x: v.x.mul_add(s, base.x),
        y: v.y.mul_add(s, base.y),
        z: v.z.mul_add(s, base.z),
    }
}

/// `FVector::SafeNormal()` through the service.
unsafe fn safe_normal(svc: &Hp2PawnSenseServices, v: FVector) -> FVector {
    unsafe {
        let mut out = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        (svc.safe_normal)(
            (&v as *const FVector).cast(),
            (&mut out as *mut FVector).cast(),
        );
        out
    }
}

/// `FRotator::Vector()` through the service.
unsafe fn rotation_vector(svc: &Hp2PawnSenseServices, rot: &FRotator) -> FVector {
    unsafe {
        let mut out = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        (svc.rotation_vector)(
            (rot as *const FRotator).cast(),
            (&mut out as *mut FVector).cast(),
        );
        out
    }
}

/// `AActor::GetCylinderExtent()` through the service.
unsafe fn cylinder_extent(svc: &Hp2PawnSenseServices, actor: *mut AActor) -> FVector {
    unsafe {
        let mut out = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        (svc.get_cylinder_extent)(actor.cast(), (&mut out as *mut FVector).cast());
        out
    }
}

/// `ULevel::SingleLineCheck(hit, this, end, start, TRACE_VisBlocking, extent)`.
unsafe fn trace_line(
    svc: &Hp2PawnSenseServices,
    level: *mut c_void,
    source: *mut AActor,
    hit: &mut FCheckResult,
    end: FVector,
    start: FVector,
    extent: FVector,
) {
    unsafe {
        (svc.single_line_check)(
            level,
            source.cast(),
            (hit as *mut FCheckResult).cast(),
            (&end as *const FVector).cast(),
            (&start as *const FVector).cast(),
            TRACE_VIS_BLOCKING,
            (&extent as *const FVector).cast(),
        );
    }
}

/// `ULevel::SingleLineCheck(hit, this, end, start, TRACE_VisBlocking)` with the
/// default zero extent.
unsafe fn trace_line0(
    svc: &Hp2PawnSenseServices,
    level: *mut c_void,
    source: *mut AActor,
    hit: &mut FCheckResult,
    end: FVector,
    start: FVector,
) {
    unsafe {
        trace_line(
            svc,
            level,
            source,
            hit,
            end,
            start,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        );
    }
}

/// `APawn::setMoveTimer(MoveSize)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn set_move_timer(pawn: *mut APawn, move_size: f32) {
    unsafe {
        let p = &mut *pawn;
        let max_speed = match p.base.physics {
            PHYS_WALKING | PHYS_FALLING | PHYS_SPIDER => p.ground_speed,
            PHYS_FLYING => p.air_speed,
            PHYS_SWIMMING => p.water_speed,
            // Safety in case called with Physics = PHYS_None.
            _ => 200.0,
        };
        p.move_timer = if p.desired_speed * max_speed == 0.0 {
            0.5
        } else {
            1.0 + 1.3 * move_size / (p.desired_speed * max_speed)
        };
    }
}

/// Sets `DesiredRotation.Yaw = Rotation.Yaw` when walking (the repeated tail of `moveToward`).
unsafe fn face_rotation_if_walking(pawn: *mut APawn) {
    unsafe {
        if (*pawn).base.physics == PHYS_WALKING {
            (*pawn).base.desired_rotation.yaw = (*pawn).base.rotation.yaw;
        }
    }
}

/// `APawn::rotateToward(FocalPoint)`. Returns 1 if the yaw is within 100 of the target.
///
/// # Safety
/// `pawn` must point to a live `APawn` with valid `MoveTarget` and services.
pub unsafe fn rotate_toward(svc: &Hp2PawnSenseServices, pawn: *mut APawn, focal: FVector) -> i32 {
    unsafe {
        let p = &mut *pawn;
        if p.base.physics == PHYS_SPIDER {
            return 1;
        }
        let direction = vec::sub(focal, p.base.location);

        // Rotate toward destination.
        (svc.vector_rotation)(
            (&direction as *const FVector).cast(),
            (&mut p.base.desired_rotation as *mut FRotator).cast(),
        );
        p.base.desired_rotation.yaw &= 65535;
        if p.base.physics == PHYS_WALKING
            && (p.move_target.is_null() || (svc.is_a)(p.move_target, CLASS_PAWN) == 0)
        {
            p.base.desired_rotation.pitch = 0;
        }

        // Only base success on Yaw.
        let cur = p.base.rotation.yaw & 65535;
        let mut success = (abs_i(p.base.desired_rotation.yaw - cur) < 100) as i32;
        if success == 0 {
            // Check if on opposite sides of zero.
            success = (abs_i(p.base.desired_rotation.yaw - cur) > 65535 - 100) as i32;
        }
        success
    }
}

/// `APawn::moveToward(Dest)`. Returns 1 if the pawn reached the point.
///
/// # Safety
/// `pawn` must point to a live `APawn` whose region zone and move target are valid.
pub unsafe fn move_toward(svc: &Hp2PawnSenseServices, pawn: *mut APawn, dest: FVector) -> i32 {
    unsafe {
        let p = &mut *pawn;
        let loc = p.base.location;
        let mut direction = vec::sub(dest, loc);
        if p.base.physics == PHYS_WALKING {
            direction.z = 0.0;
        } else if p.base.physics == PHYS_FALLING {
            // Use air control if low grav.
            let zone = p.base.region.zone.cast::<AZoneInfo>();
            if ((*zone).zone_gravity.z as f64)
                > 0.9 * ((svc.zone_default_gravity_z)(zone.cast()) as f64)
            {
                direction.z = 0.0;
                p.base.acceleration = vec::scale(safe_normal(svc, direction), p.accel_rate);
                if p.base.velocity.z < 0.0 && loc.z + 100.0 < dest.z {
                    return 1;
                }
            }
            return 0;
        }

        let mt = p.move_target;
        if !mt.is_null()
            && (svc.is_a)(mt, CLASS_INVENTORY) != 0
            && abs_f(loc.z - (*mt.cast::<AActor>()).location.z) < p.base.collision_height
        {
            let mloc = (*mt.cast::<AActor>()).location;
            let dx = loc.x - mloc.x;
            let dy = loc.y - mloc.y;
            // Square(dx) + Square(dy) < Square(CollisionRadius): the two products are not fused.
            let cr = p.base.collision_radius;
            if dx * dx + dy * dy < cr * cr {
                (svc.event_touch)(mt, pawn.cast());
            }
        }

        let distance = vec::size(direction);

        let b_glider =
            !p.b_can_strafe() && (p.base.physics == PHYS_FLYING || p.base.physics == PHYS_SWIMMING);

        let speed = vec::size(p.base.velocity);
        let moved = vec::sub(p.base.location, p.base.old_location);
        let b_is_moving = moved.x != 0.0 || moved.y != 0.0 || moved.z != 0.0 || speed == 0.0;

        let direction_sq = direction.x.mul_add(direction.x, direction.y * direction.y);
        let moved_sq = moved.x.mul_add(moved.x, moved.y * moved.y);

        let arrived_moving = dot(moved, vec::sub(dest, p.base.old_location)) > 0.0
            && dot(moved, direction) < 0.0
            && direction_sq / 3.0 / 3.0 < moved_sq;
        let arrived_still = direction_sq < 16.0;
        let arrived = if b_is_moving {
            arrived_moving
        } else {
            arrived_still
        };

        if arrived && abs_f(direction.z) < max_t(48.0, p.base.collision_height) {
            if !b_glider {
                p.base.acceleration = FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                };
            }
            // May want to try a MoveActor here at some point.
            face_rotation_if_walking(pawn);
            return 1;
        } else if b_glider {
            direction = rotation_vector(svc, &p.base.rotation);
        } else if distance > 0.0 {
            direction = vec::div(direction, distance);
        }

        p.base.acceleration = vec::scale(direction, p.accel_rate);

        // If we're not moving, don't do the full wait timer bit.
        if !b_is_moving {
            p.move_timer = min_t(0.25, p.move_timer);
        }

        if p.move_timer < 0.0 {
            face_rotation_if_walking(pawn);
            return 1;
        }

        if !mt.is_null() && (svc.is_a)(mt, CLASS_PAWN) != 0 {
            let mt_radius = (*mt.cast::<AActor>()).collision_radius;
            // CollisionRadius + MoveTarget->CollisionRadius + 0.8 * MeleeRange, in double.
            let reach = 0.8f64.mul_add(
                p.melee_range as f64,
                (p.base.collision_radius + mt_radius) as f64,
            );
            if (distance as f64) < reach {
                face_rotation_if_walking(pawn);
                return 1;
            }
            return 0;
        }

        if !b_glider && speed > 100.0 {
            let vel_dir = vec::div(p.base.velocity, speed);
            // Acceleration -= 0.2f * (1 - (Direction|VelDir)) * speed * (VelDir - Direction)
            let k = 0.2f32 * (1.0 - dot(direction, vel_dir)) * speed;
            p.base.acceleration = vec::sub(
                p.base.acceleration,
                vec::scale(vec::sub(vel_dir, direction), k),
            );
        }

        if (distance as f64) < 1.4 * (p.avg_physics_time as f64) * (speed as f64) {
            // Haven't reduced speed yet.
            if !p.b_reduced_speed() {
                p.desired_speed *= 0.5;
                p.set_b_reduced_speed(true);
            }
            if speed > 0.0 {
                p.desired_speed = min_t(p.desired_speed, 200.0 / speed);
            }
            if b_glider {
                face_rotation_if_walking(pawn);
                return 1;
            }
        }
        0
    }
}

/// `APawn::LineOfSightTo(Other, bShowSelf)`.
///
/// # Safety
/// `pawn` and `other` must point to live actors; `pawn`'s level and model must be valid.
pub unsafe fn line_of_sight_to(
    svc: &Hp2PawnSenseServices,
    pawn: *mut APawn,
    other: *mut AActor,
    mut show_self: i32,
) -> u32 {
    unsafe {
        if other.is_null() {
            return 0;
        }
        let p = &mut *pawn;
        let level = p.base.x_level;

        // FIXME in the original: when PVS, only do this test if in same PVS.
        if other.cast::<c_void>() == p.enemy {
            show_self = 0;
        }
        // Only players do showself.
        if show_self != 0 {
            let flag = p.b_lo_sflag();
            p.set_b_lo_sflag(!flag);
        }

        let mut dist_sq = vec::size_squared(vec::sub((*other).location, p.base.location));
        let maxdist_sq: f32;

        if show_self != 0 {
            if (svc.is_a)(other.cast(), CLASS_PAWN) != 0 {
                let vis = (*other.cast::<APawn>()).visibility as f32 * 0.0078125;
                let m = p.sight_radius * min_t(1.0, vis);
                maxdist_sq = if p.b_is_player() {
                    min_t(16000000.0, m * m)
                } else {
                    min_t(12000000.0, m * m)
                };
            } else {
                maxdist_sq = p.sight_radius * p.sight_radius;
            }
            if dist_sq > maxdist_sq {
                return 0;
            }
            // Check field of view.
            let sight_dir = safe_normal(svc, vec::sub((*other).location, p.base.location));
            p.stimulus =
                dot(sight_dir, rotation_vector(svc, &p.base.rotation)) - p.peripheral_vision;
            if p.stimulus > 0.0 {
                p.stimulus = 0.8f32.mul_add(p.stimulus, 0.2);
            } else {
                p.stimulus = 0.17f32.mul_add(p.stimulus, 0.2);
            }
            if p.stimulus <= 0.0 {
                return 0;
            }
            let height_mod =
                abs_f((*other).location.z - p.base.location.z) / max_t(1.0, p.skill + 1.0);
            dist_sq = height_mod.mul_add(height_mod, dist_sq);
            dist_sq /= p.stimulus * p.stimulus;
            if dist_sq > maxdist_sq {
                return 0;
            }
            p.stimulus = 1.0;
        } else if p.b_is_player() {
            if (svc.is_a)(other.cast(), CLASS_PAWN) != 0 {
                let vis = ((*other.cast::<APawn>()).visibility as i32 + 16) as f32 * 0.015;
                let m = 5000.0 * min_t(1.0, vis);
                maxdist_sq = min_t(25000000.0, m * m);
            } else {
                maxdist_sq = 16000000.0;
            }
            if dist_sq > maxdist_sq {
                return 0;
            }
        } else {
            if (svc.is_a)(other.cast(), CLASS_PAWN) != 0 {
                let vis = ((*other.cast::<APawn>()).visibility as i32 + 16) as f32 * 0.015;
                let m = 4000.0 * min_t(1.0, vis);
                maxdist_sq = min_t(16000000.0, m * m);
            } else {
                maxdist_sq = 9000000.0;
            }
            if dist_sq > maxdist_sq {
                return 0;
            }
        }

        // Look from eyes.
        let mut view_point = p.base.location;
        view_point.z += p.base_eye_height;

        if other.cast::<c_void>() == p.enemy {
            let enemy = p.enemy.cast::<AActor>();
            if (svc.fast_line_check)(
                level,
                (&(*other).location as *const FVector).cast(),
                (&view_point as *const FVector).cast(),
            ) != 0
            {
                p.last_seeing_pos = p.base.location;
                p.last_seen_pos = (*enemy).location;
                return 1;
            }
            if (svc.fast_line_check)(
                level,
                (&(*other).location as *const FVector).cast(),
                (&p.base.location as *const FVector).cast(),
            ) != 0
            {
                p.last_seeing_pos = p.base.location;
                p.last_seen_pos = (*enemy).location;
                return 1;
            }
            if dist_sq > 1000000.0 {
                return 0;
            }
        } else if dist_sq > 1000000.0 {
            if (svc.is_a)(other.cast(), CLASS_PAWN) != 0 {
                if !p.b_lo_sflag() && (dist_sq as f64) > 0.5 * (maxdist_sq as f64) {
                    return 0;
                }
                if !p.b_is_player() && ((svc.app_frand)() as f64) < 0.5 {
                    return 0;
                }
            }
            return (svc.fast_line_check)(
                level,
                (&(*other).location as *const FVector).cast(),
                (&view_point as *const FVector).cast(),
            ) as u32;
        }

        // Try viewpoint to head.
        let mut other_body = (*other).location;
        if show_self == 0 || !p.b_lo_sflag() {
            other_body.z = (*other).collision_height.mul_add(0.8f32, other_body.z);
            if (svc.fast_line_check)(
                level,
                (&other_body as *const FVector).cast(),
                (&view_point as *const FVector).cast(),
            ) != 0
            {
                return 1;
            }
        }

        if dist_sq > 250000.0 || (svc.is_a)(other.cast(), CLASS_PAWN) == 0 {
            return 0;
        }

        // Try checking sides: look at distance to four side points, and cull furthest and closest.
        let oloc = (*other).location;
        let r = (*other).collision_radius;
        let points = [
            vec::sub(
                oloc,
                FVector {
                    x: r,
                    y: -r,
                    z: 0.0,
                },
            ),
            vec::add(oloc, FVector { x: r, y: r, z: 0.0 }),
            vec::sub(oloc, FVector { x: r, y: r, z: 0.0 }),
            vec::add(
                oloc,
                FVector {
                    x: r,
                    y: -r,
                    z: 0.0,
                },
            ),
        ];
        let mut imin = 0usize;
        let mut imax = 0usize;
        let mut currentmin = vec::size_squared(points[0]);
        let mut currentmax = currentmin;
        for (i, pt) in points.iter().enumerate().skip(1) {
            let nextsize = vec::size_squared(*pt);
            if nextsize > currentmax {
                currentmax = nextsize;
                imax = i;
            } else if nextsize < currentmin {
                currentmin = nextsize;
                imin = i;
            }
        }

        let mut bskip = p.b_lo_sflag() as i32;
        for (i, pt) in points.iter().enumerate() {
            if i != imin && i != imax {
                if bskip != 0 && show_self != 0 {
                    bskip = 0;
                } else {
                    bskip = 1;
                    if (svc.fast_line_check)(
                        level,
                        (pt as *const FVector).cast(),
                        (&view_point as *const FVector).cast(),
                    ) != 0
                    {
                        return 1;
                    }
                }
            }
        }
        0
    }
}

/// `APawn::CanHear(NoiseLoc, Loudness, Other)`.
///
/// # Safety
/// `pawn` and `other` must point to live pawns; `pawn`'s level and model must be valid.
pub unsafe fn can_hear(
    svc: &Hp2PawnSenseServices,
    pawn: *mut APawn,
    noise_loc: FVector,
    loudness: f32,
    other: *mut APawn,
) -> i32 {
    unsafe {
        let p = &mut *pawn;
        let dist_sq = vec::size_squared(vec::sub(p.base.location, noise_loc));
        let level = p.base.x_level;

        let team_check = !p.b_is_player()
            || (svc.level_team_game)(level) == 0
            || !(*other).b_is_player()
            || p.player_replication_info.is_null()
            || (*other).player_replication_info.is_null()
            || (svc.teams_differ)(p.player_replication_info, (*other).player_replication_info) != 0;

        if team_check {
            if dist_sq > 4000000.0 * loudness * loudness {
                return 0;
            }
            if dist_sq == 0.0 {
                return 1;
            }
            // A loudness of 1.0 is a typical loud noise.
            let perceived = min_t(1200000.0 / dist_sq, 2.0);
            p.stimulus = loudness.mul_add(perceived, p.alertness * min_t(0.5, perceived));
            if p.stimulus < p.hearing_threshold {
                return 0;
            }
        } else if dist_sq > 16000000.0 * loudness * loudness {
            return 0;
        }

        // FIXME in the original: check PVS.
        (svc.fast_line_check)(
            level,
            (&noise_loc as *const FVector).cast(),
            (&p.base.location as *const FVector).cast(),
        )
    }
}

/// `AActor::CheckNoiseHearing(Loudness)`, minus the `clock`/`unclock` in the C++ shell.
///
/// # Safety
/// `this` must point to a live actor; `Instigator` (when set) must point to a live pawn.
pub unsafe fn check_noise_hearing(svc: &Hp2PawnSenseServices, this: *mut AActor, loudness: f32) {
    unsafe {
        let owner_ptr = (*this).instigator.cast::<APawn>();
        if owner_ptr.is_null() {
            return;
        }
        let level = (*this).x_level;
        let current_time = (svc.level_time)(level);

        // Allow only one noise per 0.2 seconds from a given instigator & area (within 50 units)
        // unless much louder.
        let recent = current_time - 0.2;
        if (*owner_ptr).noise1time > recent
            && vec::size_squared(vec::sub((*owner_ptr).noise1spot, (*this).location)) < 2500.0
            && ((*owner_ptr).noise1loudness as f64) >= 0.9 * (loudness as f64)
        {
            return;
        }
        if (*owner_ptr).noise2time > recent
            && vec::size_squared(vec::sub((*owner_ptr).noise2spot, (*this).location)) < 2500.0
            && ((*owner_ptr).noise2loudness as f64) >= 0.9 * (loudness as f64)
        {
            return;
        }

        // Put this noise in a slot.
        let stale = current_time - 0.18;
        if (*owner_ptr).noise1time < stale {
            set_noise1(owner_ptr, current_time, (*this).location, loudness);
        } else if (*owner_ptr).noise2time < stale {
            set_noise2(owner_ptr, current_time, (*this).location, loudness);
        } else if vec::size_squared(vec::sub((*owner_ptr).noise1spot, (*this).location)) < 2500.0
            && (*owner_ptr).noise1loudness <= loudness
        {
            set_noise1(owner_ptr, current_time, (*this).location, loudness);
        } else if (*owner_ptr).noise2loudness <= loudness {
            // Quirk kept from the original: this branch also writes slot 1.
            set_noise1(owner_ptr, current_time, (*this).location, loudness);
        }

        let this_c = this.cast::<c_void>();
        if !(*owner_ptr).b_is_player()
            && ((*owner_ptr).enemy.is_null()
                || !(*(*owner_ptr).enemy.cast::<APawn>()).b_is_player())
        {
            // Non-player related noise: inform only others of the same class.
            let mut next = (svc.level_pawn_head)(level).cast::<APawn>();
            while !next.is_null() {
                if next.cast::<c_void>() != (*this).instigator
                    && (svc.is_probing)(next.cast(), NAME_HEAR_NOISE) != 0
                    && ((svc.is_a_class_of)(next.cast(), this_c) != 0
                        || (svc.is_a_class_of)(this_c, next.cast()) != 0)
                    && can_hear(svc, next, (*this).location, loudness, owner_ptr) != 0
                {
                    (svc.event_hear_noise)(next.cast(), loudness, this_c);
                }
                next = (*next).next_pawn.cast::<APawn>();
            }
            return;
        }

        // All pawns can hear this noise.
        let mut pawn = (svc.level_pawn_head)(level).cast::<APawn>();
        while !pawn.is_null() {
            if (svc.is_a)(pawn.cast(), CLASS_PLAYER_PAWN) == 0
                && pawn.cast::<c_void>() != (*this).instigator
                && (svc.is_probing)(pawn.cast(), NAME_HEAR_NOISE) != 0
                && can_hear(svc, pawn, (*this).location, loudness, owner_ptr) != 0
            {
                (svc.event_hear_noise)(pawn.cast(), loudness, this_c);
            }
            pawn = (*pawn).next_pawn.cast::<APawn>();
        }
    }
}

unsafe fn set_noise1(owner: *mut APawn, time: f32, spot: FVector, loudness: f32) {
    unsafe {
        (*owner).noise1time = time;
        (*owner).noise1spot = spot;
        (*owner).noise1loudness = loudness;
    }
}

unsafe fn set_noise2(owner: *mut APawn, time: f32, spot: FVector, loudness: f32) {
    unsafe {
        (*owner).noise2time = time;
        (*owner).noise2spot = spot;
        (*owner).noise2loudness = loudness;
    }
}

/// `APawn::CheckEnemyVisible()`, minus the `clock`/`check`/`unclock` in the C++ shell.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn check_enemy_visible(svc: &Hp2PawnSenseServices, pawn: *mut APawn) {
    unsafe {
        let enemy = (*pawn).enemy.cast::<AActor>();
        if enemy.is_null() {
            return;
        }
        if line_of_sight_to(svc, pawn, enemy, 0) == 0 {
            (svc.event_enemy_not_visible)(pawn.cast());
        } else {
            (*pawn).last_seen_time = (svc.level_time)((*pawn).base.x_level);
        }
    }
}

/// `APawn::PickWallAdjust()`.
///
/// # Safety
/// `pawn` must point to a live `APawn` with valid level and region services.
pub unsafe fn pick_wall_adjust(svc: &Hp2PawnSenseServices, pawn: *mut APawn) -> i32 {
    unsafe {
        let p = &mut *pawn;
        let this = pawn.cast::<AActor>();
        let level = p.base.x_level;
        let loc = p.base.location;

        if p.base.physics == PHYS_FALLING {
            return 0;
        }

        // First pick the likely direction with traces, then check with testmove.
        let mut hit: FCheckResult = fresh_hit();
        let mut view_point = loc;
        view_point.z += p.base_eye_height;
        let mut dir = vec::sub(p.destination, loc);
        let zdiff = dir.z;
        let mut adjust_dist = p.base.collision_radius + 16.0;
        if p.b_is_player() {
            // Bots only, till tested for all.
            adjust_dist = 0.5f32.mul_add(p.base.collision_radius, adjust_dist);
        }
        dir.z = 0.0;
        let mut bcheckup: i32 = 0;
        let cr = p.base.collision_radius;
        let ch = p.base.collision_height;

        if zdiff < ch {
            // DistDiff = (Dir|Dir) - CollisionRadius * CollisionRadius
            let dist_diff = (-cr).mul_add(cr, dot(dir, dir));
            if dist_diff < 0.0 {
                return 0;
            }
            if (p.base.physics == PHYS_SWIMMING || p.base.physics == PHYS_FLYING)
                && vec::size_squared(dir) < 4.0 * ch * ch
            {
                let mut up = FVector {
                    x: 0.0,
                    y: 0.0,
                    z: ch,
                };
                bcheckup = 1;
                if loc.z < p.destination.z {
                    bcheckup = -1;
                    up = vec::scale(up, -1.0);
                }
                let ext = cylinder_extent(svc, this);
                trace_line(svc, level, this, &mut hit, vec::add(loc, up), loc, ext);
                if hit.time == 1.0 {
                    let mut short_dir = safe_normal(svc, dir);
                    short_dir = vec::scale(short_dir, cr);
                    let ext = cylinder_extent(svc, this);
                    trace_line(
                        svc,
                        level,
                        this,
                        &mut hit,
                        vec::add(vec::add(loc, up), short_dir),
                        vec::add(loc, up),
                        ext,
                    );
                    if hit.time == 1.0 {
                        p.destination = vec::add(loc, up);
                        return 1;
                    }
                }
            }
        }

        let dist = vec::size(dir);
        if dist == 0.0 {
            return 0;
        }
        dir = vec::div(dir, dist);
        if (svc.fast_line_check)(
            level,
            (&p.destination as *const FVector).cast(),
            (&view_point as *const FVector).cast(),
        ) == 0
        {
            if p.base.physics != PHYS_SWIMMING && p.base.physics != PHYS_FLYING {
                adjust_dist += cr;
                if zdiff > 256.0 {
                    return 0;
                }
            } else if zdiff > 0.0 {
                let up = FVector {
                    x: 0.0,
                    y: 0.0,
                    z: ch,
                };
                let ext = cylinder_extent(svc, this);
                trace_line(
                    svc,
                    level,
                    this,
                    &mut hit,
                    vec::add(loc, vec::scale(up, 2.0)),
                    loc,
                    ext,
                );
                if hit.time == 1.0 {
                    p.destination = vec::add(loc, up);
                    return 1;
                }
            }
        }

        // Look left and right.
        let mut left = FVector {
            x: dir.y,
            y: -dir.x,
            z: 0.0,
        };
        let mut bcheckright = 0;
        let mut check_left = vec::scale(vec::scale(left, 1.42), cr);
        trace_line0(
            svc,
            level,
            this,
            &mut hit,
            p.destination,
            vec::add(view_point, check_left),
        );
        if hit.time < 1.0 {
            // Try right.
            bcheckright = 1;
            left = vec::scale(left, -1.0);
            check_left = vec::scale(check_left, -1.0);
            trace_line0(
                svc,
                level,
                this,
                &mut hit,
                p.destination,
                vec::add(view_point, check_left),
            );
        }

        if hit.time < 1.0 {
            // Neither side has visibility.
            return 0;
        }

        let out = vec::scale(dir, 14.0);
        let mut bcheckstepup = 0;
        if p.b_is_player() && p.base.physics == PHYS_WALKING && p.b_can_jump() {
            // Try step up first.
            bcheckstepup = 1;
            let up = FVector {
                x: 0.0,
                y: 0.0,
                z: 48.0,
            };
            let ext = cylinder_extent(svc, this);
            trace_line(svc, level, this, &mut hit, vec::add(loc, up), loc, ext);
            if hit.time > 0.5 {
                let ext = cylinder_extent(svc, this);
                let t = hit.time;
                trace_line(
                    svc,
                    level,
                    this,
                    &mut hit,
                    vec::add(add_scaled(loc, up, t), out),
                    add_scaled(loc, up, t),
                    ext,
                );
                if hit.time == 1.0 {
                    if p.b_is_player() && ((svc.app_frand)() as f64) < 0.5 {
                        let ext = cylinder_extent(svc, this);
                        trace_line(svc, level, this, &mut hit, vec::add(loc, out), loc, ext);
                        if hit.time < 1.0 {
                            dir = vec::scale(hit.normal, -1.0);
                        }
                    }
                    p.base.velocity = vec::scale(dir, p.ground_speed);
                    p.base.acceleration = vec::scale(dir, p.accel_rate);
                    p.base.velocity.z = p.jump_z;
                    p.set_b_jump_off_pawn(true); // Don't let script adjust this jump again.
                    (svc.set_physics)(this.cast(), PHYS_FALLING);
                    return 1;
                }
            }
        }

        // Try step left or right.
        left = vec::scale(left, adjust_dist);
        let ext = cylinder_extent(svc, this);
        trace_line(svc, level, this, &mut hit, vec::add(loc, left), loc, ext);
        if hit.time == 1.0 {
            let ext = cylinder_extent(svc, this);
            trace_line(
                svc,
                level,
                this,
                &mut hit,
                vec::add(vec::add(loc, left), out),
                vec::add(loc, left),
                ext,
            );
            if hit.time == 1.0 {
                p.destination = vec::add(loc, left);
                return 1;
            }
        }

        if bcheckright == 0 {
            // If didn't already try right, now try it.
            check_left = vec::scale(check_left, -1.0);
            trace_line0(
                svc,
                level,
                this,
                &mut hit,
                p.destination,
                vec::add(view_point, check_left),
            );
            if hit.time < 1.0 {
                return 0;
            }
            left = vec::scale(left, -1.0);
            let ext = cylinder_extent(svc, this);
            trace_line(svc, level, this, &mut hit, vec::add(loc, left), loc, ext);
            if hit.time == 1.0 {
                let ext = cylinder_extent(svc, this);
                trace_line(
                    svc,
                    level,
                    this,
                    &mut hit,
                    vec::add(vec::add(loc, left), out),
                    vec::add(loc, left),
                    ext,
                );
                if hit.time == 1.0 {
                    p.destination = vec::add(loc, left);
                    return 1;
                }
            }
        }

        // Try jump up if walking, or adjust up or down if swimming.
        if p.base.physics == PHYS_WALKING {
            if bcheckstepup != 0 || !p.b_can_jump() {
                return 0;
            }
            let up = FVector {
                x: 0.0,
                y: 0.0,
                z: 48.0,
            };
            let ext = cylinder_extent(svc, this);
            trace_line(svc, level, this, &mut hit, vec::add(loc, up), loc, ext);
            if hit.time == 1.0 {
                let ext = cylinder_extent(svc, this);
                trace_line(
                    svc,
                    level,
                    this,
                    &mut hit,
                    vec::add(vec::add(loc, up), out),
                    vec::add(loc, up),
                    ext,
                );
                if hit.time == 1.0 {
                    if p.b_is_player() {
                        let ext = cylinder_extent(svc, this);
                        trace_line(svc, level, this, &mut hit, vec::add(loc, out), loc, ext);
                        if hit.time < 1.0 {
                            dir = vec::scale(hit.normal, -1.0);
                        }
                    }
                    p.base.velocity = vec::scale(dir, p.ground_speed);
                    p.base.acceleration = vec::scale(dir, p.accel_rate);
                    p.base.velocity.z = p.jump_z;
                    p.set_b_jump_off_pawn(true); // Don't let script adjust this jump again.
                    (svc.set_physics)(this.cast(), PHYS_FALLING);
                    return 1;
                }
            }
        } else {
            let mut up = FVector {
                x: 0.0,
                y: 0.0,
                z: ch,
            };
            if bcheckup != 1 {
                let ext = cylinder_extent(svc, this);
                trace_line(svc, level, this, &mut hit, vec::add(loc, up), loc, ext);
                if hit.time == 1.0 {
                    let ext = cylinder_extent(svc, this);
                    trace_line(
                        svc,
                        level,
                        this,
                        &mut hit,
                        vec::add(vec::add(loc, up), out),
                        vec::add(loc, up),
                        ext,
                    );
                    if hit.time == 1.0 {
                        p.destination = vec::add(loc, up);
                        return 1;
                    }
                }
            }

            if bcheckup != -1 {
                // Try adjusting down.
                up = vec::scale(up, -1.0);
                let ext = cylinder_extent(svc, this);
                trace_line(svc, level, this, &mut hit, vec::add(loc, up), loc, ext);
                if hit.time == 1.0 {
                    let ext = cylinder_extent(svc, this);
                    trace_line(
                        svc,
                        level,
                        this,
                        &mut hit,
                        vec::add(vec::add(loc, up), out),
                        vec::add(loc, up),
                        ext,
                    );
                    if hit.time == 1.0 {
                        p.destination = vec::add(loc, up);
                        return 1;
                    }
                }
            }
        }

        0
    }
}

/// `APawn::GetNetPriority(Recent, Time, Lag)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `recent` must be null or a live actor.
pub unsafe fn get_net_priority(pawn: *mut APawn, recent: *mut AActor, time: f32, lag: f32) -> f32 {
    unsafe {
        let p = &*pawn;
        let mut time = time;
        if p.b_is_player()
            && !recent.is_null()
            && !(*recent).b_net_owner()
            && p.weapon == (*recent.cast::<APawn>()).weapon
            && (*recent).b_hidden() == p.base.b_hidden()
            && p.base.physics == PHYS_WALKING
        {
            // LocationError = ((Recent->Location + (Time + 0.5f*Lag)*Recent->Velocity)
            //                  - (Location + 0.5f*Lag*Velocity)).Size();
            let rloc = (*recent).location;
            let rvel = (*recent).velocity;
            let s = 0.5f32.mul_add(lag, time);
            let h = 0.5f32 * lag;
            let a = FVector {
                x: s.mul_add(rvel.x, rloc.x),
                y: s.mul_add(rvel.y, rloc.y),
                z: s.mul_add(rvel.z, rloc.z),
            };
            let b = FVector {
                x: h.mul_add(p.base.velocity.x, p.base.location.x),
                y: h.mul_add(p.base.velocity.y, p.base.location.y),
                z: h.mul_add(p.base.velocity.z, p.base.location.z),
            };
            let location_error = vec::size(vec::sub(a, b));
            let max_velocity = p.ground_speed;
            // Time = Time*0.5f + 2.f*LocationError / MaxVelocity
            time = time.mul_add(0.5, 2.0 * location_error / max_velocity);
        }
        p.base.net_priority * time
    }
}

/// `APawn::HandleSpecial(AActor *&bestPath)`. `best_path` is the C++ reference.
///
/// `*best_path` is dereferenced unconditionally by `eventSpecialHandling`, so a null
/// `*best_path` is UB in the original (null-deref in the event call). It is passed through.
///
/// # Safety
/// `pawn` and `best_path` must be valid; `*best_path` must be a valid actor.
pub unsafe fn handle_special(
    svc: &Hp2PawnSenseServices,
    pawn: *mut APawn,
    best_path: *mut *mut AActor,
) {
    unsafe {
        let real_path = *best_path;
        let mut new_path =
            (svc.event_special_handling)((*best_path).cast(), pawn.cast()).cast::<AActor>();
        *best_path = new_path;
        if new_path.is_null() {
            return;
        }
        if !(*best_path).is_null() && *best_path != real_path {
            if !(*pawn).b_can_do_special() {
                *best_path = std::ptr::null_mut();
                return;
            }
            (*pawn).special_goal = (*best_path).cast();
            if (svc.actor_reachable)(pawn.cast(), (*best_path).cast(), 0) != 0 {
                if (svc.is_probing)((*best_path).cast(), NAME_SPECIAL_HANDLING) != 0 {
                    let returned = (svc.event_special_handling)((*best_path).cast(), pawn.cast())
                        .cast::<AActor>();
                    if returned.is_null() {
                        *best_path = std::ptr::null_mut();
                    } else if *best_path != returned {
                        if *best_path != real_path
                            && (svc.actor_reachable)(pawn.cast(), returned.cast(), 0) != 0
                        {
                            *best_path = returned;
                        } else {
                            *best_path = std::ptr::null_mut();
                        }
                    }
                }
            } else {
                let success = (svc.find_path_toward)(
                    pawn.cast(),
                    (*best_path).cast(),
                    0,
                    (&mut new_path as *mut *mut AActor).cast(),
                    1,
                );
                if success == 0 || new_path == real_path {
                    *best_path = std::ptr::null_mut();
                } else {
                    (*pawn).special_goal = (*best_path).cast();
                    *best_path = new_path;
                }
            }
        }
    }
}
