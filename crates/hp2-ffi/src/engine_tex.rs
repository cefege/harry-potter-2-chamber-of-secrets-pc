//! Guarded C exports for `Engine/Src/UnTex.cpp` (see `hp2_engine::tex`).
//! Palette pointers are `FColor[NUM_PAL_COLORS]`; colours cross the boundary
//! as `FColor::Int4` (`R` in the low byte).

use std::ffi::c_void;

use hp2_abi::FColor;
use hp2_engine::tex::{self, MipLevel, NUM_PAL_COLORS};
use hp2_ffi_support::guard;

/// One mip level of a `UTexture`: `DataArray` plus its dimensions.
#[repr(C)]
pub struct TexMipDesc {
    pub data: *mut u8,
    pub len: i32,
    pub size_u: i32,
    pub size_v: i32,
}

unsafe fn palette<'a>(p: *const c_void) -> &'a [FColor] {
    unsafe { std::slice::from_raw_parts(p as *const FColor, NUM_PAL_COLORS) }
}

unsafe fn descs<'a>(mips: *const TexMipDesc, count: i32) -> &'a [TexMipDesc] {
    if count <= 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(mips, count as usize) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_fget_hsv(h: u8, s: u8, v: u8, out: *mut f32) {
    guard("hp2_rs_tex_fget_hsv", (), move || unsafe {
        let p = tex::fget_hsv(h, s, v);
        std::ptr::copy_nonoverlapping(p.as_ptr(), out, 4);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_best_match(
    palette_ptr: *const c_void,
    color: u32,
    first: u32,
) -> u8 {
    guard("hp2_rs_tex_best_match", 0, move || unsafe {
        tex::best_match(
            palette(palette_ptr),
            tex::color_from_int4(color),
            first as usize,
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_fix_palette(palette_ptr: *mut c_void) {
    guard("hp2_rs_tex_fix_palette", (), move || unsafe {
        let colors = std::slice::from_raw_parts_mut(palette_ptr as *mut FColor, NUM_PAL_COLORS);
        tex::fix_palette(colors);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_mip_zero(
    palette_ptr: *const c_void,
    data: *const u8,
    len: i32,
    p8: u32,
) -> u32 {
    guard("hp2_rs_tex_mip_zero", 0, move || unsafe {
        let bytes: &[u8] = if data.is_null() || len <= 0 {
            &[]
        } else {
            std::slice::from_raw_parts(data, len as usize)
        };
        let c = tex::mip_zero(palette(palette_ptr), bytes, p8 != 0);
        tex::color_to_int4(c)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_color_range(
    palette_ptr: *const c_void,
    mips: *const TexMipDesc,
    count: i32,
) -> u32 {
    guard("hp2_rs_tex_color_range", 0, move || unsafe {
        let pal = if palette_ptr.is_null() {
            None
        } else {
            Some(palette(palette_ptr))
        };
        let slices: Vec<&[u8]> = descs(mips, count)
            .iter()
            .map(|d| std::slice::from_raw_parts(d.data as *const u8, d.len as usize))
            .collect();
        tex::color_to_int4(tex::color_range(pal, &slices))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_create_mips_downsample(
    palette_ptr: *const c_void,
    mips: *mut TexMipDesc,
    count: i32,
    masked: u32,
) {
    guard("hp2_rs_tex_create_mips_downsample", (), move || unsafe {
        let pal = palette(palette_ptr);
        let mut levels: Vec<MipLevel> = descs(mips, count)
            .iter()
            .map(|d| MipLevel {
                data: std::slice::from_raw_parts_mut(d.data, d.len as usize),
                size_u: d.size_u,
                size_v: d.size_v,
            })
            .collect();
        tex::downsample_p8(pal, masked != 0, &mut levels);
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_tick_plan(
    max_frame_rate: f32,
    min_frame_rate: f32,
    accumulator: *mut f32,
    delta: f32,
) -> u32 {
    guard("hp2_rs_tex_tick_plan", 0, move || unsafe {
        tex::tick_plan(max_frame_rate, min_frame_rate, &mut *accumulator, delta)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_tex_prime_steps(prime_current: u32, prime_count: u32) -> u32 {
    guard("hp2_rs_tex_prime_steps", 0, move || {
        tex::prime_steps(prime_current as u8, prime_count as u8)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_tex_cache_max_color(
    format: u8,
    max: u32,
    data: *const u8,
    data_len: i32,
    usize_: i32,
    uclamp: i32,
    vclamp: i32,
) -> u32 {
    guard("hp2_rs_tex_cache_max_color", max, move || unsafe {
        let bytes: &[u8] = if data.is_null() || data_len <= 0 {
            &[]
        } else {
            std::slice::from_raw_parts(data, data_len as usize)
        };
        let c = tex::cache_max_color(
            format,
            tex::color_from_int4(max),
            bytes,
            usize_,
            uclamp,
            vclamp,
        );
        tex::color_to_int4(c)
    })
}
