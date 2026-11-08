//! Replays the `SetupForSurf` oracle records in `fixtures/light_surf.txt`
//! (captured by `Build/light_fixture_harness/sec_surf.inc`): the map sizing
//! (`dims`), the light-kind dispatch (`kind`), the dynamic-cache outcome (`dyn`),
//! the mover-stamp compare (`stamp`), the permeating-light test (`dot`), the
//! ambient colour (`ambient`), the ambient fill and static copy loops (`fill`,
//! `copy`), the per-light fog merge (`fog`), and the log2 helper (`clog`). Results must match
//! bit for bit.
//! The `GMath` square-root tables come from `fixtures/light_kernels.txt`.
//!
//! Every record carries its inputs, so each replays on its own. Fog cases each
//! start with `FogRejectionMethod` at 0, as the harness primes it.

#![cfg(target_arch = "aarch64")]

use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

use hp2_abi::FVector;
use hp2_abi::engine::generated::AActor;
use hp2_abi::render_scene::FActorLink;
use hp2_render::light::{
    BYTE_FOG_LEN, FilterTab, GMath, PI, VolumetricParams, init_byte_fog, shadow_map_gen,
};
use hp2_render::light_manager;
use hp2_render::light_surf::{
    Coords, FogLightInputs, MoverStamp, SurfSetupParams, ambient_word, ceil_log_two,
    copy_static_rows, dims, dot_positive, dyn_cache_outcome, fill_ambient, fog_light, light_kind,
    mover_stamp_changed, setup_for_surf,
};
use hp2_render::services::Hp2RenderServices;
use hp2_render::services_light::Hp2LightServices;

const FIXTURE: &str = include_str!("fixtures/light_surf.txt");
const KERNELS: &str = include_str!("fixtures/light_kernels.txt");

// ---------------------------------------------------------------------------
// Fixture parsing
// ---------------------------------------------------------------------------

/// Every whitespace-split record whose first token is `tag`.
fn records(tag: &'static str) -> Vec<Vec<&'static str>> {
    FIXTURE
        .lines()
        .filter(|l| l.split_whitespace().next() == Some(tag))
        .map(|l| l.split_whitespace().collect())
        .collect()
}

fn hex(word: &str) -> u32 {
    u32::from_str_radix(word, 16).unwrap_or_else(|e| panic!("hex word {word:?}: {e}"))
}

fn float(word: &str) -> f32 {
    f32::from_bits(hex(word))
}

fn int(word: &str) -> i64 {
    word.parse().unwrap_or_else(|e| panic!("int {word:?}: {e}"))
}

fn uint(word: &str) -> u64 {
    word.parse()
        .unwrap_or_else(|e| panic!("uint {word:?}: {e}"))
}

fn vec3(r: &[&str], at: usize) -> FVector {
    FVector {
        x: float(r[at]),
        y: float(r[at + 1]),
        z: float(r[at + 2]),
    }
}

// ---------------------------------------------------------------------------
// Engine stand-ins (`Hp2RenderServices`), as in `light_kernels_replay.rs`
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

fn kernel_floats(tag: &'static str) -> Vec<f32> {
    let line = KERNELS
        .lines()
        .find(|l| l.split_whitespace().next() == Some(tag))
        .unwrap_or_else(|| panic!("kernels fixture has no {tag} line"));
    line.split_whitespace().skip(1).map(float).collect()
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
        sqrt_man: kernel_floats("GMATH_SQRTMAN"),
        div_sqrt_man: kernel_floats("GMATH_DIVSQRTMAN"),
        div_sqrt_exp: kernel_floats("GMATH_DIVSQRTEXP"),
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

// `GMath.SqrtApprox(F)` (UnMath.h, non-asm branch).
unsafe extern "C" fn gmath_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let a = ((bits >> 13) & 2047) as usize;
    let b = bits & 0x7F00_0000;
    f32::from_bits(tables().sqrt_man[a].to_bits().wrapping_add(b >> 1))
}

// `GMath.DivSqrtApprox(F)` (UnMath.h `Power`).
unsafe extern "C" fn gmath_div_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let t = tables();
    t.div_sqrt_man[((bits >> 13) & 1023) as usize] * t.div_sqrt_exp[(bits >> 23) as usize]
}

// Services the fog merge never calls. They return NaN (or do nothing), so any
// call that did happen would make the bit comparison fail.
unsafe extern "C" fn unused_frand() -> f32 {
    f32::NAN
}

