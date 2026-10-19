//! Engine exports from `crates/hp2-engine/src/particle.rs` (`UnParticle.cpp`,
//! `UnParticleList.cpp`). Pointer arguments are `void*`; vectors are passed by
//! address. The list handle is an owned `ParticleList` created by
//! `hp2_rs_engine_particle_list_new` and freed by `..._list_free`.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{FBox, UParticle};
use hp2_engine::particle::{self, Hp2ParticleServices, ParticleList};
use hp2_ffi_support::guard;

/// Borrows the `Hp2ParticleServices` table the C++ shell passes; `None` if null.
///
/// # Safety
/// A non-null `svc` must point at a live `Hp2ParticleServices`.
unsafe fn services<'a>(svc: *const c_void) -> Option<&'a Hp2ParticleServices> {
    unsafe { svc.cast::<Hp2ParticleServices>().as_ref() }
}

/// Reads an `FVector` argument by address; `None` if null.
///
/// # Safety
/// A non-null `v` must point at a live `FVector`.
unsafe fn vec_arg(v: *const c_void) -> Option<FVector> {
    unsafe { v.cast::<FVector>().as_ref().copied() }
}

/// Borrows the `ParticleList` handle the C++ shell owns; `None` if null.
///
/// # Safety
/// A non-null `list` must come from `hp2_rs_engine_particle_list_new`.
unsafe fn list_mut<'a>(list: *mut c_void) -> Option<&'a mut ParticleList> {
    unsafe { list.cast::<ParticleList>().as_mut() }
}

/// `UParticle::Update(SystemGravity, SystemDamping, SystemWind, Level,
/// Timestep, System)`. Returns nonzero if the particle is still alive.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_particle_update(
    particle: *mut c_void,
    system_gravity: *const c_void,
    system_damping: f32,
    system_wind: *const c_void,
    level: *mut c_void,
    timestep: f32,
    system: *mut c_void,
    svc: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_particle_update", 0, move || unsafe {
        let (Some(p), Some(gravity), Some(wind), Some(svc)) = (
            particle.cast::<UParticle>().as_mut(),
            vec_arg(system_gravity),
            vec_arg(system_wind),
            services(svc),
        ) else {
            return 0;
        };
        if system.is_null() {
            return 0;
        }
        i32::from(particle::update(
            p,
            gravity,
            system_damping,
            wind,
            level,
            timestep,
            system,
            svc,
        ))
    })
}

/// Allocates an empty Rust particle list. Owned by one `UParticleListPriv`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_new() -> *mut c_void {
    guard("hp2_rs_engine_particle_list_new", ptr::null_mut(), || {
        Box::into_raw(Box::new(ParticleList::new())).cast()
    })
}

/// Frees a list from `hp2_rs_engine_particle_list_new`. Null is a no-op.
///
/// # Safety
/// `list` must be null or a live handle from `..._list_new`, freed once.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_free(list: *mut c_void) {
    guard("hp2_rs_engine_particle_list_free", (), move || unsafe {
        if !list.is_null() {
            drop(Box::from_raw(list.cast::<ParticleList>()));
        }
    })
}

/// `StartParticle`: rewinds and returns the first particle, or null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_start(list: *mut c_void) -> *mut c_void {
    guard(
        "hp2_rs_engine_particle_list_start",
        ptr::null_mut(),
        move || unsafe { list_mut(list).map_or(ptr::null_mut(), |l| l.start().cast()) },
    )
}

/// `NextParticle`: advances (unless the last call hit end) and returns the
/// current particle, or null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_next(list: *mut c_void) -> *mut c_void {
    guard(
        "hp2_rs_engine_particle_list_next",
        ptr::null_mut(),
        move || unsafe { list_mut(list).map_or(ptr::null_mut(), |l| l.next_particle().cast()) },
    )
}

/// `Add(UParticle&)`: appends a copy of `particle`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_add(list: *mut c_void, particle: *const c_void) {
    guard("hp2_rs_engine_particle_list_add", (), move || unsafe {
        if let (Some(l), Some(p)) = (list_mut(list), particle.cast::<UParticle>().as_ref()) {
            l.add(p);
        }
    })
}

/// `Remove(UParticle*)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_remove(list: *mut c_void, particle: *const c_void) {
    guard("hp2_rs_engine_particle_list_remove", (), move || unsafe {
        if let Some(l) = list_mut(list) {
            l.remove(particle.cast());
        }
    })
}

/// `RemoveCurrent`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_remove_current(list: *mut c_void) {
    guard(
        "hp2_rs_engine_particle_list_remove_current",
        (),
        move || unsafe {
            if let Some(l) = list_mut(list) {
                l.remove_current();
            }
        },
    )
}

/// `RemoveFromFront`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_remove_from_front(list: *mut c_void) {
    guard(
        "hp2_rs_engine_particle_list_remove_from_front",
        (),
        move || unsafe {
            if let Some(l) = list_mut(list) {
                l.remove_from_front();
            }
        },
    )
}

/// `RemoveAll`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_remove_all(list: *mut c_void) {
    guard(
        "hp2_rs_engine_particle_list_remove_all",
        (),
        move || unsafe {
            if let Some(l) = list_mut(list) {
                l.remove_all();
            }
        },
    )
}

/// `Size()`: number of particles.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_size(list: *mut c_void) -> i32 {
    guard("hp2_rs_engine_particle_list_size", 0, move || unsafe {
        list_mut(list).map_or(0, |l| l.size() as i32)
    })
}

/// `GetParticle(Id)`: first particle with a matching `Id`, else null.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_get_particle(
    list: *mut c_void,
    id: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_particle_list_get_particle",
        ptr::null_mut(),
        move || unsafe { list_mut(list).map_or(ptr::null_mut(), |l| l.get_particle(id).cast()) },
    )
}

/// `UpdateBox`: writes the rebuilt world bounds to `world_box` (`FBox*`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_list_update_box(
    list: *mut c_void,
    world_box: *mut c_void,
) {
    guard(
        "hp2_rs_engine_particle_list_update_box",
        (),
        move || unsafe {
            if let (Some(l), Some(b)) = (list_mut(list), world_box.cast::<FBox>().as_mut()) {
                l.update_box(b);
            }
        },
    )
}
