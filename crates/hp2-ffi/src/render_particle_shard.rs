//! Shard and liquid particle geometry exports (`hp2-render::particle_shard`).
//! The C++ shells in `UnParticleRn.cpp` keep the `UParticle`/`Center` members
//! and forward `SetupVerts` here. `FCoords`, `FVector`, `FTransform`, and
//! `FTransTexture` cross as `void*` (hp2-abi mirrors, not parsed by cbindgen).
//! C++ owns the vertex buffer, so Rust writes into it in place.

use std::ffi::c_void;

use hp2_abi::render_particle::{FTransTexture, FTransform};
use hp2_abi::{FCoords, FVector};
use hp2_ffi_support::guard;
use hp2_render::particle::ParticleFrame;
use hp2_render::particle_shard::{ParticleState, liquid_setup_verts, shard_setup_verts};
use hp2_render::services::Hp2RenderServices;

/// `ShardParticles::SetupVerts`. Writes three `FTransTexture` vertices and
/// returns the combined clip codes.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_shard_setup_verts(
    services: *const Hp2RenderServices,
    frame: *const ParticleFrame,
    coords: *const c_void,
    tex_usize: i32,
    tex_vsize: i32,
    position: *const c_void,
    spin: f32,
    length: f32,
    width: f32,
    verts: *mut c_void,
) -> u8 {
    guard(
        "hp2_rs_render_particle_shard_setup_verts",
        0,
        move || unsafe {
            if services.is_null()
                || frame.is_null()
                || coords.is_null()
                || position.is_null()
                || verts.is_null()
            {
                return 0;
            }
            let services = *services;
            let verts = &mut *(verts as *mut [FTransTexture; 3]);
            shard_setup_verts(
                &services,
                &*frame,
                &*(coords as *const FCoords),
                tex_usize,
                tex_vsize,
                *(position as *const FVector),
                spin,
                length,
                width,
                verts,
            )
        },
    )
}

/// `LiquidParticles::SetupVerts`. `center` is the C++ `Center` member and is
/// updated in place. Writes four `FTransTexture` vertices and returns the
/// combined clip codes.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_liquid_setup_verts(
    services: *const Hp2RenderServices,
    frame: *const ParticleFrame,
    coords: *const c_void,
    tex_usize: i32,
    tex_vsize: i32,
    center: *mut c_void,
    position: *const c_void,
    velocity: *const c_void,
    length: f32,
    width: f32,
    drip_timer: f32,
    verts: *mut c_void,
) -> u8 {
    guard(
        "hp2_rs_render_particle_liquid_setup_verts",
        0,
        move || unsafe {
            if services.is_null()
                || frame.is_null()
                || coords.is_null()
                || center.is_null()
                || position.is_null()
                || velocity.is_null()
                || verts.is_null()
            {
                return 0;
            }
            let services = *services;
            let center = &mut *(center as *mut FTransform);
            let verts = &mut *(verts as *mut [FTransTexture; 4]);
            let state = ParticleState {
                position: *(position as *const FVector),
                velocity: *(velocity as *const FVector),
                length,
                width,
                drip_timer,
            };
            liquid_setup_verts(
                &services,
                &*frame,
                &*(coords as *const FCoords),
                tex_usize,
                tex_vsize,
                center,
                &state,
                verts,
            )
        },
    )
}