unsafe extern "C" fn unused_unary(_f: f32) -> f32 {
    f32::NAN
}

unsafe extern "C" fn unused_log_warning(_text: *const u8, _len: usize) {}

fn services() -> Hp2RenderServices {
    Hp2RenderServices {
        frand: unused_frand,
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
// Tests
// ---------------------------------------------------------------------------

#[test]
fn dims_replay() {
    for r in records("dims") {
        assert_eq!(r.len(), 20, "dims token count");
        let uc = int(r[1]) as i32;
        let vc = int(r[2]) as i32;
        let stamp = uint(r[3]);
        let d = dims(uc, vc, stamp);
        let words = [
            d.u_bits,
            d.v_bits,
            d.u_size,
            d.v_size,
            d.shadow_mask_u,
            d.shadow_mask_space,
            d.shadow_skip,
            d.shadow_alloc,
            d.illum_alloc,
            d.merged_words,
            d.stamp_words,
        ];
        for (k, got) in words.iter().enumerate() {
            let oracle = int(r[4 + k]);
            let expected = if k == 7 && vc == 1 {
                // `ShadowMapGen` writes the source convolution's third channel
                // into a following row. The original dimension record is the
                // logical map size; the runtime allocation must include it.
                oracle.max(i64::from(d.shadow_mask_u) * 16)
            } else {
                oracle
            };
            assert_eq!(*got as i64, expected, "dims {uc}x{vc} int field {k}");
        }
        let bytes = [
            d.fog_alloc,
            d.static_alloc,
            d.dyn_alloc,
            d.illum_cache_alloc,
            d.tail_skip,
        ];
        for (k, got) in bytes.iter().enumerate() {
            assert_eq!(*got, uint(r[15 + k]), "dims {uc}x{vc} u64 field {k}");
        }
    }
}

#[test]
fn single_row_surface_allocation_covers_shadow_spill() {
    let d = dims(1, 1, 28);
    assert_eq!(d.shadow_alloc, 16);

    let filter: FilterTab = [[1; 4]; 128];
    let mut shadow = vec![0u32; d.shadow_alloc as usize / size_of::<u32>()];
    shadow_map_gen(
        &filter,
        d.shadow_mask_u as usize,
        1,
        Some(&[0]),
        &mut shadow,
    );

    // The original third convolution channel lands in the following row.
    assert_eq!(&shadow[2..], &[1, 1]);
}

#[test]
fn kind_replay() {
    for r in records("kind") {
        let opt = uint(r[1]) as u32;
        let spatial = int(r[2]) != 0;
        assert_eq!(
            light_kind(opt, spatial),
            int(r[3]) as u32,
            "kind opt {opt} spatial {spatial}"
        );
    }
}

#[test]
fn dyn_replay() {
    for r in records("dyn") {
        let has = int(r[1]) != 0;
        let stored = int(r[2]);
        let now = int(r[3]);
        assert_eq!(
            dyn_cache_outcome(has, stored, now),
            uint(r[4]) as u32,
            "dyn has {has} stored {stored} now {now}"
        );
    }
}

fn stamp_at(r: &[&str], at: usize) -> MoverStamp {
    MoverStamp {
        i_leaf: int(r[at]) as i32,
        location: [float(r[at + 1]), float(r[at + 2]), float(r[at + 3])],
        rotation: [
            int(r[at + 4]) as i32,
            int(r[at + 5]) as i32,
            int(r[at + 6]) as i32,
        ],
    }
}

#[test]
fn stamp_replay() {
    for r in records("stamp") {
        // stamp LEAF X Y Z PITCH YAW ROLL  LEAF X Y Z PITCH YAW ROLL RESULT
        assert_eq!(r.len(), 16, "stamp token count");
        let stored = stamp_at(&r, 1);
        let current = stamp_at(&r, 8);
        assert_eq!(
            mover_stamp_changed(&stored, &current),
            int(r[15]) != 0,
            "stamp {stored:?} vs {current:?}"
        );
    }
}

#[test]
fn dot_replay() {
    for r in records("dot") {
        // dot LOC(3) ORIGIN(3) ZAXIS(3) RESULT
        assert_eq!(r.len(), 11, "dot token count");
        let loc = vec3(&r, 1);
        let origin = vec3(&r, 4);
        let z_axis = vec3(&r, 7);
        assert_eq!(
            dot_positive(loc, origin, z_axis),
            int(r[10]) != 0,
            "dot {loc:?} {origin:?} {z_axis:?}"
        );
    }
}

#[test]
fn ambient_replay() {
    for r in records("ambient") {
        let got = ambient_word([float(r[1]), float(r[2]), float(r[3])]);
        assert_eq!(got, hex(r[4]), "ambient {} {} {}", r[1], r[2], r[3]);
    }
}

#[test]
fn fill_replay() {
    for r in records("fill") {
        // fill X Y Z UC VC USIZE words...
        let ambient = [float(r[1]), float(r[2]), float(r[3])];
        let uc = int(r[4]) as i32;
        let vc = int(r[5]) as i32;
        let us = int(r[6]) as i32;
        let want: Vec<u32> = r[7..].iter().map(|w| hex(w)).collect();
        assert_eq!(want.len(), (us * vc) as usize, "fill word count");
        let mut dest = vec![0u32; want.len()];
        // SAFETY: `dest` holds `us * vc` words, which covers `us*(vc-1)+uc`.
        unsafe { fill_ambient(ambient, uc, vc, us, dest.as_mut_ptr()) };
        assert_eq!(dest, want, "fill {uc}x{vc} stride {us}");
    }
}

#[test]
fn copy_replay() {
    for r in records("copy") {
        // copy UC VC USIZE words...
        let uc = int(r[1]) as i32;
        let vc = int(r[2]) as i32;
        let us = int(r[3]) as i32;
        let want: Vec<u32> = r[4..].iter().map(|w| hex(w)).collect();
        assert_eq!(want.len(), (us * vc) as usize, "copy word count");
        let src: Vec<u32> = (0..want.len() as u32).map(|i| i * 7 + 3).collect();
        let mut dst = vec![0u32; want.len()];
        // SAFETY: both buffers hold `us * vc` words, which covers the rows read and written.
        unsafe { copy_static_rows(src.as_ptr(), dst.as_mut_ptr(), uc, vc, us) };
        assert_eq!(dst, want, "copy {uc}x{vc} stride {us}");
    }
}

fn fog_fv(v: &[f32], at: usize) -> FVector {
    FVector {
        x: v[at],
        y: v[at + 1],
        z: v[at + 2],
    }
}

#[test]
fn fog_replay() {
    // ByteFog: the table FLightManager::Init builds (UnLight.cpp:382-384), via the kernel.
    let mut byte_fog = [0u8; BYTE_FOG_LEN];
    init_byte_fog(&mut byte_fog);
    let palette: Vec<[u8; 4]> = (0..256usize)
        .map(|i| {
            [
                ((i * 7 + 1) & 127) as u8,
                ((i * 11 + 3) & 127) as u8,
                ((i * 13 + 5) & 127) as u8,
                ((i * 17 + 2) & 127) as u8,
            ]
        })
        .collect();
    let services = services();
    let g = GMath::new(&services);

    let (mut changed, mut virgin_nonzero) = (0usize, 0usize);
    for r in records("fog") {
        // fog CASE VIRGIN UC VC USIZE AMB INSIDE <24 v[]> <5 scalars> | <texel bytes>
        assert_eq!(r[37], "|", "fog separator");
        let virgin = int(r[2]) != 0;
        let uc = int(r[3]) as i32;
        let vc = int(r[4]) as i32;
        let us = int(r[5]) as i32;
        let amb = int(r[6]) != 0;
        let inside = int(r[7]) != 0;
        let v: Vec<f32> = r[8..32].iter().map(|w| float(w)).collect();
        let s: Vec<f32> = r[32..37].iter().map(|w| float(w)).collect();
        let want: Vec<u8> = r[38..]
            .iter()
            .map(|w| u8::from_str_radix(w, 16).expect("texel byte"))
            .collect();
        let texels = (us * vc) as usize;
        assert_eq!(want.len(), texels * 4, "fog texel byte count");

        let p = FogLightInputs {
            coords: Coords {
                origin: fog_fv(&v, 0),
                x_axis: fog_fv(&v, 3),
                y_axis: fog_fv(&v, 6),
                z_axis: fog_fv(&v, 9),
            },
            vertex_base: fog_fv(&v, 12),
            vertex_du: fog_fv(&v, 15),
            vertex_dv: fog_fv(&v, 18),
            vol: VolumetricParams {
                location: fog_fv(&v, 21),
                vol_radius_squared: s[0],
                rvol_radius: s[1],
                rvol_radius_squared: s[2],
                vol_brightness: s[3],
                location_size_squared: s[4],
                vol_inside: inside,
                ambient: amb,
            },
            uclamp: uc,
            vclamp: vc,
            u_size: us,
        };

        // Pre-fill: texel t, byte k (memory order) = (t*31+k*53+7)&127.
        let mut dest: Vec<[u8; 4]> = (0..texels)
            .map(|t| {
                let mut px = [0u8; 4];
                for (k, b) in px.iter_mut().enumerate() {
                    *b = ((t * 31 + k * 53 + 7) & 127) as u8;
                }
                px
            })
            .collect();
        let before = dest.clone();

        let mut fog_rejection = 0i32;
        // SAFETY: `palette` holds 256 texels, `byte_fog` holds BYTE_FOG_LEN entries
        // (indices stay below it because every A and R value is at most 127), and
        // `dest` holds `us * vc` texels, which covers the rows written.
        unsafe {
            fog_light(
                &g,
                &p,
                virgin,
                palette.as_ptr(),
                byte_fog.as_ptr(),
                dest.as_mut_ptr(),
                &mut fog_rejection,
            )
        };
        let got: Vec<u8> = dest.iter().flatten().copied().collect();
        assert_eq!(got, want, "fog case {} virgin {virgin}", r[1]);
        if virgin {
            virgin_nonzero += dest.iter().filter(|px| **px != [0u8; 4]).count();
        } else {
            changed += before.iter().zip(&dest).filter(|(a, b)| a != b).count();
        }
    }

    // Vacuity: the merge must change texels, and the virgin path must write nonzero texels.
    assert!(changed > 0, "fog merge changed no texel");
    assert!(virgin_nonzero > 0, "fog virgin path wrote only zero texels");
}

#[test]
fn clog_replay() {
    let recs = records("clog");
    assert_eq!(recs.len(), 21, "clog record count");
    for r in recs {
        assert_eq!(r.len(), 3, "clog token count");
        let arg = uint(r[1]) as u32;
        assert_eq!(u64::from(ceil_log_two(arg)), uint(r[2]), "clog {arg}");
    }
}
/// FNV-1a over the light-map bytes, matching `surf_fnv1a` in `sec_surf_driver.inc`.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(1469598103934665603u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(1099511628211)
    })
}

