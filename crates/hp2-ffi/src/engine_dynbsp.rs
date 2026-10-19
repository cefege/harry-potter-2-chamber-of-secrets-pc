//! C ABI for the `FMovingBrushTracker` port (`hp2_engine::dynbsp`). The C++
//! shell in `UnDynBsp.cpp` owns the returned handle and frees it in its
//! destructor. The services table `Hp2DynBspServices` is mirrored in
//! `Hp2DynBspServices.h`.
//!
//! Null policy: a null handle or services pointer makes the export return its
//! default (no-op, false, or zero) without touching memory.

use std::ffi::c_void;

use hp2_engine::dynbsp::{CHost, DynBsp, Hp2DynBspServices};
use hp2_ffi_support::guard;

type Tracker = DynBsp<CHost>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_new(
    svc: *const Hp2DynBspServices,
    level: *mut c_void,
    model: *mut c_void,
) -> *mut c_void {
    guard("hp2_rs_dynbsp_new", std::ptr::null_mut(), move || unsafe {
        match svc.as_ref() {
            Some(_) => {
                let host = CHost::new(&*svc, level, model);
                Box::into_raw(Box::new(DynBsp::new(host))).cast()
            }
            None => std::ptr::null_mut(),
        }
    })
}

/// `~FMovingBrushTracker`: runs `Shutdown` and frees the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_free(tracker: *mut c_void) {
    guard("hp2_rs_dynbsp_free", (), move || unsafe {
        if !tracker.is_null() {
            let t: Tracker = *Box::from_raw(tracker.cast::<Tracker>());
            t.shutdown();
        }
    })
}

/// `FMovingBrushTracker::Flush(Actor)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_flush(tracker: *mut c_void, actor: *mut c_void) {
    guard("hp2_rs_dynbsp_flush", (), move || unsafe {
        if let Some(t) = tracker.cast::<Tracker>().as_mut() {
            t.flush(actor);
        }
    })
}

/// `FMovingBrushTracker::Update(Actor)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_update(tracker: *mut c_void, actor: *mut c_void) {
    guard("hp2_rs_dynbsp_update", (), move || unsafe {
        if let Some(t) = tracker.cast::<Tracker>().as_mut() {
            t.update(actor);
        }
    })
}

/// `FMovingBrushTracker::SurfIsDynamic(ISurf)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_surf_is_dynamic(tracker: *mut c_void, i_surf: i32) -> i32 {
    guard("hp2_rs_dynbsp_surf_is_dynamic", 0, move || unsafe {
        match tracker.cast::<Tracker>().as_ref() {
            Some(t) => t.surf_is_dynamic(i_surf) as i32,
            None => 0,
        }
    })
}

/// `FMovingBrushTracker::CountBytes(Ar)`: reports the four pool sizes through
/// `svc.archive_count_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dynbsp_count_bytes(
    tracker: *mut c_void,
    svc: *const Hp2DynBspServices,
    ar: *mut c_void,
) {
    guard("hp2_rs_dynbsp_count_bytes", (), move || unsafe {
        if let (Some(t), Some(s)) = (tracker.cast::<Tracker>().as_ref(), svc.as_ref()) {
            for (num, max) in t.count_bytes() {
                (s.archive_count_bytes)(ar, num, max);
            }
        }
    })
}
