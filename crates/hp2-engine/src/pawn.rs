//! Port of the move primitives in `Engine/Src/UnPawn.cpp`: `APawn::walkMove`,
//! `flyMove` and `swimMove`.
//!
//! Engine calls made by these bodies (`GetLevel()->MoveActor/FarMoveActor`,
//! `APawn::findWaterLine`) go through [`Hp2PawnServices`], filled in by
//! `HP2PawnServices.cpp`. The C++ shells pin `Test = 1` and `IgnorePawns = 1`
//! for `MoveActor` and `noCheck = 1` for `FarMoveActor`.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, FRotator};

use crate::vec;

/// Engine services used by the pawn move primitives.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnServices {
    /// `ULevel::MoveActor(actor, delta, rot, hit, 1, 1)`; returns the engine's `UBOOL`.
    /// `delta`, `rot` and `hit` point at `FVector`, `FRotator` and `FCheckResult`.
    pub move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        delta: *const c_void,
        rot: *const c_void,
        hit: *mut c_void,
    ) -> i32,
    /// `ULevel::FarMoveActor(actor, dest, 1, 1)`; `dest` points at an `FVector`.
    pub far_move_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, dest: *const c_void) -> i32,
    /// `APawn::findWaterLine(start, end)`; `start` and `end` point at `FVector`.
    pub find_water_line:
        unsafe extern "C" fn(pawn: *mut c_void, start: *const c_void, end: *mut c_void),
}

/// Subject pawn plus the services it calls. Keeps the three move bodies below
/// short: `level` is `XLevel` and `pawn` is the `this` pointer.
struct Mover<'a> {
    svc: &'a Hp2PawnServices,
    pawn: *mut APawn,
}

impl Mover<'_> {
    unsafe fn x_level(&self) -> *mut c_void {
        unsafe { (*self.pawn).base.x_level }
    }

    unsafe fn location(&self) -> FVector {
        unsafe { (*self.pawn).base.location }
    }

    /// `GetLevel()->MoveActor(this, delta, Rotation, hit, 1, 1)`.
    unsafe fn move_by(&self, delta: FVector, hit: &mut FCheckResult) -> i32 {
        unsafe {
            let rot = &(*self.pawn).base.rotation as *const FRotator;
            (self.svc.move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&delta as *const FVector).cast(),
                rot.cast(),
                (hit as *mut FCheckResult).cast(),
            )
        }
    }

    /// `GetLevel()->FarMoveActor(this, dest, 1, 1)`.
    unsafe fn far_move(&self, dest: FVector) {
        unsafe {
            (self.svc.far_move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&dest as *const FVector).cast(),
            );
        }
    }
}

/// `FCheckResult(1.f)` (UnPrim.h): null actor and primitive, zero location and normal, time 1, item INDEX_NONE.
pub(crate) fn fresh_hit() -> FCheckResult {
    // SAFETY: `FCheckResult` is plain old data; all-zero is a valid bit pattern.
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.0;
    hit.item = -1; // INDEX_NONE, as in the FCheckResult(FLOAT) ctor
    hit
}

/// `Hit.Actor == GoalActor` with the C++ `GoalActor &&` guard.
fn hit_goal(hit: &FCheckResult, goal: *mut AActor) -> bool {
    !goal.is_null() && hit.actor == goal.cast()
}

