//! Bit-exact check of `hp2_render::particle` against outputs captured from
//! the original `Render/Src/UnParticleRn.cpp` helpers
//! (`tests/fixtures/particle_reference.txt`, 1500 random cases per op).
//!
//! Captured on arm64 with `-ffp-contract=on`, so the whole file is aarch64-only.
#![cfg(target_arch = "aarch64")]

use std::mem::offset_of;

use hp2_abi::render_particle::FTransform;
use hp2_abi::{FPlane, FVector};
use hp2_render::particle::{
    FTransTexture, ParticleFrame, combine_clip_codes, compute_clip_codes_2d, lerp_plane,
    lerp_trans, lerp_vector, set_trans, set_trans_2d, set_uv,
};

const FIXTURE: &str = include_str!("fixtures/particle_reference.txt");

fn f(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}
fn i(s: &str) -> i32 {
    u32::from_str_radix(s, 16).unwrap() as i32
}
fn byte(s: &str) -> u8 {
    u8::from_str_radix(s, 16).unwrap()
}
fn word_byte(s: &str) -> u8 {
    // `%08x` of a BYTE field (tt_dump / SETTRANS2D input).
    u32::from_str_radix(s, 16).unwrap() as u8
}

struct Rec {
    op: String,
    inp: Vec<String>,
    out: Vec<String>,
}

fn records() -> Vec<Rec> {
    FIXTURE
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("layout"))
        .map(|l| {
            let (lhs, rhs) = l.split_once(" |").expect("record separator");
            let mut it = lhs.split_whitespace();
            let op = it.next().unwrap().to_string();
            Rec {
                op,
                inp: it.map(str::to_string).collect(),
                out: rhs.split_whitespace().map(str::to_string).collect(),
            }
        })
        .collect()
}

fn of(op: &str) -> Vec<Rec> {
    records().into_iter().filter(|r| r.op == op).collect()
}

fn frame(s: &[String]) -> ParticleFrame {
    ParticleFrame {
        proj_z: f(&s[0]),
        rproj_z: f(&s[1]),
        fx15: f(&s[2]),
        fy15: f(&s[3]),
        prjxm: f(&s[4]),
        prjxp: f(&s[5]),
        prjym: f(&s[6]),
        prjyp: f(&s[7]),
    }
}

fn vec3(s: &[String]) -> FVector {
    FVector {
        x: f(&s[0]),
        y: f(&s[1]),
        z: f(&s[2]),
    }
}

fn plane(s: &[String]) -> FPlane {
    FPlane {
        x: f(&s[0]),
        y: f(&s[1]),
        z: f(&s[2]),
        w: f(&s[3]),
    }
}

fn zero_tt() -> FTransTexture {
    FTransTexture {
        transform: FTransform {
            point: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            flags: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            int_y: 0,
            rz: 0.0,
        },
        normal: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        light: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        fog: FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        u: 0.0,
        v: 0.0,
    }
}

/// Compares the transformed-vertex fields `tt_dump` prints, bit-for-bit.
/// Input order is the `tt_dump` order: point(3), flags, screen x/y, int y,
/// rz, u, v (10 tokens).
fn check_tt(tt: &FTransTexture, out: &[String], ctx: &str) {
    let t = &tt.transform;
    assert_eq!(t.point.x.to_bits(), f(&out[0]).to_bits(), "{ctx} point.x");
    assert_eq!(t.point.y.to_bits(), f(&out[1]).to_bits(), "{ctx} point.y");
    assert_eq!(t.point.z.to_bits(), f(&out[2]).to_bits(), "{ctx} point.z");
    assert_eq!(t.flags, word_byte(&out[3]), "{ctx} flags");
    assert_eq!(t.screen_x.to_bits(), f(&out[4]).to_bits(), "{ctx} screen_x");
    assert_eq!(t.screen_y.to_bits(), f(&out[5]).to_bits(), "{ctx} screen_y");
    assert_eq!(t.int_y, i(&out[6]), "{ctx} int_y");
    assert_eq!(t.rz.to_bits(), f(&out[7]).to_bits(), "{ctx} rz");
    assert_eq!(tt.u.to_bits(), f(&out[8]).to_bits(), "{ctx} u");
    assert_eq!(tt.v.to_bits(), f(&out[9]).to_bits(), "{ctx} v");
}

