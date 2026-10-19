//! Replays the original-C++ oracle from `Build/particle_fixture_harness/` (`UParticle::Update`
//! from `UnParticle.cpp` at the pinned ref) against `hp2_engine::particle::update`.
//!
//! `fixtures/particle_update.txt`: 640 cases, one record per line. Every field is a hex `u32`
//! (floats as bit patterns, ints as two's complement):
//!
//! `seed timestep gravity[3] system_damping system_wind[3] sys.damping sys.wind_modifier
//! sys.b_wind_per_particle sys.location[3] sys.rotation[3] sys.last_update_location[3]
//! sys.last_update_rotation[3] sys.attraction[3] sys.chaos sys.chaos_delay sys.alpha_grow_period
//! sys.alpha_delay sys.color_delay sys.size_grow_period sys.size_delay sys.elasticity
//! sys.b_update sys.b_system_relative sys.render_primitive palette_mode particle_in[35] ret
//! particle_out[35]`
//!
//! `palette_mode`: 0 = no colour palette, 1 = texture with a palette, 2 = texture without one.
//!
//! Mock contract (each mirrors a harness hook exactly):
//! - `app_frand`: `state = seed`; each call `state = state*6364136223846793005 +
//!   1442695040888963407`, returns `(state >> 40) as f32 * 2^-24`.
//! - `sin_tab`/`cos_tab`: `GMath.TrigFLOAT` from `fixtures/particle_trig.txt`, indexed by
//!   `(i >> 2) & 16383` (`cos` adds 16384 first).
//! - `total_wind`: `(loc.y*0.5, -loc.x*0.25, 3.0)`.
//! - `fast_line_check`: `(((end.x as i32) ^ (start.y as i32)) & 3) != 0`.
//! - `system_trace_level_hit`: `sel = ((end.y as i32) ^ (start.z as i32)) & 3`. Sel 1 or 3 is a hit
//!   on the level actor; sel 0 (no hit) and sel 2 (non-level actor) return 0. A hit writes
//!   `normal` by `(end.x as i32) & 3` (0 +Z, 1 +X, 2 -Y, 3 -Z) and `location = end * 0.5`.
//! - `palette_plane`: index `(age/lifetime*255.0) as i32` when `lifetime > 0 && age > color_delay`,
//!   else 0. Colour `i` is `(7i, 3i, 5i)` as bytes with `w = 0`. A texture without a palette
//!   writes nothing.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;
use std::sync::LazyLock;

use hp2_abi::engine::generated::{AParticleFX, FRotator, UParticle};
use hp2_abi::{FPlane, FVector};
use hp2_engine::particle::{self, Hp2ParticleServices};

const FIXTURE: &str = include_str!("fixtures/particle_update.txt");
const TRIG_FIXTURE: &str = include_str!("fixtures/particle_trig.txt");

const CASES: usize = 640;
const TRIG_ENTRIES: usize = 16384;
const PARTICLE_WORDS: usize = 35;
const RECORD_FIELDS: usize = 110;

/// Non-null stand-ins for `UTexture*`, passed as the palette pointer. They are never dereferenced.
const TEXTURE_WITH_PALETTE: usize = 1;
const TEXTURE_WITHOUT_PALETTE: usize = 2;

const _: () = assert!(std::mem::size_of::<UParticle>() == PARTICLE_WORDS * 4);

thread_local! {
    static FRAND_STATE: Cell<u64> = const { Cell::new(0) };
}

static TRIG: LazyLock<Vec<f32>> = LazyLock::new(|| {
    let table: Vec<f32> = TRIG_FIXTURE
        .lines()
        .map(|l| f32::from_bits(u32::from_str_radix(l, 16).expect("trig hex")))
        .collect();
    assert_eq!(table.len(), TRIG_ENTRIES, "trig fixture entry count");
    table
});

fn trig_table() -> &'static [f32] {
    &TRIG
}