/// `APawn::walkMove(Delta, Hit, GoalActor, threshold, bAdjust)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` whose `XLevel` and `Region.Zone` are valid
/// for the services in `svc`; `hit` must be a valid `FCheckResult`.
pub unsafe fn walk_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    hit: &mut FCheckResult,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> i32 {
    let m = Mover { svc, pawn };
    unsafe {
        let start = m.location();
        delta.z = 0.0;
        // Perform the move
        let zone = (*pawn).base.region.zone.cast::<AZoneInfo>();
        let mut grav_dir = FVector {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        if (*zone).zone_gravity.z > 0.0 {
            grav_dir.z = 1.0;
        }
        let max_step = (*pawn).max_step_height;
        let mut down = vec::scale(grav_dir, max_step);
        let up = vec::scale(down, -1.0);

        m.move_by(delta, hit);
        if hit_goal(hit, goal) {
            return 5; // bumped into goal
        }
        if hit.time < 1.0 {
            // try to step up
            delta = vec::scale(delta, 1.0 - hit.time);
            m.move_by(up, hit);
            m.move_by(delta, hit);
            if hit_goal(hit, goal) {
                return 5;
            }
            m.move_by(down, hit);
            if hit_goal(hit, goal) {
                return 5;
            }
            // Scouts want only good floors, else undo move
            if hit.time < 1.0 && hit.normal.z < 0.7f32 {
                m.far_move(start);
                return 0;
            }
        }

        // drop to floor
        let loc = m.location();
        down = vec::scale(grav_dir, max_step + 2.0);
        m.move_by(down, hit);
        if hit.time == 1.0 {
            // then falling
            if adjust != 0 {
                m.far_move(start);
            } else {
                m.far_move(loc);
            }
            return -1;
        } else if (hit.normal.z as f64) < 0.7f64 {
            m.far_move(start);
            return -1;
        }

        // check if move successful
        let real = vec::sub(m.location(), start);
        if vec::size_squared(real) < threshold * threshold {
            if adjust != 0 {
                m.far_move(start);
            }
            return 0;
        }
        1
    }
}

/// `APawn::flyMove(Delta, GoalActor, threshold, bAdjust)`.
///
/// # Safety
/// Same requirements as [`walk_move`].
pub unsafe fn fly_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> i32 {
    let m = Mover { svc, pawn };
    unsafe {
        let mut result = 1;
        let start = m.location();
        let down = vec::scale(
            FVector {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            (*pawn).max_step_height,
        );
        let up = vec::scale(down, -1.0);
        let mut hit = fresh_hit();

        m.move_by(delta, &mut hit);
        if hit_goal(&hit, goal) {
            return 5;
        }
        if hit.time < 1.0 {
            // try to step up
            delta = vec::scale(delta, 1.0 - hit.time);
            m.move_by(up, &mut hit);
            m.move_by(delta, &mut hit);
            if hit_goal(&hit, goal) {
                return 5;
            }
        }

        let real = vec::sub(m.location(), start);
        if vec::size_squared(real) < threshold * threshold {
            if adjust != 0 {
                m.far_move(start);
            }
            result = 0;
        }
        result
    }
}

/// `APawn::swimMove(Delta, GoalActor, threshold, bAdjust)`.
///
/// # Safety
/// Same requirements as [`walk_move`].
pub unsafe fn swim_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> i32 {
    let m = Mover { svc, pawn };
    unsafe {
        let mut result = 1;
        let start = m.location();
        let down = vec::scale(
            FVector {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            (*pawn).max_step_height,
        );
        let up = vec::scale(down, -1.0);
        let mut hit = fresh_hit();

        m.move_by(delta, &mut hit);
        if hit_goal(&hit, goal) {
            return 5;
        }
        let zone = (*pawn).base.region.zone.cast::<AZoneInfo>();
        if !(*zone).b_water_zone() {
            let mut end = m.location();
            (svc.find_water_line)(
                pawn.cast(),
                (&start as *const FVector).cast(),
                (&mut end as *mut FVector).cast(),
            );
            let here = m.location();
            if end != here {
                m.move_by(vec::sub(end, here), &mut hit);
            }
            return 0;
        } else if hit.time < 1.0 {
            // try to step up
            delta = vec::scale(delta, 1.0 - hit.time);
            m.move_by(up, &mut hit);
            m.move_by(delta, &mut hit);
            if hit_goal(&hit, goal) {
                return 5;
            }
        }

        let real = vec::sub(m.location(), start);
        if vec::size_squared(real) < threshold * threshold {
            if adjust != 0 {
                m.far_move(start);
            }
            result = 0;
        }
        result
    }
}
