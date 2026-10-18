//! Bit-exact check of the `FDynamicSprite::Setup` pure math in `hp2_render::sprite`
//! against the original expressions captured from `Render/Src/UnSprite.cpp`
//! (`tests/fixtures/sprite_setup_reference.txt`): projection plane (MinZ and
//! reject), box-branch screen bounds, box-axis SizeSquared test, DrawType branch.
//!
//! Captured on arm64 with `-ffp-contract=on` (the dot product is fused), so this
//! file is aarch64-only.
#![cfg(target_arch = "aarch64")]

use hp2_render::sprite::{
    FVector, SetupPath, box_axis_is_empty, box_extent, projection_plane, setup_path,
};

const FIXTURE: &str = include_str!("fixtures/sprite_setup_reference.txt");

fn bits(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn flt(s: &str) -> f32 {
    f32::from_bits(bits(s))
}

fn vec3(t: &[&str]) -> FVector {
    FVector {
        x: flt(t[0]),
        y: flt(t[1]),
        z: flt(t[2]),
    }
}

#[test]
fn projection_plane_matches_capture() {
    let (mut accepted, mut rejected) = (0usize, 0usize);
    let mut n = 0usize;
    for line in FIXTURE.lines().filter(|l| l.starts_with("PP ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), 14, "{line}");
        let loc = vec3(&t[1..4]);
        let org = vec3(&t[4..7]);
        let z = vec3(&t[7..10]);
        let proj = flt(t[10]);
        let ortho = bits(t[11]) != 0;
        let p = projection_plane(loc, org, z, proj, ortho);
        assert_eq!(p.min_z.to_bits(), bits(t[12]), "MinZ mismatch: {line}");
        assert_eq!(!p.reject as u32, bits(t[13]), "accept mismatch: {line}");
        if p.reject {
            rejected += 1;
        } else {
            accepted += 1;
        }
        n += 1;
    }
    assert_eq!(n, 3000);
    assert!(
        accepted > 0 && rejected > 0,
        "both branches must be exercised"
    );
}

#[test]
fn box_extent_matches_capture() {
    let (mut n, mut rejected) = (0usize, 0usize);
    for line in FIXTURE.lines().filter(|l| l.starts_with("BX ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), 14, "{line}");
        let b = box_extent(
            flt(t[1]),
            flt(t[2]),
            flt(t[3]),
            flt(t[4]),
            flt(t[5]),
            flt(t[6]),
        );
        assert_eq!(b.x1, bits(t[7]) as i32, "X1: {line}");
        assert_eq!(b.x2, bits(t[8]) as i32, "X2: {line}");
        assert_eq!(b.y1, bits(t[9]) as i32, "Y1: {line}");
        assert_eq!(b.y2, bits(t[10]) as i32, "Y2: {line}");
        assert_eq!(b.min_z.to_bits(), bits(t[11]), "MinZ: {line}");
        assert_eq!(b.max_z.to_bits(), bits(t[12]), "MaxZ: {line}");
        assert_eq!(!b.reject as u32, bits(t[13]), "accept: {line}");
        if b.reject {
            rejected += 1;
        }
        n += 1;
    }
    assert_eq!(n, 3000);
    assert!(
        rejected > 0 && rejected < n,
        "both branches must be exercised"
    );
}

#[test]
fn box_axis_empty_matches_capture() {
    let mut n = 0usize;
    for line in FIXTURE.lines().filter(|l| l.starts_with("AX ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), 5, "{line}");
        let empty = box_axis_is_empty(vec3(&t[1..4]));
        assert_eq!(empty as u32, bits(t[4]), "{line}");
        n += 1;
    }
    assert_eq!(n, 400);
}

#[test]
fn setup_path_matches_capture() {
    let mut n = 0usize;
    for line in FIXTURE.lines().filter(|l| l.starts_with("SP ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), 3, "{line}");
        let k = bits(t[1]);
        let got = setup_path(
            k & 1 != 0,
            k & 2 != 0,
            k & 4 != 0,
            k & 8 != 0,
            k & 16 != 0,
            k & 32 != 0,
        );
        let want = match bits(t[2]) {
            0 => SetupPath::Reject,
            1 => SetupPath::Sprite,
            2 => SetupPath::Box,
            other => panic!("bad expected path {other}"),
        };
        assert_eq!(got, want, "{line}");
        n += 1;
    }
    assert_eq!(n, 256);
}
