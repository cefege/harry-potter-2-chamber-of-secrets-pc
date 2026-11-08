//! Port of the move primitives in `Engine/Src/UnPawn.cpp`: `APawn::walkMove`,
//! `flyMove` and `swimMove`.
//!
//! Engine calls made by these bodies (`GetLevel()->MoveActor/FarMoveActor`,
//! `APawn::findWaterLine`) go through [`Hp2PawnServices`], filled in by
//! `HP2PawnServices.cpp`. The C++ shells pin `Test = 1` and `IgnorePawns = 1`
//! for `MoveActor` and `noCheck = 1` for `FarMoveActor`.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AZoneInfo, FCheckResult};

use crate::native_guard::guarded;
use crate::vec;

/// Checked native status: zero succeeds; nonzero preserves the pending native error.
pub type PawnResult<T> = Result<T, i32>;

pub(crate) fn checked(status: i32) -> PawnResult<()> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/// Native stack scope. The callback is synchronous and called exactly once.
pub type PawnHitBody = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnServices {
    pub move_actor: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut c_void,
        *mut i32,
    ) -> i32,
    pub far_move_actor:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut i32) -> i32,
    pub find_water_line: unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> i32,
    pub with_hit: unsafe extern "C" fn(*mut c_void, PawnHitBody) -> i32,
}

/// Keep the real C++ FCheckResult on its native stack, accessing only initialized
/// named fields through raw pointers. Padding is neither copied nor read in Rust.
pub(crate) unsafe fn with_hit<T, F>(svc: &Hp2PawnServices, body: F) -> PawnResult<T>
where
    F: FnOnce(*mut FCheckResult) -> PawnResult<T>,
{
    struct Scope<F, T> {
        body: Option<F>,
        value: Option<T>,
    }
    unsafe extern "C" fn run<T, F>(context: *mut c_void, hit: *mut c_void) -> i32
    where
        F: FnOnce(*mut FCheckResult) -> PawnResult<T>,
    {
        // The context is Rust-owned and inaccessible to engine callbacks.
        let scope = unsafe { &mut *context.cast::<Scope<F, T>>() };
        let Some(body) = scope.body.take() else {
            return -1;
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(hit.cast()))) {
            Ok(Ok(value)) => {
                scope.value = Some(value);
                0
            }
            Ok(Err(status)) => status,
            Err(_) => -1,
        }
    }
    let mut scope = Scope {
        body: Some(body),
        value: None,
    };
    checked(unsafe { (svc.with_hit)((&mut scope as *mut Scope<F, T>).cast(), run::<T, F>) })?;
    scope.value.ok_or(-1)
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

    /// Native movement owns collision and all public guard layers.
    unsafe fn move_by(&self, delta: FVector, hit: *mut FCheckResult) -> PawnResult<i32> {
        unsafe {
            let mut out = 0;
            checked((self.svc.move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&delta as *const FVector).cast(),
                std::ptr::addr_of!((*self.pawn).base.rotation).cast(),
                hit.cast(),
                &mut out,
            ))?;
            Ok(out)
        }
    }

    unsafe fn far_move(&self, dest: FVector) -> PawnResult<()> {
        unsafe {
            let mut out = 0;
            checked((self.svc.far_move_actor)(
                self.x_level(),
                self.pawn.cast(),
                (&dest as *const FVector).cast(),
                &mut out,
            ))
        }
    }
}

/// `Hit.Actor == GoalActor` with the C++ `GoalActor &&` guard.
unsafe fn hit_goal(hit: *mut FCheckResult, goal: *mut AActor) -> bool {
    !goal.is_null() && unsafe { (*hit).actor == goal.cast() }
}

/// `APawn::walkMove(Delta, Hit, GoalActor, threshold, bAdjust)`, under its
/// original `guard(APawn::walkMove)`.
///
/// # Safety
/// `pawn` must point to a live `APawn` whose `XLevel` and `Region.Zone` are valid
/// for the services in `svc`; `hit` must be a valid `FCheckResult`.
pub unsafe fn walk_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    delta: FVector,
    hit: *mut FCheckResult,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::walkMove", || {
            walk_move_body(svc, pawn, delta, hit, goal, threshold, adjust)
        })
    }
}

