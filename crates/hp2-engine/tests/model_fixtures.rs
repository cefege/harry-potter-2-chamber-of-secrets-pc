//! Replays the original-C++ bound fixtures (`fixtures/model_bound.txt`) against
//! `hp2_engine::model`. The fixtures were captured by compiling `UnMath.cpp` from
//! commit 40bde73 standalone and driving `FBox(const FVector*, INT)` and
//! `FSphere(const FVector*, INT)` with the vertex lists `UModel::BuildBound`
//! collects.
//!
//! Line format: `<n> <n points as 3 hex floats each> => <min xyz> <max xyz>
//! <IsValid> | <sphere center xyz> <sphere W>`. Floats are IEEE-754 bit patterns
//! in hex, so the comparison is bit-exact.

use hp2_abi::{FPlane, FVector};
use hp2_engine::model;

const FIXTURE: &str = include_str!("fixtures/model_bound.txt");

fn bits(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok, 16).expect("hex float bits"))
}

fn vec3<'a>(it: &mut impl Iterator<Item = &'a str>) -> FVector {
    let mut next = || bits(it.next().expect("float token"));
    FVector {
        x: next(),
        y: next(),
        z: next(),
    }
}

fn same_vec(a: FVector, b: FVector) -> bool {
    a.x.to_bits() == b.x.to_bits()
        && a.y.to_bits() == b.y.to_bits()
        && a.z.to_bits() == b.z.to_bits()
}

#[test]
fn bound_and_sphere_match_original_cpp() {
    let mut cases = 0;
    for line in FIXTURE.lines() {
        let mut it = line.split_whitespace();
        let n: usize = it.next().expect("count").parse().expect("count value");
        let pts: Vec<FVector> = (0..n).map(|_| vec3(&mut it)).collect();
        assert_eq!(it.next(), Some("=>"), "line {cases}");
        let min = vec3(&mut it);
        let max = vec3(&mut it);
        let is_valid: i32 = it.next().expect("IsValid").parse().expect("IsValid value");
        assert_eq!(it.next(), Some("|"), "line {cases}");
        let center = vec3(&mut it);
        let w = bits(it.next().expect("sphere W"));

        let bx = model::bound_of_points(pts.iter().copied());
        assert!(same_vec(bx.min, min), "box min, case {cases}");
        assert!(same_vec(bx.max, max), "box max, case {cases}");
        assert_eq!(bx.is_valid, is_valid != 0, "IsValid, case {cases}");

        let sph: FPlane = model::sphere_of_points(pts.iter().copied());
        assert!(
            same_vec(
                FVector {
                    x: sph.x,
                    y: sph.y,
                    z: sph.z
                },
                center
            ),
            "sphere center, case {cases}"
        );
        assert_eq!(sph.w.to_bits(), w.to_bits(), "sphere radius, case {cases}");
        cases += 1;
    }
    assert_eq!(cases, 248);
}