fn tt_from_dump(inp: &[String]) -> FTransform {
    FTransform {
        point: vec3(&inp[0..3]),
        flags: word_byte(&inp[3]),
        screen_x: f(&inp[4]),
        screen_y: f(&inp[5]),
        int_y: i(&inp[6]),
        rz: f(&inp[7]),
    }
}

#[test]
fn layout_matches_captured_c_layout() {
    use hp2_abi::render_particle::FTransTexture as T;
    let layout = FIXTURE.lines().find(|l| l.starts_with("layout")).unwrap();
    let toks: Vec<&str> = layout.split_whitespace().collect();
    let num = |key: &str| -> usize {
        let p = toks.iter().position(|t| *t == key).unwrap();
        toks[p + 1].parse().unwrap()
    };
    assert_eq!(std::mem::size_of::<T>(), num("size"));
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, point),
        num("point")
    );
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, flags),
        num("flags")
    );
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, screen_x),
        num("screenx")
    );
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, screen_y),
        num("screeny")
    );
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, int_y),
        num("inty")
    );
    assert_eq!(
        offset_of!(T, transform) + offset_of!(FTransform, rz),
        num("rz")
    );
    assert_eq!(offset_of!(T, normal), num("normal"));
    assert_eq!(offset_of!(T, light), num("light"));
    assert_eq!(offset_of!(T, fog), num("fog"));
    assert_eq!(offset_of!(T, u), num("u"));
    assert_eq!(offset_of!(T, v), num("v"));
}

#[test]
fn set_uv_matches_capture() {
    let recs = of("SETUV");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let mut tt = zero_tt();
        set_uv(
            &mut tt,
            i(&r.inp[0]),
            i(&r.inp[1]),
            f(&r.inp[2]),
            f(&r.inp[3]),
        );
        assert_eq!(tt.u.to_bits(), f(&r.out[0]).to_bits(), "{:?}", r.inp);
        assert_eq!(tt.v.to_bits(), f(&r.out[1]).to_bits(), "{:?}", r.inp);
    }
}

#[test]
fn set_trans_matches_capture() {
    let recs = of("SETTRANS");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let fr = frame(&r.inp[2..10]);
        let mut tt = zero_tt();
        set_trans(
            &mut tt,
            &fr,
            i(&r.inp[0]),
            i(&r.inp[1]),
            f(&r.inp[10]),
            f(&r.inp[11]),
            f(&r.inp[12]),
            f(&r.inp[13]),
            f(&r.inp[14]),
        );
        check_tt(&tt, &r.out, &format!("SETTRANS {:?}", r.inp));
    }
}

#[test]
fn set_trans_2d_matches_capture() {
    let recs = of("SETTRANS2D");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let p3 = tt_from_dump(&r.inp[0..8]);
        let mut tt = zero_tt();
        set_trans_2d(
            &mut tt,
            &p3,
            f(&r.inp[8]),
            f(&r.inp[9]),
            f(&r.inp[10]),
            f(&r.inp[11]),
        );
        check_tt(&tt, &r.out, &format!("SETTRANS2D {:?}", r.inp));
    }
}

#[test]
fn compute_clip_codes_2d_matches_capture() {
    let recs = of("CLIP2D");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let fr = frame(&r.inp[0..8]);
        let c = vec3(&r.inp[8..11]);
        let got = compute_clip_codes_2d(&fr, &c, f(&r.inp[11]), f(&r.inp[12]));
        assert_eq!(got, byte(&r.out[0]), "{:?}", r.inp);
    }
}

