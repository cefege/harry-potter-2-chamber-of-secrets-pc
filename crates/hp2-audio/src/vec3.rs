//! UE1 `FVector` arithmetic, replicated operation-for-operation from
//! `HarryPotter2/Unreal/Core/Inc/UnMath.h` @06bfdd3 so the `f32` results are
//! bit-identical to the C++ they replace.
//!
//! Two details matter for bit-exactness:
//! - `appSqrt` and `appPow` are `DOUBLE` functions (`UnFile.h:420-421`,
//!   `UnAnsi.cpp:135-142`). The C++ computes in double and rounds to `FLOAT`
//!   at the assignment, so [`sqrt`] and [`normalize`] do the same.
//! - `FVector::operator/` multiplies by the reciprocal (`UnMath.h:706-710`),
//!   so [`div`] does too.

/// A UE1 `FVector` as `[X, Y, Z]`.
pub type Vec3 = [f32; 3];

/// `SMALL_NUMBER` (`UnMath.h:31`).
pub const SMALL_NUMBER: f32 = 1.0e-8;

/// `appSqrt(FLOAT)` as used in `FVector::Size` and `Normalize`:
/// double-precision sqrt, rounded to `FLOAT` on return.
#[inline]
pub fn sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// `-V` (`FVector::operator-`).
#[inline]
pub fn neg(v: Vec3) -> Vec3 {
    [-v[0], -v[1], -v[2]]
}

/// `A - B` (`FVector::operator-`).
#[inline]
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `V * Scale` (`FVector::operator*(FLOAT)`).
#[inline]
pub fn scale(v: Vec3, s: f32) -> Vec3 {
    [v[0] * s, v[1] * s, v[2] * s]
}

/// `V / Scale` (`FVector::operator/(FLOAT)`, `UnMath.h:706-710`): multiplies
/// by `1.f/Scale`, not by a true division.
#[inline]
pub fn div(v: Vec3, s: f32) -> Vec3 {
    let r = 1.0f32 / s;
    [v[0] * r, v[1] * r, v[2] * r]
}

