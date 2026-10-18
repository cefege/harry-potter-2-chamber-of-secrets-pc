//! Replays the captured original `UnParticleRn.cpp` `TriTubeParticles::SetupVerts`
//! and `PolyStripParticles::SetupVerts`, plus `FVector::Rotation` and the
//! `GMath` trig and reciprocal-sqrt accessors, from
//! `fixtures/particle_tube_reference.txt`. The capture was built with the
//! reference clang flags (`-ffp-contract=on` on arm64) against the real
//! `UnMath.h` and `UnRender.h` code. Every record is self-contained: the frame,
//! coords, particles, and prior strip state are recorded, so each line replays
//! on its own. Results must match bit for bit.
//!
//! Trig and `DivSqrtApprox` are not copied from the engine into this crate. The
//! fixture records the engine tables (`TRIG`, `DIVMAN`, `DIVEXP`) and the
//! `SC`/`DS` accessor results. The replay functions below index those recorded
//! tables, and `replay_tables_match_recorded_engine_values` requires them to
//! reproduce every recorded accessor value. Services the particle-tube math
//! never calls are `panic!` stubs.

#![cfg(target_arch = "aarch64")]

use std::ffi::c_void;
use std::sync::LazyLock;

use hp2_abi::render_particle::FTransTexture;
use hp2_abi::{FCoords, FVector};
use hp2_render::particle::ParticleFrame;
use hp2_render::particle_tube::{
    TubeParticle, poly_strip_setup_verts, rotation, tritube_setup_verts,
};
use hp2_render::services::Hp2RenderServices;

const FIXTURE: &str = include_str!("fixtures/particle_tube_reference.txt");

fn hex_u32(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex word {word:?}: {e}"))
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_u32(word))
}

/// Engine tables recorded from `GMath` (`TrigFLOAT`, `DivSqrtManTbl`, `DivSqrtExpTbl`).
struct Tables {
    trig: Vec<f32>,
    div_man: Vec<f32>,
    div_exp: Vec<f32>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let (mut trig, mut div_man, mut div_exp) = (None, None, None);
    for line in FIXTURE.lines() {
        let mut words = line.split_whitespace();
        let slot = match words.next() {
            Some("TRIG") => &mut trig,
            Some("DIVMAN") => &mut div_man,
            Some("DIVEXP") => &mut div_exp,
            _ => continue,
        };
        *slot = Some(words.map(hex_f32).collect::<Vec<f32>>());
    }
    let t = Tables {
        trig: trig.expect("TRIG line"),
        div_man: div_man.expect("DIVMAN line"),
        div_exp: div_exp.expect("DIVEXP line"),
    };
    assert_eq!(t.trig.len(), 16384, "TRIG table size");
    assert_eq!(t.div_man.len(), 1024, "DIVMAN table size");
    assert_eq!(t.div_exp.len(), 512, "DIVEXP table size");
    t
});

fn tables() -> &'static Tables {
    &TABLES
}

// `GMath.SinTab(i) = TrigFLOAT[(i>>2)&16383]`, `CosTab(i) = TrigFLOAT[((i+16384)>>2)&16383]`.
unsafe extern "C" fn replay_sin_tab(angle: i32) -> f32 {
    tables().trig[((angle >> 2) & 16383) as usize]
}

unsafe extern "C" fn replay_cos_tab(angle: i32) -> f32 {
    tables().trig[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

// `DivSqrtApprox(f)`: mantissa table indexed by bits 13..23, exponent table by bits 23..32.
unsafe extern "C" fn replay_div_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let t = tables();
    t.div_man[((bits >> 13) & 1023) as usize] * t.div_exp[(bits >> 23) as usize]
}

fn unused_service(name: &str) -> ! {
    panic!("{name} is not called by the particle-tube math")
}

unsafe extern "C" fn unused_frand() -> f32 {
    unused_service("frand")
}

unsafe extern "C" fn unused_mem_push(_mem: *mut c_void, _size: i32, _align: i32) -> *mut u8 {
    unused_service("mem_push")
}

unsafe extern "C" fn unused_mem_mark_save(_mem: *mut c_void, _mark: *mut c_void) {
    unused_service("mem_mark_save")
}

unsafe extern "C" fn unused_mem_mark_pop(_mark: *mut c_void) {
    unused_service("mem_mark_pop")
}

unsafe extern "C" fn unused_log_warning(_text: *const u8, _len: usize) {
    unused_service("log_warning")
}

unsafe extern "C" fn unused_sqrt_approx(_f: f32) -> f32 {
    unused_service("sqrt_approx")
}

unsafe extern "C" fn unused_div_approx(_f: f32) -> f32 {
    unused_service("div_approx")
}

unsafe extern "C" fn unused_div_sqrt_approx(_f: f32) -> f32 {
    unused_service("div_sqrt_approx (Render.h inline; the particle-tube math uses GMath)")
}

unsafe extern "C" fn unused_gmath_sqrt_approx(_f: f32) -> f32 {
    unused_service("gmath_sqrt_approx (light.rs only)")
}

fn services() -> Hp2RenderServices {
    Hp2RenderServices {
        frand: unused_frand,
        mem_push: unused_mem_push,
        mem_mark_save: unused_mem_mark_save,
        mem_mark_pop: unused_mem_mark_pop,
        log_warning: unused_log_warning,
        sin_tab: replay_sin_tab,
        cos_tab: replay_cos_tab,
        sqrt_approx: unused_sqrt_approx,
        div_sqrt_approx: unused_div_sqrt_approx,
        gmath_div_sqrt_approx: replay_div_sqrt_approx,
        div_approx: unused_div_approx,
        gmath_sqrt_approx: unused_gmath_sqrt_approx,
    }
}

/// Fixture lines with the given leading tag, numbered from 1.
fn records(tag: &'static str) -> impl Iterator<Item = (usize, &'static str)> {
    FIXTURE
        .lines()
        .enumerate()
        .filter(move |(_, l)| l.split_whitespace().next() == Some(tag))
        .map(|(i, l)| (i + 1, l))
}

/// Token cursor over one fixture record. Index 0 is the tag.
struct Record<'a> {
    words: Vec<&'a str>,
    at: usize,
}

