//! Replays the captured original `UnMeshRnLOD.h` / `UnMeshRn.cpp` expression
//! outputs (compiled with the reference clang flags, default
//! `-ffp-contract=on`, using the real `UnMath.h` operators and templates) and
//! requires bit-identical results for every input line.

use hp2_abi::FVector;
use hp2_render::mesh::{
    LodInputs, depth_key_distance, depth_key_height, div_sqrt_approx, fatness_offset, lod_subset,
    morph_alpha, morph_inv, morph_point, non_morph_subset, tri_normal, unlit,
};

const FIXTURE: &str = include_str!("fixtures/mesh_reference.txt");

fn hex_bits(word: &str) -> u32 {
    u32::from_str_radix(word, 16).expect("hex word")
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_bits(word))
}

fn hex_i32(word: &str) -> i32 {
    hex_bits(word) as i32
}

fn vec3(words: &[&str]) -> FVector {
    FVector {
        x: hex_f32(words[0]),
        y: hex_f32(words[1]),
        z: hex_f32(words[2]),
    }
}

fn assert_vec_bits(got: FVector, want: FVector, ctx: &str) {
    assert_eq!(got.x.to_bits(), want.x.to_bits(), "{ctx} x");
    assert_eq!(got.y.to_bits(), want.y.to_bits(), "{ctx} y");
    assert_eq!(got.z.to_bits(), want.z.to_bits(), "{ctx} z");
}

/// Splits a fixture line into its tag and value words.
fn fields(line: &str) -> (&str, Vec<&str>) {
    let mut it = line.split_whitespace();
    let tag = it.next().expect("tag");
    (tag, it.collect())
}

#[test]
fn lod_subset_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("L ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 17, "line {n}");
        let inputs = LodInputs {
            lod_strength: hex_f32(w[0]),
            collapse_count: hex_i32(w[1]),
            model_verts: hex_i32(w[2]),
            lod_min_verts: hex_i32(w[3]),
            lod_bias: hex_f32(w[4]),
            global_shape_lod: hex_f32(w[5]),
            global_shape_lod_adjust: hex_f32(w[6]),
            x1: hex_i32(w[7]),
            y1: hex_i32(w[8]),
            x2: hex_i32(w[9]),
            y2: hex_i32(w[10]),
            fx: hex_f32(w[11]),
            shape_lod_mode: hex_i32(w[12]),
            shape_lod_fix: hex_f32(w[13]),
        };
        let got = lod_subset(&inputs);
        let ctx = format!("L line {n}");
        assert_eq!(got.do_lod, w[14] == "1", "{ctx} DoLOD");
        assert_eq!(
            got.target_subset.to_bits(),
            hex_bits(w[15]),
            "{ctx} TargetSubset"
        );
        assert_eq!(got.vertex_subset, hex_i32(w[16]), "{ctx} VertexSubset");
        checked += 1;
    }
    assert!(checked >= 4000, "fixture has {checked} L cases");
}

#[test]
fn morph_setup_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("N ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 4, "line {n}");
        let target = hex_f32(w[0]);
        let lod_morph = hex_f32(w[1]);
        let ctx = format!("N line {n}");
        assert_eq!(
            non_morph_subset(target, lod_morph),
            hex_i32(w[2]),
            "{ctx} NonMorph"
        );
        assert_eq!(
            morph_inv(lod_morph, target).to_bits(),
            hex_bits(w[3]),
            "{ctx} InvLODMorph"
        );
        checked += 1;
    }
    assert!(checked >= 1000, "fixture has {checked} N cases");
}

#[test]
fn morph_alpha_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("A ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 4, "line {n}");
        let got = morph_alpha(hex_i32(w[0]), hex_i32(w[1]), hex_f32(w[2]));
        assert_eq!(got.to_bits(), hex_bits(w[3]), "A line {n} Alpha");
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} A cases");
}

#[test]
fn morph_point_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("P ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 10, "line {n}");
        let got = morph_point(vec3(&w[0..3]), vec3(&w[3..6]), hex_f32(w[6]));
        assert_vec_bits(got, vec3(&w[7..10]), &format!("P line {n}"));
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} P cases");
}

#[test]
fn unlit_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("U ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 3, "line {n}");
        let ambient = u8::from_str_radix(w[1], 16).expect("byte");
        let got = unlit(hex_f32(w[0]), ambient);
        assert_eq!(got.to_bits(), hex_bits(w[2]), "U line {n}");
        checked += 1;
    }
    assert!(checked >= 2000, "fixture has {checked} U cases");
}

#[test]
fn fatness_offset_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("F ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 2, "line {n}");
        let fatness = u8::from_str_radix(w[0], 16).expect("byte");
        assert_eq!(
            fatness_offset(fatness).to_bits(),
            hex_bits(w[1]),
            "F line {n}"
        );
        checked += 1;
    }
    assert_eq!(checked, 256, "fixture has {checked} F cases");
}

#[test]
fn depth_key_height_matches_original_cpp_capture() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("H ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 5, "line {n}");
        let got = depth_key_height(hex_f32(w[0]), hex_f32(w[1]), hex_f32(w[2]), hex_f32(w[3]));
        assert_eq!(got, hex_i32(w[4]), "H line {n}");
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} H cases");
}

#[test]
fn depth_key_distance_matches_original_cpp_capture() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("D ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 13, "line {n}");
        let got = depth_key_distance(
            vec3(&w[0..3]),
            vec3(&w[3..6]),
            vec3(&w[6..9]),
            vec3(&w[9..12]),
        );
        assert_eq!(got, hex_i32(w[12]), "D line {n}");
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} D cases");
}

#[test]
fn tri_normal_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("T ") {
            continue;
        }
        let (_, w) = fields(line);
        assert_eq!(w.len(), 14, "line {n}");
        let got = tri_normal(vec3(&w[0..3]), vec3(&w[3..6]), vec3(&w[6..9]));
        assert_vec_bits(got, vec3(&w[11..14]), &format!("T line {n}"));
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} T cases");
}

#[test]
fn div_sqrt_approx_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("T ") {
            continue;
        }
        let (_, w) = fields(line);
        let sq_want = hex_f32(w[9]);
        let s_want = hex_f32(w[10]);
        assert_eq!(
            div_sqrt_approx(sq_want).to_bits(),
            s_want.to_bits(),
            "T line {n} DivSqrtApprox"
        );
        checked += 1;
    }
    assert!(checked >= 3000, "fixture has {checked} T cases");
}
