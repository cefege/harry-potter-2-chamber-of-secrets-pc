//! Named equivalents of the engine's math helpers (`Core/Inc/UnGnuG.h`,
//! `UnFile.h`), so each call site states which C function it mirrors.

/// `appRound(FLOAT)`: `static_cast<INT>(f)`, truncation toward zero.
#[inline]
pub fn app_round(f: f32) -> i32 {
    f as i32
}

/// `appFloor(FLOAT)`: `static_cast<INT>(f)`, truncation toward zero (not a
/// mathematical floor for negative inputs).
#[inline]
pub fn app_floor(f: f32) -> i32 {
    f as i32
}

/// `appExp(DOUBLE)`.
#[inline]
pub fn app_exp(v: f64) -> f64 {
    v.exp()
}

/// `appSqrt(DOUBLE)`.
#[inline]
pub fn app_sqrt(v: f64) -> f64 {
    v.sqrt()
}