/// `A ^ B` (`FVector::operator^`, `UnMath.h:677-683`).
#[inline]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `FVector::Size()` (`UnMath.h:766-769`).
#[inline]
pub fn size(v: Vec3) -> f32 {
    sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

/// `FVector::Normalize()` (`UnMath.h:800-812`). Returns `false` and leaves
/// `v` untouched when the squared length is below `SMALL_NUMBER`.
///
/// The scale is `1.f/appSqrt(SquareSum)`: the division happens in double and
/// the result is rounded to `FLOAT` before the component multiplies.
#[inline]
pub fn normalize(v: &mut Vec3) -> bool {
    let square_sum = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if square_sum >= SMALL_NUMBER {
        let scale = (1.0f32 as f64 / (square_sum as f64).sqrt()) as f32;
        v[0] *= scale;
        v[1] *= scale;
        v[2] *= scale;
        true
    } else {
        false
    }
}

/// `FDistSquared(V1, V2)` (`UnMath.h:2169-2172`).
#[inline]
pub fn dist_squared(v1: Vec3, v2: Vec3) -> f32 {
    let dx = v2[0] - v1[0];
    let dy = v2[1] - v1[1];
    let dz = v2[2] - v1[2];
    dx * dx + dy * dy + dz * dz
}

/// UE1 `Clamp` (`UnTemplate.h:124-127`): `X<Min ? Min : X<Max ? X : Max`.
#[inline]
pub fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// UE1 `Lerp` (`UnTemplate.h:143-146`): `A + Alpha * (B-A)`.
#[inline]
pub fn lerp(a: f32, b: f32, alpha: f32) -> f32 {
    a + alpha * (b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_matches_ue_ternary_chain() {
        assert_eq!(clamp(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(clamp(0.5, 0.0, 1.0), 0.5);
        assert_eq!(clamp(2.0, 0.0, 1.0), 1.0);
        // NaN falls through both comparisons and yields Max, as the C++ does.
        assert_eq!(clamp(f32::NAN, 0.0, 1.0), 1.0);
    }

    #[test]
    fn normalize_rejects_near_zero_vectors() {
        let mut v = [1.0e-5f32, 0.0, 0.0];
        assert!(!normalize(&mut v));
        assert_eq!(v, [1.0e-5, 0.0, 0.0]);
    }

    #[test]
    fn div_uses_reciprocal_multiply() {
        let v = div([3.0, 6.0, 9.0], 3.0);
        assert_eq!(v[0].to_bits(), (3.0f32 * (1.0f32 / 3.0f32)).to_bits());
    }

    // Oracle: `/tmp/hp2-m2/audio/vec3/vec3_oracle.cpp` (original `FVector`
    // operator/, Size, Normalize, operator^, `FDistSquared`, `Clamp`, `Lerp`
    // included verbatim from 06bfdd3), output `vec3_oracle.out`, built with
    // `clang++ -std=c++17 -w -O0 -ffp-contract=off`.
    const INPUTS: [Vec3; 7] = [
        [3.0, 4.0, 0.0],
        [1.0, 2.0, 2.0],
        [0.1, 0.7, -0.3],
        [1e3, -2e3, 5e2],
        [1e-4, 0.0, 0.0],
        [1e-5, 0.0, 0.0],
        [-7.25, 0.3, 11.0],
    ];
    const DIVISORS: [f32; 4] = [3.0, 7.0, 0.1, -2.5];
    const DIV_INPUTS: [Vec3; 4] = [
        [3.0, -1.0, 0.5],
        [7.0, 2.0, -9.0],
        [0.1, 0.2, 0.3],
        [-2.5, 4.25, 6.0],
    ];
    const SIZE_BITS: [u32; 7] = [
        0x40a00000, 0x40400000, 0x3f44a328, 0x450f349b, 0x38d1b717, 0x3727c5ac, 0x4152d7f9,
    ];
    const NORM_OK: [bool; 7] = [true, true, true, true, false, false, true];
    const NORM_BITS: [[u32; 3]; 7] = [
        [0x3f19999a, 0x3f4ccccd, 0x00000000],
        [0x3eaaaaab, 0x3f2aaaab, 0x3f2aaaab],
        [0x3e05503f, 0x3f694c6d, 0xbec7f85e],
        [0x3edf7482, 0xbf5f7482, 0x3e5f7482],
        [0x38d1b717, 0x00000000, 0x00000000],
        [0x3727c5ac, 0x00000000, 0x00000000],
        [0xbf0cd7fd, 0x3cba7f1a, 0x3f55b1a3],
    ];
    const DIV_BITS: [[[u32; 3]; 4]; 4] = [
        [
            [0x3f800000, 0xbeaaaaab, 0x3e2aaaab],
            [0x3edb6db8, 0xbe124925, 0x3d924925],
            [0x41f00000, 0xc1200000, 0x40a00000],
            [0xbf99999a, 0x3ecccccd, 0xbe4ccccd],
        ],
        [
            [0x40155556, 0x3f2aaaab, 0xc0400000],
            [0x3f800000, 0x3e924925, 0xbfa4924a],
            [0x428c0000, 0x41a00000, 0xc2b40000],
            [0xc0333333, 0xbf4ccccd, 0x40666667],
        ],
        [
            [0x3d088889, 0x3d888889, 0x3dccccce],
            [0x3c6a0ea2, 0x3cea0ea2, 0x3d2f8afa],
            [0x3f800000, 0x40000000, 0x40400000],
            [0xbd23d70b, 0xbda3d70b, 0xbdf5c290],
        ],
        [
            [0xbf555556, 0x3fb55556, 0x40000000],
            [0xbeb6db6e, 0x3f1b6db7, 0x3f5b6db8],
            [0xc1c80000, 0x422a0000, 0x42700000],
            [0x3f800000, 0xbfd9999a, 0xc019999a],
        ],
    ];

    fn bits(v: Vec3) -> [u32; 3] {
        [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]
    }

    #[test]
    fn size_matches_cpp_bits() {
        for (i, v) in INPUTS.iter().enumerate() {
            assert_eq!(size(*v).to_bits(), SIZE_BITS[i], "size input {i}");
        }
    }

    #[test]
    fn normalize_matches_cpp_bits_including_rejection() {
        for (i, v) in INPUTS.iter().enumerate() {
            let mut n = *v;
            assert_eq!(normalize(&mut n), NORM_OK[i], "normalize ok input {i}");
            assert_eq!(bits(n), NORM_BITS[i], "normalize bits input {i}");
        }
    }

    #[test]
    fn div_matches_cpp_reciprocal_multiply_bits() {
        for (i, v) in DIV_INPUTS.iter().enumerate() {
            for (j, d) in DIVISORS.iter().enumerate() {
                assert_eq!(
                    bits(div(*v, *d)),
                    DIV_BITS[i][j],
                    "div input {i} divisor {j}"
                );
            }
        }
    }

    #[test]
    fn cross_and_dist_squared_match_cpp_bits() {
        let a = [0.1f32, 0.7, -0.3];
        let b = [1.5f32, -2.25, 4.0];
        assert_eq!(bits(cross(a, b)), [0x40080000, 0xbf59999a, 0xbfa33333]);
        assert_eq!(bits(cross(b, a)), [0xc0080000, 0x3f59999a, 0x3fa33333]);
        assert_eq!(dist_squared(a, b).to_bits(), 0x41e93853);
        assert_eq!(dist_squared([3.0, 4.0, 5.0], [3.0, 4.0, 5.0]).to_bits(), 0);
    }

    #[test]
    fn clamp_and_lerp_match_cpp_bits() {
        assert_eq!(clamp(-1.0, 0.0, 1.0).to_bits(), 0x00000000);
        assert_eq!(clamp(0.5, 0.0, 1.0).to_bits(), 0x3f000000);
        assert_eq!(clamp(2.0, 0.0, 1.0).to_bits(), 0x3f800000);
        assert_eq!(clamp(0.0, 0.0, 1.0).to_bits(), 0x00000000);
        assert_eq!(clamp(f32::NAN, 0.0, 1.0).to_bits(), 0x3f800000);
        assert_eq!(lerp(0.0, 10.0, 0.3).to_bits(), 0x40400000);
        assert_eq!(lerp(1.5, -2.25, 0.7).to_bits(), 0xbf900000);
    }
}
