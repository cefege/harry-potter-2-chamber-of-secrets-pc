//! Tube and poly-strip particle geometry exports (`hp2-render::particle_tube`).
//! The C++ shells in `UnParticleRn.cpp` keep the `UParticle` members and the
//! `pPrevParticle` pointer, and forward `SetupVerts` here. Particle records
//! cross as `TubeParticle` (`#[repr(C)]`, 20 bytes; C++ mirror asserts the
//! size). `FCoords`, `FVector`, and `FTransTexture` cross as `void*` (hp2-abi
//! mirrors, not parsed by cbindgen). C++ owns the vertex buffer and the
//! previous-strip state, so Rust writes into them in place.

use std::ffi::c_void;

use hp2_abi::render_particle::FTransTexture;
use hp2_abi::{FCoords, FVector};
use hp2_ffi_support::guard;
use hp2_render::particle::ParticleFrame;
use hp2_render::particle_tube::{TubeParticle, poly_strip_setup_verts, tritube_setup_verts};
use hp2_render::services::Hp2RenderServices;

/// `TriTubeParticles::SetupVerts`. `prev` null is the first-call branch
/// (returns 0, `verts` untouched). `next` null means no following particle.
/// Writes twelve `FTransTexture` vertices and updates `prev_strip_vect`.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_tube_setup_verts(
    services: *const Hp2RenderServices,
    frame: *const ParticleFrame,
    coords: *const c_void,
    tex_usize: i32,
    tex_vsize: i32,
    prev: *const c_void,
    curr: *const c_void,
    next: *const c_void,
    prev_strip_vect: *mut c_void,
    verts: *mut c_void,
) -> u8 {
    guard(
        "hp2_rs_render_particle_tube_setup_verts",
        0,
        move || unsafe {
            if services.is_null()
                || frame.is_null()
                || coords.is_null()
                || curr.is_null()
                || prev_strip_vect.is_null()
                || verts.is_null()
            {
                return 0;
            }
            let services = *services;
            let prev = (prev as *const TubeParticle).as_ref();
            let next = (next as *const TubeParticle).as_ref();
            tritube_setup_verts(
                &services,
                &*frame,
                &*(coords as *const FCoords),
                tex_usize,
                tex_vsize,
                prev,
                &*(curr as *const TubeParticle),
                next,
                &mut *(prev_strip_vect as *mut FVector),
                &mut *(verts as *mut [FTransTexture; 12]),
            )
        },
    )
}

/// `PolyStripParticles::SetupVerts`. `prev` null is the first-call branch
/// (returns 0, `verts` untouched). `next` null means no following particle.
/// Writes four `FTransTexture` vertices and updates `prev_strip_vect` and
/// `prev_edge_vect`.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_poly_strip_setup_verts(
    services: *const Hp2RenderServices,
    frame: *const ParticleFrame,
    coords: *const c_void,
    tex_usize: i32,
    tex_vsize: i32,
    prev: *const c_void,
    curr: *const c_void,
    next: *const c_void,
    prev_strip_vect: *mut c_void,
    prev_edge_vect: *mut c_void,
    verts: *mut c_void,
) -> u8 {
    guard(
        "hp2_rs_render_particle_poly_strip_setup_verts",
        0,
        move || unsafe {
            if services.is_null()
                || frame.is_null()
                || coords.is_null()
                || curr.is_null()
                || prev_strip_vect.is_null()
                || prev_edge_vect.is_null()
                || verts.is_null()
            {
                return 0;
            }
            let services = *services;
            let prev = (prev as *const TubeParticle).as_ref();
            let next = (next as *const TubeParticle).as_ref();
            poly_strip_setup_verts(
                &services,
                &*frame,
                &*(coords as *const FCoords),
                tex_usize,
                tex_vsize,
                prev,
                &*(curr as *const TubeParticle),
                next,
                &mut *(prev_strip_vect as *mut FVector),
                &mut *(prev_edge_vect as *mut FVector),
                &mut *(verts as *mut [FTransTexture; 4]),
            )
        },
    )
}
