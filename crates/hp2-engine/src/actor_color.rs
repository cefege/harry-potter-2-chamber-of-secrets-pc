//! Pure `AActor` color operator behavior from `UnScript.cpp`.

use hp2_abi::FColor;

/// RGB is scaled and narrowed like the legacy operator; alpha is preserved.
pub fn multiply_color_float(mut color: FColor, factor: f32) -> FColor {
    color.r = (f32::from(color.r) * factor) as u8;
    color.g = (f32::from(color.g) * factor) as u8;
    color.b = (f32::from(color.b) * factor) as u8;
    color
}

/// Operand order is retained at the native boundary; scalar multiplication is symmetric.
pub fn multiply_float_color(factor: f32, color: FColor) -> FColor {
    multiply_color_float(color, factor)
}

/// The legacy operator adds RGB bytes with BYTE-width wraparound and preserves A's alpha.
pub fn add_color_color(mut a: FColor, b: FColor) -> FColor {
    a.r = a.r.wrapping_add(b.r);
    a.g = a.g.wrapping_add(b.g);
    a.b = a.b.wrapping_add(b.b);
    a
}

/// The legacy operator subtracts RGB bytes with BYTE-width wraparound and preserves A's alpha.
pub fn subtract_color_color(mut a: FColor, b: FColor) -> FColor {
    a.r = a.r.wrapping_sub(b.r);
    a.g = a.g.wrapping_sub(b.g);
    a.b = a.b.wrapping_sub(b.b);
    a
}
