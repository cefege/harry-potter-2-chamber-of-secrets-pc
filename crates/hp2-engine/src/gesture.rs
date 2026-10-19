//! Port of `Engine/Src/UnGesture.cpp`: point-cloud matching
//! (`InternalGestureCompare2`, `InternalPointCompare`) and the bodies of
//! `UGesture::execCompareGesturePoint` and `UGesture::execCompareGesture`.
//!
//! Deleted dead code (user decision): `PlaceSegment`, `InternalGestureCompare`
//! and its `DRAWN_SECTORS` buffer. Nothing compiled calls them. The only call
//! to `InternalGestureCompare` is commented out in `UnGesture.cpp` (original
//! line 474), and `PlaceSegment` is reached only from `InternalGestureCompare`.
//! Their undefined behaviour (out-of-bounds reads of the drawn and adjusted
//! sector arrays) goes with them.
//!
//! FMA: the original arm64 product build fuses `dx * dx + dy * dy` into
//! `fmadd(dx, dx, dy * dy)` (objdump of `UnGesture.o`: 2 sites in
//! `InternalGestureCompare2`, 1 in `InternalPointCompare`, 1 inlined copy in
//! `execCompareGesturePoint`). `point_distance` reproduces that. No other
//! expression in the file is fused; the lerps in the exec bodies are plain
//! multiply/divide/add.
//!
//! Undefined behaviour in the original, and the port's choice:
//! - `TargetMousePointArray[1024]` is indexed `8k+j` for `k < n`, so `n > 128`
//!   writes past the end. The port sizes the target buffer to `8n`. The
//!   exec fixtures stay at `n <= 128`.
//! - Int conversions of out-of-range floats saturate (arm64 `fcvtzs`, Rust
//!   `as`); NaN converts to 0 in both. Int accumulators use wrapping ops.

use hp2_abi::FVector;

/// `MAX_MOUSE_POINTS` in `UnGesture.cpp`.
pub const MAX_MOUSE_POINTS: usize = 1024;

/// `CMousePoint` from `UnGesture.cpp`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CMousePoint {
    pub x_pos: f32,
    pub y_pos: f32,
}

/// Distance of one point pair as the original computes it:
/// `dVal = appSqrt(dx * dx + dy * dy)`, with the sum fused as
/// `fmadd(dx, dx, dy * dy)` and `appSqrt(DOUBLE)` = `sqrt` in double, then
/// stored to `float`.
#[inline]
fn point_distance(dx: f32, dy: f32) -> f32 {
    let sum = dx.mul_add(dx, dy * dy);
    f64::from(sum).sqrt() as f32
}

/// `InternalGestureCompare2(MouseArray, iNumMousePoints, nGesturePoints,
/// iNumGesturePoints, fAccuracy)`: nearest-point error in both directions.
pub fn internal_gesture_compare2(
    mouse: &[CMousePoint],
    gesture: &[CMousePoint],
    accuracy: f32,
) -> f32 {
    let mut num_errors: i32 = 0;
    let mut total: i32 = 0;

    for g in gesture {
        let mut min_val: f32 = 1.0;
        for m in mouse {
            let d_val = point_distance(m.x_pos - g.x_pos, m.y_pos - g.y_pos);
            if d_val < min_val {
                min_val = d_val;
            }
        }
        if min_val > accuracy {
            // `1 + (dMinVal / (fAccuracy * 5))`, float to int.
            let mut error = (1.0 + min_val / (accuracy * 5.0)) as i32;
            if error > 2 {
                error = 2;
            }
            num_errors = num_errors.wrapping_add(error);
            total = total.wrapping_add(error);
        } else {
            total = total.wrapping_add(2);
        }
    }

    for m in mouse {
        let mut min_val: f32 = 1.0;
        for g in gesture {
            let d_val = point_distance(m.x_pos - g.x_pos, m.y_pos - g.y_pos);
            if d_val < min_val {
                min_val = d_val;
            }
        }
        if min_val > accuracy {
            // `dMinVal / fAccuracy`, float to int.
            let mut error = (min_val / accuracy) as i32;
            if error > 3 {
                error = 3;
            }
            num_errors = num_errors.wrapping_add(error);
            total = total.wrapping_add(error);
        }
    }

    // Minimum number of points.
    if mouse.len() < 10 {
        total = total.wrapping_add(500);
        num_errors = num_errors.wrapping_add(500);
    }

    // `fCorrect` is a float; the clamp reads that rounded value.
    let correct = (1.0 - f64::from(num_errors) / f64::from(total)) as f32;
    if f64::from(correct) < 0.0 {
        0.0
    } else if f64::from(correct) > 1.0 {
        1.0
    } else {
        correct
    }
}

