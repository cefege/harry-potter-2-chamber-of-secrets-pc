//! Live SDK software debug geometry, preserving the original edge and cap policy.

use crate::{softln::solve_quadratic, softln_services::Native};
use hp2_abi::{FCoords, FVector};
use hp2_core::{
    ansi_math,
    math::{
        FBox, FRotator, coords_mul, coords_mul_rotator, vector_add, vector_cross, vector_div,
        vector_dot, vector_safe_normal, vector_scale, vector_shift, vector_size_sq_2d, vector_sub,
        vector_transform, vector_transform_point,
    },
};
/// Typed leaves for the shared source software-geometry algorithms.
pub trait SoftlnDrawOps {
    type Error;

    fn rend_map(&mut self) -> Result<i32, Self::Error>;
    fn zoom(&mut self) -> Result<f32, Self::Error>;
    fn fx2(&mut self) -> Result<f32, Self::Error>;
    fn fy2(&mut self) -> Result<f32, Self::Error>;
    fn proj_z(&mut self) -> Result<f32, Self::Error>;
    fn coords(&mut self) -> Result<FCoords, Self::Error>;
    fn is_ortho(&mut self) -> Result<bool, Self::Error>;
    fn ortho_zoom(&mut self) -> Result<f32, Self::Error>;
    fn unit_coords(&mut self) -> Result<FCoords, Self::Error>;
    fn trig_table(&mut self) -> Result<&'static [f32; hp2_core::math::NUM_ANGLES], Self::Error>;
    fn render_line(
        &mut self,
        start: FVector,
        end: FVector,
        flags: u32,
    ) -> Result<(), Self::Error>;
    fn device_line(
        &mut self,
        start: FVector,
        end: FVector,
        flags: u32,
    ) -> Result<(), Self::Error>;
}

impl SoftlnDrawOps for Native {
    type Error = ();

    fn rend_map(&mut self) -> Result<i32, Self::Error> { Native::rend_map(self) }
    fn zoom(&mut self) -> Result<f32, Self::Error> { Native::zoom(self) }
    fn fx2(&mut self) -> Result<f32, Self::Error> { Native::fx2(self) }
    fn fy2(&mut self) -> Result<f32, Self::Error> { Native::fy2(self) }
    fn proj_z(&mut self) -> Result<f32, Self::Error> { Native::proj_z(self) }
    fn coords(&mut self) -> Result<FCoords, Self::Error> { Native::coords(self) }
    fn is_ortho(&mut self) -> Result<bool, Self::Error> { Native::is_ortho(self) }
    fn ortho_zoom(&mut self) -> Result<f32, Self::Error> { Native::ortho_zoom(self) }
    fn unit_coords(&mut self) -> Result<FCoords, Self::Error> { Native::unit_coords(self) }
    fn trig_table(&mut self) -> Result<&'static [f32; hp2_core::math::NUM_ANGLES], Self::Error> {
        Native::trig_table(self)
    }
    fn render_line(&mut self, start: FVector, end: FVector, flags: u32) -> Result<(), Self::Error> {
        Native::render_line(self, start, end, flags)
    }
    fn device_line(&mut self, start: FVector, end: FVector, flags: u32) -> Result<(), Self::Error> {
        Native::device_line(self, start, end, flags)
    }
}

const LINE_PRE_TRANSFORMED: u32 = 4;
const REN_ORTH_XY: i32 = 13;
const REN_ORTH_XZ: i32 = 14;

