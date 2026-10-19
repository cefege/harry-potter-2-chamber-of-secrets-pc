//! Engine exports from `crates/hp2-engine/src/gesture.rs` (`UnGesture.cpp`).
//! The gesture is passed as `UGesture*`; `TArray<FVector>` arguments are
//! passed by address. The point matcher ignores `fAccuracy`, so the C++ shell
//! does not forward it.

use std::ffi::c_void;
use std::ptr;
use std::slice;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{TArrayRaw, UGesture};
use hp2_engine::gesture;
use hp2_ffi_support::guard;

/// Borrows the elements of a `TArray<FVector>`; empty if null or nonpositive.
///
/// # Safety
/// A non-null `data` with positive `num` must point at `num` live `FVector`s.
unsafe fn fvector_array<'a>(array: TArrayRaw) -> &'a [FVector] {
    if array.data.is_null() || array.num <= 0 {
        return &[];
    }
    unsafe { slice::from_raw_parts(array.data.cast::<FVector>(), array.num as usize) }
}

/// Reads the `TArray<FVector>` at `array` by address; `None` if null.
///
/// # Safety
/// A non-null `array` must point at a live `TArray<FVector>`.
unsafe fn tarray_arg<'a>(array: *const c_void) -> Option<&'a [FVector]> {
    let raw = array.cast::<TArrayRaw>();
    if raw.is_null() {
        return None;
    }
    Some(unsafe { fvector_array(raw.read_unaligned()) })
}

/// `UGesture::execCompareGesturePoint` body: returns the distance of
/// `in_point` from the expanded gesture points.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_gesture_compare_gesture_point(
    gesture: *const c_void,
    in_point: *const c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_gesture_compare_gesture_point",
        0.0,
        move || unsafe {
            let (Some(g), Some(point)) = (
                gesture.cast::<UGesture>().as_ref(),
                in_point.cast::<FVector>().as_ref(),
            ) else {
                return 0.0;
            };
            let points = fvector_array(ptr::addr_of!(g.points).read_unaligned());
            gesture::compare_gesture_point(points, *point)
        },
    )
}

/// `UGesture::execCompareGesture` body: scores the drawn mouse points against
/// the gesture's expanded points.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_gesture_compare_gesture(
    gesture: *const c_void,
    in_points: *const c_void,
    accuracy: f32,
) -> f32 {
    guard(
        "hp2_rs_engine_gesture_compare_gesture",
        0.0,
        move || unsafe {
            let (Some(g), Some(mouse)) =
                (gesture.cast::<UGesture>().as_ref(), tarray_arg(in_points))
            else {
                return 0.0;
            };
            let points = fvector_array(ptr::addr_of!(g.points).read_unaligned());
            gesture::compare_gesture(points, mouse, accuracy)
        },
    )
}