/// `InternalPointCompare(MousePoint, nGesturePoints, iNumGesturePoints, fAccuracy)`.
/// The original never reads `fAccuracy`, so the port omits it.
pub fn internal_point_compare(point: CMousePoint, gesture: &[CMousePoint]) -> f32 {
    let mut min_val: f32 = 1.0;
    for g in gesture {
        let d_val = point_distance(point.x_pos - g.x_pos, point.y_pos - g.y_pos);
        if d_val < min_val {
            min_val = d_val;
        }
    }
    min_val
}

/// How one of the 7 extra resolution points between two `Points` is computed.
#[derive(Clone, Copy)]
enum Lerp {
    /// `((to - from) / 8) + from`, all in `float`.
    Float8,
    /// `((to - from) * mul / div) + from`, with the difference promoted to `double`.
    Double { mul: f64, div: f64 },
}

/// Extra-point formulas for `execCompareGesturePoint`, slots `8k+1`..`8k+7`.
const POINT_LERPS: [Lerp; 7] = [
    Lerp::Float8,
    Lerp::Double { mul: 2.0, div: 8.0 },
    Lerp::Double { mul: 3.0, div: 8.0 },
    Lerp::Double { mul: 4.0, div: 8.0 },
    Lerp::Double { mul: 5.0, div: 8.0 },
    Lerp::Double { mul: 6.0, div: 8.0 },
    Lerp::Double { mul: 7.0, div: 8.0 },
];

/// Extra-point formulas for `execCompareGesture`, slots `8k+1`..`8k+7`.
const GESTURE_LERPS: [Lerp; 7] = [
    Lerp::Double { mul: 1.0, div: 8.0 },
    Lerp::Double { mul: 2.0, div: 8.0 },
    Lerp::Double { mul: 3.0, div: 8.0 },
    Lerp::Double { mul: 4.0, div: 8.0 },
    Lerp::Double { mul: 5.0, div: 8.0 },
    Lerp::Double { mul: 3.0, div: 4.0 },
    Lerp::Double { mul: 7.0, div: 8.0 },
];

fn lerp_component(lerp: Lerp, from: f32, to: f32) -> f32 {
    match lerp {
        Lerp::Float8 => (to - from) / 8.0 + from,
        Lerp::Double { mul, div } => ((f64::from(to - from) * mul / div) + f64::from(from)) as f32,
    }
}

/// Expands the first `min(len, MAX_MOUSE_POINTS)` gesture points into the
/// `8n`-slot target. The original `TargetMousePointArray` is indexed `8k+j`
/// only for `k < n`, so every slot read later is written.
fn expand_target(points: &[FVector], lerps: &[Lerp; 7]) -> Vec<CMousePoint> {
    let n = points.len().min(MAX_MOUSE_POINTS);
    let mut target = vec![CMousePoint::default(); 8 * n];
    for k in 0..n {
        let from = points[k];
        target[8 * k] = CMousePoint {
            x_pos: from.x,
            y_pos: from.y,
        };
        if k + 1 < n {
            let to = points[k + 1];
            for (slot, &lerp) in lerps.iter().enumerate() {
                target[8 * k + 1 + slot] = CMousePoint {
                    x_pos: lerp_component(lerp, from.x, to.x),
                    y_pos: lerp_component(lerp, from.y, to.y),
                };
            }
        }
    }
    target
}

/// `nNumTargetMousePoints = n * 8 - 7`, floored at 0 (the loops then do nothing).
fn target_count(n: usize) -> usize {
    (8 * n).saturating_sub(7)
}

/// `UGesture::execCompareGesturePoint` body: distance from one point to the
/// expanded gesture.
pub fn compare_gesture_point(points: &[FVector], in_point: FVector) -> f32 {
    let target = expand_target(points, &POINT_LERPS);
    let count = target_count(points.len().min(MAX_MOUSE_POINTS));
    internal_point_compare(
        CMousePoint {
            x_pos: in_point.x,
            y_pos: in_point.y,
        },
        &target[..count],
    )
}

/// `UGesture::execCompareGesture` body: dedupes the drawn points, then scores
/// them against the expanded gesture.
pub fn compare_gesture(points: &[FVector], in_points: &[FVector], accuracy: f32) -> f32 {
    let mut mouse: Vec<CMousePoint> = Vec::new();
    for p in &in_points[..in_points.len().min(MAX_MOUSE_POINTS)] {
        // Z == -1 marks a point as unused.
        if p.z == -1.0 {
            continue;
        }
        let used_before = mouse.iter().any(|m| {
            f64::from((m.x_pos - p.x).abs()) < 0.001 && f64::from((m.y_pos - p.y).abs()) < 0.001
        });
        if !used_before {
            mouse.push(CMousePoint {
                x_pos: p.x,
                y_pos: p.y,
            });
        }
    }

    let target = expand_target(points, &GESTURE_LERPS);
    let count = target_count(points.len().min(MAX_MOUSE_POINTS));
    internal_gesture_compare2(&mouse, &target[..count], accuracy)
}
