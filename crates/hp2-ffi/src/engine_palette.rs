//! Heckbert quantiser export (`crates/hp2-engine/src/palette.rs`). The C++
//! `HeckbertQuantize` in `Engine/Src/palette.cpp` forwards here.

use std::ffi::c_void;

use hp2_engine::palette::{self, PaletteServices};
use hp2_ffi_support::guard;

/// `HeckbertQuantize` forward. `svc` is the C++ `Hp2PaletteServices` table.
///
/// # Safety
/// `svc` must point to a live `PaletteServices`. The buffers must satisfy
/// `palette::heckbert_quantize`'s contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_palette_heckbert_quantize(
    svc: *const c_void,
    srcdata: *mut c_void,
    dstdata: *mut u8,
    paldata: *mut u16,
    srcformat: i32,
    nentries: i32,
    width: i32,
    height: i32,
    has_colourkey: i32,
    keyval: u32,
    generate_mipmaps: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_palette_heckbert_quantize",
        0,
        move || unsafe {
            let Some(svc) = svc.cast::<PaletteServices>().as_ref() else {
                return 0;
            };
            palette::heckbert_quantize(
                svc,
                srcdata,
                dstdata,
                paldata,
                srcformat,
                nentries,
                width,
                height,
                has_colourkey != 0,
                keyval,
                generate_mipmaps != 0,
            )
        },
    )
}