fn f(hex: &str) -> f32 {
    f32::from_bits(u(hex))
}

fn u(hex: &str) -> u32 {
    u32::from_str_radix(hex, 16).expect("hex word")
}

fn i(hex: &str) -> i32 {
    u(hex) as i32
}

fn v(xs: &[&str]) -> FVector {
    FVector {
        x: f(xs[0]),
        y: f(xs[1]),
        z: f(xs[2]),
    }
}

fn rot(xs: &[&str]) -> FRotator {
    FRotator {
        pitch: i(xs[0]),
        yaw: i(xs[1]),
        roll: i(xs[2]),
    }
}

fn words(ts: &[&str]) -> [u32; PARTICLE_WORDS] {
    assert_eq!(ts.len(), PARTICLE_WORDS);
    std::array::from_fn(|k| u(ts[k]))
}

fn particle_from(w: [u32; PARTICLE_WORDS]) -> UParticle {
    // SAFETY: `UParticle` is 35 four-byte float/int fields with no padding and no invalid bit
    // patterns, so any word array is a valid value.
    unsafe { std::mem::transmute::<[u32; PARTICLE_WORDS], UParticle>(w) }
}

fn particle_words(p: &UParticle) -> [u32; PARTICLE_WORDS] {
    // SAFETY: the same layout as `particle_from`; reads the 140 bytes of a valid `UParticle`.
    unsafe { ptr::read(ptr::from_ref(p).cast::<[u32; PARTICLE_WORDS]>()) }
}

// SAFETY (all mocks): the pointers come from `particle::update`, which passes live `FVector`,
// `FPlane` and `AParticleFX` storage owned by the test.

unsafe extern "C" fn mock_frand() -> f32 {
    FRAND_STATE.with(|s| {
        let next = s
            .get()
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        s.set(next);
        ((next >> 40) as u32) as f32 * (1.0f32 / 16_777_216.0f32)
    })
}

unsafe extern "C" fn mock_sin(i: i32) -> f32 {
    trig_table()[((i >> 2) & 16383) as usize]
}

unsafe extern "C" fn mock_cos(i: i32) -> f32 {
    trig_table()[((i.wrapping_add(16384) >> 2) & 16383) as usize]
}

unsafe extern "C" fn mock_total_wind(_level: *mut c_void, loc: *const c_void, out: *mut c_void) {
    // SAFETY: see the module note.
    let loc = unsafe { *loc.cast::<FVector>() };
    unsafe {
        *out.cast::<FVector>() = FVector {
            x: loc.y * 0.5,
            y: -loc.x * 0.25,
            z: 3.0,
        };
    }
}

unsafe extern "C" fn mock_fast_line_check(
    _level: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) -> i32 {
    // SAFETY: see the module note.
    let (end, start) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    ((((end.x as i32) ^ (start.y as i32)) & 3) != 0) as i32
}

