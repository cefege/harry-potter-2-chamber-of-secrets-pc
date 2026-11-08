//! Record-level replay of the original `FLightInfo::ComputeFromActor` section
//! (`fixtures/light_info_compute.txt`), written by
//! `Build/light_fixture_harness/sec_info_compute.inc` against the original
//! `UnLight.cpp` (git ref `c9fc381`).
//!
//! Each record's inputs go to `light_info::compute_from_actor`. Every value the
//! harness printed is then compared bit for bit, in print order: the outputs,
//! the 256-entry palettes, `TemporaryTablesBuilt`, `FLightManager::MapUncoords`
//! and the vertex basis.
//!
//! Leaf services are stand-ins for engine calls. They return the values that the
//! harness hook read from the engine (actor radii, owner life and time, the
//! viewer's `Vector()` for plane sources, the `FCoords` inverse, and the GCache
//! keyed map). Those values are echoed into the record, so a wrong port still
//! fails the comparison.

#![cfg(target_arch = "aarch64")]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::ffi::{c_char, c_void};
use std::str::FromStr;
use std::sync::LazyLock;

use hp2_abi::FColor;
use hp2_abi::engine::generated::{AActor, ALevelInfo, TArrayRaw, UPalette, UTexture};
use hp2_render::light_info::{self, ComputeParams, LightCoords};
use hp2_render::light_manager;
use hp2_render::services::Hp2RenderServices;
use hp2_render::services_light::Hp2LightServices;

const FIXTURE: &str = include_str!("fixtures/light_info_compute.txt");
const INFO_FIXTURE: &str = include_str!("fixtures/light_info.txt");

/// Distinct cache kinds, passed through `ComputeParams::cid_*` by the port.
const CID_PALETTE: u32 = 1;
const CID_VOLUMETRIC: u32 = 2;

/// Sentinel table addresses. Rust never dereferences them; `LightDistTab` is
/// mapped back to the record's -1 / 0 / 1 code.
const DIST_NEAR: usize = 0x1000;
const DIST_FAR: usize = 0x2000;

const ZERO_COORDS: LightCoords = LightCoords {
    origin: [0.0; 3],
    x_axis: [0.0; 3],
    y_axis: [0.0; 3],
    z_axis: [0.0; 3],
};

/// Owner and frame values reported by the callbacks for the current record.
#[derive(Clone, Copy)]
struct Rec {
    world_radius: f32,
    world_vol_radius: f32,
    life: f32,
    /// `FLightManager::MapUncoords` as the engine inverse, read from the record's `MAPU`.
    map_uncoords: LightCoords,
}

/// Call counts per service family.
#[derive(Clone, Copy, Default)]
struct Counts {
    world_light_radius: usize,
    world_volumetric_radius: usize,
    global_hsv: usize,
    view_vector: usize,
    map_uncoords: usize,
    cache_get: usize,
    cache_create: usize,
    stat_clock: usize,
    stat_unclock: usize,
}

/// GCache stand-in key/value map type; see `CACHE`.
type CacheMap = HashMap<(u32, usize), Box<[u32; 1024]>>;

thread_local! {
    static REC: Cell<Rec> = const {
        Cell::new(Rec {
            world_radius: 0.0,
            world_vol_radius: 0.0,
            life: 0.0,
            map_uncoords: ZERO_COORDS,
        })
    };
    /// GCache stand-in: `(kind, actor) -> 1024-word block`. Boxes keep their address when the map grows.
    static CACHE: RefCell<CacheMap> = RefCell::new(HashMap::new());
    static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
}