impl<'a> Record<'a> {
    fn new(line: &'a str, tag: &str) -> Self {
        let words: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(words[0], tag, "record tag");
        Record { words, at: 1 }
    }

    fn word(&mut self) -> &'a str {
        let w = self.words[self.at];
        self.at += 1;
        w
    }

    fn expect(&mut self, want: &str) {
        assert_eq!(self.word(), want, "record separator");
    }

    fn f32(&mut self) -> f32 {
        hex_f32(self.word())
    }

    fn u32(&mut self) -> u32 {
        hex_u32(self.word())
    }

    fn i32(&mut self) -> i32 {
        self.u32() as i32
    }

    fn vec(&mut self) -> FVector {
        let x = self.f32();
        let y = self.f32();
        let z = self.f32();
        FVector { x, y, z }
    }

    fn coords(&mut self) -> FCoords {
        let origin = self.vec();
        let x_axis = self.vec();
        let y_axis = self.vec();
        let z_axis = self.vec();
        FCoords {
            origin,
            x_axis,
            y_axis,
            z_axis,
        }
    }

    fn frame(&mut self) -> ParticleFrame {
        let proj_z = self.f32();
        let rproj_z = self.f32();
        let fx15 = self.f32();
        let fy15 = self.f32();
        let prjxm = self.f32();
        let prjxp = self.f32();
        let prjym = self.f32();
        let prjyp = self.f32();
        ParticleFrame {
            proj_z,
            rproj_z,
            fx15,
            fy15,
            prjxm,
            prjxp,
            prjym,
            prjyp,
        }
    }

    fn particle(&mut self) -> TubeParticle {
        let x = self.f32();
        let y = self.f32();
        let z = self.f32();
        let width = self.f32();
        let length = self.f32();
        TubeParticle {
            x,
            y,
            z,
            width,
            length,
        }
    }
}

fn assert_bits(ctx: &str, what: &str, got: f32, want: f32) {
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "{ctx} {what}: got {got:?} want {want:?}"
    );
}

fn assert_vec_bits(ctx: &str, what: &str, got: FVector, want: FVector) {
    assert_bits(ctx, &format!("{what}.x"), got.x, want.x);
    assert_bits(ctx, &format!("{what}.y"), got.y, want.y);
    assert_bits(ctx, &format!("{what}.z"), got.z, want.z);
}

/// Reads one expected vertex (10 tokens) and compares every output field.
fn check_vert(ctx: &str, k: usize, got: &FTransTexture, r: &mut Record) {
    let ctx = format!("{ctx} vert {k}");
    let want_point = r.vec();
    let want_flags = r.u32();
    let want_screen_x = r.f32();
    let want_screen_y = r.f32();
    let want_int_y = r.i32();
    let want_rz = r.f32();
    let want_u = r.f32();
    let want_v = r.f32();
    let t = &got.transform;
    assert_vec_bits(&ctx, "point", t.point, want_point);
    assert_eq!(u32::from(t.flags), want_flags, "{ctx} flags");
    assert_bits(&ctx, "screen_x", t.screen_x, want_screen_x);
    assert_bits(&ctx, "screen_y", t.screen_y, want_screen_y);
    assert_eq!(t.int_y, want_int_y, "{ctx} int_y");
    assert_bits(&ctx, "rz", t.rz, want_rz);
    assert_bits(&ctx, "u", got.u, want_u);
    assert_bits(&ctx, "v", got.v, want_v);
}

