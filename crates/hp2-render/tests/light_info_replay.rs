//! Replays the captured original `UnLight.cpp` light-info section from
//! `fixtures/light_info.txt`: `URender::GlobalLighting` (every `LT_*` type,
//! Realtime on and off, the palette branches) and `LightInfoDistCompare`.
//! The fixture is written by `Build/light_fixture_harness/sec_info.inc` against
//! the original `UnLight.cpp` (git ref `c9fc381`) with the product flags.
//! Results must match bit for bit.

#![cfg(target_arch = "aarch64")]

use std::cell::Cell;
use std::f32::consts::PI;
use std::ffi::{c_char, c_void};
use std::ptr::null_mut;
use std::str::FromStr;
use std::sync::LazyLock;

use hp2_abi::FColor;
use hp2_abi::engine::generated::{AActor, ALevelInfo, TArrayRaw, UPalette, UTexture};
use hp2_render::light_info::{self, GlobalParams};
use hp2_render::services::Hp2RenderServices;
use hp2_render::services_light::Hp2LightServices;

const FIXTURE: &str = include_str!("fixtures/light_info.txt");

/// Record counts, as emitted by `light_gen_info`.
const GLOBAL_RECORDS: usize = 240;
const DIST_RECORDS: usize = 64;

thread_local! {
    /// Owner state reported by the callbacks for the current `global` record.
    static LIFE: Cell<f32> = const { Cell::new(0.0) };
    static HSV: Cell<[f32; 4]> = const { Cell::new([0.0; 4]) };
}

/// Engine stand-ins that the light-info records never reach. A call panics, which fails the test.
macro_rules! never {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            #[allow(unused_variables)]
            unsafe extern "C" fn $name($($arg: $ty),*) -> $ret {
                panic!(concat!("light_info replay reached ", stringify!($name)))
            }
        )*
    };
}

never! {
    unused_frand() -> f32;
    unused_log_warning(text: *const u8, len: usize) -> ();
    unused_cos_tab(angle: i32) -> f32;
    unused_sqrt_approx(f: f32) -> f32;
    unused_div_sqrt_approx(f: f32) -> f32;
    unused_div_approx(f: f32) -> f32;
    unused_gmath_div_sqrt_approx(f: f32) -> f32;
    unused_gmath_sqrt_approx(f: f32) -> f32;

    unused_line_check(actor: *mut c_void, light: *mut c_void) -> u32;
    unused_add_light(actor: *mut c_void, light: *mut c_void) -> u32;
    unused_sender_alive(index: i32, actor: *mut c_void) -> u32;
    unused_world_light_radius(actor: *mut c_void) -> f32;
    unused_world_volumetric_radius(actor: *mut c_void) -> f32;
    unused_view_vector(actor: *mut c_void, out: *mut c_void) -> ();
    unused_map_uncoords(map_coords: *const c_void, out: *mut c_void) -> ();
    unused_cache_get(kind: u32, actor: *mut c_void, out: *mut *mut c_void) -> u32;
    unused_cache_create(kind: u32, actor: *mut c_void, size: u32, out: *mut *mut c_void) -> ();
    unused_stat_clock() -> ();
    unused_stat_unclock() -> ();
    unused_surf_map_setup(ctx: *mut c_void, which: u32) -> ();
    unused_surf_zone_ambient(out: *mut c_void) -> ();
    unused_surf_cache_get(id: u64) -> *mut c_void;
    unused_surf_cache_create(id: u64, size: u64, pad: u64, check: u32) -> *mut c_void;
    unused_surf_gmem(bytes: u64) -> *mut c_void;
    unused_surf_gmem_mark(slot: u32, pop: u32) -> ();
    unused_surf_light_compute(i: u32, which: u32) -> ();
    unused_surf_light_spatial_fx(i: u32, shadow: *mut c_void) -> ();
    unused_surf_shadow_gen(i: u32, use_bits: u32, dest: *mut c_void) -> ();
    unused_random_base_data() -> *const f32;
    unused_random_data() -> *const f32;
    unused_light_compute(index: u32, frame: *mut c_void) -> ();
    unused_stat_vtric_inc() -> ();
    unused_stat_mesh_light_add(n: u32) -> ();
    unused_unlock_one() -> ();
    unused_mark_pop() -> ();
}

