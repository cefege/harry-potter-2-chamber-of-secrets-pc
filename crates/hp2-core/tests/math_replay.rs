//! Bit-exact replay of the UnMath.cpp oracle fixtures (`Build/math_fixture_harness`).
//!
//! Every record is driven through the Rust port. Inputs are decoded from the
//! recorded IEEE-754 bit patterns, the Rust result is re-encoded to the same
//! words, and the two must match exactly. Record counts are asserted exactly so
//! a silently shrunken fixture fails the test.

use std::collections::BTreeMap;

use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::math::{
    self, APPROX_EXP_BITS, APPROX_MAN_BITS, FBox, FPlace, FQuat, FRotator, NUM_ANGLES,
};

const OPS: &str = include_str!("fixtures/math_ops.txt");
const TABLES: &str = include_str!("fixtures/math_gmath_tables.txt");

const OP_NAMES: [&str; 20] = [
    "vector_rotation",
    "vector_rotation_fma_edge",
    "vector_find_best_axis_vectors",
    "vector_safe_normal",
    "coords_inverse",
    "coords_div_assign",
    "coords_div_rotator",
    "coords_ortho_rotation",
    "coords_from_place",
    "box_from_points",
    "box_from_coords",
    "box_from_box_coords",
    "box_get_coords",
    "box_shift",
    "sphere_from_points",
    "slerp_quat",
    "rotator_vector",
    "mirror_by_vector",
    "intersect_planes2",
    "intersect_planes3",
];
const OP_RECORDS_PER_NAME: usize = 600;
const FMA_EDGE_RECORDS: usize = 64;
const OP_RECORDS_TOTAL: usize = 11464;
const TABLE_RECORDS: [usize; 6] = [
    NUM_ANGLES,
    2 << APPROX_MAN_BITS,
    1 << APPROX_MAN_BITS,
    1 << APPROX_MAN_BITS,
    1 << APPROX_EXP_BITS,
    1 << APPROX_EXP_BITS,
];

struct Tables {
    trig: Box<[f32; NUM_ANGLES]>,
    sqrt_man: Box<[f32; 2 << APPROX_MAN_BITS]>,
    div_sqrt_man: Box<[f32; 1 << APPROX_MAN_BITS]>,
    div_man: Box<[f32; 1 << APPROX_MAN_BITS]>,
    div_sqrt_exp: Box<[f32; 1 << APPROX_EXP_BITS]>,
    div_exp: Box<[f32; 1 << APPROX_EXP_BITS]>,
}

fn filled() -> Tables {
    let mut t = Tables {
        trig: Box::new([0.0; NUM_ANGLES]),
        sqrt_man: Box::new([0.0; 2 << APPROX_MAN_BITS]),
        div_sqrt_man: Box::new([0.0; 1 << APPROX_MAN_BITS]),
        div_man: Box::new([0.0; 1 << APPROX_MAN_BITS]),
        div_sqrt_exp: Box::new([0.0; 1 << APPROX_EXP_BITS]),
        div_exp: Box::new([0.0; 1 << APPROX_EXP_BITS]),
    };
    math::fill_tables(
        &mut t.trig,
        &mut t.sqrt_man,
        &mut t.div_sqrt_man,
        &mut t.div_man,
        &mut t.div_sqrt_exp,
        &mut t.div_exp,
    );
    t
}

fn hex(t: &str) -> u32 {
    u32::from_str_radix(t, 16).unwrap_or_else(|e| panic!("bad hex word {t:?}: {e}"))
}

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

fn vec3(w: &[u32]) -> FVector {
    FVector {
        x: f(w[0]),
        y: f(w[1]),
        z: f(w[2]),
    }
}

fn plane(w: &[u32]) -> FPlane {
    FPlane {
        x: f(w[0]),
        y: f(w[1]),
        z: f(w[2]),
        w: f(w[3]),
    }
}

