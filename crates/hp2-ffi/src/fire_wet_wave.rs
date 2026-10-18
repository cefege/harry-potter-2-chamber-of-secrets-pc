//! FFI exports for the wet and wave texture algorithms in `UnFractal.cpp`.

use hp2_ffi_support::guard;
use hp2_fire::abi::FireTraceHooks;
use hp2_fire::wave::{self, FireWaveLight};
use hp2_fire::wet::{self, FireWetLayout, FireWetResample};

/// # Safety
/// `bitmap` must be valid for `bitmap_len` bytes of writes. `source` must be
/// valid for `source_len` bytes of reads unless `source_is_bitmap` is true, in
/// which case `source` is ignored and the source is the bitmap itself.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_apply_wet_texture(
    trace: FireTraceHooks,
    bitmap: *mut u8,
    bitmap_len: usize,
    source: *const u8,
    source_len: usize,
    source_is_bitmap: bool,
    layout: FireWetLayout,
) {
    guard("hp2_rs_fire_apply_wet_texture", (), move || unsafe {
        if bitmap.is_null() {
            return;
        }
        let bitmap = std::slice::from_raw_parts_mut(bitmap, bitmap_len);
        if source_is_bitmap {
            wet::apply_wet_texture(trace, bitmap, None, &layout);
        } else {
            if source.is_null() {
                return;
            }
            let source = std::slice::from_raw_parts(source, source_len);
            wet::apply_wet_texture(trace, bitmap, Some(source), &layout);
        }
    })
}

/// # Safety
/// `dst` must be valid for `dst_len` bytes of writes and `src` for `src_len`
/// bytes of reads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_wet_resample_source(
    trace: FireTraceHooks,
    dst: *mut u8,
    dst_len: usize,
    src: *const u8,
    src_len: usize,
    params: FireWetResample,
) {
    guard("hp2_rs_fire_wet_resample_source", (), move || unsafe {
        if dst.is_null() || src.is_null() {
            return;
        }
        let dst = std::slice::from_raw_parts_mut(dst, dst_len);
        let src = std::slice::from_raw_parts(src, src_len);
        wet::resample_wet_source(trace, dst, src, &params);
    })
}

/// # Safety
/// `render_table` must be valid for `render_table_len` bytes of writes; at
/// least 1024 entries are written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_wet_set_refraction_table(
    trace: FireTraceHooks,
    wave_amp: u8,
    render_table: *mut u8,
    render_table_len: usize,
) {
    guard("hp2_rs_fire_wet_set_refraction_table", (), move || unsafe {
        if render_table.is_null() {
            return;
        }
        let render_table = std::slice::from_raw_parts_mut(render_table, render_table_len);
        wet::set_refraction_table(trace, wave_amp, render_table);
    })
}

/// # Safety
/// `render_table` must be valid for `render_table_len` bytes of writes; at
/// least 1024 entries are written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_wave_set_wave_light(
    trace: FireTraceHooks,
    params: FireWaveLight,
    render_table: *mut u8,
    render_table_len: usize,
) {
    guard("hp2_rs_fire_wave_set_wave_light", (), move || unsafe {
        if render_table.is_null() {
            return;
        }
        let render_table = std::slice::from_raw_parts_mut(render_table, render_table_len);
        wave::set_wave_light(trace, &params, render_table);
    })
}
