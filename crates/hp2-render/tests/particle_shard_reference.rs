//! Replays the captured original `UnParticleRn.cpp` `ShardParticles::SetupVerts`,
//! `LiquidParticles::SetupVerts`, and `ScreenVector` (reference clang flags,
//! default `-ffp-contract=on`, real `UnMath.h` engine code) and requires
//! bit-identical vertices, centers, flags, and returned clip codes.
//!
//! Trig and `DivSqrtApprox` come from the fixture's recorded `T`/`D` lines:
//! the engine tables are not copied, and a lookup for a value the capture did
//! not record panics.

#![cfg(target_arch = "aarch64")]

use std::collections::HashMap;

use hp2_abi::render_particle::{FTransTexture, FTransform};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_render::particle::ParticleFrame;
use hp2_render::particle_shard::{
    ParticleState, RenderMath, liquid_setup_verts, screen_vector, shard_setup_verts,
};

const FIXTURE: &str = include_str!("fixtures/particle_shard_reference.txt");

fn hex_bits(word: &str) -> u32 {
    u32::from_str_radix(word, 16).expect("hex word")
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_bits(word))
}

fn hex_i32(word: &str) -> i32 {
    hex_bits(word) as i32
}

/// Recorded engine tables and `DivSqrtApprox` results, keyed by the exact input.
struct FixtureMath {
    trig: HashMap<i32, (f32, f32)>,
    dsa: HashMap<u32, f32>,
}

impl RenderMath for FixtureMath {
    fn sin_tab(&self, angle: i32) -> f32 {
        self.trig
            .get(&angle)
            .unwrap_or_else(|| panic!("unrecorded SinTab({angle})"))
            .0
    }

    fn cos_tab(&self, angle: i32) -> f32 {
        self.trig
            .get(&angle)
            .unwrap_or_else(|| panic!("unrecorded CosTab({angle})"))
            .1
    }

    fn gmath_div_sqrt_approx(&self, f: f32) -> f32 {
        *self
            .dsa
            .get(&f.to_bits())
            .unwrap_or_else(|| panic!("unrecorded DivSqrtApprox({f:e}) bits {:08x}", f.to_bits()))
    }
}

fn load_math() -> FixtureMath {
    let mut trig = HashMap::new();
    let mut dsa = HashMap::new();
    for line in FIXTURE.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first() {
            Some(&"T") => {
                trig.insert(hex_i32(w[1]), (hex_f32(w[2]), hex_f32(w[3])));
            }
            Some(&"D") => {
                dsa.insert(hex_bits(w[1]), hex_f32(w[2]));
            }
            _ => {}
        }
    }
    FixtureMath { trig, dsa }
}

/// Token cursor over one fixture record, skipping the `|` separators.
struct Cursor<'a> {
    words: Vec<&'a str>,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            words: line.split_whitespace().filter(|w| *w != "|").collect(),
            at: 0,
        }
    }

    fn next(&mut self) -> &'a str {
        let w = self.words[self.at];
        self.at += 1;
        w
    }

    fn f(&mut self) -> f32 {
        hex_f32(self.next())
    }

    fn i(&mut self) -> i32 {
        hex_i32(self.next())
    }

    fn vec(&mut self) -> FVector {
        FVector {
            x: self.f(),
            y: self.f(),
            z: self.f(),
        }
    }

    fn frame(&mut self) -> ParticleFrame {
        ParticleFrame {
            proj_z: self.f(),
            rproj_z: self.f(),
            fx15: self.f(),
            fy15: self.f(),
            prjxm: self.f(),
            prjxp: self.f(),
            prjym: self.f(),
            prjyp: self.f(),
        }
    }

    fn coords(&mut self) -> FCoords {
        FCoords {
            origin: self.vec(),
            x_axis: self.vec(),
            y_axis: self.vec(),
            z_axis: self.vec(),
        }
    }

    fn transform(&mut self) -> FTransform {
        FTransform {
            point: self.vec(),
            flags: hex_bits(self.next()) as u8,
            screen_x: self.f(),
            screen_y: self.f(),
            int_y: self.i(),
            rz: self.f(),
        }
    }

    fn tex(&mut self) -> FTransTexture {
        FTransTexture {
            transform: self.transform(),
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
            u: self.f(),
            v: self.f(),
        }
    }
}