unsafe extern "C" fn mock_trace_level_hit(
    _system: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    normal: *mut c_void,
    location: *mut c_void,
) -> i32 {
    // SAFETY: see the module note.
    let (end, start) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    let sel = ((end.y as i32) ^ (start.z as i32)) & 3;
    if sel != 1 && sel != 3 {
        return 0;
    }
    let n = match (end.x as i32) & 3 {
        0 => FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        1 => FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        2 => FVector {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
        _ => FVector {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    };
    unsafe {
        *normal.cast::<FVector>() = n;
        *location.cast::<FVector>() = FVector {
            x: end.x * 0.5,
            y: end.y * 0.5,
            z: end.z * 0.5,
        };
    }
    1
}

unsafe extern "C" fn mock_palette_plane(
    color_palette: *mut c_void,
    age: f32,
    lifetime: f32,
    color_delay: f32,
    out: *mut c_void,
) {
    if color_palette as usize == TEXTURE_WITHOUT_PALETTE {
        return;
    }
    assert_eq!(
        color_palette as usize, TEXTURE_WITH_PALETTE,
        "unexpected palette pointer"
    );
    let index = if lifetime > 0.0 && age > color_delay {
        ((age / lifetime) * 255.0f32) as i32
    } else {
        0
    };
    assert!(
        (0..256).contains(&index),
        "palette index {index} out of range"
    );
    let k = index as u32;
    let plane = FPlane {
        x: (7 * k) as u8 as f32 / 255.0,
        y: (3 * k) as u8 as f32 / 255.0,
        z: (5 * k) as u8 as f32 / 255.0,
        w: 0.0,
    };
    // SAFETY: `out` is `&mut p.color`, a live `FPlane`.
    unsafe { *out.cast::<FPlane>() = plane };
}

static SERVICES: Hp2ParticleServices = Hp2ParticleServices {
    app_frand: mock_frand,
    sin_tab: mock_sin,
    cos_tab: mock_cos,
    total_wind: mock_total_wind,
    fast_line_check: mock_fast_line_check,
    system_trace_level_hit: mock_trace_level_hit,
    palette_plane: mock_palette_plane,
};

#[test]
fn particle_update_fixtures_bit_exact() {
    let mut cases = 0usize;
    let mut returned_true = 0usize;
    let mut palette_modes = [0usize; 3];

    for line in FIXTURE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t.len(), RECORD_FIELDS, "malformed record: {line}");

        let seed = u(t[0]);
        let timestep = f(t[1]);
        let gravity = v(&t[2..5]);
        let system_damping = f(t[5]);
        let system_wind = v(&t[6..9]);
        let palette_mode = u(t[38]);
        let ret_expect = u(t[74]) != 0;

        let mut sys: AParticleFX = unsafe { std::mem::zeroed() };
        sys.damping = f(t[9]);
        sys.wind_modifier = f(t[10]);
        sys.set_b_wind_per_particle(u(t[11]) != 0);
        sys.base.location = v(&t[12..15]);
        sys.base.rotation = rot(&t[15..18]);
        sys.last_update_location = v(&t[18..21]);
        sys.last_update_rotation = rot(&t[21..24]);
        sys.attraction = v(&t[24..27]);
        sys.chaos = f(t[27]);
        sys.chaos_delay = f(t[28]);
        sys.alpha_grow_period = f(t[29]);
        sys.alpha_delay = f(t[30]);
        sys.color_delay = f(t[31]);
        sys.size_grow_period = f(t[32]);
        sys.size_delay = f(t[33]);
        sys.elasticity = f(t[34]);
        sys.set_b_update(u(t[35]) != 0);
        sys.set_b_system_relative(u(t[36]) != 0);
        sys.render_primitive = i(t[37]) as u8;
        sys.color_palette = match palette_mode {
            0 => ptr::null_mut(),
            1 => TEXTURE_WITH_PALETTE as *mut c_void,
            2 => TEXTURE_WITHOUT_PALETTE as *mut c_void,
            other => panic!("palette mode {other} out of range"),
        };

        let mut particle = particle_from(words(&t[39..74]));
        FRAND_STATE.with(|s| s.set(u64::from(seed)));

        let ret = unsafe {
            particle::update(
                &mut particle,
                gravity,
                system_damping,
                system_wind,
                ptr::null_mut(),
                timestep,
                ptr::from_mut(&mut sys).cast::<c_void>(),
                &SERVICES,
            )
        };

        assert_eq!(ret, ret_expect, "return value: {line}");
        assert_eq!(
            particle_words(&particle),
            words(&t[75..110]),
            "particle_out: {line}"
        );

        cases += 1;
        returned_true += usize::from(ret);
        palette_modes[palette_mode as usize] += 1;
    }

    assert_eq!(cases, CASES, "update record count");
    assert!(
        returned_true > 0 && returned_true < CASES,
        "both outcomes covered"
    );
    assert!(
        palette_modes.iter().all(|&n| n > 0),
        "every palette mode covered"
    );
}
