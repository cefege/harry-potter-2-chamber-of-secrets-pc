//! Replays the captured original `UnSoftLn.cpp` outputs for `SolveQuadratic`,
//! `URender::Project`, and `URender::Deproject` (compiled standalone with the
//! reference clang flags, default `-ffp-contract=on`) and requires bit-identical
//! results for every input line.

use hp2_abi::{FCoords, FVector};
use hp2_render::softln::{ScreenFrame, deproject, project, solve_quadratic};

const FIXTURE: &str = include_str!("fixtures/softln_reference.txt");

fn hex_bits(word: &str) -> u32 {
    u32::from_str_radix(word, 16).expect("hex word")
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_bits(word))
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

#[test]
fn solve_quadratic_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        let Some(rest) = line.strip_prefix('Q') else {
            continue;
        };
        let t: Vec<&str> = rest.split_whitespace().collect();
        let a = f32::from_bits(hex_bits(t[0]));
        let b = f32::from_bits(hex_bits(t[1]));
        let c = f32::from_bits(hex_bits(t[2]));
        let want_ret: i32 = t[3].parse().expect("ret");
        let (ret, roots) = solve_quadratic(a, b, c);
        assert_eq!(ret, want_ret, "line {n}: Q ret");
        if want_ret > 0 {
            assert_eq!(roots[0].to_bits(), hex_bits(t[4]), "line {n}: root0");
            assert_eq!(roots[1].to_bits(), hex_bits(t[5]), "line {n}: root1");
        }
        checked += 1;
    }
    assert!(checked >= 4000, "fixture has {checked} Q cases");
}

#[test]
fn project_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        let Some(rest) = line.strip_prefix('P') else {
            continue;
        };
        let t: Vec<&str> = rest.split_whitespace().collect();
        let rend_map: i32 = t[0].parse().expect("map");
        let v = vec3(&t[1..4]);
        let origin = vec3(&t[4..7]);
        let coords = FCoords {
            origin,
            x_axis: vec3(&t[7..10]),
            y_axis: vec3(&t[10..13]),
            z_axis: vec3(&t[13..16]),
        };
        let frame = ScreenFrame {
            rend_map,
            coords,
            zoom: hex_f32(t[16]),
            fx2: hex_f32(t[17]),
            fy2: hex_f32(t[18]),
            proj_z: hex_f32(t[21]),
        };
        let want_visible = t[22] == "1";
        let p = project(&frame, v);
        assert_eq!(p.visible, want_visible, "line {n}: P visible");
        assert_eq!(p.screen_x.to_bits(), hex_bits(t[23]), "line {n}: screen_x");
        assert_eq!(p.screen_y.to_bits(), hex_bits(t[24]), "line {n}: screen_y");
        assert_eq!(p.scale.to_bits(), hex_bits(t[25]), "line {n}: scale");
        checked += 1;
    }
    assert!(checked >= 2400, "fixture has {checked} P cases");
}

#[test]
fn deproject_matches_original_cpp_capture_bit_for_bit() {
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        let Some(rest) = line.strip_prefix('D') else {
            continue;
        };
        let t: Vec<&str> = rest.split_whitespace().collect();
        let rend_map: i32 = t[0].parse().expect("map");
        let sx = hex_bits(t[1]) as i32;
        let sy = hex_bits(t[2]) as i32;
        let origin = vec3(&t[6..9]);
        let frame = ScreenFrame {
            rend_map,
            coords: FCoords {
                origin,
                x_axis: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                y_axis: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                z_axis: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            zoom: hex_f32(t[3]),
            fx2: hex_f32(t[4]),
            fy2: hex_f32(t[5]),
            proj_z: 0.0,
        };
        let want_ret = t[9] == "1";
        let want_v = vec3(&t[10..13]);
        let (ret, v) = deproject(&frame, sx, sy);
        assert_eq!(ret, want_ret, "line {n}: D ret");
        assert_vec_bits(v, want_v, &format!("line {n}: D V"));
        checked += 1;
    }
    assert!(checked >= 1200, "fixture has {checked} D cases");
}