fn zero_tex() -> FTransTexture {
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

fn assert_vec_bits(got: FVector, want: FVector, ctx: &str) {
    assert_eq!(got.x.to_bits(), want.x.to_bits(), "{ctx} x");
    assert_eq!(got.y.to_bits(), want.y.to_bits(), "{ctx} y");
    assert_eq!(got.z.to_bits(), want.z.to_bits(), "{ctx} z");
}

fn assert_tex_bits(got: &FTransTexture, want: &FTransTexture, ctx: &str) {
    assert_vec_bits(
        got.transform.point,
        want.transform.point,
        &format!("{ctx} point"),
    );
    assert_eq!(got.transform.flags, want.transform.flags, "{ctx} flags");
    assert_eq!(
        got.transform.screen_x.to_bits(),
        want.transform.screen_x.to_bits(),
        "{ctx} screen_x"
    );
    assert_eq!(
        got.transform.screen_y.to_bits(),
        want.transform.screen_y.to_bits(),
        "{ctx} screen_y"
    );
    assert_eq!(got.transform.int_y, want.transform.int_y, "{ctx} int_y");
    assert_eq!(
        got.transform.rz.to_bits(),
        want.transform.rz.to_bits(),
        "{ctx} rz"
    );
    assert_eq!(got.u.to_bits(), want.u.to_bits(), "{ctx} u");
    assert_eq!(got.v.to_bits(), want.v.to_bits(), "{ctx} v");
}

fn assert_transform_bits(got: &FTransform, want: &FTransform, ctx: &str) {
    assert_vec_bits(got.point, want.point, &format!("{ctx} point"));
    assert_eq!(got.flags, want.flags, "{ctx} flags");
    assert_eq!(
        got.screen_x.to_bits(),
        want.screen_x.to_bits(),
        "{ctx} screen_x"
    );
    assert_eq!(
        got.screen_y.to_bits(),
        want.screen_y.to_bits(),
        "{ctx} screen_y"
    );
    assert_eq!(got.int_y, want.int_y, "{ctx} int_y");
    assert_eq!(got.rz.to_bits(), want.rz.to_bits(), "{ctx} rz");
}

#[test]
fn shard_setup_verts_matches_original_cpp_capture_bit_for_bit() {
    let math = load_math();
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("SHARD ") {
            continue;
        }
        let mut c = Cursor::new(line);
        c.next();
        let tex_usize = c.i();
        let tex_vsize = c.i();
        let frame = c.frame();
        let coords = c.coords();
        let position = c.vec();
        let spin = c.f();
        let length = c.f();
        let width = c.f();
        let mut verts = [zero_tex(), zero_tex(), zero_tex()];
        let code = shard_setup_verts(
            &math, &frame, &coords, tex_usize, tex_vsize, position, spin, length, width, &mut verts,
        );
        let want: Vec<FTransTexture> = (0..3).map(|_| c.tex()).collect();
        let want_code = hex_bits(c.next()) as u8;
        for k in 0..3 {
            assert_tex_bits(&verts[k], &want[k], &format!("line {n} vert {k}"));
        }
        assert_eq!(code, want_code, "line {n} clip code");
        checked += 1;
    }
    assert_eq!(checked, 600);
}

#[test]
fn liquid_setup_verts_matches_original_cpp_capture_bit_for_bit() {
    let math = load_math();
    let mut checked = 0usize;
    let mut drip_checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("LIQ ") {
            continue;
        }
        let mut c = Cursor::new(line);
        c.next();
        let tex_usize = c.i();
        let tex_vsize = c.i();
        let frame = c.frame();
        let coords = c.coords();
        let mut center = c.transform();
        let position = c.vec();
        let velocity = c.vec();
        let state = ParticleState {
            position,
            velocity,
            length: c.f(),
            width: c.f(),
            drip_timer: c.f(),
        };
        if state.drip_timer > 0.0 {
            drip_checked += 1;
        }
        let mut verts = [zero_tex(), zero_tex(), zero_tex(), zero_tex()];
        let code = liquid_setup_verts(
            &math,
            &frame,
            &coords,
            tex_usize,
            tex_vsize,
            &mut center,
            &state,
            &mut verts,
        );
        let want_center = c.transform();
        let want: Vec<FTransTexture> = (0..4).map(|_| c.tex()).collect();
        let want_code = hex_bits(c.next()) as u8;
        assert_transform_bits(&center, &want_center, &format!("line {n} center"));
        for k in 0..4 {
            assert_tex_bits(&verts[k], &want[k], &format!("line {n} vert {k}"));
        }
        assert_eq!(code, want_code, "line {n} clip code");
        checked += 1;
    }
    assert_eq!(checked, 600);
    assert!(
        drip_checked > 0 && drip_checked < checked,
        "both drip and streak branches covered"
    );
}

#[test]
fn screen_vector_matches_original_cpp_capture_bit_for_bit() {
    let math = load_math();
    let mut checked = 0usize;
    for (n, line) in FIXTURE.lines().enumerate() {
        if !line.starts_with("SV ") {
            continue;
        }
        let mut c = Cursor::new(line);
        c.next();
        let tex_usize = c.i();
        let tex_vsize = c.i();
        let frame = c.frame();
        let coords = c.coords();
        let state = ParticleState {
            position: c.vec(),
            velocity: c.vec(),
            length: c.f(),
            width: c.f(),
            drip_timer: c.f(),
        };
        let mut pts = [zero_tex(), zero_tex(), zero_tex(), zero_tex()];
        screen_vector(
            &math, &coords, &frame, tex_usize, tex_vsize, &state, &mut pts,
        );
        let want: Vec<FTransTexture> = (0..4).map(|_| c.tex()).collect();
        for k in 0..4 {
            assert_tex_bits(&pts[k], &want[k], &format!("line {n} pt {k}"));
        }
        checked += 1;
    }
    assert_eq!(checked, 600);
}
