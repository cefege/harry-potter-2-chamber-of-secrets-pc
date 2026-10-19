//! C ABI for the `FCollisionHash` port (`hp2_engine::actcol`). The C++ shell in
//! `UnActCol.cpp` owns the returned handle and frees it in its destructor. The
//! services table `Hp2ActColServices` is mirrored in `Hp2ActColServices.h`.
//!
//! Every engine object, vector, and rotation crosses as `void*` (same
//! convention as `geometry.rs`), so cbindgen never emits C++-only types.
//!
//! Null policy: a null handle or services pointer makes the export return its
//! default (no hits, or nothing) without touching memory.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FRotator};
use hp2_engine::actcol::{CollisionHash, Hp2ActColServices};
use hp2_ffi_support::guard;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_new(svc: *const Hp2ActColServices) -> *mut c_void {
    guard("hp2_rs_actcol_new", std::ptr::null_mut(), move || unsafe {
        match svc.as_ref() {
            Some(s) => Box::into_raw(Box::new(CollisionHash::new(s))).cast(),
            None => std::ptr::null_mut(),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_free(hash: *mut c_void) {
    guard("hp2_rs_actcol_free", (), move || unsafe {
        if !hash.is_null() {
            drop(Box::from_raw(hash.cast::<CollisionHash>()));
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_tick(hash: *mut c_void) {
    guard("hp2_rs_actcol_tick", (), move || unsafe {
        if let Some(h) = hash.cast::<CollisionHash>().as_mut() {
            h.tick();
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_add_actor(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    actor: *mut c_void,
) {
    guard("hp2_rs_actcol_add_actor", (), move || unsafe {
        if let (Some(h), Some(s)) = (hash.cast::<CollisionHash>().as_mut(), svc.as_ref()) {
            h.add_actor(s, actor.cast::<AActor>());
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_remove_actor(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    actor: *mut c_void,
    b_thorough: i32,
) {
    guard("hp2_rs_actcol_remove_actor", (), move || unsafe {
        if let (Some(h), Some(s)) = (hash.cast::<CollisionHash>().as_mut(), svc.as_ref()) {
            h.remove_actor(s, actor.cast::<AActor>(), b_thorough != 0);
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_actor_point_check(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    location: *const c_void,
    extent: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_actcol_actor_point_check",
        std::ptr::null_mut(),
        move || unsafe {
            match (
                hash.cast::<CollisionHash>().as_mut(),
                svc.as_ref(),
                location.cast::<FVector>().as_ref(),
                extent.cast::<FVector>().as_ref(),
            ) {
                (Some(h), Some(s), Some(loc), Some(ext)) => {
                    h.actor_point_check(s, *loc, *ext).cast()
                }
                _ => std::ptr::null_mut(),
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_actor_radius_check(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    location: *const c_void,
    radius: f32,
) -> *mut c_void {
    guard(
        "hp2_rs_actcol_actor_radius_check",
        std::ptr::null_mut(),
        move || unsafe {
            match (
                hash.cast::<CollisionHash>().as_mut(),
                svc.as_ref(),
                location.cast::<FVector>().as_ref(),
            ) {
                (Some(h), Some(s), Some(loc)) => h.actor_radius_check(s, *loc, radius).cast(),
                _ => std::ptr::null_mut(),
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_actor_encroachment_check(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    actor: *mut c_void,
    location: *const c_void,
    rotation: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_actcol_actor_encroachment_check",
        std::ptr::null_mut(),
        move || unsafe {
            match (
                hash.cast::<CollisionHash>().as_mut(),
                svc.as_ref(),
                location.cast::<FVector>().as_ref(),
                rotation.cast::<FRotator>().as_ref(),
            ) {
                (Some(h), Some(s), Some(loc), Some(rot)) if !actor.is_null() => h
                    .actor_encroachment_check(s, actor.cast::<AActor>(), *loc, *rot)
                    .cast(),
                _ => std::ptr::null_mut(),
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_actcol_actor_line_check(
    hash: *mut c_void,
    svc: *const Hp2ActColServices,
    mem: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    extent: *const c_void,
    extra_node_flags: u32,
) -> *mut c_void {
    guard(
        "hp2_rs_actcol_actor_line_check",
        std::ptr::null_mut(),
        move || unsafe {
            match (
                hash.cast::<CollisionHash>().as_mut(),
                svc.as_ref(),
                end.cast::<FVector>().as_ref(),
                start.cast::<FVector>().as_ref(),
                extent.cast::<FVector>().as_ref(),
            ) {
                (Some(h), Some(s), Some(e), Some(st), Some(ex)) => h
                    .actor_line_check(s, mem, *e, *st, *ex, extra_node_flags)
                    .cast(),
                _ => std::ptr::null_mut(),
            }
        },
    )
}
