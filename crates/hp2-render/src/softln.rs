//! Port of the pure-math part of `Render/Src/UnSoftLn.cpp`: `SolveQuadratic`,
//! `URender::Project`, `URender::Deproject`.
//!
//! Float fidelity: the reference build is clang with `-ffp-contract=on`, so
//! every `a*b + c` / `a*b - c` the C++ writes as a single expression is fused
//! into one `fma` (the LHS multiply is the fused one). Each such site is
//! marked with a `// contract:` comment and uses `f32::mul_add` in the same
//! operand order.

pub use hp2_abi::{FCoords, FVector};

/// `ERendMap` values (`Engine/Inc/UnCamera.h`) handled by Project/Deproject.
pub const REN_ORTH_XY: i32 = 13;
pub const REN_ORTH_XZ: i32 = 14;
pub const REN_ORTH_YZ: i32 = 15;

/// The `FSceneNode` fields `Project`/`Deproject` read, plus the viewport's
/// `RendMap` (`Frame->Viewport->Actor->RendMap`), passed by value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenFrame {
    pub rend_map: i32,
    pub coords: FCoords,
    pub zoom: f32,
    pub fx2: f32,
    pub fy2: f32,
    /// `Frame->Proj.Z`.
    pub proj_z: f32,
}

impl ScreenFrame {
    /// Builds the frame from `FCoords` flattened to 12 floats (Origin, XAxis,
    /// YAxis, ZAxis; each X, Y, Z), the layout the FFI passes across.
    pub fn from_flat(
        rend_map: i32,
        c: &[f32; 12],
        zoom: f32,
        fx2: f32,
        fy2: f32,
        proj_z: f32,
    ) -> Self {
        let v = |i: usize| FVector {
            x: c[i],
            y: c[i + 1],
            z: c[i + 2],
        };
        Self {
            rend_map,
            coords: FCoords {
                origin: v(0),
                x_axis: v(3),
                y_axis: v(6),
                z_axis: v(9),
            },
            zoom,
            fx2,
            fy2,
            proj_z,
        }
    }
}

/// Outputs of `URender::Project`: the `UBOOL` return, `ScreenX`, `ScreenY`,
/// and `*Scale` (always written here; the C++ writes it only when non-null).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub visible: bool,
    pub screen_x: f32,
    pub screen_y: f32,
    pub scale: f32,
}

/// `FVector::operator|` (active, non-PS2 branch): `X*V.X + Y*V.Y + Z*V.Z`.
/// Contraction (`fadd(fadd(mul,mul),mul)`, LHS-first fusion):
/// `fma(Z,V.Z, fma(X,V.X, Y*V.Y))`.
fn dot(a: FVector, b: FVector) -> f32 {
    // contract: fma(Z,V.Z, fma(X,V.X, Y*V.Y))
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `FVector::TransformVectorBy` (active branch): dot with each axis.
fn transform_vector_by(v: FVector, coords: &FCoords) -> FVector {
    FVector {
        x: dot(v, coords.x_axis),
        y: dot(v, coords.y_axis),
        z: dot(v, coords.z_axis),
    }
}

fn sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

/// `URender::Project(Frame, V, ScreenX, ScreenY, Scale)`.
pub fn project(frame: &ScreenFrame, v: FVector) -> Projection {
    let temp = sub(v, frame.coords.origin);
    let zoom = frame.zoom;
    match frame.rend_map {
        REN_ORTH_XY => Projection {
            visible: true,
            screen_x: temp.x / zoom + frame.fx2,
            screen_y: temp.y / zoom + frame.fy2,
            scale: 1.0 / zoom,
        },
        REN_ORTH_XZ => Projection {
            visible: true,
            screen_x: temp.x / zoom + frame.fx2,
            screen_y: -temp.z / zoom + frame.fy2,
            scale: 1.0 / zoom,
        },
        REN_ORTH_YZ => Projection {
            visible: true,
            screen_x: temp.y / zoom + frame.fx2,
            screen_y: -temp.z / zoom + frame.fy2,
            scale: 1.0 / zoom,
        },
        _ => {
            let temp = transform_vector_by(temp, &frame.coords);
            let mut z = temp.z;
            if z.abs() < 0.01 {
                z += 0.02;
            }
            let rz = frame.proj_z / z;
            // contract: Temp.X * RZ + FX2
            let screen_x = temp.x.mul_add(rz, frame.fx2);
            // contract: Temp.Y * RZ + FY2
            let screen_y = temp.y.mul_add(rz, frame.fy2);
            Projection {
                visible: z > 1.0,
                screen_x,
                screen_y,
                scale: rz,
            }
        }
    }
}

/// `URender::Deproject(Frame, ScreenX, ScreenY, V)`. Returns the `UBOOL`
/// result and the written `V` (the C++ writes `V = Origin` in the default
/// branch).
pub fn deproject(frame: &ScreenFrame, screen_x: i32, screen_y: i32) -> (bool, FVector) {
    let origin = frame.coords.origin;
    let sx = screen_x as f32 - frame.fx2;
    let sy = screen_y as f32 - frame.fy2;
    let zoom = frame.zoom;
    match frame.rend_map {
        REN_ORTH_XY => (
            true,
            FVector {
                // contract: +SX * Zoom + Origin.X
                x: sx.mul_add(zoom, origin.x),
                y: sy.mul_add(zoom, origin.y),
                z: 0.0,
            },
        ),
        REN_ORTH_XZ => (
            true,
            FVector {
                x: sx.mul_add(zoom, origin.x),
                y: 0.0,
                // contract: -SY * Zoom + Origin.Z
                z: (-sy).mul_add(zoom, origin.z),
            },
        ),
        REN_ORTH_YZ => (
            true,
            FVector {
                x: 0.0,
                y: sx.mul_add(zoom, origin.y),
                z: (-sy).mul_add(zoom, origin.z),
            },
        ),
        _ => (false, origin),
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
