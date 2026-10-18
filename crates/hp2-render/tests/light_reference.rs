//! Replays the captured original `UnLight.cpp` kernels from
//! `fixtures/light_reference.txt`: the `FLightManager::Init` tables, the
//! `GMath` table accessors, `ShadowMapGen`, `Merge`, `Volumetric`, and every
//! `spatial_*` entry. The capture (`/tmp/cap/light/capture_light.cpp`) was
//! built with the reference clang flags (`-ffp-contract=on` on arm64) against
//! the real `UnLight.cpp` and `UnMath.h`. Results must match bit for bit.
//!
//! Each record carries its inputs, so records replay on their own. The only
//! inputs regenerated here are the `MERGE` palette, illumination map, and
//! stream (from the printed `seed`) and the `SPATIAL` source bytes (from its
//! printed `seed`), mirroring the harness generators. `VOLUMETRIC` records
//! replay in file order because `FogRejectionMethod` is a function-static.
//!
//! The `GMath` trig table is rebuilt from the engine formula and checked
//! against the recorded FNV hash. The square-root and reciprocal-square-root
//! tables are recorded from the engine and checked through the `GMATH_SQRT`
//! records. Services the light kernels never call return NaN or do nothing.

#![cfg(target_arch = "aarch64")]

use std::ffi::c_void;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::LazyLock;

use hp2_abi::FVector;
use hp2_render::light::{
    BYTE_FOG_LEN, GMath, LIGHT_DIST_LEN, MergeParams, PI, Spatial, SpatialParams, VolumetricParams,
    init_byte_fog, init_filter_tab, init_light_dist, merge, shadow_map_gen, spatial, volumetric,
};
use hp2_render::services::Hp2RenderServices;

const FIXTURE: &str = include_str!("fixtures/light_reference.txt");

// ---------------------------------------------------------------------------
// Fixture parsing
// ---------------------------------------------------------------------------

/// Every fixture line whose first token is `tag`.
fn records(tag: &'static str) -> impl Iterator<Item = &'static str> {
    FIXTURE
        .lines()
        .filter(move |l| l.split_whitespace().next() == Some(tag))
}

fn record(tag: &'static str) -> &'static str {
    records(tag)
        .next()
        .unwrap_or_else(|| panic!("fixture has no {tag} line"))
}

fn hex_u32(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex word {word:?}: {e}"))
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_u32(word))
}

/// A run of `%02x` bytes with no separators.
fn hex_bytes(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "odd hex byte run");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// A run of `%08x` words with no separators (`wdw`).
fn hex_words_packed(s: &str) -> Vec<u32> {
    assert!(s.len().is_multiple_of(8), "packed word run");
    (0..s.len())
        .step_by(8)
        .map(|i| hex_u32(&s[i..i + 8]))
        .collect()
}

/// The second token of a single-value record (`TAG <hex>`).
fn payload(tag: &'static str) -> &'static str {
    record(tag)
        .split_whitespace()
        .nth(1)
        .expect("payload token")
}

/// The `%08x` words that follow the tag on its record (`wf`/`wu`).
fn tagged_words(tag: &'static str) -> Vec<u32> {
    record(tag)
        .split_whitespace()
        .skip(1)
        .map(hex_u32)
        .collect()
}

fn tagged_floats(tag: &'static str) -> Vec<f32> {
    tagged_words(tag).into_iter().map(f32::from_bits).collect()
}

fn fixture_u64(tag: &'static str) -> u64 {
    u64::from_str_radix(payload(tag), 16).expect("FNV hash")
}

/// Sequential reader over one fixture record.
struct Toks(std::str::SplitWhitespace<'static>);

impl Toks {
    fn new(line: &'static str) -> Self {
        Self(line.split_whitespace())
    }

    fn lit(&mut self, want: &str) {
        assert_eq!(self.0.next(), Some(want), "fixture literal");
    }

    fn word(&mut self) -> &'static str {
        self.0.next().expect("fixture token")
    }

    fn dec<T: FromStr>(&mut self) -> T
    where
        T::Err: Debug,
    {
        let w = self.word();
        w.parse().unwrap_or_else(|e| panic!("decimal {w:?}: {e:?}"))
    }

    fn hex(&mut self) -> u32 {
        hex_u32(self.word())
    }

    fn float(&mut self) -> f32 {
        hex_f32(self.word())
    }

    fn vec3(&mut self) -> FVector {
        let x = self.float();
        let y = self.float();
        let z = self.float();
        FVector { x, y, z }
    }
}

// ---------------------------------------------------------------------------
// Engine stand-ins (`Hp2RenderServices`)
// ---------------------------------------------------------------------------

struct Tables {
    /// `GMath.TrigFLOAT[16384]`, `TrigFLOAT[i] = appSin(i*2.0f*PI/16384)`.
    trig: Vec<f32>,
    /// `GMath.SqrtManTbl[2048]`, recorded from the engine.
    sqrt_man: Vec<f32>,
    /// `GMath.DivSqrtManTbl[1024]` and `GMath.DivSqrtExpTbl[512]`, recorded.
    div_sqrt_man: Vec<f32>,
    div_sqrt_exp: Vec<f32>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let trig: Vec<f32> = (0..16384i32)
        .map(|i| {
            let arg = (i as f32) * 2.0f32 * PI / 16384.0f32;
            (arg as f64).sin() as f32
        })
        .collect();
    let t = Tables {
        trig,
        sqrt_man: tagged_floats("GMATH_SQRTMAN"),
        div_sqrt_man: tagged_floats("GMATH_DIVSQRTMAN"),
        div_sqrt_exp: tagged_floats("GMATH_DIVSQRTEXP"),
    };
    assert_eq!(t.trig.len(), 16384, "TRIG size");
    assert_eq!(t.sqrt_man.len(), 2048, "SQRTMAN size");
    assert_eq!(t.div_sqrt_man.len(), 1024, "DIVSQRTMAN size");
    assert_eq!(t.div_sqrt_exp.len(), 512, "DIVSQRTEXP size");
    t
});

fn tables() -> &'static Tables {
    &TABLES
}

