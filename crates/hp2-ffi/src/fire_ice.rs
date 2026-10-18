//! FFI exports for the ice port (`crates/hp2-fire/src/ice.rs`). Raw pointers
//! become slices here and nowhere else.

use hp2_ffi_support::guard;
use hp2_fire::abi::FireTraceHooks;
use hp2_fire::ice::{FireIceResample, FireIceState, IceRead};

/// Converts a `(ptr, len)` input. A null pointer means the C++ texture is
/// absent (`None`). A pointer equal to the destination bitmap means the input
/// is the bitmap itself (`IceRead::Dest`), so no shared slice aliases `&mut`.
///
/// # Safety
/// A non-null `ptr` must be valid for reads of `len` bytes for `'a`.
unsafe fn input<'a>(ptr: *const u8, len: usize, bitmap: *const u8) -> Option<IceRead<'a>> {
    if ptr.is_null() {
        return None;
    }
    if ptr == bitmap {
        return Some(IceRead::Dest);
    }
    Some(IceRead::Separate(unsafe {
        std::slice::from_raw_parts(ptr, len)
    }))
}

/// # Safety
/// A non-null `ptr` must be valid for reads and writes of `len` bytes for `'a`,
/// and must not be aliased by any other live reference.
unsafe fn bitmap_mut<'a>(ptr: *mut u8, len: usize) -> Option<&'a mut [u8]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// Replaces `UIceTexture::MoveIcePosition`'s algorithm.
///
/// # Safety
/// `state` must be null or point to a live `FireIceState`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_move_ice_position(
    trace: FireTraceHooks,
    state: *mut FireIceState,
    delta_time: f32,
) {
    guard("hp2_rs_fire_move_ice_position", (), move || unsafe {
        if let Some(st) = state.as_mut() {
            hp2_fire::ice::move_ice_position(trace, st, delta_time);
        }
    });
}

/// Replaces `UIceTexture::BlitTexIce`.
///
/// # Safety
/// Each non-null pointer must be valid for its length. `bitmap` must not alias
/// `source` or `glass` except by being equal to them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_blit_tex_ice(
    trace: FireTraceHooks,
    state: *const FireIceState,
    bitmap: *mut u8,
    bitmap_len: usize,
    source: *const u8,
    source_len: usize,
    glass: *const u8,
    glass_len: usize,
) {
    guard("hp2_rs_fire_blit_tex_ice", (), move || unsafe {
        let (Some(st), Some(dst)) = (state.as_ref(), bitmap_mut(bitmap, bitmap_len)) else {
            return;
        };
        let bitmap_const = bitmap as *const u8;
        let (Some(tex), Some(gl)) = (
            input(source, source_len, bitmap_const),
            input(glass, glass_len, bitmap_const),
        ) else {
            return;
        };
        hp2_fire::ice::blit_tex_ice(trace, st, dst, tex, gl);
    });
}

/// Replaces `UIceTexture::BlitIceTex`.
///
/// # Safety
/// Same as `hp2_rs_fire_blit_tex_ice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_blit_ice_tex(
    trace: FireTraceHooks,
    state: *const FireIceState,
    bitmap: *mut u8,
    bitmap_len: usize,
    source: *const u8,
    source_len: usize,
    glass: *const u8,
    glass_len: usize,
) {
    guard("hp2_rs_fire_blit_ice_tex", (), move || unsafe {
        let (Some(st), Some(dst)) = (state.as_ref(), bitmap_mut(bitmap, bitmap_len)) else {
            return;
        };
        let bitmap_const = bitmap as *const u8;
        let (Some(tex), Some(gl)) = (
            input(source, source_len, bitmap_const),
            input(glass, glass_len, bitmap_const),
        ) else {
            return;
        };
        hp2_fire::ice::blit_ice_tex(trace, st, dst, tex, gl);
    });
}

/// Replaces the `PostLoad` resample loop that fills `LocalSourceBitmap`.
///
/// # Safety
/// Same as `hp2_rs_fire_blit_tex_ice`. `params` must be null or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_ice_resample_source(
    trace: FireTraceHooks,
    params: *const FireIceResample,
    bitmap: *mut u8,
    bitmap_len: usize,
    source: *const u8,
    source_len: usize,
) {
    guard("hp2_rs_fire_ice_resample_source", (), move || unsafe {
        let (Some(p), Some(dst)) = (params.as_ref(), bitmap_mut(bitmap, bitmap_len)) else {
            return;
        };
        let bitmap_const = bitmap as *const u8;
        let Some(src) = input(source, source_len, bitmap_const) else {
            return;
        };
        hp2_fire::ice::resample_source(trace, p, dst, src);
    });
}
