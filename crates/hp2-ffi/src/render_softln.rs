//! Software line-drawing exports (`hp2-render::softln`). The C++ shell in
//! `UnSoftLn.cpp` forwards `SolveQuadratic`, `URender::Project` and
//! `URender::Deproject` here. `FCoords` crosses flattened to 12 floats
//! (Origin, XAxis, YAxis, ZAxis; each X, Y, Z), the same layout as the C++
//! struct; `FVector` crosses as 3 contiguous floats.

use hp2_ffi_support::guard;
use hp2_render::softln::{FVector, ScreenFrame, deproject, project, solve_quadratic};

/// `FSceneNode` fields `Project`/`Deproject` read, plus the viewport's
/// `RendMap`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RsScreenFrame {
    pub rend_map: i32,
    pub coords: [f32; 12],
    pub zoom: f32,
    pub fx2: f32,
    pub fy2: f32,
    pub proj_z: f32,
}

/// `int SolveQuadratic(float a_results[2], float a, float b, float c)`.
/// Writes `results[0..2]` only when the returned count is positive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_softln_solve_quadratic(
    results: *mut f32,
    a: f32,
    b: f32,
    c: f32,
) -> i32 {
    guard("hp2_rs_render_softln_solve_quadratic", 0, move || unsafe {
        if results.is_null() {
            return 0;
        }
        let (ret, roots) = solve_quadratic(a, b, c);
        if ret > 0 {
            *results = roots[0];
            *results.add(1) = roots[1];
        }
        ret
    })
}

/// `URender::Project(Frame, V, ScreenX, ScreenY, Scale)`. `v` points at 3
/// floats; `scale` may be null (written only when non-null, as in C++).
/// Returns the `UBOOL` result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_softln_project(
    frame: *const Hp2RsScreenFrame,
    v: *const f32,
    screen_x: *mut f32,
    screen_y: *mut f32,
    scale: *mut f32,
) -> i32 {
    guard("hp2_rs_render_softln_project", 0, move || unsafe {
        if frame.is_null() || v.is_null() || screen_x.is_null() || screen_y.is_null() {
            return 0;
        }
        let f = *frame;
        let frame = ScreenFrame::from_flat(f.rend_map, &f.coords, f.zoom, f.fx2, f.fy2, f.proj_z);
        let p = project(
            &frame,
            FVector {
                x: *v,
                y: *v.add(1),
                z: *v.add(2),
            },
        );
        *screen_x = p.screen_x;
        *screen_y = p.screen_y;
        if !scale.is_null() {
            *scale = p.scale;
        }
        p.visible as i32
    })
}

/// `URender::Deproject(Frame, ScreenX, ScreenY, V)`. `v` receives 3 floats;
/// it is written in every case (the default branch writes the origin).
/// Returns the `UBOOL` result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_softln_deproject(
    frame: *const Hp2RsScreenFrame,
    screen_x: i32,
    screen_y: i32,
    v: *mut f32,
) -> i32 {
    guard("hp2_rs_render_softln_deproject", 0, move || unsafe {
        if frame.is_null() || v.is_null() {
            return 0;
        }
        let f = *frame;
        let frame = ScreenFrame::from_flat(f.rend_map, &f.coords, f.zoom, f.fx2, f.fy2, f.proj_z);
        let (ret, p) = deproject(&frame, screen_x, screen_y);
        *v = p.x;
        *v.add(1) = p.y;
        *v.add(2) = p.z;
        ret as i32
    })
}