const DRIVER_FIXTURE: &str = include_str!("fixtures/light_surf_driver.txt");

#[derive(Default)]
struct SurfDriverState {
    cache_id: Option<u64>,
    cache: Option<Box<[u32]>>,
    gmem: Vec<Box<[u8]>>,
    ambient: [f32; 3],
}

static SURF_DRIVER_STATE: LazyLock<Mutex<SurfDriverState>> =
    LazyLock::new(|| Mutex::new(SurfDriverState::default()));
static SURF_DRIVER_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static SURF_LINK_ADDED: LazyLock<Mutex<Vec<usize>>> = LazyLock::new(|| Mutex::new(Vec::new()));

unsafe extern "C" fn unused_light_service() {}

unsafe extern "C" fn surf_add_light(_actor: *mut c_void, light: *mut c_void) -> u32 {
    SURF_LINK_ADDED.lock().unwrap().push(light as usize);
    0
}

unsafe extern "C" fn surf_map_setup(ctx: *mut c_void, which: u32) {
    let params = unsafe { &mut *(ctx as *mut SurfSetupParams) };
    if which == 0 && !params.out_light_map.is_null() {
        unsafe { *params.out_light_map = params.light_map };
    } else if which == 1 && params.fog_realtime_changed != 0 && !params.out_fog_map.is_null() {
        unsafe { *params.out_fog_map = params.fog_map };
    }
}