/// Engine stand-ins that the compute records never reach. A call panics, which fails the test.
macro_rules! never {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            #[allow(unused_variables)]
            unsafe extern "C" fn $name($($arg: $ty),*) -> $ret {
                panic!(concat!("light_info_compute replay reached ", stringify!($name)))
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

/// `GMath.SinTab` table: `TrigFLOAT[i] = appSin(i*2*PI/16384)`, as in the other light replays.
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

/// `palette_packed(i)` decoded into the `FColor` fields the real `UPalette.Colors`
/// mirror holds, for building the fixture's real backing memory.
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
    REC.get().life
}

/// `FGetHSV(hue, sat, 255)`: `(H/255, S/255, 255/255, 1)` as float multiplies by `1/255`.
unsafe extern "C" fn global_hsv(hue: u8, sat: u8, out: *mut c_void) {
    COUNTS.with_borrow_mut(|c| c.global_hsv += 1);
    let k = 1.0f32 / 255.0f32;
    let v = [f32::from(hue) * k, f32::from(sat) * k, 255.0f32 * k, 1.0f32];
    unsafe { *(out as *mut [f32; 4]) = v };
}

unsafe extern "C" fn world_light_radius(_actor: *mut c_void) -> f32 {
    COUNTS.with_borrow_mut(|c| c.world_light_radius += 1);
    REC.get().world_radius
}

unsafe extern "C" fn world_volumetric_radius(_actor: *mut c_void) -> f32 {
    COUNTS.with_borrow_mut(|c| c.world_volumetric_radius += 1);
    REC.get().world_vol_radius
}

/// `LD_Plane` direction: the harness's zeroed viewer gives `Vector() = (0, 1, 0)`.
/// The record's `LightDir` output checks this value.
unsafe extern "C" fn view_vector(_actor: *mut c_void, out: *mut c_void) {
    COUNTS.with_borrow_mut(|c| c.view_vector += 1);
    // The harness never sets Rotation, so the zeroed actor gives FRotator(0,0,0).Vector() = (1,0,0).
    unsafe { *(out as *mut [f32; 3]) = [1.0, 0.0, 0.0] };
}

/// `FCoords` inverse of `MapC`, as computed by the engine.
unsafe extern "C" fn map_uncoords(_map_coords: *const c_void, out: *mut c_void) {
    COUNTS.with_borrow_mut(|c| c.map_uncoords += 1);
    unsafe { *(out as *mut LightCoords) = REC.get().map_uncoords };
}

unsafe extern "C" fn cache_get(kind: u32, actor: *mut c_void, out: *mut *mut c_void) -> u32 {
    COUNTS.with_borrow_mut(|c| c.cache_get += 1);
    let hit = CACHE.with_borrow(|m| {
        m.get(&(kind, actor as usize))
            .map(|b| b.as_ptr() as *mut c_void)
    });
    match hit {
        Some(ptr) => {
            unsafe { *out = ptr };
            1
        }
        None => 0,
    }
}

unsafe extern "C" fn cache_create(kind: u32, actor: *mut c_void, size: u32, out: *mut *mut c_void) {
    COUNTS.with_borrow_mut(|c| c.cache_create += 1);
    assert!(
        size as usize <= 4096,
        "cache block {size} exceeds the stand-in"
    );
    let mut block = Box::new([0u32; 1024]);
    let ptr = &mut *block as *mut [u32; 1024] as *mut c_void;
    let prev = CACHE.with_borrow_mut(|m| m.insert((kind, actor as usize), block));
    assert!(prev.is_none(), "cache_create over a live entry");
    unsafe { *out = ptr };
}

unsafe extern "C" fn stat_clock() {
    COUNTS.with_borrow_mut(|c| c.stat_clock += 1);
}

unsafe extern "C" fn stat_unclock() {
    COUNTS.with_borrow_mut(|c| c.stat_unclock += 1);
}

/// A `check` in the compute path would abort the test. The records never reach one.
unsafe extern "C" fn check_must_not_fire(_expr: *const c_char, _line: i32) {
    panic!("oracle check fired during light_info_compute replay");
}

/// Distinct, reconstructible stand-in for one `Effects[]` entry. `copy_effect` is no longer a
/// mockable leaf (it is a direct `light_manager::State` read), so the test seeds `Effects[]`
/// with these before replay and checks the `FLightInfo::Effect` the port copied out matches the
/// slot the record's `LightEffect` selects, in place of the old call-count check.
fn effect_sentinel(index: u32) -> light_manager::Effect {
    light_manager::Effect {
        SpatialFxFunc: (0x4000 + index as usize * 8) as *mut c_void,
        IsSpatialDynamic: index as i32 * 3 + 1,
        IsMergeDynamic: index as i32 * 5 + 2,
    }
}

fn compute_services() -> Hp2LightServices {
    Hp2LightServices {
        check_failed: check_must_not_fire,
        line_check: unused_line_check,
        add_light: unused_add_light,
        sender_alive: unused_sender_alive,
        owner_life_fraction,
        world_light_radius,
        world_volumetric_radius,
        view_vector,
        map_uncoords,
        global_hsv,
        cache_get,
        cache_create,
        stat_clock,
        stat_unclock,
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

/// Keyword and separator tokens: `|` and all-uppercase words (`LOC`, `OUT`, `VOLPAL`, ...).
fn is_keyword(word: &str) -> bool {
    word == "|" || (!word.is_empty() && word.bytes().all(|b| b.is_ascii_uppercase()))
}

fn random_base() -> [f32; 256] {
    let line = INFO_FIXTURE
        .lines()
        .find(|l| l.split_whitespace().next() == Some("randombase"))
        .expect("light_info.txt has a randombase record");
    let words: Vec<&str> = line.split_whitespace().skip(1).collect();
    assert_eq!(words.len(), 256, "randombase field count");
    let mut table = [0.0f32; 256];
    for (slot, word) in table.iter_mut().zip(words) {
        *slot = hex_f32(word);
    }
    table
}

/// Token cursor over one record. Keyword tokens are already removed.
struct Cur<'a> {
    words: Vec<&'a str>,
    at: usize,
    line: usize,
}

impl<'a> Cur<'a> {
    fn word(&mut self) -> &'a str {
        let w = *self
            .words
            .get(self.at)
            .unwrap_or_else(|| panic!("line {}: fields exhausted at {}", self.line, self.at));
        self.at += 1;
        w
    }

    fn bits(&mut self) -> u32 {
        hex_u32(self.word())
    }

    fn f(&mut self) -> f32 {
        f32::from_bits(self.bits())
    }

    fn int(&mut self) -> i64 {
        dec(self.word())
    }

    fn v3(&mut self) -> [f32; 3] {
        [self.f(), self.f(), self.f()]
    }

    fn plane(&mut self) -> [f32; 4] {
        [self.f(), self.f(), self.f(), self.f()]
    }

    /// `FCoords`: Origin, XAxis, YAxis, ZAxis, each printed as an `FPlane` with W = 0.
    fn coords(&mut self) -> LightCoords {
        let mut axes = [[0.0f32; 3]; 4];
        for axis in &mut axes {
            *axis = self.v3();
            assert_eq!(self.bits(), 0, "line {}: FCoords W must be zero", self.line);
        }
        LightCoords {
            origin: axes[0],
            x_axis: axes[1],
            y_axis: axes[2],
            z_axis: axes[3],
        }
    }

    fn palette(&mut self) -> [u32; 256] {
        let mut out = [0u32; 256];
        for word in &mut out {
            *word = self.bits();
        }
        out
    }
}

fn same_words(line: usize, name: &str, got: &[u32; 256], want: &[u32; 256]) {
    if let Some(i) = (0..256).find(|&i| got[i] != want[i]) {
        panic!(
            "line {line}: {name}[{i}] got {:08x} want {:08x}",
            got[i], want[i]
        );
    }
}

fn same_f(line: usize, name: &str, got: f32, want: f32) {
    assert_eq!(got.to_bits(), want.to_bits(), "line {line}: {name}");
}

fn same_v3(line: usize, name: &str, got: [f32; 3], want: [f32; 3]) {
    for i in 0..3 {
        same_f(line, &format!("{name}[{i}]"), got[i], want[i]);
    }
}

fn same_plane(line: usize, name: &str, got: [f32; 4], want: [f32; 4]) {
    for i in 0..4 {
        same_f(line, &format!("{name}[{i}]"), got[i], want[i]);
    }
}

fn same_coords(line: usize, name: &str, got: LightCoords, want: LightCoords) {
    same_v3(line, &format!("{name}.origin"), got.origin, want.origin);
    same_v3(line, &format!("{name}.x"), got.x_axis, want.x_axis);
    same_v3(line, &format!("{name}.y"), got.y_axis, want.y_axis);
    same_v3(line, &format!("{name}.z"), got.z_axis, want.z_axis);
}

/// Maps `LightDistTab` back to the record code: -1 null, 0 `LightDist`, 1 `LightDistInc`.
fn dist_code(ptr: *mut c_void) -> i64 {
    match ptr as usize {
        0 => -1,
        DIST_NEAR => 0,
        DIST_FAR => 1,
        other => panic!("LightDistTab {other:#x} is not a known table"),
    }
}

/// Palette words read back from the cache block the port wrote.
fn words_at(ptr: *mut c_void) -> [u32; 256] {
    let slice = unsafe { std::slice::from_raw_parts(ptr as *const u32, 256) };
    let mut out = [0u32; 256];
    out.copy_from_slice(slice);
    out
}

fn actor_ptr(case: i64) -> *mut c_void {
    (0x10_0000usize + (case as usize) * 0x100) as *mut c_void
}

/// Real backing memory for the `Owner` pointer `light_info::global`'s
/// `owner_time_seconds`/`owner_palette`/`palette_color` now dereference
/// directly (`AActor -> Level -> TimeSeconds`, `AActor -> Skin -> Palette ->
/// Colors`). One instance is reused across every call of a given `case` (the
/// GCache mocks key on `actor as usize`, so the address must stay stable
/// within a case). Heap-boxed so the sub-structs never move.
struct OwnerFixture {
    actor: AActor,
    level: ALevelInfo,
    texture: UTexture,
    palette: UPalette,
    colors: [FColor; 256],
}

impl OwnerFixture {
    fn new() -> Box<OwnerFixture> {
        let mut fx: Box<OwnerFixture> = Box::new(unsafe { std::mem::zeroed() });
        for i in 0..256i32 {
            fx.colors[i as usize] = palette_color_at(i);
        }
        fx.palette.colors = TArrayRaw {
            data: (&raw mut fx.colors).cast(),
            num: 256,
            max: 256,
        };
        fx.texture.base.palette = (&raw mut fx.palette).cast();
        fx.actor.level = (&raw mut fx.level).cast();
        fx.actor.skin = (&raw mut fx.texture).cast();
        fx
    }

    /// Refresh the per-record fields the record's `TIME`/`HASPALETTE` drive.
    /// Writing `self.level`/`self.actor` through the named place retags their
    /// memory, so `actor.level`/`actor.skin` are re-derived with `&raw mut`
    /// afterward to keep valid provenance for the later dereference.
    fn update(&mut self, time: f32, has_palette: bool) {
        self.level.time_seconds = time;
        self.actor.level = (&raw mut self.level).cast();
        self.actor.skin = if has_palette {
            (&raw mut self.texture).cast()
        } else {
            std::ptr::null_mut()
        };
    }

    fn ptr(&self) -> *mut c_void {
        (&raw const self.actor).cast_mut().cast()
    }
}

#[test]
fn light_info_compute_replay() {
    let render = render_services();
    let light = compute_services();
    // Seed `Effects[]` with reconstructible sentinels per slot (`copy_effect` reads these
    // directly off `light_manager::State` now; see `effect_sentinel`).
    unsafe {
        for i in 0..light_info::LE_MAX {
            (*light_manager::state()).Effects[i as usize] = effect_sentinel(i);
        }
    }
    let random_base = random_base();

    // Statics carried across records (`FLightManager`).
    let mut ttb: u32 = 0;
    let mut carry_map = ZERO_COORDS;
    let mut carry_vb = [[0.0f32; 3]; 3];
    // `global_Strobe` statics (`LastUpdateTime`, `Toggle`), zero at process start.
    let mut carry_strobe = (0.0f32, 0i32);

    let mut calls_seen: HashMap<i64, i64> = HashMap::new();
    let mut cases: HashSet<i64> = HashSet::new();
    let mut records = 0usize;
    let mut palette_seen = HashSet::new();
    let mut vol_records = 0usize;
    let mut map_records = 0usize;
    let mut plane_records = 0usize;
    let mut expect_map_uncoords = 0usize;
    let mut expect_cache_get = 0usize;
    let mut expect_cache_create = 0usize;
    let mut owner_fixtures: HashMap<i64, Box<OwnerFixture>> = HashMap::new();

    for (index, line) in FIXTURE.lines().enumerate() {
        let line_no = index + 1;
        let words: Vec<&str> = line.split_whitespace().filter(|w| !is_keyword(w)).collect();
        let mut cur = Cur {
            words,
            at: 0,
            line: line_no,
        };
        assert_eq!(
            cur.word(),
            "compute",
            "line {line_no}: not a compute record"
        );
        let case = cur.int();
        let call = cur.int();
        let seen = calls_seen.entry(case).or_insert(0);
        assert_eq!(*seen, call, "line {line_no}: call order for case {case}");
        *seen += 1;
        cases.insert(case);

        // LOC: inputs.
        let actor_location = cur.v3();
        let world_radius = cur.f();
        let world_vol_radius = cur.f();
        let light_radius_inner = cur.int();
        let light_brightness = cur.int();
        let light_effect = cur.int();
        let light_source = cur.int();
        let light_type = cur.int();
        let light_period = cur.int();
        let light_phase = cur.int();
        let light_hue = cur.int();
        let light_saturation = cur.int();
        let life = cur.f();
        let time = cur.f();
        let level_brightness = cur.f();
        let volume_fog = cur.f();
        let volume_brightness = cur.f();
        let light_changed = cur.int();
        let show_flags = cur.int();
        let is_vol = cur.int();
        let ttb_rec = cur.int();
        let has_map = cur.int();
        let has_palette = cur.int();
        palette_seen.insert((light_type, show_flags != 0, has_palette));
        let ptr_low = cur.int();
        let frame_coords = cur.coords();
        let map_coords = cur.coords();
        let map_pan_x = cur.f();
        let map_pan_y = cur.f();
        let map_uscale = cur.f();
        let map_vscale = cur.f();
        let light_pan_x = cur.f();
        let light_pan_y = cur.f();
        let light_uscale = cur.f();
        let light_vscale = cur.f();
        let light_uclamp = cur.int();
        let light_vclamp = cur.int();
        let map_uncoords_in = cur.coords();

        // The harness resets TemporaryTablesBuilt before each case's first call.
        if call == 0 {
            ttb = 0;
        }
        assert_eq!(ttb_rec, i64::from(ttb), "line {line_no}: TTB before call");

        REC.set(Rec {
            world_radius,
            world_vol_radius,
            life,
            map_uncoords: map_uncoords_in,
        });

        let mut effect: light_manager::Effect = unsafe { std::mem::zeroed() };
        let mut p: ComputeParams = unsafe { std::mem::zeroed() };
        // Owner pointer. `LT_FLICKER` never dereferences `Owner` (only its raw
        // address feeds `RandomBase`), so it keeps the record's `PTR` low byte
        // exactly (`UPTRINT(A) & 255`) over a synthetic high address. Every other
        // type now dereferences `Owner->Level->TimeSeconds` / `Owner->Skin->Palette`
        // directly, so it gets a real, stable-per-case backing `AActor`.
        p.actor = if light_type == i64::from(light_info::LT_FLICKER) {
            (actor_ptr(case) as usize | ptr_low as usize) as *mut c_void
        } else {
            let fx = owner_fixtures.entry(case).or_insert_with(OwnerFixture::new);
            fx.update(time, has_palette != 0);
            fx.ptr()
        };
        p.actor_location = actor_location;
        p.light_radius_inner = light_radius_inner as f32;
        p.light_brightness = light_brightness as f32;
        p.level_brightness = level_brightness;
        p.light_source = light_source as u32;
        p.light_effect = light_effect as u32;
        p.light_type = light_type as u32;
        p.light_period = light_period as u32;
        p.light_phase = light_phase as u32;
        p.light_hue = light_hue as u32;
        p.light_saturation = light_saturation as u32;
        p.volume_fog = volume_fog;
        p.volume_brightness = volume_brightness;
        p.light_changed = light_changed as u32;
        p.show_flags = show_flags as u32;
        p.is_volumetric = is_vol as u32;
        p.frame_coords = frame_coords;
        p.has_map = has_map as u32;
        p.map_coords = map_coords;
        p.map_pan_x = map_pan_x;
        p.map_pan_y = map_pan_y;
        p.map_uscale = map_uscale;
        p.map_vscale = map_vscale;
        p.light_pan_x = light_pan_x;
        p.light_pan_y = light_pan_y;
        p.light_uscale = light_uscale;
        p.light_vscale = light_vscale;
        p.light_uclamp = light_uclamp as i32;
        p.light_vclamp = light_vclamp as i32;
        p.light_dist = DIST_NEAR as *const c_void;
        p.light_dist_inc = DIST_FAR as *const c_void;
        p.cid_palette = CID_PALETTE;
        p.cid_volumetric = CID_VOLUMETRIC;
        p.effect_out = (&raw mut effect).cast::<c_void>();
        p.temporary_tables_built = ttb;
        p.map_uncoords = carry_map;
        p.vertex_base = carry_vb[0];
        p.vertex_du = carry_vb[1];
        p.vertex_dv = carry_vb[2];
        p.strobe_last = carry_strobe.0;
        p.strobe_toggle = carry_strobe.1;
        // FColor memory order is R,G,B,A: B at byte 2, G at 1, R at 0, A at 3.
        p.palette_offsets = [2, 1, 0, 3];

        light_info::compute_from_actor(&render, &light, &random_base, &mut p);

        // `Effect = Effects[(LightEffect<LE_MAX) ? LightEffect : 0]`, copied by `copy_effect`.
        let expect_effect_index = if light_effect < i64::from(light_info::LE_MAX) {
            light_effect as u32
        } else {
            0
        };
        let want_effect = effect_sentinel(expect_effect_index);
        let got_effect = effect;
        assert_eq!(
            got_effect.SpatialFxFunc, want_effect.SpatialFxFunc,
            "line {line_no}: Effect.SpatialFxFunc"
        );
        assert_eq!(
            got_effect.IsSpatialDynamic, want_effect.IsSpatialDynamic,
            "line {line_no}: Effect.IsSpatialDynamic"
        );
        assert_eq!(
            got_effect.IsMergeDynamic, want_effect.IsMergeDynamic,
            "line {line_no}: Effect.IsMergeDynamic"
        );

        records += 1;
        if is_vol != 0 {
            vol_records += 1;
        }
        if has_map != 0 {
            map_records += 1;
        }
        if light_source == 1 {
            plane_records += 1;
        }

        // Service-call expectations from the record's flags. Lighting palette: one cache_get per
        // HasMap record, and a miss plus one map_uncoords on the case's first call. Volumetric:
        // one cache_get per IsVol record, and a miss on the case's first call.
        if has_map != 0 {
            expect_cache_get += 1;
            if call == 0 {
                expect_map_uncoords += 1;
                expect_cache_create += 1;
            }
        }
        if is_vol != 0 {
            expect_cache_get += 1;
            if call == 0 {
                expect_cache_create += 1;
            }
        }

        // OUT: outputs in print order.
        same_f(line_no, "Radius", p.radius, cur.f());
        same_f(line_no, "RadiusInner", p.radius_inner, cur.f());
        same_f(line_no, "RRadius", p.rradius, cur.f());
        same_f(line_no, "RRadiusMult", p.rradius_mult, cur.f());
        same_v3(line_no, "Location", p.location, cur.v3());
        same_f(line_no, "Brightness", p.brightness, cur.f());
        same_plane(line_no, "FloatColor", p.float_color, cur.plane());
        same_v3(line_no, "MaxSpecDir", p.max_spec_dir, cur.v3());
        same_v3(line_no, "LightDir", p.light_dir, cur.v3());
        same_f(line_no, "Diffuse", p.diffuse, cur.f());
        assert_eq!(
            dist_code(p.light_dist_tab),
            cur.int(),
            "line {line_no}: LightDistTab"
        );
        assert_eq!(i64::from(p.min_u), cur.int(), "line {line_no}: MinU");
        assert_eq!(i64::from(p.max_u), cur.int(), "line {line_no}: MaxU");
        assert_eq!(i64::from(p.min_v), cur.int(), "line {line_no}: MinV");
        assert_eq!(i64::from(p.max_v), cur.int(), "line {line_no}: MaxV");

        // PAL: present iff HasMap.
        if has_map != 0 {
            let want = cur.palette();
            same_words(line_no, "Palette", &words_at(p.palette), &want);
        }

        // VOL: present iff IsVol.
        assert_eq!(cur.int(), is_vol, "line {line_no}: VOL");
        if is_vol != 0 {
            let want = cur.palette();
            same_words(line_no, "VolPalette", &words_at(p.vol_palette), &want);
            same_plane(line_no, "VolumetricColor", p.volumetric_color, cur.plane());
        }
        same_f(line_no, "VolRadius", p.vol_radius, cur.f());
        same_f(line_no, "VolRadiusSquared", p.vol_radius_squared, cur.f());
        same_f(line_no, "RVolRadius", p.rvol_radius, cur.f());
        same_f(line_no, "RVolRadiusSquared", p.rvol_radius_squared, cur.f());
        same_f(
            line_no,
            "LocationSizeSquared",
            p.location_size_squared,
            cur.f(),
        );
        same_f(line_no, "VolBrightness", p.vol_brightness, cur.f());
        assert_eq!(
            i64::from(p.vol_inside),
            cur.int(),
            "line {line_no}: VolInside"
        );

        // TTBA, MapUncoords, VB.
        assert_eq!(
            i64::from(p.temporary_tables_built),
            cur.int(),
            "line {line_no}: TTBA"
        );
        ttb = p.temporary_tables_built;
        same_coords(line_no, "MapUncoords", p.map_uncoords, cur.coords());
        carry_map = p.map_uncoords;
        same_v3(line_no, "VertexBase", p.vertex_base, cur.v3());
        same_v3(line_no, "VertexDU", p.vertex_du, cur.v3());
        same_v3(line_no, "VertexDV", p.vertex_dv, cur.v3());
        carry_vb = [p.vertex_base, p.vertex_du, p.vertex_dv];
        carry_strobe = (p.strobe_last, p.strobe_toggle);

        assert_eq!(cur.at, cur.words.len(), "line {line_no}: unconsumed fields");
    }

    // Exact per-family counts, derived from the fixture: 49 cases x 3 calls (k = 0..48).
    assert_eq!(records, 147, "compute records");
    assert_eq!(cases.len(), 49, "cases");
    assert!(calls_seen.values().all(|&n| n == 3), "3 calls per case");
    // IsVol: k%3==2 gives 16 cases. HasMap: k%5!=4 excludes 9 of the 49 cases, leaving 40.
    // LD_Plane: k%3==1 gives 16 cases.
    assert_eq!(vol_records, 48, "records with IsVol");
    assert_eq!(map_records, 120, "records with HasMap");
    assert_eq!(plane_records, 48, "records with LD_Plane");
    assert_eq!(expect_map_uncoords, 40, "HasMap cases");
    // Realtime types 8 (PaletteOnce) and 9 (PaletteLoop) must each reach both palette branches.
    for ty in [8, 9] {
        for pal in [0, 1] {
            assert!(
                palette_seen.contains(&(ty, true, pal)),
                "realtime type {ty} with has_palette {pal} not covered"
            );
        }
    }

    let counts = COUNTS.with_borrow(|c| *c);
    assert_eq!(
        counts.world_light_radius, records,
        "world_light_radius calls"
    );
    assert_eq!(counts.global_hsv, records, "global_hsv calls");
    assert_eq!(
        counts.world_volumetric_radius, vol_records,
        "world_volumetric_radius calls"
    );
    assert_eq!(counts.view_vector, plane_records, "view_vector calls");
    // One surface build per HasMap case, on its first call.
    assert_eq!(
        counts.map_uncoords, expect_map_uncoords,
        "map_uncoords calls"
    );
    assert_eq!(counts.cache_get, expect_cache_get, "cache_get calls");
    // Misses: lighting at C == 0 per HasMap case, volumetric at C == 0 per IsVol case.
    assert_eq!(
        counts.cache_create, expect_cache_create,
        "cache_create calls"
    );
    assert_eq!(counts.stat_clock, expect_cache_get, "stat_clock calls");
    assert_eq!(counts.stat_unclock, expect_cache_get, "stat_unclock calls");
}