/// `GMath.SinTab` table: `TrigFLOAT[i] = appSin(i*2*PI/16384)`, as in the kernels replay.
static TRIG: LazyLock<Vec<f32>> = LazyLock::new(|| {
    (0..16384i32)
        .map(|i| {
            let arg = (i as f32) * 2.0f32 * PI / 16384.0f32;
            (arg as f64).sin() as f32
        })
        .collect()
});

// `GMath.SinTab(i) = TrigFLOAT[(i>>2)&16383]`.
unsafe extern "C" fn sin_tab(angle: i32) -> f32 {
    TRIG[((angle >> 2) & 16383) as usize]
}

fn render_services() -> Hp2RenderServices {
    Hp2RenderServices {
        frand: unused_frand,
        log_warning: unused_log_warning,
        sin_tab,
        cos_tab: unused_cos_tab,
        sqrt_approx: unused_sqrt_approx,
        div_sqrt_approx: unused_div_sqrt_approx,
        div_approx: unused_div_approx,
        gmath_div_sqrt_approx: unused_gmath_div_sqrt_approx,
        gmath_sqrt_approx: unused_gmath_sqrt_approx,
    }
}

/// Palette entry `i` as the fixture's closed form, packed `R | G<<8 | B<<16 | A<<24`.
fn palette_packed(i: i32) -> u32 {
    let r = ((i * 7 + 3) & 255) as u32;
    let g = ((i * 13 + 5) & 255) as u32;
    let b = ((i * 29 + 11) & 255) as u32;
    let a = ((i * 17 + 1) & 255) as u32;
    r | (g << 8) | (b << 16) | (a << 24)
}

/// `palette_packed(i)` decoded into the `FColor` the real `UPalette.Colors` mirror holds.
fn palette_color_at(i: i32) -> FColor {
    let packed = palette_packed(i);
    FColor {
        r: packed as u8,
        g: (packed >> 8) as u8,
        b: (packed >> 16) as u8,
        a: (packed >> 24) as u8,
    }
}

unsafe extern "C" fn owner_life_fraction(_owner: *mut c_void) -> f32 {
    LIFE.get()
}

// `FGetHSV(hue, sat, 255)`: the fixture records the value per case.
unsafe extern "C" fn global_hsv(_hue: u8, _sat: u8, out: *mut c_void) {
    unsafe { *(out as *mut [f32; 4]) = HSV.get() };
}

/// A `check` in the light-info path would abort the test. The records never reach one.
unsafe extern "C" fn check_must_not_fire(_expr: *const c_char, _line: i32) {
    panic!("oracle check fired during light_info replay");
}

fn light_services() -> Hp2LightServices {
    Hp2LightServices {
        check_failed: check_must_not_fire,
        line_check: unused_line_check,
        add_light: unused_add_light,
        sender_alive: unused_sender_alive,
        owner_life_fraction,
        world_light_radius: unused_world_light_radius,
        world_volumetric_radius: unused_world_volumetric_radius,
        view_vector: unused_view_vector,
        map_uncoords: unused_map_uncoords,
        global_hsv,
        cache_get: unused_cache_get,
        cache_create: unused_cache_create,
        stat_clock: unused_stat_clock,
        stat_unclock: unused_stat_unclock,
        surf_map_setup: unused_surf_map_setup,
        surf_zone_ambient: unused_surf_zone_ambient,
        surf_cache_get: unused_surf_cache_get,
        surf_cache_create: unused_surf_cache_create,
        surf_gmem: unused_surf_gmem,
        surf_gmem_mark: unused_surf_gmem_mark,
        surf_light_compute: unused_surf_light_compute,
        surf_light_spatial_fx: unused_surf_light_spatial_fx,
        surf_shadow_gen: unused_surf_shadow_gen,
        random_base_data: unused_random_base_data,
        random_data: unused_random_data,
        light_compute: unused_light_compute,
        stat_vtric_inc: unused_stat_vtric_inc,
        stat_mesh_light_add: unused_stat_mesh_light_add,
        unlock_one: unused_unlock_one,
        mark_pop: unused_mark_pop,
    }
}

fn hex_u32(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex word {word:?}: {e}"))
}

fn hex_f32(word: &str) -> f32 {
    f32::from_bits(hex_u32(word))
}

fn dec<T: FromStr>(word: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    word.parse()
        .unwrap_or_else(|e| panic!("decimal word {word:?}: {e:?}"))
}