fn quat(w: &[u32]) -> FQuat {
    FQuat {
        x: f(w[0]),
        y: f(w[1]),
        z: f(w[2]),
        w: f(w[3]),
    }
}

fn rot(w: &[u32]) -> FRotator {
    FRotator {
        pitch: w[0] as i32,
        yaw: w[1] as i32,
        roll: w[2] as i32,
    }
}

fn coords(w: &[u32]) -> FCoords {
    FCoords {
        origin: vec3(&w[0..3]),
        x_axis: vec3(&w[3..6]),
        y_axis: vec3(&w[6..9]),
        z_axis: vec3(&w[9..12]),
    }
}

fn bx(w: &[u32]) -> FBox {
    FBox {
        min: vec3(&w[0..3]),
        max: vec3(&w[3..6]),
        is_valid: w[6] as u8,
    }
}

fn out_v(o: &mut Vec<u32>, v: FVector) {
    o.extend([v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]);
}

fn out_p(o: &mut Vec<u32>, p: FPlane) {
    o.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()]);
}

fn out_q(o: &mut Vec<u32>, q: FQuat) {
    o.extend([q.x.to_bits(), q.y.to_bits(), q.z.to_bits(), q.w.to_bits()]);
}

fn out_c(o: &mut Vec<u32>, c: FCoords) {
    out_v(o, c.origin);
    out_v(o, c.x_axis);
    out_v(o, c.y_axis);
    out_v(o, c.z_axis);
}

fn out_r(o: &mut Vec<u32>, r: FRotator) {
    o.extend([r.pitch as u32, r.yaw as u32, r.roll as u32]);
}

fn out_b(o: &mut Vec<u32>, b: FBox) {
    out_v(o, b.min);
    out_v(o, b.max);
    o.push(b.is_valid as u32);
}

/// Drive one record through the Rust port. `w` holds the recorded input words.
fn replay(name: &str, w: &[u32], trig: &[f32; NUM_ANGLES]) -> Vec<u32> {
    let mut o = Vec::new();
    match name {
        "vector_rotation" | "vector_rotation_fma_edge" => {
            out_r(&mut o, math::vector_rotation(vec3(w)))
        }
        "vector_find_best_axis_vectors" => {
            let (a, b) = math::vector_find_best_axis_vectors(vec3(w));
            out_v(&mut o, a);
            out_v(&mut o, b);
        }
        "vector_safe_normal" => out_v(&mut o, math::vector_safe_normal(vec3(w))),
        "coords_inverse" => out_c(&mut o, math::coords_inverse(&coords(w))),
        "coords_div_assign" => out_c(
            &mut o,
            math::coords_div_assign(coords(&w[0..12]), &coords(&w[12..24])),
        ),
        "coords_div_rotator" => out_c(
            &mut o,
            math::coords_div_rotator(&coords(&w[0..12]), rot(&w[12..15]), trig),
        ),
        "coords_ortho_rotation" => out_r(
            &mut o,
            math::coords_ortho_rotation(&coords(&w[0..12]), trig),
        ),
        "coords_from_place" => {
            // The harness prints FPlace as Pos (3 words) then Quat (4 words).
            let p = FPlace {
                quat: quat(&w[3..7]),
                pos: vec3(&w[0..3]),
            };
            out_c(&mut o, math::coords_from_place(&p));
        }
        "box_from_points" => {
            let n = w[0] as usize;
            let pts: Vec<FVector> = (0..n).map(|k| vec3(&w[1 + 3 * k..4 + 3 * k])).collect();
            out_b(&mut o, math::box_from_points(&pts));
        }
        "box_from_coords" => out_b(&mut o, math::box_from_coords(&coords(w))),
        "box_from_box_coords" => out_b(
            &mut o,
            math::box_from_box_coords(&bx(&w[0..7]), &coords(&w[7..19])),
        ),
        "box_get_coords" => out_c(&mut o, math::box_get_coords(&bx(w))),
        "box_shift" => out_c(&mut o, math::box_shift(&bx(&w[0..7]), &coords(&w[7..19]))),
        "sphere_from_points" => {
            let n = w[0] as usize;
            let pts: Vec<FVector> = (0..n).map(|k| vec3(&w[1 + 3 * k..4 + 3 * k])).collect();
            out_p(&mut o, math::sphere_from_points(&pts));
        }
        "slerp_quat" => out_q(
            &mut o,
            math::slerp_quat(quat(&w[0..4]), quat(&w[4..8]), f(w[8])),
        ),
        "rotator_vector" => out_v(&mut o, math::rotator_vector(rot(w), trig)),
        "mirror_by_vector" => out_v(
            &mut o,
            math::vector_mirror_by_vector(vec3(&w[0..3]), vec3(&w[3..6])),
        ),
        "intersect_planes2" => {
            let (ok, i, d) = math::intersect_planes2(plane(&w[0..4]), plane(&w[4..8]));
            o.push(ok as u32);
            out_v(&mut o, i);
            out_v(&mut o, d);
        }
        "intersect_planes3" => {
            let (ok, i) =
                math::intersect_planes3(plane(&w[0..4]), plane(&w[4..8]), plane(&w[8..12]));
            o.push(ok as u32);
            out_v(&mut o, i);
        }
        other => panic!("unknown fixture case {other:?}"),
    }
    o
}

