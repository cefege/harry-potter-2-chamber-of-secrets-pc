//! Sprite screen-extent exports (`hp2-render::sprite`). `FDynamicSprite::Setup`
//! in `UnSprite.cpp` calls these for its X and Y blocks. Both always write
//! `lo`/`hi`, as the C++ members are assigned before the reject test.

use std::ffi::c_void;

use hp2_ffi_support::guard;
use hp2_render::sprite::{
    BoxExtent, FVector, ProjectionPlane, SetupPath, box_axis_is_empty, box_extent, extent_x,
    extent_y, projection_plane, setup_path,
};

/// X extent of a sprite. Returns 1 when visible, 0 when culled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_extent_x(
    screen_x: f32,
    persp: f32,
    draw_scale: f32,
    tex_usize: i32,
    frame_x: i32,
    out_lo: *mut i32,
    out_hi: *mut i32,
) -> u8 {
    guard("hp2_rs_render_sprite_extent_x", 0, move || unsafe {
        if out_lo.is_null() || out_hi.is_null() {
            return 0;
        }
        let e = extent_x(screen_x, persp, draw_scale, tex_usize, frame_x);
        *out_lo = e.lo;
        *out_hi = e.hi;
        e.visible as u8
    })
}

/// Y extent of a sprite. Returns 1 when visible, 0 when culled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_extent_y(
    screen_y: f32,
    persp: f32,
    draw_scale: f32,
    tex_vsize: i32,
    frame_y: i32,
    out_lo: *mut i32,
    out_hi: *mut i32,
) -> u8 {
    guard("hp2_rs_render_sprite_extent_y", 0, move || unsafe {
        if out_lo.is_null() || out_hi.is_null() {
            return 0;
        }
        let e = extent_y(screen_y, persp, draw_scale, tex_vsize, frame_y);
        *out_lo = e.lo;
        *out_hi = e.hi;
        e.visible as u8
    })
}

/// Branch selection of `FDynamicSprite::Setup`. Returns 0 reject, 1 sprite, 2 box.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_setup_path(
    is_sprite_type: u8,
    is_mesh: u8,
    is_particles: u8,
    show_icons: u8,
    ortho: u8,
    realtime: u8,
) -> u8 {
    guard(
        "hp2_rs_render_sprite_setup_path",
        SetupPath::Reject as u8,
        move || {
            setup_path(
                is_sprite_type != 0,
                is_mesh != 0,
                is_particles != 0,
                show_icons != 0,
                ortho != 0,
                realtime != 0,
            ) as u8
        },
    )
}

/// Projection plane of the sprite branch. Writes `*out`; returns 1 when the
/// sprite survives, 0 when `Setup` returns 0. `location`, `origin`, `z_axis`
/// are `FVector` pointers (hp2-abi layout).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_projection_plane(
    location: *const c_void,
    origin: *const c_void,
    z_axis: *const c_void,
    proj_forward: f32,
    ortho: u8,
    out: *mut ProjectionPlane,
) -> u8 {
    guard("hp2_rs_render_sprite_projection_plane", 0, move || unsafe {
        if location.is_null() || origin.is_null() || z_axis.is_null() || out.is_null() {
            return 0;
        }
        let p = projection_plane(
            *(location as *const FVector),
            *(origin as *const FVector),
            *(z_axis as *const FVector),
            proj_forward,
            ortho != 0,
        );
        *out = p;
        (!p.reject) as u8
    })
}

/// Box branch screen bounds. Writes `*out`; returns 1 when `Y1 < Y2`, else 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_box_extent(
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
    out: *mut BoxExtent,
) -> u8 {
    guard("hp2_rs_render_sprite_box_extent", 0, move || unsafe {
        if out.is_null() {
            return 0;
        }
        let b = box_extent(min_x, max_x, min_y, max_y, min_z, max_z);
        *out = b;
        (!b.reject) as u8
    })
}

/// `BoxCoords.XAxis.SizeSquared() == 0`: returns 1 when the box is empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_sprite_box_axis_is_empty(x_axis: *const c_void) -> u8 {
    guard(
        "hp2_rs_render_sprite_box_axis_is_empty",
        1,
        move || unsafe {
            if x_axis.is_null() {
                return 1;
            }
            box_axis_is_empty(*(x_axis as *const FVector)) as u8
        },
    )
}