// `GMath.SinTab(i) = TrigFLOAT[(i>>2)&16383]`.
unsafe extern "C" fn sin_tab(angle: i32) -> f32 {
    tables().trig[((angle >> 2) & 16383) as usize]
}

// `GMath.CosTab(i) = TrigFLOAT[((i+16384)>>2)&16383]`.
unsafe extern "C" fn cos_tab(angle: i32) -> f32 {
    tables().trig[((angle.wrapping_add(16384) >> 2) & 16383) as usize]
}

// `GMath.SqrtApprox(F)` (UnMath.h, non-asm branch): table index from bits
// 13..23, exponent bits 24..30 halved and added to the table bits.
unsafe extern "C" fn gmath_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let a = ((bits >> 13) & 2047) as usize;
    let b = bits & 0x7F00_0000;
    f32::from_bits(tables().sqrt_man[a].to_bits().wrapping_add(b >> 1))
}

// `GMath.DivSqrtApprox(F)` (UnMath.h `Power`): mantissa bits 13..22 and
// exponent bits 23..31 index the recorded tables.
unsafe extern "C" fn gmath_div_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let t = tables();
    t.div_sqrt_man[((bits >> 13) & 1023) as usize] * t.div_sqrt_exp[(bits >> 23) as usize]
}

// Services the light kernels never call. They return NaN (or do nothing), so
// any call that did happen would make the bit comparison fail.
unsafe extern "C" fn unused_frand() -> f32 {
    f32::NAN
}

unsafe extern "C" fn unused_unary(_f: f32) -> f32 {
    f32::NAN
}

unsafe extern "C" fn unused_mem_push(_mem: *mut c_void, _size: i32, _align: i32) -> *mut u8 {
    std::ptr::null_mut()
}

unsafe extern "C" fn unused_mem_mark_save(_mem: *mut c_void, _mark: *mut c_void) {}

unsafe extern "C" fn unused_mem_mark_pop(_mark: *mut c_void) {}

unsafe extern "C" fn unused_log_warning(_text: *const u8, _len: usize) {}