#[test]
fn gmath_tables_replay_bit_exact() {
    let t = filled();
    let arrays: [&[f32]; 6] = [
        &t.trig[..],
        &t.sqrt_man[..],
        &t.div_sqrt_man[..],
        &t.div_man[..],
        &t.div_sqrt_exp[..],
        &t.div_exp[..],
    ];
    let mut seen: [Vec<bool>; 6] = arrays.map(|a| vec![false; a.len()]);
    let mut counts = [0usize; 6];
    for line in TABLES.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(tok.len(), 6, "{line}");
        assert_eq!(&tok[0..2], &["case", "gmath_table"], "{line}");
        assert_eq!(tok[4], "->", "{line}");
        let tid = hex(tok[2]) as usize;
        let idx = hex(tok[3]) as usize;
        assert!(tid < 6, "{line}");
        assert!(idx < arrays[tid].len(), "{line}");
        assert!(!seen[tid][idx], "duplicate record: {line}");
        seen[tid][idx] = true;
        assert_eq!(arrays[tid][idx].to_bits(), hex(tok[5]), "{line}");
        counts[tid] += 1;
    }
    assert_eq!(counts, TABLE_RECORDS);
    assert_eq!(counts.iter().sum::<usize>(), 21504);
}

#[test]
fn ops_replay_bit_exact() {
    let t = filled();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0usize;
    for line in OPS.lines() {
        let (lhs, rhs) = line
            .split_once(" -> ")
            .unwrap_or_else(|| panic!("missing separator: {line}"));
        let mut lt = lhs.split_whitespace();
        assert_eq!(lt.next(), Some("case"), "{line}");
        let name = lt.next().unwrap_or_else(|| panic!("missing name: {line}"));
        let input: Vec<u32> = lt.map(hex).collect();
        let expect: String = rhs.split_whitespace().collect::<Vec<_>>().join(" ");
        let got = replay(name, &input, &t.trig);
        let got_str = got
            .iter()
            .map(|w| format!("{w:08x}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(got_str, expect, "{line}");
        *counts.entry(name).or_default() += 1;
        total += 1;
    }
    assert_eq!(counts.len(), OP_NAMES.len(), "case names: {counts:?}");
    for name in OP_NAMES {
        assert_eq!(
            counts.get(name).copied(),
            Some(if name == "vector_rotation_fma_edge" {
                FMA_EDGE_RECORDS
            } else {
                OP_RECORDS_PER_NAME
            }),
            "record count for {name}"
        );
    }
    assert_eq!(total, OP_RECORDS_TOTAL);
}