unsafe fn walk_move_body(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    hit: *mut FCheckResult,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
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

        m.move_by(delta, hit)?;
        if hit_goal(hit, goal) {
            return Ok(5); // bumped into goal
        }
        if (*hit).time < 1.0 {
            // try to step up
            delta = vec::scale(delta, 1.0 - (*hit).time);
            m.move_by(up, hit)?;
            m.move_by(delta, hit)?;
            if hit_goal(hit, goal) {
                return Ok(5);
            }
            m.move_by(down, hit)?;
            if hit_goal(hit, goal) {
                return Ok(5);
            }
            // Scouts want only good floors, else undo move
            if (*hit).time < 1.0 && (*hit).normal.z < 0.7f32 {
                m.far_move(start)?;
                return Ok(0);
            }
        }

        // drop to floor
        let loc = m.location();
        down = vec::scale(grav_dir, (*pawn).max_step_height + 2.0);
        m.move_by(down, hit)?;
        if (*hit).time == 1.0 {
            // then falling
            if adjust != 0 {
                m.far_move(start)?;
            } else {
                m.far_move(loc)?;
            }
            return Ok(-1);
        } else if ((*hit).normal.z as f64) < 0.7f64 {
            m.far_move(start)?;
            return Ok(-1);
        }

        // check if move successful
        let real = vec::sub(m.location(), start);
        if vec::size_squared(real) < threshold * threshold {
            if adjust != 0 {
                m.far_move(start)?;
            }
            return Ok(0);
        }
        Ok(1)
    }
}

/// `APawn::flyMove(Delta, GoalActor, threshold, bAdjust)`, under its original
/// `guard(APawn::flyMove)`.
///
/// # Safety
/// Same requirements as [`walk_move`].
pub unsafe fn fly_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::flyMove", || fly_move_body(svc, pawn, delta, goal, threshold, adjust))
    }
}

unsafe fn fly_move_body(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
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
        with_hit(svc, |hit| {
            m.move_by(delta, hit)?;
            if hit_goal(hit, goal) {
                return Ok(5);
            }
            if (*hit).time < 1.0 {
                // try to step up
                delta = vec::scale(delta, 1.0 - (*hit).time);
                m.move_by(up, hit)?;
                m.move_by(delta, hit)?;
                if hit_goal(hit, goal) {
                    return Ok(5);
                }
            }

            let real = vec::sub(m.location(), start);
            if vec::size_squared(real) < threshold * threshold {
                if adjust != 0 {
                    m.far_move(start)?;
                }
                result = 0;
            }
            Ok(result)
        })
    }
}

/// `APawn::swimMove(Delta, GoalActor, threshold, bAdjust)`, under its original
/// `guard(APawn::swimMove)`.
///
/// # Safety
/// Same requirements as [`walk_move`].
pub unsafe fn swim_move(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
    unsafe {
        guarded(c"APawn::swimMove", || swim_move_body(svc, pawn, delta, goal, threshold, adjust))
    }
}

unsafe fn swim_move_body(
    svc: &Hp2PawnServices,
    pawn: *mut APawn,
    mut delta: FVector,
    goal: *mut AActor,
    threshold: f32,
    adjust: i32,
) -> PawnResult<i32> {
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
        with_hit(svc, |hit| {
            m.move_by(delta, hit)?;
            if hit_goal(hit, goal) {
                return Ok(5);
            }
            let zone = (*pawn).base.region.zone.cast::<AZoneInfo>();
            if !(*zone).b_water_zone() {
                let mut end = m.location();
                checked((svc.find_water_line)(
                    pawn.cast(),
                    (&start as *const FVector).cast(),
                    (&mut end as *mut FVector).cast(),
                ))?;
                let here = m.location();
                if end != here {
                    m.move_by(vec::sub(end, here), hit)?;
                }
                return Ok(0);
            } else if (*hit).time < 1.0 {
                // try to step up
                delta = vec::scale(delta, 1.0 - (*hit).time);
                m.move_by(up, hit)?;
                m.move_by(delta, hit)?;
                if hit_goal(hit, goal) {
                    return Ok(5);
                }
            }

            let real = vec::sub(m.location(), start);
            if vec::size_squared(real) < threshold * threshold {
                if adjust != 0 {
                    m.far_move(start)?;
                }
                result = 0;
            }
            Ok(result)
        })
    }
}