unsafe extern "C" fn surf_zone_ambient(out: *mut c_void) {
    let ambient = SURF_DRIVER_STATE.lock().unwrap().ambient;
    unsafe { std::ptr::copy_nonoverlapping(ambient.as_ptr(), out.cast::<f32>(), 3) };
}

unsafe extern "C" fn surf_cache_get(id: u64) -> *mut c_void {
    let mut state = SURF_DRIVER_STATE.lock().unwrap();
    if state.cache_id == Some(id) {
        state.cache.as_mut().unwrap().as_mut_ptr().cast()
    } else {
        std::ptr::null_mut()
    }
}

unsafe extern "C" fn surf_cache_create(id: u64, size: u64, _pad: u64, _check: u32) -> *mut c_void {
    let words = size.div_ceil(4) as usize;
    let mut state = SURF_DRIVER_STATE.lock().unwrap();
    state.cache_id = Some(id);
    state.cache = Some(vec![0u32; words].into_boxed_slice());
    state.cache.as_mut().unwrap().as_mut_ptr().cast()
}
unsafe extern "C" fn surf_gmem(bytes: u64) -> *mut c_void {
    let mut state = SURF_DRIVER_STATE.lock().unwrap();
    let mut memory = vec![0u8; bytes as usize].into_boxed_slice();
    let ptr = memory.as_mut_ptr().cast();
    state.gmem.push(memory);
    ptr
}
unsafe extern "C" fn surf_gmem_mark(_slot: u32, _pop: u32) {}