fn zeroed_verts<const N: usize>() -> [FTransTexture; N] {
    // `FTransTexture` is `#[repr(C)]` floats, one `u8`, and one `i32`, so all-zero is a valid value.
    std::array::from_fn(|_| unsafe { std::mem::zeroed() })
}

#[test]
fn replay_tables_match_recorded_engine_values() {
    let mut sc = 0;
    for (ln, line) in records("SC") {
        let mut r = Record::new(line, "SC");
        let angle = r.i32();
        let want_sin = r.f32();
        let want_cos = r.f32();
        let ctx = format!("line {ln} SC {angle}");
        assert_bits(&ctx, "sin", unsafe { replay_sin_tab(angle) }, want_sin);
        assert_bits(&ctx, "cos", unsafe { replay_cos_tab(angle) }, want_cos);
        sc += 1;
    }
    let mut ds = 0;
    for (ln, line) in records("DS") {
        let mut r = Record::new(line, "DS");
        let x = r.f32();
        let want = r.f32();
        assert_bits(
            &format!("line {ln} DS"),
            "div_sqrt_approx",
            unsafe { replay_div_sqrt_approx(x) },
            want,
        );
        ds += 1;
    }
    assert!(
        sc > 0 && ds > 0,
        "fixture has SC ({sc}) and DS ({ds}) records"
    );
}

#[test]
fn rotation_matches_recorded_engine_rotator() {
    let mut n = 0;
    for (ln, line) in records("RT") {
        let mut r = Record::new(line, "RT");
        let v = r.vec();
        let want = (r.i32(), r.i32(), r.i32());
        assert_eq!(rotation(v), want, "line {ln} Rotation({v:?})");
        n += 1;
    }
    assert!(n > 0, "fixture has RT records");
}

#[test]
fn tritube_setup_verts_is_bit_exact_against_capture() {
    let svc = services();
    let mut n = 0;
    for (ln, line) in records("TT") {
        let ctx = format!("line {ln} TT");
        let mut r = Record::new(line, "TT");
        let frame = r.frame();
        let coords = r.coords();
        let tex_usize = r.i32();
        let tex_vsize = r.i32();
        let has_prev = r.u32() != 0;
        let prev = r.particle();
        let curr = r.particle();
        let has_next = r.u32() != 0;
        let next = r.particle();
        let mut psv = r.vec();
        r.expect("|");
        let ret = r.u32();
        let want_psv = r.vec();
        let mut verts: [FTransTexture; 12] = zeroed_verts();

        let got = tritube_setup_verts(
            &svc,
            &frame,
            &coords,
            tex_usize,
            tex_vsize,
            has_prev.then_some(&prev),
            &curr,
            has_next.then_some(&next),
            &mut psv,
            &mut verts,
        );
        assert_eq!(u32::from(got), ret, "{ctx} return");
        assert_vec_bits(&ctx, "prev_strip_vect", psv, want_psv);
        for (k, vert) in verts.iter().enumerate() {
            check_vert(&ctx, k, vert, &mut r);
        }
        n += 1;
    }
    assert!(n > 0, "fixture has TT records");
}

#[test]
fn poly_strip_setup_verts_is_bit_exact_against_capture() {
    let svc = services();
    let mut n = 0;
    for (ln, line) in records("PS") {
        let ctx = format!("line {ln} PS");
        let mut r = Record::new(line, "PS");
        let frame = r.frame();
        let coords = r.coords();
        let tex_usize = r.i32();
        let tex_vsize = r.i32();
        let has_prev = r.u32() != 0;
        let prev = r.particle();
        let curr = r.particle();
        let has_next = r.u32() != 0;
        let next = r.particle();
        let mut psv = r.vec();
        let mut pev = r.vec();
        r.expect("|");
        let ret = r.u32();
        let want_psv = r.vec();
        let want_pev = r.vec();
        let mut verts: [FTransTexture; 4] = zeroed_verts();

        let got = poly_strip_setup_verts(
            &svc,
            &frame,
            &coords,
            tex_usize,
            tex_vsize,
            has_prev.then_some(&prev),
            &curr,
            has_next.then_some(&next),
            &mut psv,
            &mut pev,
            &mut verts,
        );
        assert_eq!(u32::from(got), ret, "{ctx} return");
        assert_vec_bits(&ctx, "prev_strip_vect", psv, want_psv);
        assert_vec_bits(&ctx, "prev_edge_vect", pev, want_pev);
        for (k, vert) in verts.iter().enumerate() {
            check_vert(&ctx, k, vert, &mut r);
        }
        n += 1;
    }
    assert!(n > 0, "fixture has PS records");
}
