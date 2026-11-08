//! Portable scalar policy for the eight-byte native `FTime` in `Core.h`.
//!
//! The historical expression types matter: ordinary `*` and `/` feed a float
//! into the seconds constructor, whereas their compound forms cast to raw ticks.
//! `+` rounds its delta in binary32; subtraction subtracts ticks before converting.
//! The contract covers defined C++ operations under the usual nearest-even FP
//! environment. Historical signed overflow, NaN-to-integer and out-of-range raw
//! casts have no matching requirement: here overflow wraps, casts saturate and
//! NaNs convert to zero. Constructor infinities do have defined clamp behavior.

const FIXTIME: f32 = 4_294_967_296.0;

/// Representation tags used only at the native long-double boundary.
pub const LONG_DOUBLE_BINARY64: u32 = 1;
pub const LONG_DOUBLE_BINARY128: u32 = 2;
pub const LONG_DOUBLE_X87: u32 = 3;

/// Scale an exact binary significand by 2^32 and truncate toward zero.
/// No floating-point narrowing is involved, including for IEEE binary128.
fn scaled_binary(negative: bool, significand: u128, exponent: i32) -> i64 {
    if significand == 0 {
        return 0;
    }
    let shift = exponent + 32;
    let top = 127 - significand.leading_zeros() as i32 + shift;
    if top >= 63 {
        return if negative { i64::MIN } else { i64::MAX };
    }
    if top < 0 {
        return 0;
    }
    let magnitude = if shift >= 0 {
        significand << shift as u32
    } else {
        significand >> (-shift) as u32
    } as i64;
    if negative { -magnitude } else { magnitude }
}

fn special(negative: bool, nan: bool) -> i64 {
    if nan {
        0
    } else if negative {
        i64::MIN
    } else {
        i64::MAX
    }
}

/// IEEE binary64 seconds, including the Apple arm64 long-double representation.
pub fn from_binary64(bits: u64) -> i64 {
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    if exponent == 0x7ff {
        return special(negative, fraction != 0);
    }
    let significand = fraction | if exponent == 0 { 0 } else { 1 << 52 };
    scaled_binary(negative, significand as u128, exponent.max(1) - 1023 - 52)
}

/// IEEE binary128 seconds, as used by Linux arm64 long double.
pub fn from_binary128(bits: u128) -> i64 {
    let negative = bits >> 127 != 0;
    let exponent = ((bits >> 112) & 0x7fff) as i32;
    let fraction = bits & ((1_u128 << 112) - 1);
    if exponent == 0x7fff {
        return special(negative, fraction != 0);
    }
    let significand = fraction | if exponent == 0 { 0 } else { 1 << 112 };
    scaled_binary(negative, significand, exponent.max(1) - 16383 - 112)
}

/// x87 extended seconds. The integer significand bit is explicit; padding is
/// never read. Unsupported noncanonical encodings are outside the C++ contract.
pub fn from_x87(significand: u64, sign_exponent: u16) -> i64 {
    let negative = sign_exponent >> 15 != 0;
    let exponent = (sign_exponent & 0x7fff) as i32;
    if exponent == 0x7fff {
        return special(negative, significand != 0x8000_0000_0000_0000);
    }
    scaled_binary(negative, significand as u128, exponent.max(1) - 16383 - 63)
}

#[inline]
pub fn from_float(seconds: f32) -> i64 {
    from_binary64((seconds as f64).to_bits())
}

#[inline]
pub fn from_double(seconds: f64) -> i64 {
    from_binary64(seconds.to_bits())
}

#[inline]
pub fn get_float(value: i64) -> f32 {
    value as f32 / FIXTIME
}

/// Shared by `+` and `+=`: the delta product is binary32, not binary64.
#[inline]
pub fn add(value: i64, seconds: f32) -> i64 {
    value.wrapping_add((seconds * FIXTIME) as i64)
}

#[inline]
pub fn subtract(value: i64, other: i64) -> f32 {
    value.wrapping_sub(other) as f32 / FIXTIME
}

