//! FFI exports for the fire slice: `InitTables`, `SpeedRand`, the three
//! diffusion kernels, the render-table fill, and the Blue Laguna palette.

use hp2_ffi_support::guard;
use hp2_fire::abi::FireTraceHooks;
use hp2_fire::fire;
use hp2_fire::palette::{self, FireRgb};

/// `InitTables` after the engine-side RNG and sine evaluation. `sine` must hold
/// 256 floats and `rng_seed` 512 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_init_tables(
    trace: FireTraceHooks,
    sine: *const f64,
    sine_len: usize,
    rng_seed: *const u8,
    rng_seed_len: usize,
    before: u64,
    after: u64,
) {
    guard("hp2_rs_fire_init_tables", (), move || unsafe {
        if sine.is_null() || rng_seed.is_null() {
            return;
        }
        assert_eq!(sine_len, 256, "hp2_rs_fire_init_tables: sine_len");
        assert_eq!(rng_seed_len, 512, "hp2_rs_fire_init_tables: rng_seed_len");
        let sine: &[f64; 256] = std::slice::from_raw_parts(sine, 256)
            .try_into()
            .expect("256 doubles");
        let rng: &[u8; 512] = std::slice::from_raw_parts(rng_seed, 512)
            .try_into()
            .expect("512 bytes");
        fire::init_tables(trace, sine, rng, before, after);
    })
}

/// Copies the Rust-held phase tables into the C++ `PhaseTable`,
/// `SignedPhaseTable` and `LightPhaseTable` arrays. Remove with the last C++ reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_read_phase_tables(
    phase_out: *mut u8,
    signed_out: *mut u8,
    light_out: *mut u8,
) {
    guard("hp2_rs_fire_read_phase_tables", (), move || unsafe {
        if phase_out.is_null() || signed_out.is_null() || light_out.is_null() {
            return;
        }
        let phase: &mut [u8; 256] = std::slice::from_raw_parts_mut(phase_out, 256)
            .try_into()
            .expect("256 bytes");
        let signed: &mut [u8; 256] = std::slice::from_raw_parts_mut(signed_out, 256)
            .try_into()
            .expect("256 bytes");
        let light: &mut [u8; 256] = std::slice::from_raw_parts_mut(light_out, 256)
            .try_into()
            .expect("256 bytes");
        fire::read_phase_tables(phase, signed, light);
    })
}

/// `SpeedRand()`: advances the speed-RNG and returns the next byte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_speed_rand(trace: FireTraceHooks) -> u8 {
    guard("hp2_rs_fire_speed_rand", 0, move || fire::speed_rand(trace))
}

/// `CalculateFire` over a `xdim * ydim` byte bitmap, with a 1024-byte render table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_calculate_fire(
    bitmap: *mut u8,
    bitmap_len: usize,
    render_table: *const u8,
    render_table_len: usize,
    xdim: u32,
    ydim: u32,
) {
    guard("hp2_rs_fire_calculate_fire", (), move || unsafe {
        if bitmap.is_null() || render_table.is_null() {
            return;
        }
        let bm = std::slice::from_raw_parts_mut(bitmap, bitmap_len);
        let rt = std::slice::from_raw_parts(render_table, render_table_len);
        fire::calculate_fire(bm, rt, xdim, ydim);
    })
}

/// `CalcWrapFire` over a `xdim * ydim` byte bitmap, with a 1024-byte render table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_calc_wrap_fire(
    bitmap: *mut u8,
    bitmap_len: usize,
    render_table: *const u8,
    render_table_len: usize,
    xdim: u32,
    ydim: u32,
) {
    guard("hp2_rs_fire_calc_wrap_fire", (), move || unsafe {
        if bitmap.is_null() || render_table.is_null() {
            return;
        }
        let bm = std::slice::from_raw_parts_mut(bitmap, bitmap_len);
        let rt = std::slice::from_raw_parts(render_table, render_table_len);
        fire::calc_wrap_fire(bm, rt, xdim, ydim);
    })
}

/// `CalcSlowFire` over a `xdim * ydim` byte bitmap, with a 1024-byte render table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_calc_slow_fire(
    bitmap: *mut u8,
    bitmap_len: usize,
    render_table: *const u8,
    render_table_len: usize,
    xdim: u32,
    ydim: u32,
) {
    guard("hp2_rs_fire_calc_slow_fire", (), move || unsafe {
        if bitmap.is_null() || render_table.is_null() {
            return;
        }
        let bm = std::slice::from_raw_parts_mut(bitmap, bitmap_len);
        let rt = std::slice::from_raw_parts(render_table, render_table_len);
        fire::calc_slow_fire(bm, rt, xdim, ydim);
    })
}

/// Fill `table[0..1024]` from `RenderHeat` (`UFireTexture::PostLoad`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_fill_render_table(
    render_heat: u8,
    table: *mut u8,
    len: usize,
) {
    guard("hp2_rs_fire_fill_render_table", (), move || unsafe {
        if table.is_null() {
            return;
        }
        let out = std::slice::from_raw_parts_mut(table, len);
        fire::fill_render_table(render_heat, out);
    })
}

/// Write 256 Blue Laguna entries to `out` (`BlueLagunaPalette`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_blue_laguna_palette(out: *mut FireRgb, len: usize) {
    guard("hp2_rs_fire_blue_laguna_palette", (), move || unsafe {
        if out.is_null() {
            return;
        }
        let dst = std::slice::from_raw_parts_mut(out, len);
        palette::blue_laguna_palette(dst);
    })
}