#[test]
fn light_info_replay() {
    let render = render_services();
    let light = light_services();
    let mut random_base: Option<[f32; 256]> = None;
    let mut strobe_last = 0.0f32;
    let mut strobe_toggle = 0i32;
    let mut globals = 0usize;
    let mut dists = 0usize;

    // `global()` now dereferences the owner directly (`Owner->Level->TimeSeconds`,
    // `Owner->Skin->Palette`), except for `LT_FLICKER`'s raw-address `RandomBase`
    // index. Real backing memory, mutated per record; the palette table is the
    // fixture's closed form (`palette_packed`), identical for every record.
    let colors: [FColor; 256] = std::array::from_fn(|i| palette_color_at(i as i32));
    let mut palette: UPalette = unsafe { std::mem::zeroed() };
    palette.colors = TArrayRaw {
        data: colors.as_ptr() as *mut c_void,
        num: 256,
        max: 256,
    };
    let mut texture: UTexture = unsafe { std::mem::zeroed() };
    texture.base.palette = (&raw mut palette).cast();
    let mut level: ALevelInfo = unsafe { std::mem::zeroed() };
    let mut actor: AActor = unsafe { std::mem::zeroed() };
    actor.level = (&raw mut level).cast();

    for line in FIXTURE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            None => continue,
            Some("randombase") => {
                assert_eq!(f.len(), 1 + 256, "randombase field count");
                let mut table = [0.0f32; 256];
                for (slot, word) in table.iter_mut().zip(&f[1..]) {
                    *slot = hex_f32(word);
                }
                random_base = Some(table);
            }
            Some("global") => {
                let table = random_base
                    .as_ref()
                    .expect("randombase precedes global records");
                assert_eq!(f.len(), 22, "global field count: {line:?}");
                // Record: TYPE REALTIME HUE SAT PERIOD PHASE PAL OFF TIME LIFESPAN LIFE BRIGHT_IN
                //         HSV(4) BRIGHT_OUT COLOR(4).
                // `OFF` is the owner's pool offset: only `LT_FLICKER`'s
                // `UPTRINT(Owner) & 255` index needs it; every other type
                // dereferences the owner, so it gets the real `actor` fixture.
                let off: usize = dec(f[8]);
                let light_type: u32 = dec(f[1]);
                // Direct writes to `level`/`texture` through the named place
                // retag their memory; re-derive the raw pointers afterward so
                // `actor.level`/`actor.skin` keep valid provenance for `global()`'s
                // dereference.
                level.time_seconds = hex_f32(f[9]);
                actor.level = (&raw mut level).cast();
                LIFE.set(hex_f32(f[11]));
                actor.skin = if dec::<u32>(f[7]) != 0 {
                    (&raw mut texture).cast()
                } else {
                    null_mut()
                };
                HSV.set([
                    hex_f32(f[13]),
                    hex_f32(f[14]),
                    hex_f32(f[15]),
                    hex_f32(f[16]),
                ]);
                let owner = if light_type == light_info::LT_FLICKER {
                    off as *mut c_void
                } else {
                    (&raw mut actor).cast()
                };
                let mut p = GlobalParams {
                    owner,
                    realtime: dec(f[2]),
                    light_type,
                    light_period: dec(f[5]),
                    light_phase: dec(f[6]),
                    light_hue: dec(f[3]),
                    light_saturation: dec(f[4]),
                    brightness: hex_f32(f[12]),
                    color: [0.0; 4],
                    strobe_last,
                    strobe_toggle,
                };
                light_info::global(&render, &light, table, &mut p);
                // Strobe state carries across records in file order, as the oracle's statics do.
                strobe_last = p.strobe_last;
                strobe_toggle = p.strobe_toggle;

                assert_eq!(
                    p.brightness.to_bits(),
                    hex_u32(f[17]),
                    "brightness: {line:?}"
                );
                for k in 0..4 {
                    assert_eq!(
                        p.color[k].to_bits(),
                        hex_u32(f[18 + k]),
                        "color[{k}]: {line:?}"
                    );
                }
                globals += 1;
            }
            Some("dist") => {
                assert_eq!(f.len(), 4, "dist field count: {line:?}");
                let a = hex_f32(f[1]);
                let b = hex_f32(f[2]);
                let got = light_info::dist_compare([0.0, 0.0, a], [0.0, 0.0, b]);
                assert_eq!(got, dec::<i32>(f[3]), "dist_compare: {line:?}");
                dists += 1;
            }
            Some(other) => panic!("unknown record kind {other:?}"),
        }
    }

    assert!(random_base.is_some(), "no randombase record");
    assert_eq!(globals, GLOBAL_RECORDS, "global record count");
    assert_eq!(dists, DIST_RECORDS, "dist record count");
}
