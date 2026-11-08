//! Live projection and quadratic policy from `Render/Src/UnSoftLn.cpp`.
//!
//! Native frame reads and output stores are deliberately interleaved: outputs
//! may alias each other, the input vector, or fields read by a later service.
//! Only copied arithmetic locals survive callbacks; native owners stay raw.

use crate::softln_services::Native;
pub use hp2_abi::{FCoords, FVector};
use hp2_core::math::{vector_sub, vector_transform};

/// `ERendMap` values (`Engine/Inc/UnCamera.h`) handled by Project/Deproject.
pub const REN_ORTH_XY: i32 = 13;
pub const REN_ORTH_XZ: i32 = 14;
pub const REN_ORTH_YZ: i32 = 15;

/// Copied frame values for native-independent scene visibility calculations.
/// Both this value API and live SDK access use the same ordered policy below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenFrame {
    pub rend_map: i32,
    pub coords: FCoords,
    pub zoom: f32,
    pub fx2: f32,
    pub fy2: f32,
    pub proj_z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub visible: bool,
    pub screen_x: f32,
    pub screen_y: f32,
    pub scale: f32,
}

trait FrameAccess {
    type Error;
    fn rend_map(&self) -> Result<i32, Self::Error>;
    fn coords(&self) -> Result<FCoords, Self::Error>;
    fn zoom(&self) -> Result<f32, Self::Error>;
    fn fx2(&self) -> Result<f32, Self::Error>;
    fn fy2(&self) -> Result<f32, Self::Error>;
    fn proj_z(&self) -> Result<f32, Self::Error>;
}

impl FrameAccess for Native {
    type Error = ();
    fn rend_map(&self) -> Result<i32, ()> {
        Native::rend_map(self)
    }
    fn coords(&self) -> Result<FCoords, ()> {
        Native::coords(self)
    }
    fn zoom(&self) -> Result<f32, ()> {
        Native::zoom(self)
    }
    fn fx2(&self) -> Result<f32, ()> {
        Native::fx2(self)
    }
    fn fy2(&self) -> Result<f32, ()> {
        Native::fy2(self)
    }
    fn proj_z(&self) -> Result<f32, ()> {
        Native::proj_z(self)
    }
}

impl FrameAccess for ScreenFrame {
    type Error = core::convert::Infallible;
    fn rend_map(&self) -> Result<i32, Self::Error> {
        Ok(self.rend_map)
    }
    fn coords(&self) -> Result<FCoords, Self::Error> {
        Ok(self.coords)
    }
    fn zoom(&self) -> Result<f32, Self::Error> {
        Ok(self.zoom)
    }
    fn fx2(&self) -> Result<f32, Self::Error> {
        Ok(self.fx2)
    }
    fn fy2(&self) -> Result<f32, Self::Error> {
        Ok(self.fy2)
    }
    fn proj_z(&self) -> Result<f32, Self::Error> {
        Ok(self.proj_z)
    }
}

/// Projects copied values without any native services or live aliases.
pub fn project(frame: &ScreenFrame, v: FVector) -> Projection {
    let v = [v.x, v.y, v.z];
    let mut output = Projection {
        visible: false,
        screen_x: 0.0,
        screen_y: 0.0,
        scale: 0.0,
    };
    let visible = unsafe {
        project_ordered(
            frame,
            v.as_ptr(),
            &raw mut output.screen_x,
            &raw mut output.screen_y,
            &raw mut output.scale,
        )
    }
    .unwrap_or_else(|never| match never {});
    output.visible = visible != 0;
    output
}

/// Deprojects copied values through the same policy as live SDK access.
pub fn deproject(frame: &ScreenFrame, screen_x: i32, screen_y: i32) -> (bool, FVector) {
    let mut v = [0.0; 3];
    let result = unsafe { deproject_ordered(frame, screen_x, screen_y, v.as_mut_ptr()) }
        .unwrap_or_else(|never| match never {});
    (
        result != 0,
        FVector {
            x: v[0],
            y: v[1],
            z: v[2],
        },
    )
}

/// `URender::Project(Frame, V, ScreenX, ScreenY, Scale)`.
///
/// # Safety
/// `v` must point to three readable floats, and the screen outputs must be
/// writable. `scale` may be null. All pointers may alias native frame fields
/// and each other, and must remain valid through every service callback.
pub unsafe fn project_live(
    native: &Native,
    v: *const f32,
    screen_x: *mut f32,
    screen_y: *mut f32,
    scale: *mut f32,
) -> Result<i32, ()> {
    unsafe { project_ordered(native, v, screen_x, screen_y, scale) }
}