fn vector(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

// UnGnuG.h appRound is static_cast<INT>, not nearest-integer rounding. The
// ARM64 conversion saturates and maps NaN to zero, as does Rust's cast.
fn segments(pixels: f32) -> i32 {
    (pixels as i32).clamp(6, 64)
}

fn rotate<O: SoftlnDrawOps>(native: &mut O, coords: FCoords, yaw: i32) -> Result<FCoords, O::Error> {
    let trig = native.trig_table()?;
    Ok(coords_mul_rotator(
        coords,
        FRotator {
            pitch: 0,
            yaw,
            roll: 0,
        },
        trig,
    ))
}

pub fn draw_cylinder<O: SoftlnDrawOps>(
    native: &mut O,
    place: FCoords,
    mut radius: f32,
    height: f32,
    mut width: f32,
    mut flags: u32,
) -> Result<(), O::Error> {

    let oval = width >= 0.0;
    let mut swapped = false;
    let mut frame_coords = place;
    if flags & LINE_PRE_TRANSFORMED == 0 {
        frame_coords = coords_mul(frame_coords, native.coords()?);
        flags |= LINE_PRE_TRANSFORMED;
    }

    let point0 = vector_sub(
        frame_coords.origin,
        vector_scale(frame_coords.z_axis, height),
    );
    let point1 = vector_add(
        frame_coords.origin,
        vector_scale(frame_coords.z_axis, height),
    );
    if native.is_ortho()? {
        if vector_size_sq_2d(frame_coords.z_axis) > 0.0 {
            let mut offset =
                vector_safe_normal(vector_cross(vector(0.0, 0.0, 1.0), frame_coords.z_axis));
            if oval {
                let radial_x = vector_dot(offset, frame_coords.x_axis);
                let radial_x2 = radial_x * radial_x;
                // The unsuffixed source thresholds and reciprocal are DOUBLE.
                let oval_radius = if (width as f64) < 0.001 {
                    if (radial_x2 as f64) < 0.0001 {
                        0.0
                    } else {
                        radius
                    }
                } else if (radius as f64) < 0.001 {
                    if ((1.0 - radial_x2) as f64) < 0.0001 {
                        0.0
                    } else {
                        width
                    }
                } else {
                    let inverse_square =
                        radial_x2 / (width * width) + (1.0 - radial_x2) / (radius * radius);
                    (1.0 / ansi_math::sqrt(inverse_square as f64)) as f32
                };
                offset = vector_scale(offset, oval_radius);
            } else {
                offset = vector_scale(offset, radius);
            }
            native.render_line(
                vector_add(point0, offset),
                vector_add(point1, offset),
                flags,
            )?;
            native.render_line(
                vector_sub(point0, offset),
                vector_sub(point1, offset),
                flags,
            )?;
        }
    } else {
        let mut x_axis = frame_coords.x_axis;
        let mut y_axis = frame_coords.y_axis;
        let mut xc = vector_dot(x_axis, point0);
        let mut yc = vector_dot(y_axis, point0);
        if xc.abs() > yc.abs() {
            core::mem::swap(&mut x_axis, &mut y_axis);
            core::mem::swap(&mut xc, &mut yc);
            if oval {
                core::mem::swap(&mut width, &mut radius);
                swapped = true;
            }
        }
        if oval {
            let (results0, c0, results1, c1) = if (width as f64) < 0.0001 {
                (1, [0.0; 2], 1, [0.0; 2])
            } else {
                let ratio = radius / width;
                let ratio_yc = ratio * yc;
                let radius_yc = radius * yc;
                let (results0, c0) = solve_quadratic(
                    xc.mul_add(xc, ratio_yc * ratio_yc),
                    xc * 2.0,
                    1.0 - radius_yc * radius_yc,
                );
                xc = vector_dot(x_axis, point1);
                yc = vector_dot(y_axis, point1);
                let ratio_yc = ratio * yc;
                let radius_yc = radius * yc;
                let (results1, c1) = solve_quadratic(
                    xc.mul_add(xc, ratio_yc * ratio_yc),
                    xc * 2.0,
                    1.0 - radius_yc * radius_yc,
                );
                (results0, c0, results1, c1)
            };
            if results0 == results1 {
                // Deliberately use the last endpoint's XC/YC for BOTH silhouettes.
                let ryc = -1.0 / yc;
                let xyc = -xc / yc;
                for i in 0..results0 as usize {
                    let vec0 = vector_add(
                        vector_scale(x_axis, c0[i]),
                        vector_scale(y_axis, xyc.mul_add(c0[i], ryc)),
                    );
                    let vec1 = vector_add(
                        vector_scale(x_axis, c1[i]),
                        vector_scale(y_axis, xyc.mul_add(c1[i], ryc)),
                    );
                    native.render_line(
                        vector_add(point0, vec0),
                        vector_add(point1, vec1),
                        flags,
                    )?;
                }
            }
        } else {
            let (results0, c0) = solve_quadratic(
                xc.mul_add(xc, yc * yc),
                (xc * radius) * 2.0,
                (-yc).mul_add(yc, radius * radius),
            );
            xc = vector_dot(x_axis, point1);
            yc = vector_dot(y_axis, point1);
            let (results1, c1) = solve_quadratic(
                xc.mul_add(xc, yc * yc),
                (xc * radius) * 2.0,
                (-yc).mul_add(yc, radius * radius),
            );
            if results0 == results1 {
                let ryc = -radius / yc;
                let xyc = -xc / yc;
                for i in 0..results0 as usize {
                    let vec0 = vector_add(
                        vector_scale(x_axis, c0[i]),
                        vector_scale(y_axis, xyc.mul_add(c0[i], ryc)),
                    );
                    let vec1 = vector_add(
                        vector_scale(x_axis, c1[i]),
                        vector_scale(y_axis, xyc.mul_add(c1[i], ryc)),
                    );
                    native.render_line(
                        vector_add(point0, vector_scale(vec0, radius)),
                        vector_add(point1, vector_scale(vec1, radius)),
                        flags,
                    )?;
                }
            }
        }
        if swapped {
            core::mem::swap(&mut width, &mut radius);
        }
    }

    let mut origin = vector(0.0, radius, -height);
    let z_offset = vector_scale(frame_coords.z_axis, height * 2.0);
    let mut p1 = vector_transform_point(origin, coords_mul(native.unit_coords()?, frame_coords));
    let mut pixels = if oval {
        origin.y = 1.0;
        if radius >= width { radius } else { width }
    } else {
        radius
    };
    let (bottom_visible, top_visible) = if native.is_ortho()? {
        pixels /= native.zoom()?;
        (z_offset.z > 0.0, z_offset.z < 0.0)
    } else {
        pixels *= native.proj_z()? / frame_coords.origin.z;
        (
            -vector_dot(p1, z_offset) <= 0.0,
            vector_dot(vector_add(p1, z_offset), z_offset) <= 0.0,
        )
    };
    let count = segments(pixels * 0.5);
    // The original includes count, drawing an extra segment past the closure.
    for i in 0..=count {
        let yaw = (65536 * (i + 1)) / count;
        let p2 = if oval {
            let unit = native.unit_coords()?;
            let mut p2 = vector_transform(origin, rotate(native, unit, yaw)?);
            p2.x *= width;
            p2.y *= radius;
            vector_transform_point(p2, coords_mul(native.unit_coords()?, frame_coords))
        } else {
            let unit = native.unit_coords()?;
            vector_transform_point(
                origin,
                rotate(native, coords_mul(unit, frame_coords), yaw)?,
            )
        };
        let normal = vector_sub(
            vector_scale(vector_add(vector_add(p1, p2), z_offset), 0.5),
            frame_coords.origin,
        );
        if bottom_visible
            || (if native.is_ortho()? {
                normal.z
            } else {
                vector_dot(normal, p1)
            }) < 0.0
        {
            native.render_line(p1, p2, flags)?;
        }
        if top_visible
            || (if native.is_ortho()? {
                normal.z
            } else {
                vector_dot(normal, vector_add(p1, z_offset))
            }) < 0.0
        {
            native.render_line(vector_add(p1, z_offset), vector_add(p2, z_offset), flags)?;
        }
        p1 = p2;
    }
    Ok(())
}

pub fn draw_circle<O: SoftlnDrawOps>(
    native: &mut O,
    location: FVector,
    radius: f32,
    flags: u32,
    scale: bool,
) -> Result<(), O::Error> {

    let a = native.coords()?.x_axis;
    let b = native.coords()?.y_axis;
    let mut pixels = radius;
    if native.is_ortho()? {
        pixels /= native.zoom()?;
    } else {
        let projection = native.proj_z()?;
        let delta = vector_sub(location, native.coords()?.origin);
        pixels *= projection / vector_dot(delta, native.coords()?.z_axis);
    }
    let count = segments(pixels);
    let mut angle = 0.0_f32;
    let angle_delta = (2.0_f32 * core::f32::consts::PI) / count as f32;
    let scaled = if scale {
        radius * (native.ortho_zoom()? / 10000.0)
    } else {
        radius
    };
    // Max is (A >= B) ? A : B, including equal values and unordered NaNs.
    let scaled = if scaled >= radius { scaled } else { radius };
    let radial = |angle: f32| {
        vector_scale(
            vector_add(
                vector_scale(a, ansi_math::cos(angle as f64) as f32),
                vector_scale(b, ansi_math::sin(angle as f64) as f32),
            ),
            scaled,
        )
    };
    let mut p1 = vector_add(location, radial(angle));
    for _ in 0..count {
        angle += angle_delta;
        let p2 = vector_add(location, radial(angle));
        native.device_line(p1, p2, flags)?;
        p1 = p2;
    }
    Ok(())
}

pub fn draw_box<O: SoftlnDrawOps>(native: &mut O, min: FVector, max: FVector, flags: u32) -> Result<(), O::Error> {

    let location = vector_add(min, max);
    let (a, b) = if native.rend_map()? == REN_ORTH_XY {
        (
            vector(max.x - min.x, 0.0, 0.0),
            vector(0.0, max.y - min.y, 0.0),
        )
    } else if native.rend_map()? == REN_ORTH_XZ {
        (
            vector(max.x - min.x, 0.0, 0.0),
            vector(0.0, 0.0, max.z - min.z),
        )
    } else {
        (
            vector(0.0, max.y - min.y, 0.0),
            vector(0.0, 0.0, max.z - min.z),
        )
    };
    native.device_line(
        vector_div(vector_add(vector_add(location, a), b), 2.0),
        vector_div(vector_sub(vector_add(location, a), b), 2.0),
        flags,
    )?;
    native.device_line(
        vector_div(vector_add(vector_sub(location, a), b), 2.0),
        vector_div(vector_sub(vector_sub(location, a), b), 2.0),
        flags,
    )?;
    native.device_line(
        vector_div(vector_add(vector_add(location, a), b), 2.0),
        vector_div(vector_add(vector_sub(location, a), b), 2.0),
        flags,
    )?;
    native.device_line(
        vector_div(vector_sub(vector_add(location, a), b), 2.0),
        vector_div(vector_sub(vector_sub(location, a), b), 2.0),
        flags,
    )?;
    Ok(())
}

pub fn draw_box_oriented<O: SoftlnDrawOps>(
    native: &mut O,
    bounds: FBox,
    coords: FCoords,
    mut flags: u32,
) -> Result<(), O::Error> {
    let mut frame_coords = coords;
    if flags & LINE_PRE_TRANSFORMED == 0 {
        frame_coords = coords_mul(frame_coords, native.coords()?);
        flags |= LINE_PRE_TRANSFORMED;
    }
    // The source-local FVector * FCoords is exactly the existing shift operation,
    // not TransformPointBy: column products followed by addition of the origin.
    let min = vector_shift(bounds.min, frame_coords);
    let max = vector_shift(bounds.max, frame_coords);
    let x = vector_scale(frame_coords.x_axis, bounds.max.x - bounds.min.x);
    let y = vector_scale(frame_coords.y_axis, bounds.max.y - bounds.min.y);
    let z = vector_scale(frame_coords.z_axis, bounds.max.z - bounds.min.z);
    let (xneg, xpos, yneg, ypos, zneg, zpos) = if native.is_ortho()? {
        (
            frame_coords.x_axis.z > 0.0,
            frame_coords.x_axis.z < 0.0,
            frame_coords.y_axis.z > 0.0,
            frame_coords.y_axis.z < 0.0,
            frame_coords.z_axis.z > 0.0,
            frame_coords.z_axis.z < 0.0,
        )
    } else {
        (
            vector_dot(frame_coords.x_axis, min) > 0.0,
            vector_dot(frame_coords.x_axis, max) < 0.0,
            vector_dot(frame_coords.y_axis, min) > 0.0,
            vector_dot(frame_coords.y_axis, max) < 0.0,
            vector_dot(frame_coords.z_axis, min) > 0.0,
            vector_dot(frame_coords.z_axis, max) < 0.0,
        )
    };

    // Keep the sequential additions/subtractions, including their rounding, and
    // the source X/Y/Z edge order rather than reconstructing each corner.
    let mut start = min;
    if yneg || zneg {
        native.render_line(start, vector_add(start, x), flags)?;
    }
    start = vector_add(start, y);
    if ypos || zneg {
        native.render_line(start, vector_add(start, x), flags)?;
    }
    start = vector_add(start, z);
    if ypos || zpos {
        native.render_line(start, vector_add(start, x), flags)?;
    }
    start = vector_sub(start, y);
    if yneg || zpos {
        native.render_line(start, vector_add(start, x), flags)?;
    }

    start = min;
    if xneg || zneg {
        native.render_line(start, vector_add(start, y), flags)?;
    }
    start = vector_add(start, x);
    if xpos || zneg {
        native.render_line(start, vector_add(start, y), flags)?;
    }
    start = vector_add(start, z);
    if xpos || zpos {
        native.render_line(start, vector_add(start, y), flags)?;
    }
    start = vector_sub(start, x);
    if xneg || zpos {
        native.render_line(start, vector_add(start, y), flags)?;
    }

    start = min;
    if xneg || yneg {
        native.render_line(start, vector_add(start, z), flags)?;
    }
    start = vector_add(start, x);
    if xpos || yneg {
        native.render_line(start, vector_add(start, z), flags)?;
    }
    start = vector_add(start, y);
    if xpos || ypos {
        native.render_line(start, vector_add(start, z), flags)?;
    }
    start = vector_sub(start, x);
    if xneg || ypos {
        native.render_line(start, vector_add(start, z), flags)?;
    }
    Ok(())
}