#[inline]
pub fn multiply(value: i64, factor: f32) -> i64 {
    from_float(value as f32 * factor)
}

#[inline]
pub fn divide(value: i64, divisor: f32) -> i64 {
    from_float(value as f32 / divisor)
}

#[inline]
pub fn multiply_assign(value: i64, factor: f32) -> i64 {
    (value as f32 * factor) as i64
}

#[inline]
pub fn divide_assign(value: i64, divisor: f32) -> i64 {
    (value as f32 / divisor) as i64
}

#[inline]
pub fn equal(value: i64, other: i64) -> i32 {
    i32::from(value == other)
}

#[inline]
pub fn not_equal(value: i64, other: i64) -> i32 {
    i32::from(value != other)
}

#[inline]
pub fn greater(value: i64, other: i64) -> i32 {
    i32::from(value > other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_truncate_and_clamp() {
        assert_eq!(from_double(0.0), 0);
        assert_eq!(from_float(-0.0), 0);
        assert_eq!(from_double(1.0), 1_i64 << 32);
        assert_eq!(from_double(-1.0), -(1_i64 << 32));
        assert_eq!(from_double(1.5 / 4_294_967_296.0), 1);
        assert_eq!(from_double(-1.5 / 4_294_967_296.0), -1);
        assert_eq!(from_double(2_147_483_648.0), i64::MAX);
        assert_eq!(from_double(-2_147_483_648.0), i64::MIN);
        assert_eq!(
            from_double(f64::from_bits(0x41df_ffff_ffff_ffff)),
            i64::MAX - 1023
        );
        assert_eq!(from_double(f64::MAX), i64::MAX);
        assert_eq!(from_double(-f64::MAX), i64::MIN);
        assert_eq!(from_double(f64::INFINITY), i64::MAX);
        assert_eq!(from_double(f64::NEG_INFINITY), i64::MIN);
        assert_eq!(from_double(f64::from_bits(1)), 0);
    }

    #[test]
    fn extended_precision_is_not_narrowed_to_double() {
        // 2^31 - 2^-31 seconds: two ticks below 2^63. Binary64
        // conversion would instead round to 2^31 and incorrectly clamp.
        let just_below = (16413_u128 << 112) | ((1_u128 << 112) - (1_u128 << 51));
        assert_eq!(from_binary128(just_below), i64::MAX - 1);
        assert_eq!(from_binary128(just_below | (1_u128 << 127)), i64::MIN + 2);
        assert_eq!(from_x87(u64::MAX - 3, 16413), i64::MAX - 1);
        assert_eq!(from_x87(u64::MAX - 3, 16413 | 0x8000), i64::MIN + 2);
        assert_eq!(from_binary128(16383_u128 << 112), 1_i64 << 32);
        assert_eq!(from_binary128(1), 0);
        assert_eq!(from_x87(1, 0), 0);
        assert_eq!(from_binary128(0x7fff_u128 << 112), i64::MAX);
        assert_eq!(from_x87(1_u64 << 63, 0xffff), i64::MIN);
    }

    #[test]
    fn operators_keep_the_original_expression_types() {
        let tick = from_double(1.0 / 4_294_967_296.0);
        assert_eq!(tick, 1);
        assert_eq!(multiply(tick, 2.0), 2_i64 << 32);
        assert_eq!(multiply_assign(tick, 2.0), 2);
        assert_eq!(divide(tick, 2.0), 1_i64 << 31);
        assert_eq!(divide_assign(tick, 2.0), 0);
        assert_eq!(multiply_assign(-3, 0.5), -1);
        assert_eq!(add(100, -1.5 / FIXTIME), 99);
        assert_eq!(
            subtract(16_777_217, 16_777_216).to_bits(),
            (1.0 / FIXTIME).to_bits()
        );
        assert_eq!(
            get_float(16_777_217).to_bits(),
            (16_777_216.0 / FIXTIME).to_bits()
        );
        assert_eq!(equal(-7, -7), 1);
        assert_eq!(not_equal(-7, 7), 1);
        assert_eq!(greater(-7, 7), 0);
    }
}
