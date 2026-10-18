//! Particle clipping exports (`hp2-render::particle_clip`). The C++ shell in
//! `UnParticleRn.cpp` forwards `ComputeClipCodes` and `ClipPolygon` here.
//! `FTransTexture` crosses as `void*` (hp2-abi mirror, not parsed by cbindgen);
//! C++ owns the `New<FTransTexture>(GMem)` allocation through `alloc`.

use std::ffi::c_void;

use hp2_ffi_support::guard;
use hp2_render::particle::{FTransTexture, ParticleFrame};
use hp2_render::particle_clip::{clip_polygon, compute_clip_codes};

/// `ComputeClipCodes(Frame, tt)`: writes `tt.transform.flags`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_compute_clip_codes(
    frame: *const ParticleFrame,
    tt: *mut c_void,
) {
    guard(
        "hp2_rs_render_particle_compute_clip_codes",
        (),
        move || unsafe {
            if frame.is_null() || tt.is_null() {
                return;
            }
            compute_clip_codes(&*frame, &mut *(tt as *mut FTransTexture));
        },
    );
}

/// `ClipPolygon(Frame, Pts[], LocalCount, code)`. `pts` is a `FTransTexture*`
/// array that is read for `local_count` entries and overwritten in place when the
/// returned count is > 2 (see `hp2_render::particle_clip::clip_polygon`). `alloc`
/// returns a fresh, non-null `FTransTexture*` for each crossing vertex.
/// Returns the new vertex count, or 0 if fewer than 3 vertices survive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_clip_polygon(
    frame: *const ParticleFrame,
    pts: *mut *mut c_void,
    local_count: i32,
    code: u8,
    alloc: Option<unsafe extern "C" fn() -> *mut c_void>,
) -> i32 {
    guard("hp2_rs_render_particle_clip_polygon", 0, move || unsafe {
        if frame.is_null() || pts.is_null() {
            return 0;
        }
        let Some(alloc) = alloc else {
            return 0;
        };
        let mut new_vertex = || alloc() as *mut FTransTexture;
        clip_polygon(
            &*frame,
            pts as *mut *mut FTransTexture,
            local_count,
            code,
            &mut new_vertex,
        )
    })
}
