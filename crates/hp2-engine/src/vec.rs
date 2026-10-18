//! `FVector` operators from `Core/Inc/UnMath.h` (generic, non-PSX path), with
//! the same operand order and float/double steps as the C++ bodies.
//!
//! `appSqrt` and `appExp` are `DOUBLE` in the engine, so the square roots and
//! reciprocals below are taken in f64 and narrowed to f32 at the same points.

use hp2_abi::FVector;

use crate::math::app_sqrt;

#[inline]
pub fn zero() -> FVector {
    FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }
}

/// `operator+(const FVector&)`.
#[inline]
pub fn add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

/// `operator-(const FVector&)`.
#[inline]
pub fn sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

/// `operator*(FLOAT)`.
#[inline]
pub fn scale(a: FVector, s: f32) -> FVector {
    FVector {
        x: a.x * s,
        y: a.y * s,
        z: a.z * s,
    }
}

/// `operator/(FLOAT)`: multiplies by `RScale = 1.f / s`.
#[inline]
pub fn div(a: FVector, s: f32) -> FVector {
    let r = 1.0f32 / s;
    scale(a, r)
}

/// `SizeSquared()`: `X*X + Y*Y + Z*Z`. Clang contracts this single expression
/// as `fma(Z, Z, fma(X, X, Y*Y))` (the left product is fused).
#[inline]
pub fn size_squared(a: FVector) -> f32 {
    a.z.mul_add(a.z, a.x.mul_add(a.x, a.y * a.y))
}

/// `Size()`: `appSqrt(X*X + Y*Y + Z*Z)`, double sqrt narrowed to float.
#[inline]
pub fn size(a: FVector) -> f32 {
    app_sqrt(f64::from(size_squared(a))) as f32
}

/// `UnsafeNormal()`: `Scale = 1.f / appSqrt(X*X+Y*Y+Z*Z)`, then componentwise
/// multiply. The reciprocal is computed in double and narrowed to float.
#[inline]
pub fn unsafe_normal(a: FVector) -> FVector {
    let s = (1.0f64 / app_sqrt(f64::from(size_squared(a)))) as f32;
    scale(a, s)
}