#[test]
fn compute_clip_codes_2d_sign_bit_cases_match_capture() {
    // Hand-picked: a term of exactly -0.0 (IsNegativeFloat true, `< 0` false),
    // +0.0, an exact cancellation, and a negative NaN.
    let recs = of("CLIP2DSPEC");
    assert_eq!(recs.len(), 4);
    for r in &recs {
        let fr = frame(&r.inp[0..8]);
        let c = vec3(&r.inp[8..11]);
        let got = compute_clip_codes_2d(&fr, &c, f(&r.inp[11]), f(&r.inp[12]));
        assert_eq!(got, byte(&r.out[0]), "{:?}", r.inp);
    }
}

#[test]
fn combine_clip_codes_matches_capture() {
    let recs = of("COMBINE");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let n: usize = i(&r.inp[0]) as usize;
        let verts: Vec<FTransTexture> = r.inp[1..=n]
            .iter()
            .map(|s| {
                let mut tt = zero_tt();
                tt.transform.flags = byte(s);
                tt
            })
            .collect();
        assert_eq!(combine_clip_codes(&verts), byte(&r.out[0]), "{:?}", r.inp);
    }
}

#[test]
fn lerp_vector_matches_capture() {
    let recs = of("LERPV");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let o = lerp_vector(f(&r.inp[0]), &vec3(&r.inp[1..4]), &vec3(&r.inp[4..7]));
        assert_eq!(o.x.to_bits(), f(&r.out[0]).to_bits(), "{:?}", r.inp);
        assert_eq!(o.y.to_bits(), f(&r.out[1]).to_bits(), "{:?}", r.inp);
        assert_eq!(o.z.to_bits(), f(&r.out[2]).to_bits(), "{:?}", r.inp);
    }
}

#[test]
fn lerp_plane_matches_capture() {
    let recs = of("LERPP");
    assert_eq!(recs.len(), 1500);
    for r in &recs {
        let o = lerp_plane(f(&r.inp[0]), &plane(&r.inp[1..5]), &plane(&r.inp[5..9]));
        assert_eq!(o.x.to_bits(), f(&r.out[0]).to_bits(), "{:?}", r.inp);
        assert_eq!(o.y.to_bits(), f(&r.out[1]).to_bits(), "{:?}", r.inp);
        assert_eq!(o.z.to_bits(), f(&r.out[2]).to_bits(), "{:?}", r.inp);
        assert_eq!(o.w.to_bits(), f(&r.out[3]).to_bits(), "{:?}", r.inp);
    }
}

#[test]
fn lerp_trans_matches_capture() {
    let recs = of("LERPT");
    assert_eq!(recs.len(), 1500);
    // Layout of each side: U, V, Point(3), Light(4).
    let make = |s: &[String]| {
        let mut tt = zero_tt();
        tt.u = f(&s[0]);
        tt.v = f(&s[1]);
        tt.transform.point = vec3(&s[2..5]);
        tt.light = plane(&s[5..9]);
        tt
    };
    for r in &recs {
        let a = make(&r.inp[1..10]);
        let b = make(&r.inp[10..19]);
        let mut out = zero_tt();
        lerp_trans(f(&r.inp[0]), &a, &b, &mut out);
        assert_eq!(out.u.to_bits(), f(&r.out[0]).to_bits(), "{:?}", r.inp);
        assert_eq!(out.v.to_bits(), f(&r.out[1]).to_bits(), "{:?}", r.inp);
        assert_eq!(
            out.transform.point.x.to_bits(),
            f(&r.out[2]).to_bits(),
            "{:?}",
            r.inp
        );
        assert_eq!(
            out.transform.point.y.to_bits(),
            f(&r.out[3]).to_bits(),
            "{:?}",
            r.inp
        );
        assert_eq!(
            out.transform.point.z.to_bits(),
            f(&r.out[4]).to_bits(),
            "{:?}",
            r.inp
        );
        assert_eq!(out.light.x.to_bits(), f(&r.out[5]).to_bits(), "{:?}", r.inp);
        assert_eq!(out.light.y.to_bits(), f(&r.out[6]).to_bits(), "{:?}", r.inp);
        assert_eq!(out.light.z.to_bits(), f(&r.out[7]).to_bits(), "{:?}", r.inp);
        assert_eq!(out.light.w.to_bits(), f(&r.out[8]).to_bits(), "{:?}", r.inp);
    }
}
