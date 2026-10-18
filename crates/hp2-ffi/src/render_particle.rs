//! Particle helper exports (`hp2-render::particle`). The C++ shell in
//! `UnParticleRn.cpp` forwards its inline helpers here. Pointers to
//! `FTransTexture`/`FTransform`/`FVector`/`FPlane` (hp2-abi mirrors) cross
//! as `void*`, since cbindgen does not parse hp2-abi; C++ passes its own
//! structs, which share the layout pinned by the asserts in both languages.

use std::ffi::c_void;

use hp2_ffi_support::guard;
use hp2_render::particle::{
    FTransTexture, FTransform, FVector, ParticleFrame, combine_clip_codes, compute_clip_codes_2d,
    set_trans, set_trans_2d, set_uv,
};

/// `SetUV`: `tt.U = FLOAT(USize-1)*u`, `tt.V = FLOAT(VSize-1)*v`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_set_uv(
    tt: *mut c_void,
    tex_usize: i32,
    tex_vsize: i32,
    u: f32,
    v: f32,
) {
    guard("hp2_rs_render_particle_set_uv", (), move || unsafe {
        if tt.is_null() {
            return;
        }
        set_uv(&mut *(tt as *mut FTransTexture), tex_usize, tex_vsize, u, v);
    });
}

/// `SetTrans`: fills `tt` from screen position/depth, computing point and
/// outcode from `frame`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_render_particle_set_trans(
    tt: *mut c_void,
    frame: *const ParticleFrame,
    tex_usize: i32,
    tex_vsize: i32,
    screen_x: f32,
    screen_y: f32,
    screen_z: f32,
    u: f32,
    v: f32,
) {
    guard("hp2_rs_render_particle_set_trans", (), move || unsafe {
        if tt.is_null() || frame.is_null() {
            return;
        }
        set_trans(
            &mut *(tt as *mut FTransTexture),
            &*frame,
            tex_usize,
            tex_vsize,
            screen_x,
            screen_y,
            screen_z,
            u,
            v,
        );
    });
}

/// `SetTrans2D`: copies `*point3d` (an `FTransform`) into `tt` and offsets it
/// by `(dx, dy)` in screen space.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_set_trans_2d(
    tt: *mut c_void,
    point3d: *const c_void,
    dx: f32,
    dy: f32,
    u: f32,
    v: f32,
) {
    guard("hp2_rs_render_particle_set_trans_2d", (), move || unsafe {
        if tt.is_null() || point3d.is_null() {
            return;
        }
        set_trans_2d(
            &mut *(tt as *mut FTransTexture),
            &*(point3d as *const FTransform),
            dx,
            dy,
            u,
            v,
        );
    });
}

/// `ComputeClipCodes2D`: outcode of the point `*c` (an `FVector`) offset by
/// `(dx, dy)` in screen space. Returns the reject bits, or `FVF_OutAll`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_compute_clip_codes_2d(
    frame: *const ParticleFrame,
    c: *const c_void,
    dx: f32,
    dy: f32,
) -> u8 {
    guard(
        "hp2_rs_render_particle_compute_clip_codes_2d",
        0,
        move || unsafe {
            if frame.is_null() || c.is_null() {
                return 0;
            }
            compute_clip_codes_2d(&*frame, &*(c as *const FVector), dx, dy)
        },
    )
}

/// `CombineClipCodes`: over `n` transformed vertices starting at `verts`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_particle_combine_clip_codes(
    verts: *const c_void,
    n: i32,
) -> u8 {
    guard(
        "hp2_rs_render_particle_combine_clip_codes",
        0,
        move || unsafe {
            if verts.is_null() || n <= 0 {
                return 0;
            }
            let verts = std::slice::from_raw_parts(verts as *const FTransTexture, n as usize);
            combine_clip_codes(verts)
        },
    )
}