fn unused<T: Copy>(f: unsafe extern "C" fn()) -> T {
    assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&f));
    unsafe { std::mem::transmute_copy(&f) }
}

/// `light_count`, `mark_volumetric` and `vtric_commit` now read `light_manager::State` directly
/// instead of going through a mockable leaf, so this test must set up the precondition
/// `FLightManager::SetActor` always establishes in the real engine before `SetupForSurf` runs: an
/// empty `FirstLight` list bounded by `capacity` entries.
unsafe fn reset_light_manager(capacity: u32) {
    unsafe {
        let s = light_manager::state();
        (*s).LastLight = std::ptr::addr_of_mut!((*s).FirstLight).cast();
        (*s).FinalLight = std::ptr::addr_of_mut!((*s).FirstLight)
            .cast::<light_manager::LightInfo>()
            .add(capacity as usize);
    }
}

fn surf_driver_services() -> Hp2LightServices {
    Hp2LightServices {
        check_failed: unused(unused_light_service),
        line_check: unused(unused_light_service),
        add_light: surf_add_light,
        sender_alive: unused(unused_light_service),
        owner_life_fraction: unused(unused_light_service),
        world_light_radius: unused(unused_light_service),
        world_volumetric_radius: unused(unused_light_service),
        view_vector: unused(unused_light_service),
        map_uncoords: unused(unused_light_service),
        global_hsv: unused(unused_light_service),
        cache_get: unused(unused_light_service),
        cache_create: unused(unused_light_service),
        stat_clock: unused(unused_light_service),
        stat_unclock: unused(unused_light_service),
        surf_map_setup,
        surf_zone_ambient,
        surf_cache_get,
        surf_cache_create,
        surf_gmem,
        surf_gmem_mark,
        surf_light_compute: unused(unused_light_service),
        surf_light_spatial_fx: unused(unused_light_service),
        surf_shadow_gen: unused(unused_light_service),
        random_base_data: unused(unused_light_service),
        random_data: unused(unused_light_service),
        light_compute: unused(unused_light_service),
        stat_vtric_inc: unused(unused_light_service),
        stat_mesh_light_add: unused(unused_light_service),
        unlock_one: unused(unused_light_service),
        mark_pop: unused(unused_light_service),
    }
}

#[test]
fn full_driver_static_cache_miss_and_hit_replay() {
    let _lock = SURF_DRIVER_TEST_LOCK.lock().unwrap();
    let records: Vec<Vec<&str>> = DRIVER_FIXTURE
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(records.len(), 2);
    let ambient = [
        float(records[0][10]),
        float(records[0][11]),
        float(records[0][12]),
    ];
    *SURF_DRIVER_STATE.lock().unwrap() = SurfDriverState {
        ambient,
        ..SurfDriverState::default()
    };

    let mut params: SurfSetupParams = unsafe { std::mem::zeroed() };
    let (mut light_map, mut fog_map, mut light_mip, mut fog_mip) = (0u32, 0u32, 0u32, 0u32);
    let (mut out_light_map, mut out_fog_map) = (std::ptr::null_mut(), std::ptr::null_mut());
    params.light_map = (&mut light_map as *mut u32).cast();
    params.fog_map = (&mut fog_map as *mut u32).cast();
    params.light_mip = (&mut light_mip as *mut u32).cast();
    params.fog_mip = (&mut fog_mip as *mut u32).cast();
    params.out_light_map = &mut out_light_map;
    params.out_fog_map = &mut out_fog_map;
    params.i_light_map = 0;
    params.zone_id = 0;
    params.uclamp = 3;
    params.vclamp = 2;
    params.static_light_index = -1;
    params.model_cache_index = 0;
    params.light_capacity = 256;

    let light_services = surf_driver_services();
    let render_services = services();
    unsafe { reset_light_manager(params.light_capacity) };
    for (index, record) in records.iter().take(2).enumerate() {
        assert_eq!(record.len(), 13);
        assert_eq!(
            record[0],
            if index == 0 {
                "driver_static"
            } else {
                "driver_static_hit"
            }
        );
        unsafe { setup_for_surf(&mut params, &light_services, &render_services) };
        assert_eq!(params.dims.u_size, int(record[1]) as i32);
        assert_eq!(params.dims.v_size, int(record[2]) as i32);
        assert_eq!(params.light_realtime_changed, int(record[5]) as u32);
        assert_eq!(params.light_cache_id, uint(record[7]));
        assert_eq!(out_light_map, params.light_map);
        assert!(out_fog_map.is_null());
        let actual = unsafe { *(params.light_data.cast::<u32>()) };
        assert_eq!(
            u64::from(actual),
            uint(record[8]),
            "static texel, case {index}"
        );
        let len = (params.dims.u_size * params.dims.v_size * 4) as usize;
        let bytes = unsafe { std::slice::from_raw_parts(params.light_data.cast::<u8>(), len) };
        let hash = u64::from_str_radix(record[9], 16).unwrap();
        assert_eq!(fnv1a(bytes), hash, "light map hash, case {index}");
    }
}