fn services() -> Hp2RenderServices {
    Hp2RenderServices {
        frand: unused_frand,
        mem_push: unused_mem_push,
        mem_mark_save: unused_mem_mark_save,
        mem_mark_pop: unused_mem_mark_pop,
        log_warning: unused_log_warning,
        sin_tab,
        cos_tab,
        sqrt_approx: unused_unary,
        div_sqrt_approx: unused_unary,
        div_approx: unused_unary,
        gmath_div_sqrt_approx,
        gmath_sqrt_approx,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// FNV-1a over the low-to-high bytes of each word (the capture's `fnv_words`).
fn fnv_words(words: impl Iterator<Item = u32>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in 0..4 {
            h ^= u64::from((w >> (8 * b)) & 0xff);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The harness `lcg()`.
fn lcg(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

/// Little-endian byte image of DWORDs, as `wb` prints a DWORD buffer.
fn bytes_of(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn random_bases() -> [f32; 256] {
    tagged_floats("RANDOMBASES")
        .try_into()
        .expect("RANDOMBASES holds 256 values")
}

fn randoms() -> [f32; 256] {
    tagged_floats("RANDOMS")
        .try_into()
        .expect("RANDOMS holds 256 values")
}

fn spatial_kind(kind: u32) -> Spatial {
    match kind {
        // Kind 0 runs `spatial_None` with RadiusInner > 0; kind 1 has RadiusInner 0.
        0 | 1 => Spatial::None,
        2 => Spatial::SearchLight,
        3 => Spatial::SlowWave,
        4 => Spatial::FastWave,
        5 => Spatial::Shock,
        6 => Spatial::Disco,
        7 => Spatial::Interference,
        8 => Spatial::Cylinder,
        9 => Spatial::Rotor,
        10 => Spatial::Spotlight,
        11 => Spatial::NonIncidence,
        12 => Spatial::Shell,
        _ => panic!("spatial kind {kind}"),
    }
}

// ---------------------------------------------------------------------------
// GMath tables
// ---------------------------------------------------------------------------

#[test]
fn gmath_trig_table_matches_engine_fnv() {
    let trig = &tables().trig;
    assert_eq!(
        fnv_words(trig.iter().map(|v| v.to_bits())),
        fixture_u64("GMATH_TRIG_FNV"),
        "TrigFLOAT"
    );
}

#[test]
fn gmath_sqrt_approx_matches_capture() {
    let svc = services();
    let g = GMath::new(&svc);
    let mut n = 0;
    for line in records("GMATH_SQRT") {
        // GMATH_SQRT x GMath.SqrtApprox(x) GMath.DivSqrtApprox(x)
        let mut t = Toks::new(line);
        t.lit("GMATH_SQRT");
        let x = t.float();
        let sqrt = t.hex();
        let div_sqrt = t.hex();
        assert_eq!(g.sqrt_approx(x).to_bits(), sqrt, "GMath.SqrtApprox({x:e})");
        assert_eq!(
            g.div_sqrt_approx(x).to_bits(),
            div_sqrt,
            "GMath.DivSqrtApprox({x:e})"
        );
        n += 1;
    }
    assert_eq!(n, 512);
}

// ---------------------------------------------------------------------------
// FLightManager::Init tables
// ---------------------------------------------------------------------------

#[test]
fn light_dist_tables_match_capture() {
    let mut dist = [0f32; LIGHT_DIST_LEN];
    let mut inc = [0f32; LIGHT_DIST_LEN];
    init_light_dist(&mut dist, &mut inc);
    assert_eq!(
        fnv_words(dist.iter().map(|v| v.to_bits())),
        fixture_u64("LIGHTDIST_FNV"),
        "LightDist"
    );
    assert_eq!(
        fnv_words(inc.iter().map(|v| v.to_bits())),
        fixture_u64("LIGHTDISTINC_FNV"),
        "LightDistInc"
    );
}

#[test]
fn filter_tab_matches_capture() {
    let tab = init_filter_tab();
    let got: Vec<u32> = tab.iter().flatten().copied().collect();
    assert_eq!(got, tagged_words("FILTERTAB"), "FilterTab[128][4]");
}

#[test]
fn byte_fog_matches_capture() {
    let mut fog = [0u8; BYTE_FOG_LEN];
    init_byte_fog(&mut fog);
    let want = hex_bytes(payload("BYTEFOG"));
    assert_eq!(want.len(), BYTE_FOG_LEN, "BYTEFOG size");
    assert_eq!(&fog[..], want.as_slice(), "ByteFog[128*128]");
}

// ---------------------------------------------------------------------------
// ShadowMapGen
// ---------------------------------------------------------------------------

#[test]
fn shadow_map_gen_matches_capture() {
    let tab = init_filter_tab();
    let mut n = 0;
    for line in records("SHADOW") {
        // SHADOW U V <src bytes> | <dst bytes>
        let mut t = Toks::new(line);
        t.lit("SHADOW");
        let u: usize = t.dec();
        let v: usize = t.dec();
        let src = hex_bytes(t.word());
        t.lit("|");
        let want = hex_bytes(t.word());
        assert_eq!(src.len(), u * v, "SHADOW src length");
        assert_eq!(want.len(), u * v * 8, "SHADOW dst length");
        let mut dest = vec![0xA5A5_A5A5u32; u * v * 2];
        shadow_map_gen(&tab, u, v, Some(&src), &mut dest);
        assert_eq!(bytes_of(&dest), want, "SHADOW {u} {v}");
        n += 1;
    }
    assert_eq!(n, 8);
}

#[test]
fn shadow_map_gen_null_source_matches_capture() {
    let tab = init_filter_tab();
    let mut n = 0;
    for line in records("SHADOWNULL") {
        // SHADOWNULL U V | <dst bytes>
        let mut t = Toks::new(line);
        t.lit("SHADOWNULL");
        let u: usize = t.dec();
        let v: usize = t.dec();
        t.lit("|");
        let want = hex_bytes(t.word());
        assert_eq!(want.len(), u * v * 8, "SHADOWNULL dst length");
        let mut dest = vec![0xA5A5_A5A5u32; u * v * 2];
        shadow_map_gen(&tab, u, v, None, &mut dest);
        assert_eq!(bytes_of(&dest), want, "SHADOWNULL {u} {v}");
        n += 1;
    }
    assert_eq!(n, 8);
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

#[test]
fn merge_matches_capture() {
    let rb = random_bases();
    let r = randoms();
    let mut n = 0;
    for line in records("MERGE") {
        // MERGE effect key seed minU maxU minV maxV | <160 DWORDs>
        let mut t = Toks::new(line);
        t.lit("MERGE");
        let effect: u8 = t.dec();
        let key: i32 = t.dec();
        let seed: u32 = t.dec();
        let min_u: i32 = t.dec();
        let max_u: i32 = t.dec();
        let min_v: i32 = t.dec();
        let max_v: i32 = t.dec();
        t.lit("|");
        let want = hex_words_packed(t.word());
        assert_eq!(want.len(), 20 * 8, "MERGE dest length");

        // Harness generator: palette, illumination map, and stream from `seed`.
        let mut gs = seed;
        let mut pal = [0u32; 256];
        for p in pal.iter_mut() {
            *p = lcg(&mut gs);
        }
        let mut src = [0u8; 16 * 8];
        for s in src.iter_mut() {
            *s = (lcg(&mut gs) >> 24) as u8;
        }
        let mut stream = [0u32; 20 * 8];
        for s in stream.iter_mut() {
            *s = lcg(&mut gs);
        }

        let p = MergeParams {
            effect,
            key,
            uclamp: 16,
            usize_: 20,
            min_u,
            max_u,
            min_v,
            max_v,
        };
        let mut dest = [0xA5A5_A5A5u32; 20 * 8];
        merge(&p, &src, &pal, Some(&stream), &mut dest, &rb, &r);
        assert_eq!(
            dest.as_slice(),
            want.as_slice(),
            "MERGE effect {effect} seed {seed}"
        );
        // In-place form (`Merge(..., Stream, Stream)`): the stream is `dest`.
        // Words outside the merged window keep their pre-merge value, so the
        // reference is the out-of-place merge with `dest` seeded from the stream.
        let mut inplace = stream;
        merge(&p, &src, &pal, None, &mut inplace, &rb, &r);
        let mut seeded = stream;
        merge(&p, &src, &pal, Some(&stream), &mut seeded, &rb, &r);
        assert_eq!(
            inplace.as_slice(),
            seeded.as_slice(),
            "MERGE in-place effect {effect} seed {seed}"
        );
        n += 1;
    }
    assert_eq!(n, 42);
}

// ---------------------------------------------------------------------------
// Volumetric
// ---------------------------------------------------------------------------

#[test]
fn volumetric_matches_capture() {
    let svc = services();
    let g = GMath::new(&svc);
    // `FogRejectionMethod` is a function-static in the engine, so the records
    // replay in file order from the initial value 0.
    let mut fog_rejection = 0i32;
    let mut n = 0;
    for line in records("VOLUMETRIC") {
        // VOLUMETRIC ambient inside loc(3) vert(3) VRS VB LSS RVR RVRS | out
        let mut t = Toks::new(line);
        t.lit("VOLUMETRIC");
        let ambient = t.dec::<u32>() == 1;
        let vol_inside = t.dec::<u32>() == 1;
        let location = t.vec3();
        let vertex = t.vec3();
        let vol_radius_squared = t.float();
        let vol_brightness = t.float();
        let location_size_squared = t.float();
        let rvol_radius = t.float();
        let rvol_radius_squared = t.float();
        t.lit("|");
        let want = t.float();

        let info = VolumetricParams {
            location,
            vol_radius_squared,
            rvol_radius,
            rvol_radius_squared,
            vol_brightness,
            location_size_squared,
            vol_inside,
            ambient,
        };
        let got = volumetric(&g, &info, vertex, &mut fog_rejection);
        assert_eq!(got.to_bits(), want.to_bits(), "VOLUMETRIC record {}", n + 1);
        n += 1;
    }
    assert_eq!(n, 700);
}

// ---------------------------------------------------------------------------
// Spatial kernels
// ---------------------------------------------------------------------------

#[test]
fn spatial_kernels_match_capture() {
    let svc = services();
    let g = GMath::new(&svc);
    let mut dist = [0f32; LIGHT_DIST_LEN];
    let mut inc = [0f32; LIGHT_DIST_LEN];
    init_light_dist(&mut dist, &mut inc);

    let mut n = 0;
    for line in records("SPATIAL") {
        // SPATIAL kind uclamp smu sskip minU maxU minV maxV radius radInner
        //   rradius diffuse sel timeSeconds period phase cone
        //   loc(3) view(3) vb(3) vdu(3) vdv(3) seed N | <128 dest bytes>
        let mut t = Toks::new(line);
        t.lit("SPATIAL");
        let kind: u32 = t.dec();
        let uclamp: i32 = t.dec();
        let shadow_mask_u: i32 = t.dec();
        let shadow_skip: i32 = t.dec();
        let min_u: i32 = t.dec();
        let max_u: i32 = t.dec();
        let min_v: i32 = t.dec();
        let max_v: i32 = t.dec();
        let radius = t.float();
        let radius_inner = t.float();
        let rradius = t.float();
        let diffuse = t.float();
        let sel: u32 = t.dec();
        let time_seconds = t.float();
        let light_period: u8 = t.dec();
        let light_phase: u8 = t.dec();
        let light_cone: u8 = t.dec();
        let actor_location = t.vec3();
        let view = t.vec3();
        let vertex_base = t.vec3();
        let vertex_du = t.vec3();
        let vertex_dv = t.vec3();
        t.lit("seed");
        let seed: u32 = t.dec();
        t.lit("|");
        let want = hex_bytes(t.word());
        assert_eq!(want.len(), 128, "SPATIAL dest length");

        // Harness generator: 512 source bytes from `seed`.
        let mut gs = seed.wrapping_mul(2_654_435_761).wrapping_add(7);
        let mut src = [0u8; 512];
        for s in src.iter_mut() {
            let r = lcg(&mut gs);
            *s = if (r >> 16) & 7 != 0 {
                (r >> 24) as u8
            } else {
                0
            };
        }

        let p = SpatialParams {
            uclamp,
            shadow_mask_u,
            shadow_skip,
            min_u,
            max_u,
            min_v,
            max_v,
            radius,
            rradius,
            radius_inner,
            diffuse,
            actor_location,
            view,
            light_period,
            light_phase,
            light_cone,
            time_seconds,
            vertex_base,
            vertex_du,
            vertex_dv,
        };
        // `Info->LightDistTab = sel ? LightDistInc : LightDist`; `LightDist` is always the global.
        let light_dist_tab = if sel == 1 { &inc } else { &dist };
        let mut dest = [0xA5u8; 128];
        spatial(
            &g,
            spatial_kind(kind),
            &p,
            light_dist_tab,
            &dist,
            &src,
            &mut dest,
        );
        assert_eq!(
            dest.as_slice(),
            want.as_slice(),
            "SPATIAL kind {kind} seed {seed}"
        );
        n += 1;
    }
    assert_eq!(n, 104);
}
