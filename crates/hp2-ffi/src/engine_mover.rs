//! C ABI for the `AMover` port (`hp2_engine::mover`). The C++ shells in
//! `UnMover.cpp` call the base-class method first, then one of these exports.
//! The services table `Hp2MoverServices` is mirrored in `Hp2MoverServices.h`.
//!
//! Null policy: a null mover or services pointer makes the export return
//! without touching memory.

use std::ffi::c_void;

use hp2_abi::engine::generated::AMover;
use hp2_engine::mover::{self, Hp2MoverServices};
use hp2_ffi_support::guard;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_static_constructor(svc: *const Hp2MoverServices) {
    guard("hp2_rs_mover_static_constructor", (), move || unsafe {
        if let Some(s) = svc.as_ref() {
            mover::static_constructor(s);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_spawned(mv: *mut c_void, _svc: *const Hp2MoverServices) {
    guard("hp2_rs_mover_spawned", (), move || unsafe {
        if let Some(m) = mv.cast::<AMover>().as_mut() {
            mover::spawned(m);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_post_load(mv: *mut c_void, svc: *const Hp2MoverServices) {
    guard("hp2_rs_mover_post_load", (), move || unsafe {
        if let (Some(m), Some(s)) = (mv.cast::<AMover>().as_mut(), svc.as_ref()) {
            mover::post_load(m, s);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_post_edit_move(
    mv: *mut c_void,
    _svc: *const Hp2MoverServices,
) {
    guard("hp2_rs_mover_post_edit_move", (), move || unsafe {
        if let Some(m) = mv.cast::<AMover>().as_mut() {
            mover::post_edit_move(m);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_post_edit_change(
    mv: *mut c_void,
    _svc: *const Hp2MoverServices,
) {
    guard("hp2_rs_mover_post_edit_change", (), move || unsafe {
        if let Some(m) = mv.cast::<AMover>().as_mut() {
            mover::post_edit_change(m);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_pre_raytrace(mv: *mut c_void, _svc: *const Hp2MoverServices) {
    guard("hp2_rs_mover_pre_raytrace", (), move || unsafe {
        if let Some(m) = mv.cast::<AMover>().as_mut() {
            mover::pre_raytrace(m);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_set_world_raytrace_key(
    mv: *mut c_void,
    svc: *const Hp2MoverServices,
) {
    guard("hp2_rs_mover_set_world_raytrace_key", (), move || unsafe {
        if let (Some(m), Some(s)) = (mv.cast::<AMover>().as_mut(), svc.as_ref()) {
            mover::set_world_raytrace_key(m, s);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_set_brush_raytrace_key(
    mv: *mut c_void,
    svc: *const Hp2MoverServices,
) {
    guard("hp2_rs_mover_set_brush_raytrace_key", (), move || unsafe {
        if let (Some(m), Some(s)) = (mv.cast::<AMover>().as_mut(), svc.as_ref()) {
            mover::set_brush_raytrace_key(m, s);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_mover_post_raytrace(mv: *mut c_void, svc: *const Hp2MoverServices) {
    guard("hp2_rs_mover_post_raytrace", (), move || unsafe {
        if let (Some(m), Some(s)) = (mv.cast::<AMover>().as_mut(), svc.as_ref()) {
            mover::post_raytrace(m, s);
        }
    })
}