unsafe fn project_ordered<F: FrameAccess>(
    native: &F,
    v: *const f32,
    screen_x: *mut f32,
    screen_y: *mut f32,
    scale: *mut f32,
) -> Result<i32, F::Error> {
    let origin = native.coords()?.origin;
    let v = unsafe {
        FVector {
            x: v.read(),
            y: v.add(1).read(),
            z: v.add(2).read(),
        }
    };
    let temp = vector_sub(v, origin);

    // The source tests RendMap separately in each arm, rather than switching
    // on a retained frame snapshot.
    let ortho = if native.rend_map()? == REN_ORTH_XY {
        Some((temp.x, temp.y))
    } else if native.rend_map()? == REN_ORTH_XZ {
        Some((temp.x, -temp.z))
    } else if native.rend_map()? == REN_ORTH_YZ {
        Some((temp.y, -temp.z))
    } else {
        None
    };
    if let Some((x, y)) = ortho {
        let x = x / native.zoom()? + native.fx2()?;
        unsafe { screen_x.write(x) };
        let y = y / native.zoom()? + native.fy2()?;
        unsafe { screen_y.write(y) };
        if !scale.is_null() {
            let value = 1.0_f32 / native.zoom()?;
            unsafe { scale.write(value) };
        }
        return Ok(1);
    }

    let temp = vector_transform(temp, native.coords()?);
    let mut z = temp.z;
    if z.abs() < 0.01_f32 {
        z += 0.02_f32;
    }
    let rz = native.proj_z()? / z;
    // Match native contraction, but never read FY2 before publishing ScreenX.
    let x = temp.x.mul_add(rz, native.fx2()?);
    unsafe { screen_x.write(x) };
    let y = temp.y.mul_add(rz, native.fy2()?);
    unsafe { screen_y.write(y) };
    if !scale.is_null() {
        unsafe { scale.write(rz) };
    }
    Ok(i32::from(z > 1.0_f32))
}

/// `URender::Deproject(Frame, ScreenX, ScreenY, V)`.
///
/// # Safety
/// `v` must point to three writable floats and remain valid through every
/// service callback. It may alias native frame fields. No exclusive borrow
/// of the output or a native owner is retained across a callback.
pub unsafe fn deproject_live(
    native: &Native,
    screen_x: i32,
    screen_y: i32,
    v: *mut f32,
) -> Result<i32, ()> {
    unsafe { deproject_ordered(native, screen_x, screen_y, v) }
}

unsafe fn deproject_ordered<F: FrameAccess>(
    native: &F,
    screen_x: i32,
    screen_y: i32,
    v: *mut f32,
) -> Result<i32, F::Error> {
    let origin = native.coords()?.origin;
    let sx = screen_x as f32 - native.fx2()?;
    let sy = screen_y as f32 - native.fy2()?;
    match native.rend_map()? {
        REN_ORTH_XY => {
            let x = sx.mul_add(native.zoom()?, origin.x);
            unsafe { v.write(x) };
            let y = sy.mul_add(native.zoom()?, origin.y);
            unsafe {
                v.add(1).write(y);
                v.add(2).write(0.0);
            }
            Ok(1)
        }
        REN_ORTH_XZ => {
            let x = sx.mul_add(native.zoom()?, origin.x);
            unsafe {
                v.write(x);
                v.add(1).write(0.0);
            }
            let z = (-sy).mul_add(native.zoom()?, origin.z);
            unsafe { v.add(2).write(z) };
            Ok(1)
        }
        REN_ORTH_YZ => {
            unsafe { v.write(0.0) };
            let y = sx.mul_add(native.zoom()?, origin.y);
            unsafe { v.add(1).write(y) };
            let z = (-sy).mul_add(native.zoom()?, origin.z);
            unsafe { v.add(2).write(z) };
            Ok(1)
        }
        _ => {
            unsafe {
                v.write(origin.x);
                v.add(1).write(origin.y);
                v.add(2).write(origin.z);
            }
            Ok(0)
        }
    }
}

/// `int SolveQuadratic(float a_results[2], float a, float b, float c)`.
/// Returns the C++ `int` result and the two roots (valid for result 1 and 2;
/// result 1 writes both slots equal, result 0 leaves them untouched and
/// returns `[0.0; 2]` here).
pub fn solve_quadratic(a: f32, mut b: f32, c: f32) -> (i32, [f32; 2]) {
    if a == 0.0 {
        if b == 0.0 {
            return (0, [0.0; 2]);
        }
        let r = -c / b;
        return (1, [r, r]);
    }

    b *= -0.5;
    // SReal discrim = b*b - a*c;  (float expression, then widened)
    // contract: fma(b, b, -(a*c))
    let discrim_f = b.mul_add(b, -(a * c));
    let mut discrim = discrim_f as f64;
    if discrim < 0.0 {
        return (0, [0.0; 2]);
    }
    if discrim == 0.0 {
        let r = b / a;
        return (1, [r, r]);
    }

    discrim = discrim.sqrt();
    if a < 0.0 {
        discrim = -discrim;
    }
    let inva = 1.0 / a as f64;
    let r0 = ((b as f64 - discrim) * inva) as f32;
    let r1 = ((b as f64 + discrim) * inva) as f32;
    (2, [r0, r1])
}