#[test]
fn surface_actor_lists_use_f_actor_link_reader() {
    let _lock = SURF_DRIVER_TEST_LOCK.lock().unwrap();
    *SURF_DRIVER_STATE.lock().unwrap() = SurfDriverState::default();
    SURF_LINK_ADDED.lock().unwrap().clear();

    let mut actor_storage: [Box<AActor>; 2] = [unsafe { Box::new(std::mem::zeroed()) }, unsafe {
        Box::new(std::mem::zeroed())
    }];
    let actors = [
        (&mut *actor_storage[0] as *mut AActor).cast::<c_void>(),
        (&mut *actor_storage[1] as *mut AActor).cast::<c_void>(),
    ];
    let links = [
        FActorLink {
            actor: actors[0],
            next: std::ptr::null_mut(),
        },
        FActorLink {
            actor: actors[1],
            next: std::ptr::null_mut(),
        },
    ];
    let mut links = links;
    links[0].next = std::ptr::addr_of_mut!(links[1]);
    let mut params: SurfSetupParams = unsafe { std::mem::zeroed() };
    let (mut light_map, mut fog_map, mut light_mip, mut fog_mip) = (0u32, 0u32, 0u32, 0u32);
    let (mut out_light_map, mut out_fog_map) = (std::ptr::null_mut(), std::ptr::null_mut());
    params.light_map = (&mut light_map as *mut u32).cast();
    params.fog_map = (&mut fog_map as *mut u32).cast();
    params.light_mip = (&mut light_mip as *mut u32).cast();
    params.fog_mip = (&mut fog_mip as *mut u32).cast();
    params.out_light_map = &mut out_light_map;
    params.out_fog_map = &mut out_fog_map;
    params.mover = 0x1235usize as *mut c_void;
    params.mover_dynamic_light = 1;
    params.mover_leaf = 0;
    params.mover_special_lit = 0;
    params.model_has_leaves = 1;
    params.leaf_permeating = -1;
    params.surf_lights = (&links[0] as *const FActorLink).cast_mut().cast();
    params.volumetrics = params.surf_lights;
    params.light_capacity = 256;

    unsafe {
        reset_light_manager(params.light_capacity);
        setup_for_surf(&mut params, &surf_driver_services(), &services());
    }

    let expected_actors = [actors[0] as usize, actors[1] as usize];
    // `SetupForSurf` only reaches `FActorLink` lists here; `SURF_LINK_ADDED` alone confirms the
    // `surf_lights` walk used the correct reader (the wrong one would leave the actor list empty).
    assert_eq!(*SURF_LINK_ADDED.lock().unwrap(), expected_actors);
    // `mark_volumetric` now writes `FirstLight` directly: confirm both `volumetrics` actors were
    // appended and flagged, in list order, in place of the old mock's call tracking.
    let marked_count = unsafe { light_manager::light_count() };
    assert_eq!(
        marked_count,
        expected_actors.len(),
        "FirstLight count after mark_volumetrics"
    );
    let marked: Vec<usize> = (0..marked_count)
        .map(|i| unsafe {
            let info = &(*light_manager::state()).FirstLight[i];
            assert_eq!(info.IsVolumetric, 1, "FirstLight[{i}].IsVolumetric");
            info.Actor as usize
        })
        .collect();
    assert_eq!(marked, expected_actors);
}
